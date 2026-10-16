# Cleanup and code audit

The supplied original tree was read only. Cleaned repositories were assembled separately.

## Selection

The top-level Kotlin Multiplatform MyApplication tree (`app`, `core`, `server`, root Gradle files) is a separate starter application with a default Click me screen. It is not referenced by the BestBuddy Cargo workspace, Tauri frontend or Android build script and was excluded from these deliverables. No source was deleted from the original.

IDE state, generated build caches, schemas, stale verification hashes, historical reports and unrelated Apple/Linux packaging are excluded. Windows omits the Android Gradle host; Android omits Store and desktop setup/run scripts. Both retain all Cargo workspace members, including buddy-api, and the existing shared UI and feature code.

`SOURCE_SELECTION.json` records the initial file selection and pruned directory names. `FILES_SHA256.json` records the final deliverable files. Files omitted by the platform-specific final pass are reflected in the final manifest.

## Fixes

- Rust annual reminders previously fired before their configured first year. They now follow the frontend rule and reject lead periods above 366 days. A regression test covers the initial year and upper bound.
- WAV diagnostics now use the running voice engine's configured model paths, including packaged model locations and selected voice paths, rather than reconstructing environment defaults.
- Ignore rules exclude models, native libraries, build outputs, host-local Android Gradle paths and signing secrets.
- Android version metadata is 0.6.12 / 6012. Stale generated host paths and bundled configuration snapshots are not shipped.
- Android setup prepares Android dependencies without requiring a Windows desktop build. JDK/NDK discovery no longer pins a nonexistent machine-specific JDK/NDK path.
- Android native downloads are checked against the existing pinned archive hash. APK inspection checks command failure and rejects stale output.
- Windows preflight validates the committed Cargo lock and restores its caller's working directory. Test scripts restore the frontend working directory on failure.
- Voice verification checks every pinned model hash, including denoise, instead of checking only nonempty files.
- Model download CMD delegates to the existing hash-verified Python preparation helper instead of accepting token-file presence as proof of complete models.
- Frontend repair and setup install committed npm dependencies. Reminder regression tests are available through npm test.
- Source archives use Git-tracked committed content, avoiding cache leakage and obsolete hardcoded archive versions.

## Remaining risks

Android asset installation runs in a background thread after launch. The Rust runtime retries startup every five seconds, but first-install speech and permission behavior still need device testing. The supplied Android fallback packages a freshly built JNI library after a Tauri error; this is preserved but needs validation on a host without Developer Mode. The generated Gradle build task retries Windows executable extensions after failures, which can obscure the first error. These paths have not been certified by static inspection.

Live service behavior, real microphone/speaker operation, Store signing/submission and Android release signing remain separate acceptance work. No UI elements or integrations were redesigned or removed.
