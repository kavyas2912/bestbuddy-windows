pub mod article_policy;
pub mod curation;
pub fn country_key(country: &str) -> String {
    let key = country.trim().to_ascii_lowercase();
    match key.as_str() {
        "in" => "india",
        "us" | "usa" | "united states of america" => "united states",
        "uk" | "gb" => "united kingdom",
        "au" => "australia",
        "ca" => "canada",
        "de" => "germany",
        "jp" => "japan",
        _ => &key,
    }
    .to_string()
}
pub fn recommended_sources(category: &str, country: &str, region: &str) -> Vec<&'static str> {
    let key = country_key(country);
    let home: Vec<&str> = match key.as_str() {
        "india" => vec![
            "hindu",
            "express",
            "ndtv",
            "toi",
            "hindustantimes",
            "indiatoday",
        ],
        "united states" => vec!["ap", "reuters", "npr", "cnn", "abc"],
        "united kingdom" => vec!["bbc", "guardian", "sky", "itv", "independent"],
        "australia" => vec!["abcau", "sbs", "guardian", "smh", "nine"],
        "canada" => vec!["cbc", "ctv", "globalnews", "globe", "reuters"],
        "germany" => vec!["dw", "tagesschau", "spiegel", "zeit", "reuters"],
        "japan" => vec!["nhk", "japantimes", "mainichi", "asahi", "reuters"],
        _ => vec!["bbc", "reuters", "ap", "guardian", "aljazeera"],
    };
    let mut ids = match category {
        "world" => vec!["bbc", "reuters", "ap", "guardian", "aljazeera"],
        "science" => vec!["bbc", "nasa", "nature", "reuters", "guardian"],
        "ai-tech" => vec!["bbc", "verge", "nature", "reuters", "guardian"],
        "business-world" => vec!["bbc", "reuters", "cnbc", "yahoo", "guardian"],
        "business" | "business-national" if key == "india" => vec![
            "et",
            "mint",
            "businessstandard",
            "moneycontrol",
            "yahoo",
            "cnbctv18",
            "etnow",
            "hindu",
            "express",
            "bbc",
        ],
        "business" | "business-national" => {
            let mut v = vec!["bbc", "reuters", "yahoo"];
            if key == "united states" {
                v.push("cnbc");
            }
            v.extend(home);
            v
        }
        _ => {
            let mut v = vec!["bbc"];
            if matches!(category, "local" | "regional") && key == "india" {
                v.extend(match region.trim().to_ascii_lowercase().as_str() {
                    "maharashtra" => vec!["loksatta", "lokmat", "sakal"],
                    "west bengal" => vec!["telegraph", "anandabazar"],
                    "tamil nadu" => vec!["hindu", "newindianexpress", "thanthi"],
                    "kerala" => vec!["manorama", "mathrubhumi"],
                    "karnataka" => vec!["deccanherald", "prajavani"],
                    "telangana" => vec!["deccanchronicle", "eenadu"],
                    "andhra pradesh" => vec!["eenadu", "sakshi"],
                    "gujarat" => vec!["gujaratsamachar", "divyabhaskar"],
                    _ => vec![],
                });
            }
            v.extend(home);
            v
        }
    };
    if category == "sports" {
        ids.push("espn");
    }
    if category == "entertainment" {
        ids.push("variety");
    }
    ids.extend(["google", "msn"]);
    let mut seen = std::collections::HashSet::new();
    ids.retain(|id| seen.insert(*id));
    ids
}
use async_trait::async_trait;
use buddy_plugin::{
    BuddyPlugin, Permission, PluginError, PluginManifest, PluginRequest, PluginResponse,
};
use quick_xml::{events::Event, Reader};
use serde::Deserialize;

#[derive(Debug, Deserialize, Default, Clone)]
struct Item {
    #[serde(default)]
    title: String,
    #[serde(default)]
    link: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "pubDate", default)]
    pub_date: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(skip)]
    source_url: Option<String>,
    #[serde(skip)]
    thumbnail: Option<String>,
    #[serde(skip)]
    video_url: Option<String>,
}

