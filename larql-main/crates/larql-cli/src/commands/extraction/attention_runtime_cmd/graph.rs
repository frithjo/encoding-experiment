use super::*;

pub(super) fn runnable_attention_layers(weights: &ModelWeights) -> Vec<usize> {
    (0..weights.num_layers)
        .filter(|&layer| build_attention_graph_layer(weights, layer).is_ok())
        .collect()
}

pub(super) fn select_attention_graph_layer(
    weights: &ModelWeights,
    requested_layer: Option<usize>,
    available_attention_layers: &[usize],
) -> Result<AttentionGraphLayer, Box<dyn Error>> {
    if let Some(layer) = requested_layer {
        return build_attention_graph_layer(weights, layer).map_err(|err| {
            format!(
                "requested layer {layer} is not a runnable Q/K/V/O attention graph: {err}; available graph attention layers: {:?}",
                available_attention_layers
            )
            .into()
        });
    }

    let layer = *available_attention_layers
        .first()
        .ok_or_else(|| "GGUF contains no runnable Q/K/V/O attention graph layers".to_string())?;
    build_attention_graph_layer(weights, layer)
}

pub(super) fn build_attention_graph_layer(
    weights: &ModelWeights,
    layer: usize,
) -> Result<AttentionGraphLayer, Box<dyn Error>> {
    if layer >= weights.num_layers {
        return Err(format!(
            "layer {layer} out of range for model with {} layers",
            weights.num_layers
        )
        .into());
    }

    let arch = &*weights.arch;
    let input_norm_key = arch.input_layernorm_key(layer);
    if !weights.vectors.contains_key(&input_norm_key) {
        return Err(format!("missing vector {input_norm_key}").into());
    }

    let q_key = arch.attn_q_key(layer);
    let k_key = arch.attn_k_key(layer);
    let v_key = arch.attn_v_key(layer);
    let o_key = arch.attn_o_key(layer);
    let w_q = weights
        .tensors
        .get(&q_key)
        .ok_or_else(|| format!("missing tensor {q_key}"))?;
    let w_k = weights
        .tensors
        .get(&k_key)
        .ok_or_else(|| format!("missing tensor {k_key}"))?;
    let w_v = weights
        .tensors
        .get(&v_key)
        .ok_or_else(|| format!("missing tensor {v_key}"))?;
    let w_o = weights
        .tensors
        .get(&o_key)
        .ok_or_else(|| format!("missing tensor {o_key}"))?;

    let (q_rows, q_cols) = tensor_shape_2(&q_key, w_q)?;
    let (k_rows, k_cols) = tensor_shape_2(&k_key, w_k)?;
    let (v_rows, v_cols) = tensor_shape_2(&v_key, w_v)?;
    let (o_rows, o_cols) = tensor_shape_2(&o_key, w_o)?;

    if q_cols != weights.hidden_size
        || k_cols != weights.hidden_size
        || v_cols != weights.hidden_size
        || o_rows != weights.hidden_size
    {
        return Err(format!(
            "projection edge shape mismatch: hidden={} q={}x{} k={}x{} v={}x{} o={}x{}",
            weights.hidden_size, q_rows, q_cols, k_rows, k_cols, v_rows, v_cols, o_rows, o_cols
        )
        .into());
    }
    if k_rows != v_rows {
        return Err(format!("K/V output width mismatch: k={k_rows} v={v_rows}").into());
    }

    let q_norm_key = arch
        .attn_q_norm_key(layer)
        .filter(|key| weights.vectors.contains_key(key));
    let k_norm_key = arch
        .attn_k_norm_key(layer)
        .filter(|key| weights.vectors.contains_key(key));
    let q_norm_len = q_norm_key.as_ref().map(|key| weights.vectors[key].len());
    let k_norm_len = k_norm_key.as_ref().map(|key| weights.vectors[key].len());
    let head_dim = graph_head_dim(arch, layer, q_norm_len, k_norm_len, o_cols)?;

    if o_cols % head_dim != 0 || k_rows % head_dim != 0 {
        return Err(format!(
            "attention widths not divisible by head_dim: o_cols={o_cols} k_rows={k_rows} head_dim={head_dim}"
        )
        .into());
    }
    let num_q_heads = o_cols / head_dim;
    let num_kv_heads = k_rows / head_dim;
    if num_q_heads == 0 || num_kv_heads == 0 || num_q_heads % num_kv_heads != 0 {
        return Err(format!(
            "invalid graph-derived head counts: num_q={num_q_heads} num_kv={num_kv_heads} head_dim={head_dim}"
        )
        .into());
    }
    if q_rows < o_cols {
        return Err(
            format!("Q projection width {q_rows} is smaller than O input width {o_cols}").into(),
        );
    }

    Ok(AttentionGraphLayer {
        layer,
        input_norm_key,
        q_key,
        k_key,
        v_key,
        o_key,
        q_norm_key,
        k_norm_key,
        q_rows,
        q_cols,
        k_rows,
        k_cols,
        v_rows,
        v_cols,
        o_rows,
        o_cols,
        q_norm_len,
        k_norm_len,
        num_q_heads,
        num_kv_heads,
        head_dim,
        q_activation_cols: o_cols,
        kv_activation_cols: k_rows,
        q_projection_cols: q_rows,
        q_projection_tail_cols_excluded: q_rows - o_cols,
    })
}

