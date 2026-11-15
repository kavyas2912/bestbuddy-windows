use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};
#[derive(Default)]
pub struct PhraseQueue {
    next: HashMap<String, usize>,
}
impl PhraseQueue {
    pub fn pick(&mut self, key: &str, items: &[&str]) -> String {
        if items.is_empty() {
            return String::new();
        }
        let n = self.next.entry(key.to_string()).or_default();
        let result = items[*n % items.len()].to_string();
        *n = n.wrapping_add(1);
        result
    }
}
static QUEUE: OnceLock<Mutex<PhraseQueue>> = OnceLock::new();
pub fn phrase(key: &str) -> String {
    let items: &[&str] = match key {
        "greeting" => &[
            "Ready for a little conversation?",
            "What shall we explore together?",
            "A fresh thought or a useful update? I am all ears.",
            "Lovely to hear from you. What is on your mind?",
        ],
        "wellbeing" => &[
            "Curious, cheerful, and ready to help.",
            "In fine form. How is your day treating you?",
            "Plenty of ideas, and time for yours.",
            "All set for a useful chat and perhaps a small laugh.",
        ],
        "capabilities" => &[
            "I can help with your weather, news and market updates. Which shall we try?",
            "Tell me an interest, ask a question, or check your daybook with me.",
            "We can review your schedule, explore a headline, or enjoy a light conversation.",
            "Ask for your briefing, a useful fact, or help with your reminders.",
        ],
        "clarification" => &[
            "Could you give me one more detail so I can help properly?",
            "Let us try that in a few words. What would you like me to do?",
            "I have not quite caught your meaning. Would you rephrase it?",
            "A small clue, please. Are you asking a question or giving a command?",
        ],
        "guide" => &[
            "Allow me to guide you.",
            "Let us make this easy.",
            "At your service.",
            "A small detail, neatly handled.",
        ],
        _ => &["Ready when you are."],
    };
    QUEUE
        .get_or_init(|| Mutex::new(PhraseQueue::default()))
        .lock()
        .map(|mut q| q.pick(key, items))
        .unwrap_or_else(|_| items[0].to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_situation_exhausts_choices_before_repeating() {
        let mut q = PhraseQueue::default();
        let rows = ["one", "two", "three", "four"];
        let results = (0..4)
            .map(|_| q.pick("greeting", &rows))
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(results.len(), 4);
        assert_eq!(q.pick("greeting", &rows), "one");
        assert_eq!(q.pick("guide", &rows), "one");
    }
}
