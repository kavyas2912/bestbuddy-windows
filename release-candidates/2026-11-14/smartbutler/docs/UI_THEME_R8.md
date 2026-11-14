# Shared theme and responsive dialogs — R8

Appearance is in Settings → Profile & people. Signature navy/teal remains the default. Bootstrap 5.3.8 is pinned and bundled by Vite. Bootstrap dark and light are optional. Buttons, form controls, navigation and modal frames use Bootstrap classes. Bootstrap modal JavaScript provides focus trapping, Escape/backdrop closing and backdrop placement. Legacy widget actions retain their cleanup/cancellation handlers. Theme roles adapt all controls to the selected palette.

All four apps share theme.css, theme.ts and ui-foundation.css. Colors have named roles; controls, dialogs, stock reports, settings, compact headers and horizontal tab rows consume those roles. Fonts use normal-weight Arial/Roboto fallbacks. Stock titles and close buttons stay above a separately scrolling report. Settings use one bounded frame and a content scroller. Long lists may scroll; button rows do not wrap.

The appearance choice is saved on this device independently of optional profile/model setup. It works offline and adds no CDN dependency. Existing R7 Quiet, manual briefing, category caps and phrase rotation are retained.

Validation includes production frontend builds, existing frontend tests and isolated headless browser checks with synthetic news/stock/profile data at 1366×768, 448×720 and 360×800. This does not replace physical Android device, native audio, live-source or signed installer acceptance tests. No new MSIX/APK is included in these source archives. Android local generative inference remains unavailable; bundled English speech fallback is retained.
