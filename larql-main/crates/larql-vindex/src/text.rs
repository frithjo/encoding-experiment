//! Heuristic token filters shared by DESCRIBE edge collection and LQL helpers.

/// Heuristic: is a token readable enough to show to the user?
/// Filters out encoding garbage, isolated combining marks, etc.
pub fn is_readable_token(tok: &str) -> bool {
    let tok = tok.trim();
    if tok.is_empty() || tok.len() > 30 {
        return false;
    }
    let readable = tok
        .chars()
        .filter(|c| {
            c.is_ascii_alphanumeric()
                || *c == ' '
                || *c == '-'
                || *c == '\''
                || *c == '.'
                || *c == ','
        })
        .count();
    let total = tok.chars().count();
    readable * 2 >= total && total > 0
}

/// Stricter filter for SHOW RELATIONS and DESCRIBE: content words only.
/// Must look like a real word — no code tokens, no encoding fragments.
pub fn is_content_token(tok: &str) -> bool {
    let tok = tok.trim();
    if !is_readable_token(tok) {
        return false;
    }
    let chars: Vec<char> = tok.chars().collect();
    if chars.len() < 3 || chars.len() > 25 {
        return false;
    }
    // Must be mostly alphabetic
    let alpha = chars.iter().filter(|c| c.is_ascii_alphabetic()).count();
    if alpha < chars.len() * 2 / 3 {
        return false;
    }
    // Reject camelCase code tokens
    for w in chars.windows(2) {
        if w[0].is_ascii_lowercase() && w[1].is_ascii_uppercase() {
            return false;
        }
    }
    // Reject if all non-ASCII (encoding fragment)
    if !chars.iter().any(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    // Filter English stop words and common function words
    let lower = tok.to_lowercase();
    !matches!(
        lower.as_str(),
        "the"
            | "and"
            | "for"
            | "but"
            | "not"
            | "you"
            | "all"
            | "can"
            | "her"
            | "was"
            | "one"
            | "our"
            | "out"
            | "are"
            | "has"
            | "his"
            | "how"
            | "its"
            | "may"
            | "new"
            | "now"
            | "old"
            | "see"
            | "way"
            | "who"
            | "did"
            | "get"
            | "let"
            | "say"
            | "she"
            | "too"
            | "use"
            | "from"
            | "have"
            | "been"
            | "will"
            | "with"
            | "this"
            | "that"
            | "they"
            | "were"
            | "some"
            | "them"
            | "than"
            | "when"
            | "what"
            | "your"
            | "each"
            | "make"
            | "like"
            | "just"
            | "over"
            | "such"
            | "take"
            | "also"
            | "into"
            | "only"
            | "very"
            | "more"
            | "does"
            | "most"
            | "about"
            | "which"
            | "their"
            | "would"
            | "there"
            | "could"
            | "other"
            | "after"
            | "being"
            | "where"
            | "these"
            | "those"
            | "first"
            | "should"
            | "because"
            | "through"
            | "before"
            | "par"
            | "aux"
            | "che"
            | "del"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readable_tokens() {
        assert!(is_readable_token("French"));
        assert!(is_readable_token("Paris"));
        assert!(is_readable_token("capital-of"));
        assert!(is_readable_token("is"));
        assert!(is_readable_token("Europe"));
    }

    #[test]
    fn unreadable_tokens() {
        assert!(!is_readable_token("ইসলামাবাদ"));
        assert!(!is_readable_token("южна"));
        assert!(!is_readable_token("ളാ"));
        assert!(!is_readable_token("ڪ"));
        assert!(!is_readable_token(""));
    }

    #[test]
    fn content_tokens_pass() {
        assert!(is_content_token("French"));
        assert!(is_content_token("Paris"));
        assert!(is_content_token("Europe"));
        assert!(is_content_token("Mozart"));
        assert!(is_content_token("composer"));
        assert!(is_content_token("Berlin"));
        assert!(is_content_token("IBM"));
        assert!(is_content_token("Facebook"));
    }

    #[test]
    fn stop_words_rejected() {
        assert!(!is_content_token("the"));
        assert!(!is_content_token("from"));
        assert!(!is_content_token("for"));
        assert!(!is_content_token("with"));
        assert!(!is_content_token("this"));
        assert!(!is_content_token("about"));
        assert!(!is_content_token("which"));
        assert!(!is_content_token("first"));
        assert!(!is_content_token("after"));
    }

    #[test]
    fn short_tokens_rejected() {
        assert!(!is_content_token("a"));
        assert!(!is_content_token("of"));
        assert!(!is_content_token("is"));
        assert!(!is_content_token("-"));
        assert!(!is_content_token("lö"));
        assert!(!is_content_token("par"));
    }

    #[test]
    fn code_tokens_rejected() {
        assert!(!is_content_token("trialComponents"));
        assert!(!is_content_token("NavigationBar"));
        assert!(!is_content_token("LastName"));
    }
}
