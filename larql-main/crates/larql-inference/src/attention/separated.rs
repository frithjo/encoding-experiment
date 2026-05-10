//! Separated attention runtime protocol.
//!
//! The weight side owns learned tensors and sends post-projection Q/K/V activations.
//! The attention side owns only causal GQA computation and returns attention output.

use super::{gqa_attention_with_weights, AttentionWeights};
use larql_core::capsule::{
    canonical_json_bytes, make_capsule, make_capture, validate_capsule, validate_capture,
    write_capsule_json, Capsule, Capture,
};
use ndarray::Array2;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

type RuntimeResult<T> = Result<T, Box<dyn Error>>;

pub const SEPARATED_ATTENTION_RUNTIME_PRODUCER: &str = "larql.attention_runtime.rust";
pub const SEPARATED_ATTENTION_REQUEST_CAPTURE_KIND: &str = "separated_attention_request";
pub const SEPARATED_ATTENTION_RESPONSE_CAPTURE_KIND: &str = "separated_attention_response";
pub const SEPARATED_ATTENTION_RESPONSE_CAPSULE_KIND: &str = "separated_attention_response_capsule";
pub const SEPARATED_ATTENTION_ERROR_CAPTURE_KIND: &str = "separated_attention_error";
pub const SEPARATED_ATTENTION_ERROR_CAPSULE_KIND: &str = "separated_attention_error_capsule";
pub const SEPARATED_ATTENTION_READY_CAPTURE_KIND: &str = "separated_attention_ready";
pub const SEPARATED_ATTENTION_READY_CAPSULE_KIND: &str = "separated_attention_ready_capsule";

#[allow(clippy::type_complexity)]
pub type SeparatedAttentionBlockOutput = (
    Array2<f32>,
    Array2<f32>,
    Option<AttentionWeights>,
    Array2<f32>,
    Array2<f32>,
    Vec<Vec<f32>>,
);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AttentionTensor2 {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<f32>,
}

impl AttentionTensor2 {
    pub fn from_array(array: &Array2<f32>) -> Self {
        Self {
            rows: array.shape()[0],
            cols: array.shape()[1],
            data: array.iter().copied().collect(),
        }
    }

