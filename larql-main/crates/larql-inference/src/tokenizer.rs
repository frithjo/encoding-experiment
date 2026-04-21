//! Tokenizer loading and helpers.
//!
//! Thin shim over `larql_tokenizer`: inference stores `Arc<dyn Tokenizer>`
//! everywhere via the alias [`TokenizerArc`].

use std::path::Path;
use std::sync::Arc;

use crate::error::InferenceError;

/// Alias for the shared trait-object tokenizer used across inference.
pub type TokenizerArc = Arc<dyn larql_tokenizer::Tokenizer>;

/// Load a tokenizer from a model directory.
pub fn load_tokenizer(model_dir: &Path) -> Result<TokenizerArc, InferenceError> {
    let path = model_dir.join("tokenizer.json");
    if !path.exists() {
        return Err(InferenceError::MissingTensor(
            "tokenizer.json not found".into(),
        ));
    }
    larql_tokenizer::load_tokenizer(&path).map_err(|e| InferenceError::Parse(e.to_string()))
}

/// Decode a single token ID to a trimmed string.
pub fn decode_token(tokenizer: &dyn larql_tokenizer::Tokenizer, id: u32) -> Option<String> {
    larql_tokenizer::decode_token(tokenizer, id)
}

/// Decode a single token ID, including special tokens (BOS, EOS, etc.).
/// Falls back to the raw vocabulary entry if normal decode produces nothing.
pub fn decode_token_raw(tokenizer: &dyn larql_tokenizer::Tokenizer, id: u32) -> String {
    larql_tokenizer::decode_token_raw(tokenizer, id)
}
