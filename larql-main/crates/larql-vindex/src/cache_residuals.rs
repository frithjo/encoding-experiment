//! Cached residuals storage for template-based fast inference.
//!
//! Stores pre-computed layer residuals for common templates, enabling
//! the inference engine to skip computation for template-fixed layers (L0-12).
//! This enables 155+ tok/s inference by avoiding redundant computation.
//!
//! Format follows the context.rs pattern with header + index + data layout.

use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::Path;

use larql_core::mmap::Mmap;

const MAGIC: [u8; 4] = *b"CRES";
const VERSION: u32 = 1;
const HEADER_SIZE: usize = 128;
const ENTRY_SIZE: usize = 24;

/// Storage dtype for residuals.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum CacheDtype {
    F16 = 2,
    F32 = 4,
}

impl CacheDtype {
    fn from_u8(v: u8) -> Self {
        match v {
            2 => Self::F16,
            4 => Self::F32,
            _ => Self::F16, // Default to f16
        }
    }

    fn size_of(&self) -> usize {
        match self {
            Self::F16 => 2,
            Self::F32 => 4,
        }
    }
}

/// File header for cached residuals.
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct CacheHeader {
    magic: [u8; 4],
    version: u32,
    hidden_size: u32,
    n_layers: u32,
    n_templates: u32,
    dtype: u8,
    _pad: [u8; 107],
}

impl CacheHeader {
    fn bytes_per_residual(&self) -> usize {
        let dtype = CacheDtype::from_u8(self.dtype);
        self.hidden_size as usize * dtype.size_of()
    }

    fn to_bytes(self) -> [u8; HEADER_SIZE] {
        let mut bytes = [0u8; HEADER_SIZE];
        bytes[0..4].copy_from_slice(&self.magic);
        bytes[4..8].copy_from_slice(&self.version.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.hidden_size.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.n_layers.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.n_templates.to_le_bytes());
        bytes[20] = self.dtype;
        // _pad is already zeros
        bytes
    }

    fn from_bytes(bytes: &[u8; HEADER_SIZE]) -> Self {
        Self {
            magic: bytes[0..4].try_into().unwrap(),
            version: u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            hidden_size: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            n_layers: u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            n_templates: u32::from_le_bytes(bytes[16..20].try_into().unwrap()),
            dtype: bytes[20],
            _pad: [0u8; 107],
        }
    }
}

/// Per-template entry in the index.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TemplateEntry {
    template_id: u32,
    token_count: u32,
    layer_start: u32,
    layer_end: u32,
    data_offset: u64,
}

impl TemplateEntry {
    fn to_bytes(self) -> [u8; ENTRY_SIZE] {
        let mut bytes = [0u8; ENTRY_SIZE];
        bytes[0..4].copy_from_slice(&self.template_id.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.token_count.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.layer_start.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.layer_end.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.data_offset.to_le_bytes());
        bytes
    }

    fn from_bytes(bytes: &[u8; ENTRY_SIZE]) -> Self {
        Self {
            template_id: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            token_count: u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            layer_start: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            layer_end: u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            data_offset: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
        }
    }

    fn layer_range(&self) -> std::ops::RangeInclusive<usize> {
        self.layer_start as usize..=self.layer_end as usize
    }
}

/// Read-only mmap'd cache store.
pub struct CacheStore {
    mmap: Mmap,
    header: CacheHeader,
}

