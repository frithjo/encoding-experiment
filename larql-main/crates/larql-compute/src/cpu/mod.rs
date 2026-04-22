//! CPU compute backend — BLAS for f32, C kernel for Q4.
//!
//! On macOS: Accelerate BLAS dispatches through Apple's AMX coprocessor.
//! On Linux: OpenBLAS or similar.
//! Q4: C kernel with ARM vdotq_s32 (0.95ms per 105MB matrix on M3 Max).
//!
//! ## Modules
//!
//! - `ops/f32_matmul`: BLAS sgemm dispatch
//! - `ops/q4_matvec`:  C kernel Q4×Q8 matrix-vector
//! - `ops/q4_vecmat`:  C kernel Q4 vector-matrix
//! - `ops/q4_common`:  Q8 quantization, C FFI declarations
//! - `ops/geglu`:      Element-wise GEGLU activation
//! - `ops/attention`:  Causal attention (fused QK softmax V)

pub mod ops;

// Re-export for backward compatibility (used by benchmarks/examples)
pub mod q4 {
    pub use super::ops::q4_common::{q4_0_matvec_c, q4_0_vecmat_c, quantize_q4_0, quantize_to_q8};
    pub use super::ops::q4_matvec::dispatch as q4_matvec;
    pub use super::ops::q4_vecmat::dispatch as q4_vecmat;
}

use crate::backend::ComputeBackend;
use ndarray::{Array2, ArrayView2};

/// CPU backend using BLAS (f32) and C kernel (Q4).
pub struct CpuBackend {
    /// KV cache for decode mode — initialized on first use.
    kv_cache: std::sync::Mutex<Option<ops::kv_cache::KVCache>>,
    /// Maximum sequence length for KV cache (dynamic based on model config)
    max_seq: usize,
}

impl Default for CpuBackend {
    fn default() -> Self {
        Self {
            kv_cache: std::sync::Mutex::new(None),
            max_seq: 4096,
        }
    }
}

impl CpuBackend {
    pub fn new(max_seq: usize) -> Self {
        Self {
            kv_cache: std::sync::Mutex::new(None),
            max_seq,
        }
    }
}

impl ComputeBackend for CpuBackend {
    fn matmul(&self, a: ArrayView2<f32>, b: ArrayView2<f32>) -> Array2<f32> {
        ops::f32_matmul::matmul(a, b)
    }

    fn matmul_transb(&self, a: ArrayView2<f32>, b: ArrayView2<f32>) -> Array2<f32> {
        ops::f32_matmul::matmul_transb(a, b)
    }

    fn q4_matvec(
        &self,
        q4_data: &[u8],
        q8_x: &[i8],
        q8_scales: &[f32],
        num_rows: usize,
        hidden: usize,
    ) -> Option<Vec<f32>> {
        Some(ops::q4_matvec::dispatch_q8(
            q4_data, q8_x, q8_scales, num_rows, hidden,
        ))
    }

    fn q4_vecmat(
        &self,
        activation: &[f32],
        q4_data: &[u8],
        intermediate: usize,
        hidden: usize,
    ) -> Option<Vec<f32>> {
        Some(ops::q4_vecmat::dispatch(
            activation,
            q4_data,
            intermediate,
            hidden,
        ))
    }

    fn q4k_matvec(
        &self,
        q4k_data: &[u8],
        x: &[f32],
        num_rows: usize,
        hidden: usize,
    ) -> Option<Vec<f32>> {
        Some(ops::q4k_matvec::dispatch(q4k_data, x, num_rows, hidden))
    }

    fn q6k_matvec(
        &self,
        q6k_data: &[u8],
        x: &[f32],
        num_rows: usize,
        hidden: usize,
    ) -> Option<Vec<f32>> {
        Some(ops::q6k_matvec::dispatch(q6k_data, x, num_rows, hidden))
    }

    fn has_kv_cache(&self) -> bool {
        true
    }

