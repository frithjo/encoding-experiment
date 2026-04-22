// blas_src link is provided by larql-tensor (renamed as `ndarray`) via the `blas` feature.

pub mod attention;
pub mod capture;
pub mod error;
pub mod ffn;
pub mod forward;
pub mod graph_ffn;
pub mod layer_graph;
pub mod model;
pub mod residual;
pub mod route_ffn;
pub mod tokenizer;
pub mod trace;
pub mod vindex;
pub mod walker;

// Re-export dependencies for downstream crates.
pub use larql_models;
pub use larql_tokenizer;
pub use larql_vindex;
pub use ndarray;

// Backend re-exports (from larql-compute).
pub use larql_compute::CpuBackend;
#[cfg(feature = "metal")]
pub use larql_compute::MetalBackend;
pub use larql_compute::{
    cpu_backend, default_backend, dot_proj_gpu, matmul_gpu, ComputeBackend, MatMulOp,
};

// Re-export essentials at crate root.
pub use attention::AttentionWeights;
pub use capture::{
    CaptureCallbacks, CaptureConfig, InferenceModel, TopKEntry, VectorFileHeader, VectorRecord,
};
pub use error::InferenceError;
#[allow(deprecated)]
pub use ffn::experimental::cached::CachedFfn;
#[allow(deprecated)]
pub use ffn::experimental::clustered::{ClusteredFfn, ClusteredGateIndex};
#[allow(deprecated)]
pub use ffn::experimental::down_clustered::{DownClusteredFfn, DownClusteredIndex};
#[allow(deprecated)]
pub use ffn::experimental::entity_routed::EntityRoutedFfn;
#[allow(deprecated)]
pub use ffn::experimental::feature_list::FeatureListFfn;
#[allow(deprecated)]
pub use ffn::experimental::graph::GraphFfn;
pub use ffn::{FfnBackend, HighwayFfn, LayerFfnRouter, SparseFfn, WeightFfn};
pub use forward::{
    calibrate_scalar_gains, capture_decoy_residuals, capture_ffn_activation_matrix,
    capture_residuals, estimate_ffn_covariance, forward_to_layer, logit_lens_top1, predict,
    predict_from_hidden, predict_from_hidden_with_ffn, predict_with_ffn,
    predict_with_ffn_attention, predict_with_ffn_trace, predict_with_router, predict_with_strategy,
    run_memit, trace_forward, trace_forward_full, trace_forward_with_ffn, LayerAttentionCapture,
    LayerMode, MemitFact, MemitFactResult, MemitResult, PredictResult, PredictResultWithAttention,
    PredictResultWithResiduals, TraceResult,
};
pub use graph_ffn::{GateIndex, IndexBuildCallbacks, SilentIndexCallbacks};
pub use layer_graph::{
    build_adaptive_graph,
    detect_template,
    generate,
    hybrid::predict_hybrid,
    predict_honest,
    predict_pipeline,
    predict_split_cached,
    predict_split_pass,
    predict_with_graph,
    predict_with_graph_vindex_logits,
    trace_with_graph,
    AttentionCache,
    CachedLayerGraph,
    DenseLayerGraph,
    GenerateResult,
    GuidedWalkLayerGraph,
    // Production
    LayerGraph,
    LayerOutput,
    PerLayerGraph,
    PipelinedLayerGraph,
    // Analysis/validation
    TemplatePattern,
    TemplateUniverse,
    WalkLayerGraph,
};
pub use model::{load_model_dir, resolve_model_path, ModelWeights};
pub use route_ffn::{RouteFfn, RouteGuidedFfn, RouteTable};
pub use tokenizer::{decode_token, decode_token_raw, load_tokenizer};
pub use trace::{
    trace as trace_decomposed, trace_residuals, AnswerWaypoint, BoundaryStore, BoundaryWriter,
    ContextStore, ContextTier, ContextWriter, LayerSummary, ResidualTrace, TraceNode,
    TracePositions, TraceStore, TraceWriter,
};
pub use vindex::WalkFfn;

// Walker re-exports.
pub use walker::attention_walker::{AttentionLayerResult, AttentionWalker};
pub use walker::vector_extractor::{
    ExtractCallbacks, ExtractConfig, ExtractSummary, VectorExtractor,
};
pub use walker::weight_walker::{
    walk_model, LayerResult, LayerStats, WalkCallbacks, WalkConfig, WeightWalker,
};
