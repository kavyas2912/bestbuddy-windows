use butler_plugin::{ButlerPlugin, PluginRequest, PluginResponse};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Semaphore};
static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Value)>>> = OnceLock::new();
static KEY_LOCKS: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();
static LIMIT: OnceLock<Arc<Semaphore>> = OnceLock::new();
static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub async fn clear() {
    GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if let Some(cache) = CACHE.get() {
        cache.lock().await.clear();
    }
    if let Some(locations) = LOCATIONS.get() {
        locations.lock().await.clear();
    }
}
pub const CATEGORIES: &[&str] = &[
    "breaking",
    "world",
    "national",
    "regional",
    "local",
    "political",
    "business",
    "business-world",
    "business-national",
    "science",
    "ai-tech",
    "sports",
    "entertainment",
    "viral",
];
pub fn query(category: &str, location: &str, region: &str, country: &str) -> String {
    match category {
        "breaking" => format!("{country} latest top news"),
        "world" => "world news".into(),
        "national" => format!("{country} national news"),
        "regional" => format!("\"{region}\" \"{country}\""),
        "local" => format!("\"{location}\" \"{country}\""),
        "political" => format!("\"{country}\" (politics OR government OR election)"),
        "business" | "business-national" => {
            format!("\"{country}\" (business OR economy OR markets OR companies)")
        }
        "business-world" => "world business economy stock markets".into(),
        "science" => "science research discoveries".into(),
        "ai-tech" => "artificial intelligence technology".into(),
        "sports" => format!("{country} sports news"),
        "entertainment" => format!("{country} entertainment film"),
        _ => format!("{country} viral internet news"),
    }
}
fn geographic_match(item: &Value, place: &str) -> bool {
    let text = format!(
        "{} {}",
        item["title"].as_str().unwrap_or(""),
        item["summary"].as_str().unwrap_or("")
    )
    .to_lowercase();
    let place = place.trim().to_lowercase();
    let contains = |name: &str| {
        text.match_indices(name).any(|(i, _)| {
            let before = text[..i].chars().next_back();
            let after = text[i + name.len()..].chars().next();
            before.is_none_or(|c| !c.is_alphanumeric())
                && after.is_none_or(|c| !c.is_alphanumeric())
        })
    };
    if item["geo_scope"]
        .as_str()
        .is_some_and(|scope| scope.eq_ignore_ascii_case(&place))
    {
        return true;
    }
    !place.is_empty()
        && butler_plugin_news::curation::place_names(&place)
            .iter()
            .any(|name| contains(name))
}
pub async fn fetch(
    category: String,
    source: String,
    location: String,
    region: String,
    country: String,
    force: bool,
) -> Result<Value, String> {
    fetch_language(
        category,
        source,
        location,
        region,
        country,
        force,
        "en".into(),
    )
    .await
}
pub async fn fetch_language(
    category: String,
    source: String,
    location: String,
    region: String,
    country: String,
    force: bool,
    language: String,
) -> Result<Value, String> {
    if !CATEGORIES.contains(&category.as_str()) {
        return Err("Unknown news category".into());
    }
    if category == "local" && location.trim().is_empty() {
        return Err("Set your city in Profile for local headlines.".into());
    }

    let (location, region, country) = resolve_location(location, region, country).await;
    let region = butler_plugin_news::curation::coverage_region(&location, &region, &country);
    if category == "regional" && region.is_empty() {
        return Err("Your region could not be resolved; set it in Profile.".into());
    }
    let key = json!([category, source, location, region, country, language]).to_string();
    let key_lock = {
        let mut keys = KEY_LOCKS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .await;
        keys.entry(key.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    let _key_guard = key_lock.lock().await;
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if !force {
        if let Some((time, value)) = cache.lock().await.get(&key) {
            if time.elapsed() < Duration::from_secs(300) {
                return Ok(value.clone());
            }
        }
    }
    let candidates = butler_plugin_news::curation::candidates_for_language(
        &category, &location, &region, &country, &language,
    );
    let publishers = if matches!(source.as_str(), "market" | "all") {
        candidates
    } else if candidates.contains(&source) {
        vec![source.clone()]
    } else {
        return Err(
            "This source is not curated for the selected location and category. Choose Automatic."
                .into(),
        );
    };
    let mut catalog =
        butler_plugin_news::curation::catalog(&category, &location, &region, &country);
    catalog["sources"]=json!(butler_plugin_news::curation::candidates_for_language(&category,&location,&region,&country,&language).iter().map(|id|json!({"id":id,"name":butler_plugin_news::curation::registry()["publishers"][id]["name"],"status":"not_checked"})).collect::<Vec<_>>());
    let generation = GENERATION.load(std::sync::atomic::Ordering::SeqCst);
    let search = query(&category, &location, &region, &country);
    let mut tasks = tokio::task::JoinSet::new();
    for publisher in publishers {
        let semaphore = LIMIT.get_or_init(|| Arc::new(Semaphore::new(6))).clone();
        let search = search.clone();
        let category = category.clone();
        let country = country.clone();
        let location = location.clone();
        let region = region.clone();
        tasks.spawn(async move{
  let _permit=semaphore.acquire_owned().await.map_err(|e|e.to_string())?;
  let result=butler_plugin_news::NewsPlugin::new().execute(PluginRequest{capability:"news.headlines".into(),params:json!({"query":search,"source":publisher,"category":category,"country":country,"location":location,"region":region})}).await.map_err(|e|e.to_string());
  Ok::<_,String>((publisher,result))
 });
    }
    let mut items = Vec::new();
    let mut errors = Vec::new();
    let mut successful = 0;
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok((publisher, Ok(response)))) => {
                successful += 1;
                let accepted = response.data["items"]
                    .as_array()
                    .map(|rows| {
                        rows.iter()
                            .filter(|row| {
                                !matches!(category.as_str(), "local" | "regional")
                                    || geographic_match(
                                        row,
                                        if category == "local" {
                                            &location
                                        } else {
                                            &region
                                        },
                                    )
                            })
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let accepted_count = accepted.len();
                items.extend(accepted);
                if let Some(sources) = catalog["sources"].as_array_mut() {
                    for source in sources
                        .iter_mut()
                        .filter(|source| source["id"] == publisher)
                    {
                        source["status"] = json!(if accepted_count > 0 {
                            "available"
                        } else {
                            "empty"
                        });
                        source["items_returned"] = json!(accepted_count);
                        source["checked_at"] = json!(chrono::Utc::now().to_rfc3339());
                    }
                }
            }
            Ok(Ok((publisher, Err(error)))) => {
                if let Some(sources) = catalog["sources"].as_array_mut() {
                    for source in sources
                        .iter_mut()
                        .filter(|source| source["id"] == publisher)
                    {
                        source["status"] = json!("unavailable");
                        source["checked_at"] = json!(chrono::Utc::now().to_rfc3339());
                    }
                }
                errors.push(format!("{publisher}: {error}"));
            }
            other => errors.push(format!("Feed task: {other:?}")),
        }
    }
    if successful == 0 {
        return Err(format!("Sources unavailable: {}", errors.join("; ")));
    }
    items.sort_by_key(|item| {
        std::cmp::Reverse(
            item["published"]
                .as_str()
                .and_then(|date| chrono::DateTime::parse_from_rfc2822(date).ok())
                .map(|date| date.timestamp()),
        )
    });
    if matches!(category.as_str(), "local" | "regional") {
        let place = if category == "local" {
            &location
        } else {
            &region
        };
        items.retain(|item| geographic_match(item, place));
    }
    let mut seen = std::collections::HashSet::new();
    items.retain(|item| {
        item["url"]
            .as_str()
            .map(|url| seen.insert(url.to_string()))
            .unwrap_or(false)
    });
    let value = json!({"category":category,"items":items,"source_catalog":catalog,"errors":errors,"sources_available":successful,"fetched_at":chrono::Utc::now().to_rfc3339(),"scope":match category.as_str(){"world"|"business-world"=>"Worldwide".to_string(),"science"|"ai-tech"=>"Global research".to_string(),"local"=>format!("{location}, {region}, {country}"),"regional"=>format!("{region}, {country}"),_=>country.clone()},"freshness_policy":if matches!(category.as_str(),"science"|"ai-tech"){"Archive permitted; publication dates shown"}else{"Latest first; maximum three calendar months"}});
    if generation != GENERATION.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("News refresh cancelled after profile change".into());
    }
    let mut entries = cache.lock().await;
    entries.retain(|_, (time, _)| time.elapsed() < Duration::from_secs(1800));
    if entries.len() > 128 {
        entries.clear();
    }
    entries.insert(key, (Instant::now(), value.clone()));
    Ok(value)
}
pub fn as_response(value: Value) -> PluginResponse {
    PluginResponse {
        plugin_id: "news".into(),
        capability: "news.headlines".into(),
        text: "Your news sources have been refreshed.".into(),
        data: value,
    }
}