pub(super) fn tensor_shape_2(
    key: &str,
    tensor: &WeightArray,
) -> Result<(usize, usize), Box<dyn Error>> {
    let shape = tensor.shape();
    if shape.len() != 2 {
        return Err(format!("tensor {key} is not rank-2: shape={shape:?}").into());
    }
    Ok((shape[0], shape[1]))
}

pub(super) fn graph_head_dim(
    arch: &dyn ModelArchitecture,
    layer: usize,
    q_norm_len: Option<usize>,
    k_norm_len: Option<usize>,
    attention_width: usize,
) -> Result<usize, Box<dyn Error>> {
    if let Some(len) = q_norm_len.or(k_norm_len) {
        if len == 0 || attention_width % len != 0 {
            return Err(format!(
                "Q/K norm length cannot define head_dim: len={len} attention_width={attention_width}"
            )
            .into());
        }
        return Ok(len);
    }
    let head_dim = arch.head_dim_for_layer(layer);
    if head_dim == 0 {
        return Err(format!("architecture returned zero head_dim for layer {layer}").into());
    }
    Ok(head_dim)
}

pub(super) fn build_model_proof_hidden_from_gguf_tensors(
    weights: &ModelWeights,
    graph: &AttentionGraphLayer,
    seq_len: usize,
) -> Result<(Array2<f32>, String, Vec<usize>), Box<dyn Error>> {
    let embed_shape = weights.embed.shape();
    if embed_shape.len() != 2 {
        return Err(format!("embedding tensor is not rank-2: shape={embed_shape:?}").into());
    }
    let vocab_rows = embed_shape[0];
    if seq_len <= vocab_rows {
        if let Some((hidden, row_indices)) =
            finite_rows_to_array(&weights.embed, weights.hidden_size, seq_len)?
        {
            ensure_finite_array(&hidden, "embedded hidden state")?;
            return Ok((hidden, "real_gguf_embedding_rows".to_string(), row_indices));
        }
    }

    let q = tensor(weights, &graph.q_key)?;
    if let Some((hidden, row_indices)) = finite_rows_to_array(q, weights.hidden_size, seq_len)? {
        ensure_finite_array(&hidden, "attention Q tensor row hidden input")?;
        return Ok((
            hidden,
            format!("real_gguf_tensor_rows:{}", graph.q_key),
            row_indices,
        ));
    }

    let k = tensor(weights, &graph.k_key)?;
    if let Some((hidden, row_indices)) = finite_rows_to_array(k, weights.hidden_size, seq_len)? {
        ensure_finite_array(&hidden, "attention K tensor row hidden input")?;
        return Ok((
            hidden,
            format!("real_gguf_tensor_rows:{}", graph.k_key),
            row_indices,
        ));
    }

    let v = tensor(weights, &graph.v_key)?;
    if let Some((hidden, row_indices)) = finite_rows_to_array(v, weights.hidden_size, seq_len)? {
        ensure_finite_array(&hidden, "attention V tensor row hidden input")?;
        return Ok((
            hidden,
            format!("real_gguf_tensor_rows:{}", graph.v_key),
            row_indices,
        ));
    }

    Err(format!(
        "could not find {seq_len} finite real GGUF rows in embedding or layer {} Q/K/V tensors",
        graph.layer
    )
    .into())
}

