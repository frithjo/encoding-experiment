//! In-tree safetensors parser — minimal implementation to replace external dependency.
//!
//! Parses the safetensors format:
//!   - 8 bytes LE: header length
//!   - header_len bytes: JSON header
//!   - tensor data (contiguous)
//!
//! We only consume a subset of the schema: dtype, shape, data_offsets.

use serde::Deserialize;

/// Tensor data type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dtype {
    F32,
    F16,
    BF16,
    U8,
    I32,
    I64,
    F64,
}

impl Dtype {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "F32" => Some(Dtype::F32),
            "F16" => Some(Dtype::F16),
            "BF16" => Some(Dtype::BF16),
            "U8" => Some(Dtype::U8),
            "I32" => Some(Dtype::I32),
            "I64" => Some(Dtype::I64),
            "F64" => Some(Dtype::F64),
            _ => None,
        }
    }

    pub fn size_bytes(&self) -> usize {
        match self {
            Dtype::F32 | Dtype::I32 => 4,
            Dtype::F16 | Dtype::BF16 => 2,
            Dtype::U8 => 1,
            Dtype::I64 | Dtype::F64 => 8,
        }
    }
}

/// Tensor metadata from header.
#[derive(Debug, Clone, Deserialize)]
struct TensorInfo {
    dtype: String,
    shape: Vec<usize>,
    data_offsets: Vec<usize>,
}

/// Header JSON structure.
#[derive(Debug, Deserialize)]
struct Header {
    #[serde(flatten)]
    tensors: std::collections::HashMap<String, TensorInfo>,
}

/// View into a tensor's data.
#[derive(Debug, Clone)]
pub struct TensorView<'a> {
    dtype: Dtype,
    shape: Vec<usize>,
    data: &'a [u8],
}

impl<'a> TensorView<'a> {
    pub fn dtype(&self) -> Dtype {
        self.dtype
    }

    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    pub fn data(&self) -> &[u8] {
        self.data
    }

    pub fn num_elements(&self) -> usize {
        self.shape.iter().product()
    }

    pub fn byte_len(&self) -> usize {
        self.num_elements() * self.dtype.size_bytes()
    }
}

/// Parsed safetensors file.
pub struct SafeTensorsFile<'a> {
    bytes: &'a [u8],
    header: Header,
}

impl<'a> SafeTensorsFile<'a> {
    /// Parse a safetensors file from bytes.
    pub fn deserialize(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.len() < 8 {
            return Err("File too small for header length".into());
        }

        let header_len = u64::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;

        if bytes.len() < 8 + header_len {
            return Err("File truncated: header extends beyond file".into());
        }

        let header_bytes = &bytes[8..8 + header_len];
        let header: Header = serde_json::from_slice(header_bytes)
            .map_err(|e| format!("Failed to parse header JSON: {}", e))?;

        Ok(Self { bytes, header })
    }

    /// Get all tensor names.
    pub fn names(&self) -> Vec<&str> {
        self.header.tensors.keys().map(|s| s.as_str()).collect()
    }

    /// Get a tensor view by name.
    pub fn tensor(&self, name: &str) -> Option<TensorView<'a>> {
        let info = self.header.tensors.get(name)?;
        let dtype = Dtype::from_str(&info.dtype)?;

        let (start, end) = if info.data_offsets.len() >= 2 {
            (info.data_offsets[0], info.data_offsets[1])
        } else {
            return None;
        };

        let data_start = 8 + header_len(self.bytes);
        let abs_start = data_start + start;
        let abs_end = data_start + end;

        if abs_start >= self.bytes.len() || abs_end > self.bytes.len() {
            return None;
        }

        let data = &self.bytes[abs_start..abs_end];

        Some(TensorView {
            dtype,
            shape: info.shape.clone(),
            data,
        })
    }

    /// Iterator over all tensors.
    pub fn tensors(&self) -> impl Iterator<Item = (&str, TensorView<'a>)> {
        self.names().into_iter().filter_map(move |name| {
            self.tensor(name).map(|view| (name, view))
        })
    }
}

fn header_len(bytes: &[u8]) -> usize {
    u64::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]]) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dtype_from_str() {
        assert_eq!(Dtype::from_str("F32"), Some(Dtype::F32));
        assert_eq!(Dtype::from_str("F16"), Some(Dtype::F16));
        assert_eq!(Dtype::from_str("BF16"), Some(Dtype::BF16));
        assert_eq!(Dtype::from_str("U8"), Some(Dtype::U8));
        assert_eq!(Dtype::from_str("I32"), Some(Dtype::I32));
        assert_eq!(Dtype::from_str("I64"), Some(Dtype::I64));
        assert_eq!(Dtype::from_str("F64"), Some(Dtype::F64));
        assert_eq!(Dtype::from_str("UNKNOWN"), None);
    }

    #[test]
    fn test_dtype_size_bytes() {
        assert_eq!(Dtype::F32.size_bytes(), 4);
        assert_eq!(Dtype::F16.size_bytes(), 2);
        assert_eq!(Dtype::BF16.size_bytes(), 2);
        assert_eq!(Dtype::U8.size_bytes(), 1);
        assert_eq!(Dtype::I32.size_bytes(), 4);
        assert_eq!(Dtype::I64.size_bytes(), 8);
        assert_eq!(Dtype::F64.size_bytes(), 8);
    }

    #[test]
    fn test_deserialize_minimal() {
        let header_json = r#"{"tensor1": {"dtype": "F32", "shape": [2, 3], "data_offsets": [0, 24]}}"#;
        let header_len = header_json.len();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(header_len as u64).to_le_bytes());
        bytes.extend_from_slice(header_json.as_bytes());
        // 24 bytes of dummy data
        bytes.extend_from_slice(&[0u8; 24]);

        let st = SafeTensorsFile::deserialize(&bytes).unwrap();
        assert_eq!(st.names(), vec!["tensor1"]);

        let tensor = st.tensor("tensor1").unwrap();
        assert_eq!(tensor.dtype(), Dtype::F32);
        assert_eq!(tensor.shape(), &[2, 3]);
        assert_eq!(tensor.data().len(), 24);
    }

    #[test]
    fn test_deserialize_too_small() {
        let bytes = [0u8; 4];
        assert!(SafeTensorsFile::deserialize(&bytes).is_err());
    }
}
