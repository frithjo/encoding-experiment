use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

#[derive(Args)]
pub struct ConvertArgs {
    #[command(subcommand)]
    command: ConvertCommand,
}

#[derive(Subcommand)]
enum ConvertCommand {
    /// Convert a GGUF model to a vindex.
    GgufToVindex {
        /// Path to the .gguf file.
        input: PathBuf,

        /// Output vindex directory.
        #[arg(short, long)]
        output: PathBuf,

        /// Extract level: browse (default), inference, all.
        #[arg(long, default_value = "browse")]
        level: String,

        /// Store in f16 (half precision).
        #[arg(long)]
        f16: bool,

        /// Path to tokenizer.json. Defaults to tokenizer.json next to the GGUF.
        #[arg(long)]
        tokenizer: Option<PathBuf>,

        /// Attention proof scope: decoder, decoder-mtp, all.
        #[arg(long, default_value = "decoder")]
        attention_scope: String,
    },

    /// Extract raw GGUF attention tensors for inspection.
    GgufAttentionExtract {
        /// Path to the .gguf file.
        input: PathBuf,

        /// Output directory for source_attention.bin and proof JSON.
        #[arg(short, long)]
        output: PathBuf,

        /// Attention scope: decoder, decoder-mtp, all.
        #[arg(long, default_value = "decoder")]
        attention_scope: String,
    },

    /// Verify extracted GGUF attention tensors against the source file.
    GgufAttentionVerify {
        /// Path to the source .gguf file.
        input: PathBuf,

        /// Output directory produced by gguf-attention-extract.
        #[arg(short, long)]
        extract: PathBuf,
    },

    /// Extract and byte-verify raw GGUF attention tensors in one step.
    GgufAttentionProof {
        /// Path to the source .gguf file.
        input: PathBuf,

        /// Output directory for proof artifacts.
        #[arg(short, long)]
        output: PathBuf,

        /// Attention scope: decoder, decoder-mtp, all.
        #[arg(long, default_value = "decoder")]
        attention_scope: String,
    },

    /// Convert a safetensors model to a vindex (alias for extract-index).
    SafetensorsToVindex {
        /// Path to the model directory.
        input: PathBuf,

        /// Output vindex directory.
        #[arg(short, long)]
        output: PathBuf,

        /// Extract level: browse (default), inference, all.
        #[arg(long, default_value = "browse")]
        level: String,

        /// Store in f16.
        #[arg(long)]
        f16: bool,
    },

    /// Show GGUF file metadata and tensor info.
    GgufInfo {
        /// Path to the .gguf file.
        input: PathBuf,
    },
}

pub fn run(args: ConvertArgs) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        ConvertCommand::GgufToVindex {
            input,
            output,
            level,
            f16,
            tokenizer,
            attention_scope,
        } => run_gguf_to_vindex(
            &input,
            &output,
            &level,
            f16,
            tokenizer.as_deref(),
            &attention_scope,
        ),
        ConvertCommand::GgufAttentionExtract {
            input,
            output,
            attention_scope,
        } => run_gguf_attention_extract(&input, &output, &attention_scope),
        ConvertCommand::GgufAttentionVerify { input, extract } => {
            run_gguf_attention_verify(&input, &extract)
        }
        ConvertCommand::GgufAttentionProof {
            input,
            output,
            attention_scope,
        } => run_gguf_attention_proof(&input, &output, &attention_scope),
        ConvertCommand::SafetensorsToVindex {
            input,
            output,
            level,
            f16,
        } => run_safetensors_to_vindex(&input, &output, &level, f16),
        ConvertCommand::GgufInfo { input } => run_gguf_info(&input),
    }
}