impl CacheStore {
    /// Open a cached residuals file.
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };

        if mmap.len() < HEADER_SIZE {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "file too small"));
        }

        let mut header_bytes = [0u8; HEADER_SIZE];
        header_bytes.copy_from_slice(&mmap[..HEADER_SIZE]);
        let header = CacheHeader::from_bytes(&header_bytes);

        if header.magic != MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad magic"));
        }

        #[cfg(unix)]
        unsafe {
            libc::madvise(
                mmap.as_ptr() as *mut libc::c_void,
                mmap.len(),
                libc::MADV_RANDOM,
            );
        }

        Ok(Self { mmap, header })
    }

    /// Get the number of cached templates.
    pub fn template_count(&self) -> usize {
        self.header.n_templates as usize
    }

    /// Get hidden size.
    pub fn hidden_size(&self) -> usize {
        self.header.hidden_size as usize
    }

    /// Get dtype.
    pub fn dtype(&self) -> CacheDtype {
        CacheDtype::from_u8(self.header.dtype)
    }

    /// Get template entry by index.
    pub fn get_template_entry(&self, template_id: usize) -> Option<TemplateEntry> {
        if template_id >= self.header.n_templates as usize {
            return None;
        }
        let offset = HEADER_SIZE + template_id * ENTRY_SIZE;
        if offset + ENTRY_SIZE > self.mmap.len() {
            return None;
        }
        let mut bytes = [0u8; ENTRY_SIZE];
        bytes.copy_from_slice(&self.mmap[offset..offset + ENTRY_SIZE]);
        Some(TemplateEntry::from_bytes(&bytes))
    }

    /// Get residual data for a template and layer.
    /// Returns raw bytes (f16 or f32 depending on dtype).
    pub fn get_residual_bytes(&self, template_id: usize, layer: usize) -> Option<&[u8]> {
        let entry = self.get_template_entry(template_id)?;
        let layer_range = entry.layer_range();
        if !layer_range.contains(&layer) {
            return None;
        }

        let layer_offset = layer - layer_range.start();
        let _dtype = self.dtype();
        let bytes_per_residual = self.header.bytes_per_residual();

        let data_offset = entry.data_offset as usize + layer_offset * bytes_per_residual;
        let end_offset = data_offset + bytes_per_residual;

        if end_offset > self.mmap.len() {
            return None;
        }

        Some(&self.mmap[data_offset..end_offset])
    }

    /// Get residual as f32 array (decodes from f16 if needed).
    pub fn get_residual_f32(
        &self,
        template_id: usize,
        layer: usize,
        _seq_len: usize,
    ) -> Option<Vec<f32>> {
        let bytes = self.get_residual_bytes(template_id, layer)?;
        let _dtype = self.dtype();
        let _hidden_size = self.hidden_size();

        match _dtype {
            CacheDtype::F16 => {
                // Decode f16 to f32
                let f16_data: &[u16] = unsafe {
                    std::slice::from_raw_parts(bytes.as_ptr() as *const u16, bytes.len() / 2)
                };
                Some(f16_data.iter().map(|&v| f16::to_f32(v)).collect())
            }
            CacheDtype::F32 => {
                // Direct f32
                let f32_data: &[f32] = unsafe {
                    std::slice::from_raw_parts(bytes.as_ptr() as *const f32, bytes.len() / 4)
                };
                Some(f32_data.to_vec())
            }
        }
    }

    pub fn file_size(&self) -> usize {
        self.mmap.len()
    }
}

/// Writable cache store for building cached residuals during extraction.
pub struct CacheWriter {
    file: File,
    header: CacheHeader,
    path: std::path::PathBuf,
    max_templates: usize,
    current_template: usize,
}

impl CacheWriter {
    /// Create a new cache file.
    pub fn create(
        path: &Path,
        hidden_size: usize,
        n_layers: usize,
        dtype: CacheDtype,
        max_templates: usize,
    ) -> io::Result<Self> {
        let header = CacheHeader {
            magic: MAGIC,
            version: VERSION,
            hidden_size: hidden_size as u32,
            n_layers: n_layers as u32,
            n_templates: 0,
            dtype: dtype as u8,
            _pad: [0; 107],
        };

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        file.write_all(&header.to_bytes())?;
        // Pre-allocate template index
        file.write_all(&vec![0u8; max_templates * ENTRY_SIZE])?;
        file.flush()?;

        Ok(Self {
            file,
            header,
            path: path.to_path_buf(),
            max_templates,
            current_template: 0,
        })
    }

    /// Append residuals for a template.
    ///
    /// `template_id`: Index of the template
    /// `token_count`: Number of tokens in the template prefix
    /// `layer_start`: First cached layer (e.g., 0)
    /// `layer_end`: Last cached layer (e.g., 12)
    /// `residuals`: Vec of (layer, residual_array) pairs
    pub fn append_template(
        &mut self,
        template_id: usize,
        token_count: usize,
        layer_start: usize,
        layer_end: usize,
        residuals: &[(usize, Vec<f32>)],
    ) -> io::Result<()> {
        if self.current_template >= self.max_templates {
            return Err(io::Error::other("template index full"));
        }
        if template_id != self.current_template {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "template_id must match current_template",
            ));
        }

        let hidden = self.header.hidden_size as usize;
        let dtype = CacheDtype::from_u8(self.header.dtype);
        let _bytes_per_residual = hidden * dtype.size_of();

        // Write data
        self.file.seek(SeekFrom::End(0))?;
        let data_pos = self.file.stream_position()?;

        for (layer, residual) in residuals {
            if *layer < layer_start || *layer > layer_end {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "layer {} outside range {}-{}",
                        layer, layer_start, layer_end
                    ),
                ));
            }
            if residual.len() != hidden {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "residual size mismatch: expected {}, got {}",
                        hidden,
                        residual.len()
                    ),
                ));
            }

            match dtype {
                CacheDtype::F16 => {
                    let f16_data: Vec<u16> = residual.iter().map(|&v| f16::from_f32(v)).collect();
                    let bytes: &[u8] = unsafe {
                        std::slice::from_raw_parts(
                            f16_data.as_ptr() as *const u8,
                            f16_data.len() * 2,
                        )
                    };
                    self.file.write_all(bytes)?;
                }
                CacheDtype::F32 => {
                    let bytes: &[u8] = unsafe {
                        std::slice::from_raw_parts(
                            residual.as_ptr() as *const u8,
                            residual.len() * 4,
                        )
                    };
                    self.file.write_all(bytes)?;
                }
            }
        }

        // Write index entry
        let entry = TemplateEntry {
            template_id: template_id as u32,
            token_count: token_count as u32,
            layer_start: layer_start as u32,
            layer_end: layer_end as u32,
            data_offset: data_pos,
        };
        let entry_offset = HEADER_SIZE + self.current_template * ENTRY_SIZE;
        self.file.seek(SeekFrom::Start(entry_offset as u64))?;
        self.file.write_all(&entry.to_bytes())?;

        // Update header
        self.header.n_templates += 1;
        self.current_template += 1;
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&self.header.to_bytes())?;
        self.file.flush()?;

        Ok(())
    }

    pub fn n_templates(&self) -> usize {
        self.header.n_templates as usize
    }

    pub fn finish(mut self) -> io::Result<std::path::PathBuf> {
        self.file.flush()?;
        Ok(self.path)
    }
}

