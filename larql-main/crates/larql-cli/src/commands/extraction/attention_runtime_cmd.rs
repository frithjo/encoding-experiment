use clap::{Args, Subcommand, ValueEnum};
use larql_core::capsule::{
    canonical_json_bytes, make_capsule, make_capture, read_capsule_json, sha256_hex,
    validate_capsule, write_capsule_json, Capsule,
};
use larql_inference::attention::{
    apply_rope_partial, bind_separated_attention_listener, gqa_attention_with_weights,
    request_separated_attention, separated_attention_request_wire_bytes, serve_separated_attention,
    AttentionTensor2, AttentionWeights, SeparatedAttentionReady, SeparatedAttentionRequest,
    SEPARATED_ATTENTION_READY_CAPSULE_KIND, SEPARATED_ATTENTION_READY_CAPTURE_KIND,
};
use larql_inference::forward::{
    add_bias, apply_norm, dot_proj, embed_tokens_pub, forward_to_layer,
};
use larql_inference::larql_tokenizer::{self, Tokenizer};
use larql_inference::ndarray::{s, Array2};
use larql_inference::residual::{rms_norm_heads, rms_norm_heads_no_weight};
use larql_inference::{load_model_dir, resolve_model_path};
use larql_models::loading::gguf::{AttentionInventoryScope, GgufFile, GgufTensorSummary};
use larql_models::{ModelArchitecture, ModelWeights, WeightArray};
use serde::Serialize;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

mod extract;
mod graph;
mod model_proof;
mod prompt_proof;

use extract::extract_gguf_attention_tensors;
use model_proof::run_model_attention_runtime_proof;
use prompt_proof::run_prompt_attention_runtime_proof;

const PRODUCER: &str = "larql.attention_runtime.rust";
const PROOF_CAPTURE_KIND: &str = "gguf_separated_attention_runtime_proof";
const PROOF_CAPSULE_KIND: &str = "gguf_separated_attention_runtime_proof_capsule";
const PROMPT_PROOF_CAPTURE_KIND: &str = "prompt_separated_attention_runtime_proof";
const PROMPT_PROOF_CAPSULE_KIND: &str = "prompt_separated_attention_runtime_proof_capsule";
const EXTRACTION_REPORT_CAPTURE_KIND: &str = "gguf_attention_extraction_report";
const EXTRACTION_REPORT_CAPSULE_KIND: &str = "gguf_attention_extraction_report_capsule";
const TENSOR_FILE_CAPTURE_KIND: &str = "gguf_attention_tensor_file";
const TENSOR_FILE_CAPSULE_KIND: &str = "gguf_attention_tensor_file_capsule";

#[derive(Args, Debug)]
pub struct AttentionRuntimeArgs {
    #[command(subcommand)]
    command: AttentionRuntimeCommand,
}

#[derive(Subcommand, Debug)]
enum AttentionRuntimeCommand {
    /// Serve separated attention over TCP. Payload contains Q/K/V activations, not weights.
    Serve {
        /// Bind host.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Bind port. Use 0 to let the OS choose a free port.
        #[arg(long, default_value_t = 0)]
        port: u16,

        /// Exit after one request.
        #[arg(long)]
        oneshot: bool,

        /// Write bound address after listener starts.
        #[arg(long)]
        ready_file: Option<PathBuf>,
    },

    /// Spawn a separate attention process and prove a real GGUF layer split.
    Proof {
        /// Output JSON proof file. Defaults to stdout only.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Max accepted absolute output difference.
        #[arg(long, default_value_t = 1.0e-6)]
        tolerance: f32,

        /// GGUF model for real-weight layer proof.
        #[arg(long, value_name = "GGUF", required = true)]
        gguf: PathBuf,

        /// Attention layer to prove. Omit to select the first runnable Q/K/V/O layer from GGUF.
        #[arg(long)]
        layer: Option<usize>,

        /// Sequence length for real-weight layer proof.
        #[arg(long, default_value_t = 2)]
        seq_len: usize,
    },