fn run_gguf_to_vindex(
    input: &Path,
    output: &Path,
    level: &str,
    use_f16: bool,
    tokenizer: Option<&Path>,
    attention_scope: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Loading GGUF: {}", input.display());

    let gguf = larql_models::loading::gguf::GgufFile::open(input)?;
    let extract_level = parse_extract_level(level)?;
    let attention_scope = parse_attention_scope(attention_scope)?;
    let tokenizer = resolve_gguf_tokenizer(input, tokenizer)?;

    // Show metadata summary
    if let Some(name) = gguf.metadata.get("general.name") {
        eprintln!("  Model: {:?}", name);
    }
    if let Some(arch) = gguf.metadata.get("general.architecture") {
        eprintln!("  Architecture: {:?}", arch);
    }

    eprintln!("  Loading and dequantizing tensors...");
    let weights = larql_models::load_gguf(input)?;

    eprintln!(
        "  {} layers, hidden_size={}, intermediate_size={}, vocab_size={}",
        weights.num_layers, weights.hidden_size, weights.intermediate_size, weights.vocab_size
    );

    let dtype = if use_f16 {
        larql_vindex::StorageDtype::F16
    } else {
        larql_vindex::StorageDtype::F32
    };

    let model_name = gguf
        .metadata
        .get("general.name")
        .and_then(|v| v.as_str())
        .unwrap_or("gguf-model")
        .to_string();

    eprintln!("\nExtracting to {}", output.display());

    let mut callbacks = SilentCallbacks;
    larql_vindex::build_vindex(
        &weights,
        &*tokenizer,
        &model_name,
        output,
        10,
        extract_level,
        dtype,
        &mut callbacks,
    )?;
    write_attention_vindex_proof(&gguf, output, attention_scope, extract_level, dtype)?;

    eprintln!("Done: {}", output.display());
    eprintln!(
        "Attention proof: {}",
        output.join("attention_proof.json").display()
    );
    Ok(())
}

fn run_safetensors_to_vindex(
    input: &Path,
    output: &Path,
    level: &str,
    use_f16: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // This is essentially extract-index
    eprintln!("Loading safetensors: {}", input.display());
    let weights = larql_models::load_model_dir(input)?;
    let tokenizer = larql_vindex::load_vindex_tokenizer(input).or_else(|_| {
        // Try to load from the model directory
        let tok_path = input.join("tokenizer.json");
        larql_tokenizer::load_tokenizer(&tok_path)
            .map_err(|e| larql_vindex::VindexError::Parse(e.to_string()))
    })?;

    let extract_level = parse_extract_level(level)?;

    let dtype = if use_f16 {
        larql_vindex::StorageDtype::F16
    } else {
        larql_vindex::StorageDtype::F32
    };

    let model_name = input
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "model".into());

    eprintln!("Extracting to {}", output.display());

    let mut callbacks = SilentCallbacks;
    larql_vindex::build_vindex(
        &weights,
        &*tokenizer,
        &model_name,
        output,
        10,
        extract_level,
        dtype,
        &mut callbacks,
    )?;

    eprintln!("Done: {}", output.display());
    Ok(())
}

fn run_gguf_attention_extract(
    input: &Path,
    output: &Path,
    attention_scope: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let gguf = larql_models::loading::gguf::GgufFile::open(input)?;
    let scope = parse_attention_scope(attention_scope)?;
    write_source_attention_extract(&gguf, output, scope)?;

    eprintln!("Attention extract: {}", output.display());
    eprintln!("  raw: {}", output.join("source_attention.bin").display());
    eprintln!("  proof: {}", output.join("attention_proof.json").display());
    Ok(())
}

fn run_gguf_attention_proof(
    input: &Path,
    output: &Path,
    attention_scope: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    run_gguf_attention_extract(input, output, attention_scope)?;
    run_gguf_attention_verify(input, output)?;

    eprintln!("Attention proof complete: {}", output.display());
    Ok(())
}

