use serde::Serialize;

// Original BestBuddy copy is kept here, separate from time-sensitive retrieved data.
// Factual cards have a publisher link; a link is a reference, not proof that
// BestBuddy fetched or verified a live page during this turn.
#[derive(Clone, Debug, Serialize)]
pub struct PersonaCard {
    pub id: &'static str,
    pub kind: &'static str,
    pub topic: &'static str,
    pub text: &'static str,
    pub source_name: &'static str,
    pub source_url: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct PersonaReply {
    pub id: String,
    pub kind: String,
    pub topic: String,
    pub text: String,
    pub source_name: String,
    pub source_url: String,
}

/// Only explicit first-person words are used. Microphone tone alone is not a
/// reliable mood measurement and must never be presented as a diagnosis.
pub fn stated_mood(phrase: &str) -> Option<&'static str> {
    let first_person = ["i feel ", "i'm ", "i am ", "i've been ", "i have been "]
        .iter()
        .any(|start| phrase.starts_with(start) || phrase.contains(&format!(" {start}")));
    if !first_person {
        return None;
    }
    if ["lonely", "alone", "isolated"]
        .iter()
        .any(|word| phrase.contains(word))
    {
        Some("lonely")
    } else if ["sad", "down", "low", "upset"]
        .iter()
        .any(|word| phrase.contains(word))
    {
        Some("sad")
    } else if ["anxious", "worried", "stressed", "overwhelmed"]
        .iter()
        .any(|word| phrase.contains(word))
    {
        Some("anxious")
    } else if ["frustrated", "angry"]
        .iter()
        .any(|word| phrase.contains(word))
    {
        Some("frustrated")
    } else if ["excited", "happy", "delighted"]
        .iter()
        .any(|word| phrase.contains(word))
    {
        Some("happy")
    } else {
        None
    }
}

pub fn mood_follow_up(mood: &str) -> &'static str {
    match mood {
        "lonely" => "I'm here with you. Would you like to tell me what's on your mind, or shall I share a little story?",
        "sad" => "I'm sorry this feels hard. Would talking about it help, or would you prefer a gentle story?",
        "anxious" => "That sounds like a lot to carry. Would you like to talk it through, or would a quiet story help?",
        "frustrated" => "I hear that this is frustrating. Would you like to explain what happened before we look for a next step?",
        "happy" => "That sounds lovely. What happened? I'd like to hear about it.",
        _ => "I'm listening. What would you like to share?",
    }
}

impl From<&PersonaCard> for PersonaReply {
    fn from(card: &PersonaCard) -> Self {
        Self {
            id: card.id.into(),
            kind: card.kind.into(),
            topic: card.topic.into(),
            text: card.text.into(),
            source_name: card.source_name.into(),
            source_url: card.source_url.into(),
        }
    }
}

