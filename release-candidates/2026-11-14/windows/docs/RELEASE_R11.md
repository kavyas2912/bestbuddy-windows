# R11: theme consistency and installed model discovery

24 October 2026. Source candidate with build scripts, not a signed installer or device-accepted release.

## Changes and binding design rules

Every surface owns its foreground colour. Content cards use surface/text roles; action gradients use matching action ink. The final named theme-roles layer prevents legacy important selectors from splitting those pairs. Weather, clocks, forecasts, calendar, stock detail, settings, inputs, placeholders and disabled controls share this contract. Focus and active states must be checked too. New components must consume these roles rather than introduce independent text/background colours.

Preserve compact headers, readable normal-weight labels, product identity, approved features, all profile fields and optional onboarding. Avoid wrapping horizontal navigation and accidental page overflow. Do not hide clipped content to claim a layout fits. Android touch targets remain usable. Additional changes must not remove existing inputs or features without agreement.

The reported pale-text-on-gold Weather defect now has gradient-aware regression coverage. Clock hands and the Connections navigation label were also checked visually in the compiled UI. This is evidence about the tested states, not a guarantee that all possible screens or devices are defect-free.

## Local models: desktop 10 GB; Android 3 GB

The native limit is shared by catalogue filtering and discovery. Desktop discovery checks standard Ollama, LM Studio and Hugging Face cache locations, Models, supported environment overrides, and up to eight additional folders entered by the user. Scanning is bounded and read-only. It checks GGUF signatures and sizes and complete Ollama manifests/blob sizes; it does not hash all installed weights or guarantee architecture compatibility.

Installed Ollama library entries can be listed while its service is stopped; start that service to use them. Running fixed-loopback Ollama, LM Studio and llama.cpp services are queried in parallel. Choosing a service does not require the app-managed llama.cpp runtime. An imported GGUF does require the compatible app runtime. Saved additional paths are app-private metadata; wiping the profile forgets those paths, not external model files. No local model is mandatory for saving onboarding.

Android does not scan desktop application folders or gain broad shared-storage access. App-managed generative GGUF inference remains unavailable there and preparation controls say so. Existing offline commands, phrase/composer logic, speech and optional online routes are retained. This revision does not claim new Android generative inference.

Official folder/API references: https://docs.ollama.com/faq ; https://docs.ollama.com/api/tags ; https://lmstudio.ai/docs/cli/local-models/ls ; https://huggingface.co/docs/huggingface_hub/guides/manage-cache . Download metadata was checked against pinned official Hugging Face files on 24 October 2026: eight desktop choices up to 10 GB and five mobile choices up to 3 GB. No weights downloaded for that check. These are maintained choices, not an invented quality ranking.

## Verification and limits

All four production frontends built and domain suites passed. Computed UI colour checks covered 2,912 page/theme/viewport states, including gradients and input states; a further 116 interaction/model-selection states passed. These use the actual compiled UI with test data and mocked native responses. They do not prove live feeds, installed-MSIX rendering or phone behaviour. Final Weather captures were visually inspected; this is not an exhaustive human inspection of every screen.

Both desktop native library suites passed before the last scan-cache isolation refactor; all four local-discovery tests then passed again in each desktop product, including remembered-folder persistence. Both final Android ARM64 native checks passed. Formatting, manifest/version preflight and Android foundation checks passed. Existing native build warnings remain; compilation is not hardware acceptance.

The R9 integrity checklist remains the feature inventory. R11 extends LM01/LM02/LM03 with installed-library discovery, optional service selection and 10/3 GB limits; UI03/UI05 with shared surface/ink rules and gradient-aware checks. Historical R9 all-platform 3 GB wording is superseded by this document. The later Voice and Speech placement for composer/quiet periods supersedes the old Review placement rule.

Outstanding acceptance: physical-phone speech startup, permissions, safe areas, keyboard, battery and lifecycle; installed Windows visual acceptance; live-feed/provider behaviour; actual model inference compatibility and conversational quality/latency. Android engineering readiness guards remain in force. No store upload or fresh signed MSIX/APK is claimed.

## Build and inspect

Extract into a NEW empty folder. Windows: BUILD_PREVIEW.cmd, then INSTALL_PREVIEW.cmd after success; BUILD_STORE.cmd for the store route. Android: BUILD_APK.cmd (engineering build, JDK 17/21 resolver retained). Do not mix these files into an older source tree. SOURCE_SHA256.json records packaged file hashes; personal profiles, keys, certificates and build caches are excluded. Existing application identities and old milestone archives are preserved.
