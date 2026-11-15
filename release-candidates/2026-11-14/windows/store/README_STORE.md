# BestBuddy 0.6.13 — One Clean Microsoft Store Build

**This ZIP contains the complete BestBuddy Windows/Rust/Tauri source and one consolidated MSIX builder. It does not contain a compiled `.msix`, downloaded model weights, or any developer-specific installation paths.** A Windows 11 x64/MSVC machine is required to produce the final binaries.

## Build and test: three entry points

Extract the archive to a **new empty folder**. Do not extract over an earlier BestBuddy, FIX1–FIX5 or `release-artifacts` folder. From the newly extracted `BestBuddy_Source` folder:

| Goal | Run | Output |
|---|---|---|
| Build local preview | `BUILD_PREVIEW.cmd` | `release-artifacts\msix\preview\BestBuddy_1.6.13.0_x64.msix` |
| Install local preview | `INSTALL_PREVIEW.cmd` | Trusts the checked signing certificate on this PC, then installs the preview |
| Submit to Microsoft Store | `BUILD_STORE.cmd` | `release-artifacts\msix\store\BestBuddy_1.6.13.0_x64.msix` |

### Before running

Install Windows 11 x64 MSVC Rust (`rustup`, `cargo`), Microsoft C++ build tools + Windows SDK (includes `makeappx.exe`), Python 3, Node.js/npm, and Microsoft's `winapp` CLI. Install `winapp` if needed:

```powershell
winget install -e --id Microsoft.WinAppCLI --source winget
```

Open a new terminal after installation. The builder fails with an actionable message if a tool is missing. The first build needs Internet access to obtain **SHA-256 verified, pinned voice models** and native speech libraries; those assets are copied into the final MSIX, so **customers do not need the developer's filesystem or a model download for core voice functions**. The ZIP itself is small because the voice weights are not present before the Windows build.

## Test the preview package

**Important:** The preview MSIX is self-signed. Windows error `0x800B010A`
means the development certificate is not trusted on your test PC; it does
not imply that the app source is corrupt. Double-click **`INSTALL_PREVIEW.cmd`**
from the extracted source root after `BUILD_PREVIEW.cmd`. This installer verifies
that the certificate matches the signed package, prompts for confirmation and
administrator elevation, imports only the public `.cer` into **LocalMachine\TrustedPeople**,
and installs the MSIX. It does not install a root CA or submit anything to the Store.
Uninstall the preview and remove its test certificate when finished testing.

The model downloader writes only to this build's fresh MSIX stage and a
per-user build cache. It deliberately does not scan the source `models` folder;
older models with damaged ACLs (Python `PermissionError: [WinError 5]`) are
never consulted, and are not deleted. Verified models remain bundled beside
the executable in the finished MSIX. The *source ZIP* does not contain weights.

`BUILD_PREVIEW.cmd` compiles the full Tauri release, creates the MSIX using Microsoft's **native MakeAppx** (rather than a second manifest-regenerating packaging layer), generates a unique developer certificate password, signs the preview MSIX, and runs the staged/MSIX privacy checks. The developer certificate is NEVER in the MSIX.

For local installation, run `INSTALL_PREVIEW.cmd`. This handles the certificate
trust step explicitly; merely double-clicking an untrusted preview MSIX can
raise `0x800B010A` even when it was signed correctly.

If you previously installed the exact same test package identity and version, uninstall only the **previous local preview package** first using Settings > Apps, then install again. Avoid trusting the test certificate on other PCs. The preview PFX and its random password are retained **only outside the package** under `release-artifacts`, solely for local testing. **Never submit the preview MSIX to Partner Center.**

**Voice acceptance:** finish onboarding and consent; confirm that the microphone stays OFF before consent; verify ASR, spoken response, offline operation, interruption, TTS, and post-restart profile persistence. This source has not been Windows-built or hardware-tested in this environment.

## Store submission package

1. In Partner Center, use an **MSIX/PWA** app registration for the reserved BestBuddy name. An MSI/EXE entry asking for an HTTPS installer URL is **not** an MSIX upload.
2. In Partner Center, open BestBuddy > **Product management > App identity**. Copy the **exact Package/Identity name, Publisher distinguished name, and PublisherDisplayName** (values are case-sensitive; do not guess).
3. The `store\store-identity.json` is prefilled using the *public package identity* and publisher ID in your build output. **Verify the PublisherDisplayName exactly matches Partner Center**; the display name was not shown in the log. Version is `1.6.13.0`. It has no private key, secret or machine path.
4. Run `BUILD_STORE.cmd`. It compiles a release and uses native MakeAppx to create a fully populated **unsigned** Store MSIX. Microsoft signs qualifying Store submissions; no commercial signing certificate or website/CDN is needed for MSIX apps.
5. Run the **Windows App Certification Kit** on your build PC, test on real hardware, confirm bundled model redistribution rights, complete screenshots, age/content rating, privacy-policy URL, support information and consent disclosures. Upload the unsigned MSIX in Partner Center's **MSIX Packages** upload section.

### What the builder validates

- Rust source/version preflight, pinned model SHA-256s, required executable/voice/asset files.
- A correctly ordered `AppxManifest.xml`: `internetClient`, `runFullTrust`, then `microphone`.
- A single compiler-produced EXE with bundled verified ASR, VAD, TTS/eSpeak data, KWS, and denoising models.
- Privacy validation of both staged files and final MSIX; signing keys, profiles, logs, optional GGUFs, and actual developer home paths are forbidden.
- MSIX SHA-256 and output build-information JSON.

The script **never uploads to GitHub or Partner Center** and **never deletes an earlier workspace**. It creates new output only inside the freshly extracted source folder and a dynamic per-user Cargo build cache. Build failures retain staged files for diagnosis. Your personal data created *after installation* are still stored in the installing user's own application-data area.

## Model and license disclosure

See `THIRD_PARTY_SOURCES.md` and `third_party_assets.json` for vendor/model download URLs and versions. SHA-256 verification is not a legal redistribution license. Complete an asset-by-asset license/attribution review before uploading to the Store. The app's optional LLM/intelligence models and voice downloads are not included in the base MSIX. Verify WebView2/runtime and offline speech on a clean Windows device before shipping.

## Why this rebuild is different

Previous attempts mixed `winapp pack`, scripted manifest edits, stage re-use, and `winapp tool makeappx` calls which sometimes returned help instead of creating a file. This build uses **exactly one generated manifest** named `AppxManifest.xml`, validates ordering, invokes the x64 Microsoft SDK `makeappx.exe` directly, checks the resulting artifact, and uses `winapp sign` only to sign the preview. It does not depend on previous FIX folders.
