//! App-managed desktop GGUF assets. Downloads and existing verified files are
//! confined to BestBuddy's own application-data directory.
use reqwest::{header, Client};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

pub struct ModelAsset {
    pub file: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
}

pub fn model_asset(id: &str) -> Option<ModelAsset> {
    match id {
        "qwen3-0.6b" => Some(ModelAsset {
            file: "Qwen3-0.6B-Q8_0.gguf",
            url: "https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B-Q8_0.gguf?download=true",
            sha256: "9465e63a22add5354d9bb4b99e90117043c7124007664907259bd16d043bb031",
        }),
        "qwen3-1.7b" => Some(ModelAsset {
            file: "Qwen3-1.7B-Q4_K_M.gguf",
            url: "https://huggingface.co/ggml-org/Qwen3-1.7B-GGUF/resolve/main/Qwen3-1.7B-Q4_K_M.gguf?download=true",
            sha256: "d2387ca2dbfee2ffabce7120d3770dadca0b293052bc2f0e138fdc940d9bc7b5",
        }),
        _ => None,
    }
}

fn digest_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut sha = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        sha.update(&buffer[..count]);
    }
    Ok(format!("{:x}", sha.finalize()))
}

async fn download(
    client: &Client,
    url: &str,
    path: &Path,
    expected: &str,
    progress: &mut impl FnMut(String),
) -> Result<(), String> {
    if path.is_file() && digest_file(path)?.eq_ignore_ascii_case(expected) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let part = path.with_extension("part");
    let have = part.metadata().map(|m| m.len()).unwrap_or(0);
    let mut request = client.get(url);
    if have > 0 {
        request = request.header(header::RANGE, format!("bytes={have}-"));
    }
    let mut response = request
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let resumed = have > 0 && response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    let mut output = OpenOptions::new()
        .create(true)
        .write(true)
        .append(resumed)
        .truncate(!resumed)
        .open(&part)
        .map_err(|e| e.to_string())?;
    let mut received = if resumed { have } else { 0 };
    let mut next_report = received.saturating_add(32 * 1024 * 1024);
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        output.write_all(&chunk).map_err(|e| e.to_string())?;
        received += chunk.len() as u64;
        if received >= next_report {
            progress(format!(
                "Downloaded {:.0} MB",
                received as f64 / 1_000_000.0
            ));
            next_report = received.saturating_add(32 * 1024 * 1024);
        }
    }
    output.sync_all().map_err(|e| e.to_string())?;
    if !digest_file(&part)?.eq_ignore_ascii_case(expected) {
        return Err("Downloaded asset failed SHA-256 verification. The partial file is retained for inspection.".into());
    }
    if path.is_file() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    fs::rename(&part, path).map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn prepare_model(
    client: &Client,
    data_dir: &Path,
    id: &str,
    progress: &mut impl FnMut(String),
) -> Result<PathBuf, String> {
    let asset = model_asset(id).ok_or("Unknown compact model")?;
    let path = data_dir.join("models/llm").join(asset.file);
    progress(format!("Preparing {} from Hugging Face", asset.file));
    download(client, asset.url, &path, asset.sha256, progress).await?;
    Ok(path)
}

fn runtime_asset_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("bin-win-cpu-x64.zip"),
        ("windows", "aarch64") => Some("bin-win-cpu-arm64.zip"),
        ("linux", "x86_64") => Some("bin-ubuntu-x64.tar.gz"),
        ("linux", "aarch64") => Some("bin-ubuntu-arm64.tar.gz"),
        ("macos", "x86_64") => Some("bin-macos-x64.zip"),
        ("macos", "aarch64") => Some("bin-macos-arm64.zip"),
        _ => None,
    }
}

fn official_runtime_asset(releases: &Value, suffix: &str) -> Option<(String, String, String)> {
    for release in releases.as_array()? {
        let Some(assets) = release["assets"].as_array() else {
            continue;
        };
        for asset in assets {
            let Some(name) = asset["name"].as_str() else {
                continue;
            };
            if !name.ends_with(suffix) {
                continue;
            }
            let Some(url) = asset["browser_download_url"].as_str() else {
                continue;
            };
            let Some(digest) = asset["digest"]
                .as_str()
                .and_then(|s| s.strip_prefix("sha256:"))
            else {
                continue;
            };
            if url.starts_with("https://github.com/ggml-org/llama.cpp/releases/download/")
                && digest.len() == 64
                && digest.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Some((name.into(), url.into(), digest.into()));
            }
        }
    }
    None
}

