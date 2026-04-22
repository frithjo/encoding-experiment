//! Token shape classification + aggregation used by both the LQL client
//! (`SHOW TOKENS` local executor) and the HTTP server (`/v1/tokens`).
//!
//! Promoted here so both `larql-lql` and `larql-server` can consume a
//! single source of truth for shape rules and token-hit collection.

use std::collections::HashMap;

use crate::text::is_readable_token;
use crate::{LayerBands, PatchedVindex};

/// Coarse token shape classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TokenShape {
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

pub fn classify_token_shape(tok: &str) -> TokenShape {
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
    if chars
        .iter()
        .all(|c| !c.is_alphanumeric() && !c.is_whitespace())
    {
        return TokenShape::Symbolic;
    }
    TokenShape::Other
}

pub fn token_shape_name(shape: TokenShape) -> &'static str {
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

/// Stricter filter that classifies entity-capable tokens.
/// Keeps title-case, mixed-case, and short acronyms; drops stopwords.
pub fn entity_token_kind(tok: &str) -> Option<&'static str> {
    let tok = tok.trim();
    if !is_readable_token(tok) {
        return None;
    }
    let lower = tok.to_lowercase();
    if matches!(
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

pub fn band_name_for_layer(layer: usize, bands: &LayerBands) -> &'static str {
    if layer >= bands.knowledge.0 && layer <= bands.knowledge.1 {
        "knowledge"
    } else if layer >= bands.output.0 && layer <= bands.output.1 {
        "output"
    } else {
        "syntax"
    }
}

/// Per-token aggregation, keyed by token string in [`collect_token_hits`].
#[derive(Clone, Copy, Debug)]
pub struct TokenHit {
    pub shape: TokenShape,
    pub kind: Option<&'static str>,
    pub hits: usize,
    pub syntax_hits: usize,
    pub knowledge_hits: usize,
    pub output_hits: usize,
    pub max_score: f32,
}

/// Per-shape rollup used by the shape table in `SHOW TOKENS`.
#[derive(Default, Clone, Debug)]
pub struct ShapeInfo {
    pub distinct: usize,
    pub feature_hits: usize,
    pub entity_like: usize,
    pub examples: Vec<String>,
}

/// Per-kind rollup (entity type bucket).
#[derive(Default, Clone, Debug)]
pub struct KindInfo {
    pub distinct: usize,
    pub feature_hits: usize,
    pub examples: Vec<String>,
}

/// Post-collection filter, applied while iterating features.
/// All fields are optional AND-combined.
#[derive(Clone, Default, Debug)]
pub struct TokenFilters {
    pub token_filter: Option<String>,
    pub shape_filter: Option<String>,
    pub type_filter: Option<String>,
    pub band_filter: Option<String>,
}

/// Scan the given layers and return one [`TokenHit`] per distinct top-token,
/// honouring [`TokenFilters`].
pub fn collect_token_hits(
    patched: &PatchedVindex,
    scan_layers: &[usize],
    bands: &LayerBands,
    filters: &TokenFilters,
) -> HashMap<String, TokenHit> {
    let mut token_hits: HashMap<String, TokenHit> = HashMap::new();

    for layer in scan_layers {
        let nf = patched.num_features(*layer);
        for feat in 0..nf {
            if let Some(meta) = patched.feature_meta(*layer, feat) {
                let tok = meta.top_token.trim().to_string();
                let shape = classify_token_shape(&tok);
                let kind = entity_token_kind(&tok);
                let shape_name = token_shape_name(shape);
                let band_name = band_name_for_layer(*layer, bands);

                if let Some(tf) = &filters.token_filter {
                    if !tok.to_lowercase().contains(&tf.to_lowercase()) {
                        continue;
                    }
                }
                if let Some(sf) = &filters.shape_filter {
                    if shape_name != sf {
                        continue;
                    }
                }
                if let Some(tf) = &filters.type_filter {
                    let kind_name = kind.unwrap_or("none");
                    if kind_name != tf {
                        continue;
                    }
                }
                if let Some(bf) = &filters.band_filter {
                    if band_name != bf {
                        continue;
                    }
                }

                let entry = token_hits.entry(tok).or_insert(TokenHit {
                    shape,
                    kind,
                    hits: 0,
                    syntax_hits: 0,
                    knowledge_hits: 0,
                    output_hits: 0,
                    max_score: 0.0,
                });
                entry.hits += 1;
                if meta.c_score > entry.max_score {
                    entry.max_score = meta.c_score;
                }
                match band_name {
                    "knowledge" => entry.knowledge_hits += 1,
                    "output" => entry.output_hits += 1,
                    _ => entry.syntax_hits += 1,
                }
            }
        }
    }

    token_hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::is_content_token;

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

    #[test]
    fn band_name_layers_correctly() {
        let bands = LayerBands {
            syntax: (0, 13),
            knowledge: (14, 21),
            output: (22, 27),
        };
        assert_eq!(band_name_for_layer(5, &bands), "syntax");
        assert_eq!(band_name_for_layer(15, &bands), "knowledge");
        assert_eq!(band_name_for_layer(25, &bands), "output");
    }
}
