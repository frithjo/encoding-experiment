//! Centralized configuration management for LARQL.
//!
//! This module provides a config-base approach to eliminate hardcoded values
//! and path dependencies from the codebase. Configuration is loaded from:
//! 1. Default config file (config/default.toml)
//! 2. Local override file (config/local.toml)
//! 3. Environment variables (prefixed with LARQL__)
//!
//! Environment variable mapping:
//! - LARQL_VINDEX__PATH -> vindex.path
//! - LARQL_MODEL__PATH -> models.path
//! - LARQL_HUGGINGFACE__TOKEN -> huggingface.token
//! - LARQL_HUGGINGFACE__CACHE_DIR -> huggingface.cache_dir
//! - LARQL_PATHS__HOME_DIR -> paths.home_dir

mod types;
mod loader;

pub use types::{AppConfig, VindexConfig, ModelConfig, HuggingFaceConfig, PathConfig};
pub use loader::{load_config, ConfigError};
