//! GPU prefill pipeline: full Q4 inference for seq>1 with KV cache population.
//!
//! Like full_pipeline.rs but:
//! 1. Buffers sized for seq_len positions (not just 1)
//! 2. Per-position Q4_K/Q6_K projection dispatch within one command buffer
//! 3. RoPE applied separately to K on GPU
//! 4. K/V copied to KV cache on GPU (blit)
//! 5. Fused attention called with skip_rope=1 (Q and K pre-RoPE'd)

use metal::*;
use std::ffi::c_void;

use super::full_pipeline::{encode_residual_add, encode_rms_norm};
use super::q4_common::Q4Pipelines;
use crate::metal::buffers::BufferCache;
use crate::metal::shaders::q4_matvec as q4mv_shader;

/// Encode a quant matvec for a single position at the given offsets.
/// The input buffer is read from `in_offset` bytes, output written to `out_offset` bytes.
#[allow(clippy::too_many_arguments)]
fn encode_quant_matvec_at_offset(
    enc: &ComputeCommandEncoderRef,
    format: crate::QuantFormat,
    q4_pipeline: &ComputePipelineState,
    q8_pipeline: &ComputePipelineState,
    q4k_pipeline: &ComputePipelineState,
    q6k_pipeline: &ComputePipelineState,
    buf_w: &Buffer,
    buf_input: &Buffer,
    in_offset: u64,
    buf_out: &Buffer,
    out_offset: u64,
    num_rows: usize,
    hidden: usize,
) {
    match format {
        crate::QuantFormat::Q4_K | crate::QuantFormat::Q4_KF => {
            let n = num_rows as u32;
            let k = hidden as u32;
            let tgs = (num_rows as u64).div_ceil(4);
            enc.set_compute_pipeline_state(q4k_pipeline);
            enc.set_buffer(0, Some(buf_w), 0);
            enc.set_buffer(1, Some(buf_input), in_offset);
            enc.set_buffer(2, Some(buf_out), out_offset);
            enc.set_bytes(3, 4, &n as *const u32 as *const c_void);
            enc.set_bytes(4, 4, &k as *const u32 as *const c_void);
            enc.dispatch_thread_groups(MTLSize::new(tgs, 1, 1), MTLSize::new(128, 1, 1));
        }
        crate::QuantFormat::Q6_K => {
            let n = num_rows as u32;
            let k = hidden as u32;
            let tgs = (num_rows as u64).div_ceil(4);
            enc.set_compute_pipeline_state(q6k_pipeline);
            enc.set_buffer(0, Some(buf_w), 0);
            enc.set_buffer(1, Some(buf_input), in_offset);
            enc.set_buffer(2, Some(buf_out), out_offset);
            enc.set_bytes(3, 4, &n as *const u32 as *const c_void);
            enc.set_bytes(4, 4, &k as *const u32 as *const c_void);
            enc.dispatch_thread_groups(MTLSize::new(tgs, 1, 1), MTLSize::new(128, 1, 1));
        }
        crate::QuantFormat::Q4_0 => {
            let n = num_rows as u32;
            let k = hidden as u32;
            let num_tgs = (num_rows as u64).div_ceil(q4mv_shader::ROWS_PER_TG);
            enc.set_compute_pipeline_state(q4_pipeline);
            enc.set_buffer(0, Some(buf_w), 0);
            enc.set_buffer(1, Some(buf_input), in_offset);
            enc.set_buffer(2, Some(buf_out), out_offset);
            enc.set_bytes(3, 4, &n as *const u32 as *const c_void);
            enc.set_bytes(4, 4, &k as *const u32 as *const c_void);
            enc.dispatch_thread_groups(
                MTLSize::new(num_tgs, 1, 1),
                MTLSize::new(q4mv_shader::THREADS_PER_TG, 1, 1),
            );
        }
        crate::QuantFormat::Q8_0 => {
            let n = num_rows as u32;
            let k = hidden as u32;
            enc.set_compute_pipeline_state(q8_pipeline);
            enc.set_buffer(0, Some(buf_w), 0);
            enc.set_buffer(1, Some(buf_input), in_offset);
            enc.set_buffer(2, Some(buf_out), out_offset);
            enc.set_bytes(3, 4, &n as *const u32 as *const c_void);
            enc.set_bytes(4, 4, &k as *const u32 as *const c_void);
            enc.dispatch_thread_groups(
                MTLSize::new((num_rows as u64).div_ceil(8), 1, 1),
                MTLSize::new(256, 1, 1),
            );
        }
    }
}

