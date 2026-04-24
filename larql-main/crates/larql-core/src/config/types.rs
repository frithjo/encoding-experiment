//! Configuration types for LARQL application.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Vindex path configuration
    pub vindex: VindexConfig,
    /// Model configuration
    pub models: ModelConfig,
    /// HuggingFace configuration
    pub huggingface: HuggingFaceConfig,
    /// Path resolution configuration
    pub paths: PathConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            vindex: VindexConfig::default(),
            models: ModelConfig::default(),
            huggingface: HuggingFaceConfig::default(),
            paths: PathConfig::default(),
        }
    }
}

/// Vindex path configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VindexConfig {
    /// Vindex path for loading vector index
    pub path: String,
}

impl Default for VindexConfig {
    fn default() -> Self {
        Self {
            path: "data/bitnet_b1_58-large/vindex".to_string(),
        }
    }
}

/// Model configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Model path (HuggingFace model ID or local path)
    pub path: String,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            path: "google/gemma-3-4b-it".to_string(),
        }
    }
}

/// HuggingFace configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HuggingFaceConfig {
    /// HuggingFace API token (required for private models)
    #[serde(default)]
    pub token: Option<String>,
    /// HuggingFace cache directory
    #[serde(default = "default_hf_cache")]
    pub cache_dir: String,
}

fn default_hf_cache() -> String {
    ".cache/huggingface/hub".to_string()
}

impl Default for HuggingFaceConfig {
    fn default() -> Self {
        Self {
            token: None,
            cache_dir: default_hf_cache(),
        }
    }
}

/// Path resolution configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathConfig {
    /// Home directory (required)
    #[serde(default)]
    pub home_dir: Option<String>,
}

impl Default for PathConfig {
    fn default() -> Self {
        Self {
            home_dir: None,
        }
    }
}

impl PathConfig {
    /// Get the home directory.
    pub fn home_dir(&self) -> PathBuf {
        if let Some(ref home) = self.home_dir {
            PathBuf::from(home)
        } else {
            dirs::home_dir().expect("HOME directory not found and LARQL_PATHS__HOME_DIR not set")
        }
    }

    /// Resolve a path that may contain ~ or $HOME.
    pub fn resolve_path(&self, path: &str) -> PathBuf {
        let path = path.trim();
        
        // Handle ~ expansion
        if path.starts_with("~/") {
            let mut resolved = self.home_dir();
            resolved.push(&path[2..]);
            return resolved;
        }
        
        // Handle $HOME expansion
        if path.starts_with("$HOME/") {
            let mut resolved = self.home_dir();
            resolved.push(&path[6..]);
            return resolved;
        }
        
        // Handle absolute paths
        if path.starts_with('/') {
            return PathBuf::from(path);
        }
        
        // Handle relative paths
        let mut resolved = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        resolved.push(path);
        resolved
    }
}