pub const CARDS: &[PersonaCard] = &[
    PersonaCard { id:"joke-bridge", kind:"joke", topic:"engineering", text:"Why did the bridge bring a jacket? It heard there might be a suspension in the air.", source_name:"BestBuddy original", source_url:"" },
    PersonaCard { id:"joke-clock", kind:"joke", topic:"time", text:"My clock asked for a break. I said, take your time.", source_name:"BestBuddy original", source_url:"" },
    PersonaCard { id:"joke-planet", kind:"joke", topic:"space", text:"The planets formed a band. Saturn insisted on playing the rings.", source_name:"BestBuddy original", source_url:"" },
    PersonaCard { id:"joke-book", kind:"joke", topic:"books", text:"I tried to write a book about gravity. I could not put it down.", source_name:"BestBuddy original", source_url:"" },
    PersonaCard { id:"joke-music", kind:"joke", topic:"music", text:"The piano said it was feeling flat. I told it to stay sharp.", source_name:"BestBuddy original", source_url:"" },
    PersonaCard { id:"story-seed", kind:"story", topic:"encouragement", text:"A seed sat in the dark for days, convinced nothing was happening. Then a root appeared. Quiet progress is still progress. What small step have you taken lately?", source_name:"BestBuddy original", source_url:"" },
    PersonaCard { id:"story-map", kind:"story", topic:"encouragement", text:"A traveller redrew the map each time the road changed. The journey did not become easier, but the next step became clearer. Which next step would help you today?", source_name:"BestBuddy original", source_url:"" },
    PersonaCard { id:"story-kite", kind:"story", topic:"encouragement", text:"A kite rose because it met the wind, not because the wind stopped. Some resistance can help us discover a steadier way forward.", source_name:"BestBuddy original", source_url:"" },
    PersonaCard { id:"story-teacup", kind:"story", topic:"companionship", text:"Two friends made tea and sat quietly. After a while one said, I am glad you stayed. Sometimes company is the whole gift.", source_name:"BestBuddy original", source_url:"" },
    PersonaCard { id:"thought-question", kind:"thought", topic:"philosophy", text:"A gentle question for today: which part of your routine is serving you, and which part is only familiar?", source_name:"BestBuddy original", source_url:"" },
    PersonaCard { id:"thought-ordinary", kind:"thought", topic:"philosophy", text:"Not every good day has to be remarkable. A calm conversation, a useful task, or a walk can be enough.", source_name:"BestBuddy original", source_url:"" },
    PersonaCard { id:"thought-learning", kind:"thought", topic:"philosophy", text:"Changing your mind after learning something new is a sign of attention, not a failure of conviction.", source_name:"BestBuddy original", source_url:"" },
    PersonaCard { id:"fact-venus", kind:"science", topic:"space astronomy", text:"On Venus, one rotation takes about 243 Earth days, longer than its roughly 225-day trip around the Sun.", source_name:"NASA Science", source_url:"https://science.nasa.gov/venus/venus-facts/" },
    PersonaCard { id:"fact-moon", kind:"science", topic:"moon space", text:"The Moon keeps almost the same face toward Earth because its rotation period matches its orbit around Earth.", source_name:"NASA Science", source_url:"https://science.nasa.gov/moon/facts/" },
    PersonaCard { id:"fact-ocean", kind:"science", topic:"ocean earth", text:"The ocean covers more than 70 percent of Earth's surface. There is still much to explore below its surface.", source_name:"NOAA Ocean Service", source_url:"https://oceanservice.noaa.gov/facts/oceanwater.html" },
    PersonaCard { id:"fact-sleep", kind:"health", topic:"sleep wellness", text:"Most adults need about seven to nine hours of sleep. If poor sleep keeps affecting your days, a health professional can help you work out why.", source_name:"MedlinePlus", source_url:"https://medlineplus.gov/healthysleep.html" },
    PersonaCard { id:"fact-movement", kind:"health", topic:"exercise movement wellness", text:"Regular physical activity can support health even when you start slowly. The right amount depends on your circumstances.", source_name:"MedlinePlus", source_url:"https://medlineplus.gov/benefitsofexercise.html" },
    PersonaCard { id:"book-librivox", kind:"audiobook", topic:"books stories literature", text:"For a free public-domain audiobook, we can browse LibriVox together. What author, title, or kind of story are you in the mood for?", source_name:"LibriVox catalog", source_url:"https://librivox.org/search" },
    PersonaCard { id:"book-gutenberg", kind:"audiobook", topic:"books classics literature", text:"Project Gutenberg has free public-domain books to read. If you name a title, I can help you find its catalog page.", source_name:"Project Gutenberg", source_url:"https://www.gutenberg.org/" },
];

pub fn intent(phrase: &str) -> Option<&'static str> {
    if ["joke", "pun", "make me laugh", "something funny"]
        .iter()
        .any(|word| phrase.contains(word))
    {
        return Some("joke");
    }
    if ["audiobook", "audio book", "read me a book"]
        .iter()
        .any(|word| phrase.contains(word))
    {
        return Some("audiobook");
    }
    if ["medical fact", "health fact", "wellness fact"]
        .iter()
        .any(|word| phrase.contains(word))
    {
        return Some("health");
    }
    if ["science fact", "space fact", "astronomy fact"]
        .iter()
        .any(|word| phrase.contains(word))
    {
        return Some("science");
    }
    if ["philosophy", "quote", "thought for today"]
        .iter()
        .any(|word| phrase.contains(word))
    {
        return Some("thought");
    }
    if [
        "encourage me",
        "cheer me up",
        "morale",
        "motivating story",
        "inspiring story",
    ]
    .iter()
    .any(|word| phrase.contains(word))
    {
        return Some("story");
    }
    if ["tell me a story", "little story", "anecdote"]
        .iter()
        .any(|word| phrase.contains(word))
    {
        return Some("story");
    }
    if ["fun fact", "trivia", "interesting fact"]
        .iter()
        .any(|word| phrase.contains(word))
    {
        return Some("science");
    }
    None
}

pub fn matching_resource(phrase: &str) -> Option<&'static PersonaCard> {
    if !(phrase.contains("source")
        || phrase.contains("where can i read")
        || phrase.contains("learn about"))
    {
        return None;
    }
    CARDS
        .iter()
        .filter(|card| !card.source_url.is_empty())
        .max_by_key(|card| {
            card.topic
                .split_whitespace()
                .filter(|word| phrase.contains(word))
                .count()
        })
        .filter(|card| {
            card.topic
                .split_whitespace()
                .any(|word| phrase.contains(word))
        })
}
