# Restore external build assets

Source is complete for the archived files listed in SOURCE_SHA256.json. Signing keys, local SDK settings, generated libraries, package caches and build output are excluded.

Run `python restore_external_assets.py` before Android compilation. If assets have moved, supply `--asset-root` pointing to the retained asset folder. Existing files are verified and preserved; verified assets use a hardlink where possible. Use the locked dependency manifests and platform build scripts to restore dependencies. The Windows builder downloads and SHA-256 verifies pinned voice assets. Smart Butler's Android engineering builder restores its pinned speech libraries; run it for Android builds.

EXTERNAL_ASSETS.json records existing excluded runtime assets by hash and their retained local locations. Restore an asset to its relative path only when needed and after verifying its SHA-256. The original source remains retained. Bundled voice tar archives are preserved locally, rather than duplicated into every source archive. Android local.properties is generated for the build machine's SDK. Supply upload signing settings separately to BUILD_STORE_ANDROID.ps1; never place private keys in this source archive.
