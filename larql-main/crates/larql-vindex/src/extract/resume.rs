use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json;

use crate::config::VindexLayerInfo;
use crate::error::VindexError;

pub const PROGRESS_FILE: &str = "extract-progress.json";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExtractBuildMode {
    FromVectors,
    Streaming,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractProgress {
    pub version: u32,
    pub mode: ExtractBuildMode,
    pub model_name: Option<String>,
    pub num_layers: Option<usize>,
    pub gate_next_layer: usize,
    pub embeddings_complete: bool,
    pub down_meta_next_layer: usize,
    pub tokenizer_complete: bool,
    pub weight_manifest_complete: bool,
    pub layer_infos: Vec<VindexLayerInfo>,
    pub started_at: String,
    pub updated_at: String,
}

impl ExtractProgress {
    fn now_timestamp() -> String {
        super::build::chrono_now()
    }

    pub fn new(
        mode: ExtractBuildMode,
        model_name: Option<String>,
        num_layers: Option<usize>,
    ) -> Self {
        let timestamp = Self::now_timestamp();
        Self {
            version: 1,
            mode,
            model_name,
            num_layers,
            gate_next_layer: 0,
            embeddings_complete: false,
            down_meta_next_layer: 0,
            tokenizer_complete: false,
            weight_manifest_complete: false,
            layer_infos: Vec::new(),
            started_at: timestamp.clone(),
            updated_at: timestamp,
        }
    }

    pub fn path(output_dir: &Path) -> PathBuf {
        output_dir.join(PROGRESS_FILE)
    }

    pub fn load(output_dir: &Path) -> Option<Self> {
        let path = Self::path(output_dir);
        if !path.exists() {
            return None;
        }

        let raw = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn save(&self, output_dir: &Path) -> Result<(), VindexError> {
        let path = Self::path(output_dir);
        let tmp = output_dir.join(format!("{PROGRESS_FILE}.tmp"));

        let serialized = serde_json::to_string_pretty(self)
            .map_err(|e| VindexError::Parse(format!("serialize progress: {e}")))?;
        fs::write(&tmp, serialized)?;
        fs::rename(tmp, path)?;
        Ok(())
    }

    pub fn touch(&mut self) {
        self.updated_at = Self::now_timestamp();
    }

    pub fn is_compatible(
        &self,
        mode: &ExtractBuildMode,
        model_name: Option<&str>,
        num_layers: usize,
    ) -> bool {
        if self.mode != *mode {
            return false;
        }

        let expected_model = match (&self.model_name, model_name) {
            (Some(existing), Some(next)) => existing == next,
            (None, _) => true,
            (Some(_), None) => false,
        };

        if !expected_model {
            return false;
        }

        match self.num_layers {
            Some(known_layers) if known_layers != num_layers => false,
            Some(_) => true,
            None => true,
        }
    }

    pub fn can_resume(
        &self,
        mode: &ExtractBuildMode,
        model_name: Option<&str>,
        num_layers: usize,
    ) -> bool {
        self.version == 1 && self.is_compatible(mode, model_name, num_layers)
    }

    pub fn gate_start_layer(&self, num_layers: usize) -> usize {
        self.gate_next_layer.min(num_layers)
    }

    pub fn down_meta_start_layer(&self, num_layers: usize) -> usize {
        self.down_meta_next_layer.min(num_layers)
    }

    pub fn gate_layer_done(&mut self, layer: usize) {
        if layer + 1 > self.gate_next_layer {
            self.gate_next_layer = layer + 1;
            self.touch();
        }
    }

    pub fn embeddings_done(&mut self) {
        if !self.embeddings_complete {
            self.embeddings_complete = true;
            self.touch();
        }
    }

    pub fn down_meta_layer_done(&mut self, layer: usize) {
        if layer + 1 > self.down_meta_next_layer {
            self.down_meta_next_layer = layer + 1;
            self.touch();
        }
    }

    pub fn tokenizer_done(&mut self) {
        if !self.tokenizer_complete {
            self.tokenizer_complete = true;
            self.touch();
        }
    }
}