/// Run the prefill pipeline: process seq_len>1 tokens through all layers on GPU,
/// populating the KV cache for subsequent decode.
///
/// Returns the final hidden state [seq_len * hidden] as a flat f32 vector.
#[allow(clippy::too_many_arguments)]
pub fn dispatch_prefill(
    queue: &CommandQueue,
    bufs: &BufferCache,
    q4: &Q4Pipelines,
    geglu_pipeline: &ComputePipelineState,
    _q8_quant_pipeline: &ComputePipelineState,
    fused_attn_pipeline: &ComputePipelineState,
    q8_matvec_pipeline: &ComputePipelineState,
    _q8_qkv_proj_pipeline: &ComputePipelineState,
    q4k_matvec_pipeline: &ComputePipelineState,
    q6k_matvec_pipeline: &ComputePipelineState,
    rms_norm_pipeline: &ComputePipelineState,
    residual_add_pipeline: &ComputePipelineState,
    _rope_pipeline: &ComputePipelineState,
    rope_apply_multi_head_pipeline: &ComputePipelineState,
    kv_cache: &mut super::kv_cache::KVCache,
    layers: &[crate::FullPipelineLayer],
    x: &[f32],
    hidden: usize,
    inter: usize,
    q_dim: usize,
    kv_dim: usize,
    seq_len: usize,
    _num_q_heads: usize,
    _num_kv_heads: usize,
    _head_dim: usize,
    _rope_base: f32,
    use_qk_norm: bool,
    softcap: f32,
) -> Vec<f32> {
    let num_layers = layers.len();
    let hidden_bytes = (hidden * 4) as u64;

    // Pre-cache weight buffers
    let wq_bufs: Vec<_> = layers.iter().map(|l| bufs.get_bytes(l.wq.data)).collect();
    let wk_bufs: Vec<_> = layers.iter().map(|l| bufs.get_bytes(l.wk.data)).collect();
    let wv_bufs: Vec<_> = layers.iter().map(|l| bufs.get_bytes(l.wv.data)).collect();
    let wo_bufs: Vec<_> = layers.iter().map(|l| bufs.get_bytes(l.wo.data)).collect();
    let gate_bufs: Vec<_> = layers.iter().map(|l| bufs.get_bytes(l.gate.data)).collect();
    let up_bufs: Vec<_> = layers.iter().map(|l| bufs.get_bytes(l.up.data)).collect();
    let down_bufs: Vec<_> = layers.iter().map(|l| bufs.get_bytes(l.down.data)).collect();
    let input_norm_bufs: Vec<_> = layers
        .iter()
        .map(|l| bufs.transient_from_f32(l.input_norm))
        .collect();
    let post_attn_norm_bufs: Vec<_> = layers
        .iter()
        .map(|l| bufs.transient_from_f32(l.post_attn_norm))
        .collect();

    // Initial hidden state: [seq_len, hidden]
    let mut h_buf = bufs.transient_from_f32(x);

    let cmd = queue.new_command_buffer();

    for l in 0..num_layers {
        let norm_offset = layers[l].norm_offset;
        let eps = layers[l].eps;
        let has_post_norms = layers[l].has_post_norms;
        let attn_format = layers[l].wq.format;
        let head_dim = layers[l].head_dim;
        let num_q_heads = layers[l].num_q_heads;
        let num_kv_heads = layers[l].num_kv_heads;
        let rope_base = layers[l].rope_base;
        let scale = layers[l].attn_scale;

        // ── 1. Input norm ──
        let norm_out = bufs.output(hidden_bytes * seq_len as u64);
        for s in 0..seq_len {
            let in_off = (s * hidden * 4) as u64;
            let out_off = (s * hidden * 4) as u64;
            let enc = cmd.new_compute_command_encoder();
            let len_val = hidden as u32;
            enc.set_compute_pipeline_state(rms_norm_pipeline);
            enc.set_buffer(0, Some(&h_buf), in_off);
            enc.set_buffer(1, Some(&input_norm_bufs[l]), 0);
            enc.set_buffer(2, Some(&norm_out), out_off);
            enc.set_bytes(3, 4, &len_val as *const u32 as *const c_void);
            enc.set_bytes(4, 4, &eps as *const f32 as *const c_void);
            enc.set_bytes(5, 4, &norm_offset as *const f32 as *const c_void);
            enc.dispatch_threads(
                MTLSize::new(hidden as u64, 1, 1),
                MTLSize::new(256.min(hidden as u64), 1, 1),
            );
            enc.end_encoding();
        }

        // ── 2. Q/K/V projections ──
        let q_out = bufs.output((q_dim * seq_len * 4) as u64);
        let k_out = bufs.output((kv_dim * seq_len * 4) as u64);
        let v_out = bufs.output((kv_dim * seq_len * 4) as u64);

        for s in 0..seq_len {
            let in_off = (s * hidden * 4) as u64;
            // Q projection
            let enc = cmd.new_compute_command_encoder();
            encode_quant_matvec_at_offset(
                enc,
                attn_format,
                &q4.f32_matvec,
                q8_matvec_pipeline,
                q4k_matvec_pipeline,
                q6k_matvec_pipeline,
                &wq_bufs[l],
                &norm_out,
                in_off,
                &q_out,
                (s * q_dim * 4) as u64,
                q_dim,
                hidden,
            );
            enc.end_encoding();
            // K projection
            let enc = cmd.new_compute_command_encoder();
            encode_quant_matvec_at_offset(
                enc,
                layers[l].wk.format,
                &q4.f32_matvec,
                q8_matvec_pipeline,
                q4k_matvec_pipeline,
                q6k_matvec_pipeline,
                &wk_bufs[l],
                &norm_out,
                in_off,
                &k_out,
                (s * kv_dim * 4) as u64,
                kv_dim,
                hidden,
            );
            enc.end_encoding();
            // V projection
            let enc = cmd.new_compute_command_encoder();
            encode_quant_matvec_at_offset(
                enc,
                layers[l].wv.format,
                &q4.f32_matvec,
                q8_matvec_pipeline,
                q4k_matvec_pipeline,
                q6k_matvec_pipeline,
                &wv_bufs[l],
                &norm_out,
                in_off,
                &v_out,
                (s * kv_dim * 4) as u64,
                kv_dim,
                hidden,
            );
            enc.end_encoding();
        }

        // ── 3. RoPE on GPU (K only for cache) ──
        {
            let enc = cmd.new_compute_command_encoder();
            let hd_val = head_dim as u32;
            let nkv_val = num_kv_heads as u32;
            let rotary_dim_val = layers[l].rotary_dim as u32;
            enc.set_compute_pipeline_state(rope_apply_multi_head_pipeline);
            enc.set_buffer(0, Some(&k_out), 0);
            enc.set_bytes(1, 4, &hd_val as *const u32 as *const c_void);
            enc.set_bytes(2, 4, &rope_base as *const f32 as *const c_void);
            enc.set_bytes(3, 4, &rotary_dim_val as *const u32 as *const c_void);
            enc.set_bytes(4, 4, &nkv_val as *const u32 as *const c_void);
            enc.dispatch_thread_groups(
                MTLSize::new((head_dim / 2).div_ceil(32) as u64, num_kv_heads as u64, seq_len as u64),
                MTLSize::new(32, 1, 1),
            );
            enc.end_encoding();
        }

        // ── 4. Fused attention (skip_rope=1, we manually RoPE'd K) ──
        // Actually, fused_attention expects pre-RoPE Q. If we manually RoPE K,
        // we should also manually RoPE Q and then use skip_rope=1.
        {
            let enc = cmd.new_compute_command_encoder();
            let hd_val = head_dim as u32;
            let nq_val = num_q_heads as u32;
            let rotary_dim_val = layers[l].rotary_dim as u32;
            enc.set_compute_pipeline_state(rope_apply_multi_head_pipeline);
            enc.set_buffer(0, Some(&q_out), 0);
            enc.set_bytes(1, 4, &hd_val as *const u32 as *const c_void);
            enc.set_bytes(2, 4, &rope_base as *const f32 as *const c_void);
            enc.set_bytes(3, 4, &rotary_dim_val as *const u32 as *const c_void);
            enc.set_bytes(4, 4, &nq_val as *const u32 as *const c_void);
            enc.dispatch_thread_groups(
                MTLSize::new((head_dim / 2).div_ceil(32) as u64, num_q_heads as u64, seq_len as u64),
                MTLSize::new(32, 1, 1),
            );
            enc.end_encoding();
        }

        let attn_out = bufs.output((q_dim * seq_len * 4) as u64);
        {
            let seq_val = seq_len as u32;
            let hd_val = head_dim as u32;
            let nq_val = num_q_heads as u32;
            let nkv_val = num_kv_heads as u32;
            let scale_val = scale;
            let qknorm_val = if use_qk_norm { 1u32 } else { 0u32 };
            let skip_rope_val = 1u32; // manual RoPE applied above

            let enc = cmd.new_compute_command_encoder();
            enc.set_compute_pipeline_state(fused_attn_pipeline);
            enc.set_buffer(0, Some(&q_out), 0);
            enc.set_buffer(1, Some(&k_out), 0);
            enc.set_buffer(2, Some(&v_out), 0);
            enc.set_buffer(3, Some(&attn_out), 0);
            enc.set_bytes(4, 4, &seq_val as *const u32 as *const c_void);
            enc.set_bytes(5, 4, &hd_val as *const u32 as *const c_void);
            enc.set_bytes(6, 4, &nq_val as *const u32 as *const c_void);
            enc.set_bytes(7, 4, &nkv_val as *const u32 as *const c_void);
            enc.set_bytes(8, 4, &scale_val as *const f32 as *const c_void);
            enc.set_bytes(9, 4, &rope_base as *const f32 as *const c_void);
            enc.set_bytes(10, 4, &qknorm_val as *const u32 as *const c_void);
            enc.set_bytes(11, 4, &softcap as *const f32 as *const c_void);
            enc.set_bytes(12, 4, &skip_rope_val as *const u32 as *const c_void);
            let rotary_dim_val = layers[l].rotary_dim as u32;
            enc.set_bytes(13, 4, &rotary_dim_val as *const u32 as *const c_void);
            enc.dispatch_thread_groups(
                MTLSize::new(num_q_heads as u64, seq_len as u64, 1),
                MTLSize::new(256, 1, 1),
            );
            enc.end_encoding();
        }

        // ── 5. O projection ──
        let o_out = bufs.output(hidden_bytes * seq_len as u64);
        for s in 0..seq_len {
            let enc = cmd.new_compute_command_encoder();
            encode_quant_matvec_at_offset(
                enc,
                layers[l].wo.format,
                &q4.f32_matvec,
                q8_matvec_pipeline,
                q4k_matvec_pipeline,
                q6k_matvec_pipeline,
                &wo_bufs[l],
                &attn_out,
                (s * q_dim * 4) as u64,
                &o_out,
                (s * hidden * 4) as u64,
                hidden,
                q_dim,
            );
            enc.end_encoding();
        }

        // ── 6. Residual + FFN norm ──
        let h_post_attn = bufs.output(hidden_bytes * seq_len as u64);
        let ffn_norm_out = bufs.output(hidden_bytes * seq_len as u64);

        for s in 0..seq_len {
            let h_off = (s * hidden * 4) as u64;
            if has_post_norms {
                let normed = bufs.output(hidden_bytes);
                let enc = cmd.new_compute_command_encoder();
                encode_rms_norm(enc, rms_norm_pipeline, &o_out, &post_attn_norm_bufs[l], &normed, hidden, eps, norm_offset);
                enc.end_encoding();
                let enc = cmd.new_compute_command_encoder();
                encode_residual_add(enc, residual_add_pipeline, &h_buf, &normed, &h_post_attn, hidden);
                enc.end_encoding();
            } else {
                let enc = cmd.new_compute_command_encoder();
                let len_val = hidden as u32;
                enc.set_compute_pipeline_state(residual_add_pipeline);
                enc.set_buffer(0, Some(&h_buf), h_off);
                enc.set_buffer(1, Some(&o_out), h_off);
                enc.set_buffer(2, Some(&h_post_attn), h_off);
                enc.set_bytes(3, 4, &len_val as *const u32 as *const c_void);
                enc.dispatch_threads(MTLSize::new(hidden as u64, 1, 1), MTLSize::new(256.min(hidden as u64), 1, 1));
                enc.end_encoding();
            }
            let ffn_norm_buf = if has_post_norms {
                layers[l].pre_ffn_norm.map(|n| bufs.transient_from_f32(n))
            } else {
                None
            };
            let ffn_norm_buf = ffn_norm_buf.as_ref().unwrap_or(&post_attn_norm_bufs[l]);
            let enc = cmd.new_compute_command_encoder();
            let len_val = hidden as u32;
            enc.set_compute_pipeline_state(rms_norm_pipeline);
            enc.set_buffer(0, Some(&h_post_attn), h_off);
            enc.set_buffer(1, Some(ffn_norm_buf), 0);
            enc.set_buffer(2, Some(&ffn_norm_out), h_off);
            enc.set_bytes(3, 4, &len_val as *const u32 as *const c_void);
            enc.set_bytes(4, 4, &eps as *const f32 as *const c_void);
            enc.set_bytes(5, 4, &norm_offset as *const f32 as *const c_void);
            enc.dispatch_threads(MTLSize::new(hidden as u64, 1, 1), MTLSize::new(256.min(hidden as u64), 1, 1));
            enc.end_encoding();
        }

        // ── 7. Q4 FFN ──
        let gate_out = bufs.output((inter * seq_len * 4) as u64);
        let up_out = bufs.output((inter * seq_len * 4) as u64);
        let act_buf = bufs.output((inter * seq_len * 4) as u64);
        let down_out = bufs.output(hidden_bytes * seq_len as u64);
        let inter_val = inter as u32;
        let hidden_val = hidden as u32;

        for s in 0..seq_len {
            let ffn_off = (s * hidden * 4) as u64;
            let inter_off = (s * inter * 4) as u64;
            let enc = cmd.new_compute_command_encoder();
            enc.set_compute_pipeline_state(&q4.f32_matvec);
            enc.set_buffer(0, Some(&gate_bufs[l]), 0);
            enc.set_buffer(1, Some(&ffn_norm_out), ffn_off);
            enc.set_buffer(2, Some(&gate_out), inter_off);
            enc.set_bytes(3, 4, &inter_val as *const u32 as *const c_void);
            enc.set_bytes(4, 4, &hidden_val as *const u32 as *const c_void);
            enc.dispatch_threads(MTLSize::new(inter as u64, 1, 1), MTLSize::new(256, 1, 1));
            enc.end_encoding();

            let enc = cmd.new_compute_command_encoder();
            enc.set_compute_pipeline_state(&q4.f32_matvec);
            enc.set_buffer(0, Some(&up_bufs[l]), 0);
            enc.set_buffer(1, Some(&ffn_norm_out), ffn_off);
            enc.set_buffer(2, Some(&up_out), inter_off);
            enc.set_bytes(3, 4, &inter_val as *const u32 as *const c_void);
            enc.set_bytes(4, 4, &hidden_val as *const u32 as *const c_void);
            enc.dispatch_threads(MTLSize::new(inter as u64, 1, 1), MTLSize::new(256, 1, 1));
            enc.end_encoding();

            let enc = cmd.new_compute_command_encoder();
            enc.set_compute_pipeline_state(geglu_pipeline);
            enc.set_buffer(0, Some(&gate_out), inter_off);
            enc.set_buffer(1, Some(&up_out), inter_off);
            enc.set_buffer(2, Some(&act_buf), inter_off);
            enc.set_bytes(3, 4, &inter_val as *const u32 as *const c_void);
            enc.dispatch_threads(MTLSize::new(inter as u64, 1, 1), MTLSize::new(256, 1, 1));
            enc.end_encoding();

            let enc = cmd.new_compute_command_encoder();
            enc.set_compute_pipeline_state(&q4.f32_matvec);
            enc.set_buffer(0, Some(&down_bufs[l]), 0);
            enc.set_buffer(1, Some(&act_buf), inter_off);
            enc.set_buffer(2, Some(&down_out), (s * hidden * 4) as u64);
            enc.set_bytes(3, 4, &hidden_val as *const u32 as *const c_void);
            enc.set_bytes(4, 4, &inter_val as *const u32 as *const c_void);
            enc.dispatch_threads(MTLSize::new(hidden as u64, 1, 1), MTLSize::new(256, 1, 1));
            enc.end_encoding();
        }

        // ── 8. Post-FFN ──
        let new_h = bufs.output(hidden_bytes * seq_len as u64);
        for s in 0..seq_len {
            let off = (s * hidden * 4) as u64;
            if has_post_norms {
                if let Some(post_ffn_norm) = layers[l].post_ffn_norm {
                    let post_ffn_buf = bufs.transient_from_f32(post_ffn_norm);
                    let normed = bufs.output(hidden_bytes);
                    let enc = cmd.new_compute_command_encoder();
                    let len_val = hidden as u32;
                    enc.set_compute_pipeline_state(rms_norm_pipeline);
                    enc.set_buffer(0, Some(&down_out), off);
                    enc.set_buffer(1, Some(&post_ffn_buf), 0);
                    enc.set_buffer(2, Some(&normed), 0);
                    enc.set_bytes(3, 4, &len_val as *const u32 as *const c_void);
                    enc.set_bytes(4, 4, &eps as *const f32 as *const c_void);
                    enc.set_bytes(5, 4, &norm_offset as *const f32 as *const c_void);
                    enc.dispatch_threads(MTLSize::new(hidden as u64, 1, 1), MTLSize::new(256.min(hidden as u64), 1, 1));
                    enc.end_encoding();
                    let enc = cmd.new_compute_command_encoder();
                    encode_residual_add(enc, residual_add_pipeline, &h_post_attn, &normed, &new_h, hidden);
                    enc.end_encoding();
                } else {
                    let enc = cmd.new_compute_command_encoder();
                    let len_val = hidden as u32;
                    enc.set_compute_pipeline_state(residual_add_pipeline);
                    enc.set_buffer(0, Some(&h_post_attn), off);
                    enc.set_buffer(1, Some(&down_out), off);
                    enc.set_buffer(2, Some(&new_h), off);
                    enc.set_bytes(3, 4, &len_val as *const u32 as *const c_void);
                    enc.dispatch_threads(MTLSize::new(hidden as u64, 1, 1), MTLSize::new(256.min(hidden as u64), 1, 1));
                    enc.end_encoding();
                }
            } else {
                let enc = cmd.new_compute_command_encoder();
                let len_val = hidden as u32;
                enc.set_compute_pipeline_state(residual_add_pipeline);
                enc.set_buffer(0, Some(&h_post_attn), off);
                enc.set_buffer(1, Some(&down_out), off);
                enc.set_buffer(2, Some(&new_h), off);
                enc.set_bytes(3, 4, &len_val as *const u32 as *const c_void);
                enc.dispatch_threads(MTLSize::new(hidden as u64, 1, 1), MTLSize::new(256.min(hidden as u64), 1, 1));
                enc.end_encoding();
            }
        }
        h_buf = new_h;

        // ── 9. Populate KV cache (GPU-side) ──
        {
            let blit = cmd.new_blit_command_encoder();
            let kv_layer = &kv_cache.layers[l];
            let bytes = (seq_len * kv_dim * 4) as u64;
            blit.copy_from_buffer(&k_out, 0, &kv_layer.k_cache, 0, bytes);
            blit.copy_from_buffer(&v_out, 0, &kv_layer.v_cache, 0, bytes);
            blit.end_encoding();
        }
        kv_cache.layers[l].current_len = seq_len;
    }

    cmd.commit();
    cmd.wait_until_completed();
    crate::metal::buffers::read_buffer_f32(&h_buf, seq_len * hidden)
}
