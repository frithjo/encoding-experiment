use super::graph::*;
use super::*;

pub(super) fn run_prompt_attention_runtime_proof(
    model: &str,
    tokenizer_path: Option<&Path>,
    prompts_file: &Path,
    layers: &[usize],
    seq_lens: &[usize],
    tolerance: f32,
    max_prompts: usize,
    output_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    if layers.is_empty() {
        return Err("at least one layer is required".into());
    }
    if seq_lens.is_empty() || seq_lens.iter().any(|seq_len| *seq_len == 0) {
        return Err("seq_lens must contain nonzero values".into());
    }

    let model_path = resolve_model_path(model)?;
    let weights = load_model_dir(&model_path)?;
    let tokenizer = load_prompt_proof_tokenizer(&model_path, tokenizer_path)?;
    let prompts_read = read_prompt_records(prompts_file)?;
    let prompt_count_read = prompts_read.len();
    let prompts: Vec<PromptRecord> = if max_prompts == 0 {
        prompts_read
    } else {
        prompts_read.into_iter().take(max_prompts).collect()
    };
    let available_attention_layers = runnable_attention_layers(&weights);

    let mut cells = Vec::new();
    for prompt in &prompts {
        let encoded = match tokenizer.encode(&prompt.text, false) {
            Ok(encoded) => encoded,
            Err(err) => {
                for &seq_len in seq_lens {
                    for &layer in layers {
                        cells.push(prompt_cell_failed(
                            prompt,
                            0,
                            seq_len,
                            layer,
                            "tokenize",
                            err.to_string(),
                        ));
                    }
                }
                continue;
            }
        };
        let token_ids_all: Vec<u32> = encoded.ids.clone();

        for &seq_len in seq_lens {
            for &layer in layers {
                if token_ids_all.len() < seq_len {
                    cells.push(prompt_cell_skipped(
                        prompt,
                        token_ids_all.len(),
                        seq_len,
                        layer,
                        "token_count_below_seq_len",
                    ));
                    continue;
                }
                if layer >= weights.num_layers {
                    cells.push(prompt_cell_skipped(
                        prompt,
                        token_ids_all.len(),
                        seq_len,
                        layer,
                        "layer_out_of_range",
                    ));
                    continue;
                }
                if !available_attention_layers.contains(&layer) {
                    cells.push(prompt_cell_skipped(
                        prompt,
                        token_ids_all.len(),
                        seq_len,
                        layer,
                        "layer_missing_runnable_qkvo_attention_graph",
                    ));
                    continue;
                }

                let token_ids = &token_ids_all[..seq_len];
                cells.push(run_prompt_attention_runtime_cell(
                    model,
                    &weights,
                    prompt,
                    token_ids,
                    token_ids_all.len(),
                    seq_len,
                    layer,
                    tolerance,
                ));
            }
        }
    }

    let aggregate = aggregate_prompt_cells(&cells);
    let status = if aggregate.failed_cells == 0
        && aggregate.skipped_cells == 0
        && aggregate.ok_cells == aggregate.total_cells
        && aggregate.total_cells > 0
    {
        "ok"
    } else {
        "incomplete"
    };
    let report = PromptAttentionRuntimeProofReport {
        proof_kind: "prompt-derived-separated-attention-runtime".to_string(),
        status: status.to_string(),
        model: model.to_string(),
        prompts_file: prompts_file.display().to_string(),
        prompt_count_read,
        prompt_count_used: prompts.len(),
        layers: layers.to_vec(),
        seq_lens: seq_lens.to_vec(),
        tolerance,
        support_conditions: vec![
            "real_model_loaded_on_parent".to_string(),
            "prompt_tokens_generate_residual_state".to_string(),
            "child_runtime_receives_only_qkv_activation_payload".to_string(),
            "wire_payload_contains_no_model_path_or_tensor_names".to_string(),
            "local_and_child_attention_outputs_match_within_tolerance".to_string(),
        ],
        aggregate,
        cells,
    };

    fs::create_dir_all(output_dir)?;
    let cells_path = output_dir.join("prompt-separated-attention-runtime-cells.jsonl");
    let mut cells_file = File::create(&cells_path)?;
    for cell in &report.cells {
        serde_json::to_writer(&mut cells_file, cell)?;
        cells_file.write_all(b"\n")?;
    }
    let capsule =
        make_runtime_capsule(PROMPT_PROOF_CAPTURE_KIND, PROMPT_PROOF_CAPSULE_KIND, report)?;
    write_capsule_json(
        &output_dir.join("prompt-separated-attention-runtime-proof.json"),
        &capsule,
    )?;
    Ok(())
}

