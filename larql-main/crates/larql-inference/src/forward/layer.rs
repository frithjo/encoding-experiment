//! Layer dispatch — runs attention + FFN + PLE + layer_scalar for a single layer.
//!
//! Orchestrates the per-layer computation: attention (with optional KV sharing),
//! FFN, per-layer embeddings, and layer scalar multiplication.

use super::apply_norm;
use super::ple::apply_per_layer_embedding;
use crate::attention::{AttentionWeights, SharedKV};
use crate::ffn::FfnBackend;
use crate::model::ModelWeights;
use crate::residual::rms_norm;
use ndarray::{Array2, Axis};

/// Public wrapper for run_attention (used by CachedFfn calibration).
pub fn run_attention_public(
    weights: &ModelWeights,
    h: &Array2<f32>,
    layer: usize,
) -> Option<Array2<f32>> {
    run_attention(weights, h, layer)
}

/// Run attention for a single layer. Returns the post-attention residual.
pub(super) fn run_attention(
    weights: &ModelWeights,
    h: &Array2<f32>,
    layer: usize,
) -> Option<Array2<f32>> {
    let (h_post_attn, _, _) = run_attention_inner(weights, h, layer, false, None)?;
    Some(h_post_attn)
}

/// Run attention with optional per-head weight capture and shared K/V.
pub(super) fn run_attention_inner(
    weights: &ModelWeights,
    h: &Array2<f32>,
    layer: usize,
    capture_attention: bool,
    shared_kv: Option<&SharedKV>,
) -> Option<(Array2<f32>, Option<AttentionWeights>, Vec<Vec<f32>>)> {
    let (h_post_attn, _attn_projected, attn_weights, heads) =
        crate::attention::run_attention_block_shared(
            weights,
            h,
            layer,
            capture_attention,
            shared_kv,
        )?;
    Some((h_post_attn, attn_weights, heads))
}

/// Run attention returning post-processed K/V for caching (KV sharing source layers).
pub(super) fn run_attention_with_kv_cache(
    weights: &ModelWeights,
    h: &Array2<f32>,
    layer: usize,
) -> Option<(Array2<f32>, SharedKV)> {
    let (h_post_attn, _, _, k_rope, v_final, _) =
        crate::attention::run_attention_block_core(weights, h, layer, false, None)?;
    Some((h_post_attn, (k_rope, v_final)))
}

/// Run FFN for a single layer using the given backend. Returns the post-FFN residual.
pub fn run_ffn(
    weights: &ModelWeights,
    h_post_attn: &Array2<f32>,
    layer: usize,
    ffn: &dyn FfnBackend,
    capture_activation: bool,
) -> (Array2<f32>, Option<Array2<f32>>) {
    let norm_offset = weights.arch.norm_weight_offset();
    let arch = &*weights.arch;

    let pre_ffn_key = if arch.has_post_norms() {
        arch.pre_feedforward_layernorm_key(layer)
    } else {
        Some(arch.post_attention_layernorm_key(layer))
    };
    let h_ffn = match pre_ffn_key {
        Some(key) => apply_norm(weights, h_post_attn, &key, norm_offset),
        None => rms_norm(h_post_attn, None, norm_offset),
    };

    let (ffn_out, activation) = if capture_activation {
        let (out, act) = ffn.forward_with_activation(layer, &h_ffn);
        (out, Some(act))
    } else {
        (ffn.forward(layer, &h_ffn), None)
    };

    let res_mult = arch.residual_multiplier();
    let h_out = if arch.has_post_norms() {
        let normed = match arch.post_feedforward_layernorm_key(layer) {
            Some(key) => apply_norm(weights, &ffn_out, &key, norm_offset),
            None => rms_norm(&ffn_out, None, norm_offset),
        };
        if res_mult != 1.0 {
            h_post_attn + &(&normed * res_mult)
        } else {
            h_post_attn + &normed
        }
    } else if res_mult != 1.0 {
        h_post_attn + &(&ffn_out * res_mult)
    } else {
        h_post_attn + &ffn_out
    };

    (h_out, activation)
}

/// Apply per-layer scalar multiplier if present (e.g., Gemma 4 layer_scalar).
pub(super) fn apply_layer_scalar(weights: &ModelWeights, h: &mut Array2<f32>, layer: usize) {
    if let Some(key) = weights.arch.layer_scalar_key(layer) {
        if let Some(scalars) = weights.vectors.get(&key) {
            if let Some(&scalar) = scalars.first() {
                if scalar != 1.0 {
                    *h *= scalar;
                }
            }
        }
    }
}

