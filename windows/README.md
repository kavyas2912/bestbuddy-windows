# BestBuddy — current testing candidate

17 November 2026. Windows 1.6.26 private test installer and complete portable package.

See [release notes and device checks](RELEASE_1.6.26.md) for the location-news correction, build/signing instructions and remaining acceptance requirements. Source checksums and external-asset restoration instructions accompany this source. Earlier notes below describe the retained baseline.

---

# R8 shared theme update

See docs/UI_THEME_R8.md. Desktop 1.6.20.0; Android 1.0.6 / versionCode 7.

# BestBuddy 1.6.19.0 — R7 test source

Extract into a new empty folder; do not merge with older source. Start with TEST_THIS_BUILD.txt and docs/ONBOARDING_SAVE_R6.md. This package contains source and build scripts, not a newly built installer.

Save stage and Next are available throughout onboarding. No extra conversational model is required. Built-in offline responses remain available, and online fallback is separately selected. English remains the speech fallback. Optional language voices and local models require enough storage and, when downloading, internet access.

## Build Windows

Run BUILD_PREVIEW.cmd to build the local preview MSIX. Run INSTALL_PREVIEW.cmd only after a successful build; Windows may request approval to trust the matching development certificate. Requirements: Windows x64, Rust MSVC, C++ Build Tools, Windows SDK, Node/npm, Python and Microsoft's winapp CLI. Speech assets and native speech libraries are downloaded from pinned sources and verified during the build. The finished MSIX bundles the English speech assets.

For Store submission, fill store/store-identity.json using the exact Partner Center values and run BUILD_STORE.cmd. The Store package is unsigned for submission. No submission is performed. See store/README_STORE.md for prerequisites. Outputs are written under release-artifacts/msix/preview or store; the script prints the actual package path. Existing artifacts are preserved; use a fresh output directory when rebuilding the same version.

Desktop local discovery checks running Ollama, LM Studio and llama.cpp at their default loopback ports, plus compatible GGUF files in standard caches. It does not launch other apps or scan every disk. Download popularity orders the compatible Hugging Face list; it is not a quality ranking.

## Verify and test

SOURCE_SHA256.json records packaged source hashes. Automated frontend checks and native compilation were performed; these do not establish real microphone/speaker performance or Store certification. English bundle installation and repair were tested on the host. Follow the R7 notes for known limitations.

No personal profiles, credentials or signing certificates are included. Existing app data is not erased. Keep your own signing identity and credentials. CSV watchlists and stock detail views remain available; see examples/watchlist.csv and docs/NEWS_MARKET_UPDATE.md.

See docs/QUIET_BRIEFING_R7.md for manual briefings, Quiet controls and voice guidance.
