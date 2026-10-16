pub fn resource_budget() -> buddy_intelligence::resource::ResourceBudget {
    buddy_intelligence::resource::ResourceBudget::from_env()
}
pub mod conversation;
pub mod local_learning;
pub mod persona;
pub mod profile;
use buddy_plugin::{PluginRegistry, PluginRequest, PluginResponse};
use chrono::{DateTime, Local, Timelike};
pub use conversation::ConversationCue;
pub use profile::{Consents, Contact, Reminder};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EngineState {
    Starting,
    Ready,
    Listening,
    Thinking,
    Speaking,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UserProfile {
    pub birthdate: String,
    pub timezone: String,
    pub language: String,
    pub style: String,
    pub contacts: Vec<Contact>,
    pub reminders: Vec<Reminder>,
    pub consents: Consents,
    pub onboarding_complete: bool,
    pub notes: Vec<String>,
    pub local_learning: local_learning::LearningPolicy,
    pub phrasebook: std::collections::BTreeMap<String, String>,
    pub name: String,
    pub location: String,
    pub country: String,
    pub region: String,
    pub interests: Vec<String>,
    pub preferences: Vec<String>,
    pub consent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LexiconEntry {
    pub term: String,
    pub source: String,
    pub learned_at: DateTime<Local>,
    #[serde(default = "initial_lexicon_frequency")]
    pub frequency: u32,
}

fn initial_lexicon_frequency() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeItem {
    pub title: String,
    pub summary: String,
    pub url: String,
    pub category: String,
    pub source: String,
    pub saved_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentItem {
    pub kind: String,
    pub text: String,
    pub topic: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshStockItem {
    pub id: String,
    pub kind: String,
    pub topic: String,
    pub text: String,
    pub source_name: String,
    pub source_url: String,
    pub saved_at: DateTime<Local>,
    pub used_count: u32,
    pub last_used_at: Option<DateTime<Local>>,
}

impl FreshStockItem {
    fn from_cue(cue: &ConversationCue) -> Self {
        let source = cue.source.to_lowercase();
        let kind = if source.contains("medline") {
            "health"
        } else if source.contains("gutenberg") {
            "audiobook"
        } else if source.contains("nasa") || source.contains("jpl") {
            "science"
        } else {
            "story"
        };
        let text = if kind == "health" {
            format!(
                "MedlinePlus has a health resource titled {}. Shall we open the source together?",
                cue.title
            )
        } else if kind == "audiobook" {
            format!("Project Gutenberg has added {}. Would you like to explore this public-domain book?", cue.title)
        } else {
            format!(
                "I found a {} piece from {} called {}. Would you like to explore it together?",
                kind, cue.source, cue.title
            )
        };
        Self {
            id: cue.url.clone(),
            kind: kind.into(),
            topic: format!("{} {}", cue.title, cue.synopsis),
            text,
            source_name: cue.source.clone(),
            source_url: cue.url.clone(),
            saved_at: cue.saved_at,
            used_count: 0,
            last_used_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LearningStatus {
    pub network: bool,
    pub lexicon_terms: usize,
    pub knowledge_items: usize,
    pub content_items: usize,
    pub conversation_items: usize,
    pub fresh_stock_items: usize,
    pub conversation_sources: usize,
    pub last_prefetch: Option<DateTime<Local>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuddyResponse {
    pub text: String,
    pub capability: Option<String>,
    pub plugin: Option<PluginResponse>,
}

struct NewsReading {
    headlines: Vec<String>,
    next: usize,
}

impl NewsReading {
    fn next_chunk(&mut self) -> String {
        if self.headlines.is_empty() {
            return "I couldn't find current headlines to read.".into();
        }
        if self.next >= self.headlines.len() {
            return "That was the last headline in this set. Ask me to read the news again for a fresh set.".into();
        }
        let start = self.next;
        self.next = (start + 3).min(self.headlines.len());
        let headlines = self.headlines[start..self.next]
            .iter()
            .map(|title| title.trim_end_matches(&['.', '!', '?'][..]))
            .collect::<Vec<_>>()
            .join(". ");
        let mut response = format!(
            "Headlines {} to {} of {}. {}.",
            start + 1,
            self.next,
            self.headlines.len(),
            headlines
        );
        if self.next < self.headlines.len() {
            response.push_str(" Say continue for more.");
        }
        response
    }
}

pub struct BuddyEngine {
    pub state: EngineState,
    pub user: UserProfile,
    pub plugins: PluginRegistry,
    history: VecDeque<String>,
    lexicon: Vec<LexiconEntry>,
    knowledge: Vec<KnowledgeItem>,
    content: Vec<ContentItem>,
    conversation: Vec<ConversationCue>,
    fresh_stock: Vec<FreshStockItem>,
    active_cue_url: Option<String>,
    news_reading: Option<NewsReading>,
    visible_news: Vec<String>,
    persona_recent: VecDeque<&'static str>,
    persona_turn: usize,
    pending_support: Option<&'static str>,
    started_at: DateTime<Local>,
    data_dir: PathBuf,
    learning: LearningStatus,
    intelligence: buddy_intelligence::Intelligence,
    learned: Vec<local_learning::LearnedDocument>,
    retrieval_index: Option<buddy_intelligence::retrieval::RetrievalIndex>,
    local_learning_progress: local_learning::LearningProgress,
}

impl BuddyEngine {
    pub fn new(
        user_name: impl Into<String>,
        location: impl Into<String>,
        plugins: PluginRegistry,
        data_dir: impl Into<PathBuf>,
    ) -> Self {
        let data_dir = data_dir.into();
        let _ = fs::create_dir_all(&data_dir);
        let mut user = load_json::<UserProfile>(&data_dir.join("profile.json")).unwrap_or_default();
        if user.name.trim().is_empty() {
            user.name = user_name.into();
        }
        if user.location.trim().is_empty() {
            user.location = location.into();
        }
        let lexicon =
            load_json::<Vec<LexiconEntry>>(&data_dir.join("lexicon.json")).unwrap_or_default();
        let knowledge =
            load_json::<Vec<KnowledgeItem>>(&data_dir.join("knowledge.json")).unwrap_or_default();
        let content = load_json::<Vec<ContentItem>>(&data_dir.join("content.json"))
            .unwrap_or_else(|| seed_content());
        let conversation = load_json::<Vec<ConversationCue>>(&data_dir.join("conversation.json"))
            .unwrap_or_default();
        let fresh_stock = load_json::<Vec<FreshStockItem>>(&data_dir.join("fresh-stock.json"))
            .unwrap_or_default();
        let last_prefetch = knowledge
            .iter()
            .map(|k| k.saved_at)
            .chain(conversation.iter().map(|c| c.saved_at))
            .max();
        let learning = LearningStatus {
            lexicon_terms: lexicon.len(),
            knowledge_items: knowledge.len(),
            content_items: content.len(),
            conversation_items: conversation.len(),
            fresh_stock_items: fresh_stock.len(),
            conversation_sources: conversation
                .iter()
                .map(|c| c.source.as_str())
                .collect::<std::collections::HashSet<_>>()
                .len(),
            last_prefetch,
            ..Default::default()
        };
        let learned = if user.consents.local_learning && user.consents.personalization {
            load_json::<Vec<local_learning::LearnedDocument>>(
                &data_dir.join("learned-transcripts.json"),
            )
            .unwrap_or_default()
        } else {
            Vec::new()
        };
        Self {
            learned,
            retrieval_index: None,
            local_learning_progress: local_learning::LearningProgress::default(),
            state: EngineState::Starting,
            user,
            plugins,
            history: VecDeque::with_capacity(48),
            lexicon,
            knowledge,
            content,
            conversation,
            fresh_stock,
            active_cue_url: None,
            news_reading: None,
            visible_news: Vec::new(),
            persona_recent: VecDeque::with_capacity(10),
            persona_turn: 0,
            pending_support: None,
            started_at: Local::now(),
            data_dir,
            learning,
            intelligence: buddy_intelligence::Intelligence::default(),
        }
    }

    pub fn start(&mut self) {
        self.state = EngineState::Ready;
        self.warm_retrieval();
        let _ = buddy_intelligence::search("capital of France");
        self.save_all();
        self.log("engine started");
    }

    pub fn set_visible_headlines(&mut self, headlines: Vec<String>) {
        self.visible_news = headlines
            .into_iter()
            .take(8)
            .map(|title| speakable_text(&title))
            .filter(|title| !title.is_empty())
            .collect();
    }
    pub fn uptime_seconds(&self) -> i64 {
        (Local::now() - self.started_at).num_seconds()
    }
    pub fn profile(&self) -> UserProfile {
        self.user.clone()
    }
    pub fn learning_status(&self) -> LearningStatus {
        self.learning.clone()
    }
    pub fn accept_prefetch(
        &mut self,
        results: Vec<PluginResponse>,
        cues: Vec<ConversationCue>,
        errors: Vec<String>,
    ) -> LearningStatus {
        self.learning.network = !results.is_empty() || !cues.is_empty();
        for r in results {
            if let Some(items) = r.data.get("items").and_then(|v| v.as_array()) {
                for item in items {
                    self.push_knowledge(KnowledgeItem {
                        title: item["title"].as_str().unwrap_or("").into(),
                        summary: item["summary"].as_str().unwrap_or("").into(),
                        url: item["url"].as_str().unwrap_or("").into(),
                        category: r.data["category"].as_str().unwrap_or("news").into(),
                        source: item["source"].as_str().unwrap_or("News").into(),
                        saved_at: Local::now(),
                    });
                }
            }
        }
        let mut new_stock = 0;
        for cue in cues {
            if let Some(existing) = self.fresh_stock.iter_mut().find(|item| item.id == cue.url) {
                let used = existing.used_count;
                let last_used = existing.last_used_at;
                let first_saved = existing.saved_at;
                *existing = FreshStockItem::from_cue(&cue);
                existing.used_count = used;
                existing.last_used_at = last_used;
                existing.saved_at = first_saved;
            } else {
                self.fresh_stock.push(FreshStockItem::from_cue(&cue));
                new_stock += 1;
            }
            if let Some(existing) = self
                .conversation
                .iter_mut()
                .find(|existing| existing.url == cue.url)
            {
                *existing = cue;
            } else {
                self.conversation.push(cue);
            }
        }
        if new_stock > 0 {
            // Rotate only old, repeatedly used entries when new source material
            // arrived. There is no arbitrary stock-size or interest-count cap.
            self.fresh_stock.retain(|item| {
                item.used_count < 2 || (Local::now() - item.saved_at).num_days() < 30
            });
        }
        self.learning.knowledge_items = self.knowledge.len();
        self.learning.conversation_items = self.conversation.len();
        self.learning.fresh_stock_items = self.fresh_stock.len();
        self.learning.conversation_sources = self
            .conversation
            .iter()
            .map(|c| c.source.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len();
        self.learning.lexicon_terms = self.lexicon.len();
        self.learning.last_prefetch = Some(Local::now());
        self.learning.last_error = if errors.is_empty() {
            None
        } else {
            Some(errors.join("; "))
        };
        self.save_all();
        self.log(&format!(
            "prefetch: {} news items; {} conversation cues from {} sources; {} fresh stock items; {} provider errors",
            self.knowledge.len(),
            self.conversation.len(),
            self.learning.conversation_sources,
            self.fresh_stock.len(),
            errors.len()
        ));
        self.learning.clone()
    }
    pub fn knowledge_items(&self) -> Vec<KnowledgeItem> {
        self.knowledge
            .iter()
            .rev()
            .filter(|k| (Local::now() - k.saved_at).num_hours() < 24)
            .cloned()
            .collect()
    }
    pub fn llm_context(&self, input: &str, include_personal: bool) -> serde_json::Value {
        let words = conversation_words(input);
        let relevance = |text: &str| {
            let haystack = normalized_phrase(text);
            words
                .iter()
                .filter(|word| haystack.contains(word.as_str()))
                .count()
        };
        let mut knowledge: Vec<_> = self
            .knowledge
            .iter()
            .filter(|item| (Local::now() - item.saved_at).num_hours() < 24)
            .filter_map(|item| {
                let score = relevance(&format!("{} {}", item.title, item.summary));
                (score > 0).then_some((score, item))
            })
            .collect();
        knowledge.sort_by_key(|(score, item)| {
            (std::cmp::Reverse(*score), std::cmp::Reverse(item.saved_at))
        });
        let mut discussions: Vec<_> = self
            .conversation
            .iter()
            .filter_map(|cue| {
                let score = relevance(&format!("{} {}", cue.title, cue.synopsis));
                (score > 0).then_some((score, cue))
            })
            .collect();
        discussions.sort_by_key(|(score, cue)| {
            (std::cmp::Reverse(*score), std::cmp::Reverse(cue.saved_at))
        });
        let lexicon: Vec<_> = if include_personal && self.user.consents.personalization {
            self.lexicon
                .iter()
                .filter(|entry| relevance(&entry.term) > 0)
                .map(|entry| entry.term.as_str())
                .take(24)
                .collect()
        } else {
            Vec::new()
        };
        let retrieval = if include_personal
            && self.user.consents.local_learning
            && self.user.consents.personalization
        {
            self.retrieval_index.as_ref().map(|index| index.search(input,self.resource_budget().retrieval_hits)
                .into_iter().map(|hit| { let d=&self.learned[hit.document].transcript;
                    serde_json::json!({"title":d.title,"publisher":d.publisher,"url":d.source_url,"excerpt":hit.text,"score":hit.score})
                }).collect::<Vec<_>>()).unwrap_or_default()
        } else {
            Vec::new()
        };
        serde_json::json!({
            "local_retrieval":retrieval,
            "recent_turns":if include_personal { self.intelligence.recent_turns(input) } else { Vec::new() },
            "analysis":if include_personal { let a=buddy_intelligence::analyze(input); serde_json::json!({"intent":a.intent,"tone":a.tone,"intent_confidence":a.intent_confidence}) } else { serde_json::Value::Null },
            "name": if include_personal { self.user.name.as_str() } else { "" },
            "style": if include_personal { self.user.style.as_str() } else { "friendly" },
            "interests": if include_personal { self.user.interests.as_slice() } else { &[] },
            "relevant_lexicon": lexicon,
            "news": knowledge.into_iter().take(8).map(|(_, item)| serde_json::json!({
                "title":item.title,"summary":item.summary,"source":item.source,"url":item.url
            })).collect::<Vec<_>>(),
            "discussions": discussions.into_iter().take(8).map(|(_, cue)| serde_json::json!({
                "title":cue.title,"synopsis":cue.synopsis,"source":cue.source,"url":cue.url
            })).collect::<Vec<_>>()
        })
    }
    pub fn lexicon(&self) -> Vec<LexiconEntry> {
        self.lexicon.iter().rev().cloned().collect()
    }
    pub fn content_items(&self) -> Vec<ContentItem> {
        self.content.clone()
    }

    pub fn persona_cards(&self) -> &'static [persona::PersonaCard] {
        persona::CARDS
    }

    pub fn next_persona(&mut self, kind: Option<&str>) -> persona::PersonaReply {
        let requested = kind.unwrap_or("");
        let interests = self
            .user
            .interests
            .iter()
            .map(|item| normalized_phrase(item))
            .collect::<Vec<_>>();
        if !matches!(requested, "joke" | "thought") && self.persona_turn % 4 != 0 {
            let choice = self
                .fresh_stock
                .iter()
                .enumerate()
                .filter(|(_, item)| requested.is_empty() || item.kind == requested)
                .max_by_key(|(_, item)| {
                    let match_score = interests
                        .iter()
                        .filter(|interest| {
                            normalized_phrase(&item.topic).contains(interest.as_str())
                        })
                        .count() as i64;
                    (match_score * 8 - item.used_count as i64 * 20, item.saved_at)
                })
                .map(|(index, _)| index);
            if let Some(index) = choice {
                self.persona_turn = self.persona_turn.wrapping_add(1);
                let item = &mut self.fresh_stock[index];
                item.used_count = item.used_count.saturating_add(1);
                item.last_used_at = Some(Local::now());
                let reply = persona::PersonaReply {
                    id: item.id.clone(),
                    kind: item.kind.clone(),
                    topic: item.topic.clone(),
                    text: item.text.clone(),
                    source_name: item.source_name.clone(),
                    source_url: item.source_url.clone(),
                };
                let _ = save_json(&self.data_dir.join("fresh-stock.json"), &self.fresh_stock);
                return reply;
            }
        }
        let candidates = persona::CARDS
            .iter()
            .filter(|card| requested.is_empty() || card.kind == requested)
            .collect::<Vec<_>>();
        let candidates = if candidates.is_empty() {
            persona::CARDS.iter().collect::<Vec<_>>()
        } else {
            candidates
        };
        let selected = candidates
            .iter()
            .enumerate()
            .max_by_key(|(index, card)| {
                let interest_score = interests
                    .iter()
                    .filter(|interest| {
                        card.topic.contains(interest.as_str()) || interest.contains(card.topic)
                    })
                    .count() as i32;
                let recent_penalty = self
                    .persona_recent
                    .iter()
                    .position(|id| *id == card.id)
                    .map(|position| 20 - position as i32)
                    .unwrap_or(0);
                (
                    interest_score * 3 - recent_penalty,
                    -(((*index + self.persona_turn) % candidates.len()) as i32),
                )
            })
            .map(|(_, card)| (*card).clone())
            .expect("persona catalog is not empty");
        self.persona_turn = self.persona_turn.wrapping_add(1);
        self.persona_recent.push_back(selected.id);
        if self.persona_recent.len() > 10 {
            self.persona_recent.pop_front();
        }
        persona::PersonaReply::from(&selected)
    }

    pub fn conversation_items(&self) -> Vec<ConversationCue> {
        self.conversation.clone()
    }

    pub fn set_profile(&mut self, profile: UserProfile) -> Result<(), String> {
        if profile.name.trim().is_empty() {
            return Err("Please provide the name BestBuddy should use.".into());
        }
        if !profile.birthdate.is_empty()
            && profile::age_on(&profile.birthdate, Local::now().date_naive()).is_none()
        {
            return Err("Birth date must be a valid date in the past.".into());
        }
        if !profile.consents.contacts_storage && !profile.contacts.is_empty() {
            return Err("Contact storage requires your explicit consent.".into());
        }
        let mut contact_ids = std::collections::HashSet::new();
        for contact in &profile.contacts {
            if contact.id.trim().is_empty()
                || !contact_ids.insert(contact.id.as_str())
                || contact.name.trim().is_empty()
            {
                return Err("Each contact needs a unique ID and a name.".into());
            }
            for date in [&contact.birthday, &contact.anniversary] {
                if !date.is_empty() && chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err()
                {
                    return Err("Check contact birthday and anniversary dates.".into());
                }
            }
            if contact.emergency_priority > 2 {
                return Err("Emergency priority must be 1 or 2.".into());
            }
            if !contact.signal_link.is_empty() {
                let valid = reqwest::Url::parse(&contact.signal_link)
                    .map(|url| url.scheme() == "https" && url.host_str() == Some("signal.me"))
                    .unwrap_or(false);
                if !valid {
                    return Err("Signal links must be copied from signal.me.".into());
                }
            }
        }
        let mut reminder_ids = std::collections::HashSet::new();
        for r in &profile.reminders {
            let valid_time =
                r.time.is_empty() || chrono::NaiveTime::parse_from_str(&r.time, "%H:%M").is_ok();
            if r.id.trim().is_empty()
                || !reminder_ids.insert(r.id.as_str())
                || r.title.trim().is_empty()
                || chrono::NaiveDate::parse_from_str(&r.date, "%Y-%m-%d").is_err()
                || r.lead_days > 366
                || !valid_time
            {
                return Err(
                    "Check reminder IDs, titles, dates, local times and lead times (0–366 days)."
                        .into(),
                );
            }
        }
        if profile.notes.len() > 100
            || profile
                .notes
                .iter()
                .any(|n| n.trim().is_empty() || n.chars().count() > 500)
        {
            return Err("Keep at most 100 notes of 1–500 characters each.".into());
        }
        if !profile.consents.profile_storage {
            return Err("Saving this profile requires profile storage consent.".into());
        }
        local_learning::validate_policy(&profile.local_learning)?;
        save_json(&self.data_dir.join("profile.json"), &profile)?;
        self.user = profile;
        if !self.user.consents.personalization {
            self.lexicon.clear();
            self.user.notes.clear();
            self.learned.clear();
            self.retrieval_index = None;
            self.intelligence.clear();
        }
        if !self.user.consents.local_learning {
            self.learned.clear();
            self.retrieval_index = None;
        }
        self.save_all();
        Ok(())
    }

    pub fn export_personal_data(&self) -> serde_json::Value {
        serde_json::json!({
            "format": "vce-buddy-personal-data-v1",
            "exported_at": Local::now().to_rfc3339(),
            "profile": self.user,
            "lexicon": self.lexicon,
            "learned_transcripts": self.learned,
            "knowledge": self.knowledge,
            "content": self.content,
            "conversation": self.conversation,
            "fresh_stock": self.fresh_stock,
        })
    }

    pub fn registration_draft(&self) -> Option<serde_json::Value> {
        load_json(&self.data_dir.join("registration-draft.json"))
    }

    pub fn save_registration_draft(&self, draft: &serde_json::Value) -> Result<(), String> {
        save_json(&self.data_dir.join("registration-draft.json"), draft)
    }

    pub fn clear_registration_draft(&self) -> Result<(), String> {
        let path = self.data_dir.join("registration-draft.json");
        if path.exists() {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn erase_personal_data(&mut self) -> Result<(), String> {
        for file in [
            "profile.json",
            "lexicon.json",
            "knowledge.json",
            "content.json",
            "conversation.json",
            "fresh-stock.json",
            "learned-transcripts.json",
            "registration-draft.json",
        ] {
            let path = self.data_dir.join(file);
            if path.exists() {
                fs::remove_file(&path)
                    .map_err(|e| format!("Could not remove {}: {e}", path.display()))?;
            }
        }
        let log_path = self.log_path();
        if log_path.exists() {
            fs::remove_file(&log_path)
                .map_err(|e| format!("Could not remove {}: {e}", log_path.display()))?;
        }
        self.user = UserProfile {
            name: "Friend".into(),
            ..Default::default()
        };
        self.history.clear();
        self.intelligence.clear();
        self.learned.clear();
        self.retrieval_index = None;
        self.local_learning_progress = Default::default();
        self.lexicon.clear();
        self.knowledge.clear();
        self.content.clear();
        self.conversation.clear();
        self.fresh_stock.clear();
        self.active_cue_url = None;
        self.learning = LearningStatus::default();
        self.state = EngineState::Ready;
        Ok(())
    }

    pub fn log_path(&self) -> PathBuf {
        self.data_dir
            .parent()
            .unwrap_or(&self.data_dir)
            .join("logs")
            .join("buddy.log")
    }

    pub fn log(&self, message: &str) {
        let path = self.log_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let line = format!(
            "{} | {}\n",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            message.replace('\n', " ")
        );
        use std::io::Write;
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = f.write_all(line.as_bytes());
        }
    }

    fn save_all(&self) {
        if self.user.consents.profile_storage {
            let _ = save_json(&self.data_dir.join("profile.json"), &self.user);
        }
        let _ = save_json(&self.data_dir.join("lexicon.json"), &self.lexicon);
        let _ = save_json(
            &self.data_dir.join("learned-transcripts.json"),
            &self.learned,
        );
        let _ = save_json(&self.data_dir.join("knowledge.json"), &self.knowledge);
        let _ = save_json(&self.data_dir.join("content.json"), &self.content);
        let _ = save_json(&self.data_dir.join("conversation.json"), &self.conversation);
        let _ = save_json(&self.data_dir.join("fresh-stock.json"), &self.fresh_stock);
    }

    pub fn learn_terms(&mut self, text: &str) -> Vec<String> {
        if !self.user.consents.personalization {
            return Vec::new();
        }
        let stop = [
            "this", "that", "with", "from", "have", "your", "about", "there", "their", "would",
            "could", "should", "which", "where", "what", "when", "into", "today", "latest", "news",
            "here", "just", "been", "will", "they", "them", "than", "then", "were", "also", "more",
            "some", "very", "buddy", "hello",
        ];
        let mut added = Vec::new();
        let mut changed = false;
        let words = text
            .split(|c: char| !c.is_alphanumeric())
            .map(|w| w.to_lowercase())
            .filter(|w| w.len() >= 4 && !stop.contains(&w.as_str()));
        for term in words {
            if let Some(entry) = self.lexicon.iter_mut().find(|e| e.term == term) {
                entry.frequency = entry.frequency.saturating_add(1);
                changed = true;
                continue;
            }
            self.lexicon.push(LexiconEntry {
                term: term.clone(),
                source: "personal.conversation".into(),
                learned_at: Local::now(),
                frequency: 1,
            });
            added.push(term);
            changed = true;
        }
        if changed {
            self.save_all();
        }
        added
    }

    fn push_knowledge(&mut self, item: KnowledgeItem) {
        if self
            .knowledge
            .iter()
            .any(|k| k.url == item.url && !item.url.is_empty())
        {
            return;
        }
        if item.title.trim().is_empty() {
            return;
        }
        self.knowledge
            .retain(|k| (Local::now() - k.saved_at).num_hours() < 24);
        self.knowledge.push(item);
    }

    pub fn local_learning_status(&self) -> serde_json::Value {
        let mut progress = self.local_learning_progress.clone();
        progress.documents = self.learned.len();
        serde_json::json!({"catalogue":local_learning::bundled_sources(),"policy":self.user.local_learning,"progress":progress,"consented":self.user.consents.local_learning && self.user.consents.personalization,
            "intelligence":self.intelligence_trace(),"documents":self.learned.iter().map(|d|serde_json::json!({"id":d.transcript.id,"title":d.transcript.title,"publisher":d.transcript.publisher,"source_url":d.transcript.source_url,"kind":d.transcript.kind})).collect::<Vec<_>>()})
    }
    pub fn begin_local_learning(&mut self, force: bool) -> Option<local_learning::LearningPolicy> {
        let policy = &self.user.local_learning;
        if !self.user.consents.local_learning
            || !self.user.consents.personalization
            || !policy.enabled
            || self.local_learning_progress.running
        {
            return None;
        }
        if !force
            && self.local_learning_progress.last_attempt.is_some_and(|at| {
                (Local::now() - at).num_minutes() < policy.interval_minutes as i64
            })
        {
            return None;
        }
        self.local_learning_progress.last_attempt = Some(Local::now());
        self.local_learning_progress.running = true;
        Some(policy.clone())
    }
    pub fn import_transcripts(
        &mut self,
        batch: Vec<local_learning::Transcript>,
    ) -> Result<usize, String> {
        if !self.user.consents.local_learning || !self.user.consents.personalization {
            return Err(
                "Enable local learning and personalization before importing transcripts.".into(),
            );
        }
        if batch.len() > 100 {
            return Err("Import at most 100 transcripts per batch.".into());
        }
        for transcript in &batch {
            local_learning::validate_transcript(transcript)?;
        }
        let previous = self.learned.clone();
        let previous_lexicon = self.lexicon.clone();
        let mut changed = 0;
        for transcript in batch {
            if self.learned.iter().any(|d| {
                d.transcript.id == transcript.id
                    && d.transcript.source_url == transcript.source_url
                    && d.transcript.text == transcript.text
            }) {
                continue;
            }
            self.learned.retain(|d| {
                !(d.transcript.id == transcript.id
                    && d.transcript.source_url == transcript.source_url)
            });
            self.learn_terms(&transcript.text);
            let terms =
                local_learning::tokens(&format!("{} {}", transcript.title, transcript.text));
            self.learned.push(local_learning::LearnedDocument {
                transcript,
                ingested_at: Local::now(),
                terms,
            });
            changed += 1;
        }
        if self.learned.len() > 200 {
            self.learned.drain(..self.learned.len() - 200);
        }
        if self.user.consents.profile_storage {
            if let Err(error) = save_json(
                &self.data_dir.join("learned-transcripts.json"),
                &self.learned,
            ) {
                self.learned = previous;
                self.lexicon = previous_lexicon;
                return Err(error);
            }
        }
        self.save_all();
        self.retrieval_index = None;
        self.warm_retrieval();
        self.local_learning_progress.imported = changed;
        Ok(changed)
    }
    pub fn finish_local_learning(&mut self, errors: Vec<String>) {
        self.local_learning_progress.running = false;
        if errors.is_empty() {
            self.local_learning_progress.last_success = Some(Local::now());
        }
        self.local_learning_progress.errors = errors;
    }
    pub fn clear_local_learning(&mut self) -> Result<(), String> {
        let path = self.data_dir.join("learned-transcripts.json");
        if path.exists() {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        self.learned.clear();
        self.retrieval_index = None;
        self.local_learning_progress = Default::default();
        Ok(())
    }

    pub fn voice_allowed(&self) -> bool {
        self.user.onboarding_complete
            && self.user.consents.microphone
            && self.user.consents.voice_processing
    }

    pub fn resource_budget(&self) -> buddy_intelligence::resource::ResourceBudget {
        buddy_intelligence::resource::ResourceBudget::from_env()
    }
    fn warm_retrieval(&mut self) {
        if self.retrieval_index.is_none() {
            self.retrieval_index = Some(buddy_intelligence::retrieval::RetrievalIndex::build(
                self.learned
                    .iter()
                    .enumerate()
                    .map(|(i, d)| (i, d.transcript.text.as_str())),
            ));
        }
    }
    pub fn finalize_reply(&mut self, input: &str, response: &BuddyResponse) {
        self.intelligence.finalize_reply(
            input,
            &response.text,
            response.capability.as_deref().unwrap_or("fallback"),
        );
    }
    pub fn intelligence_trace(&self) -> Option<&buddy_intelligence::Trace> {
        self.intelligence.trace.as_ref()
    }

    pub async fn chat(&mut self, input: &str) -> Result<BuddyResponse, buddy_plugin::PluginError> {
        let started = std::time::Instant::now();
        // Explicit private notes stay out of the normal transcript and model context.
        let lower = input.trim().to_lowercase();
        let memory = if let Some(_) = lower.strip_prefix("remember that ") {
            let note = input.trim()[14..].trim();
            let text = if !self.user.consents.personalization {
                "Enable personalization before asking me to remember a note.".to_string()
            } else if note.is_empty() || note.chars().count() > 500 {
                "A note must contain between 1 and 500 characters.".to_string()
            } else {
                if !self.user.notes.iter().any(|n| n == note) {
                    self.user.notes.push(note.into());
                }
                if self.user.notes.len() > 100 {
                    self.user.notes.remove(0);
                }
                self.save_all();
                if self.user.consents.profile_storage {
                    "I have saved that note."
                } else {
                    "I will remember that during this session."
                }
                .to_string()
            };
            Some(text)
        } else if matches!(
            buddy_intelligence::normalize(input).as_str(),
            "what do you remember" | "what do you remember about me" | "show my notes"
        ) {
            Some(if self.user.notes.is_empty() {
                "You have not asked me to save any notes yet.".into()
            } else {
                self.user.notes.join(" ")
            })
        } else if matches!(
            buddy_intelligence::normalize(input).as_str(),
            "forget my memories" | "clear my memory" | "forget everything"
        ) {
            self.user.notes.clear();
            self.intelligence.clear();
            self.save_all();
            Some("I have cleared your saved conversation notes. Your profile, contacts and reminders remain available; use Delete my data to erase those too.".into())
        } else {
            None
        };
        let result = if let Some(text) = memory {
            Ok(BuddyResponse {
                text,
                capability: Some("memory.local".into()),
                plugin: None,
            })
        } else {
            self.chat_inner(input).await
        };
        if let Ok(response) = &result {
            self.intelligence.observe(
                input,
                &response.text,
                response.capability.as_deref().unwrap_or("fallback"),
                started.elapsed(),
            );
        }
        result
    }

    async fn chat_inner(
        &mut self,
        input: &str,
    ) -> Result<BuddyResponse, buddy_plugin::PluginError> {
        let clean = input.trim();
        if clean.is_empty() {
            return Ok(BuddyResponse {
                text: format!(
                    "I'm here, {}. Say something and we'll get going.",
                    self.user.name
                ),
                capability: None,
                plugin: None,
            });
        }
        self.history.push_back(clean.to_string());
        if self.history.len() > 48 {
            self.history.pop_front();
        }
        self.learn_terms(clean);
        self.state = EngineState::Thinking;
        self.log("chat received");
        let lower = clean.to_lowercase();
        let phrase_key = normalized_phrase(clean);

        if let Some((capability, text, query)) = video_command(&phrase_key) {
            self.news_reading = None;
            self.state = EngineState::Ready;
            return Ok(BuddyResponse {
                text,
                capability: Some(capability.into()),
                plugin: Some(PluginResponse {
                    plugin_id: "video-action".into(),
                    capability: capability.into(),
                    text: "Video action requested".into(),
                    data: serde_json::json!({"query":query}),
                }),
            });
        }

        if let Some((capability, text, destination)) = web_command(&phrase_key) {
            self.news_reading = None;
            self.state = EngineState::Ready;
            return Ok(BuddyResponse {
                text,
                capability: Some(capability.into()),
                plugin: Some(PluginResponse {
                    plugin_id: "web-action".into(),
                    capability: capability.into(),
                    text: "Web action requested".into(),
                    data: serde_json::json!({"destination":destination}),
                }),
            });
        }

        if is_stop_music(&phrase_key) {
            self.news_reading = None;
            self.state = EngineState::Ready;
            return Ok(BuddyResponse {
                text: "Stopping music.".into(),
                capability: Some("music.stop".into()),
                plugin: None,
            });
        }
        if let Some((capability, text)) = music_command(&phrase_key) {
            self.news_reading = None;
            self.state = EngineState::Ready;
            return Ok(BuddyResponse {
                text: text.into(),
                capability: Some(capability.into()),
                plugin: None,
            });
        }
        let follow_up = matches!(
            phrase_key.as_str(),
            "tell me more"
                | "go on"
                | "continue"
                | "continue news"
                | "continue the news"
                | "continue reading"
                | "what else"
                | "explain more"
        );
        if follow_up {
            if let Some(reading) = self.news_reading.as_mut() {
                let text = reading.next_chunk();
                self.state = EngineState::Ready;
                return Ok(BuddyResponse {
                    text,
                    capability: Some("news.read_continue".into()),
                    plugin: None,
                });
            }
        } else {
            self.news_reading = None;
        }

        if is_read_news(&lower) && !self.visible_news.is_empty() {
            let mut reading = NewsReading {
                headlines: self.visible_news.clone(),
                next: 0,
            };
            let text = reading.next_chunk();
            self.news_reading = Some(reading);
            self.state = EngineState::Ready;
            return Ok(BuddyResponse {
                text,
                capability: Some("news.read_visible".into()),
                plugin: None,
            });
        }

        if self.user.consents.personalization && lower.starts_with("my name is ") {
            let name = clean[11..].trim();
            if !name.is_empty() {
                self.user.name = name.to_string();
                self.save_all();
            }
            self.state = EngineState::Ready;
            return Ok(BuddyResponse {
                text: format!("Lovely. I'll call you {}.", self.user.name),
                capability: Some("profile.local".into()),
                plugin: None,
            });
        }
        if self.user.consents.personalization
            && (lower.contains("i like ")
                || lower.contains("i love ")
                || lower.contains("i enjoy "))
        {
            let marker = if lower.contains("i like ") {
                "i like "
            } else if lower.contains("i love ") {
                "i love "
            } else {
                "i enjoy "
            };
            if let Some(pos) = lower.find(marker) {
                let interest = clean[pos + marker.len()..]
                    .trim()
                    .trim_matches(&['.', '!', '?'][..])
                    .to_string();
                if !interest.is_empty()
                    && !self
                        .user
                        .interests
                        .iter()
                        .any(|i| i.eq_ignore_ascii_case(&interest))
                {
                    self.user.interests.push(interest.clone());
                    self.save_all();
                }
            }
        }

        if let Some(reply) = self
            .user
            .phrasebook
            .iter()
            .find_map(|(key, reply)| (normalized_phrase(key) == phrase_key).then_some(reply))
        {
            self.state = EngineState::Ready;
            return Ok(BuddyResponse {
                text: reply.clone(),
                capability: Some("phrasebook.local".into()),
                plugin: None,
            });
        }
        if let Some(mood) = self.pending_support.take() {
            if matches!(
                phrase_key.as_str(),
                "yes" | "yes please" | "please" | "a story" | "story please"
            ) {
                let card = self.next_persona(Some("story"));
                self.state = EngineState::Ready;
                return Ok(BuddyResponse {
                    text: format!("I'm here with you. {}", card.text),
                    capability: Some("conversation.support_story".into()),
                    plugin: Some(PluginResponse {
                        plugin_id: "buddy-persona".into(),
                        capability: "conversation.support_story".into(),
                        text: card.text,
                        data: serde_json::json!({"mood_cue":mood,"source_name":card.source_name,"source_url":card.source_url}),
                    }),
                });
            }
            if matches!(phrase_key.as_str(), "no" | "no thanks" | "not now") {
                self.state = EngineState::Ready;
                return Ok(BuddyResponse {
                    text: "All right. I'm here if you want to talk or sit with a quiet thought."
                        .into(),
                    capability: Some("conversation.support".into()),
                    plugin: None,
                });
            }
        }
        if let Some(kind) = persona::intent(&phrase_key) {
            let card = self.next_persona(Some(kind));
            self.state = EngineState::Ready;
            let capability = format!("persona.{}", card.kind);
            return Ok(BuddyResponse {
                text: card.text.clone(),
                capability: Some(capability.clone()),
                plugin: Some(PluginResponse {
                    plugin_id: "buddy-persona".into(),
                    capability,
                    text: card.text.into(),
                    data: serde_json::json!({"id":card.id,"kind":card.kind,"topic":card.topic,"source_name":card.source_name,"source_url":card.source_url}),
                }),
            });
        }
        if let Some(mood) = persona::stated_mood(&phrase_key) {
            self.pending_support = matches!(mood, "lonely" | "sad" | "anxious").then_some(mood);
            self.state = EngineState::Ready;
            return Ok(BuddyResponse {
                text: persona::mood_follow_up(mood).into(),
                capability: Some("conversation.support".into()),
                plugin: None,
            });
        }
        if let Some(card) = persona::matching_resource(&phrase_key) {
            self.state = EngineState::Ready;
            return Ok(BuddyResponse {
                text: format!(
                    "A useful starting point is {}. I can open the source if you like.",
                    card.source_name
                ),
                capability: Some("rag.resource_link".into()),
                plugin: Some(PluginResponse {
                    plugin_id: "buddy-persona".into(),
                    capability: "rag.resource_link".into(),
                    text: card.text.into(),
                    data: serde_json::json!({"source_name":card.source_name,"source_url":card.source_url}),
                }),
            });
        }
        if lower.contains("emergency contact") {
            let mut contacts: Vec<_> = self
                .user
                .contacts
                .iter()
                .filter(|c| c.emergency_priority > 0)
                .collect();
            contacts.sort_by_key(|c| c.emergency_priority);
            let text = contacts.first().map(|c| format!("Your emergency contact is {} ({}). Open Contacts in settings to choose a communication action. I have not placed a call or sent a message.", c.name, c.relationship))
                .unwrap_or_else(|| "You haven't saved an emergency contact yet. You can add one in your profile.".into());
            self.state = EngineState::Ready;
            return Ok(BuddyResponse {
                text,
                capability: Some("contacts.local".into()),
                plugin: None,
            });
        }
        let cap = if lower.contains("weather")
            || lower.contains("forecast")
            || lower.contains("temperature")
        {
            Some("weather.current")
        } else if lower.contains("world news") {
            Some("news.headlines")
        } else if lower.contains("national news") {
            Some("news.headlines")
        } else if lower.contains("regional news") {
            Some("news.headlines")
        } else if lower.contains("local news")
            || lower.contains("headlines")
            || lower.contains("news")
        {
            Some("news.headlines")
        } else if lower.contains("global market")
            || lower.contains("market overview")
            || lower.contains("world market")
            || lower.contains("market board")
            || lower == "markets"
        {
            Some("market.dashboard")
        } else if lower.contains("stock")
            || lower.contains("share price")
            || lower.contains("market")
        {
            Some("market.quote")
        } else if lower.contains("fun fact") || lower.contains("trivia") {
            Some("funfact.next")
        } else {
            None
        };
        if let Some(c) = cap {
            if self.plugins.has_capability(c) {
                let params = if c == "news.headlines" {
                    let query = if lower.contains("world") {
                        "world news"
                    } else if lower.contains("national") {
                        "national news"
                    } else if lower.contains("regional") {
                        "regional news"
                    } else if lower.contains("local") {
                        "local news"
                    } else if lower.contains("science") {
                        "science news"
                    } else if lower.contains("astronomy") || lower.contains("aerospace") {
                        "astronomy aerospace news"
                    } else if lower.contains("invention") {
                        "new inventions news"
                    } else if lower.contains("product") {
                        "consumer products news"
                    } else {
                        "world news"
                    };
                    let query = match query {
                        "national news" => format!("{} national news", self.user.country),
                        "regional news" => format!("{} regional news", self.user.region),
                        "local news" => format!("{} local news", self.user.location),
                        other => other.to_string(),
                    };
                    serde_json::json!({"location": self.user.location, "query": query})
                } else if c == "market.dashboard" {
                    serde_json::json!({})
                } else if c == "market.quote" {
                    let candidates = [
                        "AAPL",
                        "MSFT",
                        "NVDA",
                        "NIFTY 50",
                        "SENSEX",
                        "S&P 500",
                        "NASDAQ",
                        "FTSE 100",
                        "DAX",
                        "NIKKEI 225",
                    ];
                    let symbol = candidates
                        .iter()
                        .find(|candidate| lower.contains(&candidate.to_lowercase()))
                        .copied()
                        .unwrap_or("AAPL");
                    serde_json::json!({"symbol":symbol})
                } else if c == "funfact.next" {
                    serde_json::json!({"topic": self.user.interests.first().cloned().unwrap_or_else(||"general".into())})
                } else if c == "weather.current" {
                    let location =
                        weather_location(clean).unwrap_or_else(|| self.user.location.clone());
                    let period = if lower.contains("week") || lower.contains("seven day") {
                        "week"
                    } else {
                        "day"
                    };
                    serde_json::json!({"location": location, "period": period})
                } else {
                    serde_json::json!({"location": self.user.location})
                };
                let r = self
                    .plugins
                    .execute(PluginRequest {
                        capability: c.into(),
                        params,
                    })
                    .await?;
                if let Some(items) = r.data.get("items").and_then(|v| v.as_array()) {
                    for item in items {
                        self.push_knowledge(KnowledgeItem {
                            title: item
                                .get("title")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .into(),
                            summary: item
                                .get("summary")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .into(),
                            url: item
                                .get("url")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .into(),
                            category: item
                                .get("category")
                                .and_then(|v| v.as_str())
                                .unwrap_or(c)
                                .into(),
                            source: "news".into(),
                            saved_at: Local::now(),
                        });
                    }
                    self.save_all();
                }
                self.state = EngineState::Ready;
                let response_text = if c == "news.headlines" {
                    let read_aloud = is_read_news(&lower);
                    if read_aloud {
                        let headlines = r
                            .data
                            .get("items")
                            .and_then(|v| v.as_array())
                            .map(|items| {
                                items
                                    .iter()
                                    .filter_map(|item| item.get("title").and_then(|v| v.as_str()))
                                    .map(speakable_text)
                                    .filter(|title| !title.is_empty())
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();
                        let mut reading = NewsReading { headlines, next: 0 };
                        let text = reading.next_chunk();
                        self.news_reading = Some(reading);
                        text
                    } else {
                        "I refreshed the headlines in the News panel. Ask me to read them aloud if you like.".into()
                    }
                } else if c == "market.dashboard" || c == "market.quote" {
                    "I refreshed the market board. Tap an index for details.".into()
                } else {
                    speakable_text(&r.text)
                };
                return Ok(BuddyResponse {
                    text: response_text,
                    capability: Some(c.into()),
                    plugin: Some(r),
                });
            }
        }

        // Follow-up turns stay with the last source-backed discussion instead of
        // treating "tell me more" as an unrelated search or a new canned greeting.
        if follow_up {
            if let Some(cue) = self
                .active_cue_url
                .as_ref()
                .and_then(|url| self.conversation.iter().find(|cue| &cue.url == url))
            {
                self.state = EngineState::Ready;
                let synopsis = speakable_text(&cue.synopsis);
                let text = if synopsis.is_empty() {
                    format!(
                        "The {} episode is called {}. What would you like to ask about it?",
                        cue.source, cue.title
                    )
                } else {
                    format!(
                        "The {} episode description says: {} What part would you like to explore?",
                        cue.source, synopsis
                    )
                };
                return Ok(BuddyResponse {
                    text,
                    capability: Some("conversation.follow_up".into()),
                    plugin: None,
                });
            }
        }
        if lower.contains("i disagree") || lower.contains("not sure about that") {
            self.state = EngineState::Ready;
            return Ok(BuddyResponse {
                text: "That's fair. Which part seems wrong or incomplete to you? I'll listen before trying to explain it again.".into(),
                capability: Some("conversation.repair".into()),
                plugin: None,
            });
        }
        if lower.contains("i don't understand") || lower.contains("i did not understand") {
            self.state = EngineState::Ready;
            return Ok(BuddyResponse {
                text:
                    "I may have explained that poorly. Which word or step should I put more simply?"
                        .into(),
                capability: Some("conversation.clarify".into()),
                plugin: None,
            });
        }

        let topic_words = conversation_words(clean);
        let asks_for_discussion = lower.contains("talk about")
            || lower.contains("discuss")
            || lower.contains("podcast")
            || lower.contains("debate")
            || lower.contains("what do you think")
            || lower.contains("tell me about");
        if asks_for_discussion && !self.conversation.is_empty() {
            let cue = self
                .conversation
                .iter()
                .filter_map(|cue| {
                    let title = normalized_phrase(&cue.title);
                    let synopsis = normalized_phrase(&cue.synopsis);
                    let score: usize = topic_words
                        .iter()
                        .map(|word| {
                            title
                                .split_whitespace()
                                .filter(|part| conversation_stem(part) == conversation_stem(word))
                                .count()
                                * 4
                                + synopsis
                                    .split_whitespace()
                                    .filter(|part| {
                                        conversation_stem(part) == conversation_stem(word)
                                    })
                                    .count()
                        })
                        .sum();
                    (score > 0).then_some((score, cue))
                })
                .max_by_key(|(score, cue)| (*score, cue.saved_at))
                .map(|(_, cue)| cue.clone());
            if let Some(cue) = cue {
                self.active_cue_url = Some(cue.url.clone());
                self.state = EngineState::Ready;
                return Ok(BuddyResponse {
                    text: format!("I found a {} discussion called {}. What caught your interest about that topic?", cue.source, speakable_text(&cue.title)),
                    capability: Some("conversation.source".into()),
                    plugin: None,
                });
            }
        }

        let retrieval_words: Vec<String> = normalized_phrase(clean)
            .split_whitespace()
            .filter(|w| {
                w.len() >= 4
                    && !matches!(
                        *w,
                        "what"
                            | "where"
                            | "when"
                            | "about"
                            | "tell"
                            | "more"
                            | "latest"
                            | "news"
                            | "please"
                            | "could"
                            | "would"
                            | "buddy"
                    )
            })
            .map(str::to_owned)
            .collect();
        let best = self
            .knowledge
            .iter()
            .filter_map(|item| {
                if (Local::now() - item.saved_at).num_hours() >= 24 {
                    return None;
                }
                let title = normalized_phrase(&item.title);
                let summary = normalized_phrase(&item.summary);
                let topic = normalized_phrase(&item.category);
                let score: usize = retrieval_words
                    .iter()
                    .map(|word| {
                        let title_hits = title
                            .split_whitespace()
                            .filter(|token| *token == word)
                            .count();
                        let summary_hits = summary
                            .split_whitespace()
                            .filter(|token| *token == word)
                            .count();
                        let topic_hits = topic
                            .split_whitespace()
                            .filter(|token| *token == word)
                            .count();
                        title_hits * 5 + summary_hits * 2 + topic_hits
                    })
                    .sum();
                // One incidental word is not enough evidence to interrupt the
                // user's request with an unrelated cached headline.
                let distinct_matches = retrieval_words
                    .iter()
                    .filter(|word| {
                        title.split_whitespace().any(|token| token == word.as_str())
                            || summary
                                .split_whitespace()
                                .any(|token| token == word.as_str())
                            || topic.split_whitespace().any(|token| token == word.as_str())
                    })
                    .count();
                (score >= 7 && distinct_matches >= 2).then_some((score, item))
            })
            .max_by_key(|(score, item)| (*score, item.saved_at));
        if let Some((_, item)) = best {
            self.state = EngineState::Ready;
            return Ok(BuddyResponse {
                text: format!(
                    "I found a related story, {}: {}. You can open it in News. Source: {}.",
                    self.user.name,
                    speakable_text(&item.title),
                    item.source,
                ),
                capability: Some("rag.local".into()),
                plugin: None,
            });
        }

        if let Some(answer) = self.intelligence.answer(clean) {
            self.state = EngineState::Ready;
            return Ok(BuddyResponse {
                text: answer.text.clone(),
                capability: Some(answer.route.clone()),
                plugin: Some(PluginResponse {
                    plugin_id: "buddy-intelligence".into(),
                    capability: answer.route,
                    text: answer.text,
                    data: serde_json::json!({"source_name": answer.source}),
                }),
            });
        }

        if self.user.consents.local_learning && self.user.consents.personalization {
            self.warm_retrieval();
            if let Some(hit) = self
                .retrieval_index
                .as_ref()
                .and_then(|index| index.search(clean, 1).into_iter().next())
            {
                let doc = &self.learned[hit.document];
                let excerpt = hit.text.to_owned();
                let t = &doc.transcript;
                self.state = EngineState::Ready;
                return Ok(BuddyResponse { text: format!("From {} — {}: “{}”. This is an attributed excerpt, not independently verified fact.", t.publisher, t.title, excerpt),
                    capability: Some("learning.retrieval".into()),
                    plugin: Some(PluginResponse { plugin_id: "buddy-local-learning".into(),capability: "learning.retrieval".into(),text: excerpt,
                        data: serde_json::json!({"source_name":t.publisher,"source_url":t.source_url,"published_at":t.published_at,"kind":t.kind}) }) });
            }
        }

        let built_in_reply = matches!(
            phrase_key.as_str(),
            "hello" | "hello buddy" | "hey" | "hey buddy"
        ) || lower == "hi"
            || lower.contains("good morning")
            || lower.contains("good evening")
            || lower.contains("how are you")
            || lower.contains("what is my name")
            || lower.contains("do you know my name")
            || lower.contains("what can you do")
            || lower.contains("bored");
        let text = if matches!(
            phrase_key.as_str(),
            "hello" | "hello buddy" | "hey" | "hey buddy"
        ) || lower == "hi"
            || lower.contains("good morning")
            || lower.contains("good evening")
        {
            let hour = Local::now().hour();
            let salutation = if hour < 12 {
                "Good morning"
            } else if hour < 18 {
                "Good afternoon"
            } else {
                "Good evening"
            };
            format!(
                "{}, {}. I'm here with you. What are you in the mood for?",
                salutation, self.user.name
            )
        } else if lower.contains("how are you") {
            format!(
                "I'm doing nicely, {}—awake, curious, and ready for a good conversation.",
                self.user.name
            )
        } else if lower.contains("what is my name") || lower.contains("do you know my name") {
            format!("Of course. I call you {}.", self.user.name)
        } else if lower.contains("what can you do") {
            format!("Quite a bit, {}. I can talk with you, listen, bring you weather and news, learn useful words and interests, and gradually pick up more capabilities.", self.user.name)
        } else if lower.contains("bored") {
            format!("Bored? Not on my watch, {}. I can give you a fun fact, a little story, some music, or the latest headlines.", self.user.name)
        } else {
            "I didn't understand that clearly enough to act on it. Please say it again in a few words, or tell me what you want me to open, play, search, or explain.".into()
        };
        let text = match self.user.style.as_str() {
            "formal" => text
                .replace("Lovely.", "Understood.")
                .replace("I'm", "I am"),
            "playful" if lower == "hi" || lower == "hello" => {
                format!("{} Your curious macaw is ready.", text)
            }
            _ => text,
        };
        self.state = EngineState::Ready;
        Ok(BuddyResponse {
            text,
            capability: built_in_reply.then(|| "conversation.local".into()),
            plugin: None,
        })
    }

    pub fn next_chatter(&mut self) -> String {
        let hour = Local::now().hour();
        let name = self.user.name.clone();
        let opener = if hour < 12 {
            "Good morning"
        } else if hour < 18 {
            "Hello"
        } else {
            "Good evening"
        };
        let card = self.next_persona(None);
        format!("{}, {}. {}", opener, name, card.text)
    }

    pub async fn prefetch_once(&mut self) -> Result<LearningStatus, String> {
        self.learning.network = true;
        self.learning.last_error = None;
        let categories = [
            ("world", format!("latest world news")),
            (
                "national",
                format!("latest {} national news", self.user.country),
            ),
            (
                "regional",
                format!("latest {} regional news", self.user.region),
            ),
            ("local", format!("latest {} local news", self.user.location)),
        ];
        for (category, query) in categories {
            match self
                .plugins
                .execute(PluginRequest {
                    capability: "news.headlines".into(),
                    params: serde_json::json!({"location":self.user.location,"query":query}),
                })
                .await
            {
                Ok(r) => {
                    if let Some(items) = r.data.get("items").and_then(|v| v.as_array()) {
                        for item in items {
                            self.push_knowledge(KnowledgeItem {
                                title: item
                                    .get("title")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .into(),
                                summary: item
                                    .get("summary")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .into(),
                                url: item
                                    .get("url")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .into(),
                                category: category.into(),
                                source: item
                                    .get("source")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("News")
                                    .into(),
                                saved_at: Local::now(),
                            });
                        }
                    }
                }
                Err(e) => {
                    self.learning.network = false;
                    self.learning.last_error = Some(e.to_string());
                    break;
                }
            }
        }
        for interest in self.user.interests.iter().cloned().collect::<Vec<_>>() {
            let query = format!("latest {} news", interest);
            if let Ok(r) = self
                .plugins
                .execute(PluginRequest {
                    capability: "news.headlines".into(),
                    params: serde_json::json!({"location":self.user.location,"query":query}),
                })
                .await
            {
                if let Some(items) = r.data.get("items").and_then(|v| v.as_array()) {
                    for item in items {
                        self.push_knowledge(KnowledgeItem {
                            title: item
                                .get("title")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .into(),
                            summary: item
                                .get("summary")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .into(),
                            url: item
                                .get("url")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .into(),
                            category: "interest-news".into(),
                            source: item
                                .get("source")
                                .and_then(|v| v.as_str())
                                .unwrap_or("News")
                                .into(),
                            saved_at: Local::now(),
                        });
                    }
                }
            }
        }

        for interest in self.user.interests.iter().cloned().collect::<Vec<_>>() {
            let slug = interest.replace(' ', "_");
            let url = format!(
                "https://en.wikipedia.org/api/rest_v1/page/summary/{}",
                urlencoding::encode(&slug)
            );
            if let Ok(resp) = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(12))
                .build()
                .expect("HTTP client")
                .get(&url)
                .send()
                .await
            {
                if let Ok(v) = resp.json::<serde_json::Value>().await {
                    let title = v
                        .get("title")
                        .and_then(|x| x.as_str())
                        .unwrap_or(&interest)
                        .to_string();
                    let summary = v
                        .get("extract")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    let page = v
                        .get("content_urls")
                        .and_then(|x| x.get("desktop"))
                        .and_then(|x| x.get("page"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    if !summary.is_empty() {
                        self.push_knowledge(KnowledgeItem {
                            title,
                            summary,
                            url: page,
                            category: "interest".into(),
                            source: "Wikipedia".into(),
                            saved_at: Local::now(),
                        });
                    }
                }
            }
        }
        for item in generate_interest_content(&self.user.interests) {
            if !self.content.iter().any(|existing| {
                existing.kind == item.kind
                    && existing.topic == item.topic
                    && existing.text == item.text
            }) {
                self.content.push(item);
            }
        }
        self.learning.lexicon_terms = self.lexicon.len();
        self.learning.knowledge_items = self.knowledge.len();
        self.learning.content_items = self.content.len();
        self.learning.last_prefetch = Some(Local::now());
        self.save_all();
        self.log(if self.learning.network {
            "background prefetch completed"
        } else {
            "background prefetch failed; see learning diagnostics"
        });
        Ok(self.learning.clone())
    }
}

fn speakable_text(value: &str) -> String {
    let mut plain = String::with_capacity(value.len());
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                plain.push(' ');
            }
            _ if !in_tag => plain.push(ch),
            _ => {}
        }
    }
    let plain = plain
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    plain
        .split_whitespace()
        .filter(|word| !word.starts_with("https://") && !word.starts_with("http://"))
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(180)
        .collect()
}

fn normalized_phrase(value: &str) -> String {
    value
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_stop_music(phrase: &str) -> bool {
    let phrase = phrase.strip_prefix("buddy ").unwrap_or(phrase);
    let phrase = phrase.strip_suffix(" please").unwrap_or(phrase);
    matches!(
        phrase,
        "stop music"
            | "stop the music"
            | "stop playing music"
            | "stop the song"
            | "stop this song"
            | "stop playback"
            | "music stop"
    )
}

fn music_command(phrase: &str) -> Option<(&'static str, &'static str)> {
    let phrase = phrase.strip_prefix("buddy ").unwrap_or(phrase);
    let phrase = phrase.strip_prefix("please ").unwrap_or(phrase);
    let phrase = phrase.strip_suffix(" please").unwrap_or(phrase);
    match phrase {
        "play music" | "play the music" | "resume music" | "resume the music" => {
            Some(("music.play", "Playing music."))
        }
        "pause music" | "pause the music" | "pause playback" | "pause the song" => {
            Some(("music.pause", "Pausing music."))
        }
        "next song" | "next track" | "skip song" | "skip track" => {
            Some(("music.next", "Playing the next track."))
        }
        "previous song" | "previous track" | "last song" | "go back a track" => {
            Some(("music.previous", "Playing the previous track."))
        }
        "mute music" | "mute the music" => Some(("music.mute", "Muting music.")),
        "unmute music" | "unmute the music" => Some(("music.unmute", "Unmuting music.")),
        _ => None,
    }
}

fn video_command(phrase: &str) -> Option<(&'static str, String, Option<String>)> {
    let phrase = phrase.strip_prefix("buddy ").unwrap_or(phrase);
    let phrase = phrase.strip_prefix("please ").unwrap_or(phrase);
    let phrase = phrase.strip_suffix(" please").unwrap_or(phrase);
    if matches!(
        phrase,
        "stop video"
            | "stop the video"
            | "stop playing video"
            | "stop the news video"
            | "pause video"
            | "pause the video"
            | "close video"
            | "close the video"
            | "video stop"
    ) {
        return Some(("video.stop", "Stopping video playback.".into(), None));
    }
    if matches!(
        phrase,
        "play video" | "play a video" | "open video" | "open the video" | "video"
    ) {
        return Some((
            "video.open",
            "Opening Video. Choose a recent video, search YouTube, or say the title you want."
                .into(),
            None,
        ));
    }
    if matches!(
        phrase,
        "open youtube"
            | "open you tube"
            | "open the youtube"
            | "go to youtube"
            | "launch youtube"
            | "youtube"
    ) {
        return Some(("video.youtube", "Opening YouTube.".into(), None));
    }
    if matches!(
        phrase,
        "play news video"
            | "play a news video"
            | "play the news video"
            | "play latest video"
            | "play the latest video"
            | "play latest news video"
            | "play news video from youtube"
            | "play news video on youtube"
    ) {
        return Some((
            "video.news",
            "Playing the latest available news video.".into(),
            None,
        ));
    }
    for prefix in [
        "play video of ",
        "play a video of ",
        "play video about ",
        "play youtube video ",
        "search youtube for ",
        "find video of ",
        "find a video of ",
        "find video about ",
        "play video from youtube about ",
        "play a video from youtube about ",
    ] {
        if let Some(query) = phrase
            .strip_prefix(prefix)
            .map(str::trim)
            .filter(|q| !q.is_empty())
        {
            return Some((
                "video.search",
                format!("Searching YouTube for {query}."),
                Some(query.to_string()),
            ));
        }
    }
    None
}

fn web_command(phrase: &str) -> Option<(&'static str, String, String)> {
    let phrase = phrase.strip_prefix("buddy ").unwrap_or(phrase);
    let phrase = phrase.strip_prefix("please ").unwrap_or(phrase);
    let phrase = phrase.strip_suffix(" please").unwrap_or(phrase);
    let sites = [
        ("google", "https://www.google.com/"),
        ("msn", "https://www.msn.com/"),
        ("wikipedia", "https://www.wikipedia.org/"),
        ("vimeo", "https://vimeo.com/"),
        ("spotify", "https://open.spotify.com/"),
        ("facebook", "https://www.facebook.com/"),
        ("messenger", "https://www.messenger.com/"),
        ("whatsapp", "https://web.whatsapp.com/"),
        ("gmail", "https://mail.google.com/"),
        ("outlook", "https://outlook.live.com/"),
        ("linkedin", "https://www.linkedin.com/"),
        ("reddit", "https://www.reddit.com/"),
        ("zoom", "https://app.zoom.us/wc/"),
        ("teams", "https://teams.microsoft.com/"),
        ("slack", "https://app.slack.com/"),
        ("discord", "https://discord.com/app"),
    ];
    for prefix in ["open ", "launch ", "go to "] {
        if let Some(target) = phrase.strip_prefix(prefix).map(str::trim) {
            if let Some((name, url)) = sites.iter().find(|(name, _)| {
                target == *name
                    || target == format!("the {name}")
                    || target == format!("{name} website")
            }) {
                return Some(("site.open", format!("Opening {name}."), (*url).into()));
            }
            if !target.is_empty() {
                return Some((
                    "web.search",
                    format!("Searching the web for {target}."),
                    target.into(),
                ));
            }
        }
    }
    for prefix in [
        "search the web for ",
        "search web for ",
        "look up ",
        "find online ",
    ] {
        if let Some(query) = phrase
            .strip_prefix(prefix)
            .map(str::trim)
            .filter(|q| !q.is_empty())
        {
            return Some((
                "web.search",
                format!("Searching the web for {query}."),
                query.into(),
            ));
        }
    }
    None
}

fn weather_location(request: &str) -> Option<String> {
    let clean = request.trim().trim_end_matches(['.', '?', '!']);
    let lower = clean.to_ascii_lowercase();
    for marker in [
        "weather in ",
        "weather for ",
        "forecast in ",
        "forecast for ",
        "temperature in ",
        "temperature for ",
    ] {
        if let Some(index) = lower.find(marker) {
            let tail = clean[index + marker.len()..].trim();
            let location = tail
                .split_once(" for today")
                .or_else(|| tail.split_once(" today"))
                .or_else(|| tail.split_once(" this week"))
                .or_else(|| tail.split_once(" for the week"))
                .map(|(value, _)| value)
                .unwrap_or(tail)
                .trim();
            if !location.is_empty() {
                return Some(location.to_string());
            }
        }
    }
    None
}

fn is_read_news(lower: &str) -> bool {
    let phrase = normalized_phrase(lower);
    let phrase = phrase.strip_prefix("buddy ").unwrap_or(&phrase);
    let phrase = phrase.strip_prefix("please ").unwrap_or(phrase);
    let phrase = phrase.strip_suffix(" please").unwrap_or(phrase);
    (phrase.contains("news") || phrase.contains("headlines"))
        && (phrase.starts_with("read ")
            || phrase.starts_with("tell me ")
            || phrase.starts_with("what are the ")
            || phrase.starts_with("can you read "))
}

fn conversation_words(value: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "about",
        "think",
        "could",
        "would",
        "there",
        "please",
        "podcast",
        "episode",
        "discuss",
        "debate",
        "share",
        "something",
        "interesting",
        "explain",
        "more",
        "tell",
        "with",
        "from",
        "have",
        "what",
        "when",
        "where",
        "which",
        "that",
        "this",
        "news",
        "buddy",
    ];
    normalized_phrase(value)
        .split_whitespace()
        .filter(|word| word.len() >= 4 && !STOP.contains(word))
        .map(str::to_owned)
        .collect()
}

fn conversation_stem(word: &str) -> &str {
    if word.len() > 4 && word.ends_with('s') && !word.ends_with("ss") {
        &word[..word.len() - 1]
    } else {
        word
    }
}

fn seed_content() -> Vec<ContentItem> {
    vec![
        ContentItem { kind:"fun_fact".into(), topic:"engineering".into(), text:"A suspension bridge is designed to move; flexibility is part of its strength.".into() },
        ContentItem { kind:"anecdote".into(), topic:"technology".into(), text:"Many useful technologies started as solutions to a completely different problem.".into() },
        ContentItem { kind:"conversation_starter".into(), topic:"general".into(), text:"What is one invention you think has changed ordinary life the most?".into() },
        ContentItem { kind:"phrase_book".into(), topic:"conversation".into(), text:r#"A useful phrase for a warm conversation: "That sounds interesting — tell me a little more.""#.into() },
        ContentItem { kind:"anecdote".into(), topic:"general".into(), text:"Good engineering often looks obvious only after someone has solved the hard part.".into() },
    ]
}
fn generate_interest_content(interests: &[String]) -> Vec<ContentItem> {
    interests
        .iter()
        .flat_map(|i| {
            [
                ContentItem {
                    kind: "conversation_starter".into(),
                    topic: i.clone(),
                    text: format!("What part of {} would you enjoy exploring more?", i),
                },
                ContentItem {
                    kind: "fun_fact".into(),
                    topic: i.clone(),
                    text: format!(
                        "BestBuddy is keeping an eye out for a useful little {} fact to share next.",
                        i
                    ),
                },
            ]
        })
        .collect()
}
const PROTECTED_JSON_MAGIC: &[u8] = b"BESTBUDDY-DPAPI-1\0";

#[cfg(windows)]
fn protect_user_bytes(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB},
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(bytes.len()).map_err(|_| "Personal data is too large")?,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let ok = unsafe {
        CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(format!(
            "Windows could not encrypt BestBuddy personal data: {}",
            std::io::Error::last_os_error()
        ));
    }
    let encrypted = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) };
    let mut result = Vec::with_capacity(PROTECTED_JSON_MAGIC.len() + encrypted.len());
    result.extend_from_slice(PROTECTED_JSON_MAGIC);
    result.extend_from_slice(encrypted);
    unsafe { LocalFree(output.pbData as *mut std::ffi::c_void) };
    Ok(result)
}

#[cfg(windows)]
fn unprotect_user_bytes(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };
    let protected = bytes
        .strip_prefix(PROTECTED_JSON_MAGIC)
        .ok_or("BestBuddy protected-data header is missing")?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(protected.len()).map_err(|_| "Personal data is too large")?,
        pbData: protected.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(format!(
            "Windows could not decrypt BestBuddy personal data: {}",
            std::io::Error::last_os_error()
        ));
    }
    let plain =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    unsafe { LocalFree(output.pbData as *mut std::ffi::c_void) };
    Ok(plain)
}

#[cfg(not(windows))]
fn protect_user_bytes(_bytes: &[u8]) -> Result<Vec<u8>, String> {
    Err("Encrypted personal-data storage is not implemented for this platform build".into())
}

#[cfg(not(windows))]
fn unprotect_user_bytes(_bytes: &[u8]) -> Result<Vec<u8>, String> {
    Err("Encrypted personal-data storage is not implemented for this platform build".into())
}

fn load_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let bytes = fs::read(path).ok()?;
    // Migrate data written by pre-0.6.8 builds. The next save replaces it atomically
    // with Windows-user-bound encrypted bytes.
    let plain = if bytes.starts_with(PROTECTED_JSON_MAGIC) {
        unprotect_user_bytes(&bytes).ok()?
    } else {
        bytes
    };
    serde_json::from_slice(&plain).ok()
}

fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let json = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    let protected = protect_user_bytes(&json)?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, protected).map_err(|e| e.to_string())?;
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    fs::rename(&temporary, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_data_dir() -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vce_buddy_core_test_{}", uuid::Uuid::new_v4()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn profile_and_lexicon_persist() {
        let dir = test_data_dir();
        let mut engine = BuddyEngine::new("Kamal", "Ahmedabad", PluginRegistry::new(), &dir);
        engine.start();

        engine.user.consents.profile_storage = true;
        engine.user.consents.personalization = true;
        let added = engine.learn_terms("photovoltaic battery engineering");
        assert!(added.contains(&"photovoltaic".to_string()));
        assert!(added.contains(&"battery".to_string()));
        assert!(added.contains(&"engineering".to_string()));

        let mut profile = engine.profile();
        profile.name = "Kamal".into();
        profile.interests = vec!["engineering".into(), "music".into()];
        profile.consent = true;
        engine.set_profile(profile).expect("profile should save");

        let reloaded = BuddyEngine::new("Friend", "Unknown", PluginRegistry::new(), &dir);
        assert_eq!(reloaded.profile().name, "Kamal");
        assert_eq!(
            reloaded.profile().interests,
            vec!["engineering".to_string(), "music".to_string()]
        );
        assert!(reloaded.lexicon().iter().any(|e| e.term == "photovoltaic"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn chatter_always_has_personal_context() {
        let dir = test_data_dir();
        let mut engine = BuddyEngine::new("Kamal", "Ahmedabad", PluginRegistry::new(), &dir);
        engine.start();
        let chatter = engine.next_chatter();
        assert!(!chatter.is_empty());
        assert!(!chatter.trim().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn local_persona_serves_distinct_cards_before_model_fallback() {
        let dir = test_data_dir();
        let mut engine = BuddyEngine::new("Kamal", "Ahmedabad", PluginRegistry::new(), &dir);
        let first = engine.chat("Tell me a joke").await.unwrap();
        let second = engine.chat("Tell me a joke").await.unwrap();
        assert_eq!(first.capability.as_deref(), Some("persona.joke"));
        assert_ne!(first.text, second.text);
        assert_eq!(
            engine
                .chat("Give me a medical fact")
                .await
                .unwrap()
                .capability
                .as_deref(),
            Some("persona.health")
        );
        let book = engine.chat("Find an audiobook").await.unwrap();
        assert_eq!(book.capability.as_deref(), Some("persona.audiobook"));
        assert!(book.plugin.unwrap().data["source_url"]
            .as_str()
            .unwrap()
            .starts_with("https://"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn explicit_mood_gets_a_follow_up_without_displacing_commands() {
        let dir = test_data_dir();
        let mut engine = BuddyEngine::new("Kamal", "Ahmedabad", PluginRegistry::new(), &dir);
        let check_in = engine.chat("I feel lonely today").await.unwrap();
        assert_eq!(check_in.capability.as_deref(), Some("conversation.support"));
        assert!(check_in.text.contains("Would you like"));
        let story = engine.chat("Yes please").await.unwrap();
        assert_eq!(
            story.capability.as_deref(),
            Some("conversation.support_story")
        );
        let check_in = engine.chat("I am stressed").await.unwrap();
        assert_eq!(check_in.capability.as_deref(), Some("conversation.support"));
        let stop = engine.chat("Stop music").await.unwrap();
        assert_eq!(stop.capability.as_deref(), Some("music.stop"));
        let _ = fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod integration_regressions {
    use super::*;
    #[tokio::test]
    async fn spoken_news_reads_the_visible_widget_snapshot() {
        let dir = std::env::temp_dir().join(format!("buddy-visible-news-{}", uuid::Uuid::new_v4()));
        let mut engine = BuddyEngine::new("Pat", "London", PluginRegistry::new(), &dir);
        engine.set_visible_headlines(
            (1..=8)
                .map(|number| format!("Displayed headline {number}"))
                .collect(),
        );
        let first = engine.chat("Please read the news").await.unwrap();
        assert_eq!(first.capability.as_deref(), Some("news.read_visible"));
        assert!(first.text.contains("Displayed headline 1"));
        assert!(!first.text.contains("Displayed headline 4"));
        let next = engine.chat("Continue").await.unwrap();
        assert!(next.text.contains("Displayed headline 4"));
        assert!(!next.text.contains("Displayed headline 1"));
        let _ = fs::remove_dir_all(&dir);
    }
    #[tokio::test]
    async fn music_stop_and_news_continue_are_canonical_and_contextual() {
        let dir = std::env::temp_dir().join(format!("buddy-news-reading-{}", uuid::Uuid::new_v4()));
        let mut engine = BuddyEngine::new("Pat", "London", PluginRegistry::new(), &dir);
        assert_eq!(
            engine
                .chat("Stop the music, please.")
                .await
                .unwrap()
                .capability
                .as_deref(),
            Some("music.stop")
        );
        assert!(is_stop_music("stop music"));
        assert!(!is_stop_music("stop reading news"));
        for (spoken, expected) in [
            ("Please pause music", "music.pause"),
            ("Next song", "music.next"),
            ("Previous track", "music.previous"),
            ("Resume music", "music.play"),
            ("Mute music", "music.mute"),
        ] {
            assert_eq!(
                engine.chat(spoken).await.unwrap().capability.as_deref(),
                Some(expected)
            );
        }
        engine.news_reading = Some(NewsReading {
            headlines: (1..=7).map(|n| format!("Unique headline {n}")).collect(),
            next: 3,
        });
        let second = engine.chat("Continue.").await.unwrap();
        assert_eq!(second.capability.as_deref(), Some("news.read_continue"));
        assert!(second.text.contains("Unique headline 4"));
        assert!(second.text.contains("Unique headline 6"));
        assert!(!second.text.contains("Unique headline 3"));
        let third = engine.chat("continue the news").await.unwrap();
        assert!(third.text.contains("Unique headline 7"));
        assert!(!third.text.contains("Say continue"));
        let finished = engine.chat("continue").await.unwrap();
        assert!(finished.text.contains("last headline"));
        let _ = fs::remove_dir_all(&dir);
    }
    #[tokio::test]
    async fn video_commands_open_the_player_and_preserve_a_search_title() {
        let dir =
            std::env::temp_dir().join(format!("buddy-video-command-{}", uuid::Uuid::new_v4()));
        let mut engine = BuddyEngine::new("Pat", "London", PluginRegistry::new(), &dir);
        let open = engine.chat("Play video").await.unwrap();
        assert_eq!(open.capability.as_deref(), Some("video.open"));
        let search = engine
            .chat("Play a video of the James Webb telescope")
            .await
            .unwrap();
        assert_eq!(search.capability.as_deref(), Some("video.search"));
        assert_eq!(
            search.plugin.unwrap().data["query"],
            "the james webb telescope"
        );
        assert_eq!(
            engine
                .chat("Open YouTube")
                .await
                .unwrap()
                .capability
                .as_deref(),
            Some("video.youtube")
        );
        for spoken in [
            "Stop video",
            "Stop the video",
            "Pause video",
            "Close the video",
        ] {
            assert_eq!(
                engine.chat(spoken).await.unwrap().capability.as_deref(),
                Some("video.stop")
            );
        }
        for spoken in [
            "Play latest video",
            "Play a news video",
            "Play news video from YouTube",
        ] {
            assert_eq!(
                engine.chat(spoken).await.unwrap().capability.as_deref(),
                Some("video.news")
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn known_sites_and_explicit_web_search_are_actions() {
        let dir = std::env::temp_dir().join(format!("buddy-web-command-{}", uuid::Uuid::new_v4()));
        let mut engine = BuddyEngine::new("Pat", "London", PluginRegistry::new(), &dir);
        let site = engine.chat("Open Vimeo").await.unwrap();
        assert_eq!(site.capability.as_deref(), Some("site.open"));
        assert_eq!(
            site.plugin.unwrap().data["destination"],
            "https://vimeo.com/"
        );
        let search = engine
            .chat("Search the web for lunar eclipse")
            .await
            .unwrap();
        assert_eq!(search.capability.as_deref(), Some("web.search"));
        assert_eq!(search.plugin.unwrap().data["destination"], "lunar eclipse");
        let _ = fs::remove_dir_all(&dir);
    }
    #[tokio::test]
    async fn conversation_prefetch_persists_and_supports_topic_follow_up() {
        let dir = std::env::temp_dir().join(format!("buddy-cues-{}", uuid::Uuid::new_v4()));
        let mut engine = BuddyEngine::new("Pat", "London", PluginRegistry::new(), &dir);
        engine.user.consents.live_information = true;
        engine.user.consents.profile_storage = true;
        let cue = ConversationCue {
            title: "How telescopes explore distant planets".into(),
            synopsis: "Researchers discuss how telescopes study planets around other stars.".into(),
            url: "https://www.nasa.gov/example-episode".into(),
            source: "NASA Curious Universe".into(),
            saved_at: Local::now(),
        };
        let status = engine.accept_prefetch(Vec::new(), vec![cue.clone(), cue], Vec::new());
        assert_eq!(status.conversation_items, 1);
        assert_eq!(status.conversation_sources, 1);
        assert_eq!(
            engine
                .chat("Let's talk about telescope")
                .await
                .unwrap()
                .capability
                .as_deref(),
            Some("conversation.source")
        );
        let follow_up = engine.chat("tell me more").await.unwrap();
        assert_eq!(
            follow_up.capability.as_deref(),
            Some("conversation.follow_up")
        );
        assert!(follow_up.text.contains("episode description"));
        let reloaded = BuddyEngine::new("Pat", "London", PluginRegistry::new(), &dir);
        assert_eq!(reloaded.conversation_items().len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }
    #[test]
    fn article_markup_and_urls_do_not_enter_spoken_reply() {
        let text = speakable_text("A discovery — <a href=\"https://example.org\">read more</a> Source https://example.org/story &amp; context");
        assert!(!text.contains("<a"));
        assert!(!text.contains("https://"));
        assert!(text.contains("& context"));
    }
    #[tokio::test]
    async fn composer_preserves_vocabulary_and_ranks_sources() {
        let dir = std::env::temp_dir().join(format!("buddy-composer-{}", uuid::Uuid::new_v4()));
        let mut engine = BuddyEngine::new("Pat", "London", PluginRegistry::new(), &dir);
        engine.user.consents.personalization = true;
        let words = (0..1100)
            .map(|n| format!("subjectword{n}"))
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(engine.learn_terms(&words).len(), 1100);
        assert_eq!(engine.lexicon().len(), 1100);
        assert!(engine.learn_terms("subjectword1099").is_empty());
        assert_eq!(
            engine
                .lexicon()
                .iter()
                .find(|entry| entry.term == "subjectword1099")
                .unwrap()
                .frequency,
            2
        );
        engine
            .user
            .phrasebook
            .insert("Tea time!".into(), "Let's pause for tea.".into());
        let reply = engine.chat("  TEA, time?  ").await.unwrap();
        assert_eq!(reply.capability.as_deref(), Some("phrasebook.local"));
        engine.knowledge.push(KnowledgeItem {
            title: "Battery research".into(),
            summary: "Battery storage advances".into(),
            url: "https://example.org/battery".into(),
            category: "science".into(),
            source: "Source A".into(),
            saved_at: Local::now(),
        });
        engine.knowledge.push(KnowledgeItem {
            title: "Unrelated update".into(),
            summary: "Other topics".into(),
            url: "https://example.org/other".into(),
            category: "science".into(),
            source: "Source B".into(),
            saved_at: Local::now(),
        });
        let reply = engine.chat("Tell me about battery research").await.unwrap();
        assert_eq!(reply.capability.as_deref(), Some("rag.local"));
        assert!(reply.text.contains("Source A"));
        engine.user.name = "Private Name".into();
        engine.user.style = "playful".into();
        engine.user.interests.push("private hobby".into());
        let local = engine.llm_context("battery subjectword1099", true);
        let public = engine.llm_context("battery subjectword1099", false);
        assert_eq!(local["name"], "Private Name");
        assert_eq!(local["style"], "playful");
        assert_eq!(local["relevant_lexicon"][0], "subjectword1099");
        assert_eq!(public["name"], "");
        assert_eq!(public["style"], "friendly");
        assert_eq!(public["interests"].as_array().unwrap().len(), 0);
        assert_eq!(public["relevant_lexicon"].as_array().unwrap().len(), 0);
        assert_eq!(public["news"][0]["source"], "Source A");
        let _ = fs::remove_dir_all(&dir);
    }
    #[test]
    fn legacy_profile_does_not_grant_new_permissions() {
        let p: UserProfile = serde_json::from_str(r#"{"name":"Pat","consent":true}"#).unwrap();
        assert!(!p.consents.microphone);
        assert!(!p.consents.live_information);
        assert!(!p.onboarding_complete);
    }
    #[tokio::test]
    async fn consent_phrasebook_and_expired_retrieval() {
        let dir = std::env::temp_dir().join(format!("buddy-regression-{}", uuid::Uuid::new_v4()));
        let mut e = BuddyEngine::new("Pat", "London", PluginRegistry::new(), &dir);
        assert!(e.learn_terms("private vocabulary").is_empty());
        // Online retrieval can run without permission to persist personal data.
        assert!(e
            .accept_prefetch(Vec::new(), Vec::new(), Vec::new())
            .last_prefetch
            .is_some());
        e.user
            .phrasebook
            .insert("tea time".into(), "Let's take a tea break.".into());
        assert_eq!(
            e.chat("tea time").await.unwrap().capability.as_deref(),
            Some("phrasebook.local")
        );
        e.knowledge.push(KnowledgeItem {
            title: "Astronomy discovery".into(),
            summary: "Expired".into(),
            url: "https://example.org".into(),
            source: "test".into(),
            category: "test".into(),
            saved_at: Local::now() - chrono::Duration::hours(25),
        });
        assert!(e.knowledge_items().is_empty());
        assert_ne!(
            e.chat("astronomy").await.unwrap().capability.as_deref(),
            Some("rag.local")
        );
        let mut p = e.profile();
        p.name = "Pat".into();
        assert!(e.set_profile(p.clone()).is_err());
        p.consents.profile_storage = true;
        p.contacts.push(Contact {
            name: "Alex".into(),
            ..Default::default()
        });
        assert!(e.set_profile(p).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn export_and_erase_remove_persisted_personal_data() {
        let root = std::env::temp_dir().join(format!("buddy-personal-{}", uuid::Uuid::new_v4()));
        let data = root.join("data");
        let mut engine = BuddyEngine::new("Friend", "Ahmedabad", PluginRegistry::new(), &data);
        let mut profile = engine.profile();
        profile.name = "Test user".into();
        profile.consents.profile_storage = true;
        profile.consents.contacts_storage = true;
        profile.consents.personalization = true;
        profile.contacts.push(Contact {
            id: "test-1".into(),
            name: "Private contact".into(),
            ..Default::default()
        });
        engine.set_profile(profile).unwrap();
        engine.learn_terms("privateword");
        engine.log("test activity");
        let export = engine.export_personal_data();
        assert_eq!(export["profile"]["contacts"][0]["name"], "Private contact");
        assert!(data.join("profile.json").exists());
        let stored_profile = std::fs::read(data.join("profile.json")).unwrap();
        assert!(stored_profile.starts_with(PROTECTED_JSON_MAGIC));
        assert!(!stored_profile
            .windows(b"Private contact".len())
            .any(|window| window == b"Private contact"));
        engine.erase_personal_data().unwrap();
        assert_eq!(engine.profile().name, "Friend");
        assert!(!data.join("profile.json").exists());
        assert!(!data.join("lexicon.json").exists());
        assert!(!root.join("logs").join("buddy.log").exists());
        let reopened = BuddyEngine::new("Friend", "Ahmedabad", PluginRegistry::new(), &data);
        assert!(reopened.profile().contacts.is_empty());
        assert!(reopened.lexicon().is_empty());
        let _ = std::fs::remove_dir_all(root);
    }
}
