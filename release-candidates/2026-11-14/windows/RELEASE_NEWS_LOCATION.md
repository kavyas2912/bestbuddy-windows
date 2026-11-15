# Location news correction — 14 November 2026

BestBuddy and Smart Butler preserve their separate identities and existing features.

## Versions

| Product | Android | Windows MSIX | Desktop source |
| --- | --- | --- | --- |
| BestBuddy Senior Assistant | 1.11.6 (11106) | 1.6.25.0 | 1.6.25 |
| Smart Butler | 1.0.12 (13) | 1.0.15.0 | 1.0.15 |

## Location news behavior

- An explicit address such as Boston, MA, USA overrides an earlier India selection.
- Boston Local uses Boston publishers. Regional uses New England publishers. National and World use US publishers.
- Publisher provenance is checked against the selected source domain; a search result alone is insufficient.
- Country changes invalidate cached headlines, and late responses cannot restore headlines for the previous location.
- Android discovery no longer merges fixed Indian feeds into scoped news. Its publisher list follows the selected location.
- Weather, markets, media, voice and the products' existing branding remain in place.
- Known city publisher groups include Boston, London and Toronto. Other cities use the configured country's publishers with location-scoped queries; a local-owned publisher catalogue for every city is not claimed.

The catalogue supplies national publisher choices for all 20 countries currently offered in Android settings. City-owned publisher groups are explicitly curated only where noted above.

## Testing and publication

Debug APKs are for existing debug-signed test installations. Release APKs and Android App Bundles use new, separate upload keys approved for first publication. Keep the private-signing folder separately backed up; it is excluded from source archives.

A release-signed APK cannot update a debug-signed installation under the same application ID. Use the debug APK for an in-place test update. Do not uninstall an existing installation before preserving any needed data.

The Microsoft Store packages use the supplied Partner Center identities and are unsigned for Store signing. Direct test packages use separate preview identities/certificates. Store acceptance, account declarations, listing assets and real device acceptance remain separate from successful packaging.

Android native local language-model inference, speech/device/battery acceptance and existing feature limitations are not established by the news fix. Smart Butler's existing docs/ANDROID_PORT.md remains applicable.

Official publication references:
- https://developer.android.com/guide/practices/page-sizes
- https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/upload-app-packages
- https://learn.microsoft.com/en-us/windows/apps/publish/publish-your-app/msix/app-certification-process

Smart Butler Android retains its bundled English speech fallback and Amy starter voice. The two source build archives under tools/bundled-voice are required for Android compilation; restore them from the retained paths and SHA-256 values in EXTERNAL_ASSETS.json before building. They are excluded from source ZIPs to avoid duplicating shared model assets.

## Location test on a device

1. Enable online news and save Boston, MA, USA. Reopen News and refresh.
2. Local should show Boston publishers such as WBUR, GBH, WCVB or Boston.com. Regional should show New England publishers. National and World should show US publishers.
3. Open an article and confirm its headline and publisher. Switch between the news categories.
4. Change to Ahmedabad, India and refresh. US-scoped cached headlines must disappear.
5. Change location while a refresh is running, then restart the app and confirm the saved location and sources.
6. Check briefing, source settings, weather, market watch, voice and reminders still behave as before.
