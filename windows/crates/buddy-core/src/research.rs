//! Small source-attributed extracts; source text is never executable authority.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResearchExtract {
    pub title: String,
    pub summary: String,
    pub url: String,
    pub source: String,
    pub topic: String,
}
impl ResearchExtract {
    pub fn validated(self) -> Option<Self> {
        let url = self.url.trim();
        if url.len() > 2048 || !url.starts_with("https://") || url.chars().any(char::is_whitespace)
        {
            return None;
        }
        let authority = url[8..].split(['/', '?', '#']).next()?;
        if authority.contains('@') {
            return None;
        }
        let mut parts = authority.split(':');
        let host = parts.next()?;
        if let Some(port) = parts.next() {
            if port.parse::<u16>().is_err() {
                return None;
            }
        }
        if parts.next().is_some()
            || !host.contains('.')
            || host.parse::<std::net::IpAddr>().is_ok()
            || [".local", ".localhost", ".internal", ".test", ".invalid"]
                .iter()
                .any(|suffix| host.ends_with(suffix))
        {
            return None;
        }
        let title = self.title.trim().chars().take(180).collect::<String>();
        let summary = self.summary.trim().chars().take(900).collect::<String>();
        if title.chars().count() < 4 || summary.chars().count() < 80 {
            return None;
        }
        Some(Self {
            title,
            summary,
            url: url.to_string(),
            source: self.source.chars().take(100).collect(),
            topic: self.topic.chars().take(64).collect(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_require_public_attribution_and_real_content() {
        let make = |url: &str| {
            ResearchExtract{title:"A science topic".into(),summary:"A source-backed observation about a science topic with enough explanatory detail to be useful offline.".into(),url:url.into(),source:"Publisher".into(),topic:"science".into()}
        };
        assert!(make("https://example.org/science").validated().is_some());
        for url in [
            "http://example.org",
            "https://127.0.0.1/a",
            "https://user:secret@example.org/a",
            "https://a.local/a",
            "javascript:alert(1)",
        ] {
            assert!(make(url).validated().is_none())
        }
    }
}
