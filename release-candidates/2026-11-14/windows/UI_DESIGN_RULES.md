# Shared UI design rules

These rules apply to both products on desktop and Android. Preserve product features and identities.

- Use readable text/background combinations: at least 4.5:1 normal text, 3:1 large text and meaningful control boundaries. Test active, inactive, hover, focus, disabled, input and native select states.
- Compact headers and deliberate spacing; avoid decorative padding. Keep main workspace, settings and registration shells stable in size while switching pages.
- Use short pages/tabs when fixed options do not fit. Keep primary Save/Back/Next and destructive controls visible. Lists may scroll inside their own regions. Do not hide overflow or shrink text to force a page to fit. Small screens, keyboards and increased text sizes may still scroll safely.
- Android uses touch-sized controls (at least 44 CSS px), visible labels and horizontal tab navigation where needed. Verify desktop, short windows, mobile portrait/landscape and increased text scale.
- Model lists must state ranking metric/source and refresh time; never invent quality rankings, free quotas or unsupported download choices. Tokens and keys use the app password vault; never localStorage or plain exports.
- Put composer/fallback, three optional quiet periods, app-password Save and a separate prominent red confirmed data-wipe box on Review. Finish with Meet your product.
- News entries must be actual article headlines, never home, section, navigation or publisher page titles. Apply this to briefings, tickers, business news and cached displays too.

- Desktop BestBuddy keeps the dark conversation palette across widgets, controls, settings and onboarding. Never apply light global control backgrounds without changing all nested foreground colours and checking the resulting screen. Platform-specific appearance changes do not automatically restyle other products.

Desktop appearance: use the splash navy/teal palette and Segoe UI typography consistently, including Settings and calendar surfaces. Use uniform 44px widget headers and rounded compact controls. Preserve readable labels and focus indication. Remove unnecessary container scrolling through layout and compact spacing; retain internal scrolling for long lists. See docs/SPLASH_THEME_PREVIEW.md.

R9 approved baseline: Buddy uses playful acrylic-gradient theme choices and compact normal-weight type; Butler retains a formal signature. Keep all profile and consent fields. Voice, composer and quiet-period controls stay under Voice; password/data controls stay under Review. Direct Settings navigation is required on all platforms. Existing Bootstrap light/dark options remain. No presentation change may replace native speech, conversation, prefetch, consent, model or media logic.


## R11 binding clarification — 24 October 2026

- Pair surface and text roles in readability-contract.css / theme-roles; do not independently override one side of a pair. Action gradients require matching action ink, including descendants and disabled states.
- Check gradients, opacity, input values/placeholders, selected/disabled controls, modal content, focus and hover. A passing automated check is evidence for its states only; visually inspect actual compiled UI before calling a defect fixed.
- Desktop model ceiling is 10 GB; mobile ceiling is 3 GB. Use the native shared limit for catalogues and local discovery. Do not disable a running local service just because the app-managed runtime is unavailable.
- Composer and quiet periods belong to Voice and Speech under the later approved design; Review contains the final summary/password/save and separate confirmed wipe controls.
- Preserve feature/input inventory and explicit device-acceptance gates. See docs/RELEASE_R11.md for exact scope and limitations.
