//! Prediction — logits computation and all predict_* entry points.

use super::embed::embed_tokens;
use super::layer::{
    run_attention, run_layer_decode_with_ffn, run_layer_with_capture, run_layer_with_ffn,
};
use super::ple::precompute_per_layer_inputs;
use super::{
    apply_norm, dot_proj, LayerAttentionCapture, LayerMode, PredictResult,
    PredictResultWithAttention, PredictResultWithResiduals, TokenPrediction,
};
use crate::attention::SharedKV;
use crate::ffn::{FfnBackend, LayerFfnRouter, WeightFfn};
use crate::model::ModelWeights;
use ndarray::Array2;

#[derive(Clone)]
pub struct IncrementalForwardState {
    seq_len: usize,
    kv_cache: std::collections::HashMap<usize, SharedKV>,
    final_hidden: Vec<f32>,
}

impl IncrementalForwardState {
    pub fn seq_len(&self) -> usize {
        self.seq_len
    }

    pub fn final_hidden(&self) -> &[f32] {
        &self.final_hidden
    }
}

/// Project the final hidden state to logits and return top-k predictions.
pub fn logits_to_predictions_pub(
    weights: &ModelWeights,
    h: &Array2<f32>,
    tokenizer: &dyn larql_tokenizer::Tokenizer,
    top_k: usize,
    temperature: f32,
) -> PredictResult {
    logits_to_predictions(weights, h, tokenizer, top_k, temperature)
}

fn logits_from_hidden(weights: &ModelWeights, h: &Array2<f32>, temperature: f32) -> Vec<f32> {
    let seq_len = h.shape()[0];
    let norm_offset = weights.arch.norm_weight_offset();
    let h_final = apply_norm(weights, h, weights.arch.final_norm_key(), norm_offset);

    let logits_scale = weights.arch.logits_scaling();
    let final_softcap = weights.arch.final_logit_softcapping();
    let last_2d = h_final.slice(ndarray::s![seq_len - 1..seq_len, ..]);
    let logits_raw = dot_proj(&last_2d, &weights.lm_head);
    let inv_scale = 1.0 / logits_scale;

    logits_raw
        .row(0)
        .iter()
        .map(|&v| {
            let mut logit = v * inv_scale;
            if let Some(cap) = final_softcap {
                logit = (logit / cap).tanh() * cap;
            }
            logit / temperature.max(1e-6)
        })
        .collect()
}

fn logits_to_token_predictions(
    logits: &[f32],
    tokenizer: &dyn larql_tokenizer::Tokenizer,
    top_k: usize,
) -> Vec<TokenPrediction> {
    if logits.is_empty() {
        return Vec::new();
    }

    let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp_sum: f64 = logits.iter().map(|l| ((l - max_logit) as f64).exp()).sum();
    let probs: Vec<f32> = logits
        .iter()
        .map(|l| (((l - max_logit) as f64).exp() / exp_sum) as f32)
        .collect();

    let mut indexed: Vec<(usize, f32)> = probs.iter().copied().enumerate().collect();
    let k = top_k.min(indexed.len()).max(1);
    indexed.select_nth_unstable_by(k - 1, |a, b| b.1.partial_cmp(&a.1).unwrap());
    indexed.truncate(k);
    indexed.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    indexed
        .into_iter()
        .filter_map(|(idx, prob)| {
            tokenizer
                .decode(&[idx as u32], true)
                .ok()
                .map(|s| TokenPrediction {
                    token_id: idx as u32,
                    text: s,
                    probability: prob as f64,
                })
        })
        .collect()
}

pub(super) fn logits_to_predictions(
    weights: &ModelWeights,
    h: &Array2<f32>,
    tokenizer: &dyn larql_tokenizer::Tokenizer,
    top_k: usize,
    temperature: f32,
) -> PredictResult {
    let logits = logits_from_hidden(weights, h, temperature);
    let predictions = logits_to_token_predictions(&logits, tokenizer, top_k)
        .into_iter()
        .map(|pred| (pred.text, pred.probability))
        .collect();

    PredictResult { predictions }
}

pub fn hidden_vec_to_token_predictions(
    weights: &ModelWeights,
    tokenizer: &dyn larql_tokenizer::Tokenizer,
    hidden: &[f32],
    top_k: usize,
) -> Vec<TokenPrediction> {
    let hidden_size = weights.hidden_size;
    if hidden.len() != hidden_size {
        return Vec::new();
    }

    let h = match Array2::from_shape_vec((1, hidden_size), hidden.to_vec()) {
        Ok(h) => h,
        Err(_) => return Vec::new(),
    };

    let logits = logits_from_hidden(weights, &h, 1.0);
    logits_to_token_predictions(&logits, tokenizer, top_k)
}

