# BestBuddy 1.6.13.0 — News and Market Watch update

Baseline: the supplied 0.6.12 source whose Store build record identifies BestBuddy 1.6.12.0. The user has confirmed that version was uploaded to Microsoft Store. The original source ZIP and build records are preserved separately.

The update retains BestBuddy branding, app identity, macaw assets, conversation engine, music, video and connections. The new source version is 0.6.13, mapped by the existing Windows builder to MSIX 1.6.13.0. No new MSIX or Store submission is claimed by this source update.

News now uses the same topic/location-aware source registry and prefetch service as Smart Butler: home-country publishers, regional/state/city candidates, publication-date filtering, bundled exchange catalogs and source health. Category-specific source choices appear in the News header. Related coverage is grouped below the existing compact headline row. General headlines older than three calendar months are excluded; science and technology retain their established exception.

Market Watch has compact header/control/card spacing, three small watchlist controls at the upper left, and an in-pane overlay showing each stock's latest available price and daily percentage movement. Selecting a stock opens its price/dated chart, available statistics, current company coverage, published analyst views and computed daily SMA20/SMA50/RSI14 when sufficient history exists. Missing provider fields remain unavailable; no ratings or trade recommendations are invented. Provider pages remain available through explicit links.

CSV imports are local, reviewed before adding to a draft, and saved only with Save watchlists. Append is the default; replacement changes only the selected draft. Each list supports 50 symbols, each CSV at most 256 KB and 1,000 rows. Symbol/Ticker/Trading Symbol headers, optional Name/Exchange fields, quoted fields, semicolon/tab exports and one-column symbol files are accepted. Exchange prefixes and suffixes are normalized; duplicates and invalid rows are reported. See examples/watchlist.csv.

Prices retain the chosen 5/6/7-minute refresh, cache age and delayed/closed-session labels. New watchlist fields default safely for existing profiles and use the existing encrypted profile storage. Existing data is not bundled in this archive. Preview signing material and old generated packages are excluded.

Validation is recorded separately. Source compilation and tests do not establish device microphone behaviour, external-feed availability or Microsoft Store acceptance.