fn load_prompt_proof_tokenizer(
    model_path: &Path,
    tokenizer_path: Option<&Path>,
) -> Result<Arc<dyn Tokenizer>, Box<dyn Error>> {
    let path = if let Some(path) = tokenizer_path {
        path.to_path_buf()
    } else {
        model_path.join("tokenizer.json")
    };
    if !path.exists() {
        return Err(format!(
            "tokenizer file not found at {}; pass --tokenizer <tokenizer.json> for GGUF or external tokenizer layouts",
            path.display()
        )
        .into());
    }
    larql_tokenizer::load_tokenizer(&path)
        .map_err(|err| format!("failed to load tokenizer {}: {err}", path.display()).into())
}

fn run_prompt_attention_runtime_cell(
    model: &str,
    weights: &ModelWeights,
    prompt: &PromptRecord,
    token_ids: &[u32],
    token_count_original: usize,
    seq_len: usize,
    layer: usize,
    tolerance: f32,
) -> PromptAttentionRuntimeProofCell {
    let hidden_source = if layer == 0 {
        "prompt_embedding_output_pre_attention_layer_0".to_string()
    } else {
        format!(
            "prompt_forward_to_layer_{}_pre_attention_layer_{layer}",
            layer - 1
        )
    };

    let graph = match build_attention_graph_layer(weights, layer) {
        Ok(graph) => graph,
        Err(err) => {
            return prompt_cell_failed(
                prompt,
                token_count_original,
                seq_len,
                layer,
                &hidden_source,
                err.to_string(),
            )
        }
    };
    let hidden = if layer == 0 {
        embed_tokens_pub(weights, token_ids)
    } else {
        forward_to_layer(weights, token_ids, layer - 1)
    };
    if let Err(err) = ensure_finite_array(&hidden, "prompt-derived hidden state") {
        return prompt_cell_failed(
            prompt,
            token_count_original,
            seq_len,
            layer,
            &hidden_source,
            err.to_string(),
        );
    }

    let request = match build_graph_attention_request(weights, &hidden, &graph, true) {
        Ok(request) => request,
        Err(err) => {
            return prompt_cell_failed(
                prompt,
                token_count_original,
                seq_len,
                layer,
                &hidden_source,
                err.to_string(),
            )
        }
    };
    let request_summary = attention_request_summary(&request, &graph);
    let local = match run_graph_attention_reference(weights, &hidden, &graph, &request) {
        Ok(local) => local,
        Err(err) => {
            return prompt_cell_failed(
                prompt,
                token_count_original,
                seq_len,
                layer,
                &hidden_source,
                err.to_string(),
            )
        }
    };
    let wire = match build_prompt_attention_wire_proof(weights, model, &request) {
        Ok(wire) => wire,
        Err(err) => {
            return prompt_cell_failed(
                prompt,
                token_count_original,
                seq_len,
                layer,
                &hidden_source,
                err.to_string(),
            )
        }
    };

    let mut runtime = match spawn_attention_runtime_child() {
        Ok(runtime) => runtime,
        Err(err) => {
            return prompt_cell_failed(
                prompt,
                token_count_original,
                seq_len,
                layer,
                &hidden_source,
                err.to_string(),
            )
        }
    };
    let server_pid = runtime.pid;
    let addr = runtime.addr.clone();
    let remote = match run_graph_attention_remote(weights, &hidden, &graph, &request, &addr) {
        Ok(remote) => remote,
        Err(err) => {
            kill_attention_runtime_child(&mut runtime);
            return prompt_cell_failed(
                prompt,
                token_count_original,
                seq_len,
                layer,
                &hidden_source,
                err.to_string(),
            );
        }
    };
    let child_status = match finish_attention_runtime_child(runtime) {
        Ok(status) => status,
        Err(err) => {
            return prompt_cell_failed(
                prompt,
                token_count_original,
                seq_len,
                layer,
                &hidden_source,
                err.to_string(),
            )
        }
    };

    let h_post_diff = max_abs_diff(
        array_data(&local.h_post_attn, "local h_post_attn").unwrap_or(&[]),
        array_data(&remote.h_post_attn, "remote h_post_attn").unwrap_or(&[]),
    );
    let attention_output_diff = max_abs_diff(
        array_data(&local.attention_output, "local attention_output").unwrap_or(&[]),
        array_data(&remote.attention_output, "remote attention_output").unwrap_or(&[]),
    );
    let attn_projected_diff = max_abs_diff(
        array_data(&local.attn_projected, "local attn_projected").unwrap_or(&[]),
        array_data(&remote.attn_projected, "remote attn_projected").unwrap_or(&[]),
    );
    let attn_weight_diff =
        attention_weights_max_abs_diff(&local.attention_weights, &remote.attention_weights);

    let diffs = match (
        h_post_diff,
        attention_output_diff,
        attn_projected_diff,
        attn_weight_diff,
    ) {
        (Ok(h), Ok(a), Ok(p), Ok(w)) => (h, a, p, w),
        (h, a, p, w) => {
            return prompt_cell_failed(
                prompt,
                token_count_original,
                seq_len,
                layer,
                &hidden_source,
                format!(
                    "diff_error h_post={:?} attention_output={:?} attn_projected={:?} attention_weights={:?}",
                    h.err(),
                    a.err(),
                    p.err(),
                    w.err()
                ),
            )
        }
    };
    let max_abs_diff = diffs
        .0
        .max(diffs.1)
        .max(diffs.2)
        .max(diffs.3.unwrap_or(0.0));
    let wire_failed = wire.learned_weight_identifiers_sent
        || !wire.request_top_level_fields_match_contract
        || !wire.request_payload_fields_match_contract;
    let status = if max_abs_diff <= tolerance && !wire_failed {
        "ok"
    } else {
        "failed"
    };
    let error = if status == "ok" {
        None
    } else {
        Some(format!(
            "max_abs_diff={max_abs_diff} tolerance={tolerance} wire_failed={wire_failed}"
        ))
    };

    PromptAttentionRuntimeProofCell {
        prompt_id: prompt.id.clone(),
        prompt_sha256: sha256_hex(prompt.text.as_bytes()),
        prompt_chars: prompt.text.chars().count(),
        token_count_original,
        seq_len,
        layer,
        status: status.to_string(),
        skipped_reason: None,
        error,
        hidden_source,
        server_addr: Some(addr),
        server_pid: Some(server_pid),
        server_exit_status: Some(child_status),
        request: Some(request_summary),
        separation: Some(prompt_model_attention_separation()),
        wire: Some(wire),
        max_abs_diff: Some(max_abs_diff),
        h_post_max_abs_diff: Some(diffs.0),
        attention_output_max_abs_diff: Some(diffs.1),
        attention_projected_max_abs_diff: Some(diffs.2),
        attention_weight_max_abs_diff: diffs.3,
        output_rows: Some(remote.h_post_attn.shape()[0]),
        output_cols: Some(remote.h_post_attn.shape()[1]),
        attention_weight_heads: Some(
            remote
                .attention_weights
                .as_ref()
                .map(|weights| weights.heads.len())
                .unwrap_or(0),
        ),
    }
}