    fn populate_kv_layer(
        &self,
        layer: usize,
        k_data: &[f32],
        v_data: &[f32],
        seq_len: usize,
        num_kv_heads: usize,
        head_dim: usize,
    ) {
        let mut cache_guard = self.kv_cache.lock().unwrap();
        if cache_guard.is_none() {
            // Lazy init with current layer count (will grow if needed)
            *cache_guard = Some(ops::kv_cache::KVCache::new(
                layer + 1,
                self.max_seq,
                num_kv_heads,
                head_dim,
            ));
        }
        let kv = cache_guard.as_mut().unwrap();
        while kv.layers.len() <= layer {
            kv.layers.push(ops::kv_cache::LayerKVCache::new(
                self.max_seq,
                num_kv_heads,
                head_dim,
            ));
        }

        let lc = &mut kv.layers[layer];
        let total = seq_len * num_kv_heads * head_dim;
        
        // Grow if prefill exceeds max_seq
        if seq_len > lc.max_seq {
            lc.grow(seq_len.next_power_of_two());
        }

        lc.k_cache[..total].copy_from_slice(&k_data[..total]);
        lc.v_cache[..total].copy_from_slice(&v_data[..total]);
        lc.current_len = seq_len;
    }

    fn reset_kv_cache(&self) {
        let mut cache_guard = self.kv_cache.lock().unwrap();
        if let Some(kv) = cache_guard.as_mut() {
            kv.clear();
        }
    }

    fn has_q4(&self) -> bool {
        true
    }

