//! Shared helpers: formatting, token filtering.

use std::path::Path;

pub(crate) use larql_vindex::{is_content_token, is_readable_token};

/// Get total size of a directory in bytes.
pub(crate) fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

pub(crate) fn format_number(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{n}")
    }
}

pub(crate) fn format_bytes(b: u64) -> String {
    if b >= 1_073_741_824 {
        format!("{:.2} GB", b as f64 / 1_073_741_824.0)
    } else if b >= 1_048_576 {
        format!("{:.1} MB", b as f64 / 1_048_576.0)
    } else if b >= 1024 {
        format!("{:.1} KB", b as f64 / 1024.0)
    } else {
        format!("{b} B")
    }
}

/// Stricter filter for entity enumeration.
/// Classify tokens without dropping classes on the floor.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum TokenShape {
    Empty,
    LowerWord,
    TitleWord,
    UpperWord,
    MixedWord,
    Number,
    AlphaNumeric,
    Punctuation,
    Symbolic,
    Other,
}

pub(crate) fn classify_token_shape(tok: &str) -> TokenShape {
    let tok = tok.trim();
    if tok.is_empty() {
        return TokenShape::Empty;
    }

    let chars: Vec<char> = tok.chars().collect();
    if chars.iter().all(|c| c.is_numeric()) {
        return TokenShape::Number;
    }
    if chars.iter().all(|c| c.is_alphanumeric()) {
        let alpha = chars.iter().filter(|c| c.is_alphabetic()).count();
        let upper = chars.iter().filter(|c| c.is_uppercase()).count();
        let lower = chars.iter().filter(|c| c.is_lowercase()).count();
        let digits = chars.iter().filter(|c| c.is_numeric()).count();

        if digits > 0 && alpha > 0 {
            return TokenShape::AlphaNumeric;
        }
        if alpha == chars.len() {
            if lower == chars.len() {
                return TokenShape::LowerWord;
            }
            if upper == chars.len() {
                return TokenShape::UpperWord;
            }
            if chars[0].is_uppercase() && chars[1..].iter().all(|c| c.is_lowercase()) {
                return TokenShape::TitleWord;
            }
            return TokenShape::MixedWord;
        }
    }
    if chars.iter().all(|c| c.is_ascii_punctuation()) {
        return TokenShape::Punctuation;
    }
    if chars.iter().any(|c| c.is_alphabetic()) {
        return TokenShape::MixedWord;
    }
    if chars.iter().all(|c| !c.is_alphanumeric() && !c.is_whitespace()) {
        return TokenShape::Symbolic;
    }
    TokenShape::Other
}

pub(crate) fn token_shape_name(shape: TokenShape) -> &'static str {
    match shape {
        TokenShape::Empty => "empty",
        TokenShape::LowerWord => "lower",
        TokenShape::TitleWord => "title",
        TokenShape::UpperWord => "upper",
        TokenShape::MixedWord => "mixed",
        TokenShape::Number => "number",
        TokenShape::AlphaNumeric => "alnum",
        TokenShape::Punctuation => "punct",
        TokenShape::Symbolic => "symbol",
        TokenShape::Other => "other",
    }
}

/// Prefer proper-name-like tokens, but keep acronyms and mixed-case brands visible.
pub(crate) fn entity_token_kind(tok: &str) -> Option<&'static str> {
    let tok = tok.trim();
    if !is_readable_token(tok) {
        return None;
    }
    let lower = tok.to_lowercase();
    if matches!(
        lower.as_str(),
        "the" | "and" | "for" | "but" | "not" | "you" | "all" | "can"
        | "her" | "was" | "one" | "our" | "out" | "are" | "has" | "his"
        | "how" | "its" | "may" | "new" | "now" | "old" | "see" | "way"
        | "who" | "did" | "get" | "let" | "say" | "she" | "too" | "use"
        | "from" | "have" | "been" | "will" | "with" | "this" | "that"
        | "they" | "were" | "some" | "them" | "than" | "when"
        | "what" | "your" | "each" | "make" | "like" | "just" | "over"
        | "such" | "take" | "also" | "into" | "only" | "very" | "more"
        | "does" | "most" | "about" | "which" | "their" | "would" | "there"
        | "could" | "other" | "after" | "being" | "where" | "these" | "those"
        | "first" | "should" | "because" | "through" | "before"
        | "par" | "aux" | "che" | "del"
    ) {
        return None;
    }
    match classify_token_shape(tok) {
        TokenShape::TitleWord => Some("title"),
        TokenShape::MixedWord => Some("mixed"),
        TokenShape::UpperWord if tok.chars().count() <= 6 => Some("acronym"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_token_shape, entity_token_kind, is_content_token, token_shape_name, TokenShape};

    #[test]
    fn content_token_accepts_words() {
        assert!(is_content_token("France"));
        assert!(is_content_token("conversion"));
    }

    #[test]
    fn entity_token_accepts_title_case_names() {
        assert_eq!(entity_token_kind("France"), Some("title"));
        assert_eq!(entity_token_kind("Napoleon"), Some("title"));
    }

    #[test]
    fn token_shape_accounts_for_multiple_token_classes() {
        assert_eq!(classify_token_shape("france"), TokenShape::LowerWord);
        assert_eq!(classify_token_shape("France"), TokenShape::TitleWord);
        assert_eq!(classify_token_shape("NASA"), TokenShape::UpperWord);
        assert_eq!(classify_token_shape("iPhone"), TokenShape::MixedWord);
        assert_eq!(classify_token_shape("B52"), TokenShape::AlphaNumeric);
        assert_eq!(classify_token_shape("..."), TokenShape::Punctuation);
        assert_eq!(token_shape_name(TokenShape::MixedWord), "mixed");
    }

    #[test]
    fn entity_token_keeps_acronyms_and_mixed_case_visible() {
        assert_eq!(entity_token_kind("NASA"), Some("acronym"));
        assert_eq!(entity_token_kind("iPhone"), Some("mixed"));
    }

    #[test]
    fn entity_token_rejects_plain_words_and_symbols() {
        assert_eq!(entity_token_kind("language"), None);
        assert_eq!(entity_token_kind("1234"), None);
        assert_eq!(entity_token_kind("..."), None);
    }
}