    /// Prove separated attention on prompt-derived residual states from a real model.
    PromptProof {
        /// Model directory, GGUF-adjacent tokenizer directory, or HuggingFace model ID.
        #[arg(long, value_name = "MODEL", required = true)]
        model: String,

        /// Explicit tokenizer.json for GGUF or externally mirrored model/tokenizer pairs.
        #[arg(long, value_name = "TOKENIZER_JSON")]
        tokenizer: Option<PathBuf>,

        /// Human-authored prompt file. Accepts JSONL with text/prompt fields or plain text lines.
        #[arg(long, value_name = "JSONL_OR_TEXT", required = true)]
        prompts_file: PathBuf,

        /// Attention layers to test.
        #[arg(long, value_delimiter = ',', required = true)]
        layers: Vec<usize>,

        /// Prefix token lengths to test.
        #[arg(long, value_delimiter = ',', default_value = "32,128")]
        seq_lens: Vec<usize>,

        /// Max accepted absolute output difference for a cell to pass.
        #[arg(long, default_value_t = 1.0e-6)]
        tolerance: f32,

        /// Output directory for proof capsule and cell JSONL.
        #[arg(long, value_name = "DIR", required = true)]
        output_dir: PathBuf,

        /// Limit prompts after reading. Zero means all prompts.
        #[arg(long, default_value_t = 0)]
        max_prompts: usize,
    },

