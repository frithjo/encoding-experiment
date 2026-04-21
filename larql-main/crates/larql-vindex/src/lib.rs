//! Vindex — the queryable model format.
//!
//! Decompile, browse, edit, and recompile neural networks.
//! This crate owns the complete vindex lifecycle:
//! extract, load, query, mutate, patch, save, compile.

// BLAS provided by larql-compute dependency (no direct blas_src needed)

// ── Module structure ──
pub mod clustering;
pub mod config;
pub mod describe;
pub mod describe_collect;
pub mod error;
pub mod extract;
pub mod format;
pub mod index;
pub mod patch;
pub mod mmap_util;
pub mod text;
pub mod token_summary;
pub mod vindexfile;

// ── Re-export dependencies ──
pub use ndarray;
pub use larql_tokenizer;
pub use larql_tokenizer::Tokenizer as TokenizerTrait;

/// Convenience alias for a shared, trait-object tokenizer handle.
///
/// All vindex APIs that hold a tokenizer use this type, letting callers swap
/// the underlying impl (HF, vocab-only, ...) without changing signatures.
pub type TokenizerArc = std::sync::Arc<dyn larql_tokenizer::Tokenizer>;

// ── Re-export essentials at crate root ──

// Config
pub use config::dtype::StorageDtype;
pub use config::types::{
    DownMetaRecord, DownMetaTopK, ExtractLevel, LayerBands, MoeConfig,
    VindexConfig, VindexLayerInfo, VindexModelConfig, VindexSource,
};

// Error
pub use error::VindexError;

// Index
pub use index::core::{
    FeatureMeta, GateIndex, IndexLoadCallbacks, SilentLoadCallbacks, VectorIndex, WalkHit, WalkTrace,
};
pub use index::router::{RouterIndex, RouteResult};
pub use index::residency::{ResidencyManager, LayerState};

// Describe
pub use describe::{DescribeEdge, LabelSource};
pub use describe_collect::{
    collect_describe_edges_from_trace, CollectedDescribeEdge, DESCRIBE_GATE_FLOOR_DEFAULT,
    DESCRIBE_WALK_TOP_K,
};
pub use text::{is_content_token, is_readable_token};
pub use token_summary::{
    band_name_for_layer, classify_token_shape, collect_token_hits, entity_token_kind,
    token_shape_name, KindInfo, ShapeInfo, TokenFilters, TokenHit, TokenShape,
};

// Extract
pub use extract::{
    build_vindex, build_vindex_resume, build_vindex_from_vectors,
    build_vindex_streaming,
    IndexBuildCallbacks, SilentBuildCallbacks,
};

// Format
pub use format::checksums;
pub use format::down_meta;
pub use format::load::{
    load_feature_labels, load_vindex_config, load_vindex_embeddings, load_vindex_tokenizer,
};
// Model loading: use larql_models::{load_model_dir, resolve_model_path, load_gguf} directly
pub use format::huggingface::{
    resolve_hf_vindex, download_hf_weights, publish_vindex,
    is_hf_path, PublishCallbacks, SilentPublishCallbacks,
};
pub use format::weights::{write_model_weights, load_model_weights, WeightSource, StreamingWeights};

// Patch
pub use patch::core::{PatchOp, PatchedVindex, VindexPatch};
pub use patch::knn_store::{KnnStore, KnnEntry};
pub use patch::refine::{refine_gates, RefineInput, RefineResult, RefinedGate};

// Vindexfile
pub use vindexfile::{Vindexfile, VindexfileDirective, VindexfileStage, parse_vindexfile, build_from_vindexfile};
