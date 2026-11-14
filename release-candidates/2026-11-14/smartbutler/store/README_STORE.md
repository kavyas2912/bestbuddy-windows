# Smart Butler Windows packaging

Use the root README for the current R5 instructions. Build a local test package with BUILD_PREVIEW.cmd, then install it with INSTALL_PREVIEW.cmd after checking the generated package and certificate. The expected package is SmartButler_CLEAN_1.0.3.0_x64.msix. The preview identity is SmartButler.CleanPreviewR5, distinct from SmartButler.CleanPreviewR5.

For Store packaging, supply the exact Smart Butler Partner Center identity in store-identity.json and run BUILD_STORE.cmd. Store output is intentionally unsigned for Microsoft submission. Preview certificates/private keys must never be submitted or bundled in a source archive. The build acquires pinned model assets, compiles the current frontend/native app, audits the staged payload and produces MSIX. Actual installation, microphone/speaker acceptance and Store certification remain separate checks.
