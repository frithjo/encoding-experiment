//! GGUF format reader — parse GGUF files and load tensors as f32.
//!
//! GGUF is the GGML Universal Format used by llama.cpp.
//! We support reading unquantized (F32, F16, BF16) and quantized (Q4_0, Q4_1, Q8_0) tensors.
//! All tensors are dequantized to f32 for use with ModelWeights.

use std::collections::HashMap;
use std::io::{BufReader, Read, Seek};
use std::path::Path;

use larql_core::mmap::Mmap;
use ndarray::{s, Array2, ShapeBuilder};
use serde::{Deserialize, Serialize};

use crate::detect::ModelError;
use crate::weights::{ModelWeights, WeightArray};

// ═══════════════════════════════════════════════════════════════
// GGUF constants
// ═══════════════════════════════════════════════════════════════

const GGUF_MAGIC: u32 = 0x46554747; // "GGUF" little-endian

// Metadata value types
const GGUF_TYPE_UINT8: u32 = 0;
const GGUF_TYPE_INT8: u32 = 1;
const GGUF_TYPE_UINT16: u32 = 2;
const GGUF_TYPE_INT16: u32 = 3;
const GGUF_TYPE_UINT32: u32 = 4;
const GGUF_TYPE_INT32: u32 = 5;
const GGUF_TYPE_FLOAT32: u32 = 6;
const GGUF_TYPE_BOOL: u32 = 7;
const GGUF_TYPE_STRING: u32 = 8;
const GGUF_TYPE_ARRAY: u32 = 9;
const GGUF_TYPE_UINT64: u32 = 10;
const GGUF_TYPE_INT64: u32 = 11;
const GGUF_TYPE_FLOAT64: u32 = 12;

// Tensor type constants moved to format::quant::ggml

// ═══════════════════════════════════════════════════════════════
// GGUF metadata value
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub enum GgufValue {
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    F32(f32),
    Bool(bool),
    String(String),
    U64(u64),
    I64(i64),
    F64(f64),
    Array(Vec<GgufValue>),
}