pub fn hidden_vec_token_probability(
    weights: &ModelWeights,
    hidden: &[f32],
    token_id: u32,
) -> Option<f64> {
    let hidden_size = weights.hidden_size;
    if hidden.len() != hidden_size {
        return None;
    }

    let h = Array2::from_shape_vec((1, hidden_size), hidden.to_vec()).ok()?;
    let logits = logits_from_hidden(weights, &h, 1.0);
    let token_idx = token_id as usize;
    if token_idx >= logits.len() {
        return None;
    }

    let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp_sum: f64 = logits.iter().map(|l| ((l - max_logit) as f64).exp()).sum();
    let prob = ((logits[token_idx] - max_logit) as f64).exp() / exp_sum;
    Some(prob)
}

/// Run a full forward pass and return the top-k next token predictions.
pub fn predict(
    weights: &ModelWeights,
    tokenizer: &dyn larql_tokenizer::Tokenizer,
    token_ids: &[u32],
    top_k: usize,
) -> PredictResult {
    predict_with_temperature(weights, tokenizer, token_ids, top_k, 1.0)
}

pub fn predict_with_temperature(
    weights: &ModelWeights,
    tokenizer: &dyn larql_tokenizer::Tokenizer,
    token_ids: &[u32],
    top_k: usize,
    temperature: f32,
) -> PredictResult {
    let ffn = WeightFfn { weights };
    let num_layers = weights.num_layers;
    let mut h = embed_tokens(weights, token_ids);
    let ple_inputs = precompute_per_layer_inputs(weights, &h, token_ids);
    let mut kv_cache: std::collections::HashMap<usize, SharedKV> = std::collections::HashMap::new();
    for layer in 0..num_layers {
        let shared_kv = weights
            .arch
            .kv_shared_source_layer(layer)
            .and_then(|src| kv_cache.get(&src));
        match run_layer_with_ffn(
            weights,
            &h,
            layer,
            &ffn,
            false,
            ple_inputs.get(layer),
            shared_kv,
        ) {
            Some((h_new, _, kv_out)) => {
                h = h_new;
                if let Some(kv) = kv_out {
                    kv_cache.insert(layer, kv);
                }
            }
            None => continue,
        }
    }
    logits_to_predictions(weights, &h, tokenizer, top_k, temperature)
}

/// Run a full forward pass with a custom FFN backend for all layers.
pub fn predict_with_ffn(
    weights: &ModelWeights,
    tokenizer: &dyn larql_tokenizer::Tokenizer,
    token_ids: &[u32],
    top_k: usize,
    ffn: &dyn FfnBackend,
) -> PredictResult {
    let num_layers = weights.num_layers;
    let mut h = embed_tokens(weights, token_ids);
    let ple_inputs = precompute_per_layer_inputs(weights, &h, token_ids);

    let mut kv_cache: std::collections::HashMap<usize, SharedKV> = std::collections::HashMap::new();

    for layer in 0..num_layers {
        let shared_kv = weights
            .arch
            .kv_shared_source_layer(layer)
            .and_then(|src| kv_cache.get(&src));

        match run_layer_with_ffn(
            weights,
            &h,
            layer,
            ffn,
            false,
            ple_inputs.get(layer),
            shared_kv,
        ) {
            Some((h_new, _, kv_out)) => {
                h = h_new;
                if let Some(kv) = kv_out {
                    kv_cache.insert(layer, kv);
                }
            }
            None => continue,
        }
    }

    logits_to_predictions(weights, &h, tokenizer, top_k, 1.0)
}

