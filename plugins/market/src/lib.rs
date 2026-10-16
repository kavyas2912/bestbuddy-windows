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

fn market_defs() -> [(&'static str, &'static str, &'static str, &'static str); 8] {
    [
        ("NIFTY 50", "^nsei", "India", "index"),
        ("SENSEX", "^bsesn", "India", "index"),
        ("S&P 500", "^spx", "USA", "index"),
        ("NASDAQ", "^ndq", "USA", "index"),
        ("DOW JONES", "^dji", "USA", "index"),
        ("FTSE 100", "^ftse", "UK", "index"),
        ("DAX", "^dax", "Germany", "index"),
        ("NIKKEI 225", "^nky", "Japan", "index"),
    ]
}

fn normalize_market_symbol(
    s: &str,
) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
    let q = s.trim().to_ascii_lowercase();
    market_defs()
        .into_iter()
        .find(|(name, code, _region, _kind)| {
            q == name.to_ascii_lowercase()
                || q == *code
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
    match symbol {
        "^nsei" => Some("^NSEI"),
        "^bsesn" => Some("^BSESN"),
        "^spx" => Some("^GSPC"),
        "^ndq" => Some("^IXIC"),
        "^dji" => Some("^DJI"),
        "^ftse" => Some("^FTSE"),
        "^dax" => Some("^GDAXI"),
        "^nky" => Some("^N225"),
        _ => None,
    }
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
        .get("chartPreviousClose")
        .or_else(|| meta.get("previousClose"))
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0);
    let change = previous.map(|value| price - value);
    let change_percent = previous.map(|value| (price - value) / value * 100.0);
    Ok(
        json!({"close":price,"previous_close":previous,"change":change,"change_percent":change_percent,"currency":meta.get("currency"),"timestamp":meta.get("regularMarketTime"),"source":"Yahoo Finance","symbol":meta.get("symbol")}),
    )
}
async fn fetch_yahoo_quote(symbol: &str) -> Result<Value, PluginError> {
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=1d&range=5d",
        urlencoding::encode(symbol)
    );
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| PluginError::Failed(e.to_string()))?
        .get(url)
        .header("User-Agent", "BestBuddy/0.6.12")
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
async fn fetch_stooq_quote(symbol: &str) -> Result<Value, PluginError> {
    let clean = symbol.trim().to_ascii_lowercase();
    let url = if clean.starts_with('^') {
        format!(
            "https://stooq.com/q/l/?s={}&f=sd2t2ohlcv&h&e=json",
            urlencoding::encode(&clean)
        )
    } else {
        format!(
            "https://stooq.com/q/l/?s={}.us&f=sd2t2ohlcv&h&e=json",
            urlencoding::encode(&clean)
        )
    };
    let value = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build()
        .expect("HTTP client")
        .get(url)
        .send()
        .await
        .map_err(|e| PluginError::Failed(e.to_string()))?
        .json::<Value>()
        .await
        .map_err(|e| PluginError::Failed(e.to_string()))?;
    let mut quote = parse_quote_response(&value, symbol)?;
    if let Some(object) = quote.as_object_mut() {
        object.insert("source".into(), json!("Stooq"));
    }
    Ok(quote)
}
async fn fetch_quote(symbol: &str) -> Result<Value, PluginError> {
    if let Some(yahoo) = yahoo_symbol(symbol) {
        match fetch_yahoo_quote(yahoo).await {
            Ok(quote) => return Ok(quote),
            Err(yahoo_error) => {
                return fetch_stooq_quote(symbol).await.map_err(|stooq_error| {
                    PluginError::Failed(format!("Yahoo: {yahoo_error}; Stooq: {stooq_error}"))
                })
            }
        }
    }
    match fetch_yahoo_quote(symbol).await {
        Ok(quote) => Ok(quote),
        Err(_) => fetch_stooq_quote(symbol).await,
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
                .header("User-Agent", "BestBuddy/0.6.12")
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
async fn fetch_yahoo_chart(symbol: &str) -> Result<Value, PluginError> {
    if symbol.len() > 20
        || !symbol
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "^.-_".contains(c))
    {
        return Err(PluginError::Failed("Invalid market symbol".into()));
    }
    let url = format!(
        "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval=5m&range=1d",
        urlencoding::encode(symbol)
    );
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| PluginError::Failed(e.to_string()))?
        .get(url)
        .header("User-Agent", "BestBuddy/0.6.12")
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
    let quote = parse_yahoo_quote(&data)?;
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
    let (components, components_complete, components_note, components_url) =
        index_components(symbol).await;
    Ok(
        json!({"quote":quote,"points":points,"components":components,"components_complete":components_complete,"components_note":components_note,"components_url":components_url}),
    )
}

#[async_trait]
impl BuddyPlugin for MarketPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: "markets".into(),
            name: "Markets".into(),
            version: "0.6.12".into(),
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
            let data = fetch_yahoo_chart(symbol).await?;
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
            for (name, code, region, kind) in market_defs() {
                tasks.spawn(async move {
                    let result = fetch_quote(code).await;
                    let error = result.as_ref().err().map(|e| e.to_string());
                    let q = result.unwrap_or(Value::Null);
                    let price = quote_price(&q);
                    json!({"name":name,"symbol":code,"region":region,"kind":kind,"price":price,"close":price,"change":q.get("change"),"change_percent":q.get("change_percent"),"currency":q.get("currency"),"source":q.get("source"),"timestamp":q.get("timestamp"),"quote":q,"error":error})
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
        let last = q.get("close").and_then(|v| v.as_str()).unwrap_or("N/A");
        let text = if last == "N/A" {
            format!("I couldn't get a live quote for {} right now.", label)
        } else {
            format!("{} is at {} in the latest available quote.", label, last)
        };
        Ok(PluginResponse {
            plugin_id: "markets".into(),
            capability: r.capability,
            text,
            data: json!({"name":label,"symbol":symbol,"region":region,"kind":kind,"close":last,"quote":q}),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use buddy_plugin::BuddyPlugin;
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
