# Smart Butler â€” Orangutan desktop edition

Build marker: **SB-20261018-R6-CURATED**. This is the navy, parchment, gold and teal app with the orangutan butler, top tabs and Chat, News, Market Watch, Weather and Daybook. Music, Video and Connections are removed.

Extract this archive into a NEW empty folder. Its root is `SmartButler_Source`. Do not merge it into an older source folder.

## See the layout first

Double-click `START_BUTLER_LAYOUT.cmd`. It installs locked frontend dependencies, builds the current UI, and serves that exact build at **http://127.0.0.1:1433/**. It refuses to reuse a busy port. Open that address; the page title and footer show **SB-20261018-R6-CURATED**. The browser layout preview cannot use microphone, speech or native live feeds. Use the desktop app for those functions.

## Build the Windows desktop app

Run `BUILD_PREVIEW.cmd` from THIS folder. Requirements: Windows x64 MSVC Rust, C++ Build Tools, Windows SDK, Node/npm, Python and Microsoft's winapp CLI. Large pinned speech models and native speech libraries are downloaded and verified by the build workflow; they are omitted from this source ZIP. The output is `release-artifacts/msix/preview/SmartButler_CLEAN_1.0.3.0_x64.msix`.

Run `INSTALL_PREVIEW.cmd` only after the build succeeds. It checks the package's matching local test certificate and asks for installation/trust approval. We have not installed or trusted a certificate on your behalf.

The preview uses the separate identity **SmartButler.CleanPreviewR5**, publisher **CN=SmartButler Local Preview**, and Tauri identifier **com.vce.smartbutler.cleanr5**. Its data directory is `%LOCALAPPDATA%/SmartButler/CleanR5/data`. The original app and its saved data are retained separately. The application uses shared Rust engine libraries internally. Its desktop executable and release artifacts use Butler names.

For Store packaging, populate `store/store-identity.json` with the actual identity assigned to SMART BUTLER in Partner Center and run `BUILD_STORE.cmd`. A new Butler Store listing requires its own assigned identity. No Store submission is performed by these scripts.

## Verify the extracted source

Run `python tools/verify_butler_release.py` before building. It validates the packaged SHA-256 file manifest, branding, all five poses, app icons and required UI changes. The MSIX builder also checks the Butler product identity before building. If you see a macaw or no R3 marker, you are not seeing this package's UI; stop and check the exact extracted folder, build output and URL before proceeding.

See `docs/SMART_BUTLER_R5_NOTES.md` for validation and voice limitations. Source version remains 1.0.0 (Windows mapping 1.0.3.0). This ZIP is source, not a prebuilt MSIX. Android versions of BOTH Butler and Smart Butler are the next phase after desktop acceptance; Android voice/services and packaging need platform-specific work.

This CLEAN build starts a new profile and uses a new frontend preference namespace. No previous profile, keys, contacts, reminders or conversation history are imported. Existing saved data is not erased.

This beta release uses source version **1.0.3** and Windows package version **1.0.3.0**.
