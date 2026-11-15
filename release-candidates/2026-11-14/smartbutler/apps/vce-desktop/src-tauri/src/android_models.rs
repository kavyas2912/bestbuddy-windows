use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
static ACTIVE: AtomicBool = AtomicBool::new(false);
static CANCEL: AtomicBool = AtomicBool::new(false);
static PROGRESS: Mutex<Option<Value>> = Mutex::new(None);
const MANIFEST: &str = include_str!("../../../../tools/voice-model-manifest.json");
const KEYWORDS: &str = include_str!("../../../../tools/butler-wake-keywords.txt");
const MAX_DOWNLOAD: u64 = 700 * 1024 * 1024;
const MAX_EXTRACTED: u64 = 2 * 1024 * 1024 * 1024;
#[derive(Clone, Deserialize)]
struct Package {
    url: String,
    directory: Option<String>,
    archive_root: Option<String>,
    archive_sha256: Option<String>,
    path: Option<String>,
    sha256: Option<String>,
    #[serde(default)]
    files: BTreeMap<String, String>,
}
struct Job;
impl Drop for Job {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::SeqCst);
    }
}
fn check_cancel() -> Result<(), String> {
    if CANCEL.load(Ordering::SeqCst) {
        Err("Download paused. Press Install to resume.".into())
    } else {
        Ok(())
    }
}
fn progress(package: &str, phase: &str, bytes: u64, total: Option<u64>) {
    if let Ok(mut value) = PROGRESS.lock() {
        *value = Some(json!({"package":package,"phase":phase,"bytes":bytes,"total":total}));
    }
}
pub fn status() -> Value {
    json!({"supported":true,"active":ACTIVE.load(Ordering::SeqCst),"progress":PROGRESS.lock().ok().and_then(|p| p.clone())})
}
pub fn cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}
fn relative(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() || !path.components().all(|p| matches!(p, Component::Normal(_)))
    {
        return Err("Unsafe model package path".into());
    }
    Ok(path.to_path_buf())
}
fn hash(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buf = [0; 65536];
    loop {
        check_cancel()?;
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(format!("{:X}", hash.finalize()))
}
fn verified(path: &Path, expected: &str) -> bool {
    hash(path).is_ok_and(|h| h.eq_ignore_ascii_case(expected))
}
fn complete(root: &Path, package: &Package) -> bool {
    if let (Some(path), Some(expected)) = (&package.path, &package.sha256) {
        return verified(&root.join(path), expected);
    }
    let Some(dir) = package.directory.as_ref() else {
        return false;
    };
    !package.files.is_empty()
        && package
            .files
            .iter()
            .all(|(name, expected)| verified(&root.join(dir).join(name), expected))
}
async fn download(
    client: &reqwest::Client,
    name: &str,
    package: &Package,
    cache: &Path,
) -> Result<PathBuf, String> {
    let part = cache.join(format!("{name}.part"));
    let mut offset = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    if offset > MAX_DOWNLOAD {
        return Err("Cached model download is too large".into());
    }
    let mut request = client.get(&package.url);
    if offset > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={offset}-"));
    }
    let mut response = request.send().await.map_err(|e| e.to_string())?;
    if response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        // A complete partial file may already be present; verification happens below.
        return Ok(part);
    }
    response = response.error_for_status().map_err(|e| e.to_string())?;
    if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        let range = response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !range.starts_with(&format!("bytes {offset}-")) {
            return Err("Server returned an unexpected model download range".into());
        }
    } else {
        offset = 0;
    }
    let total = response
        .content_length()
        .and_then(|n| n.checked_add(offset));
    if total.is_some_and(|n| n > MAX_DOWNLOAD) {
        return Err("Model package exceeds the download limit".into());
    }
    let mut output = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(offset > 0)
        .truncate(offset == 0)
        .open(&part)
        .map_err(|e| e.to_string())?;
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        check_cancel()?;
        #[cfg(target_os = "android")]
        if crate::android_storage::voice_call("transferNetwork").unwrap_or(0) != 1 {
            return Err(
                "Waiting for your selected network; download will resume when available.".into(),
            );
        }
        offset += chunk.len() as u64;
        if offset > MAX_DOWNLOAD {
            return Err("Model download exceeds the size limit".into());
        }
        output.write_all(&chunk).map_err(|e| e.to_string())?;
        progress(name, "Downloading", offset, total);
    }
    output.sync_all().map_err(|e| e.to_string())?;
    Ok(part)
}
fn install(root: &Path, name: &str, package: &Package, download: &Path) -> Result<(), String> {
    check_cancel()?;
    if let Some(expected) = &package.archive_sha256 {
        if !verified(download, expected) {
            let _ = fs::remove_file(download);
            return Err(
                "Model archive failed its integrity check. Retry to download a fresh copy.".into(),
            );
        }
    }
    if let (Some(path), Some(expected)) = (&package.path, &package.sha256) {
        if !verified(download, expected) {
            let _ = fs::remove_file(download);
            return Err("Model file failed its integrity check".into());
        }
        let destination = root.join(relative(Path::new(path))?);
        fs::create_dir_all(destination.parent().ok_or("Missing model directory")?)
            .map_err(|e| e.to_string())?;
        let stage = destination.with_extension("installing");
        fs::copy(download, &stage).map_err(|e| e.to_string())?;
        fs::rename(stage, destination).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let archive_root = package
        .archive_root
        .as_ref()
        .ok_or("Missing archive root")?;
    let stage = root.join(format!(".{name}-installing"));
    if stage.exists() {
        fs::remove_dir_all(&stage).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        let decoder =
            bzip2::read::BzDecoder::new(fs::File::open(download).map_err(|e| e.to_string())?);
        let mut archive = tar::Archive::new(decoder);
        let mut extracted = 0u64;
        for entry in archive.entries().map_err(|e| e.to_string())? {
            check_cancel()?;
            let mut entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path().map_err(|e| e.to_string())?.into_owned();
            let mut parts = path.components();
            if parts.next() != Some(Component::Normal(archive_root.as_ref())) {
                return Err("Unexpected model archive root".into());
            }
            let path: PathBuf = parts.collect();
            if path.as_os_str().is_empty() {
                continue;
            }
            let target = stage.join(relative(&path)?);
            let kind = entry.header().entry_type();
            if kind.is_dir() {
                fs::create_dir_all(target).map_err(|e| e.to_string())?;
            } else if kind.is_file() {
                extracted = extracted
                    .checked_add(entry.size())
                    .ok_or("Model package is too large")?;
                if extracted > MAX_EXTRACTED {
                    return Err("Model archive exceeds its expanded size limit".into());
                }
                // Keep only the quantized runtime inputs for recognition/wake models.
                // TTS also needs its language-data tree, so retain that package in full.
                if matches!(name, "asr" | "kws")
                    && !package
                        .files
                        .contains_key(&path.to_string_lossy().replace('\\', "/"))
                {
                    continue;
                }
                fs::create_dir_all(target.parent().ok_or("Missing archive directory")?)
                    .map_err(|e| e.to_string())?;
                let mut output = fs::File::create(target).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut output).map_err(|e| e.to_string())?;
            } else {
                return Err("Model archive contains a link or unsupported entry".into());
            }
        }
        for (file, expected) in &package.files {
            if !verified(&stage.join(relative(Path::new(file))?), expected) {
                return Err("Extracted model failed its pinned integrity check".into());
            }
        }
        let destination = root.join(relative(Path::new(
            package
                .directory
                .as_ref()
                .ok_or("Missing model directory")?,
        ))?);
        fs::create_dir_all(destination.parent().ok_or("Missing destination")?)
            .map_err(|e| e.to_string())?;
        if destination.exists() {
            fs::rename(
                &destination,
                root.join(format!(
                    ".{name}-previous-{}",
                    chrono::Utc::now().timestamp_millis()
                )),
            )
            .map_err(|e| e.to_string())?;
        }
        fs::rename(&stage, destination).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(stage);
        if !CANCEL.load(Ordering::SeqCst) {
            let _ = fs::remove_file(download);
        }
    }
    result
}
pub async fn prepare(root: PathBuf) -> Result<(), String> {
    if ACTIVE.swap(true, Ordering::SeqCst) {
        return Err("Voice installation is already running".into());
    }
    let _job = Job;
    CANCEL.store(false, Ordering::SeqCst);
    let result = async {
        let packages: BTreeMap<String, Package> =
            serde_json::from_str(MANIFEST).map_err(|e| e.to_string())?;
        let cache = root.join(".downloads");
        fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(20))
            .timeout(std::time::Duration::from_secs(1800))
            .build()
            .map_err(|e| e.to_string())?;
        for name in ["vad", "kws", "denoise", "tts", "asr"] {
            check_cancel()?;
            let package = packages
                .get(name)
                .ok_or("Model manifest is incomplete")?
                .clone();
            let check_root = root.clone();
            let check_package = package.clone();
            if tauri::async_runtime::spawn_blocking(move || complete(&check_root, &check_package))
                .await
                .map_err(|e| e.to_string())?
            {
                continue;
            }
            let archive = download(&client, name, &package, &cache).await?;
            progress(name, "Verifying and installing", 0, None);
            let destination = root.clone();
            let label = name.to_string();
            tauri::async_runtime::spawn_blocking(move || {
                install(&destination, &label, &package, &archive)
            })
            .await
            .map_err(|e| e.to_string())??;
            let _ = fs::remove_file(cache.join(format!("{name}.part")));
        }
        fs::write(root.join("kws/keywords.txt"), KEYWORDS).map_err(|e| e.to_string())?;
        fs::write(root.join("MODEL_SOURCES.json"), MANIFEST).map_err(|e| e.to_string())?;
        progress(
            "Voice",
            "Installed — press Talk to enable listening",
            0,
            None,
        );
        Ok::<_, String>(())
    }
    .await;
    if let Err(error) = &result {
        progress("Voice", error, 0, None);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_are_confined() {
        assert!(relative(Path::new("models/tokens.txt")).is_ok());
        for path in ["../outside", "/absolute", "models/../../outside", ""] {
            assert!(relative(Path::new(path)).is_err());
        }
    }
    #[test]
    fn manifest_contains_pinned_required_models() {
        let packages: BTreeMap<String, Package> = serde_json::from_str(MANIFEST).unwrap();
        assert_eq!(packages.len(), 5);
        for package in packages.values() {
            assert!(package
                .url
                .starts_with("https://github.com/k2-fsa/sherpa-onnx/releases/download/"));
            assert!(package.sha256.is_some() || !package.files.is_empty());
        }
    }
}

