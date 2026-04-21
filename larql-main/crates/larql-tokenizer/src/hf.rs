//! HuggingFace `tokenizers` adapter. Default impl for LARQL.

use std::path::Path;

use crate::{Encoded, Tokenizer, TokenizerError};

/// Thin newtype around [`tokenizers::Tokenizer`] so downstream crates never
/// see the concrete HF type.
pub struct HfTokenizer {
    inner: tokenizers::Tokenizer,
}

impl HfTokenizer {
    pub fn from_file(path: &Path) -> Result<Self, TokenizerError> {
        let inner = tokenizers::Tokenizer::from_file(path)
            .map_err(|e| TokenizerError::Parse(e.to_string()))?;
        Ok(Self { inner })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TokenizerError> {
        let inner = tokenizers::Tokenizer::from_bytes(bytes)
            .map_err(|e| TokenizerError::Parse(e.to_string()))?;
        Ok(Self { inner })
    }

    /// Escape hatch for code that genuinely needs the HF handle. Avoid in new
    /// code — prefer trait methods so impls can be swapped.
    pub fn inner(&self) -> &tokenizers::Tokenizer {
        &self.inner
    }
}

impl Tokenizer for HfTokenizer {
    fn encode(&self, text: &str, add_special: bool) -> Result<Encoded, TokenizerError> {
        let enc = self
            .inner
            .encode(text, add_special)
            .map_err(|e| TokenizerError::Parse(e.to_string()))?;
        Ok(Encoded {
            ids: enc.get_ids().to_vec(),
            tokens: enc.get_tokens().to_vec(),
        })
    }

    fn decode(&self, ids: &[u32], skip_special: bool) -> Result<String, TokenizerError> {
        self.inner
            .decode(ids, skip_special)
            .map_err(|e| TokenizerError::Parse(e.to_string()))
    }

    fn id_to_token(&self, id: u32) -> Option<String> {
        self.inner.id_to_token(id)
    }

    fn token_to_id(&self, tok: &str) -> Option<u32> {
        self.inner.token_to_id(tok)
    }

    fn vocab_size(&self, with_special: bool) -> usize {
        self.inner.get_vocab_size(with_special)
    }

    fn vocab(&self, with_special: bool) -> std::collections::HashMap<String, u32> {
        self.inner.get_vocab(with_special)
    }

    fn to_json(&self, pretty: bool) -> Result<String, TokenizerError> {
        self.inner
            .to_string(pretty)
            .map_err(|e| TokenizerError::Parse(e.to_string()))
    }

    fn backend_name(&self) -> &'static str {
        "hf"
    }
}