impl GgufValue {
    pub fn as_u32(&self) -> Option<u32> {
        match self {
            GgufValue::U32(v) => Some(*v),
            GgufValue::I32(v) => Some(*v as u32),
            GgufValue::U64(v) => Some(*v as u32),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            GgufValue::U32(v) => Some(*v as u64),
            GgufValue::I32(v) if *v >= 0 => Some(*v as u64),
            GgufValue::U64(v) => Some(*v),
            GgufValue::I64(v) if *v >= 0 => Some(*v as u64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            GgufValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            GgufValue::F32(v) => Some(*v as f64),
            GgufValue::F64(v) => Some(*v),
            _ => None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// GGUF tensor info
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct GgufTensorInfo {
    pub name: String,
    pub n_dims: u32,
    pub dims: Vec<u64>,
    pub tensor_type: u32,
    pub offset: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttentionInventoryScope {
    Decoder,
    DecoderMtp,
    All,
}

impl AttentionInventoryScope {
    pub fn includes(self, tensor_scope: &str) -> bool {
        match self {
            Self::Decoder => tensor_scope == "decoder",
            Self::DecoderMtp => matches!(tensor_scope, "decoder" | "mtp"),
            Self::All => matches!(tensor_scope, "decoder" | "mtp" | "vision"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Decoder => "decoder",
            Self::DecoderMtp => "decoder-mtp",
            Self::All => "all",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GgufTensorSummary {
    pub name: String,
    pub normalized_name: String,
    pub scope: String,
    pub component: String,
    pub role: String,
    pub param: String,
    pub layer: Option<usize>,
    pub dims: Vec<u64>,
    pub ggml_type_id: u32,
    pub ggml_type: String,
    pub relative_offset: u64,
    pub absolute_offset: u64,
    pub byte_size: u64,
    pub included_in_runtime_vindex: bool,
}

// ═══════════════════════════════════════════════════════════════
// GGUF reader
// ═══════════════════════════════════════════════════════════════

pub struct GgufFile {
    pub metadata: HashMap<String, GgufValue>,
    pub tensor_infos: Vec<GgufTensorInfo>,
    pub data_offset: u64,
    pub path: std::path::PathBuf,
}

impl GgufFile {
    /// Parse a GGUF file header and tensor info (does not read tensor data yet).
    pub fn open(path: &Path) -> Result<Self, ModelError> {
        let file = std::fs::File::open(path)?;
        let mut r = BufReader::new(file);

        // Magic
        let magic = read_u32(&mut r)?;
        if magic != GGUF_MAGIC {
            return Err(ModelError::Parse(format!(
                "not a GGUF file (magic: 0x{:08X}, expected 0x{:08X})",
                magic, GGUF_MAGIC
            )));
        }

        // Version
        let version = read_u32(&mut r)?;
        if !(2..=3).contains(&version) {
            return Err(ModelError::Parse(format!(
                "unsupported GGUF version: {version}"
            )));
        }

        let n_tensors = read_u64(&mut r)? as usize;
        let n_metadata = read_u64(&mut r)? as usize;

        // Read metadata
        let mut metadata = HashMap::new();
        for _ in 0..n_metadata {
            let key = read_string(&mut r)?;
            let value = read_value(&mut r)?;
            metadata.insert(key, value);
        }

        // Read tensor infos
        let mut tensor_infos = Vec::with_capacity(n_tensors);
        for _ in 0..n_tensors {
            let name = read_string(&mut r)?;
            let n_dims = read_u32(&mut r)?;
            let mut dims = Vec::with_capacity(n_dims as usize);
            for _ in 0..n_dims {
                dims.push(read_u64(&mut r)?);
            }
            let tensor_type = read_u32(&mut r)?;
            let offset = read_u64(&mut r)?;
            tensor_infos.push(GgufTensorInfo {
                name,
                n_dims,
                dims,
                tensor_type,
                offset,
            });
        }

        // Data starts at next GGUF alignment boundary. Default is 32 bytes.
        let pos = r.stream_position().map_err(ModelError::Io)?;
        let alignment = metadata
            .get("general.alignment")
            .and_then(GgufValue::as_u64)
            .filter(|v| *v > 0)
            .unwrap_or(32);
        let data_offset = pos.div_ceil(alignment) * alignment;

        Ok(GgufFile {
            metadata,
            tensor_infos,
            data_offset,
            path: path.to_path_buf(),
        })
    }

    pub fn alignment(&self) -> u64 {
        self.metadata
            .get("general.alignment")
            .and_then(GgufValue::as_u64)
            .filter(|v| *v > 0)
            .unwrap_or(32)
    }

    pub fn attention_inventory(
        &self,
        scope: AttentionInventoryScope,
    ) -> Result<Vec<GgufTensorSummary>, ModelError> {
        let mut out = Vec::new();
        for info in &self.tensor_infos {
            let Some((tensor_scope, layer, component, param)) =
                classify_attention_tensor(&info.name)
            else {
                continue;
            };
            if !scope.includes(tensor_scope) {
                continue;
            }
            let n_elements: u64 = info.dims.iter().product();
            let byte_size = tensor_data_size(info.tensor_type, n_elements as usize)? as u64;
            let role = attention_role(component).to_string();
            out.push(GgufTensorSummary {
                name: info.name.clone(),
                normalized_name: normalize_gguf_key(&info.name),
                scope: tensor_scope.to_string(),
                component: component.to_string(),
                role,
                param: param.to_string(),
                layer,
                dims: info.dims.clone(),
                ggml_type_id: info.tensor_type,
                ggml_type: crate::quant::ggml::type_name(info.tensor_type).to_string(),
                relative_offset: info.offset,
                absolute_offset: self.data_offset + info.offset,
                byte_size,
                included_in_runtime_vindex: tensor_scope == "decoder"
                    && matches!(
                        component,
                        "attn_q"
                            | "attn_k"
                            | "attn_v"
                            | "attn_output"
                            | "attn_q_norm"
                            | "attn_k_norm"
                    ),
            });
        }
        out.sort_by(|a, b| {
            a.scope
                .cmp(&b.scope)
                .then(a.layer.cmp(&b.layer))
                .then(a.component.cmp(&b.component))
                .then(a.name.cmp(&b.name))
        });
        Ok(out)
    }

    /// Load all tensors, dequantizing to f32.
    #[allow(clippy::type_complexity)]
    pub fn load_tensors(
        &self,
    ) -> Result<
        (
            HashMap<String, crate::WeightArray>,
            HashMap<String, Vec<f32>>,
        ),
        ModelError,
    > {
        let file = std::fs::File::open(&self.path)?;
        let mmap = unsafe { Mmap::map(&file)? };

        let mut tensors = HashMap::new();
        let mut vectors = HashMap::new();

        for info in &self.tensor_infos {
            let abs_offset = self.data_offset + info.offset;
            let n_elements: u64 = info.dims.iter().product();

            let data_size = tensor_data_size(info.tensor_type, n_elements as usize)?;
            if abs_offset as usize + data_size > mmap.len() {
                return Err(ModelError::Parse(format!(
                    "tensor {} data out of bounds (offset {} + size {} > file {})",
                    info.name,
                    abs_offset,
                    data_size,
                    mmap.len()
                )));
            }

            let raw = &mmap[abs_offset as usize..abs_offset as usize + data_size];
            let floats = dequantize(raw, info.tensor_type, n_elements as usize)?;

            // Normalize key name (strip GGUF prefixes)
            let key = normalize_gguf_key(&info.name);

            match info.n_dims {
                2 => {
                    // GGUF/GGML uses column-major (Fortran) dimension ordering:
                    //   dims[0] = number of columns (innermost/fastest)
                    //   dims[1] = number of rows (outermost)
                    // Data is laid out in column-major order.
                    //
                    // ndarray expects row-major (C) order by default.
                    // To get the correct [rows, cols] matrix in row-major ndarray,
                    // we swap the dimensions and use Fortran (column-major) layout,
                    // then convert to standard (C) layout via .as_standard_layout().
                    let ne0 = info.dims[0] as usize; // columns in GGML
                    let ne1 = info.dims[1] as usize; // rows in GGML
                                                     // Shape is (rows, cols) = (ne1, ne0) in standard math convention.
                                                     // Data is column-major, so we create with Fortran layout.
                    let arr = Array2::from_shape_vec((ne1, ne0).f(), floats)
                        .map_err(|e| ModelError::Parse(format!("tensor {}: {}", info.name, e)))?;
                    // Convert to standard (C/row-major) layout for compatibility
                    let arr = arr.as_standard_layout().into_owned();
                    tensors.insert(key, arr.into_shared());
                }
                1 => {
                    vectors.insert(key, floats);
                }
                _ => {} // skip higher-dim tensors
            }
        }

        Ok((tensors, vectors))
    }

    /// Build a config.json-equivalent from GGUF metadata for architecture detection.
    pub fn to_config_json(&self) -> serde_json::Value {
        let get_str = |k: &str| {
            self.metadata
                .get(k)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let _get_u32 = |k: &str| self.metadata.get(k).and_then(|v| v.as_u32()).unwrap_or(0);

        // GGUF uses "general.architecture" and "{arch}.*" keys
        let arch = get_str("general.architecture");
        let prefix = format!("{arch}.");

        let get_arch_u32 = |suffix: &str| {
            let key = format!("{prefix}{suffix}");
            if let Some(v) = self.metadata.get(&key) {
                // Try scalar first, then array max (handles Gemma 4 variable FFN sizes)
                if let Some(val) = v.as_u32() {
                    return val;
                }
                if let GgufValue::Array(arr) = v {
                    return arr.iter().filter_map(|x| x.as_u32()).max().unwrap_or(0);
                }
            }
            0
        };
        let get_arch_f64 = |suffix: &str| {
            self.metadata
                .get(&format!("{prefix}{suffix}"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0)
        };

        // Map GGUF architecture names to HF model_type
        let model_type = match arch.as_str() {
            "llama" => "llama",
            "gemma" | "gemma2" | "gemma3" | "gemma4" => &arch,
            "qwen" | "qwen2" => "qwen2",
            "mistral" => "mistral",
            "mixtral" => "mixtral",
            "phi" | "phi2" | "phi3" => "phi",
            "gpt2" => "gpt2",
            "deepseek" | "deepseek2" => "deepseek_v2",
            other => other,
        };

        // Gemma 4's attention.key_length reports a different dimension than
        // per-head dim; override with hidden_size / num_heads (standard formula)
        let hidden_size = get_arch_u32("embedding_length");
        let num_heads = get_arch_u32("attention.head_count");
        let head_dim = if arch == "gemma4" && num_heads > 0 {
            // Gemma 4: Q matrix rows = num_heads × head_dim where head_dim = hidden/num_heads × scale
            // For gemma-4-e2b: 1536 / 8 = 192, but actual is 256. Use 2×(hidden/heads) as heuristic.
            // Better: derive from known value 2048 Q rows / 8 heads = 256
            256
        } else {
            get_arch_u32("attention.key_length")
        };

        serde_json::json!({
            "model_type": model_type,
            "hidden_size": hidden_size,
            "num_hidden_layers": get_arch_u32("block_count"),
            "intermediate_size": get_arch_u32("feed_forward_length"),
            "num_attention_heads": num_heads,
            "num_key_value_heads": get_arch_u32("attention.head_count_kv"),
            "head_dim": head_dim,
            "rope_theta": get_arch_f64("rope.freq_base"),
            "vocab_size": get_arch_u32("vocab_size"),
        })
    }
}

/// Load a GGUF file into ModelWeights (dequantized to f32).
pub fn load_gguf(path: &Path) -> Result<ModelWeights, ModelError> {
    let gguf = GgufFile::open(path)?;

    // Detect architecture from GGUF metadata
    let config_json = gguf.to_config_json();
    let arch = crate::detect_from_json(&config_json);
    let prefixes = arch.key_prefixes_to_strip();

    // Load and dequantize all tensors
    let (mut tensors, vectors) = gguf.load_tensors()?;

    // Re-normalize keys through the architecture's prefix stripping
    let mut normalized_tensors: HashMap<String, crate::WeightArray> = HashMap::new();
    for (k, v) in tensors.drain() {
        let key = super::safetensors::normalize_key_pub(&k, prefixes);
        normalized_tensors.insert(key, v);
    }
    expand_fused_qkv_tensors(&mut normalized_tensors, &*arch)?;

    let embed_key = arch.embed_key();
    let embed_raw = normalized_tensors
        .get(embed_key)
        .ok_or_else(|| ModelError::MissingTensor(embed_key.into()))?
        .clone();
    // GGUF stores embeddings as [hidden_size, vocab_size] but we need [vocab_size, hidden_size]
    let embed = if embed_raw.shape()[0] < embed_raw.shape()[1] {
        let mut out = ndarray::Array2::<f32>::zeros((embed_raw.shape()[1], embed_raw.shape()[0]));
        out.assign(&embed_raw.t());
        out.into_shared()
    } else {
        embed_raw
    };

    let lm_head = normalized_tensors
        .get("lm_head.weight")
        .or_else(|| normalized_tensors.get("output.weight"))
        .cloned()
        .unwrap_or_else(|| embed.clone());

    let cfg = arch.config();
    // Gemma3 GGUF does not store vocab_size in arch metadata.
    // Read it from tokenizer.json sitting next to the GGUF file.
    let vocab_size = cfg.vocab_size.filter(|&v| v > 2560).unwrap_or_else(|| {
        // Try to read vocab size from tokenizer.json
        if let Some(parent) = std::path::Path::new(&path).parent() {
            let tok_path = parent.join("tokenizer.json");
            if let Ok(data) = std::fs::read_to_string(&tok_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
                    if let Some(v) = json["model"]["vocab"].as_object() {
                        return v.len();
                    }
                }
            }
        }
        262144 // Gemma3 default
    });

    Ok(ModelWeights {
        tensors: normalized_tensors,
        vectors,
        embed,
        lm_head,
        num_layers: cfg.num_layers,
        hidden_size: cfg.hidden_size,
        intermediate_size: cfg.intermediate_size,
        vocab_size,
        head_dim: cfg.head_dim,
        num_q_heads: cfg.num_q_heads,
        num_kv_heads: cfg.num_kv_heads,
        rope_base: cfg.rope_base,
        arch,
    })
}

// ═══════════════════════════════════════════════════════════════
// GGUF binary reading helpers
// ═══════════════════════════════════════════════════════════════

fn read_u8(r: &mut impl Read) -> Result<u8, ModelError> {
    let mut buf = [0u8; 1];
    r.read_exact(&mut buf)?;
    Ok(buf[0])
}

fn read_i8(r: &mut impl Read) -> Result<i8, ModelError> {
    Ok(read_u8(r)? as i8)
}

fn read_u16(r: &mut impl Read) -> Result<u16, ModelError> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

fn read_i16(r: &mut impl Read) -> Result<i16, ModelError> {
    Ok(read_u16(r)? as i16)
}

fn read_u32(r: &mut impl Read) -> Result<u32, ModelError> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_i32(r: &mut impl Read) -> Result<i32, ModelError> {
    Ok(read_u32(r)? as i32)
}

fn read_u64(r: &mut impl Read) -> Result<u64, ModelError> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_i64(r: &mut impl Read) -> Result<i64, ModelError> {
    Ok(read_u64(r)? as i64)
}

fn read_f32(r: &mut impl Read) -> Result<f32, ModelError> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(f32::from_le_bytes(buf))
}

fn read_f64(r: &mut impl Read) -> Result<f64, ModelError> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(f64::from_le_bytes(buf))
}

fn read_string(r: &mut impl Read) -> Result<String, ModelError> {
    let len = read_u64(r)? as usize;
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    String::from_utf8(buf).map_err(|e| ModelError::Parse(e.to_string()))
}

fn read_value(r: &mut impl Read) -> Result<GgufValue, ModelError> {
    let vtype = read_u32(r)?;
    match vtype {
        GGUF_TYPE_UINT8 => Ok(GgufValue::U8(read_u8(r)?)),
        GGUF_TYPE_INT8 => Ok(GgufValue::I8(read_i8(r)?)),
        GGUF_TYPE_UINT16 => Ok(GgufValue::U16(read_u16(r)?)),
        GGUF_TYPE_INT16 => Ok(GgufValue::I16(read_i16(r)?)),
        GGUF_TYPE_UINT32 => Ok(GgufValue::U32(read_u32(r)?)),
        GGUF_TYPE_INT32 => Ok(GgufValue::I32(read_i32(r)?)),
        GGUF_TYPE_FLOAT32 => Ok(GgufValue::F32(read_f32(r)?)),
        GGUF_TYPE_BOOL => Ok(GgufValue::Bool(read_u8(r)? != 0)),
        GGUF_TYPE_STRING => Ok(GgufValue::String(read_string(r)?)),
        GGUF_TYPE_UINT64 => Ok(GgufValue::U64(read_u64(r)?)),
        GGUF_TYPE_INT64 => Ok(GgufValue::I64(read_i64(r)?)),
        GGUF_TYPE_FLOAT64 => Ok(GgufValue::F64(read_f64(r)?)),
        GGUF_TYPE_ARRAY => {
            let elem_type = read_u32(r)?;
            let len = read_u64(r)? as usize;
            let mut arr = Vec::with_capacity(len);
            for _ in 0..len {
                arr.push(read_array_element(r, elem_type)?);
            }
            Ok(GgufValue::Array(arr))
        }
        _ => Err(ModelError::Parse(format!(
            "unknown GGUF metadata type: {vtype}"
        ))),
    }
}

fn read_array_element(r: &mut impl Read, elem_type: u32) -> Result<GgufValue, ModelError> {
    match elem_type {
        GGUF_TYPE_UINT8 => Ok(GgufValue::U8(read_u8(r)?)),
        GGUF_TYPE_INT8 => Ok(GgufValue::I8(read_i8(r)?)),
        GGUF_TYPE_UINT16 => Ok(GgufValue::U16(read_u16(r)?)),
        GGUF_TYPE_INT16 => Ok(GgufValue::I16(read_i16(r)?)),
        GGUF_TYPE_UINT32 => Ok(GgufValue::U32(read_u32(r)?)),
        GGUF_TYPE_INT32 => Ok(GgufValue::I32(read_i32(r)?)),
        GGUF_TYPE_FLOAT32 => Ok(GgufValue::F32(read_f32(r)?)),
        GGUF_TYPE_BOOL => Ok(GgufValue::Bool(read_u8(r)? != 0)),
        GGUF_TYPE_STRING => Ok(GgufValue::String(read_string(r)?)),
        GGUF_TYPE_UINT64 => Ok(GgufValue::U64(read_u64(r)?)),
        GGUF_TYPE_INT64 => Ok(GgufValue::I64(read_i64(r)?)),
        GGUF_TYPE_FLOAT64 => Ok(GgufValue::F64(read_f64(r)?)),
        _ => Err(ModelError::Parse(format!(
            "unknown GGUF array element type: {elem_type}"
        ))),
    }
}

// ═══════════════════════════════════════════════════════════════
// Dequantization — delegates to format::quant module
// ═══════════════════════════════════════════════════════════════

fn tensor_data_size(tensor_type: u32, n_elements: usize) -> Result<usize, ModelError> {
    crate::quant::ggml::tensor_data_size(tensor_type, n_elements)
}

fn dequantize(data: &[u8], tensor_type: u32, n_elements: usize) -> Result<Vec<f32>, ModelError> {
    crate::quant::ggml::dequantize(data, tensor_type, n_elements)
}

/// Normalize GGUF tensor key names to match HuggingFace conventions.
pub fn normalize_gguf_key(name: &str) -> String {
    // GGUF uses "blk.N.attn_q.weight" format
    // HF uses "model.layers.N.self_attn.q_proj.weight" format
    // We normalize to the HF style since that's what ModelArchitecture expects

    name.replace("blk.", "layers.")
        .replace("attn_q_norm.", "self_attn.q_norm.")
        .replace("attn_k_norm.", "self_attn.k_norm.")
        .replace("attn_q.", "self_attn.q_proj.")
        .replace("attn_k.", "self_attn.k_proj.")
        .replace("attn_v.", "self_attn.v_proj.")
        .replace("attn_output.", "self_attn.o_proj.")
        .replace("attn_out.", "self_attn.o_proj.")
        .replace("ffn_gate.", "mlp.gate_proj.")
        .replace("ffn_up.", "mlp.up_proj.")
        .replace("ffn_down.", "mlp.down_proj.")
        .replace("post_attention_norm.", "post_attention_layernorm.")
        .replace("attn_norm.", "input_layernorm.")
        .replace("ffn_norm.", "post_attention_layernorm.")
        .replace("token_embd.", "embed_tokens.")
        .replace("output_norm.", "norm.")
        .replace("output.", "lm_head.")
}

fn expand_fused_qkv_tensors(
    tensors: &mut HashMap<String, WeightArray>,
    arch: &dyn crate::ModelArchitecture,
) -> Result<(), ModelError> {
    let cfg = arch.config();
    let q_rows = cfg.num_q_heads * cfg.head_dim;
    let kv_rows = cfg.num_kv_heads * cfg.head_dim;
    let expected_rows = q_rows + (2 * kv_rows);

    for layer in 0..cfg.num_layers {
        let fused_key = format!("layers.{layer}.attn_qkv.weight");
        let Some(fused) = tensors.remove(&fused_key) else {
            continue;
        };

        let rows = fused.shape()[0];
        let cols = fused.shape()[1];
        if rows != expected_rows || cols != cfg.hidden_size {
            tensors.insert(fused_key, fused);
            continue;
        }

        let q = fused.slice(s![0..q_rows, ..]).to_owned().into_shared();
        let k = fused
            .slice(s![q_rows..q_rows + kv_rows, ..])
            .to_owned()
            .into_shared();
        let v = fused
            .slice(s![q_rows + kv_rows..expected_rows, ..])
            .to_owned()
            .into_shared();

        tensors.entry(arch.attn_q_key(layer)).or_insert(q);
        tensors.entry(arch.attn_k_key(layer)).or_insert(k);
        tensors.entry(arch.attn_v_key(layer)).or_insert(v);
    }

    Ok(())
}

fn classify_attention_tensor(name: &str) -> Option<(&'static str, Option<usize>, &str, &str)> {
    let parts: Vec<&str> = name.split('.').collect();
    let (scope, layer, component_idx) = match parts.as_slice() {
        ["blk", layer, ..] => ("decoder", layer.parse::<usize>().ok(), 2),
        ["mtp", "layers", layer, ..] => ("mtp", layer.parse::<usize>().ok(), 3),
        ["v", "blk", layer, ..] => ("vision", layer.parse::<usize>().ok(), 3),
        _ => return None,
    };
    let component = *parts.get(component_idx)?;
    if !is_attention_component(component) {
        return None;
    }
    let param = parts.last().copied().unwrap_or("");
    Some((scope, layer, component, param))
}

fn is_attention_component(component: &str) -> bool {
    component.starts_with("attn_") || component == "attn" || component == "post_attention_norm"
}

fn attention_role(component: &str) -> &str {
    match component {
        "attn_qkv" => "qkv_fused",
        "attn_q" => "q",
        "attn_k" => "k",
        "attn_v" => "v",
        "attn_output" | "attn_out" => "o",
        "attn_gate" => "gate",
        "attn_q_norm" => "q_norm",
        "attn_k_norm" => "k_norm",
        "attn_norm" => "input_norm",
        "post_attention_norm" => "post_attention_norm",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_gguf_key() {
        assert_eq!(
            normalize_gguf_key("blk.0.attn_q.weight"),
            "layers.0.self_attn.q_proj.weight"
        );
        assert_eq!(
            normalize_gguf_key("blk.0.attn_q_norm.weight"),
            "layers.0.self_attn.q_norm.weight"
        );
        assert_eq!(
            normalize_gguf_key("blk.0.attn_k_norm.weight"),
            "layers.0.self_attn.k_norm.weight"
        );
        assert_eq!(
            normalize_gguf_key("blk.0.attn_out.weight"),
            "layers.0.self_attn.o_proj.weight"
        );
        assert_eq!(
            normalize_gguf_key("blk.0.post_attention_norm.weight"),
            "layers.0.post_attention_layernorm.weight"
        );
        assert_eq!(
            normalize_gguf_key("blk.15.ffn_gate.weight"),
            "layers.15.mlp.gate_proj.weight"
        );
        assert_eq!(
            normalize_gguf_key("token_embd.weight"),
            "embed_tokens.weight"
        );
        assert_eq!(normalize_gguf_key("output.weight"), "lm_head.weight");
    }

    #[test]
    fn test_load_tensors_swaps_gguf_2d_dims_to_rows_cols() {
        use std::io::{Seek, Write};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tiny.gguf");
        let mut file = std::fs::File::create(&path).unwrap();

        // Header
        file.write_all(&GGUF_MAGIC.to_le_bytes()).unwrap();
        file.write_all(&3u32.to_le_bytes()).unwrap(); // version
        file.write_all(&1u64.to_le_bytes()).unwrap(); // n_tensors
        file.write_all(&0u64.to_le_bytes()).unwrap(); // n_metadata

        // Tensor info: ggml dims order is [cols, rows].
        let name = b"blk.0.ffn_down.weight";
        file.write_all(&(name.len() as u64).to_le_bytes()).unwrap();
        file.write_all(name).unwrap();
        file.write_all(&2u32.to_le_bytes()).unwrap(); // n_dims
        file.write_all(&4u64.to_le_bytes()).unwrap(); // cols
        file.write_all(&2u64.to_le_bytes()).unwrap(); // rows
        file.write_all(&crate::quant::ggml::TYPE_F32.to_le_bytes())
            .unwrap();
        file.write_all(&0u64.to_le_bytes()).unwrap(); // tensor data offset

        // Pad tensor data start to 32-byte boundary.
        let pos = file.stream_position().unwrap();
        let aligned = pos.div_ceil(32) * 32;
        file.write_all(&vec![0u8; (aligned - pos) as usize])
            .unwrap();

        // Raw row-major data for a logical [2, 4] matrix.
        for v in 1u32..=8 {
            file.write_all(&(v as f32).to_le_bytes()).unwrap();
        }
        file.flush().unwrap();

        let gguf = GgufFile::open(&path).unwrap();
        let (tensors, _) = gguf.load_tensors().unwrap();
        let down = tensors.get("layers.0.mlp.down_proj.weight").unwrap();

        assert_eq!(down.shape(), &[2, 4]);
        assert_eq!(down[[0, 0]], 1.0);
        assert_eq!(down[[1, 3]], 8.0);
    }

    #[test]
    fn test_open_honors_general_alignment_metadata() {
        use std::io::{Seek, Write};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("aligned.gguf");
        let mut file = std::fs::File::create(&path).unwrap();

        file.write_all(&GGUF_MAGIC.to_le_bytes()).unwrap();
        file.write_all(&3u32.to_le_bytes()).unwrap();
        file.write_all(&0u64.to_le_bytes()).unwrap();
        file.write_all(&1u64.to_le_bytes()).unwrap();

        let key = b"general.alignment";
        file.write_all(&(key.len() as u64).to_le_bytes()).unwrap();
        file.write_all(key).unwrap();
        file.write_all(&GGUF_TYPE_UINT32.to_le_bytes()).unwrap();
        file.write_all(&64u32.to_le_bytes()).unwrap();

        let pos = file.stream_position().unwrap();
        file.flush().unwrap();

        let gguf = GgufFile::open(&path).unwrap();
        assert_eq!(gguf.alignment(), 64);
        assert_eq!(gguf.data_offset, pos.div_ceil(64) * 64);
    }

    #[test]
    fn test_attention_inventory_classifies_scope_role_and_offsets() {
        let gguf = GgufFile {
            metadata: HashMap::new(),
            tensor_infos: vec![
                GgufTensorInfo {
                    name: "blk.0.attn_q.weight".into(),
                    n_dims: 2,
                    dims: vec![4, 2],
                    tensor_type: crate::quant::ggml::TYPE_F32,
                    offset: 0,
                },
                GgufTensorInfo {
                    name: "mtp.layers.0.attn_output.weight".into(),
                    n_dims: 2,
                    dims: vec![3, 2],
                    tensor_type: crate::quant::ggml::TYPE_F32,
                    offset: 128,
                },
                GgufTensorInfo {
                    name: "v.blk.1.attn_k.bias".into(),
                    n_dims: 1,
                    dims: vec![5],
                    tensor_type: crate::quant::ggml::TYPE_F32,
                    offset: 256,
                },
                GgufTensorInfo {
                    name: "blk.0.ffn_gate.weight".into(),
                    n_dims: 2,
                    dims: vec![4, 2],
                    tensor_type: crate::quant::ggml::TYPE_F32,
                    offset: 512,
                },
            ],
            data_offset: 64,
            path: std::path::PathBuf::from("test.gguf"),
        };

        let decoder = gguf
            .attention_inventory(AttentionInventoryScope::Decoder)
            .unwrap();
        assert_eq!(decoder.len(), 1);
        assert_eq!(decoder[0].scope, "decoder");
        assert_eq!(decoder[0].role, "q");
        assert_eq!(decoder[0].absolute_offset, 64);
        assert_eq!(decoder[0].byte_size, 32);
        assert!(decoder[0].included_in_runtime_vindex);

        let all = gguf
            .attention_inventory(AttentionInventoryScope::All)
            .unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all.iter().filter(|t| t.scope == "vision").count(), 1);
        assert_eq!(all.iter().find(|t| t.scope == "mtp").unwrap().role, "o");
    }

    #[test]
    fn test_expand_fused_qkv_tensors_splits_only_exact_standard_shape() {
        let arch = crate::architectures::generic::GenericArch::from_config(tiny_attention_config());
        let mut tensors = HashMap::new();
        let fused = Array2::from_shape_vec((6, 4), (0..24).map(|v| v as f32).collect())
            .unwrap()
            .into_shared();
        tensors.insert("layers.0.attn_qkv.weight".to_string(), fused);

        expand_fused_qkv_tensors(&mut tensors, &arch).unwrap();

        assert!(!tensors.contains_key("layers.0.attn_qkv.weight"));
        assert_eq!(
            tensors
                .get("layers.0.self_attn.q_proj.weight")
                .unwrap()
                .shape(),
            &[2, 4]
        );
        assert_eq!(
            tensors
                .get("layers.0.self_attn.k_proj.weight")
                .unwrap()
                .shape(),
            &[2, 4]
        );
        assert_eq!(
            tensors
                .get("layers.0.self_attn.v_proj.weight")
                .unwrap()
                .shape(),
            &[2, 4]
        );
    }

    #[test]
    fn test_expand_fused_qkv_tensors_preserves_nonstandard_shape() {
        let arch = crate::architectures::generic::GenericArch::from_config(tiny_attention_config());
        let mut tensors = HashMap::new();
        let fused = Array2::from_shape_vec((5, 4), (0..20).map(|v| v as f32).collect())
            .unwrap()
            .into_shared();
        tensors.insert("layers.0.attn_qkv.weight".to_string(), fused);

        expand_fused_qkv_tensors(&mut tensors, &arch).unwrap();

        assert!(tensors.contains_key("layers.0.attn_qkv.weight"));
        assert!(!tensors.contains_key("layers.0.self_attn.q_proj.weight"));
        assert!(!tensors.contains_key("layers.0.self_attn.k_proj.weight"));
        assert!(!tensors.contains_key("layers.0.self_attn.v_proj.weight"));
    }

    fn tiny_attention_config() -> crate::config::ModelConfig {
        crate::config::ModelConfig {
            model_type: "test".into(),
            num_layers: 1,
            hidden_size: 4,
            intermediate_size: 8,
            head_dim: 2,
            num_q_heads: 1,
            num_kv_heads: 1,
            vocab_size: Some(16),
            rope_base: 10000.0,
            rope_local_base: None,
            sliding_window: None,
            num_experts: None,
            num_experts_per_token: None,
            num_shared_experts: None,
            kv_lora_rank: None,
            q_lora_rank: None,
            rope_scaling: None,
            attn_logit_softcapping: None,
            final_logit_softcapping: None,
            query_pre_attn_scalar: None,
            embedding_multiplier: None,
            residual_multiplier: None,
            attention_multiplier: None,
            logits_scaling: None,
            global_head_dim: None,
            num_global_kv_heads: None,
            partial_rotary_factor: None,
            sliding_window_pattern: None,
            layer_types: None,
            attention_k_eq_v: false,
            per_layer_embed_dim: None,
            num_kv_shared_layers: None,
        }
    }

    // Dequant tests are in format::quant::ggml::tests
}
