static REFRESH_SECONDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(360);
pub fn set_refresh_minutes(minutes: u8) -> Result<(), String> {
    let minutes = if minutes == 0 { 6 } else { minutes };
    if !(5..=7).contains(&minutes) {
        return Err("Choose 5–7 minutes".into());
    }
    REFRESH_SECONDS.store(minutes as u64 * 60, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}
use async_trait::async_trait;
use buddy_plugin::{
    BuddyPlugin, Permission, PluginError, PluginManifest, PluginRequest, PluginResponse,
};
use serde_json::{json, Value};

pub struct MarketPlugin;
impl MarketPlugin {
    pub fn new() -> Self {
        Self
    }
}

pub fn exchange_catalog() -> &'static Value {
    static REGISTRY: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| {
        serde_json::from_str(include_str!("../market_registry.json"))
            .expect("bundled exchange registry")
    })
}
pub fn home_exchange(country: &str) -> Option<&'static str> {
    let key = match country.trim().to_ascii_lowercase().as_str() {
        "usa" | "us" => "united states".into(),
        "uk" | "gb" => "united kingdom".into(),
        "in" => "india".into(),
        other => other.to_string(),
    };
    exchange_catalog()
        .as_array()?
        .iter()
        .find(|row| {
            row["country"]
                .as_str()
                .is_some_and(|country| country.to_lowercase() == key)
        })
        .and_then(|row| row["id"].as_str())
}
pub fn country_for_exchange(exchange: &str, home: &str) -> String {
    exchange_catalog()
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["id"] == exchange))
        .and_then(|row| row["country"].as_str())
        .unwrap_or(home)
        .to_string()
}
fn market_defs() -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    exchange_catalog()
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|market| {
            market["symbols"]
                .as_array()
                .unwrap()
                .iter()
                .map(move |symbol| {
                    (
                        symbol["name"].as_str().unwrap(),
                        symbol["code"].as_str().unwrap(),
                        market["id"].as_str().unwrap(),
                        symbol["kind"].as_str().unwrap(),
                    )
                })
        })
        .collect()
}
fn normalize_market_symbol(
    s: &str,
) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
    let q = s.trim().to_ascii_lowercase();
    market_defs()
        .into_iter()
        .find(|(name, code, _region, _kind)| {
            q == name.to_ascii_lowercase()
                || q.eq_ignore_ascii_case(code)
                || q == name.to_ascii_lowercase().replace(' ', "-")
        })
}

fn quote_price(q: &Value) -> Option<f64> {
    q.get("close")
        .and_then(|v| {
            v.as_f64()
                .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
        })
        .filter(|p| p.is_finite())
}
#[cfg(test)]
fn parse_quote_response(value: &Value, symbol: &str) -> Result<Value, PluginError> {
    value
        .get("symbols")
        .and_then(Value::as_array)
        .and_then(|symbols| symbols.first())
        .cloned()
        .ok_or_else(|| {
            PluginError::Failed(format!(
                "Stooq returned no quote for {symbol}; response: {}",
                value.to_string().chars().take(180).collect::<String>()
            ))
        })
}
fn yahoo_symbol(symbol: &str) -> Option<&'static str> {
    exchange_catalog()
        .as_array()?
        .iter()
        .flat_map(|row| row["symbols"].as_array().unwrap())
        .find(|row| row["code"] == symbol)
        .and_then(|row| row["provider_symbol"].as_str())
}
fn parse_yahoo_quote(value: &Value) -> Result<Value, PluginError> {
    let meta = value
        .pointer("/chart/result/0/meta")
        .ok_or_else(|| PluginError::Failed("Market provider returned no quote metadata".into()))?;
    let price = meta
        .get("regularMarketPrice")
        .and_then(Value::as_f64)
        .filter(|price| price.is_finite() && *price > 0.0)
        .ok_or_else(|| PluginError::Failed("Market provider returned no valid price".into()))?;
    let previous = meta
        .get("previousClose")
        .or_else(|| meta.get("chartPreviousClose"))
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0);
    let change = previous.map(|value| price - value);
    let change_percent = previous.map(|value| (price - value) / value * 100.0);
    Ok(
        json!({"close":price,"previous_close":previous,"change":change,"change_percent":change_percent,"currency":meta.get("currency"),"timestamp":meta.get("regularMarketTime"),"source":"Yahoo Finance","symbol":meta.get("symbol"),"exchange":meta.get("exchangeName"),"market_state":meta.get("marketState"),"data_delay":meta.get("exchangeDataDelayedBy"),"quote_status":"Latest available; may be delayed","session":session_status(meta,epoch_seconds()),"stale":session_status(meta,epoch_seconds())=="Open"&&meta["regularMarketTime"].as_u64().is_some_and(|timestamp|epoch_seconds().saturating_sub(timestamp)>900),"fetched_at":epoch_seconds(),"regular_session":meta.pointer("/currentTradingPeriod/regular")}),
    )
}
async fn fetch_yahoo_quote_at(symbol: &str, host: &str) -> Result<Value, PluginError> {
    let url = format!(
        "https://{host}/v8/finance/chart/{}?interval=1m&range=1d",
        urlencoding::encode(symbol)
    );
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| PluginError::Failed(e.to_string()))?
        .get(url)
        .header("User-Agent", "BestBuddy/0.6.13")
        .send()
        .await
        .map_err(|e| PluginError::Failed(e.to_string()))?;
    if !response.status().is_success() {
        return Err(PluginError::Failed(format!(
            "Market provider HTTP {}",
            response.status()
        )));
    }
    let value = response
        .json::<Value>()
        .await
        .map_err(|e| PluginError::Failed(e.to_string()))?;
    parse_yahoo_quote(&value)
}
async fn fetch_yahoo_quote(symbol: &str) -> Result<Value, PluginError> {
    match fetch_yahoo_quote_at(symbol, "query1.finance.yahoo.com").await {
        Ok(quote) => Ok(quote),
        Err(first) => fetch_yahoo_quote_at(symbol, "query2.finance.yahoo.com")
            .await
            .map_err(|second| {
                PluginError::Failed(format!("Primary: {first}; alternate: {second}"))
            }),
    }
}
static QUOTES: std::sync::OnceLock<
    tokio::sync::Mutex<std::collections::HashMap<String, (u64, Value)>>,
