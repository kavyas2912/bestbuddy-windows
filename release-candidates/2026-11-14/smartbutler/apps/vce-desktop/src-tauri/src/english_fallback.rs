//! English STT/TTS fallback is bundled independently of optional language packs.
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path},
};
const ARCHIVE: &[u8] = include_bytes!("../../../../tools/bundled-voice/english-fallback.tar");
const FILES: &str = include_str!("../../../../tools/bundled-voice/english-fallback-files.json");
const MANIFEST: &str = include_str!("../../../../tools/voice-model-manifest.json");
pub fn install(root: &Path, keywords: &str) -> Result<(), String> {
    let expected: std::collections::BTreeMap<String, String> =
        serde_json::from_str(FILES).map_err(|e| e.to_string())?;
    if root.join("BUNDLED_ENGLISH_V1").is_file() && expected.keys().all(|p| root.join(p).is_file())
    {
        return Ok(());
    }
    if format!("{:x}", Sha256::digest(ARCHIVE))
        != "97e0ebf97aba3967bfd80a4913c78548e13a5d4e43406b5724e358e63eb178f3"
    {
        return Err("Bundled English archive integrity check failed".into());
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let mut archive = tar::Archive::new(ARCHIVE);
    let mut total = 0u64;
    let mut count = 0;
    for entry in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let rel = entry.path().map_err(|e| e.to_string())?.into_owned();
        if !entry.header().entry_type().is_file()
            || !rel.components().all(|p| matches!(p, Component::Normal(_)))
        {
            return Err("Unsafe English bundle member".into());
        }
        let key = rel.to_str().ok_or("Invalid bundle path")?;
        let hash = expected.get(key).ok_or("Unexpected English bundle file")?;
        total += entry.size();
        if total > 300 * 1024 * 1024 {
            return Err("English bundle size limit exceeded".into());
        }
        let target = root.join(&rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut output = fs::File::create(&target).map_err(|e| e.to_string())?;
        let mut digest = Sha256::new();
        let mut buf = [0; 65536];
        loop {
            let n = entry.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            digest.update(&buf[..n]);
            output.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        }
        output.sync_all().map_err(|e| e.to_string())?;
        if format!("{:x}", digest.finalize()) != *hash {
            return Err("Bundled English file checksum failed".into());
        }
        count += 1;
    }
    if count != expected.len() {
        return Err("English bundle is incomplete".into());
    }
    fs::write(root.join("kws/keywords.txt"), keywords).map_err(|e| e.to_string())?;
    fs::write(root.join("MODEL_SOURCES.json"), MANIFEST).map_err(|e| e.to_string())?;
    fs::write(root.join("BUNDLED_ENGLISH_V1"), b"verified").map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_english_installs_and_repairs_missing_files() {
        let root = std::env::temp_dir().join(format!("english-fallback-{}", std::process::id()));
        install(&root, "test keyword").unwrap();
        assert!(root
            .join("asr/sherpa-onnx-whisper-base.en/base.en-encoder.int8.onnx")
            .is_file());
        assert!(root
            .join("tts/vits-piper-en_US-lessac-medium/en_US-lessac-medium.onnx")
            .is_file());
        std::fs::remove_file(root.join("vad/silero_vad.onnx")).unwrap();
        install(&root, "test keyword").unwrap();
        assert!(root.join("vad/silero_vad.onnx").is_file());
        std::fs::remove_dir_all(root).unwrap();
    }
}
