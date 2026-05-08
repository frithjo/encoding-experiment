//! Attention computation — RoPE, GQA, causal masking, GPU dispatch.
//!
//! Submodules:
//! - `rope`: Rotary Position Embeddings (full and partial rotation)
//! - `gqa`: Grouped-Query Attention with BLAS-fused dot products
//! - `block`: CPU attention block (norm → proj → RoPE → GQA → O → residual)
//! - `gpu`: GPU-accelerated attention, KV-capture, Q4 projection

pub mod block;
pub mod gpu;
pub mod gqa;
pub mod rope;
pub mod separated;

use ndarray::Array2;

/// Per-head attention weights for the last token position.
pub struct AttentionWeights {
    /// Per-head attention distribution for the last sequence position.
    /// `heads[h][j]` = attention weight from last token to position j.
    pub heads: Vec<Vec<f32>>,
}

/// Shared KV pair: post-RoPE K and post-V-norm V from a source layer.
pub type SharedKV = (Array2<f32>, Array2<f32>);

// ── Re-exports: preserve `crate::attention::*` paths ──

pub use block::{
    run_attention_block, run_attention_block_core, run_attention_block_shared,
    run_attention_block_with_kv_out,
};
pub use gpu::{
    q4_attention_proj, run_attention_block_gpu, run_attention_with_kv,
    run_attention_with_kv_backend,
};
pub use gqa::{gqa_attention, gqa_attention_with_weights};
pub use rope::{apply_rope, apply_rope_partial};
pub use separated::{
    bind_separated_attention_listener, request_separated_attention,
    run_attention_block_via_separated_runtime, run_separated_attention_capture,
    run_separated_attention_request, separated_attention_request_capture,
    separated_attention_request_wire_bytes, serve_separated_attention, AttentionTensor2,
    SeparatedAttentionBlockOutput, SeparatedAttentionReady, SeparatedAttentionRequest,
    SeparatedAttentionResponse, SeparatedAttentionRuntimeError,
    SEPARATED_ATTENTION_ERROR_CAPSULE_KIND, SEPARATED_ATTENTION_ERROR_CAPTURE_KIND,
    SEPARATED_ATTENTION_READY_CAPSULE_KIND, SEPARATED_ATTENTION_READY_CAPTURE_KIND,
    SEPARATED_ATTENTION_REQUEST_CAPTURE_KIND, SEPARATED_ATTENTION_RESPONSE_CAPSULE_KIND,
    SEPARATED_ATTENTION_RESPONSE_CAPTURE_KIND, SEPARATED_ATTENTION_RUNTIME_PRODUCER,
};
