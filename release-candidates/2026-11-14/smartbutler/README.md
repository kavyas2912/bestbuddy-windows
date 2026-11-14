# Smart Butler — current testing candidate

14 November 2026. Android 1.0.12 (13); Windows 1.0.15 / MSIX 1.0.15.0.

See [release notes and device checks](RELEASE_NEWS_LOCATION.md) for the location-news correction, build/signing instructions and remaining acceptance requirements. Source checksums and external-asset restoration instructions accompany this source. Earlier notes below describe the retained baseline.

---

# R8 shared theme update

See docs/UI_THEME_R8.md. Desktop 1.0.9.0; Android 1.0.6 / versionCode 7.

# Smart Butler 1.0.5 — R7 test source

Extract into a new empty folder; do not merge with older source. Start with TEST_THIS_BUILD.txt and docs/ONBOARDING_SAVE_R6.md. This package contains source and build scripts, not a newly built installer.

Save stage and Next are available throughout onboarding. No extra conversational model is required. Built-in offline responses remain available, and online fallback is separately selected. English remains the speech fallback. Optional language voices and local models require enough storage and, when downloading, internet access.

## Build Android

Run BUILD_APK.cmd from the extracted root. Requirements: Node/npm, Rust with aarch64-linux-android target, Android SDK/NDK, JDK 17 or 21. Set ANDROID_HOME, NDK_HOME and JAVA_HOME if automatic discovery cannot locate them. The workflow fetches the checksum-pinned native speech library and builds an ARM64 engineering APK. See docs/ANDROID_ENGINEERING_R4.md and docs/ANDROID_PORT.md for prerequisites and device checks.

Android version is 1.0.5, versionCode 6. A light onboarding voice and the English STT/TTS fallback archive are bundled. Optional voices do not delete English. Native local generative inference is not enabled; model preferences do not imply that this runtime is available. Real phone audio, battery, interruptions and latency still require testing. No Play Store release or signing identity is supplied.

## Verify and test

SOURCE_SHA256.json records packaged source hashes. Automated frontend checks and native compilation were performed; these do not establish real microphone/speaker performance or Store certification. English bundle installation and repair were tested on the host. Follow the R7 notes for known limitations.

No personal profiles, credentials or signing certificates are included. Existing app data is not erased. Keep your own signing identity and credentials. CSV watchlists and stock detail views remain available; see examples/watchlist.csv and docs/NEWS_MARKET_UPDATE.md.

See docs/QUIET_BRIEFING_R7.md for manual briefings, Quiet controls and voice guidance.