> = std::sync::OnceLock::new();
static QUOTE_LOCKS: std::sync::OnceLock<
    tokio::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>>,
> = std::sync::OnceLock::new();
fn epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn session_status(meta: &Value, now: u64) -> &'static str {
    let start = meta
        .pointer("/currentTradingPeriod/regular/start")
        .and_then(Value::as_u64);
    let end = meta
        .pointer("/currentTradingPeriod/regular/end")
        .and_then(Value::as_u64);
    match (start, end) {
        (Some(start), Some(end)) if start <= now && now < end => "Open",
        (Some(_), Some(_)) => "Closed — last regular-session price",
        _ => match meta["marketState"].as_str() {
            Some("REGULAR") => "Open",
            Some("CLOSED" | "PRE" | "POST" | "PREPRE" | "POSTPOST") => {
                "Closed — last regular-session price"
            }
            _ => "Session unknown",
        },
    }
}
async fn fetch_quote(symbol: &str) -> Result<Value, PluginError> {
    let symbol = yahoo_symbol(symbol).unwrap_or(symbol);
    let cache = QUOTES.get_or_init(|| tokio::sync::Mutex::new(std::collections::HashMap::new()));
    let lock = {
        let mut locks = QUOTE_LOCKS
            .get_or_init(|| tokio::sync::Mutex::new(std::collections::HashMap::new()))
            .lock()
            .await;
        locks
            .entry(symbol.into())
            .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    };
    let _guard = lock.lock().await;
    let prior = cache.lock().await.get(symbol).cloned();
    if let Some((at, quote)) = &prior {
        if epoch_seconds().saturating_sub(*at)
            < REFRESH_SECONDS.load(std::sync::atomic::Ordering::SeqCst)
        {
            let mut quote = quote.clone();
            if quote["regular_session"].is_object() {
                quote["session"] = json!(session_status(
                    &json!({"currentTradingPeriod":{"regular":quote["regular_session"]}}),
                    epoch_seconds()
                ));
            }
            return Ok(quote);
        }
    }
    match fetch_yahoo_quote(symbol).await {
        Ok(quote) => {
            let mut cache = cache.lock().await;
            if cache.len() > 256 {
                cache.clear();
            }
            cache.insert(symbol.into(), (epoch_seconds(), quote.clone()));
            Ok(quote)
        }
        Err(error) => {
            if let Some((_, mut quote)) = prior {
                quote["stale"] = json!(true);
                quote["refresh_error"] = json!(error.to_string());
                quote["quote_status"] = json!("Cached — refresh failed");
                Ok(quote)
            } else {
                Err(error)
            }
        }
    }
}