/// Run a full forward pass with a custom FFN backend, capturing attention weights
/// and per-layer residuals for logit lens.
pub fn predict_with_ffn_attention(
    weights: &ModelWeights,
    tokenizer: &dyn larql_tokenizer::Tokenizer,
    token_ids: &[u32],
    top_k: usize,
    ffn: &dyn FfnBackend,
) -> PredictResultWithAttention {
    let num_layers = weights.num_layers;
    let seq_len = token_ids.len();
    let mut h = embed_tokens(weights, token_ids);
    let ple_inputs = precompute_per_layer_inputs(weights, &h, token_ids);
    let mut attention = Vec::with_capacity(num_layers);
    let mut residuals = Vec::with_capacity(num_layers);
    let mut logit_lens = Vec::with_capacity(num_layers);
    let mut head_dla = Vec::with_capacity(num_layers);
    let mut kv_cache: std::collections::HashMap<usize, SharedKV> = std::collections::HashMap::new();

    for layer in 0..num_layers {
        let shared_kv = weights
            .arch
            .kv_shared_source_layer(layer)
            .and_then(|src| kv_cache.get(&src));
        match run_layer_with_capture(
            weights,
            &h,
            layer,
            ffn,
            false,
            true,
            ple_inputs.get(layer),
            shared_kv,
        ) {
            Some((h_new, _, attn_weights, kv_out, heads)) => {
                h = h_new;
                let residual = h.row(seq_len - 1).to_vec();
                residuals.push((layer, residual.clone()));

                if let Some(lens_pred) = logit_lens_top1(weights, tokenizer, &residual) {
                    logit_lens.push((layer, vec![lens_pred]));
                }

                if let Some(w) = attn_weights {
                    attention.push(LayerAttentionCapture { layer, weights: w });
                }

                if !heads.is_empty() {
                    head_dla.push((layer, heads));
                }

                if let Some(kv) = kv_out {
                    kv_cache.insert(layer, kv);
                }
            }
            None => continue,
        }
    }

    let result = logits_to_predictions(weights, &h, tokenizer, top_k, 1.0);
    PredictResultWithAttention {
        predictions: result.predictions,
        attention,
        residuals,
        logit_lens,
        head_dla,
        final_hidden: h.row(seq_len - 1).to_vec(),
    }
}

pub fn prepare_incremental_forward_state(
    weights: &ModelWeights,
    token_ids: &[u32],
    ffn: &dyn FfnBackend,
) -> Option<IncrementalForwardState> {
    if token_ids.is_empty() {
        return None;
    }

    let num_layers = weights.num_layers;
    let seq_len = token_ids.len();
    let mut h = embed_tokens(weights, token_ids);
    let ple_inputs = precompute_per_layer_inputs(weights, &h, token_ids);
    let mut kv_cache: std::collections::HashMap<usize, SharedKV> = std::collections::HashMap::new();

    for layer in 0..num_layers {
        let shared_kv = weights
            .arch
            .kv_shared_source_layer(layer)
            .and_then(|src| kv_cache.get(&src));
        let (h_new, _, kv_out) = run_layer_with_ffn(
            weights,
            &h,
            layer,
            ffn,
            false,
            ple_inputs.get(layer),
            shared_kv,
        )?;
        h = h_new;
        if let Some(kv) = kv_out {
            kv_cache.insert(layer, kv);
        }
    }

    Some(IncrementalForwardState {
        seq_len,
        kv_cache,
        final_hidden: h.row(seq_len - 1).to_vec(),
    })
}

pub fn incremental_forward_append_token(
    weights: &ModelWeights,
    state: &IncrementalForwardState,
    token_id: u32,
    ffn: &dyn FfnBackend,
) -> Option<IncrementalForwardState> {
    let mut h = embed_tokens(weights, &[token_id]);
    let ple_inputs = precompute_per_layer_inputs(weights, &h, &[token_id]);
    let mut kv_cache: std::collections::HashMap<usize, SharedKV> = std::collections::HashMap::new();

    for layer in 0..weights.num_layers {
        let shared_kv = weights
            .arch
            .kv_shared_source_layer(layer)
            .and_then(|src| kv_cache.get(&src));
        let prior_kv = if shared_kv.is_none() {
            state.kv_cache.get(&layer)
        } else {
            None
        };
        let shared_kv = shared_kv.or_else(|| {
            weights
                .arch
                .kv_shared_source_layer(layer)
                .and_then(|src| state.kv_cache.get(&src))
        });

        let (h_new, _, kv_out) = run_layer_decode_with_ffn(
            weights,
            &h,
            layer,
            state.seq_len,
            ffn,
            false,
            ple_inputs.get(layer),
            prior_kv,
            shared_kv,
        )?;
        h = h_new;
        if let Some(kv) = kv_out {
            kv_cache.insert(layer, kv);
        }
    }

    Some(IncrementalForwardState {
        seq_len: state.seq_len + 1,
        kv_cache,
        final_hidden: h.row(0).to_vec(),
    })
}

/// Project a single residual vector through final norm + lm_head to get top-1 prediction.
pub fn logit_lens_top1(
    weights: &ModelWeights,
    tokenizer: &dyn larql_tokenizer::Tokenizer,
    residual: &[f32],
) -> Option<(String, f64)> {
    let hidden = weights.hidden_size;
    if residual.len() != hidden {
        return None;
    }

    let h = Array2::from_shape_vec((1, hidden), residual.to_vec()).ok()?;
    let result = logits_to_predictions(weights, &h, tokenizer, 1, 1.0);
    result.predictions.into_iter().next()
}

