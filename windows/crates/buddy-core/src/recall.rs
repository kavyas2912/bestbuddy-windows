//! Explicit entertainment recall, kept separate from ordinary requests and reminders.
pub struct RecallRequest {
    pub kind: &'static str,
    pub words: Vec<String>,
    pub last: bool,
}
pub fn request(input: &str) -> Option<RecallRequest> {
    let words: Vec<String> = input
        .to_lowercase()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect();
    let has = |value: &str| words.iter().any(|word| word == value);
    let kind = if has("anecdote") || has("anecdotes") {
        "anecdote"
    } else if has("story") || has("stories") || has("tale") {
        "story"
    } else if has("joke") || has("jokes") || has("pun") {
        "joke"
    } else {
        return None;
    };
    if !(has("recall") || has("repeat") || has("again")) {
        return None;
    }
    let last = has("last") || has("latest") || has("same");
    let skip = [
        "recall",
        "repeat",
        "again",
        "last",
        "latest",
        "same",
        "the",
        "a",
        "an",
        "my",
        "your",
        "that",
        "this",
        "one",
        "please",
        "can",
        "could",
        "you",
        "tell",
        "told",
        "me",
        "us",
        "share",
        "shared",
        "earlier",
        "previous",
        "previously",
        "about",
        "called",
        "named",
        "joke",
        "jokes",
        "pun",
        "story",
        "stories",
        "tale",
        "anecdote",
        "anecdotes",
        "funny",
    ];
    Some(RecallRequest {
        kind,
        last,
        words: words
            .into_iter()
            .filter(|word| !skip.contains(&word.as_str()))
            .collect(),
    })
}
pub fn same_content(a: &str, b: &str) -> bool {
    use std::collections::HashSet;
    let normal = |s: &str| {
        s.to_lowercase()
            .split(|ch: char| !ch.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let a = normal(a);
    let b = normal(b);
    if a == b {
        return true;
    }
    if a.len() < 3 || b.len() < 3 {
        return false;
    }
    let a: HashSet<_> = a.windows(3).map(|w| w.join(" ")).collect();
    let b: HashSet<_> = b.windows(3).map(|w| w.join(" ")).collect();
    a.intersection(&b).count() as f32 / a.union(&b).count() as f32 >= 0.85
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recall_is_explicit_and_does_not_capture_new_content_or_reminders() {
        assert!(request("Tell me a story").is_none());
        assert!(request("Remember my appointment").is_none());
        let r = request("Please recall the story about the kite").unwrap();
        assert_eq!(r.kind, "story");
        assert_eq!(r.words, ["kite"]);
        assert!(!r.last);
        assert!(request("Tell me the same joke again").unwrap().last);
    }
    #[test]
    fn normalisation_blocks_cross_source_reprints_without_blocking_distinct_content() {
        assert!(same_content(
            "The kite rose, and the wind sang!",
            "THE kite rose and the wind sang."
        ));
        assert!(!same_content(
            "The kite rose above the sea.",
            "A piano player practised scales at home."
        ));
    }
}