pub async fn topic(interest: String, country: String) -> Result<PluginResponse, String> {
    let semaphore = LIMIT.get_or_init(|| Arc::new(Semaphore::new(6))).clone();
    let _permit = semaphore.acquire_owned().await.map_err(|e| e.to_string())?;
    butler_plugin_news::NewsPlugin::new().execute(PluginRequest{capability:"news.headlines".into(),params:json!({"query":format!("{} news",interest),"country":country,"category":"interest"})}).await.map_err(|e|e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queries_preserve_region_and_distinguish_business_scope() {
        assert!(query("local", "Pune", "Maharashtra", "India").contains("Pune"));
        assert!(query("business-world", "", "", "India").starts_with("world"));
        assert!(query("business-national", "", "", "India").contains("India"));
    }
    #[test]
    fn location_and_publisher_routing_do_not_bleed_between_tabs() {
        assert!(geographic_match(
            &json!({"title":"Bangalore Metro opens"}),
            "Bengaluru"
        ));
        assert!(!geographic_match(
            &json!({"title":"Puneville market opens"}),
            "Pune"
        ));
        assert!(!geographic_match(
            &json!({"title":"London election"}),
            "Pune"
        ));
        let indian = butler_plugin_news::recommended_sources("business", "India", "Maharashtra");
        assert!(indian.contains(&"et"));
        assert!(!indian.contains(&"cnbc"));
        assert!(
            butler_plugin_news::recommended_sources("local", "India", "Maharashtra")
                .contains(&"loksatta")
        );
        assert!(!butler_plugin_news::recommended_sources("national", "USA", "").contains(&"hindu"));
    }
    #[tokio::test]
    #[ignore = "requires public live feeds"]
    async fn live_sources_and_cache() {
        let data = fetch(
            "world".into(),
            "all".into(),
            "".into(),
            "".into(),
            "India".into(),
            true,
        )
        .await
        .unwrap();
        let items = data["items"].as_array().unwrap();
        assert!(!items.is_empty());
        assert!(data["sources_available"].as_u64().unwrap() >= 2);
        assert!(items.iter().any(|item| item["summary"]
            .as_str()
            .map(|text| text.len() > 40)
            .unwrap_or(false)));
        let cached = fetch(
            "world".into(),
            "all".into(),
            "".into(),
            "".into(),
            "India".into(),
            false,
        )
        .await
        .unwrap();
        assert_eq!(cached["fetched_at"], data["fetched_at"]);
        println!(
            "World: {} items, {} available sources, cache reused",
            items.len(),
            data["sources_available"]
        );
        for source in ["yahoo", "moneycontrol"] {
            let data = fetch(
                "business-national".into(),
                source.into(),
                "".into(),
                "".into(),
                "India".into(),
                true,
            )
            .await
            .unwrap();
            assert!(!data["items"].as_array().unwrap().is_empty());
            println!(
                "{}: {} market stories",
                source,
                data["items"].as_array().unwrap().len()
            );
        }
    }
}

static LOCATIONS: OnceLock<Mutex<HashMap<String, (String, String, String)>>> = OnceLock::new();
pub async fn resolve_location(
    city: String,
    region: String,
    country: String,
) -> (String, String, String) {
    let (city, country) = butler_plugin_news::curation::explicit_location(&city, &country);
    let region = butler_plugin_news::curation::city_state(&city, &country).unwrap_or(region);
    let key = json!([city, region, country]).to_string();
    let cache = LOCATIONS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(location) = cache.lock().await.get(&key).cloned() {
        return location;
    }
    let generation = GENERATION.load(std::sync::atomic::Ordering::SeqCst);
    let mut result = (
        city.clone(),
        if region.trim().is_empty() {
            butler_plugin_news::curation::city_state(&city, &country).unwrap_or(region)
        } else {
            region
        },
        country.clone(),
    );
    if (result.1.trim().is_empty() || result.2.trim().is_empty()) && !city.is_empty() {
        if let Ok(client) = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
        {
            if let Ok(response)=client.get(format!("https://geocoding-api.open-meteo.com/v1/search?name={}&count=10&language=en&format=json",urlencoding::encode(&city))).send().await{
    if let Ok(data)=response.json::<Value>().await{if let Some(rows)=data["results"].as_array(){
      let key=butler_plugin_news::country_key(&country);let found=rows.iter().find(|row|country.is_empty()||butler_plugin_news::country_key(row["country"].as_str().unwrap_or(""))==key);
      if let Some(row)=found{result.1=row["admin1"].as_str().unwrap_or("").into();if result.2.is_empty(){result.2=row["country"].as_str().unwrap_or("").into();}}
    }}
   }
        }
    }
    let mut cache = cache.lock().await;
    if cache.len() > 128 {
        cache.clear();
    }
    if generation == GENERATION.load(std::sync::atomic::Ordering::SeqCst) {
        cache.insert(key, result.clone());
    }
    result
}
pub async fn catalog(category: &str, city: String, region: String, country: String) -> Value {
    let (city, region, country) = resolve_location(city, region, country).await;
    let region = butler_plugin_news::curation::coverage_region(&city, &region, &country);
    let key = json!([category, "all", city, region, country]).to_string();
    if let Some(cache) = CACHE.get() {
        if let Some((time, value)) = cache.lock().await.get(&key) {
            if time.elapsed() < Duration::from_secs(1800) {
                return value["source_catalog"].clone();
            }
        }
    }
    butler_plugin_news::curation::catalog(category, &city, &region, &country)
}

#[cfg(test)]
mod curated_live_tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires public local publisher feeds"]
    async fn pune_is_resolved_and_curated_before_widget_fetch() {
        let data = fetch(
            "local".into(),
            "all".into(),
            "Pune".into(),
            "".into(),
            "India".into(),
            true,
        )
        .await
        .unwrap();
        assert_eq!(data["source_catalog"]["region"], "Maharashtra");
        assert!(data["source_catalog"]["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["id"] == "loksatta"));
        let rows = data["items"].as_array().unwrap();
        assert!(!rows.is_empty());
        assert!(rows.iter().all(|item| geographic_match(item, "Pune")));
        println!(
            "Pune: {} local stories; curated region {}",
            rows.len(),
            data["source_catalog"]["region"]
        );
    }
}
