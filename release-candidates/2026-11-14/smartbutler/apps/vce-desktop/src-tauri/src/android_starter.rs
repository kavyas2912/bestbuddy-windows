use butler_voice::{VoiceConfig, VoiceEngine, VoiceMonitor};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Component, Path},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
};
const ARCHIVE: &[u8] = include_bytes!("../../../../tools/bundled-voice/amy-low.tar");
static ENGINE: OnceLock<Mutex<Option<VoiceEngine>>> = OnceLock::new();
static MONITOR: OnceLock<VoiceMonitor> = OnceLock::new();
static EPOCH: AtomicU64 = AtomicU64::new(0);
pub fn ticket() -> u64 {
    EPOCH.load(Ordering::SeqCst)
}
pub fn stop() {
    EPOCH.fetch_add(1, Ordering::SeqCst);
    if let Some(m) = MONITOR.get() {
        m.stop();
    }
}
fn unpack(root: &Path) -> Result<std::path::PathBuf, String> {
    let folder = root.join("starter-voice");
    let ready = folder.join("READY");
    if !ready.is_file() {
        if format!("{:X}", Sha256::digest(ARCHIVE))
            != "2852D2AB2DC5D8B64C2C8455197947A665D729D639D838FB20756BA98BD2ED6E"
        {
            return Err("Bundled voice integrity check failed".into());
        }
        fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let mut archive = tar::Archive::new(ARCHIVE);
        let mut total = 0;
        for item in archive.entries().map_err(|e| e.to_string())? {
            let mut item = item.map_err(|e| e.to_string())?;
            let path = item.path().map_err(|e| e.to_string())?.into_owned();
            if !path.components().all(|c| matches!(c, Component::Normal(_)))
                || !(item.header().entry_type().is_file() || item.header().entry_type().is_dir())
            {
                return Err("Unsafe bundled voice path".into());
            }
            total += item.size();
            if total > 100 * 1024 * 1024 {
                return Err("Bundled voice exceeds size limit".into());
            }
            if !item.unpack_in(&folder).map_err(|e| e.to_string())? {
                return Err("Bundled voice path escaped destination".into());
            }
        }
        fs::write(&ready, b"verified").map_err(|e| e.to_string())?;
    }
    Ok(folder.join("vits-piper-en_US-amy-low"))
}
pub fn speak(mut config: VoiceConfig, root: &Path, text: &str, ticket: u64) -> Result<(), String> {
    let mut engine = ENGINE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|e| e.to_string())?;
    if ticket != EPOCH.load(Ordering::SeqCst) {
        return Ok(());
    }
    if engine.is_none() {
        config.tts_dir = unpack(root)?;
        config.num_threads = 2;
        let voice = VoiceEngine::new(config);
        let _ = MONITOR.set(voice.monitor());
        *engine = Some(voice);
    }
    if ticket != EPOCH.load(Ordering::SeqCst) {
        return Ok(());
    }
    engine
        .as_mut()
        .unwrap()
        .speak(text)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_starter_unpacks_without_network() {
        let root = std::env::temp_dir().join(format!("starter-bundle-{}", std::process::id()));
        let folder = unpack(&root).unwrap();
        for name in [
            "en_US-amy-low.onnx",
            "tokens.txt",
            "espeak-ng-data/phontab",
            "espeak-ng-data/phondata",
        ] {
            assert!(folder.join(name).is_file(), "{}", name);
        }
        assert_eq!(
            fs::metadata(folder.join("en_US-amy-low.onnx"))
                .unwrap()
                .len(),
            63104657
        );
        assert_eq!(unpack(&root).unwrap(), folder);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn stopping_invalidates_old_narration() {
        let old = ticket();
        stop();
        assert!(ticket() > old);
    }
}
