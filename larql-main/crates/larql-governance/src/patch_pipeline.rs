use crate::binding::{derive_struct_bindings, BindingError, RustStructBinding};
use crate::hash::{hash_json, hash_text};
use crate::policy::{
    require_hash, require_hashes, MachineRuleProfile, PolicyDecision, PolicyError,
};
use crate::struct_minter::{verify_struct_spec_with_profile, StructMinterError, StructSpec};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PatchPipelineError {
    #[error(transparent)]
    Binding(#[from] BindingError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Policy(#[from] PolicyError),
    #[error(transparent)]
    StructMinter(#[from] StructMinterError),
    #[error("invalid patch intent: {0}")]
    InvalidIntent(&'static str),
    #[error("invalid repo-relative path: {0}")]
    InvalidPath(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PatchIntentSpec {
    pub schema_version: String,
    pub intent_hash: String,
    pub intent_kind: IntentKind,
    #[serde(default)]
    pub proposed_change_hashes: Vec<String>,
    pub evidence_hashes: Vec<String>,
    #[serde(default)]
    pub touched_paths: Vec<String>,
    #[serde(default)]
    pub struct_specs: Vec<StructSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IntentKind {
    Feature,
    Defect,
    Refactor,
    Governance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PatchEnvelope {
    pub schema_version: String,
    pub event_type: String,
    pub machine: String,
    pub intent_hash: String,
    pub intent_kind: IntentKind,
    pub status: PatchEnvelopeStatus,
    pub stages: Vec<PatchStage>,
    pub repo_facts: RepoFactSummary,
    pub new_types: Vec<String>,
    pub modified_types: Vec<String>,
    pub target_modules: Vec<String>,
    pub public_api_impact: ImpactLevel,
    pub serialization_impact: ImpactLevel,
    pub storage_impact: ImpactLevel,
    pub migration_required: GateAnswer,
    pub caller_breakage: GateAnswer,
    pub security_invariants: GateAnswer,
    pub tests_required: GateAnswer,
    pub spec_sha256: String,
    pub envelope_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PatchEnvelopeStatus {
    PatchCandidate,
    NeedsReview,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PatchStage {
    pub stage: PatchStageKind,
    pub status: StageStatus,
    pub answer: PatchAnswerToken,
    pub confidence: Confidence,
    pub evidence_hashes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PatchStageKind {
    ProseIntentExtracted,
    RepoFactsCollected,
    TypeNeedAnalyzed,
    PlacementResolved,
    PublicApiImpactResolved,
    SerializationImpactResolved,
    StorageImpactResolved,
    MigrationImpactResolved,
    CallerImpactResolved,
    SecurityInvariantResolved,
    TestCoverageResolved,
    PatchCandidateMinted,
    PolicyChecked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PatchAnswerToken {
    CandidateTypeNameAlreadyExists,
    HashBackedIntent,
    HashBackedPatchCandidate,
    NewTypesHaveTypeNeedHashes,
    No,
    NoNewTypes,
    NoPublicApi,
    NoStorageImpact,
    NotSerialized,
    Passed,
    Possible,
    PublicApi,
    RepoFactsCollected,
    RepoRelativeTargets,
    RequiresCargoCheck,
    RequiresReview,
    Serialized,
    StorageImpact,
    Yes,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum StageStatus {
    Resolved,
    NeedsReview,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoFactSummary {
    pub scanned_files: usize,
    pub rust_structs: Vec<RustStructBinding>,
    pub matching_existing_types: Vec<RustStructBinding>,
    pub test_file_hashes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ImpactLevel {
    None,
    Possible,
    Definite,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GateAnswer {
    No,
    Yes,
    Possible,
    RequiresReview,
}

pub fn plan_governed_patch(
    repo_root: &Path,
    spec: &PatchIntentSpec,
    profile: &MachineRuleProfile,
) -> Result<PatchEnvelope, PatchPipelineError> {
    validate_patch_intent(spec)?;
    let repo_facts = collect_repo_facts(repo_root, spec)?;
    let struct_policy = verify_struct_specs(spec, profile)?;
    let new_types = planned_type_names(spec);
    let modified_types = matching_type_names(&repo_facts);
    let target_modules = planned_target_modules(spec);
    let stages = vec![
        stage(
            PatchStageKind::ProseIntentExtracted,
            StageStatus::Resolved,
            PatchAnswerToken::HashBackedIntent,
            Confidence::High,
            vec![spec.intent_hash.clone()],
        ),
        stage(
            PatchStageKind::RepoFactsCollected,
            StageStatus::Resolved,
            PatchAnswerToken::RepoFactsCollected,
            Confidence::High,
            repo_facts_evidence(&repo_facts, spec),
        ),
        type_need_stage(spec, &repo_facts),
        placement_stage(spec),
        public_api_stage(spec),
        serialization_stage(spec),
        storage_stage(spec),
        migration_stage(spec),
        caller_stage(&repo_facts),
        security_stage(spec),
        test_stage(spec, &repo_facts),
        patch_candidate_stage(spec),
        stage(
            PatchStageKind::PolicyChecked,
            StageStatus::Resolved,
            policy_answer(&struct_policy),
            Confidence::High,
            spec.evidence_hashes.clone(),
        ),
    ];

    let public_api_impact = max_impact(spec.struct_specs.iter().map(|s| {
        if s.governance.classifications.public_api {
            ImpactLevel::Definite
        } else if s.item.visibility == "pub" {
            ImpactLevel::Possible
        } else {
            ImpactLevel::None
        }
    }));
    let serialization_impact = max_impact(spec.struct_specs.iter().map(|s| {
        if s.governance.classifications.serialized {
            ImpactLevel::Definite
        } else {
            ImpactLevel::None
        }
    }));
    let storage_impact = max_impact(spec.struct_specs.iter().map(|s| {
        if s.governance.classifications.persisted {
            ImpactLevel::Definite
        } else if s.governance.classifications.stable_contract {
            ImpactLevel::Possible
        } else {
            ImpactLevel::None
        }
    }));

    let status = envelope_status(&stages);
    let spec_sha256 = hash_json(spec)?;
    let mut envelope = PatchEnvelope {
        schema_version: "larql.governance.patch_envelope.v1".to_string(),
        event_type: "GovernedPatchEnvelopePlanned".to_string(),
        machine: "prose_to_governed_patch".to_string(),
        intent_hash: spec.intent_hash.clone(),
        intent_kind: spec.intent_kind.clone(),
        status,
        stages,
        repo_facts,
        new_types,
        modified_types,
        target_modules,
        public_api_impact,
        serialization_impact,
        storage_impact,
        migration_required: migration_answer(spec),
        caller_breakage: GateAnswer::RequiresReview,
        security_invariants: security_answer(spec),
        tests_required: tests_answer(spec),
        spec_sha256,
        envelope_sha256: String::new(),
    };
    envelope.envelope_sha256 = hash_json(&envelope)?;
    Ok(envelope)
}

fn validate_patch_intent(spec: &PatchIntentSpec) -> Result<(), PatchPipelineError> {
    if spec.schema_version != "larql.governance.patch_intent.v1" {
        return Err(PatchPipelineError::InvalidIntent("unsupported schema"));
    }
    require_hash("intent_hash", &spec.intent_hash)?;
    require_hashes("evidence_hashes", &spec.evidence_hashes)?;
    for hash in &spec.proposed_change_hashes {
        require_hash("proposed_change_hashes", hash)?;
    }
    for path in &spec.touched_paths {
        validate_repo_relative_path(path)?;
    }
    Ok(())
}

fn verify_struct_specs(
    spec: &PatchIntentSpec,
    profile: &MachineRuleProfile,
) -> Result<PolicyDecision, PatchPipelineError> {
    for struct_spec in &spec.struct_specs {
        verify_struct_spec_with_profile(struct_spec, profile)?;
    }
    Ok(PolicyDecision::passed(&profile.name))
}

fn collect_repo_facts(
    repo_root: &Path,
    spec: &PatchIntentSpec,
) -> Result<RepoFactSummary, PatchPipelineError> {
    let candidate_names: BTreeSet<String> = planned_type_names(spec).into_iter().collect();
    let mut rust_structs = Vec::new();
    let mut test_file_hashes = Vec::new();
    let mut scanned_files = 0usize;

    for path in rust_files(repo_root)? {
        scanned_files += 1;
        let source = fs::read_to_string(&path)?;
        let relative = path
            .strip_prefix(repo_root)
            .map_err(|_| PatchPipelineError::InvalidPath(path.display().to_string()))?;
        let module_path = module_path_for(relative);
        rust_structs.extend(derive_struct_bindings(relative, &module_path, &source)?);
        if is_test_path(relative) {
            test_file_hashes.push(hash_text(&source));
        }
    }

    let matching_existing_types = rust_structs
        .iter()
        .filter(|binding| candidate_names.contains(&binding.item_name))
        .cloned()
        .collect();

    Ok(RepoFactSummary {
        scanned_files,
        rust_structs,
        matching_existing_types,
        test_file_hashes,
    })
}

fn rust_files(root: &Path) -> Result<Vec<PathBuf>, PatchPipelineError> {
    let mut out = Vec::new();
    collect_rust_files(root, &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), PatchPipelineError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if entry.file_type()?.is_dir() {
            if matches!(name.as_ref(), ".git" | "target" | "node_modules") {
                continue;
            }
            collect_rust_files(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn module_path_for(path: &Path) -> Vec<String> {
    path.with_extension("")
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .filter(|part| !matches!(*part, "crates" | "apps" | "src" | "lib" | "main" | "tests"))
        .map(str::to_string)
        .collect()
}

fn is_test_path(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|part| part == "tests" || part.contains("test"))
    })
}

fn validate_repo_relative_path(path: &str) -> Result<(), PatchPipelineError> {
    let path = Path::new(path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return Err(PatchPipelineError::InvalidPath(path.display().to_string()));
    }
    Ok(())
}

fn planned_type_names(spec: &PatchIntentSpec) -> Vec<String> {
    spec.struct_specs
        .iter()
        .map(|struct_spec| struct_spec.item.name.clone())
        .collect()
}

fn matching_type_names(repo_facts: &RepoFactSummary) -> Vec<String> {
    repo_facts
        .matching_existing_types
        .iter()
        .map(|binding| binding.item_name.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn planned_target_modules(spec: &PatchIntentSpec) -> Vec<String> {
    spec.struct_specs
        .iter()
        .map(|struct_spec| struct_spec.item.module_path.join("::"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn repo_facts_evidence(repo_facts: &RepoFactSummary, spec: &PatchIntentSpec) -> Vec<String> {
    let mut hashes = spec.evidence_hashes.clone();
    hashes.extend(
        repo_facts
            .matching_existing_types
            .iter()
            .map(|binding| binding.rust_struct_binding_hash.clone()),
    );
    hashes.extend(repo_facts.test_file_hashes.iter().take(8).cloned());
    hashes.sort();
    hashes.dedup();
    hashes
}

fn type_need_stage(spec: &PatchIntentSpec, repo_facts: &RepoFactSummary) -> PatchStage {
    if spec.struct_specs.is_empty() {
        return stage(
            PatchStageKind::TypeNeedAnalyzed,
            StageStatus::Resolved,
            PatchAnswerToken::NoNewTypes,
            Confidence::High,
            spec.evidence_hashes.clone(),
        );
    }
    if repo_facts.matching_existing_types.is_empty() {
        let evidence = spec
            .struct_specs
            .iter()
            .map(|s| s.governance.type_need_hash.clone())
            .collect();
        stage(
            PatchStageKind::TypeNeedAnalyzed,
            StageStatus::Resolved,
            PatchAnswerToken::NewTypesHaveTypeNeedHashes,
            Confidence::High,
            evidence,
        )
    } else {
        stage(
            PatchStageKind::TypeNeedAnalyzed,
            StageStatus::NeedsReview,
            PatchAnswerToken::CandidateTypeNameAlreadyExists,
            Confidence::Medium,
            repo_facts_evidence(repo_facts, spec),
        )
    }
}

fn placement_stage(spec: &PatchIntentSpec) -> PatchStage {
    let status = if spec
        .struct_specs
        .iter()
        .all(|s| validate_repo_relative_path(&s.item.target_path).is_ok())
    {
        StageStatus::Resolved
    } else {
        StageStatus::Blocked
    };
    stage(
        PatchStageKind::PlacementResolved,
        status,
        PatchAnswerToken::RepoRelativeTargets,
        Confidence::High,
        spec.evidence_hashes.clone(),
    )
}

fn public_api_stage(spec: &PatchIntentSpec) -> PatchStage {
    let public = spec
        .struct_specs
        .iter()
        .any(|s| s.governance.classifications.public_api);
    stage(
        PatchStageKind::PublicApiImpactResolved,
        StageStatus::Resolved,
        if public {
            PatchAnswerToken::PublicApi
        } else {
            PatchAnswerToken::NoPublicApi
        },
        Confidence::High,
        spec.evidence_hashes.clone(),
    )
}

fn serialization_stage(spec: &PatchIntentSpec) -> PatchStage {
    let serialized = spec
        .struct_specs
        .iter()
        .any(|s| s.governance.classifications.serialized);
    stage(
        PatchStageKind::SerializationImpactResolved,
        StageStatus::Resolved,
        if serialized {
            PatchAnswerToken::Serialized
        } else {
            PatchAnswerToken::NotSerialized
        },
        Confidence::High,
        spec.evidence_hashes.clone(),
    )
}

fn storage_stage(spec: &PatchIntentSpec) -> PatchStage {
    let persisted = spec
        .struct_specs
        .iter()
        .any(|s| s.governance.classifications.persisted);
    stage(
        PatchStageKind::StorageImpactResolved,
        StageStatus::Resolved,
        if persisted {
            PatchAnswerToken::StorageImpact
        } else {
            PatchAnswerToken::NoStorageImpact
        },
        Confidence::High,
        spec.evidence_hashes.clone(),
    )
}

fn migration_stage(spec: &PatchIntentSpec) -> PatchStage {
    let answer = migration_answer(spec);
    stage(
        PatchStageKind::MigrationImpactResolved,
        if answer == GateAnswer::RequiresReview {
            StageStatus::NeedsReview
        } else {
            StageStatus::Resolved
        },
        gate_answer_token(&answer),
        Confidence::Medium,
        spec.evidence_hashes.clone(),
    )
}

fn caller_stage(repo_facts: &RepoFactSummary) -> PatchStage {
    stage(
        PatchStageKind::CallerImpactResolved,
        StageStatus::NeedsReview,
        PatchAnswerToken::RequiresCargoCheck,
        Confidence::Low,
        repo_facts
            .matching_existing_types
            .iter()
            .map(|binding| binding.rust_struct_binding_hash.clone())
            .collect(),
    )
}

fn security_stage(spec: &PatchIntentSpec) -> PatchStage {
    let answer = security_answer(spec);
    stage(
        PatchStageKind::SecurityInvariantResolved,
        StageStatus::Resolved,
        gate_answer_token(&answer),
        Confidence::High,
        spec.struct_specs
            .iter()
            .map(|s| s.governance.invariant_hash.clone())
            .collect(),
    )
}

fn test_stage(spec: &PatchIntentSpec, repo_facts: &RepoFactSummary) -> PatchStage {
    let answer = tests_answer(spec);
    let status = if answer == GateAnswer::Yes && repo_facts.test_file_hashes.is_empty() {
        StageStatus::NeedsReview
    } else {
        StageStatus::Resolved
    };
    stage(
        PatchStageKind::TestCoverageResolved,
        status,
        gate_answer_token(&answer),
        Confidence::Medium,
        repo_facts
            .test_file_hashes
            .iter()
            .take(8)
            .cloned()
            .collect(),
    )
}

fn patch_candidate_stage(spec: &PatchIntentSpec) -> PatchStage {
    let status = if spec.struct_specs.is_empty() && spec.proposed_change_hashes.is_empty() {
        StageStatus::NeedsReview
    } else {
        StageStatus::Resolved
    };
    let mut evidence = spec.proposed_change_hashes.clone();
    evidence.extend(spec.evidence_hashes.clone());
    stage(
        PatchStageKind::PatchCandidateMinted,
        status,
        PatchAnswerToken::HashBackedPatchCandidate,
        Confidence::Medium,
        evidence,
    )
}

fn migration_answer(spec: &PatchIntentSpec) -> GateAnswer {
    if spec
        .struct_specs
        .iter()
        .any(|s| s.governance.classifications.persisted)
    {
        GateAnswer::RequiresReview
    } else if spec
        .struct_specs
        .iter()
        .any(|s| s.governance.classifications.serialized)
    {
        GateAnswer::Possible
    } else {
        GateAnswer::No
    }
}

fn security_answer(spec: &PatchIntentSpec) -> GateAnswer {
    if spec
        .struct_specs
        .iter()
        .any(|s| s.governance.classifications.authority_bearing)
    {
        GateAnswer::Yes
    } else {
        GateAnswer::No
    }
}

fn tests_answer(spec: &PatchIntentSpec) -> GateAnswer {
    if spec.struct_specs.iter().any(|s| {
        s.governance.classifications.public_api
            || s.governance.classifications.authority_bearing
            || s.governance.classifications.serialized
            || s.governance.classifications.persisted
    }) {
        GateAnswer::Yes
    } else {
        GateAnswer::No
    }
}

fn envelope_status(stages: &[PatchStage]) -> PatchEnvelopeStatus {
    if stages
        .iter()
        .any(|stage| stage.status == StageStatus::Blocked)
    {
        PatchEnvelopeStatus::Blocked
    } else if stages
        .iter()
        .any(|stage| stage.status == StageStatus::NeedsReview)
    {
        PatchEnvelopeStatus::NeedsReview
    } else {
        PatchEnvelopeStatus::PatchCandidate
    }
}

fn max_impact(values: impl Iterator<Item = ImpactLevel>) -> ImpactLevel {
    values.max().unwrap_or(ImpactLevel::None)
}

fn stage(
    stage: PatchStageKind,
    status: StageStatus,
    answer: PatchAnswerToken,
    confidence: Confidence,
    mut evidence_hashes: Vec<String>,
) -> PatchStage {
    evidence_hashes.sort();
    evidence_hashes.dedup();
    PatchStage {
        stage,
        status,
        answer,
        confidence,
        evidence_hashes,
    }
}

fn gate_answer_token(answer: &GateAnswer) -> PatchAnswerToken {
    match answer {
        GateAnswer::No => PatchAnswerToken::No,
        GateAnswer::Yes => PatchAnswerToken::Yes,
        GateAnswer::Possible => PatchAnswerToken::Possible,
        GateAnswer::RequiresReview => PatchAnswerToken::RequiresReview,
    }
}

fn policy_answer(decision: &PolicyDecision) -> PatchAnswerToken {
    if decision.status == "passed" {
        PatchAnswerToken::Passed
    } else {
        PatchAnswerToken::RequiresReview
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::hash_text;
    use crate::policy::StructClassifications;
    use crate::struct_minter::{
        FieldSpec, GovernanceSpec, IntegrationMode, LayoutSpec, StructItemSpec,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_repo() -> PathBuf {
        let mut path = std::env::temp_dir();
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("larql-governance-test-{suffix}"));
        fs::create_dir_all(path.join("crates/example/src")).unwrap();
        fs::write(
            path.join("crates/example/src/lib.rs"),
            "pub struct Existing;\n",
        )
        .unwrap();
        fs::create_dir_all(path.join("crates/example/tests")).unwrap();
        fs::write(
            path.join("crates/example/tests/basic.rs"),
            "#[test] fn t() {}\n",
        )
        .unwrap();
        path
    }

    fn struct_spec(name: &str) -> StructSpec {
        let h = hash_text("evidence");
        StructSpec {
            schema_version: "larql.governance.struct_spec.v1".to_string(),
            item: StructItemSpec {
                name: name.to_string(),
                visibility: "pub".to_string(),
                target_path: "crates/example/src/generated/governed_types.rs".to_string(),
                module_path: vec!["example".to_string(), "generated".to_string()],
                integration_mode: IntegrationMode::PatchOnly,
            },
            derives: vec!["Debug".to_string()],
            layout: LayoutSpec {
                repr: "Rust".to_string(),
                repr_c_justification_hash: None,
            },
            fields: vec![FieldSpec {
                name: "id".to_string(),
                type_expr: "String".to_string(),
                visibility: "pub".to_string(),
            }],
            governance: GovernanceSpec {
                principle_hash: h.clone(),
                invariant_hash: h.clone(),
                type_need_hash: h.clone(),
                evidence_hashes: vec![h.clone()],
                classifications: StructClassifications {
                    internal_state: false,
                    public_api: true,
                    serialized: false,
                    persisted: false,
                    ffi_boundary: false,
                    authority_bearing: true,
                    stable_contract: true,
                },
                public_api_policy_hash: Some(h.clone()),
                test_policy_hash: Some(h.clone()),
                doc_policy_hash: Some(h),
            },
            dependency_additions: Vec::new(),
        }
    }

    #[test]
    fn plans_hash_backed_patch_envelope() {
        let repo = temp_repo();
        let h = hash_text("intent");
        let spec = PatchIntentSpec {
            schema_version: "larql.governance.patch_intent.v1".to_string(),
            intent_hash: h.clone(),
            intent_kind: IntentKind::Governance,
            proposed_change_hashes: vec![h.clone()],
            evidence_hashes: vec![h],
            touched_paths: vec!["crates/example/src/lib.rs".to_string()],
            struct_specs: vec![struct_spec("ExecutionToken")],
        };

        let envelope =
            plan_governed_patch(&repo, &spec, &MachineRuleProfile::default_starter()).unwrap();
        assert_eq!(envelope.machine, "prose_to_governed_patch");
        assert_eq!(envelope.new_types, vec!["ExecutionToken"]);
        assert!(envelope.envelope_sha256.starts_with("sha256:"));
        let _ = fs::remove_dir_all(repo);
    }

    #[test]
    fn existing_candidate_type_forces_review() {
        let repo = temp_repo();
        let h = hash_text("intent");
        let spec = PatchIntentSpec {
            schema_version: "larql.governance.patch_intent.v1".to_string(),
            intent_hash: h.clone(),
            intent_kind: IntentKind::Feature,
            proposed_change_hashes: vec![h.clone()],
            evidence_hashes: vec![h],
            touched_paths: Vec::new(),
            struct_specs: vec![struct_spec("Existing")],
        };

        let envelope =
            plan_governed_patch(&repo, &spec, &MachineRuleProfile::default_starter()).unwrap();
        assert_eq!(envelope.status, PatchEnvelopeStatus::NeedsReview);
        assert_eq!(envelope.modified_types, vec!["Existing"]);
        let _ = fs::remove_dir_all(repo);
    }
}