pub struct NewsPlugin;
impl NewsPlugin {
    pub fn new() -> Self {
        Self
    }
}

fn parse_feed(xml: &str) -> Result<Vec<Item>, PluginError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut items = Vec::<Item>::new();
    let mut current = Item::default();
    let mut in_item = false;
    let mut field = String::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "item" {
                    in_item = true;
                    current = Item::default();
                    field.clear();
                } else if in_item {
                    field = name.clone();
                    if name == "source" {
                        current.source_url = e
                            .attributes()
                            .flatten()
                            .find(|a| a.key.as_ref() == b"url")
                            .map(|a| String::from_utf8_lossy(&a.value).into_owned());
                    }
                }
                if in_item
                    && (name == "media:content" || name == "media:thumbnail" || name == "enclosure")
                {
                    let mut media_type = None;
                    let mut media_url = None;
                    for a in e.attributes().flatten() {
                        if a.key.as_ref() == b"url" {
                            media_url = Some(String::from_utf8_lossy(&a.value).to_string());
                        } else if a.key.as_ref() == b"type" {
                            media_type = Some(String::from_utf8_lossy(&a.value).to_string());
                        }
                    }
                    if let Some(url) = media_url {
                        if media_type.as_deref().unwrap_or("").starts_with("video/") {
                            current.video_url = Some(url);
                        } else if current.thumbnail.is_none() {
                            current.thumbnail = Some(url);
                        }
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                if in_item {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name == "media:content" || name == "media:thumbnail" || name == "enclosure" {
                        let mut media_url = None;
                        let mut media_type = None;
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"url" {
                                media_url = Some(String::from_utf8_lossy(&a.value).to_string());
                            } else if a.key.as_ref() == b"type" {
                                media_type = Some(String::from_utf8_lossy(&a.value).to_string());
                            }
                        }
                        if let Some(u) = media_url {
                            if media_type.as_deref().unwrap_or("").starts_with("video/") {
                                current.video_url = Some(u);
                            } else if current.thumbnail.is_none() {
                                current.thumbnail = Some(u);
                            }
                        }
                    }
                }
            }
            Ok(Event::CData(t)) => {
                let v = String::from_utf8_lossy(t.as_ref()).into_owned();
                if in_item {
                    match field.as_str() {
                        "title" => current.title = v,
                        "description" => current.description = Some(v),
                        "link" => current.link = v,
                        _ => {}
                    }
                }
            }
            Ok(Event::Text(t)) => {
                if in_item && !field.is_empty() {
                    let v = t
                        .unescape()
                        .map(|x| x.into_owned())
                        .unwrap_or_else(|_| String::from_utf8_lossy(t.as_ref()).to_string());
                    match field.as_str() {
                        "title" => current.title = v,
                        "link" => current.link = v,
                        "description" => current.description = Some(v),
                        "pubDate" => current.pub_date = Some(v),
                        "source" => current.source = Some(v),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "item" {
                    if !current.title.is_empty() {
                        items.push(current.clone());
                    }
                    in_item = false;
                    field.clear();
                } else if in_item {
                    field.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(PluginError::Failed(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    items.retain(|item| {
        article_policy::is_article_headline(
            &item.title,
            &item.link,
            item.source.as_deref().unwrap_or(""),
        )
    });
    Ok(items)
}
#[cfg(test)]
fn retain_fresh_items(items: &mut Vec<Item>, now: chrono::DateTime<chrono::Utc>, hours: i64) {
    items.retain(|item| {
        item.pub_date
            .as_deref()
            .and_then(|date| chrono::DateTime::parse_from_rfc2822(date).ok())
            .map(|date| {
                let age = now.signed_duration_since(date);
                age.num_seconds() >= -300 && age < chrono::Duration::hours(hours)
            })
            .unwrap_or(false)
    });
}
fn within_news_policy(
    date: Option<&str>,
    category: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    let Some(published) = date.and_then(|date| chrono::DateTime::parse_from_rfc2822(date).ok())
    else {
        return false;
    };
    if published > now + chrono::Duration::minutes(5) {
        return false;
    }
    matches!(category, "science" | "ai-tech")
        || published >= now.checked_sub_months(chrono::Months::new(3)).unwrap()
}
fn clean_html(s: &str) -> String {
    let document = dom_query::Document::fragment(s);
    document.select("script,style").remove();
    document
        .text()
        .split_whitespace()
        .take(100)
        .collect::<Vec<_>>()
        .join(" ")
}
fn extract_img(s: &str) -> Option<String> {
    let lower = s.to_ascii_lowercase();
    let pos = lower.find("<img")?;
    let rest = &s[pos..];
    let low = rest.to_ascii_lowercase();
    let src = low.find("src=")?;
    let after = &rest[src + 4..];
    let quote = after.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let end = after[1..].find(quote)? + 1;
    Some(after[1..end].to_string())
}
fn source_from(title: &str) -> String {
    title
        .rsplit_once(" - ")
        .map(|(_, s)| s.trim().to_string())
        .unwrap_or_else(|| "News".into())
}
fn safe_media_url(value: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| parsed.to_string())
}
fn parse_video_feed(xml: &str, source: &str) -> Result<Vec<serde_json::Value>, PluginError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut videos = Vec::new();
    let (mut active, mut field) = (false, String::new());
    let (mut id, mut title, mut published, mut thumb) =
        (String::new(), String::new(), String::new(), String::new());
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let name = String::from_utf8_lossy(event.name().as_ref()).to_string();
                if name == "entry" {
                    active = true;
                    id.clear();
                    title.clear();
                    published.clear();
                    thumb.clear();
                } else if active {
                    field = name;
                }
            }
            Ok(Event::Empty(event)) if active && event.name().as_ref() == b"media:thumbnail" => {
                for attr in event.attributes().flatten() {
                    if attr.key.as_ref() == b"url" {
                        thumb = String::from_utf8_lossy(&attr.value).to_string();
                    }
                }
            }
            Ok(Event::Text(event)) if active => {
                let value = event
                    .unescape()
                    .map(|text| text.into_owned())
                    .unwrap_or_default();
                match field.as_str() {
                    "yt:videoId" => id = value,
                    "title" => title = value,
                    "published" => published = value,
                    _ => {}
                }
            }
            Ok(Event::End(event)) if event.name().as_ref() == b"entry" => {
                if id.len() == 11
                    && id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                    && !title.is_empty()
                {
                    videos.push(serde_json::json!({"title":title,"url":format!("https://www.youtube.com/watch?v={id}"),"video_url":format!("https://www.youtube.com/watch?v={id}"),"thumbnail":safe_media_url(&thumb),"published":published,"source":source}));
                }
                active = false;
                field.clear();
            }
            Ok(Event::End(_)) => field.clear(),
            Ok(Event::Eof) => break,
            Err(error) => return Err(PluginError::Failed(error.to_string())),
            _ => {}
        }
        buf.clear();
    }
    Ok(videos)
}

