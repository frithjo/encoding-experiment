//! Full transformer forward pass.
//!
//! Runs tokens through embedding → layers → final norm → logits.
//! Uses the ModelArchitecture trait for model-specific behavior
//! and FfnBackend trait for swappable FFN computation.
//!
//! Submodules:
//! - `embed`: Token embedding with architecture-specific scaling
//! - `ple`: Per-Layer Embeddings (gated per-layer token embeddings)
//! - `layer`: Single-layer dispatch (attention + FFN + PLE + scalar)
//! - `predict`: Logits computation and all predict_* entry points
//! - `trace`: Residual/activation capture and calibration

pub mod embed;
pub mod layer;
pub mod memit;
pub mod ple;
pub mod predict;
pub mod trace;

use crate::attention::AttentionWeights;
use crate::ffn::FfnBackend;
use crate::model::ModelWeights;
use crate::residual::rms_norm;
use larql_models::NormType;
use ndarray::Array2;

// ── Types ──

/// Per-head attention pattern for the last token at one layer.
pub struct LayerAttentionCapture {
    pub layer: usize,
    pub weights: AttentionWeights,
}

/// Result of a forward trace — residuals and optional sparse activations.
pub struct TraceResult {
    pub residuals: Vec<(usize, Vec<f32>)>,
    pub activations: Vec<(usize, Vec<(usize, f32)>)>,
    pub attention: Vec<LayerAttentionCapture>,
}

/// Prediction result from a full forward pass.
pub struct PredictResult {
    pub predictions: Vec<(String, f64)>,
}

#[derive(Clone, Debug)]
pub struct TokenPrediction {
    pub token_id: u32,
    pub text: String,
    pub probability: f64,
}

/// Prediction result with per-layer residual capture.
pub struct PredictResultWithResiduals {
    pub predictions: Vec<(String, f64)>,
    pub residuals: Vec<Vec<f32>>,
}

/// Prediction result with per-layer attention captures and logit lens.
pub struct PredictResultWithAttention {
    pub predictions: Vec<(String, f64)>,
    pub attention: Vec<LayerAttentionCapture>,
    pub residuals: Vec<(usize, Vec<f32>)>,
    pub logit_lens: Vec<(usize, Vec<(String, f64)>)>,
    pub head_dla: Vec<(usize, Vec<Vec<f32>>)>, // (layer, head_index -> DLA_vector)
    pub final_hidden: Vec<f32>,
}

/// Per-layer computation strategy.
pub enum LayerMode<'a> {
    Compute(&'a dyn FfnBackend),
    ScalarGain(f32),
    AttentionOnly,
}

// ── Utilities ──

/// Apply the appropriate norm (RMSNorm or LayerNorm) based on architecture.
pub fn apply_norm(
    weights: &ModelWeights,
    x: &Array2<f32>,
    weight_key: &str,
    norm_offset: f32,
) -> Array2<f32> {
    match weights.arch.norm_type() {
        NormType::LayerNorm => {
            let bias_key = weight_key.replace(".weight", ".bias");
            crate::residual::layer_norm(
                x,
                weights.vectors.get(weight_key),
                weights.vectors.get(&bias_key),
            )
        }
        _ => rms_norm(x, weights.vectors.get(weight_key), norm_offset),
    }
}

/// Compute x @ w.T via BLAS.
pub fn dot_proj(
    x: &ndarray::ArrayBase<impl ndarray::Data<Elem = f32>, ndarray::Ix2>,
    w: &ndarray::ArrayBase<impl ndarray::Data<Elem = f32>, ndarray::Ix2>,
) -> Array2<f32> {
    x.dot(&w.t())
}

/// Add a 1D bias vector to each row of a 2D matrix.
pub fn add_bias(x: &mut Array2<f32>, bias: &[f32]) {
    let cols = x.shape()[1];
    let n = cols.min(bias.len());
    for mut row in x.rows_mut() {
        for j in 0..n {
            row[j] += bias[j];
        }
    }
}

// ── Re-exports: preserve all `crate::forward::*` paths ──

pub use embed::embed_tokens_pub;
pub use layer::{run_attention_public, run_ffn};
pub use memit::{run_memit, MemitFact, MemitFactResult, MemitResult};
pub use predict::{
    hidden_vec_to_token_predictions, hidden_vec_token_probability,
    incremental_forward_append_token, logit_lens_top1, logits_to_predictions_pub, predict,
    predict_from_hidden, predict_from_hidden_with_ffn, predict_with_ffn,
    predict_with_ffn_attention, predict_with_ffn_trace, predict_with_router, predict_with_strategy,
    predict_with_temperature, prepare_incremental_forward_state, IncrementalForwardState,
};
pub use trace::{
    calibrate_scalar_gains, capture_decoy_residuals, capture_ffn_activation_matrix,
    capture_residuals, estimate_ffn_covariance, forward_to_layer, trace_forward,
    trace_forward_full, trace_forward_with_ffn,
};