fn run_gguf_attention_verify(
    input: &Path,
    extract: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let gguf = larql_models::loading::gguf::GgufFile::open(input)?;
    let inventory_path = extract.join("attention_inventory.json");
    let raw_path = extract.join("source_attention.bin");

    let inventory_json: Vec<serde_json::Value> =
        serde_json::from_str(&fs::read_to_string(&inventory_path)?)?;
    let expected_by_name: BTreeMap<String, larql_models::loading::gguf::GgufTensorSummary> = gguf
        .attention_inventory(larql_models::loading::gguf::AttentionInventoryScope::All)?
        .into_iter()
        .map(|tensor| (tensor.name.clone(), tensor))
        .collect();

    let mut source = BufReader::new(fs::File::open(input)?);
    let mut packed = BufReader::new(fs::File::open(&raw_path)?);
    let mut seen = BTreeSet::new();
    let mut expected_packed_offset = 0u64;
    let mut verified_bytes = 0u64;

    for entry in &inventory_json {
        let name = json_str(entry, "name")?;
        if !seen.insert(name.to_string()) {
            return Err(invalid_input(format!(
                "duplicate tensor in inventory: {name}"
            )));
        }

        let expected = expected_by_name
            .get(name)
            .ok_or_else(|| invalid_input(format!("non-attention tensor in inventory: {name}")))?;
        let absolute_offset = json_u64(entry, "absolute_offset")?;
        let byte_size = json_u64(entry, "byte_size")?;
        let packed_offset = json_u64(entry, "packed_offset")?;
        let packed_length = json_u64(entry, "packed_length")?;

        if absolute_offset != expected.absolute_offset || byte_size != expected.byte_size {
            return Err(invalid_input(format!(
                "source metadata mismatch for {name}: inventory offset/bytes={absolute_offset}/{byte_size}, source offset/bytes={}/{}",
                expected.absolute_offset, expected.byte_size
            )));
        }
        if packed_offset != expected_packed_offset || packed_length != byte_size {
            return Err(invalid_input(format!(
                "packed layout mismatch for {name}: packed offset/length={packed_offset}/{packed_length}, expected {expected_packed_offset}/{byte_size}"
            )));
        }

        verify_same_bytes(
            &mut source,
            absolute_offset,
            &mut packed,
            packed_offset,
            byte_size,
            name,
        )?;
        expected_packed_offset += byte_size;
        verified_bytes += byte_size;
    }

    let raw_len = fs::metadata(&raw_path)?.len();
    if raw_len != verified_bytes {
        return Err(invalid_input(format!(
            "source_attention.bin size mismatch: file={raw_len}, verified={verified_bytes}"
        )));
    }

    let sha256 = larql_vindex::checksums::sha256_file(&raw_path)?;
    verify_attention_proof(extract, inventory_json.len(), verified_bytes, &sha256)?;

    let verification = serde_json::json!({
        "status": "ok",
        "source_path": input.display().to_string(),
        "extract_path": extract.display().to_string(),
        "verified_tensor_count": inventory_json.len(),
        "verified_attention_bytes": verified_bytes,
        "source_attention_sha256": sha256,
    });
    fs::write(
        extract.join("attention_verification.json"),
        serde_json::to_string_pretty(&verification)?,
    )?;

    eprintln!("Attention verify: OK");
    eprintln!("  tensors: {}", inventory_json.len());
    eprintln!("  bytes: {}", verified_bytes);
    eprintln!("  sha256: {}", sha256);
    eprintln!(
        "  certificate: {}",
        extract.join("attention_verification.json").display()
    );
    Ok(())
}

fn run_gguf_info(input: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let gguf = larql_models::loading::gguf::GgufFile::open(input)?;

    println!("GGUF: {}", input.display());
    println!();

    // Print metadata
    println!("Metadata ({} keys):", gguf.metadata.len());
    let mut keys: Vec<&String> = gguf.metadata.keys().collect();
    keys.sort();
    for key in &keys {
        let val = &gguf.metadata[*key];
        match val {
            larql_models::loading::gguf::GgufValue::String(s) => {
                if s.len() > 80 {
                    println!("  {}: \"{}...\"", key, &s[..80]);
                } else {
                    println!("  {}: \"{}\"", key, s);
                }
            }
            larql_models::loading::gguf::GgufValue::Array(arr) => {
                println!("  {}: [{} elements]", key, arr.len());
            }
            other => println!("  {}: {:?}", key, other),
        }
    }

    println!();

    // Print synthesised config
    let config = gguf.to_config_json();
    println!("Detected config:");
    println!("  {}", serde_json::to_string_pretty(&config)?);

    Ok(())
}

