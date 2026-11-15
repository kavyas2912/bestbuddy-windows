# Smart Butler Android engineering test — 1.0.3 (build 4)

This is an engineering test build, not an accepted production beta. It is a separate app with a fresh private data directory and no desktop user data. Android application identity: com.smartbutler.app. Both apps can be installed together. ARM64 phones are the current build target; minimum Android API 26 is imposed by the AAudio backend. Android 14 and 15 midrange phones remain the intended acceptance targets.

## Implemented

- Original conversation routing, analyzers, response composition, phrase books, personal lexicon and news/market logic retained. Smart Butler keeps its orangutan and own layout.
- Android Keystore AES-256-GCM protection for personal JSON, fresh nonces and authenticated context; no plaintext fallback. Existing unreadable or damaged personal files are preserved and block replacement. Android saves use atomic rename. Explicit data erasure removes the storage key.
- Microphone and notification permission requests after in-app consent and a Talk action. A microphone foreground service shows a Stop action. Stopping it prevents automatic follow-up from restarting listening. Pending permission cancellation and in-app consent withdrawal stop the service. No boot receiver silently starts recording.
- Voice installer in Voice settings: official pinned model sources, per-file integrity checks, bounded downloads and extraction, resumable partial downloads, progress and pause, archive traversal/link rejection and private staging before publication. Use Wi-Fi, leave the app open, and allow several hundred MB plus installation space. Models are not embedded in the APK.
- Compact mobile layouts and existing CSV watchlists, quote movements, charts and market news. Resume triggers consented news/price refresh. Price refresh remains five–seven minutes while the app is active; Android may defer background work.

## Try the engineering APK

1. Install the matching ARM64 APK as a test app. It is signed with an Android debug certificate, not a Play production certificate. Never distribute it as the Store release.
2. Complete onboarding with your location. Personal storage, voice processing, microphone and external-service choices remain under your control.
3. In Settings → Voice, choose Install / resume voice. If interrupted, reopen and resume. Model downloads come from the official Sherpa releases; source manifest and pinned hashes are saved alongside installed models.
4. Press Talk while the app is visible. Android asks for notifications and microphone access. Turn microphone off in Voice settings or use Stop on the persistent notification.
5. Check denial, reopening, quiet commands, reminders, CSV import, market drill-down and screen rotation before trying background listening. Export no personal data without reviewing it.

## Checks and limits

Frontend compilation and existing domain tests pass. Native model-installer tests cover confined paths, required hash pins, archive publication, modified-file detection and link rejection. ARM64 native/Gradle builds have succeeded; latest artifact/build status is recorded in the handoff report. Keystore instrumentation tests are included but require a working Android device/emulator. Compilation does not establish acoustic accuracy, low latency, battery life or first-launch acceptance.

Still required: real device startup/storage/permission tests; full model download and speech round trip; calls/headsets/audio-focus handling; battery/thermal measurements; robust OS-scheduled background refresh; Android native local language-model inference; release signing and Play policy/16 KB page compatibility acceptance. The portable desktop local-language-model executable is not available on Android; the existing consented online providers and deterministic local routing remain available. A phrase book is not unrestricted generative intelligence.

Do not claim continuous listening survives every Android battery restriction, or that background quotes are real time. Prices can be delayed and retain their timestamps/session labels. No performance numbers have been measured on a phone.

## Build

Use JDK 17 or 21, the Android SDK/NDK, installed Rust Android target and locked npm/Cargo dependencies. Run `tools/BUILD_ANDROID_ENGINEERING.ps1` with the official Sherpa Android archive in `-SpeechArchiveDirectory`. Its SHA-256 is verified. Do not use Windows libraries, signing certificates or private keys. Keep SDK paths, Gradle caches, model downloads and built libraries outside source control.

References: https://v2.tauri.app/develop/plugins/develop-mobile/ ; https://developer.android.com/privacy-and-security/keystore ; https://developer.android.com/develop/background-work/services/fgs/service-types#microphone ; https://k2-fsa.github.io/sherpa/onnx/android/index.html

## Readable setup update

Settings and onboarding now use compact subpages with readable controls, three quiet periods, password-protected profile/model/voice saving (minimum eight characters), and a confirmed personal-data wipe. News selection rejects website home and section headings. See docs/READABLE_SETUP_UPDATE.md and UI_DESIGN_RULES.md. Android local generative inference remains unavailable; its preparation control is disabled rather than offering an unsupported download.

## First-launch voice setup update

The APK now includes an offline 16 kHz English Piper Amy low starter voice (63 MB model; approximately 81 MB uncompressed TAR (compressed inside the APK)). It speaks during onboarding without requiring the full recognition downloads. First use unpacks and initializes the voice; subsequent use reuses it. It is a starter voice, not a claim of Android-native local generative inference.

First launch offers Wi-Fi or mobile data consent for automatic model downloads. The floating setup panel and a data-sync foreground notification explain progress, pause, network waiting and readiness. Wi-Fi mode avoids metered connections. Incomplete downloads automatically resume on app reopening; force-stopped apps require reopening. The service keeps setup visible in the background, with a Pause action and a bounded wake lock. Android scheduling limits still apply. No setup code runs during APK installation.

The microphone button requests Android permission only after both voice consents are saved. It does not start listening. Speaker test uses the bundled voice and needs no recording permission. Models ready plus Talk enables the existing voice engine. No microphone starts just because models finish downloading.

Phone text uses the installed Samsung/system face with Roboto fallback, 17 px body/inputs, 16 px labels, 14 px supporting text and 48 px controls. Proprietary Samsung fonts are not redistributed. See public/licenses/starter-voice for offline model and eSpeak notices.

Windows-host synthesis produced a valid 16 kHz offline sample. This is not Android timing or acoustic acceptance. BlueStacks/phone first launch, model-download completion, network changes, permission denial, speaker routes and battery behavior still need device testing.

## R5 Windows build entry point

Run BUILD_APK.cmd. Install Node.js, Rust with aarch64-linux-android target, JDK 17/21 and Android SDK/NDK first. Set JAVA_HOME, ANDROID_HOME and NDK_HOME if automatic discovery does not find them. The script downloads and verifies the official pinned Android speech runtime, installs locked frontend dependencies, and builds an ARM64 debug APK. No Play signing key is included. See docs/SETTINGS_REFRESH_2026-10-20.md for this revision and its limitations.
