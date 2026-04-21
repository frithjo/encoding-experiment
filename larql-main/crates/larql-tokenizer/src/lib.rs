//! Minimal tokenizer contract for LARQL.
//!
//! A dyn-dispatched trait with two impls:
//! - [`HfTokenizer`] — wraps `tokenizers::Tokenizer`; feature `hf` (default).
//! - [`VocabOnlyTokenizer`] — decode-only, zero `tokenizers` dependency.
//!
//! Downstream crates store tokenizers as `Arc<dyn Tokenizer>` so that the HF
//! crate can be swapped out (or compiled away) without touching call sites.

use std::path::Path;
use std::sync::Arc;

pub mod vocab_only;
pub use vocab_only::VocabOnlyTokenizer;

#[cfg(feature = "hf")]
pub mod hf;
#[cfg(feature = "hf")]
pub use hf::HfTokenizer;

/// One-shot encode result. Carries both ids and per-token strings so callers
/// that need `.get_ids()` or `.get_tokens()` don't need a second round-trip.
#[derive(Clone, Debug, Default)]
pub struct Encoded {
    pub ids: Vec<u32>,
    pub tokens: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum TokenizerError {
    #[error("tokenizer I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("tokenizer parse error: {0}")]
    Parse(String),
    #[error("operation not supported by this tokenizer backend: {0}")]
    Unsupported(&'static str),
}

/// The contract every tokenizer in LARQL implements.
///
/// Implementations MUST be `Send + Sync` so `Arc<dyn Tokenizer>` can be shared
/// across threads (e.g. the `DownMetaMmap` hot path + async HTTP handlers).
pub trait Tokenizer: Send + Sync {
    /// Encode text into token ids and their string pieces.
    ///
    /// `add_special` controls BOS/EOS injection, matching the HF semantic.
    fn encode(&self, text: &str, add_special: bool) -> Result<Encoded, TokenizerError>;

    /// Decode ids back to text. `skip_special` mirrors HF's skip_special_tokens.
    fn decode(&self, ids: &[u32], skip_special: bool) -> Result<String, TokenizerError>;

    /// Vocab lookups.
    fn id_to_token(&self, id: u32) -> Option<String>;
    fn token_to_id(&self, tok: &str) -> Option<u32>;

    /// Total vocab size.
    fn vocab_size(&self, with_special: bool) -> usize;

    /// Full vocab map. Default rebuilds it via `vocab_size` + `id_to_token`
    /// (O(V)); impls with a native call should override for speed.
    fn vocab(&self, with_special: bool) -> std::collections::HashMap<String, u32> {
        let mut out = std::collections::HashMap::new();
        let n = self.vocab_size(with_special);
        for id in 0..n as u32 {
            if let Some(tok) = self.id_to_token(id) {
                out.insert(tok, id);
            }
        }
        out
    }

    /// Serialize the tokenizer as `tokenizer.json`-compatible text. Used by
    /// the extract pipeline to bake a tokenizer into the vindex directory.
    ///
    /// Default impl returns [`TokenizerError::Unsupported`]; impls that can
    /// reconstruct the full HF format (i.e. [`HfTokenizer`]) override this.
    fn to_json(&self, _pretty: bool) -> Result<String, TokenizerError> {
        Err(TokenizerError::Unsupported(
            "this tokenizer backend cannot serialize to tokenizer.json",
        ))
    }

    /// Short identifier for diagnostics (`"hf"`, `"vocab-only"`, ...).
    fn backend_name(&self) -> &'static str;
}

/// Decode a single token ID to a trimmed string. Returns `None` if the decoded
/// form is empty after trimming — useful for skipping whitespace-only pieces.
pub fn decode_token(t: &dyn Tokenizer, id: u32) -> Option<String> {
    t.decode(&[id], true)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Decode a single token, falling back to the vocab entry (so BOS/EOS/PAD
/// still produce a readable label) and finally to `[id]`.
pub fn decode_token_raw(t: &dyn Tokenizer, id: u32) -> String {
    if let Some(s) = decode_token(t, id) {
        return s;
    }
    if let Some(s) = t.id_to_token(id) {
        return s;
    }
    format!("[{id}]")
}

/// Load the tokenizer at `path`. Uses [`HfTokenizer`] when the `hf` feature is
/// on (the workspace default), otherwise [`VocabOnlyTokenizer`].
pub fn load_tokenizer(path: &Path) -> Result<Arc<dyn Tokenizer>, TokenizerError> {
    #[cfg(feature = "hf")]
    {
        return HfTokenizer::from_file(path).map(|t| Arc::new(t) as Arc<dyn Tokenizer>);
    }
    #[cfg(not(feature = "hf"))]
    {
        return VocabOnlyTokenizer::from_file(path).map(|t| Arc::new(t) as Arc<dyn Tokenizer>);
    }
}

/// Construct an `Arc<dyn Tokenizer>` from an already-built concrete tokenizer.
pub fn into_dyn<T: Tokenizer + 'static>(t: T) -> Arc<dyn Tokenizer> {
    Arc::new(t)
}
