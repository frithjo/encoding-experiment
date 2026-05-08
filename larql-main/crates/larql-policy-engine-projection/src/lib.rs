//! Policy engine snapshots are built in Rust (`PolicyEngineStateSnapshot`). UI layers must treat
//! this crate as the only authority-bearing source for Phase A renders.

mod snapshot;

#[cfg(not(target_arch = "wasm32"))]
pub use snapshot::build_policy_engine_state_snapshot_from_paths;
pub use snapshot::{
    build_policy_engine_state_snapshot_from_embedded_workspace_policies, sample_repo_ci_policy_input,
    PolicyEngineEvaluationSnapshot, PolicyEngineStateSnapshot, PolicyRegistrySnapshot,
    PolicySourceFingerprint, SnapshotError, POLICY_ENGINE_STATE_SNAPSHOT_SCHEMA,
    POLICY_REPO_FINGERPRINT_PREFIX,
};
