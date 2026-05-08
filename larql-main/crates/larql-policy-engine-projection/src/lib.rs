//! Policy engine snapshots are built in Rust (`PolicyEngineStateSnapshot`). UI layers must treat
//! this crate as the only authority-bearing source for Phase A renders.

mod ceremony_debt;
mod eval_preview;
mod snapshot;

pub use ceremony_debt::{
    build_ceremony_debt_state_snapshot_embedded, embedded_ceremony_debt_source_paths_expectation,
    CeremonyDebtSnapshotError, CeremonyDebtStateSnapshot, CeremonyDefinitionLeaf,
    CEREMONY_DEBT_STATE_SNAPSHOT_SCHEMA,
};
pub use eval_preview::{
    build_policy_eval_preview_embedded, policy_input_from_preview_draft,
    policy_input_from_preview_draft_with_base, preview_draft_from_ci_fixture, PolicyEvalPreview,
    PolicyEvalPreviewDraft, PreviewFactsSpecification, POLICY_EVAL_PREVIEW_SCHEMA,
};
pub use larql_governance::decision_surface::{
    CeremonyDecisionBudget, CeremonyDecisionBudgets, DecisionOwner, DecisionRisk, DecisionStatus,
    DecisionSurfaceItem, DecisionSurfaceRegistry, PromotionPath,
};
#[cfg(not(target_arch = "wasm32"))]
pub use snapshot::build_policy_engine_state_snapshot_from_paths;
pub use snapshot::{
    build_policy_engine_state_snapshot_from_embedded_workspace_policies,
    sample_repo_ci_policy_input, PolicyEngineEvaluationSnapshot, PolicyEngineStateSnapshot,
    PolicyRegistrySnapshot, PolicySourceFingerprint, SnapshotError,
    POLICY_ENGINE_STATE_SNAPSHOT_SCHEMA, POLICY_REPO_FINGERPRINT_PREFIX,
};