pub(super) fn finite_rows_to_array(
    tensor: &WeightArray,
    required_cols: usize,
    seq_len: usize,
) -> Result<Option<(Array2<f32>, Vec<usize>)>, Box<dyn Error>> {
    let shape = tensor.shape();
    if shape.len() != 2 || shape[1] != required_cols {
        return Ok(None);
    }
    let mut data = Vec::with_capacity(seq_len * required_cols);
    let mut row_indices = Vec::with_capacity(seq_len);
    for row_index in 0..shape[0] {
        let row = tensor.row(row_index);
        if row.iter().all(|value| value.is_finite()) {
            data.extend(row.iter().copied());
            row_indices.push(row_index);
            if row_indices.len() == seq_len {
                let hidden = Array2::from_shape_vec((seq_len, required_cols), data)?;
                return Ok(Some((hidden, row_indices)));
            }
        }
    }
    Ok(None)
}

pub(super) fn build_graph_attention_request(
    weights: &ModelWeights,
    hidden: &Array2<f32>,
    graph: &AttentionGraphLayer,
    capture_attention: bool,
) -> Result<SeparatedAttentionRequest, Box<dyn Error>> {
    let arch = &*weights.arch;
    let norm_offset = arch.norm_weight_offset();
    let qk_offset = weights.arch.qk_norm_weight_offset();
    let qk_norm_offset = if qk_offset != 0.0 {
        qk_offset
    } else {
        norm_offset
    };

    let h_norm = apply_norm(weights, hidden, &graph.input_norm_key, norm_offset);
    ensure_finite_array(&h_norm, "input-normalized hidden state")?;

    let w_q = tensor(weights, &graph.q_key)?;
    let w_k = tensor(weights, &graph.k_key)?;
    let w_v = tensor(weights, &graph.v_key)?;
    let mut q_full = dot_proj(&h_norm, w_q);
    let mut k_full = dot_proj(&h_norm, w_k);
    let mut v_full = dot_proj(&h_norm, w_v);

    if let Some(bias) = arch
        .attn_q_bias_key(graph.layer)
        .and_then(|key| weights.vectors.get(&key))
    {
        add_bias(&mut q_full, bias);
    }
    if let Some(bias) = arch
        .attn_k_bias_key(graph.layer)
        .and_then(|key| weights.vectors.get(&key))
    {
        add_bias(&mut k_full, bias);
    }
    if let Some(bias) = arch
        .attn_v_bias_key(graph.layer)
        .and_then(|key| weights.vectors.get(&key))
    {
        add_bias(&mut v_full, bias);
    }

    if q_full.shape()[1] < graph.q_activation_cols {
        return Err(format!(
            "Q projection result too narrow: cols={} required={}",
            q_full.shape()[1],
            graph.q_activation_cols
        )
        .into());
    }
    if k_full.shape()[1] != graph.kv_activation_cols
        || v_full.shape()[1] != graph.kv_activation_cols
    {
        return Err(format!(
            "K/V activation width mismatch: k={} v={} expected={}",
            k_full.shape()[1],
            v_full.shape()[1],
            graph.kv_activation_cols
        )
        .into());
    }

    let q_active = q_full.slice(s![.., 0..graph.q_activation_cols]).to_owned();
    ensure_finite_array(&q_active, "active Q projection")?;
    ensure_finite_array(&k_full, "K projection")?;
    ensure_finite_array(&v_full, "V projection")?;

    let q_normed = match graph
        .q_norm_key
        .as_ref()
        .and_then(|key| weights.vectors.get(key))
    {
        Some(norm_w) => rms_norm_heads(
            &q_active,
            norm_w,
            graph.num_q_heads,
            graph.head_dim,
            qk_norm_offset,
        ),
        None => q_active,
    };
    if arch.has_v_norm() {
        v_full = rms_norm_heads_no_weight(&v_full, graph.num_kv_heads, graph.head_dim);
    }
    let k_normed = match graph
        .k_norm_key
        .as_ref()
        .and_then(|key| weights.vectors.get(key))
    {
        Some(norm_w) => rms_norm_heads(
            &k_full,
            norm_w,
            graph.num_kv_heads,
            graph.head_dim,
            qk_norm_offset,
        ),
        None => k_full,
    };

    ensure_finite_array(&q_normed, "Q normalized projection")?;
    ensure_finite_array(&k_normed, "K normalized projection")?;
    ensure_finite_array(&v_full, "V normalized projection")?;

    let q_rope = apply_rope_partial(
        &q_normed,
        graph.num_q_heads,
        graph.head_dim,
        arch.rope_base_for_layer(graph.layer),
        arch.rotary_fraction_for_layer(graph.layer),
    );
    let k_rope = apply_rope_partial(
        &k_normed,
        graph.num_kv_heads,
        graph.head_dim,
        arch.rope_base_for_layer(graph.layer),
        arch.rotary_fraction_for_layer(graph.layer),
    );
    ensure_finite_array(&q_rope, "Q RoPE projection")?;
    ensure_finite_array(&k_rope, "K RoPE projection")?;

    Ok(SeparatedAttentionRequest {
        protocol_version: 1,
        q: AttentionTensor2::from_array(&q_rope),
        k: AttentionTensor2::from_array(&k_rope),
        v: AttentionTensor2::from_array(&v_full),
        num_q_heads: graph.num_q_heads,
        num_kv_heads: graph.num_kv_heads,
        head_dim: graph.head_dim,
        scale: attention_scale_for_layer(arch, graph.layer),
        capture_attention,
        softcap: arch.attn_logit_softcapping(),
    })
}

