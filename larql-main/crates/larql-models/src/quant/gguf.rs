//! GGUF quantization and file format writer.
//!
//! Supports Q4_K_M, Q4_0, Q8_0 quantization formats for efficient CPU inference.

use std::collections::HashMap;
use std::io::{BufWriter, Seek, Write};
use std::path::Path;

// GGUF constants
const GGUF_MAGIC: u32 = 0x46554747; // "GGUF" little-endian
const GGUF_VERSION: u32 = 3;

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

// GGML tensor type IDs (from ggml module)
const GGML_TYPE_Q4_0: u32 = 2;
const GGML_TYPE_Q8_0: u32 = 8;

/// GGUF quantization format variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GgufQuantFormat {
    /// 4-bit quantization with K-means (recommended for balance)
    Q4Km,
    /// 4-bit quantization (simpler, less accurate)
    Q4_0,
    /// 8-bit quantization (most accurate, larger size)
    Q8_0,
}

impl std::fmt::Display for GgufQuantFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GgufQuantFormat::Q4Km => write!(f, "Q4_K_M"),
            GgufQuantFormat::Q4_0 => write!(f, "Q4_0"),
            GgufQuantFormat::Q8_0 => write!(f, "Q8_0"),
        }
    }
}

impl std::str::FromStr for GgufQuantFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "Q4_K_M" => Ok(GgufQuantFormat::Q4Km),
            "Q4_0" => Ok(GgufQuantFormat::Q4_0),
            "Q8_0" => Ok(GgufQuantFormat::Q8_0),
            _ => Err(format!(
                "Unsupported GGUF quant format '{}'. Expected Q4_K_M, Q4_0, or Q8_0",
                s
            )),
        }
    }
}

/// GGUF quantization configuration.
#[derive(Debug, Clone)]
pub struct GgufQuantConfig {
    pub format: GgufQuantFormat,
    pub block_size: usize,
}

impl Default for GgufQuantConfig {
    fn default() -> Self {
        Self {
            format: GgufQuantFormat::Q4Km,
            block_size: 64,
        }
    }
}

/// Quantize f32 data to int8 (Q8_0 format).
pub fn quantize_to_int8(data: &[f32]) -> (Vec<i8>, f32) {
    let scale = data
        .iter()
        .map(|&x| x.abs())
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(1.0)
        / 127.0;

    let quantized: Vec<i8> = data
        .iter()
        .map(|&x| (x / scale).clamp(-127.0, 127.0) as i8)
        .collect();

    (quantized, scale)
}

/// Dequantize int8 data back to f32.
pub fn dequantize_from_int8(data: &[i8], scale: f32) -> Vec<f32> {
    data.iter().map(|&x| x as f32 * scale).collect()
}

/// Quantize f32 data to int4 (Q4_0 format).
/// Returns packed bytes (2 int4 values per byte) and scale.
pub fn quantize_to_int4(data: &[f32]) -> (Vec<u8>, f32) {
    let scale = data
        .iter()
        .map(|&x| x.abs())
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(1.0)
        / 7.0;

    let mut packed = Vec::with_capacity((data.len() + 1) / 2);

    for chunk in data.chunks(2) {
        // Convert to signed int4 range [-7, 7], then offset to [0, 15] for packing
        let first = ((chunk[0] / scale).clamp(-7.0, 7.0) as i8 + 8) as u8;
        let second = if chunk.len() > 1 {
            ((chunk[1] / scale).clamp(-7.0, 7.0) as i8 + 8) as u8
        } else {
            0
        };
        packed.push(first | (second << 4));
    }

    (packed, scale)
}

/// Dequantize int4 packed data back to f32.
pub fn dequantize_from_int4(packed: &[u8], scale: f32, original_len: usize) -> Vec<f32> {
    let mut result = Vec::with_capacity(original_len);

    for (i, &byte) in packed.iter().enumerate() {
        // Extract packed values [0, 15], convert back to signed [-7, 7]
        let first = ((byte & 0x0F) as i8 - 8) as f32;
        result.push(first * scale);

        if i * 2 + 1 < original_len {
            let second = (((byte >> 4) & 0x0F) as i8 - 8) as f32;
            result.push(second * scale);
        }
    }

    result
}

