use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Sentiment {
    pub positive: f64,
    pub negative: f64,
    pub neutral: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis {
    pub intent: String,
    pub intent_confidence: f64,
    pub sentiment: Sentiment,
    pub emotion: String,
    pub emotion_confidence: f64,
    pub keywords: Vec<String>,
    pub tone: String,
}
pub fn normalize(text: &str) -> String {
    text.to_lowercase()
        .replace('ß', "ss")
        .replace('ς', "σ")
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
fn contains(text: &str, phrase: &str) -> bool {
    text.match_indices(phrase).any(|(at, _)| {
        let word = |c: char| c.is_alphanumeric() || c == '_';
        !text[..at].chars().next_back().is_some_and(word)
            && !text[at + phrase.len()..].chars().next().is_some_and(word)
    })
}
pub fn analyze(text: &str) -> Analysis {
    let lower = text.to_lowercase();
    // Python's stable tie-break order: specific intentions precede generic questions.
    let groups: &[(&str, &[&str])] = &[
        (
            "clarification",
            &[
                "what do you mean",
                "can you clarify",
                "explain",
                "i don't understand",
            ],
        ),
        (
            "request",
            &["please", "can you help", "could you help", "would you help"],
        ),
        (
            "disagreement",
            &[
                "no",
                "not really",
                "not quite",
                "wrong",
                "disagree",
                "doesn't sound right",
            ],
        ),
        (
            "agreement",
            &[
                "yes",
                "sure",
                "absolutely",
                "right",
                "correct",
                "agreed",
                "agree",
            ],
        ),
        (
            "greeting",
            &[
                "hi",
                "hello",
                "hey",
                "good morning",
                "good evening",
                "greetings",
            ],
        ),
        (
            "command",
            &["start", "stop", "open", "close", "run", "play", "pause"],
        ),
        (
            "question",
            &[
                "what",
                "when",
                "where",
                "how",
                "why",
                "who",
                "can you",
                "could you",
                "would you",
            ],
        ),
    ];
    let mut intent = "statement";
    let (mut highest, mut total) = (0, 0);
    for (name, phrases) in groups {
        let hits = phrases.iter().filter(|p| contains(&lower, p)).count();
        total += hits;
        if hits > highest {
            highest = hits;
            intent = name;
        }
    }
    let hits = |words: &[&str]| {
        (words.iter().filter(|p| contains(&lower, p)).count() as f64 / 2.0).min(1.0)
    };
    let positive = hits(&[
        "good",
        "great",
        "love",
        "happy",
        "wonderful",
        "excellent",
        "nice",
        "thanks",
        "thank you",
    ]);
    let negative = hits(&[
        "bad",
        "angry",
        "hate",
        "sad",
        "terrible",
        "worst",
        "frustrated",
        "upset",
        "awful",
    ]);
    let mut emotion = "neutral";
    let mut confidence = 0.7;
    for (name, words, score) in [
        (
            "happy",
            &["excited", "great", "happy", "amazing", "wonderful"][..],
            0.9,
        ),
        ("angry", &["angry", "mad", "upset", "furious"][..], 0.8),
        ("sad", &["sad", "sorry", "down", "depressed"][..], 0.8),
    ] {
        if words.iter().any(|w| contains(&lower, w)) {
            emotion = name;
            confidence = score;
            break;
        }
    }
    let mut counts: Vec<(String, usize)> = Vec::new();
    for word in lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() > 2)
    {
        if let Some((_, count)) = counts.iter_mut().find(|(w, _)| w == word) {
            *count += 1;
        } else {
            counts.push((word.into(), 1));
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    Analysis {
        intent: intent.into(),
        intent_confidence: if total == 0 {
            0.5
        } else {
            highest as f64 / total as f64
        },
        sentiment: Sentiment {
            positive,
            negative,
            neutral: (1.0 - positive - negative).max(0.0),
        },
        emotion: emotion.into(),
        emotion_confidence: confidence,
        keywords: counts.into_iter().take(5).map(|(w, _)| w).collect(),
        tone: if positive > 0.5 { "friendly" } else { "casual" }.into(),
    }
}