fn safe_archive_path(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(File::open(archive).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|e| e.to_string())?;
        let relative = entry
            .enclosed_name()
            .ok_or("Unsafe path in runtime archive")?
            .to_path_buf();
        if !safe_archive_path(&relative) {
            return Err("Unsafe path in runtime archive".into());
        }
        let target = dest.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut output = File::create(target).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut output).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<(), String> {
    let input = File::open(archive).map_err(|e| e.to_string())?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(input));
    for entry in tar.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let relative = entry.path().map_err(|e| e.to_string())?;
        if !safe_archive_path(&relative) {
            return Err("Unsafe path in runtime archive".into());
        }
        entry.unpack_in(dest).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn find_server(dir: &Path, depth: usize) -> Option<PathBuf> {
    if depth > 4 {
        return None;
    }
    let exe = if cfg!(windows) {
        "llama-server.exe"
    } else {
        "llama-server"
    };
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_file() && path.file_name().is_some_and(|name| name == exe) {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_server(&path, depth + 1) {
                return Some(found);
            }
        }
    }
    None
}

pub async fn prepare_runtime(
    client: &Client,
    data_dir: &Path,
    progress: &mut impl FnMut(String),
) -> Result<PathBuf, String> {
    let suffix = runtime_asset_name()
        .ok_or("No tested desktop llama.cpp package is mapped for this platform yet")?;
    progress("Finding a verified llama.cpp desktop package".into());
    let releases: Value = client
        .get("https://api.github.com/repos/ggml-org/llama.cpp/releases?per_page=20")
        .header(header::USER_AGENT, "BestBuddy/0.6.12")
        .header(header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let (name, url, digest) = official_runtime_asset(&releases, suffix).ok_or(
        "No verified CPU runtime for this platform was found in current official releases",
    )?;
    let archive = data_dir.join("cache/llama_cpp").join(&name);
    progress(format!("Preparing official runtime {name}"));
    download(client, &url, &archive, &digest, progress).await?;
    let unpack = data_dir.join("runtime/llama_cpp.download");
    if unpack.exists() {
        fs::remove_dir_all(&unpack).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&unpack).map_err(|e| e.to_string())?;
    if name.ends_with(".zip") {
        extract_zip(&archive, &unpack)?;
    } else {
        extract_tar_gz(&archive, &unpack)?;
    }
    let server = find_server(&unpack, 0)
        .ok_or("Verified runtime archive contains no llama-server executable")?;
    let source_dir = server.parent().ok_or("Runtime archive layout is invalid")?;
    let installed = data_dir.join("runtime/llama_cpp");
    if installed.exists() {
        fs::remove_dir_all(&installed).map_err(|e| e.to_string())?;
    }
    fs::rename(source_dir, &installed).map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(unpack);
    let result = installed.join(if cfg!(windows) {
        "llama-server.exe"
    } else {
        "llama-server"
    });
    if !result.is_file() {
        return Err("Runtime extraction did not produce the server".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&result, fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    progress("Portable local runtime ready".into());
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chooses_only_verified_official_cpu_runtime() {
        let sample = serde_json::json!([{"assets":[
            {"name":"llama-b1-bin-win-cuda-x64.zip","browser_download_url":"https://github.com/ggml-org/llama.cpp/releases/download/b1/cuda.zip","digest":"sha256:1234"},
            {"name":"llama-b1-bin-win-cpu-x64.zip","browser_download_url":"https://github.com/ggml-org/llama.cpp/releases/download/b1/cpu.zip","digest":format!("sha256:{}", "a".repeat(64))}
        ]}]);
        assert!(official_runtime_asset(&sample, "bin-win-cpu-x64.zip").is_some());
        assert!(official_runtime_asset(&sample, "bin-ubuntu-x64.tar.gz").is_none());
        assert!(!safe_archive_path(Path::new("../other")));
    }
}