fn selected_components(symbol: &str) -> &'static [(&'static str, &'static str)] {
    match symbol {
        "^nsei" | "^NSEI" => &[
            ("Reliance", "RELIANCE.NS"),
            ("HDFC Bank", "HDFCBANK.NS"),
            ("ICICI Bank", "ICICIBANK.NS"),
            ("Infosys", "INFY.NS"),
            ("TCS", "TCS.NS"),
        ],
        "^bsesn" | "^BSESN" => &[
            ("Reliance", "RELIANCE.BO"),
            ("HDFC Bank", "HDFCBANK.BO"),
            ("ICICI Bank", "ICICIBANK.BO"),
            ("Infosys", "INFY.BO"),
            ("TCS", "TCS.BO"),
        ],
        "^spx" | "^GSPC" | "^ndq" | "^IXIC" => &[
            ("Apple", "AAPL"),
            ("Microsoft", "MSFT"),
            ("Nvidia", "NVDA"),
            ("Amazon", "AMZN"),
            ("Alphabet", "GOOGL"),
        ],
        "^dji" | "^DJI" => &[
            ("Apple", "AAPL"),
            ("Microsoft", "MSFT"),
            ("Goldman Sachs", "GS"),
            ("Visa", "V"),
            ("Walmart", "WMT"),
        ],
        "^ftse" | "^FTSE" => &[
            ("Shell", "SHEL.L"),
            ("HSBC", "HSBA.L"),
            ("AstraZeneca", "AZN.L"),
        ],
        "^dax" | "^GDAXI" => &[
            ("SAP", "SAP.DE"),
            ("Siemens", "SIE.DE"),
            ("Allianz", "ALV.DE"),
        ],
        "^nky" | "^N225" => &[
            ("Toyota", "7203.T"),
            ("Sony", "6758.T"),
            ("SoftBank", "9984.T"),
        ],
        _ => &[],
    }
}

fn csv_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                fields.push(field.trim().to_string());
                field.clear();
            }
            _ => field.push(ch),
        }
    }
    fields.push(field.trim().to_string());
    fields
}