    fn decode_token(
        &self,
        layers: &[crate::FullPipelineLayer<'_>],
        x: &[f32],
        hidden: usize,
        inter: usize,
        q_dim: usize,
        kv_dim: usize,
        num_q_heads: usize,
        num_kv_heads: usize,
        head_dim: usize,
        rope_base: f32,
    ) -> Option<Vec<f32>> {
        let mut cache_guard = self.kv_cache.lock().unwrap();
        if cache_guard.is_none() {
            *cache_guard = Some(ops::kv_cache::KVCache::new(
                layers.len(),
                self.max_seq,
                num_kv_heads,
                head_dim,
            ));
        }
        let kv = cache_guard.as_mut().unwrap();
        
        // Ensure KV cache is large enough
        if kv.current_len() >= kv.layers[0].max_seq {
            kv.grow(kv.layers[0].max_seq * 2);
        }

        let mut h = x.to_vec();
        let pos = kv.current_len();

        for (l, layer) in layers.iter().enumerate() {
            // 1. RMS Norm
            let h_norm = ops::linalg::rms_norm(&h, layer.input_norm, layer.eps, layer.norm_offset);

            // 2. QKV Projections (using optimized matvecs)
            let q = self.q4k_matvec(layer.wq.data, &h_norm, q_dim, hidden).unwrap();
            let k = self.q4k_matvec(layer.wk.data, &h_norm, kv_dim, hidden).unwrap();
            let v = self.q4k_matvec(layer.wv.data, &h_norm, kv_dim, hidden).unwrap();

            // 3. RoPE (in-place)
            let mut q = q;
            let mut k = k;
            for hi in 0..num_q_heads {
                ops::linalg::rope_at_pos(&mut q[hi * head_dim..(hi + 1) * head_dim], head_dim, rope_base, pos);
            }
            for hi in 0..num_kv_heads {
                ops::linalg::rope_at_pos(&mut k[hi * head_dim..(hi + 1) * head_dim], head_dim, rope_base, pos);
            }

            // 4. KV Append
            let kv_layer = &mut kv.layers[l];
            let kv_off = pos * num_kv_heads * head_dim;
            kv_layer.k_cache[kv_off..kv_off + kv_dim].copy_from_slice(&k);
            kv_layer.v_cache[kv_off..kv_off + kv_dim].copy_from_slice(&v);

            // 5. Attention
            let mut attn_out = vec![0.0; q_dim];
            let scale = 1.0 / (head_dim as f32).sqrt();
            
            // GQA/MQA aware attention
            let n_groups = num_q_heads / num_kv_heads;
            for hi in 0..num_q_heads {
                let kv_hi = hi / n_groups;
                let head_q = &q[hi * head_dim..(hi + 1) * head_dim];
                
                // Extract K/V for this head from cache
                let mut head_k = Vec::with_capacity((pos + 1) * head_dim);
                let mut head_v = Vec::with_capacity((pos + 1) * head_dim);
                for p in 0..=pos {
                    let p_off = p * num_kv_heads * head_dim + kv_hi * head_dim;
                    head_k.extend_from_slice(&kv_layer.k_cache[p_off..p_off + head_dim]);
                    head_v.extend_from_slice(&kv_layer.v_cache[p_off..p_off + head_dim]);
                }

                let head_out = ops::attention::causal_attention(head_q, &head_k, &head_v, 1, head_dim, scale);
                attn_out[hi * head_dim..(hi + 1) * head_dim].copy_from_slice(&head_out);
            }

            // 6. O Projection
            let o_out = self.q4k_matvec(layer.wo.data, &attn_out, hidden, q_dim).unwrap();

            // 7. Residual + FFN Norm
            for i in 0..hidden {
                h[i] += o_out[i];
            }
            let h_ffn_norm = ops::linalg::rms_norm(&h, layer.post_attn_norm, layer.eps, layer.norm_offset);

            // 8. FFN (Gate + Up -> GEGLU -> Down)
            let gate = self.q4k_matvec(layer.gate.data, &h_ffn_norm, inter, hidden).unwrap();
            let up = self.q4k_matvec(layer.up.data, &h_ffn_norm, inter, hidden).unwrap();
            
            let mut act = vec![0.0; inter];
            ops::geglu::geglu_silu(&gate, &up, &mut act);

            let down = self.q6k_matvec(layer.down.data, &act, hidden, inter).unwrap();

            // 9. Final Residual
            for i in 0..hidden {
                h[i] += down[i];
            }
        }
        
        for layer in &mut kv.layers {
            layer.current_len += 1;
        }

        Some(h)
    }

    fn prefill_q4(
        &self,
        layers: &[crate::FullPipelineLayer<'_>],
        x: &[f32],
        hidden: usize,
        inter: usize,
        q_dim: usize,
        kv_dim: usize,
        seq_len: usize,
        num_q_heads: usize,
        num_kv_heads: usize,
        head_dim: usize,
        rope_base: f32,
        _use_qk_norm: bool,
        _softcap: f32,
    ) -> Option<Vec<f32>> {
        let mut cache_guard = self.kv_cache.lock().unwrap();
        if cache_guard.is_none() {
            *cache_guard = Some(ops::kv_cache::KVCache::new(
                layers.len(),
                self.max_seq.max(seq_len),
                num_kv_heads,
                head_dim,
            ));
        }
        let kv = cache_guard.as_mut().unwrap();
        
        if seq_len > kv.layers[0].max_seq {
            kv.grow(seq_len.next_power_of_two());
        }

        let mut h = Array2::from_shape_vec((seq_len, hidden), x.to_vec()).unwrap();

        for (l, layer) in layers.iter().enumerate() {
            // 1. RMS Norm
            let h_norm = ops::linalg::rms_norm_2d(h.view(), layer.input_norm, layer.eps, layer.norm_offset);

            // 2. QKV Projections (per position)
            let mut q_full = Array2::zeros((seq_len, q_dim));
            let mut k_full = Array2::zeros((seq_len, kv_dim));
            let mut v_full = Array2::zeros((seq_len, kv_dim));

            for s in 0..seq_len {
                let row = h_norm.row(s).to_vec();
                let q = self.q4k_matvec(layer.wq.data, &row, q_dim, hidden).unwrap();
                let k = self.q4k_matvec(layer.wk.data, &row, kv_dim, hidden).unwrap();
                let v = self.q4k_matvec(layer.wv.data, &row, kv_dim, hidden).unwrap();
                
                let mut q = q;
                let mut k = k;
                for hi in 0..num_q_heads {
                    ops::linalg::rope_at_pos(&mut q[hi * head_dim..(hi + 1) * head_dim], head_dim, rope_base, s);
                }
                for hi in 0..num_kv_heads {
                    ops::linalg::rope_at_pos(&mut k[hi * head_dim..(hi + 1) * head_dim], head_dim, rope_base, s);
                }

                q_full.row_mut(s).assign(&ndarray::Array1::from(q).view());
                k_full.row_mut(s).assign(&ndarray::Array1::from(k).view());
                v_full.row_mut(s).assign(&ndarray::Array1::from(v).view());
            }

            // 3. Populate KV Cache
            let kv_layer = &mut kv.layers[l];
            let total = seq_len * num_kv_heads * head_dim;
            kv_layer.k_cache[..total].copy_from_slice(k_full.as_slice().unwrap());
            kv_layer.v_cache[..total].copy_from_slice(v_full.as_slice().unwrap());
            kv_layer.current_len = seq_len;

            // 4. Attention
            let mut attn_out = Array2::zeros((seq_len, q_dim));
            let scale = 1.0 / (head_dim as f32).sqrt();
            let n_groups = num_q_heads / num_kv_heads;

            for s in 0..seq_len {
                for hi in 0..num_q_heads {
                    let kv_hi = hi / n_groups;
                    let head_q = q_full.row(s).slice(ndarray::s![hi * head_dim..(hi + 1) * head_dim]).to_vec();
                    
                    let mut head_k = Vec::with_capacity((s + 1) * head_dim);
                    let mut head_v = Vec::with_capacity((s + 1) * head_dim);
                    for p in 0..=s {
                        let p_off = p * num_kv_heads * head_dim + kv_hi * head_dim;
                        head_k.extend_from_slice(&kv_layer.k_cache[p_off..p_off + head_dim]);
                        head_v.extend_from_slice(&kv_layer.v_cache[p_off..p_off + head_dim]);
                    }

                    let head_out = ops::attention::causal_attention(&head_q, &head_k, &head_v, 1, head_dim, scale);
                    attn_out.row_mut(s).slice_mut(ndarray::s![hi * head_dim..(hi + 1) * head_dim]).assign(&ndarray::Array1::from(head_out).view());
                }
            }

            // 5. O Projection
            let mut o_full = Array2::zeros((seq_len, hidden));
            for s in 0..seq_len {
                let row = attn_out.row(s).to_vec();
                let o = self.q4k_matvec(layer.wo.data, &row, hidden, q_dim).unwrap();
                o_full.row_mut(s).assign(&ndarray::Array1::from(o).view());
            }

            // 6. Residual + FFN Norm
            h += &o_full;
            let h_ffn_norm = ops::linalg::rms_norm_2d(h.view(), layer.post_attn_norm, layer.eps, layer.norm_offset);

            // 7. FFN
            let mut down_full = Array2::zeros((seq_len, hidden));
            for s in 0..seq_len {
                let row = h_ffn_norm.row(s).to_vec();
                let gate = self.q4k_matvec(layer.gate.data, &row, inter, hidden).unwrap();
                let up = self.q4k_matvec(layer.up.data, &row, inter, hidden).unwrap();
                
                let mut act = vec![0.0; inter];
                ops::geglu::geglu_silu(&gate, &up, &mut act);

                let down = self.q6k_matvec(layer.down.data, &act, hidden, inter).unwrap();
                down_full.row_mut(s).assign(&ndarray::Array1::from(down).view());
            }

            // 8. Final Residual
            h += &down_full;
        }

        Some(h.into_raw_vec_and_offset().0)
    }

    fn name(&self) -> &str {
        "cpu (BLAS + C Q4 kernel)"
    }

    fn device_info(&self) -> String {
        #[cfg(target_os = "macos")]
        {
            "macOS Accelerate AMX".to_string()
        }
        #[cfg(not(target_os = "macos"))]
        {
            "CPU BLAS".to_string()
        }
    }
}