fn parse_extract_level(
    level: &str,
) -> Result<larql_vindex::ExtractLevel, Box<dyn std::error::Error>> {
    match level {
        "browse" => Ok(larql_vindex::ExtractLevel::Browse),
        "inference" => Ok(larql_vindex::ExtractLevel::Inference),
        "all" => Ok(larql_vindex::ExtractLevel::All),
        other => Err(invalid_input(format!(
            "invalid extract level {other:?}; expected browse, inference, or all"
        ))),
    }
}

fn parse_attention_scope(
    scope: &str,
) -> Result<larql_models::loading::gguf::AttentionInventoryScope, Box<dyn std::error::Error>> {
    match scope {
        "decoder" => Ok(larql_models::loading::gguf::AttentionInventoryScope::Decoder),
        "decoder-mtp" | "decoder_mtp" => {
            Ok(larql_models::loading::gguf::AttentionInventoryScope::DecoderMtp)
        }
        "all" => Ok(larql_models::loading::gguf::AttentionInventoryScope::All),
        other => Err(invalid_input(format!(
            "invalid attention scope {other:?}; expected decoder, decoder-mtp, or all"
        ))),
    }
}

fn resolve_gguf_tokenizer(
    input: &Path,
    explicit: Option<&Path>,
) -> Result<std::sync::Arc<dyn larql_tokenizer::Tokenizer>, Box<dyn std::error::Error>> {
    let tokenizer_path = if let Some(path) = explicit {
        path.to_path_buf()
    } else {
        input
            .parent()
            .map(|dir| dir.join("tokenizer.json"))
            .filter(|path| path.exists())
            .ok_or_else(|| {
                invalid_input(
                    "tokenizer.json not found for GGUF conversion; pass --tokenizer <path> or place tokenizer.json next to <input>",
                )
            })?
    };

    larql_tokenizer::load_tokenizer(&tokenizer_path).map_err(|err| {
        invalid_input(format!(
            "failed to load tokenizer {}: {err}",
            tokenizer_path.display()
        ))
    })
}

fn write_source_attention_extract(
    gguf: &larql_models::loading::gguf::GgufFile,
    output: &Path,
    scope: larql_models::loading::gguf::AttentionInventoryScope,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(output)?;
    let inventory = gguf.attention_inventory(scope)?;

    let raw_path = output.join("source_attention.bin");
    let mut input = BufReader::new(fs::File::open(&gguf.path)?);
    let mut raw = BufWriter::new(fs::File::create(&raw_path)?);
    let mut packed_inventory = Vec::with_capacity(inventory.len());
    let mut packed_offset = 0u64;

    for tensor in &inventory {
        input.seek(SeekFrom::Start(tensor.absolute_offset))?;
        let mut limited = input.by_ref().take(tensor.byte_size);
        let copied = std::io::copy(&mut limited, &mut raw)?;
        if copied != tensor.byte_size {
            return Err(invalid_input(format!(
                "short read for {}: copied {} of {} bytes",
                tensor.name, copied, tensor.byte_size
            )));
        }

        let mut value = serde_json::to_value(tensor)?;
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "packed_file".into(),
                serde_json::json!("source_attention.bin"),
            );
            obj.insert("packed_offset".into(), serde_json::json!(packed_offset));
            obj.insert("packed_length".into(), serde_json::json!(copied));
        }
        packed_inventory.push(value);
        packed_offset += copied;
    }
    raw.flush()?;
    drop(raw);

    fs::write(
        output.join("attention_inventory.json"),
        serde_json::to_string_pretty(&packed_inventory)?,
    )?;

    let raw_sha256 = larql_vindex::checksums::sha256_file(&raw_path)?;
    let proof = attention_proof_json(
        gguf,
        scope,
        &inventory,
        Some(serde_json::json!({
            "attention_raw_file": "source_attention.bin",
            "attention_raw_bytes": packed_offset,
            "attention_raw_sha256": raw_sha256,
        })),
        None,
    );
    fs::write(
        output.join("attention_proof.json"),
        serde_json::to_string_pretty(&proof)?,
    )?;
    Ok(())
}

