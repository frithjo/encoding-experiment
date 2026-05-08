//! Phase B — policy **eval preview**: Rust evaluates embedded registry against a typed
//! [`PolicyInput`]. Hosts render `PolicyEvalPreview` leaves only.

use larql_governance::hash::hash_json;
use larql_governance::rules_engine::{PolicyEngine, PolicyFact, PolicyInput};

use crate::snapshot::{
    evaluation_snapshot, load_embedded_policy_material, registry_snapshot, EmbeddedPolicyMaterial,
    PolicyEngineEvaluationSnapshot, PolicyRegistrySnapshot, SnapshotError,
};

/// Schema marker for eval preview payloads (distinct from full state snapshot hash).
pub const POLICY_EVAL_PREVIEW_SCHEMA: &str = "larql.governance.policy_eval_preview.v1";

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Eq, PartialEq)]
pub struct PolicyEvalPreviewDraft {
    pub actor: String,
    pub action: String,
    pub state_hash: String,
    pub risk: u64,
    /// Lines -> `policy_input.target_paths` (trimmed, empty lines skipped).
    pub target_paths_raw: String,
    /// Lines -> `policy_input.evidence`.
    pub evidence_raw: String,
    #[serde(default)]
    pub facts: PreviewFactsSpecification,
}

impl Default for PolicyEvalPreviewDraft {
    fn default() -> Self {
        Self {
            actor: String::new(),
            action: String::new(),
            state_hash: String::new(),
            risk: 0,
            target_paths_raw: String::new(),
            evidence_raw: String::new(),
            facts: PreviewFactsSpecification::Fixture,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PreviewFactsSpecification {
    /// Reuse governance facts bundled with [`crate::snapshot::sample_repo_ci_policy_input`].
    Fixture,
    /// Serialized JSON array of [`PolicyFact`] — parsed only inside this crate.
    CustomJson(String),
}

impl Default for PreviewFactsSpecification {
    fn default() -> Self {
        PreviewFactsSpecification::Fixture
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Eq, PartialEq)]
pub struct PolicyEvalPreview {
    pub schema_version: String,
    pub preview_content_sha256: String,
    pub registry: PolicyRegistrySnapshot,
    pub evaluation: PolicyEngineEvaluationSnapshot,
}

#[derive(serde::Serialize)]
struct PolicyEvalPreviewBody<'a> {
    schema_version: &'a str,
    registry: &'a PolicyRegistrySnapshot,
    evaluation: &'a PolicyEngineEvaluationSnapshot,
}

fn split_nonempty_lines(block: &str) -> Vec<String> {
    block
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn compute_preview_hash(
    registry: &PolicyRegistrySnapshot,
    evaluation: &PolicyEngineEvaluationSnapshot,
) -> Result<String, serde_json::Error> {
    let body = PolicyEvalPreviewBody {
        schema_version: POLICY_EVAL_PREVIEW_SCHEMA,
        registry,
        evaluation,
    };
    hash_json(&body)
}

/// Construct [`PolicyInput`] from typed UI defaults (CI fixtures) layered with draft fields.
pub fn policy_input_from_preview_draft(draft: &PolicyEvalPreviewDraft) -> Result<PolicyInput, SnapshotError> {
    policy_input_from_preview_draft_inner(
        draft,
        crate::snapshot::sample_repo_ci_policy_input().map_err(SnapshotError::Json)?,
    )
}

/// Same as [`policy_input_from_preview_draft`] but allows callers to substitute the baseline fixture.
pub fn policy_input_from_preview_draft_with_base(
    draft: &PolicyEvalPreviewDraft,
    base_fixture: PolicyInput,
) -> Result<PolicyInput, SnapshotError> {
    policy_input_from_preview_draft_inner(draft, base_fixture)
}

fn policy_input_from_preview_draft_inner(
    draft: &PolicyEvalPreviewDraft,
    mut input: PolicyInput,
) -> Result<PolicyInput, SnapshotError> {
    input.schema_version = "larql.governance.policy_input.v1".to_string();
    input.actor = draft.actor.trim().to_string();
    input.action = draft.action.trim().to_string();
    input.state_hash = draft.state_hash.trim().to_string();
    input.risk = draft.risk;
    input.target_paths = split_nonempty_lines(&draft.target_paths_raw);
    input.evidence = split_nonempty_lines(&draft.evidence_raw);

    match &draft.facts {
        PreviewFactsSpecification::Fixture => {
            let fixture = crate::snapshot::sample_repo_ci_policy_input()
                .map_err(|err| SnapshotError::Draft(format!("facts fixture unavailable: {err}")))?;
            input.facts = fixture.facts;
        }
        PreviewFactsSpecification::CustomJson(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(SnapshotError::Draft(
                    "facts custom json payload is empty".to_string(),
                ));
            }
            let facts: Vec<PolicyFact> = serde_json::from_str(trimmed).map_err(|err| {
                SnapshotError::Draft(format!("facts json must be PolicyFact[]: {err}"))
            })?;
            input.facts = facts;
        }
    }

    Ok(input)
}

/// Initialize a [`PolicyEvalPreviewDraft`] backed by CI fixtures (`Fixture` facts mode).
pub fn preview_draft_from_ci_fixture() -> Result<PolicyEvalPreviewDraft, serde_json::Error> {
    let input = crate::snapshot::sample_repo_ci_policy_input()?;
    Ok(PolicyEvalPreviewDraft {
        actor: input.actor,
        action: input.action,
        state_hash: input.state_hash,
        risk: input.risk,
        target_paths_raw: input.target_paths.join("\n"),
        evidence_raw: input.evidence.join("\n"),
        facts: PreviewFactsSpecification::Fixture,
    })
}

/// Run embedded policy engine against supplied input (CSR/WASM safe).
pub fn build_policy_eval_preview_embedded(
    policy_input: &PolicyInput,
) -> Result<PolicyEvalPreview, SnapshotError> {
    let EmbeddedPolicyMaterial { registry, packs: _ } = load_embedded_policy_material()?;

    let excerpt = registry_snapshot(&registry);

    let engine = PolicyEngine::new(registry)?;
    let decision = engine.evaluate(policy_input);
    let evaluation = evaluation_snapshot(policy_input, &decision).map_err(SnapshotError::Json)?;

    let preview_content_sha256 = compute_preview_hash(&excerpt, &evaluation).map_err(SnapshotError::Json)?;

    Ok(PolicyEvalPreview {
        schema_version: POLICY_EVAL_PREVIEW_SCHEMA.to_string(),
        preview_content_sha256,
        registry: excerpt,
        evaluation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{
        build_policy_engine_state_snapshot_from_embedded_workspace_policies, sample_repo_ci_policy_input,
    };

    #[test]
    fn eval_preview_hashes_are_stable() {
        let input = sample_repo_ci_policy_input().unwrap();
        let first = build_policy_eval_preview_embedded(&input).unwrap();
        let second = build_policy_eval_preview_embedded(&input).unwrap();
        assert_eq!(first.preview_content_sha256, second.preview_content_sha256);
        assert_eq!(
            first.evaluation.decision_sha256,
            second.evaluation.decision_sha256
        );
    }

    #[test]
    fn eval_preview_evaluation_matches_snapshot_slice() {
        let input = sample_repo_ci_policy_input().unwrap();
        let preview = build_policy_eval_preview_embedded(&input).unwrap();
        let snapshot =
            build_policy_engine_state_snapshot_from_embedded_workspace_policies(&input).unwrap();
        assert_eq!(preview.registry, snapshot.registry);
        assert_eq!(
            preview.evaluation.policy_input_sha256,
            snapshot.evaluation.policy_input_sha256
        );
        assert_eq!(
            preview.evaluation.decision_sha256,
            snapshot.evaluation.decision_sha256
        );
    }

    #[test]
    fn golden_eval_preview_fixture_hash_matches() {
        #[derive(serde::Deserialize)]
        struct Fixture {
            preview_content_sha256: String,
        }

        let input = sample_repo_ci_policy_input().unwrap();
        let preview = build_policy_eval_preview_embedded(&input).unwrap();
        let fixture_txt = include_str!("../fixtures/golden_policy_eval_preview_fixture.json");
        let parsed: Fixture = serde_json::from_str(fixture_txt).unwrap();
        assert_eq!(
            preview.preview_content_sha256, parsed.preview_content_sha256,
            "Update fixtures/golden_policy_eval_preview_fixture.json when embedded preview materially changes.",
        );
    }

    #[test]
    fn draft_fixture_mode_preserves_facts() {
        let draft = preview_draft_from_ci_fixture().unwrap();
        let base = sample_repo_ci_policy_input().unwrap();
        let merged = policy_input_from_preview_draft_with_base(&draft, base.clone()).unwrap();
        assert_eq!(merged.facts, base.facts);
    }
}