#[cfg(test)]
mod installation_tests {
    use super::*;
    #[test]
    fn validates_archive_before_publishing_and_rejects_links() {
        let root = std::env::temp_dir().join(format!(
            "mobile-model-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let archive = root.join("fixture.tar.bz2");
        let payload = b"model test fixture";
        let mut files = BTreeMap::new();
        files.insert(
            "tokens.txt".to_string(),
            format!("{:X}", Sha256::digest(payload)),
        );
        let package = Package {
            url: String::new(),
            directory: Some("tts/voice".into()),
            archive_root: Some("voice".into()),
            archive_sha256: None,
            path: None,
            sha256: None,
            files,
        };
        {
            let encoder = bzip2::write::BzEncoder::new(
                fs::File::create(&archive).unwrap(),
                bzip2::Compression::fast(),
            );
            let mut tar = tar::Builder::new(encoder);
            let mut header = tar::Header::new_gnu();
            header.set_size(payload.len() as u64);
            header.set_mode(0o600);
            header.set_cksum();
            tar.append_data(&mut header, "voice/tokens.txt", &payload[..])
                .unwrap();
            tar.into_inner().unwrap().finish().unwrap();
        }
        install(&root, "voice", &package, &archive).unwrap();
        assert!(complete(&root, &package));
        fs::write(root.join("tts/voice/tokens.txt"), b"altered").unwrap();
        assert!(!complete(&root, &package));
        assert!(require_assets(&root).is_err());
        {
            let encoder = bzip2::write::BzEncoder::new(
                fs::File::create(&archive).unwrap(),
                bzip2::Compression::fast(),
            );
            let mut tar = tar::Builder::new(encoder);
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_mode(0o600);
            header.set_link_name("../../outside").unwrap();
            header.set_cksum();
            tar.append_data(&mut header, "voice/link", std::io::empty())
                .unwrap();
            tar.into_inner().unwrap().finish().unwrap();
        }
        assert!(install(&root, "voice", &package, &archive).is_err());
        assert_eq!(
            fs::read(root.join("tts/voice/tokens.txt")).unwrap(),
            b"altered"
        );
        fs::remove_dir_all(root).unwrap();
    }
}

pub fn require_assets(root: &Path) -> Result<(), String> {
    if ACTIVE.load(Ordering::SeqCst) {
        return Err("Wait for voice installation to finish, then press Talk.".into());
    }
    let packages: BTreeMap<String, Package> =
        serde_json::from_str(MANIFEST).map_err(|e| e.to_string())?;
    if !root.join("MODEL_SOURCES.json").is_file() || !root.join("kws/keywords.txt").is_file() {
        return Err(
            "Install offline voice models in Voice settings first, then press Talk.".into(),
        );
    }
    for package in packages.values() {
        if let Some(path) = &package.path {
            if !root.join(path).is_file() {
                return Err(
                    "Voice installation is incomplete. Resume it in Voice settings.".into(),
                );
            }
        }
        if let Some(dir) = &package.directory {
            for file in package.files.keys() {
                if !root.join(dir).join(file).is_file() {
                    return Err(
                        "Voice installation is incomplete. Resume it in Voice settings.".into(),
                    );
                }
            }
        }
    }
    Ok(())
}