fn read_prompt_records(path: &Path) -> Result<Vec<PromptRecord>, Box<dyn Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut prompts = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let default_id = format!("line_{}", index + 1);
        let record = match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(value) => {
                let id = value
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(&default_id)
                    .to_string();
                let text = value
                    .get("text")
                    .or_else(|| value.get("prompt"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
                match text {
                    Some(text) if !text.trim().is_empty() => PromptRecord { id, text },
                    _ => continue,
                }
            }
            Err(_) => PromptRecord {
                id: default_id,
                text: trimmed.to_string(),
            },
        };
        prompts.push(record);
    }
    if prompts.is_empty() {
        return Err(format!("no prompts found in {}", path.display()).into());
    }
    Ok(prompts)
}

fn prompt_cell_skipped(
    prompt: &PromptRecord,
    token_count_original: usize,
    seq_len: usize,
    layer: usize,
    reason: &str,
) -> PromptAttentionRuntimeProofCell {
    PromptAttentionRuntimeProofCell {
        prompt_id: prompt.id.clone(),
        prompt_sha256: sha256_hex(prompt.text.as_bytes()),
        prompt_chars: prompt.text.chars().count(),
        token_count_original,
        seq_len,
        layer,
        status: "skipped".to_string(),
        skipped_reason: Some(reason.to_string()),
        error: None,
        hidden_source: "not_computed".to_string(),
        server_addr: None,
        server_pid: None,
        server_exit_status: None,
        request: None,
        separation: None,
        wire: None,
        max_abs_diff: None,
        h_post_max_abs_diff: None,
        attention_output_max_abs_diff: None,
        attention_projected_max_abs_diff: None,
        attention_weight_max_abs_diff: None,
        output_rows: None,
        output_cols: None,
        attention_weight_heads: None,
    }
}