/// f16 conversion utilities.
mod f16 {
    #[inline]
    pub fn from_f32(v: f32) -> u16 {
        // Simple f32 to f16 conversion
        // This is a basic implementation - for production, consider using half::f16
        let bits = v.to_bits();
        let sign = (bits >> 31) & 1;
        let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
        let mantissa = bits & 0x7fffff;

        if exponent <= 0 {
            // Underflow to zero or subnormal
            0
        } else if exponent >= 31 {
            // Overflow to infinity
            ((sign as u16) << 15) | 0x7c00
        } else {
            ((sign as u16) << 15) | ((exponent as u16) << 10) | ((mantissa >> 13) as u16)
        }
    }

    #[inline]
    pub fn to_f32(v: u16) -> f32 {
        let sign = (v >> 15) & 1;
        let exponent = ((v >> 10) & 0x1f) as i32 - 15 + 127;
        let mantissa = v & 0x3ff;

        if exponent <= 0 {
            if mantissa == 0 {
                f32::from_bits((sign as u32) << 31)
            } else {
                // Subnormal - convert to zero for simplicity
                f32::from_bits((sign as u32) << 31)
            }
        } else if exponent >= 255 {
            f32::from_bits((sign as u32) << 31 | 0x7f800000)
        } else {
            f32::from_bits((sign as u32) << 31 | (exponent as u32) << 23 | (mantissa as u32) << 13)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_cache_writer_and_reader() {
        let path = std::env::temp_dir().join("test_cache.bin");
        let hidden_size = 2560;
        let n_layers = 34;
        let dtype = CacheDtype::F16;

        // Create cache
        let mut writer = CacheWriter::create(&path, hidden_size, n_layers, dtype, 3).unwrap();

        // Template 0: cache layers 0-12
        let residuals: Vec<(usize, Vec<f32>)> = (0..=12)
            .map(|layer| (layer, vec![0.5f32; hidden_size]))
            .collect();
        writer.append_template(0, 10, 0, 12, &residuals).unwrap();

        // Template 1: cache layers 0-8
        let residuals: Vec<(usize, Vec<f32>)> = (0..=8)
            .map(|layer| (layer, vec![1.0f32; hidden_size]))
            .collect();
        writer.append_template(1, 8, 0, 8, &residuals).unwrap();

        writer.finish().unwrap();

        // Read back
        let store = CacheStore::open(&path).unwrap();
        assert_eq!(store.template_count(), 2);
        assert_eq!(store.hidden_size(), hidden_size);
        assert_eq!(store.dtype(), CacheDtype::F16);

        // Get template 0 entry
        let entry0 = store.get_template_entry(0).unwrap();
        assert_eq!(entry0.template_id, 0);
        assert_eq!(entry0.token_count, 10);
        assert_eq!(entry0.layer_start, 0);
        assert_eq!(entry0.layer_end, 12);

        // Get residuals
        let residual = store.get_residual_f32(0, 5, 10).unwrap();
        assert_eq!(residual.len(), hidden_size);
        // Check approximate value (f16 conversion has some precision loss)
        assert!((residual[0] - 0.5).abs() < 0.01);

        // Cleanup
        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_f16_conversion() {
        let val = 0.5f32;
        let f16 = f16::from_f32(val);
        let back = f16::to_f32(f16);
        assert!((val - back).abs() < 0.01);
    }
}