pub(super) fn run_graph_attention_reference(
    weights: &ModelWeights,
    hidden: &Array2<f32>,
    graph: &AttentionGraphLayer,
    request: &SeparatedAttentionRequest,
) -> Result<GraphAttentionProofOutput, Box<dyn Error>> {
    let q = request.q.to_array("q")?;
    let k = request.k.to_array("k")?;
    let v = request.v.to_array("v")?;
    let reps = graph.num_q_heads / graph.num_kv_heads;
    let seq_len = q.shape()[0];
    let (attention_output, attention_weights) = gqa_attention_with_weights(
        &q,
        &k,
        &v,
        graph.num_q_heads,
        graph.head_dim,
        reps,
        request.scale,
        seq_len,
        request.capture_attention,
        request.softcap,
    );
    complete_graph_attention(weights, hidden, graph, attention_output, attention_weights)
}

pub(super) fn run_graph_attention_remote(
    weights: &ModelWeights,
    hidden: &Array2<f32>,
    graph: &AttentionGraphLayer,
    request: &SeparatedAttentionRequest,
    addr: &str,
) -> Result<GraphAttentionProofOutput, Box<dyn Error>> {
    let response = request_separated_attention(addr, request)?;
    let attention_output = response.output.to_array("attention output")?;
    complete_graph_attention(
        weights,
        hidden,
        graph,
        attention_output,
        response
            .attention_weights
            .map(|heads| AttentionWeights { heads }),
    )
}

pub(super) fn complete_graph_attention(
    weights: &ModelWeights,
    hidden: &Array2<f32>,
    graph: &AttentionGraphLayer,
    attention_output: Array2<f32>,
    attention_weights: Option<AttentionWeights>,
) -> Result<GraphAttentionProofOutput, Box<dyn Error>> {
    let arch = &*weights.arch;
    ensure_finite_array(&attention_output, "attention output")?;
    let w_o = tensor(weights, &graph.o_key)?;
    let mut attn_projected = dot_proj(&attention_output, w_o);
    if let Some(bias) = arch
        .attn_o_bias_key(graph.layer)
        .and_then(|key| weights.vectors.get(&key))
    {
        add_bias(&mut attn_projected, bias);
    }
    ensure_finite_array(&attn_projected, "attention O projection")?;

    let norm_offset = arch.norm_weight_offset();
    let res_mult = arch.residual_multiplier();
    let h_post_attn = if arch.has_post_norms() {
        let normed = apply_norm(
            weights,
            &attn_projected,
            &arch.post_attention_layernorm_key(graph.layer),
            norm_offset,
        );
        if res_mult != 1.0 {
            hidden + &(&normed * res_mult)
        } else {
            hidden + &normed
        }
    } else if res_mult != 1.0 {
        hidden + &(&attn_projected * res_mult)
    } else {
        hidden + &attn_projected
    };
    ensure_finite_array(&h_post_attn, "post-attention residual expression")?;

    Ok(GraphAttentionProofOutput {
        h_post_attn,
        attn_projected,
        attention_output,
        attention_weights,
    })
}

