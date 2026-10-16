// A bounded recursive-descent calculator. Transcript text is never executed.
pub fn calculate(text: &str) -> Option<String> {
    let mut expression = text
        .trim()
        .to_lowercase()
        .trim_end_matches(['?', '.', '!'])
        .to_string();
    for p in ["what is ", "calculate ", "compute "] {
        if let Some(s) = expression.strip_prefix(p) {
            expression = s.into();
            break;
        }
    }
    for suffix in [
        " giving the answer",
        " giving answer",
        " give me the answer",
        " give the answer",
        " give answer",
        " what is the answer",
    ] {
        if let Some(s) = expression.strip_suffix(suffix) {
            expression = s.into();
            break;
        }
    }
    // Replace whole words, not fragments of arbitrary prose.
    for (word, symbol) in [
        ("multiplied by", "*"),
        ("divided by", "/"),
        ("plus", "+"),
        ("minus", "-"),
        ("times", "*"),
        ("into", "*"),
    ] {
        expression = format!(" {} ", expression)
            .replace(&format!(" {word} "), &format!(" {symbol} "))
            .trim()
            .into();
    }
    expression = expression.replace('^', "**");
    if !expression.chars().any(|c| c.is_ascii_digit())
        || expression
            .chars()
            .any(|c| !c.is_ascii_digit() && !c.is_ascii_whitespace() && !".()+*/%-".contains(c))
    {
        return None;
    }
    if expression.len() > 120 {
        return Some("That calculation is too large for the built-in calculator.".into());
    }
    let bytes: Vec<_> = expression
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let mut parser = Parser {
        bytes: &bytes,
        at: 0,
    };
    let result = parser.sum().and_then(|v| {
        if parser.at == bytes.len() {
            Ok(v)
        } else {
            Err("syntax")
        }
    });
    Some(match result {
        Ok(value) => format!("The answer is {}.", value),
        Err("zero") => "Division by zero is undefined.".into(),
        Err(_) => "I could not calculate that safely. Try a shorter arithmetic expression.".into(),
    })
}
struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }
    fn checked(value: f64) -> Result<f64, &'static str> {
        if value.is_finite() && value.abs() <= 1e15 {
            Ok(value)
        } else {
            Err("bound")
        }
    }
    fn sum(&mut self) -> Result<f64, &'static str> {
        let mut value = self.product()?;
        while let Some(op @ (b'+' | b'-')) = self.peek() {
            self.at += 1;
            let right = self.product()?;
            value = Self::checked(if op == b'+' {
                value + right
            } else {
                value - right
            })?;
        }
        Ok(value)
    }
    fn product(&mut self) -> Result<f64, &'static str> {
        let mut value = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(b'*') if self.bytes.get(self.at + 1) != Some(&b'*') => b'*',
                Some(b'/') => b'/',
                Some(b'%') => b'%',
                _ => break,
            };
            self.at += 1;
            let right = self.unary()?;
            if op != b'*' && right == 0.0 {
                return Err("zero");
            }
            value = Self::checked(match op {
                b'*' => value * right,
                b'/' => value / right,
                _ => value - right * (value / right).floor(),
            })?;
        }
        Ok(value)
    }
    fn unary(&mut self) -> Result<f64, &'static str> {
        match self.peek() {
            Some(b'+') => {
                self.at += 1;
                self.unary()
            }
            Some(b'-') => {
                self.at += 1;
                Ok(-self.unary()?)
            }
            _ => self.power(),
        }
    }
    fn power(&mut self) -> Result<f64, &'static str> {
        let value = self.atom()?;
        if self.bytes.get(self.at..self.at + 2) == Some(b"**") {
            self.at += 2;
            let exponent = self.unary()?;
            if exponent.abs() > 12.0 {
                return Err("bound");
            }
            Self::checked(value.powf(exponent))
        } else {
            Ok(value)
        }
    }
    fn atom(&mut self) -> Result<f64, &'static str> {
        if self.peek() == Some(b'(') {
            self.at += 1;
            let value = self.sum()?;
            if self.peek() != Some(b')') {
                return Err("syntax");
            }
            self.at += 1;
            return Ok(value);
        }
        let start = self.at;
        while self.peek().is_some_and(|b| b.is_ascii_digit() || b == b'.') {
            self.at += 1;
        }
        let number = std::str::from_utf8(&self.bytes[start..self.at])
            .map_err(|_| "syntax")?
            .parse::<f64>()
            .map_err(|_| "syntax")?;
        Self::checked(number)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arithmetic_precedence_bounds_and_nonexecution() {
        for (input, expected) in [
            ("what is 2 plus 3 times 4", "The answer is 14."),
            ("calculate -2^2", "The answer is -4."),
            ("2^3^2", "The answer is 512."),
            ("(12 - 2) / 5", "The answer is 2."),
            ("-5 % 3", "The answer is 1."),
            ("4 divided by 0", "Division by zero is undefined."),
        ] {
            assert_eq!(calculate(input).as_deref(), Some(expected), "{input}");
        }
        assert!(calculate("import os").is_none());
        assert!(calculate("open the calculator").is_none());
        assert!(calculate("2^99").unwrap().contains("safely"));
    }
}
