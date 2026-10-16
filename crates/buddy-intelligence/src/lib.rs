//! No audio devices, networking or persistence. Hosts own actions, storage and consent.
mod analysis;
mod reasoning;
pub mod resource;
pub mod retrieval;
pub use analysis::{analyze, normalize, Analysis, Sentiment};
pub use reasoning::calculate;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::OnceLock,
    time::Instant,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    pub questions: Vec<String>,
    pub answer: String,
    pub subject: String,
    pub relation: String,
    pub source: String,
    pub followups: BTreeMap<String, String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phrase {
    pub phrase: String,
    pub intent: String,
    pub sentiment: String,
    pub tone: String,
    pub probability: f64,
    pub tenor: String,
    pub prosody: String,
}
pub fn bundled_knowledge() -> &'static [KnowledgeEntry] {
    static BANK: OnceLock<Vec<KnowledgeEntry>> = OnceLock::new();
    BANK.get_or_init(|| {
        serde_json::from_str(include_str!("../data/knowledge.json"))
            .expect("validated bundled knowledge")
    })
}
pub fn bundled_phrases() -> &'static [Phrase] {
    static BANK: OnceLock<Vec<Phrase>> = OnceLock::new();
    BANK.get_or_init(|| {
        serde_json::from_str(include_str!("../data/phrases.json"))
            .expect("validated bundled phrases")
    })
}
fn terms(text: &str) -> BTreeSet<String> {
    normalize(text)
        .split_whitespace()
        .filter(|w| {
            !matches!(
                *w,
                "what"
                    | "which"
                    | "is"
                    | "are"
                    | "was"
                    | "the"
                    | "a"
                    | "an"
                    | "of"
                    | "in"
                    | "tell"
                    | "me"
                    | "about"
                    | "please"
                    | "can"
                    | "could"
                    | "you"
                    | "do"
                    | "does"
            )
        })
        .map(str::to_string)
        .collect()
}
pub fn search(text: &str) -> Option<&'static KnowledgeEntry> {
    let query = normalize(text);
    let tokens = terms(text);
    if tokens.is_empty() {
        return None;
    }
    type IndexedQuestion = (String, BTreeSet<String>, usize);
    static INDEX: OnceLock<Vec<IndexedQuestion>> = OnceLock::new();
    let index = INDEX.get_or_init(|| {
        bundled_knowledge()
            .iter()
            .enumerate()
            .flat_map(|(id, e)| {
                e.questions
                    .iter()
                    .map(move |q| (normalize(q), terms(q), id))
            })
            .collect()
    });
    index
        .iter()
        .find(|(q, t, _)| *q == query || *t == tokens)
        .map(|(_, _, id)| &bundled_knowledge()[*id])
}
pub fn prefetch(analysis: &Analysis) -> Vec<&'static Phrase> {
    let label = if analysis.sentiment.positive >= 0.6 {
        "positive"
    } else if analysis.sentiment.negative >= 0.6 {
        "negative"
    } else {
        "neutral"
    };
    let mut phrases: Vec<_> = bundled_phrases()
        .iter()
        .filter(|p| {
            p.intent == analysis.intent && (p.sentiment == label || p.sentiment == "neutral")
        })
        .collect();
    phrases.sort_by(|a, b| {
        (b.tone == analysis.tone)
            .cmp(&(a.tone == analysis.tone))
            .then_with(|| b.probability.total_cmp(&a.probability))
    });
    phrases.truncate(5);
    phrases
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalAnswer {
    pub text: String,
    pub route: String,
    pub source: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub analysis: Analysis,
    pub route: String,
    pub elapsed_us: u64,
}
#[derive(Default)]
pub struct Intelligence {
    focus: Option<&'static KnowledgeEntry>,
    last: Option<LocalAnswer>,
    history: VecDeque<(String, String)>,
    pub trace: Option<Trace>,
}
impl Intelligence {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn observe(
        &mut self,
        input: &str,
        answer: &str,
        route: &str,
        elapsed: std::time::Duration,
    ) {
        // Private notes never enter ordinary dialogue history.
        if !route.starts_with("memory.") {
            self.history.push_back((
                input.chars().take(4000).collect(),
                answer.chars().take(4000).collect(),
            ));
            while self.history.len() > 12 {
                self.history.pop_front();
            }
        }
        if !route.starts_with("intelligence.knowledge") && route != "intelligence.repeat" {
            self.focus = None;
        }
        self.last = Some(LocalAnswer {
            text: answer.into(),
            route: route.into(),
            source: String::new(),
        });
        self.trace = Some(Trace {
            analysis: analyze(input),
            route: route.into(),
            elapsed_us: elapsed.as_micros().min(u64::MAX as u128) as u64,
        });
    }
    /// Local-only history; explicit memory routes are excluded at observation.
    pub fn recent_turns(&self, current_input: &str) -> Vec<(String, String)> {
        self.history
            .iter()
            .rev()
            .skip(usize::from(
                self.history.back().is_some_and(|(q, _)| q == current_input),
            ))
            .take(3)
            .map(|(q, a)| (q.chars().take(200).collect(), a.chars().take(300).collect()))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
    /// Replace the local draft with the actual model response for repeat/follow-up.
    pub fn finalize_reply(&mut self, input: &str, answer: &str, route: &str) {
        if let Some((q, a)) = self.history.back_mut() {
            if q == input {
                *a = answer.chars().take(4000).collect();
            }
        }
        self.last = Some(LocalAnswer {
            text: answer.into(),
            route: route.into(),
            source: String::new(),
        });
        if let Some(trace) = &mut self.trace {
            trace.route = route.into();
        }
    }
    pub fn answer(&mut self, input: &str) -> Option<LocalAnswer> {
        let query = normalize(input);
        if matches!(query.as_str(), "say that again" | "repeat that") {
            return self.last.clone().map(|mut a| {
                a.route = if a.route.starts_with("memory.") {
                    "memory.repeat"
                } else {
                    "intelligence.repeat"
                }
                .into();
                a
            });
        }
        if let Some(entry) = search(&spoken_query(input)) {
            self.focus = Some(entry);
            return Some(LocalAnswer {
                text: entry.answer.clone(),
                route: "intelligence.knowledge".into(),
                source: entry.source.clone(),
            });
        }
        if let Some(previous) = self.focus {
            let followup = previous
                .followups
                .iter()
                .find(|(q, _)| normalize(q) == query)
                .map(|(_, a)| a.clone());
            let subject = ["and ", "what about ", "how about "]
                .iter()
                .find_map(|p| query.strip_prefix(p));
            if let Some(entry) = subject.and_then(|s| {
                bundled_knowledge().iter().find(|e| {
                    !previous.relation.is_empty()
                        && e.relation == previous.relation
                        && normalize(&e.subject) == s
                })
            }) {
                self.focus = Some(entry);
                return Some(LocalAnswer {
                    text: entry.answer.clone(),
                    route: "intelligence.knowledge.context".into(),
                    source: entry.source.clone(),
                });
            }
            let answer = followup.or_else(|| {
                matches!(
                    query.as_str(),
                    "explain it" | "explain that" | "explain simply"
                )
                .then(|| {
                    previous
                        .followups
                        .get("explain simply")
                        .unwrap_or(&previous.answer)
                        .clone()
                })
            });
            if let Some(text) = answer {
                return Some(LocalAnswer {
                    text,
                    route: "intelligence.knowledge.context".into(),
                    source: previous.source.clone(),
                });
            }
        }
        for prefix in ["spell ", "how do you spell "] {
            if let Some(word) = query.strip_prefix(prefix).filter(|w| !w.contains(' ')) {
                return Some(LocalAnswer {
                    text: format!(
                        "{} is spelled {}.",
                        word,
                        word.to_uppercase()
                            .chars()
                            .map(|c| c.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    route: "intelligence.spelling".into(),
                    source: "Deterministic spelling".into(),
                });
            }
        }
        if let Some(text) = calculate(input) {
            return Some(LocalAnswer {
                text,
                route: "intelligence.calculation".into(),
                source: "Bounded arithmetic".into(),
            });
        }
        // Narrow social matches must not swallow questions or connected app actions.
        let context = analyze(input);
        if matches!(
            query.as_str(),
            "yes"
                | "sure"
                | "agreed"
                | "okay"
                | "ok"
                | "no"
                | "wrong"
                | "i disagree"
                | "not quite"
                | "can you clarify"
                | "can you clarify that"
        ) {
            if let Some(phrase) = prefetch(&context).first() {
                return Some(LocalAnswer {
                    text: phrase.phrase.clone(),
                    route: "intelligence.phrase".into(),
                    source: "Bundled local phrase bank".into(),
                });
            }
        }
        None
    }
    /// Host diagnostic timing; never a microphone-to-speaker latency claim.
    pub fn timed_answer(&mut self, input: &str) -> (Option<LocalAnswer>, std::time::Duration) {
        let start = Instant::now();
        let answer = self.answer(input);
        (answer, start.elapsed())
    }
}
fn spoken_query(input: &str) -> String {
    let mut text = normalize(input);
    for prefix in [
        "hey buddy ",
        "buddy ",
        "can you tell me ",
        "could you tell me ",
        "please ",
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest.to_string();
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn python_analysis_parity() {
        let rows: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../tests/python_analysis.json")).unwrap();
        for row in rows {
            let a = analyze(row["text"].as_str().unwrap());
            assert_eq!(a.intent, row["intent"], "{}", row["text"]);
            assert!(
                (a.intent_confidence - row["intent_confidence"].as_f64().unwrap()).abs() < 1e-9
            );
            assert_eq!(a.emotion, row["emotion"]);
            assert_eq!(serde_json::to_value(&a.keywords).unwrap(), row["keywords"]);
            for (name, value) in [
                ("positive", a.sentiment.positive),
                ("negative", a.sentiment.negative),
                ("neutral", a.sentiment.neutral),
            ] {
                assert!((value - row["sentiment"][name].as_f64().unwrap()).abs() < 1e-9);
            }
        }
    }
    #[test]
    fn python_retrieval_parity() {
        let rows: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../tests/python_retrieval.json")).unwrap();
        for row in rows {
            let actual = search(row["question"].as_str().unwrap()).map(|e| e.subject.as_str());
            assert_eq!(actual, row["subject"].as_str(), "{}", row["question"]);
        }
    }
    #[test]
    fn followups_preserve_relation_and_abstain_on_unrelated_questions() {
        let mut i = Intelligence::default();
        assert!(i
            .answer("capital of France")
            .unwrap()
            .text
            .contains("Paris"));
        assert!(i.answer("and India").unwrap().text.contains("New Delhi"));
        assert!(i.answer("how does a satellite work").is_none());
        i.observe(
            "stop music",
            "Stopping music.",
            "music.stop",
            std::time::Duration::ZERO,
        );
        assert!(i.answer("and Japan").is_none());
    }
}