fn pbs_player_url(html: &str) -> Option<String> {
    let prefix = "https://player.pbs.org/widget/partnerplayer/";
    let rest = html.split_once(prefix)?.1;
    let id: String = rest.chars().take_while(|ch| ch.is_ascii_digit()).collect();
    (!id.is_empty()).then(|| format!("{prefix}{id}/?autoplay=false"))
}

async fn pbs_recent_videos(
    client: &reqwest::Client,
) -> Result<Vec<serde_json::Value>, PluginError> {
    let xml = client
        .get("https://www.pbs.org/newshour/feeds/rss/headlines")
        .send()
        .await
        .map_err(|e| PluginError::Failed(format!("PBS News: {e}")))?
        .error_for_status()
        .map_err(|e| PluginError::Failed(format!("PBS News: {e}")))?
        .text()
        .await
        .map_err(|e| PluginError::Failed(format!("PBS News: {e}")))?;
    let mut tasks = tokio::task::JoinSet::new();
    for item in parse_feed(&xml)?
        .into_iter()
        .filter(|item| item.link.starts_with("https://www.pbs.org/newshour/show/"))
        .take(12)
    {
        let client = client.clone();
        tasks.spawn(async move {
            let html = client
                .get(&item.link)
                .send()
                .await
                .ok()?
                .error_for_status()
                .ok()?
                .text()
                .await
                .ok()?;
            let video_url = pbs_player_url(&html)?;
            Some(serde_json::json!({
                "title": item.title,
                "url": item.link,
                "video_url": video_url,
                "thumbnail": item.thumbnail.and_then(|url| safe_media_url(&url)),
                "published": item.pub_date,
                "source": "PBS News"
            }))
        });
    }
    let mut videos = Vec::new();
    while let Some(result) = tasks.join_next().await {
        if let Ok(Some(item)) = result {
            videos.push(item);
        }
    }
    Ok(videos)
}

