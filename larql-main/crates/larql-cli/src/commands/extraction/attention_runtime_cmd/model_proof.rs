use super::graph::*;
use super::*;

pub(super) fn run_model_attention_runtime_proof(
    gguf: &Path,
    requested_layer: Option<usize>,
    seq_len: usize,
    tolerance: f32,
) -> Result<AttentionRuntimeProof, Box<dyn Error>> {
    if seq_len == 0 {
        return Err("seq_len must be nonzero".into());
    }

    let weights = larql_models::load_gguf(gguf)?;
    let available_attention_layers = runnable_attention_layers(&weights);
    let graph =
        select_attention_graph_layer(&weights, requested_layer, &available_attention_layers)?;
    let arch = &*weights.arch;
    let (hidden, hidden_source, hidden_row_indices) =
        build_model_proof_hidden_from_gguf_tensors(&weights, &graph, seq_len)?;
    let request = build_graph_attention_request(&weights, &hidden, &graph, true)?;
    let local = run_graph_attention_reference(&weights, &hidden, &graph, &request)?;
    let wire = build_attention_wire_proof(&weights, gguf, &request)?;

    let mut runtime = spawn_attention_runtime_child()?;
    let server_pid = runtime.pid;
    let addr = runtime.addr.clone();
    let remote = match run_graph_attention_remote(&weights, &hidden, &graph, &request, &addr) {
        Ok(response) => response,
        Err(err) => {
            kill_attention_runtime_child(&mut runtime);
            return Err(err);
        }
    };
    let child_status = finish_attention_runtime_child(runtime)?;

    let h_post_diff = max_abs_diff(
        array_data(&local.h_post_attn, "local h_post_attn")?,
        array_data(&remote.h_post_attn, "remote h_post_attn")?,
    )?;
    let attention_output_diff = max_abs_diff(
        array_data(&local.attention_output, "local attention_output")?,
        array_data(&remote.attention_output, "remote attention_output")?,
    )?;
    let attn_projected_diff = max_abs_diff(
        array_data(&local.attn_projected, "local attn_projected")?,
        array_data(&remote.attn_projected, "remote attn_projected")?,
    )?;
    let attn_weight_diff =
        attention_weights_max_abs_diff(&local.attention_weights, &remote.attention_weights)?;
    let max_abs_diff = h_post_diff
        .max(attention_output_diff)
        .max(attn_projected_diff)
        .max(attn_weight_diff.unwrap_or(0.0));
    if max_abs_diff > tolerance {
        return Err(format!(
            "real separated attention proof failed: max_abs_diff={max_abs_diff} h_post={h_post_diff} attention_output={attention_output_diff} attn_projected={attn_projected_diff} attention_weights={:?} tolerance={tolerance}",
            attn_weight_diff
        )
        .into());
    }

    let scale = attention_scale_for_layer(arch, graph.layer);
    let softcap = arch.attn_logit_softcapping();
    let output_rows = remote.h_post_attn.shape()[0];
    let output_cols = remote.h_post_attn.shape()[1];
    let attention_weight_heads = remote
        .attention_weights
        .as_ref()
        .map(|weights| weights.heads.len())
        .unwrap_or(0);

    Ok(AttentionRuntimeProof {
        proof_kind: "gguf-tensor-graph-separated-attention".to_string(),
        status: "ok".to_string(),
        transport: "tcp-capsule-json-shutdown".to_string(),
        server_addr: addr,
        server_pid,
        client_pid: std::process::id(),
        reference: "local_graph_causal_gqa_real_gguf_qkvo".to_string(),
        server_exit_status: child_status,
        model: AttentionRuntimeProofModel {
            gguf_path: gguf.display().to_string(),
            requested_layer,
            layer: graph.layer,
            layer_selection: if requested_layer.is_some() {
                "requested_layer_validated_against_gguf_tensor_graph".to_string()
            } else {
                "first_runnable_qkvo_attention_layer_from_gguf_tensor_graph".to_string()
            },
            available_attention_layers,
            seq_len,
            hidden_source,
            hidden_row_indices,
            num_layers: weights.num_layers,
            hidden_size: weights.hidden_size,
            num_q_heads: graph.num_q_heads,
            num_kv_heads: graph.num_kv_heads,
            head_dim: graph.head_dim,
            q_activation_cols: graph.q_activation_cols,
            kv_activation_cols: graph.kv_activation_cols,
            q_projection_cols: graph.q_projection_cols,
            q_projection_tail_cols_excluded: graph.q_projection_tail_cols_excluded,
            loaded_model_weights_on_parent: true,
            attention_runtime_loaded_model_weights: false,
            tensor_graph: graph.to_proof_tensor_graph(),
            compared: vec![
                "h_post_attn",
                "attention_output",
                "attn_projected",
                "attention_weights",
                "child_process_exit",
                "model_weights_absent_from_wire_payload",
                "real_gguf_tensor_graph",
            ],
        },
        request: AttentionRuntimeProofRequest {
            protocol_version: 1,
            seq_len,
            q_rows: request.q.rows,
            q_cols: request.q.cols,
            k_rows: request.k.rows,
            k_cols: request.k.cols,
            v_rows: request.v.rows,
            v_cols: request.v.cols,
            num_q_heads: graph.num_q_heads,
            num_kv_heads: graph.num_kv_heads,
            head_dim: graph.head_dim,
            activation_float_count: request.q.data.len()
                + request.k.data.len()
                + request.v.data.len(),
            activation_f32_bytes: (request.q.data.len()
                + request.k.data.len()
                + request.v.data.len())
                * size_of::<f32>(),
            scale,
            capture_attention: true,
            softcap,
        },
        separation: model_attention_separation(),
        wire,
        max_abs_diff,
        h_post_max_abs_diff: h_post_diff,
        attention_output_max_abs_diff: attention_output_diff,
        attention_projected_max_abs_diff: attn_projected_diff,
        attention_weight_max_abs_diff: attn_weight_diff,
        tolerance,
        output_rows,
        output_cols,
        attention_weight_heads,
    })
}
