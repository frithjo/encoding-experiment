//! Shared helpers: formatting, token filtering.

use std::path::Path;

pub(crate) use larql_vindex::is_content_token;
#[cfg(test)]
pub(crate) use larql_vindex::is_readable_token;

// Token shape classification + collection moved to `larql_vindex::token_summary`.
// Re-exported here to keep existing `pub(crate)` call sites unchanged.
pub(crate) use larql_vindex::token_summary::{
    classify_token_shape, collect_token_hits, entity_token_kind, token_shape_name, KindInfo,
    ShapeInfo, TokenFilters, TokenHit, TokenShape,
};

/// Get total size of a directory in bytes.
pub(crate) fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

pub(crate) fn format_number(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{n}")
    }
}

pub(crate) fn format_bytes(b: u64) -> String {
    if b >= 1_073_741_824 {
        format!("{:.2} GB", b as f64 / 1_073_741_824.0)
    } else if b >= 1_048_576 {
        format!("{:.1} MB", b as f64 / 1_048_576.0)
    } else if b >= 1024 {
        format!("{:.1} KB", b as f64 / 1024.0)
    } else {
        format!("{b} B")
    }
}

// Token-shape classification, stopword list, `entity_token_kind`, and their
// tests now live in `larql_vindex::token_summary` — re-exported above.
