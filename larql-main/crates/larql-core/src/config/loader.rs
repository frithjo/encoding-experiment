//! Configuration loading logic.

use config::{Config, Environment, File};
use thiserror::Error;

use crate::config::types::AppConfig;

/// Configuration loading errors.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to load configuration: {0}")]
    LoadError(#[from] config::Error),
    
    #[error("Configuration validation failed: {0}")]
    ValidationError(String),
}

/// Load application configuration from multiple sources.
///
/// Loading order (later sources override earlier ones):
/// 1. Default values from structs
/// 2. config/default.toml (if present)
/// 3. config/local.toml (if present, for local overrides)
/// 4. .env file (if present)
/// 5. Environment variables (LARQL__ prefix)
///
/// Environment variable mapping:
/// - LARQL_VINDEX__DEFAULT_PATH -> vindex.default_path
/// - LARQL_VINDEX__BITNET_PATH -> vindex.bitnet_path
/// - LARQL_VINDEX__GEMMA_PATH -> vindex.gemma_path
/// - LARQL_MODELS__BITNET_ID -> models.bitnet_id
/// - LARQL_MODELS__GEMMA_ID -> models.gemma_id
/// - LARQL_HUGGINGFACE__TOKEN -> huggingface.token
/// - LARQL_HUGGINGFACE__CACHE_DIR -> huggingface.cache_dir
pub fn load_config() -> Result<AppConfig, ConfigError> {
    // Load .env file if present
    dotenvy::dotenv().ok();
    
    let config = Config::builder()
        // Add default config file (optional)
        .add_source(File::with_name("config/default").required(false))
        // Add local override config file (optional)
        .add_source(File::with_name("config/local").required(false))
        // Add environment variables with LARQL__ prefix
        .add_source(
            Environment::with_prefix("LARQL")
                .prefix_separator("__")
                .separator("__")
        )
        .build()?;
    
    let app_config: AppConfig = config.try_deserialize()?;
    
    // Validate configuration
    validate_config(&app_config)?;
    
    Ok(app_config)
}

/// Validate configuration values.
fn validate_config(config: &AppConfig) -> Result<(), ConfigError> {
    // Check that default vindex path is not empty
    if config.vindex.default_path.is_empty() {
        return Err(ConfigError::ValidationError(
            "vindex.default_path cannot be empty".to_string()
        ));
    }
    
    // Check that output directory is not empty
    if config.vindex.output_dir.is_empty() {
        return Err(ConfigError::ValidationError(
            "vindex.output_dir cannot be empty".to_string()
        ));
    }
    
    Ok(())
}

/// Get a configuration value or return a default.
///
/// This is a convenience function for optional configuration values.
pub fn get_or_default<T, F>(value: Option<T>, default: F) -> T
where
    F: FnOnce() -> T,
{
    value.unwrap_or_else(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert!(!config.vindex.default_path.is_empty());
        assert_eq!(config.models.bitnet_id, "bitnet-ml/BitNet-1B58-3B");
        assert_eq!(config.models.gemma_id, "google/gemma-3-4b-it");
    }
    
    #[test]
    fn test_path_resolution() {
        let path_config = PathConfig::default();
        
        // Test relative path resolution
        let resolved = path_config.resolve_path("data/test");
        assert!(resolved.is_absolute());
        
        // Test absolute path
        let resolved = path_config.resolve_path("/tmp/test");
        assert_eq!(resolved, PathBuf::from("/tmp/test"));
    }
}
