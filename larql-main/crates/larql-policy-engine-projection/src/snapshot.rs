use std::collections::BTreeMap;
use std::path::Path;

use larql_governance::hash::{hash_bytes, hash_json};
use larql_governance::rules_engine::{
    load_policy_registry, load_policy_registry_from_material, PolicyEngine, PolicyEngineDecision,
    PolicyInput, PolicyLoadError, PolicyMode, PolicyRegistry,
};
use serde::{Deserialize, Serialize};

/// Schema marker for snapshots consumed by projection-only hosts (Leptos, Tauri RPC, etc.).
pub const POLICY_ENGINE_STATE_SNAPSHOT_SCHEMA: &str =
    "larql.governance.policy_engine_state_snapshot.v1";

/// Slash-normalized prefix for provenance fingerprints in this repo (`governance/policies`).
/// This must not encode host-specific absolute paths.
pub const POLICY_REPO_FINGERPRINT_PREFIX: &str = "governance/policies";

#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    #[error(transparent)]
    Policy(#[from] larql_governance::rules_engine::PolicyLoadError),
    #[cfg(not(target_arch = "wasm32"))]
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    InvalidFingerprint(String),
}

fn fingerprint_under_policies(include_style_path: &str) -> Result<String, SnapshotError> {
    assert_safe_piece(include_style_path)?;
    let normalized = include_style_path.replace('\\', "/");
    Ok(format!(
        "{POLICY_REPO_FINGERPRINT_PREFIX}/{normalized}",
    ))
}

fn assert_safe_piece(piece: &str) -> Result<(), SnapshotError> {
    if piece.is_empty() {
        return Err(SnapshotError::InvalidFingerprint(
            "path piece must not be empty".into(),
        ));
    }
    if piece.contains("..")
        || piece.starts_with('/')
        || piece.starts_with('\\')
        || piece.contains(':')
    {
        return Err(SnapshotError::InvalidFingerprint(format!(
            "unsafe repo-relative fingerprint piece: {piece:?}"
        )));
    }
    Ok(())
}

/// Raw file-byte identity (`sha256:` + hex). Material is normalized UTF-8 policy text bytes.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct PolicySourceFingerprint {
    pub repo_relative_path: String,
    pub content_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct PolicyRegistrySnapshot {
    pub schema_version: String,
    pub active_policy_set_id: String,
    pub active_policy_set_version: String,
    pub policy_hash: String,
    pub mode: PolicyMode,
    pub rule_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct PolicyEngineEvaluationSnapshot {
    pub policy_input: PolicyInput,
    pub policy_input_sha256: String,
    pub decision: PolicyEngineDecision,
    pub decision_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct PolicyEngineStateSnapshot {
    pub schema_version: String,
    pub snapshot_content_sha256: String,
    pub policy_index: PolicySourceFingerprint,
    pub policy_packs: Vec<PolicySourceFingerprint>,
    pub registry: PolicyRegistrySnapshot,
    pub evaluation: PolicyEngineEvaluationSnapshot,
}

#[derive(Serialize)]
struct PolicyEngineStateSnapshotBody<'a> {
    schema_version: &'a str,
    policy_index: &'a PolicySourceFingerprint,
    policy_packs: &'a [PolicySourceFingerprint],
    registry: &'a PolicyRegistrySnapshot,
    evaluation: &'a PolicyEngineEvaluationSnapshot,
}

fn registry_snapshot(registry: &PolicyRegistry) -> PolicyRegistrySnapshot {
    PolicyRegistrySnapshot {
        schema_version: registry.schema_version.clone(),
        active_policy_set_id: registry.active_policy_set.id.clone(),
        active_policy_set_version: registry.active_policy_set.version.clone(),
        policy_hash: registry.policy_hash.clone(),
        mode: registry.mode.clone(),
        rule_count: registry.rules.len() as u32,
    }
}

fn evaluation_snapshot(policy_input: &PolicyInput, decision: &PolicyEngineDecision) -> Result<PolicyEngineEvaluationSnapshot, serde_json::Error> {
    Ok(PolicyEngineEvaluationSnapshot {
        policy_input: policy_input.clone(),
        policy_input_sha256: hash_json(policy_input)?,
        decision: decision.clone(),
        decision_sha256: hash_json(decision)?,
    })
}

fn sort_policy_pack_fingerprints(mut packs: Vec<PolicySourceFingerprint>) -> Vec<PolicySourceFingerprint> {
    packs.sort_by(|left, right| left.repo_relative_path.cmp(&right.repo_relative_path));
    packs
}

fn compute_snapshot_body_hash(
    policy_index: &PolicySourceFingerprint,
    policy_packs: &[PolicySourceFingerprint],
    registry: &PolicyRegistrySnapshot,
    evaluation: &PolicyEngineEvaluationSnapshot,
) -> Result<String, serde_json::Error> {
    let body = PolicyEngineStateSnapshotBody {
        schema_version: POLICY_ENGINE_STATE_SNAPSHOT_SCHEMA,
        policy_index,
        policy_packs,
        registry,
        evaluation,
    };
    hash_json(&body)
}

pub fn sample_repo_ci_policy_input() -> Result<PolicyInput, serde_json::Error> {
    const FIXTURE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../governance/facts/ci_governance_rules.json"
    ));
    serde_json::from_str(FIXTURE)
}