pub(super) fn tensor<'a>(
    weights: &'a ModelWeights,
    key: &str,
) -> Result<&'a WeightArray, Box<dyn Error>> {
    weights
        .tensors
        .get(key)
        .ok_or_else(|| format!("missing tensor {key}").into())
}

pub(super) fn model_attention_separation() -> AttentionRuntimeProofSeparation {
    AttentionRuntimeProofSeparation {
        attention_runtime_process: "child_process_attention_runtime_serve_no_model_loader"
            .to_string(),
        weights_side_process: "parent_process_gguf_weights_qkv_projection_o_residual".to_string(),
        model_weights_sent: false,
        payload_fields: attention_payload_fields(),
    }
}

pub(super) fn prompt_model_attention_separation() -> AttentionRuntimeProofSeparation {
    AttentionRuntimeProofSeparation {
        attention_runtime_process: "child_process_attention_runtime_serve_no_model_loader"
            .to_string(),
        weights_side_process: "parent_process_real_model_prompt_residual_qkv_projection_o_residual"
            .to_string(),
        model_weights_sent: false,
        payload_fields: attention_payload_fields(),
    }
}

pub(super) fn attention_request_summary(
    request: &SeparatedAttentionRequest,
    graph: &AttentionGraphLayer,
) -> AttentionRuntimeProofRequest {
    let activation_float_count = request.q.data.len() + request.k.data.len() + request.v.data.len();
    AttentionRuntimeProofRequest {
        protocol_version: request.protocol_version,
        seq_len: request.q.rows,
        q_rows: request.q.rows,
        q_cols: request.q.cols,
        k_rows: request.k.rows,
        k_cols: request.k.cols,
        v_rows: request.v.rows,
        v_cols: request.v.cols,
        num_q_heads: graph.num_q_heads,
        num_kv_heads: graph.num_kv_heads,
        head_dim: graph.head_dim,
        activation_float_count,
        activation_f32_bytes: activation_float_count * size_of::<f32>(),
        scale: request.scale,
        capture_attention: request.capture_attention,
        softcap: request.softcap,
    }
}

