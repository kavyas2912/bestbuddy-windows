//! A small deterministic, fast, offline subset of the Python VoiceEngine's
//! built-in knowledge and arithmetic helpers. No new model, Python process or
//! network request is required. App actions always retain higher priority.

/// Answer only unambiguous local questions; return None for the existing
/// phrasebook/RAG/optional LLM routing to handle other utterances.
pub fn answer(input: &str) -> Option<String> {
    let normal = input.to_lowercase();
    let query = normal.trim().trim_end_matches(&['?', '!', '.'][..]).trim();
    if let Some(country) = query
        .strip_prefix("what is the capital of ")
        .or_else(|| query.strip_prefix("what's the capital of "))
        .or_else(|| query.strip_prefix("tell me the capital of "))
        .or_else(|| query.strip_prefix("capital of "))
        .or_else(|| query.strip_suffix(" capital"))
    {
        const CAPITALS: [(&str, &str, &str); 11] = [
            ("france", "Paris", "France"),
            ("india", "New Delhi", "India"),
            ("japan", "Tokyo", "Japan"),
            ("germany", "Berlin", "Germany"),
            ("italy", "Rome", "Italy"),
            ("spain", "Madrid", "Spain"),
            ("australia", "Canberra", "Australia"),
            ("canada", "Ottawa", "Canada"),
            ("united kingdom", "London", "the United Kingdom"),
            ("united states", "Washington, D.C.", "the United States"),
            ("brazil", "Brasília", "Brazil"),
        ];
        if let Some((_, city, label)) = CAPITALS.iter().find(|(name, _, _)| *name == country.trim())
        {
            return Some(format!("{city} is the capital of {label}."));
        }
    }
    // Covers the high-frequency spoken arithmetic forms used by VoiceEngine,
    // intentionally limited to a single binary operation (no expression eval).
    let expr = query
        .strip_prefix("what is ")
        .or_else(|| query.strip_prefix("what's "))
        .or_else(|| query.strip_prefix("calculate "))
        .or_else(|| query.strip_prefix("compute "))?;
    let ops: [(&str, char); 10] = [
        (" multiplied by ", '*'),
        (" divided by ", '/'),
        (" times ", '*'),
        (" into ", '*'),
        (" plus ", '+'),
        (" minus ", '-'),
        (" + ", '+'),
        (" - ", '-'),
        (" * ", '*'),
        (" / ", '/'),
    ];
    if expr.len() > 120 {
        return None;
    }
    for (sep, op) in ops {
        if let Some((a, b)) = expr.split_once(sep) {
            let (Ok(left), Ok(right)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>()) else {
                return None;
            };
            if !left.is_finite() || !right.is_finite() || left.abs() > 1e12 || right.abs() > 1e12 {
                return None;
            }
            if op == '/' && right == 0.0 {
                return Some("Division by zero is undefined.".into());
            }
            let result = match op {
                '+' => left + right,
                '-' => left - right,
                '*' => left * right,
                '/' => left / right,
                _ => unreachable!(),
            };
            return (result.is_finite() && result.abs() <= 1e15)
                .then(|| format!("The answer is {result}."));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::answer;
    #[test]
    fn bundled_capitals_work_without_any_provider() {
        assert_eq!(
            answer("What is the capital of France?").as_deref(),
            Some("Paris is the capital of France.")
        );
        assert_eq!(
            answer("India capital").as_deref(),
            Some("New Delhi is the capital of India.")
        );
        assert!(answer("What is the capital of Atlantis?").is_none());
    }
    #[test]
    fn simple_spoken_calculations_are_bounded_and_safe() {
        assert_eq!(
            answer("What is 7 plus 5?").as_deref(),
            Some("The answer is 12.")
        );
        assert_eq!(
            answer("Calculate 9 divided by 0").as_deref(),
            Some("Division by zero is undefined.")
        );
        assert!(answer("Calculate 9 plus 5 and delete files").is_none());
        assert!(answer("How is the weather?").is_none());
    }
}