fn video_title_matches(category: &str, title: &str) -> bool {
    let title = title.to_lowercase();
    let terms: &[&str] = match category {
        "political" => &[
            "politic",
            "election",
            "government",
            "minister",
            "parliament",
            "policy",
        ],
        "business" => &[
            "business", "market", "economy", "company", "stock", "trade", "finance",
        ],
        "ai-tech" => &[
            "artificial intelligence",
            " ai ",
            "technology",
            "robot",
            "software",
            "chip",
            "computer",
        ],
        "sports" => &[
            "sport",
            "cricket",
            "football",
            "tennis",
            "match",
            "league",
            "tournament",
        ],
        "entertainment" => &[
            "film",
            "movie",
            "music",
            "actor",
            "celebrity",
            "television",
            "cinema",
        ],
        "viral" => &[
            "viral",
            "social media",
            "internet",
            "trend",
            "video",
            "online",
        ],
        "astronomy" => &[
            "astronaut",
            "telescope",
            "planet",
            "moon",
            "star",
            "galaxy",
            "universe",
            "mars",
            "solar",
            "space",
        ],
        "aerospace" => &[
            "astronaut",
            "spacecraft",
            "rocket",
            "launch",
            "flight",
            "aircraft",
            "station",
            "mission",
            "nasa",
            "mars",
        ],
        "science" => &[
            "science",
            "research",
            "discover",
            "experiment",
            "planet",
            "moon",
            "space",
            "climate",
            "earth",
            "telescope",
        ],
        "inventions" => &[
            "invent",
            "prototype",
            "robot",
            "breakthrough",
            "new technology",
            "first look",
            "innovation",
            "future",
        ],
        "products" => &[
            "review",
            "hands-on",
            "first look",
            "phone",
            "laptop",
            "mac",
            "tablet",
            "camera",
            "headphone",
            "gadget",
            "product",
        ],
        _ => return true,
    };
    terms.iter().any(|term| title.contains(term))
}

