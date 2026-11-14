# Third-party downloads, sources and distribution review

All URLs below originate from actual Smart Butler source (the pinned voice manifest, native library build script, optional GGUF/voice download handlers). A link is **not** permission to redistribute a model. The Store requires a rights review before release.

## Base voice assets — downloaded on the build machine, embedded in finished MSIX

| Function | Upstream HTTPS file | Verified using |
|---|---|---|
| asr | [sherpa-onnx-whisper-base.en.tar.bz2](https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-base.en.tar.bz2) | SHA-256 manifest (`tools/voice-model-manifest.json`) |
| tts | [vits-piper-en_US-lessac-medium.tar.bz2](https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-en_US-lessac-medium.tar.bz2) | SHA-256 manifest (`tools/voice-model-manifest.json`) |
| vad | [silero_vad.onnx](https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx) | SHA-256 manifest (`tools/voice-model-manifest.json`) |
| kws | [sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01.tar.bz2](https://github.com/k2-fsa/sherpa-onnx/releases/download/kws-models/sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01.tar.bz2) | SHA-256 manifest (`tools/voice-model-manifest.json`) |
| denoise | [gtcrn_simple.onnx](https://github.com/k2-fsa/sherpa-onnx/releases/download/speech-enhancement-models/gtcrn_simple.onnx) | SHA-256 manifest (`tools/voice-model-manifest.json`) |

## Native build-time library

- [sherpa-onnx v1.13.8 Windows x64 static archive](https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/sherpa-onnx-v1.13.8-win-x64-static-MT-Release-lib.tar.bz2). Pinned SHA-256 in `tools/BUILD_STORE_MSIX_WINDOWS.ps1`.

## Additional voice packages (optional downloads initiated by user)

| Voice | Archive download | Model card |
|---|---|---|
| vits-piper-en_GB-cori-medium | [Upstream release](https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-en_GB-cori-medium.tar.bz2) | [Model card](https://huggingface.co/rhasspy/piper-voices/blob/main/en/en_GB/cori/medium/MODEL_CARD) |
| vits-piper-en_US-kristin-medium | [Upstream release](https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-en_US-kristin-medium.tar.bz2) | [Model card](https://huggingface.co/rhasspy/piper-voices/blob/main/en/en_US/kristin/medium/MODEL_CARD) |
| vits-piper-en_US-joe-medium | [Upstream release](https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-en_US-joe-medium.tar.bz2) | [Model card](https://huggingface.co/rhasspy/piper-voices/blob/main/en/en_US/joe/medium/MODEL_CARD) |
| vits-piper-en_US-john-medium | [Upstream release](https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-en_US-john-medium.tar.bz2) | [Model card](https://huggingface.co/rhasspy/piper-voices/blob/main/en/en_US/john/medium/MODEL_CARD) |

## Optional local LLM weights (not included in MSIX)

- [Qwen3-0.6B-Q8_0.gguf](https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B-Q8_0.gguf?download=true) — in-app opt-in; SHA-256 pinned in `apps/vce-desktop/src-tauri/src/llm_assets.rs`.
- [Qwen3-1.7B-Q4_K_M.gguf](https://huggingface.co/ggml-org/Qwen3-1.7B-GGUF/resolve/main/Qwen3-1.7B-Q4_K_M.gguf?download=true) — in-app opt-in; SHA-256 pinned in `apps/vce-desktop/src-tauri/src/llm_assets.rs`.

## Web service endpoints referenced by the app

These are user-facing content/data services, **not** files needed to install Smart Butler. Requests need working network access and compliant privacy disclosure.

- [Open-Meteo forecast API](https://open-meteo.com/en/docs) and [geocoding API](https://open-meteo.com/en/docs/geocoding-api) — weather.
- [Google News](https://news.google.com/) — news links/feed; availability may depend on query and policy.
- [Wikimedia REST API](https://www.mediawiki.org/wiki/API:REST_API) — facts/events.
- [Stooq](https://stooq.com/) and [Yahoo Finance](https://finance.yahoo.com/) — market links/data.
- [Microsoft Store publishing guidelines](https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/app-package-requirements) — MSIX distribution.

## Rust, frontend and native components

Rust dependencies are fetched from [crates.io](https://crates.io/) by Cargo using `Cargo.lock`; frontend dependencies are fetched from [npm registry](https://www.npmjs.com/) using `package-lock.json`.
The complete versioned dependency list and registry URLs are in `store/third_party_dependencies.csv` generated from the lockfiles.

**Not allowed in Store payload:** npm caches, Cargo archives, package lockfiles, release signing keys, personal user profiles, credentials, private keys, machine-bound symlinks, temporary `.part` files, build source and local source paths.

## License release gate

Before sending the MSIX to Partner Center, check each upstream **model weights license**, training dataset restrictions, voice model card and any required attribution. The fact that sherpa-onnx code is open source does not establish redistribution rights for unrelated model weights. Add the resulting reviewed notices to the application package or Store listing as required.

## Where the app looks for models

After installation, mandatory voice models are **under the executable’s own packaged `models/` directory**, resolved at runtime. Source checkout paths, developer home folders and any previous app version are not required. User profile, reminders and optional-model downloads are stored under platform-resolved, writable application data.
