//! Bounded, read-only discovery. Installed libraries are separate from running services.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Duration,
};
static FILES: OnceLock<Mutex<HashMap<String, PathBuf>>> = OnceLock::new();
static SCAN_CACHE: OnceLock<PathBuf> = OnceLock::new();
pub fn model_limit() -> u64 {
    if cfg!(target_os = "android") {
        3_000_000_000
    } else {
        10_000_000_000
    }
}
pub fn configure_scan_cache(path: PathBuf) {
    let _ = SCAN_CACHE.set(path);
}
pub fn endpoint(id: &str) -> Option<(u16, &str)> {
    if cfg!(target_os = "android") {
        return None;
    }
    for (prefix, port) in [("ollama:", 11434), ("lmstudio:", 1234), ("llamacpp:", 8080)] {
        if let Some(name) = id.strip_prefix(prefix) {
            if !name.is_empty() && name.len() <= 200 && !name.chars().any(char::is_control) {
                return Some((port, name));
            }
        }
    }
    None
}
pub fn scan_roots() -> Vec<PathBuf> {
    if cfg!(target_os = "android") {
        return vec![];
    }
    let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) else {
        return vec![];
    };
    let home = PathBuf::from(home);
    let mut roots = vec![
        home.join(".ollama/models"),
        home.join(".lmstudio/models"),
        home.join(".cache/lm-studio/models"),
        home.join(".cache/huggingface/hub"),
        home.join("Models"),
    ];
    for key in ["OLLAMA_MODELS", "HF_HUB_CACHE"] {
        if let Some(p) = std::env::var_os(key).filter(|v| !v.is_empty()) {
            roots.push(PathBuf::from(p));
        }
    }
    if let Some(p) = std::env::var_os("HF_HOME").filter(|v| !v.is_empty()) {
        roots.push(PathBuf::from(p).join("hub"));
    }
    if let Some(p) = std::env::var_os("XDG_CACHE_HOME").filter(|v| !v.is_empty()) {
        roots.push(PathBuf::from(p).join("huggingface/hub"));
    }
    if let Some(cache) = SCAN_CACHE.get() {
        if let Ok(raw) = std::fs::read(cache) {
            if raw.len() <= 65536 {
                if let Ok(extra) = serde_json::from_slice::<Vec<PathBuf>>(&raw) {
                    roots.extend(extra.into_iter().filter(|p| p.is_absolute()).take(8));
                }
            }
        }
    }
    let mut seen = HashSet::new();
    roots.retain(|p| seen.insert(p.clone()));
    roots
}
pub fn remember_folder(folder: &str) -> Result<(), String> {
    if cfg!(target_os = "android") {
        return Err(
            "Desktop model-library folders are not accessible in this Android build.".into(),
        );
    }
    let p = PathBuf::from(folder.trim());
    if !p.is_absolute() {
        return Err("Enter an absolute model-folder path.".into());
    }
    let p = p
        .canonicalize()
        .map_err(|_| "The model folder is not accessible.")?;
    if !p.is_dir() || p.parent().is_none() {
        return Err("Select a model folder, not an entire drive.".into());
    }
    let cache = SCAN_CACHE
        .get()
        .ok_or("Model scan settings are not initialized.")?;
    remember_folder_at(cache, p)
}
fn remember_folder_at(cache: &Path, p: PathBuf) -> Result<(), String> {
    let mut folders = std::fs::read(cache)
        .ok()
        .and_then(|v| serde_json::from_slice::<Vec<PathBuf>>(&v).ok())
        .unwrap_or_default();
    folders.retain(|v| v != &p);
    folders.insert(0, p);
    folders.truncate(8);
    if let Some(parent) = cache.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let raw = serde_json::to_vec(&folders).map_err(|e| e.to_string())?;
    let temp = cache.with_extension("pending");
    std::fs::write(&temp, raw).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, cache).map_err(|e| e.to_string())?;
    Ok(())
}
pub fn forget_discovered_paths() {
    if let Some(files) = FILES.get() {
        if let Ok(mut files) = files.lock() {
            files.clear();
        }
    }
}
pub fn file_path(id: &str) -> Option<PathBuf> {
    if id.starts_with("file:") && FILES.get().is_none() {
        let _ = scan_files();
    }
    FILES
        .get()?
        .lock()
        .ok()?
        .get(id)
        .filter(|p| p.is_file())
        .cloned()
}
fn walk(root: &Path, max: usize) -> Vec<PathBuf> {
    let mut queue = vec![(root.to_path_buf(), 0)];
    let mut files = Vec::new();
    let mut visited = 0;
    while let Some((dir, depth)) = queue.pop() {
        if depth > 8 || visited >= max {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > max {
                break;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() && !kind.is_symlink() {
                queue.push((entry.path(), depth + 1));
            } else if kind.is_file() || kind.is_symlink() {
                files.push(entry.path());
            }
        }
    }
    files
}
fn scan_gguf(roots: &[PathBuf]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for root in roots {
        for p in walk(root, 12000) {
            if out.len() >= 256 {
                return out;
            }
            if !p
                .extension()
                .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("gguf"))
            {
                continue;
            }
            let Ok(meta) = p.metadata() else {
                continue;
            };
            if meta.len() == 0 || meta.len() > model_limit() {
                continue;
            }
            use std::io::Read;
            let mut magic = [0; 4];
            if std::fs::File::open(&p)
                .and_then(|mut f| f.read_exact(&mut magic))
                .is_err()
                || magic != *b"GGUF"
            {
                continue;
            }
            let Ok(path) = p.canonicalize() else {
                continue;
            };
            if !seen.insert(path.clone()) {
                continue;
            }
            let id = format!(
                "file:{:x}",
                Sha256::digest(path.to_string_lossy().as_bytes())
            );
            if let Ok(mut files) = FILES.get_or_init(|| Mutex::new(HashMap::new())).lock() {
                files.insert(id.clone(), path.clone());
            }
            out.push(json!({"id":id,"name":p.file_name().unwrap_or_default().to_string_lossy(),"size_bytes":meta.len(),"source":"Local GGUF file","installed":true,"availability":"runtime_required","path":path,"provenance":"User-installed; compatibility and licence not independently verified"}));
        }
    }
    out
}
fn ollama_name(root: &Path, path: &Path) -> Option<String> {
    let parts = path
        .strip_prefix(root.join("manifests"))
        .ok()?
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    if parts.len() != 4 {
        return None;
    }
    let registry = &parts[0];
    let namespace = &parts[1];
    let name = &parts[2];
    let tag = &parts[3];
    let name = if registry == "registry.ollama.ai" && namespace == "library" {
        format!("{name}:{tag}")
    } else if registry == "registry.ollama.ai" {
        format!("{namespace}/{name}:{tag}")
    } else {
        format!("{registry}/{namespace}/{name}:{tag}")
    };
    endpoint(&format!("ollama:{name}"))?;
    Some(name)
}
fn scan_ollama(root: &Path) -> Vec<Value> {
    let mut out = Vec::new();
    for path in walk(&root.join("manifests"), 3000) {
        if out.len() >= 256 {
            break;
        }
        let Some(name) = ollama_name(root, &path) else {
            continue;
        };
        if name.to_lowercase().contains("embed") {
            continue;
        }
        let Ok(meta) = path.metadata() else {
            continue;
        };
        if meta.len() > 1_048_576 {
            continue;
        }
        let Ok(raw) = std::fs::read(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&raw) else {
            continue;
        };
        let Some(layers) = value["layers"].as_array() else {
            continue;
        };
        let model_layers = layers
            .iter()
            .filter(|v| {
                v["mediaType"]
                    .as_str()
                    .is_some_and(|s| s.ends_with(".model"))
            })
            .collect::<Vec<_>>();
        if model_layers.is_empty() {
            continue;
        }
        let mut size = 0u64;
        let mut complete = true;
        for layer in layers {
            let Some(digest) = layer["digest"]
                .as_str()
                .and_then(|s| s.strip_prefix("sha256:"))
            else {
                complete = false;
                break;
            };
            if digest.len() != 64 || !digest.bytes().all(|c| c.is_ascii_hexdigit()) {
                complete = false;
                break;
            }
            let Some(bytes) = layer["size"].as_u64() else {
                complete = false;
                break;
            };
            size = size.saturating_add(bytes);
            let blob = root.join("blobs").join(format!("sha256-{digest}"));
            if !blob
                .metadata()
                .is_ok_and(|m| m.is_file() && m.len() == bytes)
            {
                complete = false;
            }
        }
        if !complete || size == 0 || size > model_limit() {
            continue;
        }
        out.push(json!({"id":format!("ollama:{name}"),"name":name,"source":"Ollama library · start service to use","size_bytes":size,"installed":true,"service_running":false,"availability":"start_service"}));
    }
    out
}
fn scan_files() -> Vec<Value> {
    if cfg!(target_os = "android") {
        return vec![];
    }
    let roots = scan_roots();
    let mut out = scan_gguf(&roots);
    for root in roots {
        out.extend(scan_ollama(&root));
    }
    out
}
fn service_rows(prefix: &str, data: &Value) -> Vec<Value> {
    let list = if prefix == "ollama" {
        data["models"].as_array()
    } else {
        data["data"].as_array()
    };
    let mut rows = Vec::new();
    for model in list.into_iter().flatten().take(256) {
        let Some(name) = (if prefix == "ollama" {
            model["name"].as_str().or(model["model"].as_str())
        } else {
            model["id"].as_str()
        }) else {
            continue;
        };
        if name.to_lowercase().contains("embed") {
            continue;
        }
        let size = model["size"].as_u64();
        if size.is_some_and(|s| s > model_limit()) {
            continue;
        }
        let id = format!("{prefix}:{name}");
        if endpoint(&id).is_none() {
            continue;
        }
        rows.push(json!({"id":id,"name":name,"source":format!("{prefix} · running"),"size_bytes":size,"installed":true,"service_running":true,"availability":"service_ready"}));
    }
    rows
}
pub async fn discover() -> Vec<Value> {
    let out = tokio::task::spawn_blocking(scan_files)
        .await
        .unwrap_or_default();
    if cfg!(target_os = "android") {
        return out;
    }
    let Ok(client) = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(2))
        .build()
    else {
        return out;
    };
    let replies = tokio::join!(
        client.get("http://127.0.0.1:11434/api/tags").send(),
        client.get("http://127.0.0.1:1234/v1/models").send(),
        client.get("http://127.0.0.1:8080/v1/models").send()
    );
    let mut combined = out
        .into_iter()
        .filter_map(|m| Some((m["id"].as_str()?.to_owned(), m)))
        .collect::<HashMap<_, _>>();
    for (prefix, response) in [
        ("ollama", replies.0),
        ("lmstudio", replies.1),
        ("llamacpp", replies.2),
    ] {
        let Ok(mut response) = response else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        let mut body = Vec::new();
        let mut valid = true;
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) if body.len() + chunk.len() <= 1_048_576 => {
                    body.extend_from_slice(&chunk)
                }
                Ok(None) => break,
                _ => {
                    valid = false;
                    break;
                }
            }
        }
        if !valid {
            continue;
        }
        let Ok(data) = serde_json::from_slice::<Value>(&body) else {
            continue;
        };
        for row in service_rows(prefix, &data) {
            combined.insert(row["id"].as_str().unwrap().to_owned(), row);
        }
    }
    let mut rows = combined.into_values().collect::<Vec<_>>();
    rows.sort_by_key(|m| {
        (
            m["service_running"] != true,
            m["name"].as_str().unwrap_or("").to_lowercase(),
        )
    });
    rows
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chosen_model_folders_are_remembered_and_deduplicated() {
        let root = std::env::temp_dir().join(format!(
            "model-folder-preferences-test-{}",
            std::process::id()
        ));
        let folder = root.join("models");
        std::fs::create_dir_all(&folder).unwrap();
        let folder = folder.canonicalize().unwrap();
        let cache = root.join("app-data/local-model-folders.json");
        remember_folder_at(&cache, folder.clone()).unwrap();
        remember_folder_at(&cache, folder.clone()).unwrap();
        let saved: Vec<PathBuf> = serde_json::from_slice(&std::fs::read(&cache).unwrap()).unwrap();
        assert_eq!(saved, vec![folder]);
        assert!(root
            .canonicalize()
            .unwrap()
            .starts_with(std::env::temp_dir().canonicalize().unwrap()));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn endpoints_are_fixed_loopback_only() {
        assert_eq!(
            endpoint("ollama:smollm2:1.7b"),
            Some((11434, "smollm2:1.7b"))
        );
        assert!(endpoint("https://example.com/model").is_none());
        assert!(endpoint("lmstudio:\n").is_none());
        assert!(endpoint("llamacpp:").is_none());
    }
    #[test]
    fn desktop_keeps_installed_models_between_three_and_ten_gb() {
        let rows = service_rows(
            "ollama",
            &json!({"models":[{"name":"local:7b","size":5_000_000_000u64},{"name":"too-large:70b","size":11_000_000_000u64},{"name":"nomic-embed-text","size":1000}]}),
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "ollama:local:7b");
    }
    #[test]
    fn library_scan_finds_complete_models_without_a_running_service() {
        let root =
            std::env::temp_dir().join(format!("local-model-library-test-{}", std::process::id()));
        let manifest = root.join("manifests/registry.ollama.ai/library/example/latest");
        std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
        std::fs::create_dir_all(root.join("blobs")).unwrap();
        let digest = "a".repeat(64);
        let blob = root.join("blobs").join(format!("sha256-{digest}"));
        std::fs::write(&blob, b"GGUFtest").unwrap();
        std::fs::write(&manifest,json!({"layers":[{"mediaType":"application/vnd.ollama.image.model","digest":format!("sha256:{digest}"),"size":8}]}).to_string()).unwrap();
        let rows = scan_ollama(&root);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "ollama:example:latest");
        assert_eq!(rows[0]["service_running"], false);
        std::fs::remove_file(blob).unwrap();
        assert!(scan_ollama(&root).is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