    pub fn to_array(&self, field: &str) -> RuntimeResult<Array2<f32>> {
        let expected = self
            .rows
            .checked_mul(self.cols)
            .ok_or_else(|| format!("{field} shape overflow"))?;
        if self.data.len() != expected {
            return Err(format!(
                "{field} data length mismatch: rows={} cols={} len={} expected={}",
                self.rows,
                self.cols,
                self.data.len(),
                expected
            )
            .into());
        }
        Ok(Array2::from_shape_vec(
            (self.rows, self.cols),
            self.data.clone(),
        )?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeparatedAttentionRequest {
    pub protocol_version: u32,
    pub q: AttentionTensor2,
    pub k: AttentionTensor2,
    pub v: AttentionTensor2,
    pub num_q_heads: usize,
    pub num_kv_heads: usize,
    pub head_dim: usize,
    pub scale: f64,
    pub capture_attention: bool,
    pub softcap: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeparatedAttentionResponse {
    pub protocol_version: u32,
    pub output: AttentionTensor2,
    pub attention_weights: Option<Vec<Vec<f32>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeparatedAttentionRuntimeError {
    pub protocol_version: u32,
    pub site: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeparatedAttentionReady {
    pub protocol_version: u32,
    pub addr: String,
    pub pid: u32,
}

pub fn separated_attention_request_wire_bytes(
    request: &SeparatedAttentionRequest,
) -> RuntimeResult<Vec<u8>> {
    let capture = separated_attention_request_capture(request)?;
    Ok(canonical_json_bytes(&capture)?)
}

pub fn separated_attention_request_capture(
    request: &SeparatedAttentionRequest,
) -> RuntimeResult<Capture<SeparatedAttentionRequest>> {
    Ok(make_capture(
        SEPARATED_ATTENTION_REQUEST_CAPTURE_KIND,
        request.clone(),
    )?)
}

pub fn run_separated_attention_capture(
    capture: &Capture<SeparatedAttentionRequest>,
) -> RuntimeResult<SeparatedAttentionResponse> {
    validate_capture(capture, Some(SEPARATED_ATTENTION_REQUEST_CAPTURE_KIND))?;
    run_separated_attention_request(&capture.payload)
}

pub fn run_separated_attention_request(
    request: &SeparatedAttentionRequest,
) -> RuntimeResult<SeparatedAttentionResponse> {
    if request.protocol_version != 1 {
        return Err(format!(
            "unsupported separated attention protocol version {}",
            request.protocol_version
        )
        .into());
    }
    if request.num_kv_heads == 0 || request.num_q_heads == 0 || request.head_dim == 0 {
        return Err("attention head counts and head_dim must be nonzero".into());
    }
    if request.num_q_heads % request.num_kv_heads != 0 {
        return Err(format!(
            "num_q_heads must be divisible by num_kv_heads: {} % {} != 0",
            request.num_q_heads, request.num_kv_heads
        )
        .into());
    }

    let q = request.q.to_array("q")?;
    let k = request.k.to_array("k")?;
    let v = request.v.to_array("v")?;
    let seq_len = q.shape()[0];
    if seq_len == 0 {
        return Err("q rows must be nonzero".into());
    }
    if k.shape()[0] != seq_len || v.shape()[0] != seq_len {
        return Err(format!(
            "prefill separated attention requires q/k/v row match: q={} k={} v={}",
            seq_len,
            k.shape()[0],
            v.shape()[0]
        )
        .into());
    }

    let q_cols = request.num_q_heads * request.head_dim;
    let kv_cols = request.num_kv_heads * request.head_dim;
    if q.shape()[1] != q_cols {
        return Err(format!("q cols mismatch: got {} expected {}", q.shape()[1], q_cols).into());
    }
    if k.shape()[1] != kv_cols || v.shape()[1] != kv_cols {
        return Err(format!(
            "k/v cols mismatch: k={} v={} expected {}",
            k.shape()[1],
            v.shape()[1],
            kv_cols
        )
        .into());
    }

    let reps = request.num_q_heads / request.num_kv_heads;
    let (output, weights) = gqa_attention_with_weights(
        &q,
        &k,
        &v,
        request.num_q_heads,
        request.head_dim,
        reps,
        request.scale,
        seq_len,
        request.capture_attention,
        request.softcap,
    );

    Ok(SeparatedAttentionResponse {
        protocol_version: 1,
        output: AttentionTensor2::from_array(&output),
        attention_weights: weights.map(|AttentionWeights { heads }| heads),
    })
}

pub fn handle_separated_attention_stream(mut stream: TcpStream) -> RuntimeResult<()> {
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;

    let mut body = String::new();
    stream.read_to_string(&mut body)?;
    let bytes = match serde_json::from_str::<Capture<SeparatedAttentionRequest>>(&body) {
        Ok(capture) => match run_separated_attention_capture(&capture) {
            Ok(response) => response_capsule_bytes(response)?,
            Err(err) => error_capsule_bytes("separated_attention_compute", err.to_string())?,
        },
        Err(err) => error_capsule_bytes("separated_attention_admission", err.to_string())?,
    };
    stream.write_all(&bytes)?;
    stream.flush()?;
    Ok(())
}

pub fn bind_separated_attention_listener(
    host: &str,
    port: u16,
    ready_file: Option<&Path>,
) -> RuntimeResult<TcpListener> {
    let listener = TcpListener::bind((host, port))?;
    let addr = listener.local_addr()?;
    if let Some(path) = ready_file {
        let payload = SeparatedAttentionReady {
            protocol_version: 1,
            addr: addr.to_string(),
            pid: std::process::id(),
        };
        let capture = make_capture(SEPARATED_ATTENTION_READY_CAPTURE_KIND, payload)?;
        let capsule = make_capsule(
            SEPARATED_ATTENTION_READY_CAPSULE_KIND,
            capture,
            SEPARATED_ATTENTION_RUNTIME_PRODUCER,
        )?;
        write_capsule_json(path, &capsule)?;
    }
    Ok(listener)
}

pub fn serve_separated_attention(listener: TcpListener, oneshot: bool) -> RuntimeResult<usize> {
    let mut handled = 0usize;
    loop {
        let (stream, _) = listener.accept()?;
        handle_separated_attention_stream(stream)?;
        handled += 1;
        if oneshot {
            return Ok(handled);
        }
    }
}

pub fn request_separated_attention<A: ToSocketAddrs>(
    addr: A,
    request: &SeparatedAttentionRequest,
) -> RuntimeResult<SeparatedAttentionResponse> {
    let mut stream = TcpStream::connect(addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(30)))?;

    let request_body = separated_attention_request_wire_bytes(request)?;
    stream.write_all(&request_body)?;
    stream.flush()?;
    stream.shutdown(Shutdown::Write)?;

    let mut body = String::new();
    stream.read_to_string(&mut body)?;
    let value: serde_json::Value = serde_json::from_str(&body)?;
    match value
        .get("capsule_kind")
        .and_then(serde_json::Value::as_str)
    {
        Some(SEPARATED_ATTENTION_RESPONSE_CAPSULE_KIND) => {
            let capsule: Capsule<SeparatedAttentionResponse> = serde_json::from_value(value)?;
            validate_capsule(
                &capsule,
                Some(SEPARATED_ATTENTION_RESPONSE_CAPSULE_KIND),
                Some(SEPARATED_ATTENTION_RESPONSE_CAPTURE_KIND),
            )?;
            Ok(capsule.capture.payload)
        }
        Some(SEPARATED_ATTENTION_ERROR_CAPSULE_KIND) => {
            let capsule: Capsule<SeparatedAttentionRuntimeError> = serde_json::from_value(value)?;
            validate_capsule(
                &capsule,
                Some(SEPARATED_ATTENTION_ERROR_CAPSULE_KIND),
                Some(SEPARATED_ATTENTION_ERROR_CAPTURE_KIND),
            )?;
            Err(capsule.capture.payload.message.into())
        }
        other => Err(format!("unexpected separated attention capsule kind: {other:?}").into()),
    }
}

fn response_capsule_bytes(response: SeparatedAttentionResponse) -> RuntimeResult<Vec<u8>> {
    let capture = make_capture(SEPARATED_ATTENTION_RESPONSE_CAPTURE_KIND, response)?;
    let capsule = make_capsule(
        SEPARATED_ATTENTION_RESPONSE_CAPSULE_KIND,
        capture,
        SEPARATED_ATTENTION_RUNTIME_PRODUCER,
    )?;
    Ok(canonical_json_bytes(&capsule)?)
}

fn error_capsule_bytes(site: &str, message: String) -> RuntimeResult<Vec<u8>> {
    let capture = make_capture(
        SEPARATED_ATTENTION_ERROR_CAPTURE_KIND,
        SeparatedAttentionRuntimeError {
            protocol_version: 1,
            site: site.to_string(),
            message,
        },
    )?;
    let capsule = make_capsule(
        SEPARATED_ATTENTION_ERROR_CAPSULE_KIND,
        capture,
        SEPARATED_ATTENTION_RUNTIME_PRODUCER,
    )?;
    Ok(canonical_json_bytes(&capsule)?)
}

/// Run one real model attention layer with weights and attention separated.
///
/// The caller keeps learned weights local, computes post-projection/post-RoPE Q/K/V,
/// sends only those activations to the attention runtime, then applies O projection
/// and residual locally.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub fn run_attention_block_via_separated_runtime<A: ToSocketAddrs>(
    weights: &crate::model::ModelWeights,
    h: &Array2<f32>,
    layer: usize,
    capture_attention: bool,
    addr: A,
) -> RuntimeResult<SeparatedAttentionBlockOutput> {
    use crate::forward::{add_bias, dot_proj};
    use crate::residual::{rms_norm_heads, rms_norm_heads_no_weight};

    let arch = &*weights.arch;
    let head_dim = arch.head_dim_for_layer(layer);
    let num_q = arch.num_q_heads_for_layer(layer);
    let num_kv = arch.num_kv_heads_for_layer(layer);
    if num_kv == 0 || num_q == 0 || head_dim == 0 || num_q % num_kv != 0 {
        return Err(format!(
            "invalid attention shape for layer {layer}: num_q={num_q} num_kv={num_kv} head_dim={head_dim}"
        )
        .into());
    }
    let scale = if arch.attention_multiplier() != 1.0 {
        arch.attention_multiplier() as f64
    } else {
        arch.attention_scale_for_layer(layer)
    };
    let seq_len = h.shape()[0];
    if seq_len == 0 {
        return Err("hidden state must contain at least one token".into());
    }
    let norm_offset = arch.norm_weight_offset();
    let qk_offset = weights.arch.qk_norm_weight_offset();
    let qk_norm_off = if qk_offset != 0.0 {
        qk_offset
    } else {
        norm_offset
    };

    let h_norm =
        crate::forward::apply_norm(weights, h, &arch.input_layernorm_key(layer), norm_offset);

    let w_q = weights
        .tensors
        .get(&arch.attn_q_key(layer))
        .ok_or_else(|| format!("missing tensor {}", arch.attn_q_key(layer)))?;
    let w_k = weights
        .tensors
        .get(&arch.attn_k_key(layer))
        .ok_or_else(|| format!("missing tensor {}", arch.attn_k_key(layer)))?;
    let v_from_k = !weights.tensors.contains_key(&arch.attn_v_key(layer));
    let w_v = if v_from_k {
        w_k
    } else {
        weights
            .tensors
            .get(&arch.attn_v_key(layer))
            .ok_or_else(|| format!("missing tensor {}", arch.attn_v_key(layer)))?
    };
    let w_o = weights
        .tensors
        .get(&arch.attn_o_key(layer))
        .ok_or_else(|| format!("missing tensor {}", arch.attn_o_key(layer)))?;

    let mut q_full = dot_proj(&h_norm, w_q);
    let mut k_full = dot_proj(&h_norm, w_k);
    let mut v_full = dot_proj(&h_norm, w_v);

    if let Some(bias) = arch
        .attn_q_bias_key(layer)
        .and_then(|k| weights.vectors.get(&k))
    {
        add_bias(&mut q_full, bias);
    }
    if let Some(bias) = arch
        .attn_k_bias_key(layer)
        .and_then(|k| weights.vectors.get(&k))
    {
        add_bias(&mut k_full, bias);
    }
    if let Some(bias) = arch
        .attn_v_bias_key(layer)
        .and_then(|k| weights.vectors.get(&k))
    {
        add_bias(&mut v_full, bias);
    }

    let q_normed = match arch
        .attn_q_norm_key(layer)
        .and_then(|k| weights.vectors.get(&k))
    {
        Some(norm_w) => rms_norm_heads(&q_full, norm_w, num_q, head_dim, qk_norm_off),
        None => q_full,
    };
    if arch.has_v_norm() {
        v_full = rms_norm_heads_no_weight(&v_full, num_kv, head_dim);
    }
    let k_normed = match arch
        .attn_k_norm_key(layer)
        .and_then(|k| weights.vectors.get(&k))
    {
        Some(norm_w) => rms_norm_heads(&k_full, norm_w, num_kv, head_dim, qk_norm_off),
        None => k_full,
    };

    let layer_rope_base = arch.rope_base_for_layer(layer);
    let rotary_frac = arch.rotary_fraction_for_layer(layer);
    let q_rope =
        super::apply_rope_partial(&q_normed, num_q, head_dim, layer_rope_base, rotary_frac);
    let k_rope =
        super::apply_rope_partial(&k_normed, num_kv, head_dim, layer_rope_base, rotary_frac);

    let request = SeparatedAttentionRequest {
        protocol_version: 1,
        q: AttentionTensor2::from_array(&q_rope),
        k: AttentionTensor2::from_array(&k_rope),
        v: AttentionTensor2::from_array(&v_full),
        num_q_heads: num_q,
        num_kv_heads: num_kv,
        head_dim,
        scale,
        capture_attention,
        softcap: arch.attn_logit_softcapping(),
    };
    let response = request_separated_attention(addr, &request)?;
    let attn_out = response.output.to_array("attention output")?;

    let mut attn_projected = dot_proj(&attn_out, w_o);
    if let Some(bias) = arch
        .attn_o_bias_key(layer)
        .and_then(|k| weights.vectors.get(&k))
    {
        add_bias(&mut attn_projected, bias);
    }

    let mut head_projections = Vec::new();
    if capture_attention {
        for h_idx in 0..num_q {
            let start = h_idx * head_dim;
            let end = start + head_dim;
            let head_out = attn_out.slice(ndarray::s![seq_len - 1..seq_len, start..end]);
            let w_o_slice = w_o.slice(ndarray::s![.., start..end]);
            let head_proj = head_out.dot(&w_o_slice.t());
            head_projections.push(head_proj.row(0).to_vec());
        }
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

    Ok((
        h_post_attn,
        attn_projected,
        response
            .attention_weights
            .map(|heads| AttentionWeights { heads }),
        k_rope,
        v_full,
        head_projections,
    ))
}