/// Convert GgufQuantFormat to GGML tensor type ID.
fn format_to_ggml_type(format: GgufQuantFormat) -> u32 {
    match format {
        GgufQuantFormat::Q4Km => GGML_TYPE_Q4_0, // Map to Q4_0 for now (Q4_K requires more complex encoding)
        GgufQuantFormat::Q4_0 => GGML_TYPE_Q4_0,
        GgufQuantFormat::Q8_0 => GGML_TYPE_Q8_0,
    }
}

/// Quantize tensor data based on format.
fn quantize_tensor(data: &[f32], format: GgufQuantFormat) -> Vec<u8> {
    match format {
        GgufQuantFormat::Q4Km => {
            // For now, use Q4_0 as approximation (full Q4_K requires complex super-block encoding)
            crate::quant::ggml::quantize_q4_0(data)
        }
        GgufQuantFormat::Q4_0 => crate::quant::ggml::quantize_q4_0(data),
        GgufQuantFormat::Q8_0 => crate::quant::ggml::quantize_q8_0(data),
    }
}

/// Pad data to 32-byte alignment.
fn pad_to_alignment<W: Write>(
    writer: &mut W,
    current_pos: u64,
    alignment: u64,
) -> std::io::Result<()> {
    let aligned_pos = current_pos.div_ceil(alignment) * alignment;
    if aligned_pos > current_pos {
        let padding = (aligned_pos - current_pos) as usize;
        writer.write_all(&vec![0u8; padding])?;
    }
    Ok(())
}

/// Write a GGUF string (length prefix + bytes).
fn write_string<W: Write>(writer: &mut W, s: &str) -> std::io::Result<()> {
    let bytes = s.as_bytes();
    writer.write_all(&(bytes.len() as u64).to_le_bytes())?;
    writer.write_all(bytes)?;
    Ok(())
}

/// Write a GGUF value.
fn write_value<W: Write>(writer: &mut W, value: &GgufMetadataValue) -> std::io::Result<()> {
    match value {
        GgufMetadataValue::U8(v) => {
            writer.write_all(&GGUF_TYPE_UINT8.to_le_bytes())?;
            writer.write_all(&[*v])?;
        }
        GgufMetadataValue::I8(v) => {
            writer.write_all(&GGUF_TYPE_INT8.to_le_bytes())?;
            writer.write_all(&[*v as u8])?;
        }
        GgufMetadataValue::U16(v) => {
            writer.write_all(&GGUF_TYPE_UINT16.to_le_bytes())?;
            writer.write_all(&v.to_le_bytes())?;
        }
        GgufMetadataValue::I16(v) => {
            writer.write_all(&GGUF_TYPE_INT16.to_le_bytes())?;
            writer.write_all(&v.to_le_bytes())?;
        }
        GgufMetadataValue::U32(v) => {
            writer.write_all(&GGUF_TYPE_UINT32.to_le_bytes())?;
            writer.write_all(&v.to_le_bytes())?;
        }
        GgufMetadataValue::I32(v) => {
            writer.write_all(&GGUF_TYPE_INT32.to_le_bytes())?;
            writer.write_all(&v.to_le_bytes())?;
        }
        GgufMetadataValue::F32(v) => {
            writer.write_all(&GGUF_TYPE_FLOAT32.to_le_bytes())?;
            writer.write_all(&v.to_le_bytes())?;
        }
        GgufMetadataValue::Bool(v) => {
            writer.write_all(&GGUF_TYPE_BOOL.to_le_bytes())?;
            writer.write_all(&[*v as u8])?;
        }
        GgufMetadataValue::String(v) => {
            writer.write_all(&GGUF_TYPE_STRING.to_le_bytes())?;
            write_string(writer, v)?;
        }
        GgufMetadataValue::U64(v) => {
            writer.write_all(&GGUF_TYPE_UINT64.to_le_bytes())?;
            writer.write_all(&v.to_le_bytes())?;
        }
        GgufMetadataValue::I64(v) => {
            writer.write_all(&GGUF_TYPE_INT64.to_le_bytes())?;
            writer.write_all(&v.to_le_bytes())?;
        }
        GgufMetadataValue::F64(v) => {
            writer.write_all(&GGUF_TYPE_FLOAT64.to_le_bytes())?;
            writer.write_all(&v.to_le_bytes())?;
        }
        GgufMetadataValue::Array(arr) => {
            writer.write_all(&GGUF_TYPE_ARRAY.to_le_bytes())?;
            if let Some(first) = arr.first() {
                writer.write_all(&value_type_id(first).to_le_bytes())?;
            } else {
                writer.write_all(&GGUF_TYPE_UINT32.to_le_bytes())?;
            }
            writer.write_all(&(arr.len() as u64).to_le_bytes())?;
            for elem in arr {
                write_value(writer, elem)?;
            }
        }
    }
    Ok(())
}