fn prompt_cell_failed(
    prompt: &PromptRecord,
    token_count_original: usize,
    seq_len: usize,
    layer: usize,
    hidden_source: &str,
    error: String,
) -> PromptAttentionRuntimeProofCell {
    PromptAttentionRuntimeProofCell {
        prompt_id: prompt.id.clone(),
        prompt_sha256: sha256_hex(prompt.text.as_bytes()),
        prompt_chars: prompt.text.chars().count(),
        token_count_original,
        seq_len,
        layer,
        status: "failed".to_string(),
        skipped_reason: None,
        error: Some(error),
        hidden_source: hidden_source.to_string(),
        server_addr: None,
        server_pid: None,
        server_exit_status: None,
        request: None,
        separation: None,
        wire: None,
        max_abs_diff: None,
        h_post_max_abs_diff: None,
        attention_output_max_abs_diff: None,
        attention_projected_max_abs_diff: None,
        attention_weight_max_abs_diff: None,
        output_rows: None,
        output_cols: None,
        attention_weight_heads: None,
    }
}

fn aggregate_prompt_cells(
    cells: &[PromptAttentionRuntimeProofCell],
) -> PromptAttentionRuntimeProofAggregate {
    let mut aggregate = PromptAttentionRuntimeProofAggregate {
        total_cells: cells.len(),
        ..Default::default()
    };
    for cell in cells {
        match cell.status.as_str() {
            "ok" => aggregate.ok_cells += 1,
            "skipped" => aggregate.skipped_cells += 1,
            "failed" => aggregate.failed_cells += 1,
            _ => {}
        }
        update_optional_max(&mut aggregate.max_abs_diff, cell.max_abs_diff);
        update_optional_max(&mut aggregate.h_post_max_abs_diff, cell.h_post_max_abs_diff);
        update_optional_max(
            &mut aggregate.attention_output_max_abs_diff,
            cell.attention_output_max_abs_diff,
        );
        update_optional_max(
            &mut aggregate.attention_projected_max_abs_diff,
            cell.attention_projected_max_abs_diff,
        );
        update_optional_max(
            &mut aggregate.attention_weight_max_abs_diff,
            cell.attention_weight_max_abs_diff,
        );
    }
    aggregate
}

fn update_optional_max(target: &mut Option<f32>, candidate: Option<f32>) {
    if let Some(candidate) = candidate {
        *target = Some(
            target
                .map(|value| value.max(candidate))
                .unwrap_or(candidate),
        );
    }
}
