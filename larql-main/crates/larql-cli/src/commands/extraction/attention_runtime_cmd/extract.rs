use super::*;

pub(super) fn extract_gguf_attention_tensors(
    gguf: &Path,
    out: &Path,
    scope: AttentionInventoryScope,
) -> Result<(), Box<dyn Error>> {
    let gguf_file = GgufFile::open(gguf)?;
    let inventory = gguf_file.attention_inventory(scope)?;
    fs::create_dir_all(out)?;
    let tensors_dir = out.join("tensors");
    let capsules_dir = out.join("tensor-capsules");
    fs::create_dir_all(&tensors_dir)?;
    fs::create_dir_all(&capsules_dir)?;

    let mut source = File::open(gguf)?;
    let mut records = Vec::with_capacity(inventory.len());
    for summary in inventory {
        let basename = tensor_file_basename(&summary);
        let tensor_rel = format!("tensors/{basename}");
        let capsule_name = format!("{basename}.capsule.json");
        let capsule_rel = format!("tensor-capsules/{capsule_name}");
        let tensor_path = out.join(&tensor_rel);
        let capsule_path = out.join(&capsule_rel);
        let byte_size = usize::try_from(summary.byte_size).map_err(|_| {
            format!(
                "tensor byte size exceeds addressable memory: {}",
                summary.name
            )
        })?;
        let mut bytes = vec![0u8; byte_size];
        source.seek(SeekFrom::Start(summary.absolute_offset))?;
        source.read_exact(&mut bytes)?;
        let digest = sha256_hex(&bytes);
        fs::write(&tensor_path, &bytes)?;

        let record = GgufAttentionTensorRecord {
            name: summary.name,
            normalized_name: summary.normalized_name,
            scope: summary.scope,
            component: summary.component,
            role: summary.role,
            param: summary.param,
            layer: summary.layer,
            dims: summary.dims,
            ggml_type_id: summary.ggml_type_id,
            ggml_type: summary.ggml_type,
            relative_offset: summary.relative_offset,
            absolute_offset: summary.absolute_offset,
            byte_size: summary.byte_size,
            included_in_runtime_vindex: summary.included_in_runtime_vindex,
            file: tensor_rel,
            capsule: capsule_rel,
            sha256: digest,
        };
        let tensor_capsule = make_runtime_capsule(
            TENSOR_FILE_CAPTURE_KIND,
            TENSOR_FILE_CAPSULE_KIND,
            record.clone(),
        )?;
        write_capsule_json(&capsule_path, &tensor_capsule)?;
        records.push(record);
    }

    let total_attention_tensor_bytes = records.iter().map(|record| record.byte_size).sum();
    let report = GgufAttentionExtractionReport {
        gguf_path: gguf.display().to_string(),
        scope: scope.as_str().to_string(),
        data_offset: gguf_file.data_offset,
        total_gguf_tensors: gguf_file.tensor_infos.len(),
        selected_tensor_count: records.len(),
        total_attention_tensor_bytes,
        tensors: records,
    };
    let report_capsule = make_runtime_capsule(
        EXTRACTION_REPORT_CAPTURE_KIND,
        EXTRACTION_REPORT_CAPSULE_KIND,
        report,
    )?;
    write_capsule_json(&out.join("attention_manifest.json"), &report_capsule)?;
    Ok(())
}

fn tensor_file_basename(summary: &GgufTensorSummary) -> String {
    let scope = sanitize_path_segment(&summary.scope);
    let layer = summary
        .layer
        .map(|layer| format!("layer-{layer:03}"))
        .unwrap_or_else(|| sanitize_path_segment(&summary.normalized_name));
    let component = sanitize_path_segment(&summary.component);
    let param = sanitize_path_segment(&summary.param);
    let ggml_type = sanitize_path_segment(&summary.ggml_type);
    format!("{scope}.{layer}.{component}.{param}.{ggml_type}.bin")
}

fn sanitize_path_segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut last_was_separator = false;
    for ch in value.chars() {
        let next = if ch.is_ascii_alphanumeric() {
            last_was_separator = false;
            Some(ch.to_ascii_lowercase())
        } else if matches!(ch, '-' | '_') {
            last_was_separator = false;
            Some(ch)
        } else if !last_was_separator {
            last_was_separator = true;
            Some('_')
        } else {
            None
        };
        if let Some(ch) = next {
            out.push(ch);
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed.to_string()
    }
}