/// Get GGUF type ID for a value.
fn value_type_id(value: &GgufMetadataValue) -> u32 {
    match value {
        GgufMetadataValue::U8(_) => GGUF_TYPE_UINT8,
        GgufMetadataValue::I8(_) => GGUF_TYPE_INT8,
        GgufMetadataValue::U16(_) => GGUF_TYPE_UINT16,
        GgufMetadataValue::I16(_) => GGUF_TYPE_INT16,
        GgufMetadataValue::U32(_) => GGUF_TYPE_UINT32,
        GgufMetadataValue::I32(_) => GGUF_TYPE_INT32,
        GgufMetadataValue::F32(_) => GGUF_TYPE_FLOAT32,
        GgufMetadataValue::Bool(_) => GGUF_TYPE_BOOL,
        GgufMetadataValue::String(_) => GGUF_TYPE_STRING,
        GgufMetadataValue::U64(_) => GGUF_TYPE_UINT64,
        GgufMetadataValue::I64(_) => GGUF_TYPE_INT64,
        GgufMetadataValue::F64(_) => GGUF_TYPE_FLOAT64,
        GgufMetadataValue::Array(_) => GGUF_TYPE_ARRAY,
    }
}

/// GGUF metadata value.
#[derive(Debug, Clone)]
pub enum GgufMetadataValue {
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
    Array(Vec<GgufMetadataValue>),
}