    /// Extract raw GGUF attention tensors into capsule-backed files.
    ExtractGguf {
        /// GGUF model to inspect.
        #[arg(long, value_name = "GGUF", required = true)]
        gguf: PathBuf,

        /// Output directory for tensor bytes and capsule sidecars.
        #[arg(long, value_name = "DIR", required = true)]
        out: PathBuf,

        /// Attention tensor scope.
        #[arg(long, value_enum, default_value_t = AttentionRuntimeScopeArg::Decoder)]
        scope: AttentionRuntimeScopeArg,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AttentionRuntimeScopeArg {
    Decoder,
    DecoderMtp,
    All,
}

impl From<AttentionRuntimeScopeArg> for AttentionInventoryScope {
    fn from(value: AttentionRuntimeScopeArg) -> Self {
        match value {
            AttentionRuntimeScopeArg::Decoder => Self::Decoder,
            AttentionRuntimeScopeArg::DecoderMtp => Self::DecoderMtp,
            AttentionRuntimeScopeArg::All => Self::All,
        }
    }
}

pub fn run(args: AttentionRuntimeArgs) -> Result<(), Box<dyn Error>> {
    match args.command {
        AttentionRuntimeCommand::Serve {
            host,
            port,
            oneshot,
            ready_file,
        } => {
            let listener = bind_separated_attention_listener(&host, port, ready_file.as_deref())?;
            let _handled = serve_separated_attention(listener, oneshot)?;
            Ok(())
        }
        AttentionRuntimeCommand::Proof {
            output,
            tolerance,
            gguf,
            layer,
            seq_len,
        } => {
            let proof = run_model_attention_runtime_proof(&gguf, layer, seq_len, tolerance)?;
            let capsule = make_runtime_capsule(PROOF_CAPTURE_KIND, PROOF_CAPSULE_KIND, proof)?;
            if let Some(path) = output {
                write_capsule_json(&path, &capsule)?;
            } else {
                write_capsule_stdout(&capsule)?;
            }
            Ok(())
        }
        AttentionRuntimeCommand::PromptProof {
            model,
            tokenizer,
            prompts_file,
            layers,
            seq_lens,
            tolerance,
            output_dir,
            max_prompts,
        } => {
            run_prompt_attention_runtime_proof(
                &model,
                tokenizer.as_deref(),
                &prompts_file,
                &layers,
                &seq_lens,
                tolerance,
                max_prompts,
                &output_dir,
            )?;
            Ok(())
        }
        AttentionRuntimeCommand::ExtractGguf { gguf, out, scope } => {
            extract_gguf_attention_tensors(&gguf, &out, scope.into())?;
            Ok(())
        }
    }
}

fn make_runtime_capsule<T: Serialize>(
    capture_kind: &str,
    capsule_kind: &str,
    payload: T,
) -> Result<Capsule<T>, Box<dyn Error>> {
    let capture = make_capture(capture_kind, payload)?;
    Ok(make_capsule(capsule_kind, capture, PRODUCER)?)
}

fn write_capsule_stdout<T: Serialize>(capsule: &Capsule<T>) -> Result<(), Box<dyn Error>> {
    validate_capsule(capsule, None, None)?;
    let mut bytes = canonical_json_bytes(capsule)?;
    bytes.push(b'\n');
    std::io::stdout().lock().write_all(&bytes)?;
    Ok(())
}

#[derive(Debug, Serialize)]
struct AttentionRuntimeProof {
    proof_kind: String,
    status: String,
    transport: String,
    server_addr: String,
    server_pid: u32,
    client_pid: u32,
    reference: String,
    server_exit_status: String,
    model: AttentionRuntimeProofModel,
    request: AttentionRuntimeProofRequest,
    separation: AttentionRuntimeProofSeparation,
    wire: AttentionRuntimeWireProof,
    max_abs_diff: f32,
    h_post_max_abs_diff: f32,
    attention_output_max_abs_diff: f32,
    attention_projected_max_abs_diff: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    attention_weight_max_abs_diff: Option<f32>,
    tolerance: f32,
    output_rows: usize,
    output_cols: usize,
    attention_weight_heads: usize,
}

#[derive(Debug, Serialize)]
struct AttentionRuntimeProofModel {
    gguf_path: String,
    requested_layer: Option<usize>,
    layer: usize,
    layer_selection: String,
    available_attention_layers: Vec<usize>,
    seq_len: usize,
    hidden_source: String,
    hidden_row_indices: Vec<usize>,
    num_layers: usize,
    hidden_size: usize,
    num_q_heads: usize,
    num_kv_heads: usize,
    head_dim: usize,
    q_activation_cols: usize,
    kv_activation_cols: usize,
    q_projection_cols: usize,
    q_projection_tail_cols_excluded: usize,
    loaded_model_weights_on_parent: bool,
    attention_runtime_loaded_model_weights: bool,
    tensor_graph: AttentionRuntimeTensorGraph,
    compared: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
struct AttentionRuntimeProofRequest {
    protocol_version: u32,
    seq_len: usize,
    q_rows: usize,
    q_cols: usize,
    k_rows: usize,
    k_cols: usize,
    v_rows: usize,
    v_cols: usize,
    num_q_heads: usize,
    num_kv_heads: usize,
    head_dim: usize,
    activation_float_count: usize,
    activation_f32_bytes: usize,
    scale: f64,
    capture_attention: bool,
    softcap: Option<f32>,
}

#[derive(Debug, Serialize)]
struct AttentionRuntimeProofSeparation {
    attention_runtime_process: String,
    weights_side_process: String,
    model_weights_sent: bool,
    payload_fields: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
struct AttentionRuntimeWireProof {
    request_bytes: usize,
    request_sha256: String,
    request_capture_kind: String,
    request_top_level_fields: Vec<String>,
    request_top_level_fields_match_contract: bool,
    request_payload_fields: Vec<String>,
    request_payload_fields_match_contract: bool,
    activation_float_count: usize,
    activation_f32_bytes: usize,
    q_shape: [usize; 2],
    k_shape: [usize; 2],
    v_shape: [usize; 2],
    model_tensor_names_checked: usize,
    model_vector_names_checked: usize,
    model_names_found_in_payload: Vec<String>,
    gguf_path_found_in_payload: bool,
    learned_weight_identifiers_sent: bool,
}

#[derive(Debug, Serialize)]
struct PromptAttentionRuntimeProofReport {
    proof_kind: String,
    status: String,
    model: String,
    prompts_file: String,
    prompt_count_read: usize,
    prompt_count_used: usize,
    layers: Vec<usize>,
    seq_lens: Vec<usize>,
    tolerance: f32,
    support_conditions: Vec<String>,
    aggregate: PromptAttentionRuntimeProofAggregate,
    cells: Vec<PromptAttentionRuntimeProofCell>,
}

#[derive(Debug, Default, Serialize)]
struct PromptAttentionRuntimeProofAggregate {
    total_cells: usize,
    ok_cells: usize,
    skipped_cells: usize,
    failed_cells: usize,
    max_abs_diff: Option<f32>,
    h_post_max_abs_diff: Option<f32>,
    attention_output_max_abs_diff: Option<f32>,
    attention_projected_max_abs_diff: Option<f32>,
    attention_weight_max_abs_diff: Option<f32>,
}

#[derive(Debug, Serialize)]
struct PromptAttentionRuntimeProofCell {
    prompt_id: String,
    prompt_sha256: String,
    prompt_chars: usize,
    token_count_original: usize,
    seq_len: usize,
    layer: usize,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    hidden_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    server_addr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    server_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    server_exit_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request: Option<AttentionRuntimeProofRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    separation: Option<AttentionRuntimeProofSeparation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    wire: Option<PromptAttentionRuntimeWireProof>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_abs_diff: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    h_post_max_abs_diff: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attention_output_max_abs_diff: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attention_projected_max_abs_diff: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attention_weight_max_abs_diff: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_rows: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_cols: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attention_weight_heads: Option<usize>,
}

#[derive(Debug, Serialize)]
struct PromptAttentionRuntimeWireProof {
    request_bytes: usize,
    request_sha256: String,
    request_capture_kind: String,
    request_top_level_fields: Vec<String>,
    request_top_level_fields_match_contract: bool,
    request_payload_fields: Vec<String>,
    request_payload_fields_match_contract: bool,
    activation_float_count: usize,
    activation_f32_bytes: usize,
    q_shape: [usize; 2],
    k_shape: [usize; 2],
    v_shape: [usize; 2],
    model_tensor_names_checked: usize,
    model_vector_names_checked: usize,
    model_names_found_in_payload: Vec<String>,
    model_identifiers_found_in_payload: Vec<String>,
    learned_weight_identifiers_sent: bool,
}

struct PromptRecord {
    id: String,
    text: String,
}

#[derive(Debug, Serialize)]
struct GgufAttentionExtractionReport {
    gguf_path: String,
    scope: String,
    data_offset: u64,
    total_gguf_tensors: usize,
    selected_tensor_count: usize,
    total_attention_tensor_bytes: u64,
    tensors: Vec<GgufAttentionTensorRecord>,
}

#[derive(Debug, Clone, Serialize)]
struct GgufAttentionTensorRecord {
    name: String,
    normalized_name: String,
    scope: String,
    component: String,
    role: String,
    param: String,
    layer: Option<usize>,
    dims: Vec<u64>,
    ggml_type_id: u32,
    ggml_type: String,
    relative_offset: u64,
    absolute_offset: u64,
    byte_size: u64,
    included_in_runtime_vindex: bool,
    file: String,
    capsule: String,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct AttentionRuntimeTensorGraph {
    input_norm: String,
    q: AttentionRuntimeTensorNode,
    k: AttentionRuntimeTensorNode,
    v: AttentionRuntimeTensorNode,
    o: AttentionRuntimeTensorNode,
    q_norm: Option<AttentionRuntimeVectorNode>,
    k_norm: Option<AttentionRuntimeVectorNode>,
}

#[derive(Debug, Serialize)]
struct AttentionRuntimeTensorNode {
    name: String,
    rows: usize,
    cols: usize,
}

#[derive(Debug, Serialize)]
struct AttentionRuntimeVectorNode {
    name: String,
    len: usize,
}

struct AttentionGraphLayer {
    layer: usize,
    input_norm_key: String,
    q_key: String,
    k_key: String,
    v_key: String,
    o_key: String,
    q_norm_key: Option<String>,
    k_norm_key: Option<String>,
    q_rows: usize,
    q_cols: usize,
    k_rows: usize,
    k_cols: usize,
    v_rows: usize,
    v_cols: usize,
    o_rows: usize,
    o_cols: usize,
    q_norm_len: Option<usize>,
    k_norm_len: Option<usize>,
    num_q_heads: usize,
    num_kv_heads: usize,
    head_dim: usize,
    q_activation_cols: usize,
    kv_activation_cols: usize,
    q_projection_cols: usize,
    q_projection_tail_cols_excluded: usize,
}

struct GraphAttentionProofOutput {
    h_post_attn: Array2<f32>,
    attn_projected: Array2<f32>,
    attention_output: Array2<f32>,
    attention_weights: Option<AttentionWeights>,
}

impl AttentionGraphLayer {
    fn to_proof_tensor_graph(&self) -> AttentionRuntimeTensorGraph {
        AttentionRuntimeTensorGraph {
            input_norm: self.input_norm_key.clone(),
            q: AttentionRuntimeTensorNode {
                name: self.q_key.clone(),
                rows: self.q_rows,
                cols: self.q_cols,
            },
            k: AttentionRuntimeTensorNode {
                name: self.k_key.clone(),
                rows: self.k_rows,
                cols: self.k_cols,
            },
            v: AttentionRuntimeTensorNode {
                name: self.v_key.clone(),
                rows: self.v_rows,
                cols: self.v_cols,
            },
            o: AttentionRuntimeTensorNode {
                name: self.o_key.clone(),
                rows: self.o_rows,
                cols: self.o_cols,
            },
            q_norm: self
                .q_norm_key
                .as_ref()
                .zip(self.q_norm_len)
                .map(|(name, len)| AttentionRuntimeVectorNode {
                    name: name.clone(),
                    len,
                }),
            k_norm: self
                .k_norm_key
                .as_ref()
                .zip(self.k_norm_len)
                .map(|(name, len)| AttentionRuntimeVectorNode {
                    name: name.clone(),
                    len,
                }),
        }
    }
}

struct AttentionRuntimeChild {
    child: Child,
    ready_file: PathBuf,
    addr: String,
    pid: u32,
}

fn spawn_attention_runtime_child() -> Result<AttentionRuntimeChild, Box<dyn Error>> {
    let exe = std::env::current_exe()?;
    let ready_file = proof_ready_file();
    let mut child = Command::new(&exe)
        .args([
            "attention-runtime",
            "serve",
            "--host",
            "127.0.0.1",
            "--port",
            "0",
            "--oneshot",
            "--ready-file",
        ])
        .arg(&ready_file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    let addr = match wait_for_ready_addr(&ready_file, &mut child, Duration::from_secs(5)) {
        Ok(addr) => addr,
        Err(err) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&ready_file);
            return Err(err);
        }
    };

    let pid = child.id();
    Ok(AttentionRuntimeChild {
        child,
        ready_file,
        addr,
        pid,
    })
}

fn finish_attention_runtime_child(
    mut runtime: AttentionRuntimeChild,
) -> Result<String, Box<dyn Error>> {
    let wait_start = Instant::now();
    let child_status = loop {
        if let Some(status) = runtime.child.try_wait()? {
            break status;
        }
        if wait_start.elapsed() > Duration::from_secs(5) {
            let _ = runtime.child.kill();
            let _ = runtime.child.wait();
            let _ = std::fs::remove_file(&runtime.ready_file);
            return Err("attention runtime child did not exit after oneshot request".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let _ = std::fs::remove_file(&runtime.ready_file);
    if !child_status.success() {
        return Err(
            format!("attention runtime child exited unsuccessfully: {child_status}").into(),
        );
    }
    Ok(child_status.to_string())
}

fn kill_attention_runtime_child(runtime: &mut AttentionRuntimeChild) {
    let _ = runtime.child.kill();
    let _ = runtime.child.wait();
    let _ = std::fs::remove_file(&runtime.ready_file);
}

fn array_data<'a>(array: &'a Array2<f32>, field: &str) -> Result<&'a [f32], Box<dyn Error>> {
    array
        .as_slice()
        .ok_or_else(|| format!("{field} array is not contiguous").into())
}

fn ensure_finite_array(array: &Array2<f32>, field: &str) -> Result<(), Box<dyn Error>> {
    for (index, value) in array.iter().enumerate() {
        if !value.is_finite() {
            return Err(format!(
                "{field} contains non-finite value at flat index {index}: {value}"
            )
            .into());
        }
    }
    Ok(())
}

fn attention_weights_max_abs_diff(
    left: &Option<AttentionWeights>,
    right: &Option<AttentionWeights>,
) -> Result<Option<f32>, Box<dyn Error>> {
    match (left, right) {
        (None, None) => Ok(None),
        (Some(left), Some(right)) => {
            if left.heads.len() != right.heads.len() {
                return Err(format!(
                    "attention weight head count mismatch: local={} remote={}",
                    left.heads.len(),
                    right.heads.len()
                )
                .into());
            }
            let mut max_diff = 0.0f32;
            for (index, (left_head, right_head)) in
                left.heads.iter().zip(right.heads.iter()).enumerate()
            {
                let diff = max_abs_diff(left_head, right_head)
                    .map_err(|err| format!("attention weight head {index} mismatch: {err}"))?;
                max_diff = max_diff.max(diff);
            }
            Ok(Some(max_diff))
        }
        _ => Err("attention weight capture mismatch".into()),
    }
}

fn proof_ready_file() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "larql-attention-runtime-{}-{nanos}.addr",
        std::process::id()
    ))
}

fn wait_for_ready_addr(
    ready_file: &PathBuf,
    child: &mut std::process::Child,
    timeout: Duration,
) -> Result<String, Box<dyn Error>> {
    let start = Instant::now();
    loop {
        if ready_file.exists() {
            if let Ok(capsule) = read_capsule_json::<SeparatedAttentionReady>(ready_file) {
                validate_capsule(
                    &capsule,
                    Some(SEPARATED_ATTENTION_READY_CAPSULE_KIND),
                    Some(SEPARATED_ATTENTION_READY_CAPTURE_KIND),
                )?;
                if capsule.capture.payload.protocol_version != 1 {
                    return Err(format!(
                        "unsupported attention ready protocol version {}",
                        capsule.capture.payload.protocol_version
                    )
                    .into());
                }
                if !capsule.capture.payload.addr.is_empty() {
                    return Ok(capsule.capture.payload.addr);
                }
            }
        }
        if let Some(status) = child.try_wait()? {
            return Err(format!("attention runtime child exited before ready: {status}").into());
        }
        if start.elapsed() > timeout {
            return Err("timed out waiting for attention runtime ready file".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn max_abs_diff(left: &[f32], right: &[f32]) -> Result<f32, Box<dyn Error>> {
    if left.len() != right.len() {
        return Err(format!(
            "output length mismatch: local={} remote={}",
            left.len(),
            right.len()
        )
        .into());
    }
    Ok(left
        .iter()
        .zip(right.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max))
}