#[async_trait]
impl BuddyPlugin for NewsPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            id: "news".into(),
            name: "News".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            icon: "news".into(),
            capabilities: vec![
                "news.headlines".into(),
                "news.search".into(),
                "news.videos".into(),
            ],
            permissions: vec![Permission::Network],
        }
    }
    async fn execute(&self, r: PluginRequest) -> Result<PluginResponse, PluginError> {
        if r.capability == "news.videos" {
            let category = r
                .params
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("world");
            // YouTube's legacy channel-feed endpoint currently returns 404/500 even
            // for verified publisher channel IDs. Keep it out of the production path
            // until it is reliable again; PBS pages are checked for an actual playable
            // publisher iframe before a tile is returned.
            let sources: &[(&str, &str)] = &[];
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .map_err(|e| PluginError::Failed(e.to_string()))?;
            let mut videos = Vec::new();
            let mut errors = Vec::new();
            match pbs_recent_videos(&client).await {
                Ok(items) => videos.extend(items),
                Err(error) => errors.push(error.to_string()),
            }
            for (source, url) in sources {
                match client.get(*url).send().await {
                    Ok(response) if response.status().is_success() => match response.text().await {
                        Ok(xml) => match parse_video_feed(&xml, source) {
                            Ok(items) => videos.extend(items),
                            Err(error) => errors.push(format!("{source}: {error}")),
                        },
                        Err(error) => errors.push(format!("{source}: {error}")),
                    },
                    Ok(response) => errors.push(format!("{source}: HTTP {}", response.status())),
                    Err(error) => errors.push(format!("{source}: {error}")),
                }
            }
            if !matches!(
                category,
                "breaking" | "world" | "national" | "regional" | "local"
            ) {
                videos.retain(|video| {
                    video_title_matches(category, video["title"].as_str().unwrap_or(""))
                });
            }
            videos.sort_by(|a, b| b["published"].as_str().cmp(&a["published"].as_str()));
            videos.dedup_by(|a, b| a["video_url"] == b["video_url"]);
            videos.truncate(8);
            if videos.is_empty() {
                let reason = if errors.is_empty() {
                    format!("No recent {category} videos matched the category")
                } else {
                    errors.join("; ")
                };
                return Err(PluginError::Failed(reason));
            }
            return Ok(PluginResponse {
                plugin_id: "news".into(),
                capability: r.capability,
                text: format!("{} recent videos are available.", videos.len()),
                data: serde_json::json!({"category":category,"items":videos,"errors":errors}),
            });
        }
        let q = r
            .params
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("latest news");
        let publisher = r
            .params
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("all");
        let registry = curation::registry();
        let domain = if matches!(publisher, "all" | "google") {
            None
        } else {
            Some(
                registry["publishers"][publisher]["domain"]
                    .as_str()
                    .ok_or_else(|| PluginError::Failed("Unknown curated source".into()))?,
            )
        };
        let category = r
            .params
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("general");
        let archive_allowed = matches!(category, "science" | "ai-tech");

        let native_language = registry["publishers"][publisher]["language"]
            .as_str()
            .unwrap_or("en");
        let native_scope = if native_language != "en" && matches!(category, "local" | "regional") {
            let place = r
                .params
                .get(if category == "local" {
                    "location"
                } else {
                    "region"
                })
                .and_then(|value| value.as_str())
                .unwrap_or("");
            Some(format!(
                "({})",
                curation::place_names(place)
                    .iter()
                    .map(|name| format!("\"{name}\""))
                    .collect::<Vec<_>>()
                    .join(" OR ")
            ))
        } else {
            None
        };
        let scoped = format!(
            "{} {} {}",
            native_scope.as_deref().unwrap_or(q),
            domain
                .map(|d| if publisher == "bbc" {
                    "(site:bbc.com OR site:bbc.co.uk)".into()
                } else {
                    format!("(site:{d})")
                })
                .unwrap_or_default(),
            if archive_allowed || q.contains("when:") {
                ""
            } else {
                "when:7d"
            }
        );
        let country = country_key(
            r.params
                .get("country")
                .and_then(|v| v.as_str())
                .unwrap_or("India"),
        );
        let region = match country.as_str() {
            "india" => "IN",
            "united states" => "US",
            "united kingdom" => "GB",
            "germany" => "DE",
            "japan" => "JP",
            "australia" => "AU",
            "canada" => "CA",
            "france" => "FR",
            "singapore" => "SG",
            "united arab emirates" | "uae" => "AE",
            "pakistan" => "PK",
            "bangladesh" => "BD",
            "south africa" => "ZA",
            "brazil" => "BR",
            _ => "US",
        };
        let mut url = format!(
            "https://news.google.com/rss/search?q={}&hl={native_language}&gl={region}&ceid={region}:{native_language}",
            urlencoding::encode(&scoped)
        );
        let city = r
            .params
            .get("location")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        let city_feed = if publisher == "express" && category == "local" && region == "IN" {
            let slug = match city.as_str() {
                "pune" => Some("pune"),
                "mumbai" => Some("mumbai"),
                "delhi" | "new delhi" => Some("delhi"),
                "bengaluru" | "bangalore" => Some("bangalore"),
                "chennai" => Some("chennai"),
                "kolkata" => Some("kolkata"),
                "hyderabad" => Some("hyderabad"),
                "ahmedabad" => Some("ahmedabad"),
                "lucknow" => Some("lucknow"),
                "chandigarh" => Some("chandigarh"),
                "jaipur" => Some("jaipur"),
                _ => None,
            };
            slug.map(|slug| format!("https://indianexpress.com/section/cities/{slug}/feed/"))
        } else {
            None
        };
        let direct = match (publisher, category) {
            ("bbc", "national" | "breaking") if region == "GB" => {
                Some("https://feeds.bbci.co.uk/news/uk/rss.xml")
            }
            ("bbc", "national" | "breaking") if region == "IN" => {
                Some("https://feeds.bbci.co.uk/news/world/asia/india/rss.xml")
            }
            ("bbc", "world") => Some("https://feeds.bbci.co.uk/news/world/rss.xml"),
            ("bbc", "business-world") => Some("https://feeds.bbci.co.uk/news/business/rss.xml"),
            ("bbc", "science") => {
                Some("https://feeds.bbci.co.uk/news/science_and_environment/rss.xml")
            }
            ("bbc", "ai-tech") => Some("https://feeds.bbci.co.uk/news/technology/rss.xml"),
            ("express", "business" | "business-national") if region == "IN" => {
                Some("https://indianexpress.com/section/business/feed/")
            }
            ("guardian", "world") => Some("https://www.theguardian.com/world/rss"),
            ("guardian", "business-world") => Some("https://www.theguardian.com/business/rss"),
            ("guardian", "science") => Some("https://www.theguardian.com/science/rss"),
            ("guardian", "ai-tech") => Some("https://www.theguardian.com/technology/rss"),
            ("cnbc", "business-world") => {
                Some("https://www.cnbc.com/id/100003114/device/rss/rss.html")
            }
            ("aljazeera", "world") => Some("https://www.aljazeera.com/xml/rss/all.xml"),
            _ => None,
        };
        if let Some(feed) = &city_feed {
            url = feed.clone();
        } else if let Some(feed) = direct {
            url = feed.into();
        }
        let xml = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .build()
            .expect("HTTP client")
            .get(url)
            .header(
                reqwest::header::CACHE_CONTROL,
                "no-cache, no-store, max-age=0",
            )
            .header(reqwest::header::PRAGMA, "no-cache")
            .send()
            .await
            .map_err(|e| PluginError::Failed(e.to_string()))?
            .error_for_status()
            .map_err(|e| PluginError::Failed(e.to_string()))?
            .text()
            .await
            .map_err(|e| PluginError::Failed(e.to_string()))?;
        let mut items = parse_feed(&xml)?;
        items.retain(|item| {
            within_news_policy(item.pub_date.as_deref(), category, chrono::Utc::now())
        });
        if direct.is_none() && city_feed.is_none() {
            if let Some(domain) = domain {
                items.retain(|item| {
                    item.source_url
                        .as_deref()
                        .and_then(|url| reqwest::Url::parse(url).ok())
                        .and_then(|url| url.host_str().map(str::to_lowercase))
                        .is_some_and(|host| {
                            host == domain
                                || host.ends_with(&format!(".{domain}"))
                                || publisher == "bbc"
                                    && (host == "bbc.co.uk" || host.ends_with(".bbc.co.uk"))
                        })
                });
            }
        }
        if direct.is_some() || city_feed.is_some() {
            for item in &mut items {
                item.source = Some(
                    match publisher {
                        "bbc" => "BBC News",
                        "express" => "Indian Express",
                        "guardian" => "The Guardian",
                        "cnbc" => "CNBC",
                        "aljazeera" => "Al Jazeera",
                        _ => publisher,
                    }
                    .into(),
                );
            }
        }
        items.sort_by_key(|item| {
            std::cmp::Reverse(
                item.pub_date
                    .as_deref()
                    .and_then(|date| chrono::DateTime::parse_from_rfc2822(date).ok())
                    .map(|date| date.timestamp()),
            )
        });
        let items=items.into_iter().filter_map(|i|{let article=safe_media_url(&i.link)?;let source=i.source.clone().unwrap_or_else(||source_from(&i.title));let summary=i.description.clone().map(|s|clean_html(&s));let thumb=i.thumbnail.clone().or_else(||i.description.as_deref().and_then(extract_img)).and_then(|u|safe_media_url(&u));let video=i.video_url.clone().or_else(||i.link.contains("youtube.com").then(||i.link.clone())).and_then(|u|safe_media_url(&u));Some(serde_json::json!({"title":i.title,"url":article,"summary":summary,"published":i.pub_date,"source":source,"thumbnail":thumb,"video_url":video,"geo_scope":if city_feed.is_some(){Some(city.as_str())}else{None}}))}).collect::<Vec<_>>();
        let text = if let Some(first) = items.first() {
            format!(
                "I've got the latest headlines, and the lead story is {}.",
                first["title"].as_str().unwrap_or("the latest story")
            )
        } else {
            "I couldn't find any current headlines right now.".into()
        };
        Ok(PluginResponse {
            plugin_id: "news".into(),
            capability: r.capability,
            text,
            data: serde_json::json!({"category":q,"items":items}),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use buddy_plugin::BuddyPlugin;
    #[test]
    fn three_calendar_months_and_science_exception() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-17T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert!(within_news_policy(
            Some("Fri, 17 Jul 2026 12:00:00 +0000"),
            "world",
            now
        ));
        assert!(!within_news_policy(
            Some("Thu, 16 Jul 2026 12:00:00 +0000"),
            "world",
            now
        ));
        assert!(within_news_policy(
            Some("Thu, 16 Jul 2026 12:00:00 +0000"),
            "science",
            now
        ));
        assert!(within_news_policy(
            Some("Thu, 16 Jul 2026 12:00:00 +0000"),
            "ai-tech",
            now
        ));
        assert!(!within_news_policy(None, "science", now));
        assert!(!within_news_policy(
            Some("Sun, 18 Oct 2026 12:00:00 +0000"),
            "ai-tech",
            now
        ));
    }
    #[test]
    fn freshness_excludes_stale_unknown_and_future_dates() {
        let xml = "<rss><channel><item><title>Recent</title><link>https://example.test/news/recent-report</link><pubDate>Sat, 17 Oct 2026 10:00:00 +0000</pubDate></item><item><title>Old</title><link>https://example.test/news/old-report</link><pubDate>Tue, 13 Oct 2026 10:00:00 +0000</pubDate></item><item><title>Unknown</title><link>https://example.test/news/unknown-report</link></item><item><title>Future</title><link>https://example.test/news/future-report</link><pubDate>Sun, 18 Oct 2026 10:00:00 +0000</pubDate></item></channel></rss>";
        let mut items = parse_feed(xml).unwrap();
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-17T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        retain_fresh_items(&mut items, now, 48);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Recent");
    }

    #[test]
    fn pbs_video_page_player_is_embeddable() {
        let html = r#"<iframe src="https://player.pbs.org/widget/partnerplayer/3113997181/?autoplay=false"></iframe>"#;
        assert_eq!(
            pbs_player_url(html).as_deref(),
            Some("https://player.pbs.org/widget/partnerplayer/3113997181/?autoplay=false")
        );
        assert!(pbs_player_url("no PBS player here").is_none());
    }
    #[tokio::test]
    #[ignore = "requires live PBS News"]
    async fn pbs_feed_returns_playable_videos() {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap();
        let videos = pbs_recent_videos(&client).await.unwrap();
        assert!(!videos.is_empty(), "PBS feed returned no playable videos");
        assert!(videos.iter().all(|item| item["video_url"]
            .as_str()
            .is_some_and(|url| url.starts_with("https://player.pbs.org/widget/partnerplayer/"))));
    }
    #[test]
    fn media_and_cdata_are_preserved() {
        let xml = r#"<rss><channel><item><title><![CDATA[A & B]]></title><link>https://example.org/story</link><description><![CDATA[A summary]]></description><media:thumbnail url="https://example.org/a.jpg"/><enclosure type="video/mp4" url="https://example.org/a.mp4"/></item></channel></rss>"#;
        let items = parse_feed(xml).unwrap();
        assert_eq!(items[0].title, "A & B");
        assert_eq!(
            items[0].thumbnail.as_deref(),
            Some("https://example.org/a.jpg")
        );
        assert_eq!(
            items[0].video_url.as_deref(),
            Some("https://example.org/a.mp4")
        );
    }
    #[test]
    fn start_tag_video_and_media_urls_are_handled_safely() {
        let xml = r#"<rss><channel><item><title>Video</title><link>https://example.org/story</link><media:content type="video/mp4" url="https://example.org/clip.mp4"></media:content></item></channel></rss>"#;
        let items = parse_feed(xml).unwrap();
        assert_eq!(
            items[0].video_url.as_deref(),
            Some("https://example.org/clip.mp4")
        );
        assert!(items[0].thumbnail.is_none());
        assert!(safe_media_url("javascript:alert(1)").is_none());
        assert!(safe_media_url("file:///secret").is_none());
    }
    #[test]
    fn youtube_feed_parser_produces_links_and_thumbnails() {
        let xml = r#"<feed><entry><yt:videoId>41vjLT_fM4A</yt:videoId><title>World update</title><published>2026-09-22T23:02:04Z</published><media:group><media:thumbnail url="https://i1.ytimg.com/vi/41vjLT_fM4A/hqdefault.jpg"/></media:group></entry></feed>"#;
        let videos = parse_video_feed(xml, "BBC News").unwrap();
        assert_eq!(videos.len(), 1);
        assert_eq!(
            videos[0]["video_url"],
            "https://www.youtube.com/watch?v=41vjLT_fM4A"
        );
        assert!(videos[0]["thumbnail"]
            .as_str()
            .unwrap()
            .starts_with("https://"));
    }
    #[test]
    fn video_categories_do_not_show_unrelated_titles() {
        assert!(video_title_matches(
            "astronomy",
            "New telescope reveals a galaxy"
        ));
        assert!(!video_title_matches("astronomy", "A laptop review"));
        assert!(video_title_matches("products", "First look at a new phone"));
        assert!(!video_title_matches("inventions", "The Vergecast episode"));
    }
    #[test]
    fn manifest_exposes_news_capabilities() {
        let m = NewsPlugin::new().manifest();
        assert_eq!(m.id, "news");
        assert!(m.capabilities.contains(&"news.headlines".to_string()));
        assert!(m.capabilities.contains(&"news.search".to_string()));
    }
    #[tokio::test]
    #[ignore = "requires live Google News publisher feeds"]
    async fn live_boston_location_publishers_return_headlines() {
        for (category, publisher, query) in [
            ("local", "wbur", "Boston"),
            ("regional", "nhpr", "New England"),
            ("national", "npr", "United States"),
            ("world", "npr", "world news"),
        ] {
            let response = NewsPlugin::new().execute(PluginRequest {
                capability: "news.headlines".into(),
                params: serde_json::json!({"category":category,"source":publisher,"query":query,"location":"Boston","region":"New England","country":"United States"}),
            }).await.expect("live source request failed");
            let items = response.data["items"].as_array().expect("headline list");
            println!(
                "{category}: {publisher}: {} verified headlines",
                items.len()
            );
            assert!(
                !items.is_empty(),
                "No current {category} headlines from {publisher}"
            );
            assert!(items.iter().all(|item| !item["title"]
                .as_str()
                .unwrap_or("")
                .trim()
                .is_empty()));
        }
    }
}
