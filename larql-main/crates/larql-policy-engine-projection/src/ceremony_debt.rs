//! Phase C — ceremony/debt projections: Rust parses embedded ceremony definitions, budgets, and decision
//! surface registry, then emits [`CeremonyDebtStateSnapshot`]. Projection hosts render leaves only.

use std::collections::BTreeMap;

use larql_governance::decision_surface::{
    parse_ceremony_decision_budgets_from_str, parse_decision_surface_registry_from_str,
    CeremonyDecisionBudgets, DecisionStatus, DecisionSurfaceError, DecisionSurfaceItem,
    DecisionSurfaceRegistry,
};
use larql_governance::hash::{hash_bytes, hash_json};
use serde::{Deserialize, Serialize};

use crate::snapshot::PolicySourceFingerprint;

pub const CEREMONY_DEBT_STATE_SNAPSHOT_SCHEMA: &str =
    "larql.governance.ceremony_debt_state_snapshot.v1";

const EMBEDDED_CEREMONY_DEBT: &[(&str, &str)] = &[
    (
        "governance/ceremonies/budgets.toml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../governance/ceremonies/budgets.toml"
        )),
    ),
    (
        "governance/ceremonies/capability_minting.toml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../governance/ceremonies/capability_minting.toml"
        )),
    ),
    (
        "governance/ceremonies/machine_creation.toml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../governance/ceremonies/machine_creation.toml"
        )),
    ),
    (
        "governance/ceremonies/policy_update.toml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../governance/ceremonies/policy_update.toml"
        )),
    ),
    (
        "governance/ceremonies/policy_weakening.toml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../governance/ceremonies/policy_weakening.toml"
        )),
    ),
    (
        "governance/decision_surface/registry.toml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../governance/decision_surface/registry.toml"
        )),
    ),
];