/// Forward pass with residual capture — predictions + per-layer residuals.
pub fn predict_with_ffn_trace(
    weights: &ModelWeights,
    tokenizer: &dyn larql_tokenizer::Tokenizer,
    token_ids: &[u32],
    top_k: usize,
    ffn: &dyn FfnBackend,
) -> PredictResultWithResiduals {
    let num_layers = weights.num_layers;
    let mut h = embed_tokens(weights, token_ids);
    let ple_inputs = precompute_per_layer_inputs(weights, &h, token_ids);
    let mut residuals = Vec::with_capacity(num_layers);

    for layer in 0..num_layers {
        let last_pos = h.shape()[0] - 1;
        residuals.push(h.row(last_pos).to_vec());

        h = match run_layer_with_ffn(weights, &h, layer, ffn, false, ple_inputs.get(layer), None) {
            Some((h_new, _, _)) => h_new,
            None => continue,
        };
    }

    let result = logits_to_predictions(weights, &h, tokenizer, top_k, 1.0);
    PredictResultWithResiduals {
        predictions: result.predictions,
        residuals,
    }
}

/// Run a full forward pass with per-layer FFN backend selection.
pub fn predict_with_router(
    weights: &ModelWeights,
    tokenizer: &dyn larql_tokenizer::Tokenizer,
    token_ids: &[u32],
    top_k: usize,
    router: &LayerFfnRouter,
) -> PredictResult {
    let num_layers = weights.num_layers;
    let mut h = embed_tokens(weights, token_ids);
    let ple_inputs = precompute_per_layer_inputs(weights, &h, token_ids);

    for layer in 0..num_layers {
        let ffn = router.get(layer);
        h = match run_layer_with_ffn(weights, &h, layer, ffn, false, ple_inputs.get(layer), None) {
            Some((h_new, _, _)) => h_new,
            None => continue,
        };
    }

    logits_to_predictions(weights, &h, tokenizer, top_k, 1.0)
}

/// Run a forward pass with per-layer strategy: full compute or scalar gain bypass.
pub fn predict_with_strategy(
    weights: &ModelWeights,
    tokenizer: &dyn larql_tokenizer::Tokenizer,
    token_ids: &[u32],
    top_k: usize,
    strategy: &[LayerMode],
) -> PredictResult {
    let num_layers = weights.num_layers;
    let mut h = embed_tokens(weights, token_ids);
    let ple_inputs = precompute_per_layer_inputs(weights, &h, token_ids);

    for (layer, mode) in strategy.iter().enumerate().take(num_layers) {
        match mode {
            LayerMode::Compute(ffn) => {
                h = match run_layer_with_ffn(
                    weights,
                    &h,
                    layer,
                    *ffn,
                    false,
                    ple_inputs.get(layer),
                    None,
                ) {
                    Some((h_new, _, _)) => h_new,
                    None => continue,
                };
            }
            LayerMode::ScalarGain(gain) => {
                h *= *gain;
            }
            LayerMode::AttentionOnly => {
                if let Some(h_post_attn) = run_attention(weights, &h, layer) {
                    h = h_post_attn;
                }
            }
        }
    }

    logits_to_predictions(weights, &h, tokenizer, top_k, 1.0)
}

/// Resume a forward pass from a pre-computed hidden state.
pub fn predict_from_hidden(
    weights: &ModelWeights,
    tokenizer: &dyn larql_tokenizer::Tokenizer,
    h_init: &Array2<f32>,
    start_layer: usize,
    top_k: usize,
) -> PredictResult {
    let ffn = WeightFfn { weights };
    predict_from_hidden_with_ffn(weights, tokenizer, h_init, start_layer, top_k, &ffn, &[])
}

/// Resume a forward pass from a pre-computed hidden state with a custom FFN backend.
pub fn predict_from_hidden_with_ffn(
    weights: &ModelWeights,
    tokenizer: &dyn larql_tokenizer::Tokenizer,
    h_init: &Array2<f32>,
    start_layer: usize,
    top_k: usize,
    ffn: &dyn FfnBackend,
    token_ids: &[u32],
) -> PredictResult {
    let num_layers = weights.num_layers;
    let mut h = h_init.clone();
    let ple_inputs: Vec<Array2<f32>> = if token_ids.is_empty() {
        Vec::new()
    } else {
        let embeds = embed_tokens(weights, token_ids);
        precompute_per_layer_inputs(weights, &embeds, token_ids)
    };

    for layer in start_layer..num_layers {
        h = match run_layer_with_ffn(weights, &h, layer, ffn, false, ple_inputs.get(layer), None) {
            Some((h_new, _, _)) => h_new,
            None => continue,
        };
    }

    logits_to_predictions(weights, &h, tokenizer, top_k, 1.0)
}

