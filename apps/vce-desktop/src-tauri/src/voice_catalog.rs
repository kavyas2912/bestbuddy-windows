use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Serialize)]
pub struct VoiceOffer {
    pub id: &'static str,
    pub label: &'static str,
    pub gender: &'static str,
    pub language: &'static str,
    pub source: &'static str,
    pub dataset_license: &'static str,
    #[serde(skip)]
    pub sha256: &'static str,
}

pub const OFFERS: &[VoiceOffer] = &[
    VoiceOffer { id:"vits-piper-en_GB-cori-medium", label:"Cori · British English", gender:"female", language:"en-GB", source:"https://huggingface.co/rhasspy/piper-voices/blob/main/en/en_GB/cori/medium/MODEL_CARD", dataset_license:"Public domain LibriVox recordings", sha256:"49c9a5361bbdd95d7ca9687c4de11e5908481f65e7c7c368960df79949fdac2b" },
    VoiceOffer { id:"vits-piper-en_US-kristin-medium", label:"Kristin · US English", gender:"female", language:"en-US", source:"https://huggingface.co/rhasspy/piper-voices/blob/main/en/en_US/kristin/medium/MODEL_CARD", dataset_license:"Public domain LibriVox recordings", sha256:"c2206f572df2956c50b1ae3367eebce3853c663e890cba8048cd62b1e4dbe6c7" },
    VoiceOffer { id:"vits-piper-en_US-joe-medium", label:"Joe · US English", gender:"male", language:"en-US", source:"https://huggingface.co/rhasspy/piper-voices/blob/main/en/en_US/joe/medium/MODEL_CARD", dataset_license:"CC0 dataset", sha256:"261b903a1fb2ce621e2b5b14233cd41223c3b9f9590f896c11daee4df22d9bd3" },
    VoiceOffer { id:"vits-piper-en_US-john-medium", label:"John · US English", gender:"male", language:"en-US", source:"https://huggingface.co/rhasspy/piper-voices/blob/main/en/en_US/john/medium/MODEL_CARD", dataset_license:"Public domain LibriVox recordings", sha256:"d90362cfa48e429d3096f644785184446145b6e38216f2eaf4c0601b8aad5a36" },
];

pub fn offer(id: &str) -> Option<&'static VoiceOffer> {
    OFFERS.iter().find(|voice| voice.id == id)
}

pub fn complete(dir: &Path) -> bool {
    dir.join("tokens.txt").is_file()
        && dir.join("espeak-ng-data/phontab").is_file()
        && dir.join("espeak-ng-data/phonindex").is_file()
        && dir.join("espeak-ng-data/phondata").is_file()
        && dir.join("espeak-ng-data/intonations").is_file()
        && dir.join("espeak-ng-data/lang").is_dir()
        && fs::read_dir(dir)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .any(|entry| entry.path().extension().is_some_and(|ext| ext == "onnx"))
}

fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn safe_relative(path: &Path, expected_root: &str) -> Result<PathBuf, String> {
    let mut parts = path.components();
    if parts.next() != Some(Component::Normal(expected_root.as_ref())) {
        return Err("Unexpected voice archive root".into());
    }
    let mut relative = PathBuf::new();
    for part in parts {
        if let Component::Normal(name) = part {
            relative.push(name);
        } else {
            return Err("Unsafe voice archive path".into());
        }
    }
    Ok(relative)
}

pub async fn prepare(id: &str, library: &Path) -> Result<PathBuf, String> {
    let entry = offer(id).ok_or("Unknown voice package")?;
    let destination = library.join(id);
    if complete(&destination) {
        return Ok(destination);
    }
    fs::create_dir_all(library).map_err(|e| e.to_string())?;
    let cache = library.join(".downloads").join(format!("{id}.tar.bz2"));
    let archive_bytes = if cache.is_file() {
        fs::read(&cache).map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };
    let archive_bytes = if digest(&archive_bytes) == entry.sha256 {
        archive_bytes
    } else {
        let url = format!(
            "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/{id}.tar.bz2"
        );
        let bytes = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .map_err(|e| e.to_string())?
            .get(url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| e.to_string())?
            .bytes()
            .await
            .map_err(|e| e.to_string())?
            .to_vec();
        if digest(&bytes) != entry.sha256 {
            return Err("Voice archive failed its pinned SHA-256 check".into());
        }
        fs::create_dir_all(cache.parent().ok_or("No voice cache directory")?)
            .map_err(|e| e.to_string())?;
        let staged_cache = cache.with_extension("download");
        fs::write(&staged_cache, &bytes).map_err(|e| e.to_string())?;
        fs::rename(staged_cache, &cache).map_err(|e| e.to_string())?;
        bytes
    };
    let stage = library.join(format!(
        ".{id}.stage-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_millis()
    ));
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let install = (|| -> Result<(), String> {
        let decoder = bzip2::read::BzDecoder::new(archive_bytes.as_slice());
        let mut archive = tar::Archive::new(decoder);
        for item in archive.entries().map_err(|e| e.to_string())? {
            let mut item = item.map_err(|e| e.to_string())?;
            let path = item.path().map_err(|e| e.to_string())?;
            let relative = safe_relative(path.as_ref(), id)?;
            if relative.as_os_str().is_empty() {
                continue;
            }
            let target = stage.join(relative);
            if item.header().entry_type().is_dir() {
                fs::create_dir_all(target).map_err(|e| e.to_string())?;
            } else if item.header().entry_type().is_file() {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut output = fs::File::create(target).map_err(|e| e.to_string())?;
                io::copy(&mut item, &mut output).map_err(|e| e.to_string())?;
            } else {
                return Err("Voice archive contains unsupported link or device entry".into());
            }
        }
        if !complete(&stage) {
            return Err("Extracted voice package is incomplete".into());
        }
        if destination.exists() {
            let old = library.join(format!(
                ".{id}.incomplete-{}",
                chrono::Utc::now().timestamp_millis()
            ));
            fs::rename(&destination, old).map_err(|e| e.to_string())?;
        }
        fs::rename(&stage, &destination).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if install.is_err() {
        let _ = fs::remove_dir_all(&stage);
    }
    install.map(|()| destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archive_paths_cannot_escape_voice_library() {
        assert_eq!(
            safe_relative(
                Path::new("vits-piper-en_US-joe-medium/tokens.txt"),
                "vits-piper-en_US-joe-medium"
            )
            .unwrap(),
            PathBuf::from("tokens.txt")
        );
        assert!(safe_relative(
            Path::new("vits-piper-en_US-joe-medium/../outside"),
            "vits-piper-en_US-joe-medium"
        )
        .is_err());
    }
}