fn write_attention_vindex_proof(
    gguf: &larql_models::loading::gguf::GgufFile,
    output: &Path,
    scope: larql_models::loading::gguf::AttentionInventoryScope,
    extract_level: larql_vindex::ExtractLevel,
    dtype: larql_vindex::StorageDtype,
) -> Result<(), Box<dyn std::error::Error>> {
    let inventory = gguf.attention_inventory(scope)?;
    fs::write(
        output.join("attention_inventory.json"),
        serde_json::to_string_pretty(&inventory)?,
    )?;

    let (manifest_attn_entries, manifest_attn_bytes, manifest_attn_layers) =
        read_weight_manifest_attention(output)?;
    let attn_weights_bin_bytes = fs::metadata(output.join("attn_weights.bin"))
        .ok()
        .map(|m| m.len());
    let proof = attention_proof_json(
        gguf,
        scope,
        &inventory,
        None,
        Some(serde_json::json!({
            "extract_level": extract_level.to_string(),
            "dtype": dtype.to_string(),
            "attn_weights_bin_bytes": attn_weights_bin_bytes,
            "weight_manifest_attn_entries": manifest_attn_entries,
            "weight_manifest_attn_bytes": manifest_attn_bytes,
            "weight_manifest_attn_layers": manifest_attn_layers,
        })),
    );
    fs::write(
        output.join("attention_proof.json"),
        serde_json::to_string_pretty(&proof)?,
    )?;
    Ok(())
}

fn read_weight_manifest_attention(
    output: &Path,
) -> Result<(usize, u64, Vec<usize>), Box<dyn std::error::Error>> {
    let path = output.join("weight_manifest.json");
    if !path.exists() {
        return Ok((0, 0, Vec::new()));
    }

    let manifest: Vec<serde_json::Value> = serde_json::from_str(&fs::read_to_string(path)?)?;
    let mut count = 0usize;
    let mut bytes = 0u64;
    let mut layers = BTreeSet::new();

    for entry in manifest {
        if entry.get("file").and_then(|v| v.as_str()) != Some("attn_weights.bin") {
            continue;
        }
        count += 1;
        bytes += entry.get("length").and_then(|v| v.as_u64()).unwrap_or(0);
        if let Some(layer) = entry
            .get("key")
            .and_then(|v| v.as_str())
            .and_then(layer_from_manifest_key)
        {
            layers.insert(layer);
        }
    }

    Ok((count, bytes, layers.into_iter().collect()))
}

fn verify_attention_proof(
    extract: &Path,
    count: usize,
    bytes: u64,
    sha256: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let proof_path = extract.join("attention_proof.json");
    if !proof_path.exists() {
        return Ok(());
    }

    let proof: serde_json::Value = serde_json::from_str(&fs::read_to_string(proof_path)?)?;
    if proof
        .get("source_attention_tensor_count")
        .and_then(|value| value.as_u64())
        != Some(count as u64)
    {
        return Err(invalid_input("attention_proof tensor count mismatch"));
    }
    if proof
        .get("source_attention_bytes_total")
        .and_then(|value| value.as_u64())
        != Some(bytes)
    {
        return Err(invalid_input("attention_proof byte total mismatch"));
    }

    if let Some(raw_extract) = proof.get("raw_extract") {
        if raw_extract
            .get("attention_raw_bytes")
            .and_then(|value| value.as_u64())
            != Some(bytes)
        {
            return Err(invalid_input("attention_proof raw byte mismatch"));
        }
        if raw_extract
            .get("attention_raw_sha256")
            .and_then(|value| value.as_str())
            != Some(sha256)
        {
            return Err(invalid_input("attention_proof raw sha256 mismatch"));
        }
    }

    Ok(())
}

fn layer_from_manifest_key(key: &str) -> Option<usize> {
    let rest = key.strip_prefix("layers.")?;
    let (layer, _) = rest.split_once('.')?;
    layer.parse().ok()
}

