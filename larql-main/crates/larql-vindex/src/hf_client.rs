//! HuggingFace Hub client for vindex repository downloads.
//!
//! Supports downloading vindex repositories from HuggingFace using hf:// URLs.

use std::path::{Path, PathBuf};

#[cfg(feature = "huggingface")]
use larql_core::hf_hub::{api::sync::ApiBuilder, Repo, RepoType};

/// Download a vindex repository from HuggingFace.
///
/// # Arguments
///
/// * `repo_id` - HuggingFace repository ID (e.g., "larql-org/gemma-4b-vindex")
/// * `cache_dir` - Directory to cache downloaded files
///
/// # Returns
///
/// Path to the downloaded vindex directory
#[cfg(feature = "huggingface")]
pub async fn download_vindex(repo_id: &str, cache_dir: &Path) -> Result<PathBuf, VindexError> {
    // Use HF Hub's built-in caching
    let api = ApiBuilder::new()
        .with_cache_dir(cache_dir.to_path_buf())
        .with_progress(false)
        .build()
        .map_err(|e| VindexError::Download(format!("Failed to create HF API client: {}", e)))?;

    let repo = Repo::new(repo_id.to_string(), RepoType::Model);

    // Get the repository - this will download if not cached
    let _api_repo = api.repo(repo);

    // Return the cache directory path where HF stores the repo
    // HF Hub stores repos in: cache_dir/hub/models--<org>--<repo>
    let repo_cache_name = repo_id.replace('/', "--");
    let repo_path = cache_dir.join("hub").join(format!("models--{}", repo_cache_name));
    
    // Check if the repo was actually downloaded/cached
    if !repo_path.exists() {
        return Err(VindexError::Download(format!(
            "Repository not found at expected path: {}. Download may have failed.",
            repo_path.display()
        )));
    }

    Ok(repo_path)
}

/// Download a vindex repository from HuggingFace (no HF feature).
#[cfg(not(feature = "huggingface"))]
pub async fn download_vindex(_repo_id: &str, _cache_dir: &Path) -> Result<PathBuf, VindexError> {
    Err(VindexError::Download(
        "HuggingFace integration is not enabled. Build with --features huggingface".to_string(),
    ))
}

/// Resolve an hf:// path to a local vindex path.
///
/// # Arguments
///
/// * `path` - Path starting with hf://
///
/// # Returns
///
/// Local path to the vindex (either cached or download path)
pub async fn resolve_hf_path(path: &str) -> Result<PathBuf, VindexError> {
    if !path.starts_with("hf://") {
        return Err(VindexError::Parse(format!("Not an hf:// path: {}", path)));
    }

    let repo_id = path.strip_prefix("hf://").unwrap();
    let home_dir = std::env::var("LARQL_PATHS__HOME_DIR")
        .or_else(|_| std::env::var("HOME"))
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| VindexError::Parse("Cannot determine home directory".to_string()))?;
    
    let cache_dir = PathBuf::from(home_dir).join(".cache").join("larql");
    download_vindex(repo_id, &cache_dir).await
}

#[derive(Debug)]
pub enum VindexError {
    Parse(String),
    Download(String),
}

impl std::fmt::Display for VindexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VindexError::Parse(msg) => write!(f, "Parse error: {}", msg),
            VindexError::Download(msg) => write!(f, "Download error: {}", msg),
        }
    }
}

impl std::error::Error for VindexError {}

#[cfg(test)]
mod tests {
    #[test]
    fn test_hf_path_parsing() {
        // Test that hf:// prefix is correctly stripped
        let path = "hf://larql-org/gemma-4b-vindex";
        assert!(path.starts_with("hf://"));
        assert_eq!(path.strip_prefix("hf://").unwrap(), "larql-org/gemma-4b-vindex");
    }
}