/// Run a single transformer layer with the given FFN backend.
#[allow(clippy::type_complexity)]
pub(super) fn run_layer_with_ffn(
    weights: &ModelWeights,
    h: &Array2<f32>,
    layer: usize,
    ffn: &dyn FfnBackend,
    capture_activation: bool,
    ple_input: Option<&Array2<f32>>,
    shared_kv: Option<&SharedKV>,
) -> Option<(Array2<f32>, Option<Array2<f32>>, Option<SharedKV>)> {
    let (h_post_attn, kv_out) = if shared_kv.is_some() {
        (
            run_attention_inner(weights, h, layer, false, shared_kv)?.0,
            None,
        )
    } else {
        let (h_pa, kv) = run_attention_with_kv_cache(weights, h, layer)?;
        (h_pa, Some(kv))
    };
    let (h_post_ffn, activation) = run_ffn(weights, &h_post_attn, layer, ffn, capture_activation);
    let mut h_out = apply_per_layer_embedding(weights, &h_post_ffn, layer, ple_input);
    apply_layer_scalar(weights, &mut h_out, layer);
    Some((h_out, activation, kv_out))
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub(super) fn run_layer_decode_with_ffn(
    weights: &ModelWeights,
    h: &Array2<f32>,
    layer: usize,
    position: usize,
    ffn: &dyn FfnBackend,
    capture_activation: bool,
    ple_input: Option<&Array2<f32>>,
    prior_kv: Option<&SharedKV>,
    shared_kv: Option<&SharedKV>,
) -> Option<(Array2<f32>, Option<Array2<f32>>, Option<SharedKV>)> {
    use crate::attention::gqa::gqa_attention_last_token_with_weights;
    use crate::attention::rope::apply_rope_partial_with_offset;
    use crate::forward::{add_bias, dot_proj};
    use crate::residual::{rms_norm_heads, rms_norm_heads_no_weight};

    let arch = &*weights.arch;
    let head_dim = arch.head_dim_for_layer(layer);
    let num_q = arch.num_q_heads_for_layer(layer);
    let num_kv = arch.num_kv_heads_for_layer(layer);
    let reps = num_q / num_kv;
    let scale = if arch.attention_multiplier() != 1.0 {
        arch.attention_multiplier() as f64
    } else {
        arch.attention_scale_for_layer(layer)
    };
    let norm_offset = arch.norm_weight_offset();
    let qk_offset = weights.arch.qk_norm_weight_offset();
    let qk_norm_off = if qk_offset != 0.0 {
        qk_offset
    } else {
        norm_offset
    };

    let h_norm =
        crate::forward::apply_norm(weights, h, &arch.input_layernorm_key(layer), norm_offset);

    let w_q = weights.tensors.get(&arch.attn_q_key(layer))?;
    let w_o = weights.tensors.get(&arch.attn_o_key(layer))?;
    let mut q_full = dot_proj(&h_norm, w_q);
    if let Some(bias) = arch
        .attn_q_bias_key(layer)
        .and_then(|k| weights.vectors.get(&k))
    {
        add_bias(&mut q_full, bias);
    }
    let q_normed = match arch
        .attn_q_norm_key(layer)
        .and_then(|k| weights.vectors.get(&k))
    {
        Some(norm_w) => rms_norm_heads(&q_full, norm_w, num_q, head_dim, qk_norm_off),
        None => q_full,
    };
    let layer_rope_base = arch.rope_base_for_layer(layer);
    let rotary_frac = arch.rotary_fraction_for_layer(layer);
    let q_rope = apply_rope_partial_with_offset(
        &q_normed,
        num_q,
        head_dim,
        layer_rope_base,
        rotary_frac,
        position,
    );

    let (k_full, v_full, kv_out, seq_len) = if let Some((cached_k, cached_v)) = shared_kv {
        (
            cached_k.clone(),
            cached_v.clone(),
            None,
            cached_k.shape()[0],
        )
    } else {
        let w_k = weights.tensors.get(&arch.attn_k_key(layer))?;
        let v_from_k = !weights.tensors.contains_key(&arch.attn_v_key(layer));
        let w_v = if v_from_k {
            w_k
        } else {
            weights.tensors.get(&arch.attn_v_key(layer))?
        };

        let mut k_proj = dot_proj(&h_norm, w_k);
        let mut v_proj = dot_proj(&h_norm, w_v);

        if let Some(bias) = arch
            .attn_k_bias_key(layer)
            .and_then(|k| weights.vectors.get(&k))
        {
            add_bias(&mut k_proj, bias);
        }
        if let Some(bias) = arch
            .attn_v_bias_key(layer)
            .and_then(|k| weights.vectors.get(&k))
        {
            add_bias(&mut v_proj, bias);
        }

        if arch.has_v_norm() {
            v_proj = rms_norm_heads_no_weight(&v_proj, num_kv, head_dim);
        }

        let k_normed = match arch
            .attn_k_norm_key(layer)
            .and_then(|k| weights.vectors.get(&k))
        {
            Some(norm_w) => rms_norm_heads(&k_proj, norm_w, num_kv, head_dim, qk_norm_off),
            None => k_proj,
        };
        let k_rope = apply_rope_partial_with_offset(
            &k_normed,
            num_kv,
            head_dim,
            layer_rope_base,
            rotary_frac,
            position,
        );

        let merged_k = if let Some((prior_k, _)) = prior_kv {
            ndarray::concatenate(Axis(0), &[prior_k.view(), k_rope.view()]).ok()?
        } else {
            k_rope.clone()
        };
        let merged_v = if let Some((_, prior_v)) = prior_kv {
            ndarray::concatenate(Axis(0), &[prior_v.view(), v_proj.view()]).ok()?
        } else {
            v_proj.clone()
        };
        let seq_len = merged_k.shape()[0];
        (
            merged_k.clone(),
            merged_v.clone(),
            Some((merged_k, merged_v)),
            seq_len,
        )
    };

    let softcap = arch.attn_logit_softcapping();
    let (attn_out, _) = gqa_attention_last_token_with_weights(
        &q_rope, &k_full, &v_full, num_q, head_dim, reps, scale, seq_len, false, softcap,
    );

    let mut attn_projected = dot_proj(&attn_out, w_o);
    if let Some(bias) = arch
        .attn_o_bias_key(layer)
        .and_then(|k| weights.vectors.get(&k))
    {
        add_bias(&mut attn_projected, bias);
    }

    let res_mult = arch.residual_multiplier();
    let h_post_attn = if arch.has_post_norms() {
        let normed = crate::forward::apply_norm(
            weights,
            &attn_projected,
            &arch.post_attention_layernorm_key(layer),
            norm_offset,
        );
        if res_mult != 1.0 {
            h + &(&normed * res_mult)
        } else {
            h + &normed
        }
    } else if res_mult != 1.0 {
        h + &(&attn_projected * res_mult)
    } else {
        h + &attn_projected
    };

    let (h_post_ffn, activation) = run_ffn(weights, &h_post_attn, layer, ffn, capture_activation);
    let mut h_out = apply_per_layer_embedding(weights, &h_post_ffn, layer, ple_input);
    apply_layer_scalar(weights, &mut h_out, layer);
    Some((h_out, activation, kv_out))
}

/// Run a single transformer layer, optionally capturing attention weights.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub(super) fn run_layer_with_capture(
    weights: &ModelWeights,
    h: &Array2<f32>,
    layer: usize,
    ffn: &dyn FfnBackend,
    capture_activation: bool,
    capture_attention: bool,
    ple_input: Option<&Array2<f32>>,
    shared_kv: Option<&SharedKV>,
) -> Option<(
    Array2<f32>,
    Option<Array2<f32>>,
    Option<AttentionWeights>,
    Option<SharedKV>,
    Vec<Vec<f32>>,
)> {
    let (h_post_attn, attn_weights, kv_out, head_projections) = if shared_kv.is_some() {
        let (h_post_attn, attn_weights, head_projections) =
            run_attention_inner(weights, h, layer, capture_attention, shared_kv)?;
        (h_post_attn, attn_weights, None, head_projections)
    } else {
        let (h_post_attn, _, attn_weights, k_rope, v_final, head_projections) =
            crate::attention::run_attention_block_with_kv_out(
                weights,
                h,
                layer,
                capture_attention,
                None,
            )?;
        (
            h_post_attn,
            attn_weights,
            Some((k_rope, v_final)),
            head_projections,
        )
    };
    let (h_post_ffn, activation) = run_ffn(weights, &h_post_attn, layer, ffn, capture_activation);
    let mut h_out = apply_per_layer_embedding(weights, &h_post_ffn, layer, ple_input);
    apply_layer_scalar(weights, &mut h_out, layer);
    Some((h_out, activation, attn_weights, kv_out, head_projections))
}
