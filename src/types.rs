use std::str::FromStr;

/// Result that can return any error type.
pub type AnyResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone)]
pub enum Delimiter {
    Character(u8),
    Whitespace,
}

impl FromStr for Delimiter {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_lowercase();
        if s.is_empty() {
            return Err("Delimiter cannot be empty".into());
        }

        if s == "whitespace" {
            Ok(Delimiter::Whitespace)
        } else if s.len() == 1 {
            Ok(Delimiter::Character(s.as_bytes()[0]))
        } else {
            Err("Delimiter must be a single character or 'whitespace'".into())
        }
    }
}