async fn index_components(symbol: &str) -> (Vec<Value>, bool, String, String) {
    if matches!(symbol, "^nsei" | "^NSEI") {
        let url = "https://www.niftyindices.com/IndexConstituent/ind_nifty50list.csv";
        let result = async {
            let text = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(12)).build()
                .map_err(|e| e.to_string())?.get(url)
                .header("User-Agent", "BestBuddy/0.6.13")
                .send().await.map_err(|e| e.to_string())?
                .error_for_status().map_err(|e| e.to_string())?
                .text().await.map_err(|e| e.to_string())?;
            let mut lines = text.lines();
            let headers = csv_fields(lines.next().unwrap_or(""));
            let name_at = headers.iter().position(|x| x.eq_ignore_ascii_case("Company Name")).unwrap_or(0);
            let symbol_at = headers.iter().position(|x| x.eq_ignore_ascii_case("Symbol")).unwrap_or(2);
            let industry_at = headers.iter().position(|x| x.eq_ignore_ascii_case("Industry"));
            let rows = lines.filter_map(|line| {
                let fields = csv_fields(line);
                let name = fields.get(name_at)?.trim();
                let code = fields.get(symbol_at)?.trim();
                if name.is_empty() || code.is_empty() { return None; }
                Some(json!({"name":name,"symbol":format!("{code}.NS"),"industry":industry_at.and_then(|i|fields.get(i)).cloned().unwrap_or_default()}))
            }).collect::<Vec<_>>();
            if rows.len() < 45 { return Err(format!("official constituent file returned only {} rows", rows.len())); }
            Ok::<_, String>(rows)
        }.await;
        if let Ok(rows) = result {
            return (
                rows,
                true,
                "Complete official NIFTY 50 constituent file".into(),
                "https://www.niftyindices.com/indices/equity/broad-based-indices/nifty--50".into(),
            );
        }
    }
    let rows = selected_components(symbol)
        .iter()
        .map(|(name, code)| json!({"name":name,"symbol":code}))
        .collect();
    let official = match symbol {
        "^bsesn" | "^BSESN" => "https://www.bseindices.com/indices-equity/sp-bse-sensex-167.html",
        "^spx" | "^GSPC" => "https://www.spglobal.com/spdji/en/indices/equity/sp-500/",
        "^dji" | "^DJI" => {
            "https://www.spglobal.com/spdji/en/indices/equity/dow-jones-industrial-average/"
        }
        "^ftse" | "^FTSE" => "https://www.lseg.com/en/ftse-russell/indices/uk",
        "^dax" | "^GDAXI" => "https://www.stoxx.com/index-details?symbol=DAX",
        "^nky" | "^N225" => "https://indexes.nikkei.co.jp/en/nkave/index/profile",
        _ => "https://finance.yahoo.com/markets/world-indices/",
    };
    (rows, false, "Selected companies are shown here; open the index publisher for complete current membership.".into(), official.into())
}
fn chart_interval(range: &str) -> Result<&'static str, PluginError> {
    match range {
        "1d" => Ok("5m"),
        "1mo" | "6mo" | "1y" => Ok("1d"),
        "5y" => Ok("1wk"),
        _ => Err(PluginError::Failed("Unsupported chart range".into())),
    }
}
async fn fetch_yahoo_chart(symbol: &str, range: &str) -> Result<Value, PluginError> {
    let interval = chart_interval(range)?;
    if symbol.len() > 32
        || !symbol
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "^.-=".contains(c))
    {
        return Err(PluginError::Failed("Invalid market symbol".into()));
    }
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval={interval}&range={range}",
        urlencoding::encode(symbol)
    );
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| PluginError::Failed(e.to_string()))?
        .get(url)
        .header("User-Agent", "BestBuddy/0.6.13")
        .send()
        .await
        .map_err(|e| PluginError::Failed(e.to_string()))?;
    if !response.status().is_success() {
        return Err(PluginError::Failed(format!(
            "Chart HTTP {}",
            response.status()
        )));
    }
    let data = response
        .json::<Value>()
        .await
        .map_err(|e| PluginError::Failed(e.to_string()))?;
    let quote = if range == "1d" {
        parse_yahoo_quote(&data)?
    } else {
        // A long-range chartPreviousClose is the range baseline, not yesterday's
        // closing price. Keep the displayed daily movement on the daily quote feed.
        fetch_quote(symbol).await.unwrap_or_else(|_| {
            let mut quote = parse_yahoo_quote(&data).unwrap_or(Value::Null);
            if let Some(object) = quote.as_object_mut() {
                object.remove("change");
                object.remove("change_percent");
                object.remove("previous_close");
            }
            quote
        })
    };
    let times = data
        .pointer("/chart/result/0/timestamp")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let closes = data
        .pointer("/chart/result/0/indicators/quote/0/close")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let points = times
        .iter()
        .zip(closes.iter())
        .filter_map(|(t, p)| Some(json!([t.as_i64()?, p.as_f64()?])))
        .collect::<Vec<_>>();
    let adjusted = data
        .pointer("/chart/result/0/indicators/adjclose/0/adjclose")
        .and_then(Value::as_array)
        .unwrap_or(&closes);
    let history = times
        .iter()
        .zip(adjusted.iter())
        .filter_map(|(time, price)| Some(json!([time.as_i64()?, price.as_f64()?])))
        .collect::<Vec<_>>();
    let meta = data
        .pointer("/chart/result/0/meta")
        .cloned()
        .unwrap_or(Value::Null);
    let (components, components_complete, components_note, components_url) =
        index_components(symbol).await;
    Ok(
        json!({"quote":quote,"points":points,"history":history,"range":range,"interval":interval,"metadata":meta,"components":components,"components_complete":components_complete,"components_note":components_note,"components_url":components_url}),
    )
}

