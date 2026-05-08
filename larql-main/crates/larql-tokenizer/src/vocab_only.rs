//! Decode-only tokenizer: reads only `model.vocab` from a `tokenizer.json`.
//!
//! Zero dependency on the `tokenizers` crate. Sufficient for consumers that
//! need `id_to_token`, `token_to_id`, and approximate decode for display —
//! e.g. label collectors, diagnostic tools, and read-only vindex readers.
//!
//! Known divergence from [`crate::HfTokenizer`]:
//! - `encode` is unsupported.
//! - `decode` concatenates vocab pieces with `Ġ`→` ` and `▁`→` ` stripping.
//!   Byte-level BPE merges and special decoders are NOT undone. Rare/
//!   multi-piece tokens may differ from HF output.

use std::collections::HashMap;
use std::path::Path;

use crate::{Encoded, Tokenizer, TokenizerError};

/// Minimal tokenizer that only knows the vocab table.
pub struct VocabOnlyTokenizer {
    /// `id_to_token[i]` is the raw vocab piece for id `i` (including `Ġ`/`▁`).
    id_to_token: Vec<String>,
    /// Reverse lookup.
    token_to_id: HashMap<String, u32>,
}

impl VocabOnlyTokenizer {
    pub fn from_file(path: &Path) -> Result<Self, TokenizerError> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TokenizerError> {
        let root: serde_json::Value = serde_json::from_slice(bytes)
            .map_err(|e| TokenizerError::Parse(format!("invalid tokenizer.json: {e}")))?;

        // tokenizer.json layout: { "model": { "vocab": { "token": id, ... }, ... }, ... }
        let vocab = root
            .get("model")
            .and_then(|m| m.get("vocab"))
            .and_then(|v| v.as_object())
            .ok_or_else(|| TokenizerError::Parse("missing .model.vocab object".into()))?;

        let mut pairs: Vec<(String, u32)> = Vec::with_capacity(vocab.len());
        for (tok, v) in vocab {
            let id = v
                .as_u64()
                .ok_or_else(|| TokenizerError::Parse(format!("vocab[{tok}] is not a u32")))?;
            pairs.push((tok.clone(), id as u32));
        }

        // Build dense id_to_token table sized to max_id + 1. Gaps remain empty
        // strings — matches HF behaviour for unused ids.
        let max_id = pairs.iter().map(|(_, id)| *id).max().unwrap_or(0);
        let mut id_to_token = vec![String::new(); max_id as usize + 1];
        let mut token_to_id: HashMap<String, u32> = HashMap::with_capacity(pairs.len());
        for (tok, id) in pairs {
            id_to_token[id as usize] = tok.clone();
            token_to_id.insert(tok, id);
        }

        // `added_tokens` lives outside `.model.vocab` in most tokenizer.json
        // files; fold them in so specials (BOS/EOS/PAD) are resolvable.
        if let Some(added) = root.get("added_tokens").and_then(|v| v.as_array()) {
            for entry in added {
                let id = entry.get("id").and_then(|v| v.as_u64());
                let content = entry.get("content").and_then(|v| v.as_str());
                if let (Some(id), Some(content)) = (id, content) {
                    let id = id as u32;
                    let idx = id as usize;
                    if idx >= id_to_token.len() {
                        id_to_token.resize(idx + 1, String::new());
                    }
                    id_to_token[idx] = content.to_string();
                    token_to_id.insert(content.to_string(), id);
                }
            }
        }

        Ok(Self {
            id_to_token,
            token_to_id,
        })
    }

    /// Strip byte-level BPE piece markers (`Ġ`, `▁`) and emit readable text.
    fn piece_to_text(piece: &str) -> String {
        // Byte-fallback hex tokens (e.g. `<0x41>`) → the raw byte as a char.
        if let Some(hex) = piece.strip_prefix("<0x").and_then(|s| s.strip_suffix('>')) {
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                return (b as char).to_string();
            }
        }
        piece.replace('Ġ', " ").replace('▁', " ")
    }
}

impl Tokenizer for VocabOnlyTokenizer {
    fn encode(&self, _text: &str, _add_special: bool) -> Result<Encoded, TokenizerError> {
        Err(TokenizerError::Unsupported(
            "VocabOnlyTokenizer cannot encode; use HfTokenizer",
        ))
    }

    fn decode(&self, ids: &[u32], _skip_special: bool) -> Result<String, TokenizerError> {
        let mut out = String::new();
        for &id in ids {
            let idx = id as usize;
            if idx >= self.id_to_token.len() {
                continue;
            }
            let piece = &self.id_to_token[idx];
            if piece.is_empty() {
                continue;
            }
            out.push_str(&Self::piece_to_text(piece));
        }
        Ok(out.trim_start().to_string())
    }

    fn id_to_token(&self, id: u32) -> Option<String> {
        let idx = id as usize;
        if idx >= self.id_to_token.len() {
            return None;
        }
        let s = &self.id_to_token[idx];
        if s.is_empty() {
            None
        } else {
            Some(s.clone())
        }
    }

    fn token_to_id(&self, tok: &str) -> Option<u32> {
        self.token_to_id.get(tok).copied()
    }

    fn vocab_size(&self, _with_special: bool) -> usize {
        self.id_to_token.len()
    }

    fn vocab(&self, _with_special: bool) -> HashMap<String, u32> {
        self.token_to_id.clone()
    }

    fn backend_name(&self) -> &'static str {
        "vocab-only"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_json() -> &'static str {
        r#"{
            "model": {
                "vocab": {
                    "hello": 0,
                    "Ġworld": 1,
                    "▁foo": 2,
                    "<0x41>": 3
                }
            },
            "added_tokens": [
                { "id": 4, "content": "<bos>" },
                { "id": 5, "content": "<eos>" }
            ]
        }"#
    }

    #[test]
    fn loads_vocab_and_added_tokens() {
        let t = VocabOnlyTokenizer::from_bytes(fixture_json().as_bytes()).unwrap();
        assert_eq!(t.token_to_id("hello"), Some(0));
        assert_eq!(t.token_to_id("<bos>"), Some(4));
        assert_eq!(t.id_to_token(5).as_deref(), Some("<eos>"));
        assert_eq!(t.vocab_size(true), 6);
    }

    #[test]
    fn decode_strips_bpe_markers() {
        let t = VocabOnlyTokenizer::from_bytes(fixture_json().as_bytes()).unwrap();
        assert_eq!(t.decode(&[0, 1], true).unwrap(), "hello world");
        assert_eq!(t.decode(&[0, 2], true).unwrap(), "hello foo");
    }

    #[test]
    fn decode_byte_fallback_hex() {
        let t = VocabOnlyTokenizer::from_bytes(fixture_json().as_bytes()).unwrap();
        // <0x41> = 'A'
        assert_eq!(t.decode(&[3], true).unwrap(), "A");
    }

    #[test]
    fn encode_is_unsupported() {
        let t = VocabOnlyTokenizer::from_bytes(fixture_json().as_bytes()).unwrap();
        assert!(matches!(
            t.encode("hello", true),
            Err(TokenizerError::Unsupported(_))
        ));
    }

    #[test]
    fn backend_name_is_vocab_only() {
        let t = VocabOnlyTokenizer::from_bytes(fixture_json().as_bytes()).unwrap();
        assert_eq!(t.backend_name(), "vocab-only");
    }
}
