// vce-core/src/values.rs
//! Deterministic safety layer: crisis and scam detection run before anything else. Fixed rules, never an LLM judgement.
use std::collections::HashSet;

const CRISIS: &[&str] = &[
    "kill myself",
    "want to die",
    "end my life",
    "end it all",
    "suicide",
    "suicidal",
    "hurt myself",
    "self harm",
    "no reason to live",
    "better off dead",
    "dont want to live",
    "do not want to live",
];
const SCAM_PHRASES: &[&str] = &[
    "gift card",
    "remote access",
    "your account will be blocked",
    "digital arrest",
    "upi pin",
    "refund link",
    "share my password",
    "lottery winner",
];
const SCAM_WORDS: &[&str] = &["otp", "kyc", "cvv", "anydesk", "teamviewer", "lottery"];

fn norm(s: &str) -> String {
    s.to_lowercase()
        .replace(['\u{2019}', '\''], "")
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Default support line: India's Tele-MANAS when the phone is on IST, otherwise a generic instruction. Editable in Settings.
pub fn default_helpline(utc_offset_secs: i64) -> String {
    if utc_offset_secs == 19_800 {
        "Tele-MANAS on 14416 or 1-800-891-4416 (free, 24x7)".into()
    } else {
        "a crisis helpline in your country".into()
    }
}

pub fn check(text: &str, helpline: &str) -> Option<String> {
    let t = norm(text);
    if CRISIS.iter().any(|p| t.contains(&norm(p))) {
        return Some(format!("I am really sorry you are hurting, and I am glad you told me. You deserve support from a real person right now. Please call {helpline}, or reach out to a family member or friend you trust. If you might act on these thoughts, please call your local emergency number now. I am here to keep you company too."));
    }
    let words: HashSet<&str> = t.split(' ').collect();
    if SCAM_PHRASES.iter().any(|p| t.contains(*p)) || SCAM_WORDS.iter().any(|w| words.contains(w)) {
        return Some("Please be careful. Banks, officials and companies never ask for your OTP, PIN, CVV or password, and they do not ask you to install remote-access apps or buy gift cards. If someone is pressuring you, hang up and call the bank directly on the number printed on your card, or talk to a family member first.".into());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crisis_and_scam_detected_without_false_positives() {
        let h = default_helpline(19_800);
        assert!(check("I don't want to live anymore", &h)
            .unwrap()
            .contains("14416"));
        assert!(check("someone asked for my OTP on the phone", &h)
            .unwrap()
            .contains("never ask"));
        assert!(
            check("I love hotpot and potatoes", &h).is_none(),
            "'otp' inside words must not trigger"
        );
        assert!(check("tell me a joke", &h).is_none());
        assert!(default_helpline(0).contains("your country"));
    }
}