fn attention_proof_json(
    gguf: &larql_models::loading::gguf::GgufFile,
    scope: larql_models::loading::gguf::AttentionInventoryScope,
    inventory: &[larql_models::loading::gguf::GgufTensorSummary],
    raw_extract: Option<serde_json::Value>,
    vindex: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut count_by_scope: BTreeMap<String, usize> = BTreeMap::new();
    let mut bytes_by_scope: BTreeMap<String, u64> = BTreeMap::new();
    let mut count_by_component: BTreeMap<String, usize> = BTreeMap::new();
    let mut bytes_by_component: BTreeMap<String, u64> = BTreeMap::new();
    let mut count_by_role: BTreeMap<String, usize> = BTreeMap::new();
    let mut bytes_by_role: BTreeMap<String, u64> = BTreeMap::new();
    let mut source_attention_bytes_total = 0u64;

    for tensor in inventory {
        source_attention_bytes_total += tensor.byte_size;
        *count_by_scope.entry(tensor.scope.clone()).or_default() += 1;
        *bytes_by_scope.entry(tensor.scope.clone()).or_default() += tensor.byte_size;
        *count_by_component
            .entry(tensor.component.clone())
            .or_default() += 1;
        *bytes_by_component
            .entry(tensor.component.clone())
            .or_default() += tensor.byte_size;
        *count_by_role.entry(tensor.role.clone()).or_default() += 1;
        *bytes_by_role.entry(tensor.role.clone()).or_default() += tensor.byte_size;
    }

    serde_json::json!({
        "scope": scope.as_str(),
        "source_path": gguf.path.display().to_string(),
        "alignment": gguf.alignment(),
        "data_offset": gguf.data_offset,
        "source_attention_tensor_count": inventory.len(),
        "source_attention_bytes_total": source_attention_bytes_total,
        "source_attention_mib_total": (source_attention_bytes_total as f64) / (1024.0 * 1024.0),
        "source_attention_count_by_scope": count_by_scope,
        "source_attention_bytes_by_scope": bytes_by_scope,
        "source_attention_count_by_component": count_by_component,
        "source_attention_bytes_by_component": bytes_by_component,
        "source_attention_count_by_role": count_by_role,
        "source_attention_bytes_by_role": bytes_by_role,
        "raw_extract": raw_extract,
        "vindex": vindex,
        "notes": [
            "source_attention_bytes_total counts raw GGUF encoded tensor storage bytes",
            "source_attention.bin preserves raw GGUF tensor payloads in attention_inventory order",
            "vindex attention bytes are Larql runtime serialization bytes when a vindex build is run"
        ]
    })
}

fn invalid_input(message: impl Into<String>) -> Box<dyn std::error::Error> {
    Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        message.into(),
    ))
}

fn json_str<'a>(
    value: &'a serde_json::Value,
    field: &str,
) -> Result<&'a str, Box<dyn std::error::Error>> {
    value
        .get(field)
        .and_then(|value| value.as_str())
        .ok_or_else(|| invalid_input(format!("missing string field {field:?}")))
}

fn json_u64(value: &serde_json::Value, field: &str) -> Result<u64, Box<dyn std::error::Error>> {
    value
        .get(field)
        .and_then(|value| value.as_u64())
        .ok_or_else(|| invalid_input(format!("missing u64 field {field:?}")))
}

fn verify_same_bytes(
    source: &mut BufReader<fs::File>,
    source_offset: u64,
    packed: &mut BufReader<fs::File>,
    packed_offset: u64,
    byte_size: u64,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    const CHUNK: usize = 1024 * 1024;
    let mut left = byte_size;
    let mut source_buf = vec![0u8; CHUNK];
    let mut packed_buf = vec![0u8; CHUNK];

    source.seek(SeekFrom::Start(source_offset))?;
    packed.seek(SeekFrom::Start(packed_offset))?;

    while left > 0 {
        let n = left.min(CHUNK as u64) as usize;
        source.read_exact(&mut source_buf[..n])?;
        packed.read_exact(&mut packed_buf[..n])?;
        if source_buf[..n] != packed_buf[..n] {
            return Err(invalid_input(format!("byte mismatch for tensor {name}")));
        }
        left -= n as u64;
    }

    Ok(())
}

struct SilentCallbacks;
impl larql_vindex::IndexBuildCallbacks for SilentCallbacks {}
