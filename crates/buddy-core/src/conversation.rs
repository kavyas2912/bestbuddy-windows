use chrono::{DateTime, Local};
use quick_xml::{events::Event, Reader};
use serde::{Deserialize, Serialize};

/// A source-backed topic cue, not a copied transcript or a claim of understanding the audio.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationCue {
    pub title: String,
    pub synopsis: String,
    pub url: String,
    pub source: String,
    pub saved_at: DateTime<Local>,
}

pub struct ConversationFeed {
    pub name: &'static str,
    pub url: &'static str,
}

// Official publisher feeds. Each feed supplies as many entries as it publishes; BestBuddy
// does not impose a fixed word, phrase, or episode-count ceiling.
pub const FEEDS: &[ConversationFeed] = &[
    ConversationFeed {
        name: "NASA Curious Universe",
        url: "https://www.nasa.gov/feeds/podcasts/curious-universe",
    },
    ConversationFeed {
        name: "NASA Houston We Have a Podcast",
        url: "https://www.nasa.gov/feeds/podcasts/houston-we-have-a-podcast",
    },
    ConversationFeed {
        name: "PBS Brooks and Capehart",
        url: "https://www.pbs.org/newshour/feeds/rss/podcasts/brooks-and-capehart",
    },
    ConversationFeed {
        name: "PBS NewsHour Segments",
        url: "https://www.pbs.org/newshour/feeds/rss/podcasts/segments",
    },
    ConversationFeed {
        name: "NASA Photojournal",
        url: "https://science.nasa.gov/feed/photojournal/latest-content/",
    },
    ConversationFeed {
        name: "NASA JPL Stories",
        url: "https://www.jpl.nasa.gov/feeds/news/",
    },
    ConversationFeed {
        name: "MedlinePlus New Links",
        url: "https://medlineplus.gov/groupfeeds/new.xml",
    },
    ConversationFeed {
        name: "Project Gutenberg New Books",
        url: "https://www.gutenberg.org/cache/epub/feeds/today.rss",
    },
];

pub async fn fetch_feed(feed: &'static ConversationFeed) -> Result<Vec<ConversationCue>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(feed.url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| format!("{}: {e}", feed.name))?;
    let body = response.text().await.map_err(|e| e.to_string())?;
    parse_feed(&body, feed.name)
}

pub fn parse_feed(xml: &str, source: &str) -> Result<Vec<ConversationCue>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut cues = Vec::new();
    let mut in_entry = false;
    let mut field = String::new();
    let (mut title, mut synopsis, mut url) = (String::new(), String::new(), String::new());
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let name = String::from_utf8_lossy(event.name().as_ref()).to_string();
                if name == "item" || name == "entry" {
                    in_entry = true;
                    title.clear();
                    synopsis.clear();
                    url.clear();
                } else if in_entry {
                    field = name;
                }
            }
            Ok(Event::Empty(event)) if in_entry && event.name().as_ref() == b"link" => {
                for attr in event.attributes().flatten() {
                    if attr.key.as_ref() == b"href" {
                        url = String::from_utf8_lossy(&attr.value).into_owned();
                    }
                }
            }
            Ok(Event::Text(event)) if in_entry => {
                let value = event.unescape().map_err(|e| e.to_string())?.into_owned();
                append_field(&field, &value, &mut title, &mut synopsis, &mut url);
            }
            Ok(Event::CData(event)) if in_entry => {
                let value = String::from_utf8_lossy(event.as_ref());
                append_field(&field, &value, &mut title, &mut synopsis, &mut url);
            }
            Ok(Event::End(event)) => {
                let name = event.name();
                if name.as_ref() == b"item" || name.as_ref() == b"entry" {
                    let title = clean(&title);
                    let synopsis = synopsis
                        .split("PBS News is supported by")
                        .next()
                        .unwrap_or(&synopsis);
                    let url = url.trim().to_string();
                    if !title.is_empty()
                        && reqwest::Url::parse(&url)
                            .map(|u| u.scheme() == "https")
                            .unwrap_or(false)
                    {
                        cues.push(ConversationCue {
                            title,
                            synopsis: clean(synopsis),
                            url,
                            source: source.to_string(),
                            saved_at: Local::now(),
                        });
                    }
                    in_entry = false;
                }
                field.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("{source} XML: {e}")),
            _ => {}
        }
        buffer.clear();
    }
    if cues.is_empty() {
        return Err(format!("{source}: feed contained no usable episode links"));
    }
    Ok(cues)
}

fn append_field(
    field: &str,
    value: &str,
    title: &mut String,
    synopsis: &mut String,
    url: &mut String,
) {
    match field {
        "title" => title.push_str(value),
        "description" | "summary" | "content:encoded" => synopsis.push_str(value),
        "link" | "guid" if url.is_empty() => url.push_str(value),
        _ => {}
    }
}

fn clean(value: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for ch in value.chars() {
        match ch {
            '<' => tag = true,
            '>' => tag = false,
            _ if !tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_rss_and_atom_without_copying_markup() {
        let rss = r#"<rss><channel><item><title>Finding life</title><link>https://www.nasa.gov/episode</link><description><![CDATA[<p>How do scientists search?</p>]]></description></item></channel></rss>"#;
        let cues = parse_feed(rss, "NASA").unwrap();
        assert_eq!(cues[0].title, "Finding life");
        assert_eq!(cues[0].synopsis, "How do scientists search?");
        let atom = r#"<feed><entry><title>Space talk</title><link href="https://www.nasa.gov/talk"/><summary>Questions and answers</summary></entry></feed>"#;
        assert_eq!(
            parse_feed(atom, "NASA").unwrap()[0].url,
            "https://www.nasa.gov/talk"
        );
    }
    #[tokio::test]
    #[ignore = "requires live publisher feeds"]
    async fn live_publishers_supply_usable_episodes() {
        for feed in FEEDS {
            let cues = fetch_feed(feed).await.expect(feed.name);
            assert!(!cues.is_empty(), "{} returned no cues", feed.name);
            assert!(cues.iter().all(|cue| cue.url.starts_with("https://")));
        }
    }
}