fn core_snapshot_from_loaded_registry(
    registry: PolicyRegistry,
    policy_index: PolicySourceFingerprint,
    raw_policy_pack_fingerprints: Vec<PolicySourceFingerprint>,
    policy_input: &PolicyInput,
    engine_eval: PolicyEngineDecision,
) -> Result<PolicyEngineStateSnapshot, SnapshotError> {
    let packs_sorted = sort_policy_pack_fingerprints(raw_policy_pack_fingerprints);
    let registry_excerpt = registry_snapshot(&registry);
    let evaluation =
        evaluation_snapshot(policy_input, &engine_eval).map_err(SnapshotError::Json)?;
    let snapshot_content_sha256 = compute_snapshot_body_hash(
        &policy_index,
        &packs_sorted,
        &registry_excerpt,
        &evaluation,
    )
    .map_err(SnapshotError::Json)?;

    Ok(PolicyEngineStateSnapshot {
        schema_version: POLICY_ENGINE_STATE_SNAPSHOT_SCHEMA.to_string(),
        snapshot_content_sha256,
        policy_index,
        policy_packs: packs_sorted,
        registry: registry_excerpt,
        evaluation,
    })
}

/// Build a snapshot from on-disk governance policy paths (native targets only).
#[cfg(not(target_arch = "wasm32"))]
pub fn build_policy_engine_state_snapshot_from_paths(
    index_path: &Path,
    policy_input: &PolicyInput,
) -> Result<PolicyEngineStateSnapshot, SnapshotError> {
    let registry = load_policy_registry(index_path)?;
    let engine = PolicyEngine::new(registry.clone())?;
    let decision = engine.evaluate(policy_input);

    let index_bytes = std::fs::read_to_string(index_path)?;
    assert_safe_piece(
        index_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| SnapshotError::InvalidFingerprint("policy index missing file name".into()))?,
    )?;

    let index_file_name = index_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap();
    let policy_index_fp = PolicySourceFingerprint {
        repo_relative_path: format!("{POLICY_REPO_FINGERPRINT_PREFIX}/{index_file_name}",),
        content_sha256: hash_bytes(index_bytes.as_bytes()),
    };

    let root_dir = index_path
        .parent()
        .ok_or_else(|| SnapshotError::Policy(PolicyLoadError::MissingPolicyRoot(
            index_path.display().to_string(),
        )))?;

    let mut pack_fps = Vec::new();
    for include in &registry.active_policy_set.includes {
        let pack_path = root_dir.join(include);
        let text = std::fs::read_to_string(&pack_path)?;
        pack_fps.push(PolicySourceFingerprint {
            repo_relative_path: fingerprint_under_policies(include)?,
            content_sha256: hash_bytes(text.as_bytes()),
        });
    }

    core_snapshot_from_loaded_registry(registry, policy_index_fp, pack_fps, policy_input, decision)
}

// --- WASM / CSR bundle helpers (pinned to exactly the tracked policy index contents) ---
const EMBEDDED_POLICY_INDEX_TXT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../governance/policies/index.toml"
));

fn embedded_policy_pack_map() -> BTreeMap<String, String> {
    let mut packs = BTreeMap::new();
    packs.insert(
        "ci.toml".into(),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../governance/policies/ci.toml"
        ))
        .to_string(),
    );
    packs.insert(
        "machine-profile.toml".into(),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../governance/policies/machine-profile.toml"
        ))
        .to_string(),
    );
    packs.insert(
        "machine-runtime.toml".into(),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../governance/policies/machine-runtime.toml"
        ))
        .to_string(),
    );
    packs.insert(
        "rule-admission.toml".into(),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../governance/policies/rule-admission.toml"
        ))
        .to_string(),
    );
    packs
}

#[derive(Debug, Deserialize)]
struct PolicyIndexFingerprintsGuard {
    active_policy_set: IncludesGuard,
}

#[derive(Debug, Deserialize)]
struct IncludesGuard {
    includes: Vec<String>,
}

/// Guard used by tests to ensure embedded `include_str!` packs match the declared index order.
pub fn embedded_pack_keys_match_embedded_policy_index() -> Result<(), SnapshotError> {
    let guard: PolicyIndexFingerprintsGuard =
        toml::from_str(EMBEDDED_POLICY_INDEX_TXT).map_err(|err| {
            SnapshotError::InvalidFingerprint(format!("embedded policy index parse failed: {err}"))
        })?;
    let mut declared = guard.active_policy_set.includes;
    declared.sort();

    let mut embedded_keys: Vec<String> = embedded_policy_pack_map().keys().cloned().collect();
    embedded_keys.sort();

    if declared != embedded_keys {
        return Err(SnapshotError::InvalidFingerprint(format!(
            "embedded policy pack map keys {embedded_keys:?} differ from index includes {declared:?}"
        )));
    }
    Ok(())
}

