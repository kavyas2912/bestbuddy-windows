# R11 P1 small source fix — 24 October 2026

For the four R11 source ZIPs only. No app data, models or signing material included.

1. Extract this fix ZIP to its own folder. Close active source builds.
2. Double-click APPLY_FIX.cmd and choose the extracted app SOURCE folder containing Cargo.toml. Choose one app at a time; repeat for each R11 source you use.
3. The script detects the product/platform, checks every affected file, and backs up original source files under .patch-backups. It stops if your files differ, rather than overwrite your edits. Existing installed apps and personal data are not modified.
4. Rebuild using BUILD_PREVIEW.cmd and INSTALL_PREVIEW.cmd for Windows, or BUILD_APK.cmd for Android. New versions are Companion Windows 1.6.23.0, Smart Butler Windows 1.0.13.0, both Android 1.0.9 (code 10), so older build artifacts need not be overwritten. Keep the same signing identity to update an installed app. Any explicit Version in your private store identity file still needs to match the new release.

Changes: small Hear again/Quiet actions above Get started; visible privacy checkboxes; one Save & Next action with encrypted draft persistence; grouped local-model dropdown; Windows folder chooser; model download guide with official account/token links and optional encrypted token entry; preparation after model selection; online-plan popup/spoken guidance; Piper engine and English male/female speaker radios with download on selection; final preparation spinner and message; global bundled indices board and ticker followed by the selected/home stock basket; weather warning reports and local weather articles below the forecast.

The existing password-protected save boundary is retained. Download/prepare runs upon selection; persistent model routing and voice activation are committed when you save. Failed saves keep the form available for retry. Optional model/voice downloads do not replace the bundled fallback. The first release offers English voices; profile fields are preserved.

The model download list remains the compatible, pinned GGUF catalogue, rather than arbitrary model weights that the runtime cannot load. Installed libraries/services are separate. Desktop limit 10 GB, mobile limit 3 GB. Windows Explorer scanning is implemented; Android broad desktop-folder scanning and app-managed generative GGUF inference remain unavailable in the engineering build. This patch does not claim otherwise.

Weather warning rows are dated news reports (last 48 hours), not an official live alert integration. Official warning links are provided for India, UK and US. Empty/unavailable coverage is never shown as an all-clear. The selected non-home city is searched by its own location text; the profile country supplies the official warning link. Verify warnings with the local weather authority when travelling. Official links and current model/token/plan documentation were checked on 24 October 2026.

Validation: four frontend production builds and existing domain suites; both desktop native library suites and focused global-index selection tests; Android ARM64 compilation; theme/phone/desktop UI regression flow using compiled UI and mocked native/download responses. The flow checks optional models, import selection, automatic preparation, voice download, save retry, wait overlay, weather placement and ticker ordering. Native tests verify dated weather reports and global indices/home-stock filtering. Actual model downloads, Explorer interaction, audible speech and phone acceptance still require device testing. Compilation and mocked UI results are not acceptance of live services.

Official references:
https://huggingface.co/docs/hub/security-tokens
https://openrouter.ai/docs/faq
https://ai.google.dev/gemini-api/docs/pricing
https://mausam.imd.gov.in/responsive/districtWiseWarningGIS.php
https://weather.metoffice.gov.uk/warnings-and-advice/uk-warnings
https://www.weather.gov/alerts

If the patch reports edited files, keep that message and your source changes; do not bypass the hash check. It can also be run in check-only mode: powershell -NoProfile -ExecutionPolicy Bypass -File APPLY_FIX.ps1 -SourceRoot "your source folder" -CheckOnly