/// Write quantized tensors to GGUF format.
pub fn write_gguf_file(
    output_path: &Path,
    tensors: &HashMap<String, Vec<f32>>,
    config: &GgufQuantConfig,
) -> Result<(), String> {
    let file = std::fs::File::create(output_path)
        .map_err(|e| format!("Failed to create output file: {}", e))?;
    let mut writer = BufWriter::new(file);

    // Prepare metadata
    let mut metadata = HashMap::new();
    metadata.insert(
        "general.architecture".to_string(),
        GgufMetadataValue::String("llama".to_string()),
    );
    metadata.insert(
        "general.quantization_version".to_string(),
        GgufMetadataValue::U32(2),
    );
    metadata.insert(
        format!("{}.quantization_version", "llama"),
        GgufMetadataValue::U32(2),
    );

    // Write header
    writer
        .write_all(&GGUF_MAGIC.to_le_bytes())
        .map_err(|e| format!("Failed to write magic: {}", e))?;
    writer
        .write_all(&GGUF_VERSION.to_le_bytes())
        .map_err(|e| format!("Failed to write version: {}", e))?;
    writer
        .write_all(&(tensors.len() as u64).to_le_bytes())
        .map_err(|e| format!("Failed to write tensor count: {}", e))?;
    writer
        .write_all(&(metadata.len() as u64).to_le_bytes())
        .map_err(|e| format!("Failed to write metadata count: {}", e))?;

    // Write metadata KV pairs
    for (key, value) in &metadata {
        write_string(&mut writer, key)
            .map_err(|e| format!("Failed to write metadata key '{}': {}", key, e))?;
        write_value(&mut writer, value)
            .map_err(|e| format!("Failed to write metadata value for '{}': {}", key, e))?;
    }

    // Write tensor info (placeholders for offsets, will update later)
    let tensor_type = format_to_ggml_type(config.format);
    let mut tensor_offsets: Vec<(String, u64)> = Vec::new();
    let mut data_offset: u64 = 0;

    for (name, data) in tensors {
        let quantized = quantize_tensor(data, config.format);
        let offset = data_offset;
        tensor_offsets.push((name.clone(), offset));
        data_offset += quantized.len() as u64;

        write_string(&mut writer, name)
            .map_err(|e| format!("Failed to write tensor name '{}': {}", name, e))?;
        writer
            .write_all(&1u32.to_le_bytes()) // n_dims (assume 1D for simplicity)
            .map_err(|e| format!("Failed to write tensor dims for '{}': {}", name, e))?;
        writer
            .write_all(&(data.len() as u64).to_le_bytes()) // dims[0]
            .map_err(|e| format!("Failed to write tensor dim for '{}': {}", name, e))?;
        writer
            .write_all(&tensor_type.to_le_bytes())
            .map_err(|e| format!("Failed to write tensor type for '{}': {}", name, e))?;
        writer
            .write_all(&offset.to_le_bytes())
            .map_err(|e| format!("Failed to write tensor offset for '{}': {}", name, e))?;
    }

    // Pad to 32-byte alignment before data section
    let current_pos = writer
        .stream_position()
        .map_err(|e| format!("Failed to get stream position: {}", e))?;
    pad_to_alignment(&mut writer, current_pos, 32)
        .map_err(|e| format!("Failed to pad to alignment: {}", e))?;

    // Write tensor data
    for (name, data) in tensors {
        let quantized = quantize_tensor(data, config.format);
        writer
            .write_all(&quantized)
            .map_err(|e| format!("Failed to write tensor data for '{}': {}", name, e))?;
    }

    writer
        .flush()
        .map_err(|e| format!("Failed to flush output file: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int8_roundtrip() {
        let original = vec![1.0, -2.5, 3.7, -4.2, 0.0];
        let (quantized, scale) = quantize_to_int8(&original);
        let dequantized = dequantize_from_int8(&quantized, scale);

        // Check that dequantized values are close to original
        for (orig, deq) in original.iter().zip(dequantized.iter()) {
            assert!(
                (orig - deq).abs() < 0.1,
                "Original: {}, Dequantized: {}",
                orig,
                deq
            );
        }
    }

    #[test]
    fn test_int4_roundtrip() {
        let original = vec![1.0, -2.5, 3.7, -4.2, 0.0, 5.1];
        let (packed, scale) = quantize_to_int4(&original);
        let dequantized = dequantize_from_int4(&packed, scale, original.len());

        // Check that dequantized values are close to original (with more tolerance for int4)
        for (orig, deq) in original.iter().zip(dequantized.iter()) {
            assert!(
                (orig - deq).abs() < 0.6,
                "Original: {}, Dequantized: {}",
                orig,
                deq
            );
        }
    }

    #[test]
    fn test_gguf_format_parsing() {
        assert_eq!(
            "Q4_K_M".parse::<GgufQuantFormat>().unwrap(),
            GgufQuantFormat::Q4Km
        );
        assert_eq!(
            "q4_0".parse::<GgufQuantFormat>().unwrap(),
            GgufQuantFormat::Q4_0
        );
        assert_eq!(
            "Q8_0".parse::<GgufQuantFormat>().unwrap(),
            GgufQuantFormat::Q8_0
        );
        assert!("INVALID".parse::<GgufQuantFormat>().is_err());
    }

    #[test]
    fn test_gguf_writer_basic() {
        use std::collections::HashMap;
        let mut tensors = HashMap::new();
        // Create test data with multiple of 32 elements for Q4_0
        let test_data: Vec<f32> = (0..64).map(|i| i as f32).collect();
        tensors.insert("test_tensor".to_string(), test_data);
        let config = GgufQuantConfig::default();
        let result = write_gguf_file(Path::new("/tmp/test.gguf"), &tensors, &config);
        assert!(result.is_ok());

        // Verify file was created
        assert!(Path::new("/tmp/test.gguf").exists());

        // Clean up
        std::fs::remove_file("/tmp/test.gguf").ok();
    }

    #[test]
    fn test_gguf_writer_q8_0() {
        use std::collections::HashMap;
        let mut tensors = HashMap::new();
        let test_data: Vec<f32> = (0..64).map(|i| i as f32).collect();
        tensors.insert("test_tensor".to_string(), test_data);
        let config = GgufQuantConfig {
            format: GgufQuantFormat::Q8_0,
            block_size: 64,
        };
        let result = write_gguf_file(Path::new("/tmp/test_q8.gguf"), &tensors, &config);
        assert!(result.is_ok());
        assert!(Path::new("/tmp/test_q8.gguf").exists());
        std::fs::remove_file("/tmp/test_q8.gguf").ok();
    }
}