#[cfg(test)]
mod tests {
    use super::{incremental_forward_append_token, prepare_incremental_forward_state};
    use crate::ffn::HighwayFfn;
    use larql_models::{GenericArch, ModelConfig, ModelWeights};
    use ndarray::{ArcArray2, Array2};
    use std::collections::HashMap;

    fn shared(matrix: [[f32; 2]; 2]) -> ArcArray2<f32> {
        Array2::from_shape_vec((2, 2), matrix.into_iter().flatten().collect())
            .unwrap()
            .into_shared()
    }

    fn build_test_weights() -> ModelWeights {
        let config = ModelConfig {
            model_type: "generic".to_string(),
            num_layers: 1,
            hidden_size: 2,
            intermediate_size: 2,
            head_dim: 2,
            num_q_heads: 1,
            num_kv_heads: 1,
            vocab_size: Some(4),
            rope_base: 10_000.0,
            rope_local_base: None,
            sliding_window: None,
            num_experts: None,
            num_experts_per_token: None,
            num_shared_experts: None,
            kv_lora_rank: None,
            q_lora_rank: None,
            rope_scaling: None,
            attn_logit_softcapping: None,
            final_logit_softcapping: None,
            query_pre_attn_scalar: None,
            embedding_multiplier: None,
            residual_multiplier: None,
            attention_multiplier: None,
            logits_scaling: None,
            global_head_dim: None,
            num_global_kv_heads: None,
            partial_rotary_factor: None,
            sliding_window_pattern: None,
            layer_types: None,
            attention_k_eq_v: false,
            per_layer_embed_dim: None,
            num_kv_shared_layers: None,
        };

        let mut tensors = HashMap::new();
        tensors.insert(
            "layers.0.self_attn.q_proj.weight".to_string(),
            shared([[1.0, 0.0], [0.0, 1.0]]),
        );
        tensors.insert(
            "layers.0.self_attn.k_proj.weight".to_string(),
            shared([[0.7, 0.2], [0.1, 0.9]]),
        );
        tensors.insert(
            "layers.0.self_attn.v_proj.weight".to_string(),
            shared([[0.6, -0.1], [0.3, 0.8]]),
        );
        tensors.insert(
            "layers.0.self_attn.o_proj.weight".to_string(),
            shared([[0.9, 0.2], [-0.3, 0.7]]),
        );

        let mut vectors = HashMap::new();
        vectors.insert(
            "layers.0.input_layernorm.weight".to_string(),
            vec![1.0, 1.0],
        );
        vectors.insert(
            "layers.0.post_attention_layernorm.weight".to_string(),
            vec![1.0, 1.0],
        );
        vectors.insert("norm.weight".to_string(), vec![1.0, 1.0]);

        let embed = Array2::from_shape_vec(
            (4, 2),
            vec![
                0.1, 0.2, //
                0.3, -0.4, //
                -0.2, 0.5, //
                0.7, 0.1,
            ],
        )
        .unwrap()
        .into_shared();
        let lm_head = embed.clone();

        ModelWeights {
            tensors,
            vectors,
            embed,
            lm_head,
            arch: Box::new(GenericArch::from_config(config)),
            num_layers: 1,
            hidden_size: 2,
            intermediate_size: 2,
            vocab_size: 4,
            head_dim: 2,
            num_q_heads: 1,
            num_kv_heads: 1,
            rope_base: 10_000.0,
        }
    }

    #[test]
    fn incremental_forward_append_matches_full_recompute() {
        let weights = build_test_weights();
        let ffn = HighwayFfn;
        let prompt_state = prepare_incremental_forward_state(&weights, &[0, 1], &ffn).unwrap();
        let appended_state =
            incremental_forward_append_token(&weights, &prompt_state, 2, &ffn).unwrap();
        let full_state = prepare_incremental_forward_state(&weights, &[0, 1, 2], &ffn).unwrap();

        assert_eq!(appended_state.seq_len(), full_state.seq_len());
        assert_eq!(
            appended_state.final_hidden().len(),
            full_state.final_hidden().len()
        );
        for (lhs, rhs) in appended_state
            .final_hidden()
            .iter()
            .zip(full_state.final_hidden().iter())
        {
            assert!((lhs - rhs).abs() < 1e-5, "{lhs} vs {rhs}");
        }
    }
}
