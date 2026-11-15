/// Reject navigation and section pages; retain event-specific, multilingual headlines.
pub fn is_article_headline(title: &str, url: &str, source: &str) -> bool {
    fn normal(s: &str) -> String {
        s.chars()
            .flat_map(char::to_lowercase)
            .map(|c| if c.is_alphanumeric() { c } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
    fn generic(s: &str) -> bool {
        matches!(
            s,
            "" | "home"
                | "homepage"
                | "news"
                | "latest news"
                | "breaking news"
                | "latest breaking news"
                | "news headlines"
                | "latest news headlines"
                | "top stories"
                | "top news"
                | "world"
                | "world news"
                | "india"
                | "india news"
                | "national news"
                | "regional news"
                | "local news"
                | "business"
                | "business news"
                | "markets"
                | "market news"
                | "stock market news"
                | "science"
                | "science news"
                | "technology"
                | "technology news"
                | "tech news"
                | "sports"
                | "sports news"
                | "entertainment news"
                | "live news"
                | "today s news"
                | "news today"
                | "समाचार"
                | "ताज़ा खबरें"
                | "मुख्य समाचार"
        )
    }
    let clean = normal(title);
    let publisher = normal(source);
    let navigation_words = [
        "breaking",
        "latest",
        "national",
        "international",
        "world",
        "india",
        "news",
        "headlines",
        "updates",
        "live",
        "today",
        "and",
    ];
    let title_without_source = clean.strip_suffix(&publisher).unwrap_or(&clean).trim();
    if title_without_source
        .split_whitespace()
        .all(|word| navigation_words.contains(&word))
    {
        return false;
    }

    if generic(&clean) || (!publisher.is_empty() && clean == publisher) {
        return false;
    }
    let parts: Vec<_> = title
        .split(['|', '–', '—'])
        .flat_map(|part| part.split(" - "))
        .map(normal)
        .collect();
    if parts
        .iter()
        .all(|part| generic(part) || (!publisher.is_empty() && part == &publisher))
    {
        return false;
    }
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return false;
    }
    let path = parsed.path().trim_matches('/');
    if path.is_empty() {
        return parsed.query_pairs().any(|(k, v)| {
            matches!(k.as_ref(), "p" | "article" | "article_id" | "storyid") && !v.is_empty()
        });
    }
    let sections = [
        "news",
        "world",
        "india",
        "business",
        "markets",
        "market",
        "sports",
        "science",
        "technology",
        "tech",
        "entertainment",
        "latest",
        "index.html",
        "index.htm",
        "home",
        "national",
        "regional",
        "local",
        "opinion",
        "videos",
        "video",
        "photos",
        "gallery",
        "about",
        "contact",
        "subscribe",
        "login",
        "newsletter",
        "live",
        "europe",
        "asia",
        "africa",
        "us",
        "uk",
        "canada",
        "australia",
        "japan",
    ];
    if path.split('/').all(|part| {
        sections.contains(&part.to_ascii_lowercase().as_str()) || generic(&normal(part))
    }) {
        return false;
    }
    !clean.is_empty()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_sections_but_keeps_real_reports() {
        for (t, u, s) in [
            ("Latest News | BBC News", "https://bbc.com/news", "BBC News"),
            (
                "Business News - Reuters",
                "https://reuters.com/business",
                "Reuters",
            ),
            ("World News", "https://example.org/news/world/", "Example"),
            ("BBC News", "https://bbc.com/", "BBC News"),
        ] {
            assert!(!is_article_headline(t, u, s), "{t}");
        }
        for (t, u) in [
            (
                "Bank cuts lending rate after inflation eases",
                "https://example.org/business/rates-cut-123",
            ),
            (
                "चंद्रमा पर नए खनिज मिले",
                "https://example.org/science/article/123",
            ),
            (
                "Election results: Live updates as counting begins",
                "https://example.org/news/live/election-123",
            ),
            ("Markets fall", "https://example.org/?p=42"),
            (
                "Government announces new railway link - BBC News",
                "https://news.google.com/rss/articles/ABC",
            ),
        ] {
            assert!(is_article_headline(t, u, "BBC News"), "{t}");
        }
    }
}

/// Require prose beyond the headline/publisher; this does not certify access to the full article.
pub fn has_article_excerpt(title: &str, source: &str, summary: &str) -> bool {
    fn normal(text: &str) -> String {
        text.chars()
            .flat_map(char::to_lowercase)
            .map(|c| if c.is_alphanumeric() { c } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
    let text = normal(summary);
    if text.chars().count() < 60 {
        return false;
    }
    if [
        "read more",
        "read the full",
        "click here",
        "subscribe",
        "sign in",
        "enable javascript",
        "accept cookies",
    ]
    .iter()
    .any(|prefix| text == *prefix || text.starts_with(&format!("{prefix} ")))
    {
        return false;
    }
    let known = normal(&format!("{title} {source}"));
    let words: std::collections::HashSet<_> = known.split_whitespace().collect();
    text.split_whitespace()
        .filter(|word| !words.contains(word))
        .map(|word| word.chars().count())
        .sum::<usize>()
        >= 24
}
#[cfg(test)]
mod excerpt_tests {
    use super::*;
    #[test]
    fn requires_article_prose_beyond_headline_and_source() {
        let title = "Council approves transport funding after public consultation";
        assert!(!has_article_excerpt(title, "Publisher", ""));
        assert!(!has_article_excerpt(
            title,
            "Publisher",
            &format!("{title} - Publisher")
        ));
        assert!(!has_article_excerpt(
            title,
            "Publisher",
            "Subscribe to read more of our exclusive reporting from across the world today."
        ));
        assert!(has_article_excerpt(title, "Publisher", "The authority published a detailed decision setting out construction dates, projected costs and the expected impact on local households."));
        assert!(has_article_excerpt("चंद्रमा पर नए खनिज मिले", "समाचार", "वैज्ञानिकों की टीम ने अपने अध्ययन में बताया कि इन खनिजों से चंद्रमा के निर्माण और उसकी सतह की संरचना को समझने में सहायता मिलेगी।"));
    }
}