#[derive(Debug, thiserror::Error)]
pub enum CeremonyDebtSnapshotError {
    #[error(transparent)]
    DecisionSurface(#[from] DecisionSurfaceError),
    #[error(transparent)]
    Toml(#[from] toml::de::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct CeremonyDefinitionLeaf {
    pub repo_relative_path: String,
    pub content_sha256: String,
    pub schema_version: String,
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_open_llm_decisions: Option<u64>,
    pub phases: Vec<String>,
    pub required_evidence: Vec<String>,
    pub required_artifacts: Vec<String>,
    pub authority: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct CeremonyDebtStateSnapshot {
    pub schema_version: String,
    pub snapshot_content_sha256: String,
    pub sources: Vec<PolicySourceFingerprint>,
    pub ceremony_budgets: CeremonyDecisionBudgets,
    pub decision_surface_registry: DecisionSurfaceRegistry,
    pub ceremony_definitions: Vec<CeremonyDefinitionLeaf>,
    pub debt_decisions: Vec<DecisionSurfaceItem>,
}

#[derive(Deserialize)]
struct CeremonyToml {
    schema_version: String,
    id: String,
    name: String,
    #[serde(default)]
    max_open_llm_decisions: Option<u64>,
    phases: Vec<String>,
    #[serde(default)]
    required_evidence: Vec<String>,
    #[serde(default)]
    required_artifacts: Vec<String>,
    #[serde(default)]
    authority: Option<toml::value::Table>,
}

#[derive(Serialize)]
struct CeremonyDebtStateSnapshotBody<'a> {
    schema_version: &'a str,
    sources: &'a [PolicySourceFingerprint],
    ceremony_budgets: &'a CeremonyDecisionBudgets,
    decision_surface_registry: &'a DecisionSurfaceRegistry,
    ceremony_definitions: &'a [CeremonyDefinitionLeaf],
    debt_decisions: &'a [DecisionSurfaceItem],
}

fn fingerprint_for_repo_path(repo_relative_path: &str, text: &str) -> PolicySourceFingerprint {
    PolicySourceFingerprint {
        repo_relative_path: repo_relative_path.to_string(),
        content_sha256: hash_bytes(text.as_bytes()),
    }
}

fn flatten_authority(table: &toml::value::Table) -> BTreeMap<String, String> {
    table
        .iter()
        .map(|(key, value)| (key.clone(), format!("{}", value)))
        .collect()
}

fn parse_ceremony_definition_leaf(
    repo_relative_path: &str,
    text: &str,
) -> Result<CeremonyDefinitionLeaf, CeremonyDebtSnapshotError> {
    let parsed: CeremonyToml = toml::from_str(text)?;
    let authority = parsed
        .authority
        .as_ref()
        .map(flatten_authority)
        .unwrap_or_default();
    Ok(CeremonyDefinitionLeaf {
        repo_relative_path: repo_relative_path.to_string(),
        content_sha256: hash_bytes(text.as_bytes()),
        schema_version: parsed.schema_version,
        id: parsed.id,
        name: parsed.name,
        max_open_llm_decisions: parsed.max_open_llm_decisions,
        phases: parsed.phases,
        required_evidence: parsed.required_evidence,
        required_artifacts: parsed.required_artifacts,
        authority,
    })
}

fn debt_projection_rows(registry: &DecisionSurfaceRegistry) -> Vec<DecisionSurfaceItem> {
    let mut rows: Vec<DecisionSurfaceItem> = registry
        .decision
        .iter()
        .filter(|item| matches!(item.status, DecisionStatus::Open | DecisionStatus::Assisted))
        .cloned()
        .collect();
    rows.sort_by(|left, right| left.id.cmp(&right.id));
    rows
}

fn compute_snapshot_hash(
    sources: &[PolicySourceFingerprint],
    ceremony_budgets: &CeremonyDecisionBudgets,
    decision_surface_registry: &DecisionSurfaceRegistry,
    ceremony_definitions: &[CeremonyDefinitionLeaf],
    debt_decisions: &[DecisionSurfaceItem],
) -> Result<String, serde_json::Error> {
    let body = CeremonyDebtStateSnapshotBody {
        schema_version: CEREMONY_DEBT_STATE_SNAPSHOT_SCHEMA,
        sources,
        ceremony_budgets,
        decision_surface_registry,
        ceremony_definitions,
        debt_decisions,
    };
    hash_json(&body)
}

/// Build a [`CeremonyDebtStateSnapshot`] from embedded governance ceremony + decision-surface material
/// (`include_str!`, CSR/WASM safe).
pub fn build_ceremony_debt_state_snapshot_embedded(
) -> Result<CeremonyDebtStateSnapshot, CeremonyDebtSnapshotError> {
    manifest_paths_lexicographic(EMBEDDED_CEREMONY_DEBT)?;

    let mut budgets: Option<CeremonyDecisionBudgets> = None;
    let mut registry: Option<DecisionSurfaceRegistry> = None;
    let mut ceremony_definitions = Vec::<CeremonyDefinitionLeaf>::new();

    for &(path, text) in EMBEDDED_CEREMONY_DEBT {
        if path == "governance/ceremonies/budgets.toml" {
            budgets = Some(parse_ceremony_decision_budgets_from_str(text, path)?);
        } else if path == "governance/decision_surface/registry.toml" {
            registry = Some(parse_decision_surface_registry_from_str(text, path)?);
        } else if path.starts_with("governance/ceremonies/") && path.ends_with(".toml") {
            ceremony_definitions.push(parse_ceremony_definition_leaf(path, text)?);
        } else {
            return Err(CeremonyDebtSnapshotError::Invalid(format!(
                "unexpected embedded governance path for ceremony snapshot: {path}"
            )));
        }
    }

    ceremony_definitions.sort_by(|left, right| left.id.cmp(&right.id));

    let ceremony_budgets = budgets.ok_or_else(|| {
        CeremonyDebtSnapshotError::Invalid("embedded ceremony budgets.toml missing".into())
    })?;
    let decision_surface_registry = registry.ok_or_else(|| {
        CeremonyDebtSnapshotError::Invalid("embedded decision_surface/registry.toml missing".into())
    })?;

    let debt_decisions = debt_projection_rows(&decision_surface_registry);

    let mut sources: Vec<PolicySourceFingerprint> = EMBEDDED_CEREMONY_DEBT
        .iter()
        .map(|(path, text)| fingerprint_for_repo_path(path, text))
        .collect();
    sources.sort_by(|left, right| left.repo_relative_path.cmp(&right.repo_relative_path));

    let snapshot_content_sha256 = compute_snapshot_hash(
        &sources,
        &ceremony_budgets,
        &decision_surface_registry,
        &ceremony_definitions,
        &debt_decisions,
    )?;

    Ok(CeremonyDebtStateSnapshot {
        schema_version: CEREMONY_DEBT_STATE_SNAPSHOT_SCHEMA.to_string(),
        snapshot_content_sha256,
        sources,
        ceremony_budgets,
        decision_surface_registry,
        ceremony_definitions,
        debt_decisions,
    })
}

fn manifest_paths_lexicographic(
    embedded: &[(&str, &str)],
) -> Result<(), CeremonyDebtSnapshotError> {
    let declared: Vec<&str> = embedded.iter().map(|pair| pair.0).collect();
    let mut sorted = declared.clone();
    sorted.sort_unstable();
    if declared != sorted {
        return Err(CeremonyDebtSnapshotError::Invalid(format!(
            "EMBEDDED_CEREMONY_DEBT must list repo_relative_path keys in lexicographic order (declared {declared:?}, sorted {sorted:?})",
        )));
    }
    Ok(())
}

/// Expected slash-normalized sources embedded by [`build_ceremony_debt_state_snapshot_embedded`].
pub fn embedded_ceremony_debt_source_paths_expectation() -> Vec<String> {
    EMBEDDED_CEREMONY_DEBT
        .iter()
        .map(|(path, _)| (*path).to_string())
        .collect()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
fn workspace_governance_repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../governance")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ceremony_debt_snapshot_hashes_are_stable() {
        let first = build_ceremony_debt_state_snapshot_embedded().unwrap();
        let second = build_ceremony_debt_state_snapshot_embedded().unwrap();
        assert_eq!(
            first.snapshot_content_sha256,
            second.snapshot_content_sha256
        );
    }

    #[test]
    fn debt_rows_are_filtered_by_status_leaf_only_in_rust() {
        let snap = build_ceremony_debt_state_snapshot_embedded().unwrap();
        for item in &snap.decision_surface_registry.decision {
            if matches!(item.status, DecisionStatus::Open | DecisionStatus::Assisted) {
                assert!(snap.debt_decisions.iter().any(|row| row.id == item.id));
            }
        }
        for row in &snap.debt_decisions {
            assert!(matches!(
                row.status,
                DecisionStatus::Open | DecisionStatus::Assisted
            ));
        }
    }

    #[test]
    fn golden_ceremony_debt_fixture_hash_matches() {
        #[derive(Deserialize)]
        struct Fixture {
            snapshot_content_sha256: String,
        }

        let snapshot = build_ceremony_debt_state_snapshot_embedded().unwrap();
        let fixture_txt = include_str!("../fixtures/golden_ceremony_debt_fixture.json");
        let parsed: Fixture = serde_json::from_str(fixture_txt).unwrap();
        assert_eq!(
            snapshot.snapshot_content_sha256, parsed.snapshot_content_sha256,
            "Update fixtures/golden_ceremony_debt_fixture.json when embedded ceremony/decision_surface material materially changes.",
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn embedded_ceremony_manifest_tracks_workspace_ceremony_dir() {
        use std::fs;

        let ceremony_dir = workspace_governance_repo_root().join("ceremonies");
        let mut on_disk = fs::read_dir(&ceremony_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".toml"))
            .collect::<Vec<_>>();
        on_disk.sort();

        let mut expected = embedded_ceremony_debt_source_paths_expectation()
            .into_iter()
            .filter(|path| path.starts_with("governance/ceremonies/"))
            .map(|path| path.split('/').next_back().unwrap().to_string())
            .collect::<Vec<_>>();
        expected.sort();

        assert_eq!(
            expected, on_disk,
            "`EMBEDDED_CEREMONY_DEBT` must cover every tracked ceremony TOML in governance/ceremonies",
        );

        assert!(
            expected.iter().any(|name| name == "budgets.toml"),
            "`budgets.toml` must remain embedded for ceremony_budgets projections"
        );
    }
}