/// Snapshot built entirely from embedded repo policy sources (CSR/WASM-safe).
pub fn build_policy_engine_state_snapshot_from_embedded_workspace_policies(
    policy_input: &PolicyInput,
) -> Result<PolicyEngineStateSnapshot, SnapshotError> {
    embedded_pack_keys_match_embedded_policy_index()?;
    let packs = embedded_policy_pack_map();
    let registry = load_policy_registry_from_material(
        "governance/policies/index.toml",
        EMBEDDED_POLICY_INDEX_TXT,
        &packs,
    )?;
    let engine = PolicyEngine::new(registry.clone())?;
    let decision = engine.evaluate(policy_input);

    let policy_index_fp = PolicySourceFingerprint {
        repo_relative_path: format!("{POLICY_REPO_FINGERPRINT_PREFIX}/index.toml"),
        content_sha256: hash_bytes(EMBEDDED_POLICY_INDEX_TXT.as_bytes()),
    };

    let mut pack_fps = Vec::new();
    for include in &registry.active_policy_set.includes {
        let text = packs.get(include).ok_or_else(|| {
            SnapshotError::InvalidFingerprint(format!(
                "embedded pack missing for index include {include:?}",
            ))
        })?;        pack_fps.push(PolicySourceFingerprint {
            repo_relative_path: fingerprint_under_policies(include)?,
            content_sha256: hash_bytes(text.as_bytes()),
        });
    }

    core_snapshot_from_loaded_registry(
        registry,
        policy_index_fp,
        pack_fps,
        policy_input,
        decision,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    fn repo_index_path() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../governance/policies/index.toml")
    }

    #[test]
    fn embedded_keys_track_policy_index() {
        embedded_pack_keys_match_embedded_policy_index().unwrap();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn disk_and_embedded_policy_hash_match_for_repo_index() {
        let index_path = repo_index_path();
        if !index_path.exists() {
            return;
        }
        let disk = load_policy_registry(&index_path).unwrap();
        let packs = embedded_policy_pack_map();
        let mem = load_policy_registry_from_material(
            "governance/policies/index.toml",
            EMBEDDED_POLICY_INDEX_TXT,
            &packs,
        )
        .unwrap();
        assert_eq!(disk.policy_hash, mem.policy_hash);
    }

    #[test]
    fn snapshot_hashes_are_stable_for_deterministic_inputs() {
        let input = sample_repo_ci_policy_input().unwrap();
        let first = build_policy_engine_state_snapshot_from_embedded_workspace_policies(&input).unwrap();
        let second = build_policy_engine_state_snapshot_from_embedded_workspace_policies(&input).unwrap();
        assert_eq!(first.snapshot_content_sha256, second.snapshot_content_sha256);
        assert_eq!(
            first.evaluation.policy_input_sha256,
            second.evaluation.policy_input_sha256
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn golden_snapshot_fixture_hash_matches() {
        if !repo_index_path().exists() {
            return;
        }
        let input = sample_repo_ci_policy_input().unwrap();
        let embedded =
            build_policy_engine_state_snapshot_from_embedded_workspace_policies(&input).unwrap();
        let disk = build_policy_engine_state_snapshot_from_paths(&repo_index_path(), &input).unwrap();
        assert_eq!(
            embedded.snapshot_content_sha256,
            disk.snapshot_content_sha256,
            "embedded governance material must mirror disk-loaded registry semantics",
        );

        #[derive(Deserialize)]
        struct Fixture {
            snapshot_content_sha256: String,
        }
        let fixture_txt = include_str!("../fixtures/golden_policy_engine_state_snapshot_fixture.json");
        let parsed: Fixture = serde_json::from_str(fixture_txt).unwrap();
        assert_eq!(
            embedded.snapshot_content_sha256, parsed.snapshot_content_sha256,
            "Update fixtures/golden_policy_engine_state_snapshot_fixture.json when deterministic snapshot intentionally changes.",
        );
    }

    #[test]
    fn policy_pack_fingerprints_are_sorted_for_body_hash_inputs() {
        let input = sample_repo_ci_policy_input().unwrap();
        let snapshot =
            build_policy_engine_state_snapshot_from_embedded_workspace_policies(&input).unwrap();
        let paths: Vec<&str> = snapshot
            .policy_packs
            .iter()
            .map(|fp| fp.repo_relative_path.as_str())
            .collect();
        let mut sorted = paths.clone();
        sorted.sort();
        assert_eq!(paths, sorted);
    }
}