pub(super) fn build_attention_wire_proof(
    weights: &ModelWeights,
    gguf: &Path,
    request: &SeparatedAttentionRequest,
) -> Result<AttentionRuntimeWireProof, Box<dyn Error>> {
    let request_body = separated_attention_request_wire_bytes(request)?;
    let request_sha256 = sha256_hex(&request_body);
    let request_value: serde_json::Value = serde_json::from_slice(&request_body)?;
    let mut request_top_level_fields = object_keys(&request_value);
    request_top_level_fields.sort();
    let mut request_payload_fields = request_value
        .get("payload")
        .map(object_keys)
        .unwrap_or_default();
    request_payload_fields.sort();

    let mut expected_top_level_fields = vec![
        "capture_kind".to_string(),
        "payload".to_string(),
        "payload_sha256".to_string(),
        "schema_version".to_string(),
    ];
    expected_top_level_fields.sort();
    let mut expected_payload_fields: Vec<String> = attention_payload_fields()
        .into_iter()
        .map(str::to_string)
        .collect();
    expected_payload_fields.sort();

    let request_capture_kind = request_value
        .get("capture_kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let request_text = String::from_utf8_lossy(&request_body);
    let mut model_names_found_in_payload = Vec::new();
    for name in weights.tensors.keys().chain(weights.vectors.keys()) {
        if request_text.contains(name) {
            model_names_found_in_payload.push(name.clone());
        }
    }
    model_names_found_in_payload.sort();
    let gguf_path_found_in_payload = request_text.contains(&gguf.display().to_string());
    let learned_weight_identifiers_sent =
        !model_names_found_in_payload.is_empty() || gguf_path_found_in_payload;
    let activation_float_count = request.q.data.len() + request.k.data.len() + request.v.data.len();

    Ok(AttentionRuntimeWireProof {
        request_bytes: request_body.len(),
        request_sha256,
        request_capture_kind,
        request_top_level_fields_match_contract: request_top_level_fields
            == expected_top_level_fields,
        request_top_level_fields,
        request_payload_fields_match_contract: request_payload_fields == expected_payload_fields,
        request_payload_fields,
        activation_float_count,
        activation_f32_bytes: activation_float_count * size_of::<f32>(),
        q_shape: [request.q.rows, request.q.cols],
        k_shape: [request.k.rows, request.k.cols],
        v_shape: [request.v.rows, request.v.cols],
        model_tensor_names_checked: weights.tensors.len(),
        model_vector_names_checked: weights.vectors.len(),
        model_names_found_in_payload,
        gguf_path_found_in_payload,
        learned_weight_identifiers_sent,
    })
}

pub(super) fn build_prompt_attention_wire_proof(
    weights: &ModelWeights,
    model: &str,
    request: &SeparatedAttentionRequest,
) -> Result<PromptAttentionRuntimeWireProof, Box<dyn Error>> {
    let request_body = separated_attention_request_wire_bytes(request)?;
    let request_sha256 = sha256_hex(&request_body);
    let request_value: serde_json::Value = serde_json::from_slice(&request_body)?;
    let mut request_top_level_fields = object_keys(&request_value);
    request_top_level_fields.sort();
    let mut request_payload_fields = request_value
        .get("payload")
        .map(object_keys)
        .unwrap_or_default();
    request_payload_fields.sort();

    let mut expected_top_level_fields = vec![
        "capture_kind".to_string(),
        "payload".to_string(),
        "payload_sha256".to_string(),
        "schema_version".to_string(),
    ];
    expected_top_level_fields.sort();
    let mut expected_payload_fields: Vec<String> = attention_payload_fields()
        .into_iter()
        .map(str::to_string)
        .collect();
    expected_payload_fields.sort();

    let request_capture_kind = request_value
        .get("capture_kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let request_text = String::from_utf8_lossy(&request_body);
    let mut model_names_found_in_payload = Vec::new();
    for name in weights.tensors.keys().chain(weights.vectors.keys()) {
        if request_text.contains(name) {
            model_names_found_in_payload.push(name.clone());
        }
    }
    model_names_found_in_payload.sort();

    let mut model_identifiers_found_in_payload = Vec::new();
    for identifier in model_identifier_candidates(model) {
        if !identifier.is_empty() && request_text.contains(&identifier) {
            model_identifiers_found_in_payload.push(identifier);
        }
    }
    model_identifiers_found_in_payload.sort();
    model_identifiers_found_in_payload.dedup();
    let learned_weight_identifiers_sent =
        !model_names_found_in_payload.is_empty() || !model_identifiers_found_in_payload.is_empty();
    let activation_float_count = request.q.data.len() + request.k.data.len() + request.v.data.len();

    Ok(PromptAttentionRuntimeWireProof {
        request_bytes: request_body.len(),
        request_sha256,
        request_capture_kind,
        request_top_level_fields_match_contract: request_top_level_fields
            == expected_top_level_fields,
        request_top_level_fields,
        request_payload_fields_match_contract: request_payload_fields == expected_payload_fields,
        request_payload_fields,
        activation_float_count,
        activation_f32_bytes: activation_float_count * size_of::<f32>(),
        q_shape: [request.q.rows, request.q.cols],
        k_shape: [request.k.rows, request.k.cols],
        v_shape: [request.v.rows, request.v.cols],
        model_tensor_names_checked: weights.tensors.len(),
        model_vector_names_checked: weights.vectors.len(),
        model_names_found_in_payload,
        model_identifiers_found_in_payload,
        learned_weight_identifiers_sent,
    })
}

pub(super) fn model_identifier_candidates(model: &str) -> Vec<String> {
    let mut identifiers = vec![model.to_string()];
    let path = Path::new(model);
    if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
        identifiers.push(name.to_string());
    }
    if let Some(parent) = path.parent().and_then(|parent| parent.to_str()) {
        identifiers.push(parent.to_string());
    }
    identifiers.sort();
    identifiers.dedup();
    identifiers
}

pub(super) fn object_keys(value: &serde_json::Value) -> Vec<String> {
    value
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default()
}

pub(super) fn attention_payload_fields() -> Vec<&'static str> {
    vec![
        "protocol_version",
        "q",
        "k",
        "v",
        "num_q_heads",
        "num_kv_heads",
        "head_dim",
        "scale",
        "capture_attention",
        "softcap",
    ]
}

pub(super) fn attention_scale_for_layer(
    arch: &dyn larql_models::ModelArchitecture,
    layer: usize,
) -> f64 {
    if arch.attention_multiplier() != 1.0 {
        arch.attention_multiplier() as f64
    } else {
        arch.attention_scale_for_layer(layer)
    }
}
