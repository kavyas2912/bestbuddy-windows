# BestBuddy for Windows

English conversation and voice with a shared Rust core, analyzer-guided replies, cached source-aware retrieval and a Tauri interface.

**0.8.0 candidate — source prerelease.** Targets CPU-first 4 GB / 8 GB devices; those memory and performance targets still require physical-device validation. No dedicated GPU is required by the default configuration.

## What is included

- Conversation analysis, bundled deterministic knowledge and contextual follow-ups.
- English recognition, wake-word detection and speech-output integration.
- Cached lexical retrieval, bounded context, private local history and cancellation.
- Source, build/setup scripts and validation records.

Existing plugin implementations and UI are retained for compatibility; plugin development is deferred.

## Build

Extract or clone to a short folder path. Run `tools/SETUP_WINDOWS.cmd` on Windows to obtain verified dependencies and voice models and build the Windows candidate. Read the scripts before running them; they download development/runtime dependencies. Compilation needs a suitable development machine and toolchain.

Model weights, native inference archives, SDKs, signing keys, personal data and generated builds are **not** stored in this repository or its source ZIP. Setup stages the English model pack before application packaging. See [model/runtime download links and implementation](docs/CORE_0.8.0.md).

## Validation and limits

The local candidate passed 70 Windows tests (10 ignored), Android arm64 Rust compilation checks, and both frontend builds. See [validation](docs/VALIDATION.md) for the exact scope. These checks do not establish microphone, speaker, Android lifecycle or 4 GB/8 GB performance acceptance. This release does not include signed installers or an APK.

Android-native generative inference and token streaming remain open. Android includes offline voice, analyzer, deterministic conversation and retrieval; its desktop GGUF download path is intentionally unavailable.

## License and dependencies

Project source is published under the existing MIT workspace license; see [LICENSE](LICENSE). Third-party dependencies and model weights retain their own licenses; see [third-party notes](THIRD_PARTY.md).