#[async_trait]
impl BuddyPlugin for MarketPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: "markets".into(),
            name: "Markets".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            icon: "market".into(),
            capabilities: vec![
                "market.quote".into(),
                "market.watchlist".into(),
                "market.dashboard".into(),
                "market.detail".into(),
            ],
            permissions: vec![Permission::Network],
        }
    }

    async fn execute(&self, r: PluginRequest) -> Result<PluginResponse, PluginError> {
        if r.capability == "market.detail" {
            let raw = r
                .params
                .get("symbol")
                .and_then(Value::as_str)
                .ok_or_else(|| PluginError::Failed("Market symbol is required".into()))?;
            let symbol = yahoo_symbol(raw).unwrap_or(raw);
            let range = r
                .params
                .get("range")
                .and_then(Value::as_str)
                .unwrap_or("1d");
            let data = fetch_yahoo_chart(symbol, range).await?;
            return Ok(PluginResponse {
                plugin_id: "markets".into(),
                capability: r.capability,
                text: "Market detail loaded".into(),
                data,
            });
        }
        if r.capability == "market.dashboard" || r.capability == "market.watchlist" {
            let mut items = Vec::new();
            let mut tasks = tokio::task::JoinSet::new();
            let exchange = r
                .params
                .get("exchange")
                .and_then(Value::as_str)
                .unwrap_or("all");
            let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
            for (name, code, region, kind) in
                market_defs().into_iter().filter(|(_, _, region, kind)| {
                    include_market_row(
                        exchange,
                        region,
                        kind,
                        r.params["include_global_indices"]
                            .as_bool()
                            .unwrap_or(false),
                    )
                })
            {
                let permits = permits.clone();
                tasks.spawn(async move {
                    let _permit=permits.acquire_owned().await.expect("quote semaphore is open");
                    let result = fetch_quote(code).await;
                    let error = result.as_ref().err().map(|e| e.to_string());
                    let q = result.unwrap_or(Value::Null);
                    let price = quote_price(&q);
                    json!({"name":name,"symbol":code,"region":region,"kind":kind,"price":price,"close":price,"change":q.get("change"),"change_percent":q.get("change_percent"),"currency":q.get("currency"),"source":q.get("source"),"timestamp":q.get("timestamp"),"quote":q,"market_status":q.get("session"),"stale":q.get("stale"),"error":error})
                });
            }
            while let Some(result) = tasks.join_next().await {
                if let Ok(item) = result {
                    items.push(item);
                }
            }
            let live = items.iter().filter(|x| x["price"].is_number()).count();
            let text = if live > 0 {
                format!("I refreshed the world market board: {} of {} leading market feeds returned a latest quote.",live,items.len())
            } else {
                "The market board is unavailable right now; I'll keep the watchlist ready for another refresh.".into()
            };
            return Ok(PluginResponse {
                plugin_id: "markets".into(),
                capability: r.capability,
                text,
                data: json!({"items":items,"live":live}),
            });
        }
        let raw = r
            .params
            .get("symbol")
            .and_then(|v| v.as_str())
            .unwrap_or("AAPL");
        let (label, symbol, region, kind) =
            normalize_market_symbol(raw).unwrap_or(("Equity", raw, "USA", "equity"));
        let q = fetch_quote(symbol).await?;
        let text=match quote_price(&q){Some(price)=>format!("The latest available quote for {label} is {price:.2}, from Yahoo Finance. It may be delayed."),None=>format!("I could not retrieve a current quote for {label}.")};
        Ok(PluginResponse {
            plugin_id: "markets".into(),
            capability: r.capability,
            text,
            data: json!({"name":label,"symbol":symbol,"region":region,"kind":kind,"close":quote_price(&q),"quote":q}),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use buddy_plugin::BuddyPlugin;
    #[test]
    fn chart_ranges_use_daily_bars_for_daily_indicators() {
        assert_eq!(chart_interval("1d").unwrap(), "5m");
        assert_eq!(chart_interval("1y").unwrap(), "1d");
        assert_eq!(chart_interval("5y").unwrap(), "1wk");
        assert!(chart_interval("../../../").is_err());
    }
    #[test]
    fn manifest_exposes_market_capabilities() {
        let m = MarketPlugin::new().manifest();
        assert_eq!(m.id, "markets");
        assert!(m.capabilities.contains(&"market.dashboard".to_string()));
        assert!(m.capabilities.contains(&"market.watchlist".to_string()));
    }
    #[test]
    fn quote_accepts_numbers_and_strings_but_not_missing_prices() {
        assert_eq!(quote_price(&json!({"close":123.5})), Some(123.5));
        assert_eq!(quote_price(&json!({"close":"123.5"})), Some(123.5));
        assert_eq!(quote_price(&json!({"close":"N/D"})), None);
        assert_eq!(quote_price(&json!({})), None);
    }
    #[test]
    fn common_market_names_resolve() {
        assert_eq!(
            normalize_market_symbol("NIFTY 50").map(|x| x.1),
            Some("^nsei")
        );
        assert_eq!(
            normalize_market_symbol("S&P 500").map(|x| x.1),
            Some("^spx")
        );
    }
    #[test]
    fn stooq_json_uses_symbols_not_data() {
        let response = json!({"symbols":[{"symbol":"^SPX","close":6484.2}]});
        assert_eq!(
            quote_price(&parse_quote_response(&response, "^spx").unwrap()),
            Some(6484.2)
        );
        assert!(parse_quote_response(&json!({"data":[]}), "^spx").is_err());
    }
    #[test]
    fn yahoo_chart_quote_preserves_value_and_provenance() {
        let response = json!({"chart":{"result":[{"meta":{"symbol":"^GSPC","currency":"USD","regularMarketPrice":7764.64,"regularMarketTime":1790109395}}]}});
        let quote = parse_yahoo_quote(&response).unwrap();
        assert_eq!(quote_price(&quote), Some(7764.64));
        assert_eq!(quote["source"], "Yahoo Finance");
        assert_eq!(yahoo_symbol("^nsei"), Some("^NSEI"));
    }
    #[test]
    fn constituent_csv_preserves_quoted_company_names() {
        assert_eq!(
            csv_fields("\"Example Holdings, Ltd.\",Financial Services,EXAMPLE"),
            vec!["Example Holdings, Ltd.", "Financial Services", "EXAMPLE"]
        );
    }
    #[tokio::test]
    #[ignore = "requires live official Nifty constituent file"]
    async fn official_nifty_file_returns_complete_membership() {
        let (items, complete, _, source) = index_components("^nsei").await;
        assert!(complete);
        assert!(items.len() >= 45);
        assert!(source.starts_with("https://www.niftyindices.com/"));
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    #[test]
    fn home_exchange_and_close_status() {
        for (_, code, _, _) in market_defs() {
            assert!(normalize_market_symbol(code).is_some(), "{code}");
        }
        assert_eq!(home_exchange("Australia"), Some("Australia"));
        assert_eq!(home_exchange("US"), Some("USA"));
        assert_eq!(home_exchange("Unknown"), None);
        let meta = json!({"currentTradingPeriod":{"regular":{"start":100,"end":200}}});
        assert_eq!(session_status(&meta, 150), "Open");
        assert!(session_status(&meta, 250).starts_with("Closed"));
        assert_eq!(session_status(&json!({}), 250), "Session unknown");
        assert!(market_defs()
            .iter()
            .any(|(_, _, country, kind)| *country == "India" && *kind == "stock"));
    }
}

pub async fn quote_symbols(symbols: Vec<(String, String, String)>) -> Value {
    let permits = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
    let mut tasks = tokio::task::JoinSet::new();
    for (name, symbol, exchange) in symbols.into_iter().take(50) {
        let permits = permits.clone();
        tasks.spawn(async move{let _permit=permits.acquire_owned().await.ok();let result=fetch_quote(&symbol).await;match result{Ok(quote)=>json!({"name":name,"symbol":symbol,"region":exchange,"kind":"stock","price":quote["close"],"change":quote["change"],"change_percent":quote["change_percent"],"currency":quote["currency"],"timestamp":quote["timestamp"],"source":quote["source"],"quote":quote}),Err(error)=>json!({"name":name,"symbol":symbol,"region":exchange,"kind":"stock","error":error.to_string()})}});
    }
    let mut items = Vec::new();
    while let Some(Ok(item)) = tasks.join_next().await {
        items.push(item);
    }
    items.sort_by_key(|item| item["symbol"].as_str().unwrap_or("").to_string());
    json!({"items":items,"fetched_at":epoch_seconds()})
}

#[cfg(test)]
mod live_price_tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires public Yahoo price feeds"]
    async fn bundled_home_baskets_return_prices() {
        let mut symbols = Vec::new();
        for market in exchange_catalog().as_array().unwrap() {
            for item in market["symbols"].as_array().unwrap().iter().take(2) {
                symbols.push((
                    item["name"].as_str().unwrap().into(),
                    item["code"].as_str().unwrap().into(),
                    market["id"].as_str().unwrap().into(),
                ));
            }
        }
        let result = quote_symbols(symbols).await;
        let items = result["items"].as_array().unwrap();
        for row in items {
            println!(
                "{} {} price={} status={} error={}",
                row["region"], row["symbol"], row["price"], row["quote"]["session"], row["error"]
            );
        }
        assert!(items.iter().filter(|row| row["price"].is_number()).count() >= 10);
    }
}

/// Keep the world index board and the home stock basket separate.
fn include_market_row(exchange: &str, region: &str, kind: &str, global: bool) -> bool {
    exchange == "all" || exchange == region || (global && kind == "index")
}
#[cfg(test)]
mod global_index_tests {
    use super::*;
    #[test]
    fn global_board_keeps_all_indices_and_only_home_stocks() {
        for (_, _, region, kind) in market_defs() {
            assert_eq!(
                include_market_row("India", region, kind, true),
                kind == "index" || region == "India"
            );
            assert_eq!(
                include_market_row("India", region, kind, false),
                region == "India"
            );
        }
    }
}
