# Current location-news testing sources

15 November 2026. These candidates come from the locally recovered app sources, rather than the older 0.8.0 baseline at this repository root. Build from the relevant folder below.

- BestBuddy Android: 1.11.7 (11107), `release-candidates/2026-11-15/android` in the Android repository.
- BestBuddy Windows: 1.6.25 / MSIX 1.6.25.0, `release-candidates/2026-11-14/windows` in the Windows repository.
- Smart Butler Android: 1.0.12 (13), Windows: 1.0.15 / MSIX 1.0.15.0, shared source under `release-candidates/2026-11-14/smartbutler` in the Windows repository.

Boston uses Boston publishers locally, New England publishers regionally, and US publishers for national/world news. Changing countries invalidates cached headlines. Publisher provenance is checked against source domains. The shared catalogue covers all 20 country choices offered by Android settings.

The source includes current release notes, source checksums and external-asset restoration instructions. Private signing keys and passwords, model archives, generated native libraries and build caches are excluded. Smart Butler Android needs its retained, checksum-verified bundled speech archives; run `restore_external_assets.py` before building.

Validation: 25 Android JavaScript checks and 61 core tests pass. Native news plugin tests and both frontend test/build checks pass. A live native Boston probe returned WBUR local, NHPR regional, and NPR national/world headlines. Device voice, permissions and store certification still require acceptance; these are testing sources.

The Android 1.11.7 correction adds audible launch greetings, the prominent spoken daily briefing action, real selected-country persistence, an optional Senior role, bounded casual questions, permission-controlled learning, timely reminders and fresh companionship. Validation: 65 core tests and 45 JavaScript checks pass, including seven full-app DOM/native-command scenarios; phone audio acceptance remains separate.
