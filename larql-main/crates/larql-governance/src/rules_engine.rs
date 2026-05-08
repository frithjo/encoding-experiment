use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum PolicyLoadError {
    #[error("policy index has no parent directory: {0}")]
    MissingPolicyRoot(String),
    #[error("policy file IO failed for {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("policy TOML parse failed for {path}: {source}")]
    Toml {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("policy TOML serialization failed: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("policy registry invalid: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyRegistry {
    pub schema_version: String,
    pub active_policy_set: PolicySet,
    pub policy_hash: String,
    pub mode: PolicyMode,
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicySet {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub includes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyMode {
    pub unknown_rule: UnknownRuleMode,
    pub unknown_fact: UnknownFactMode,
    pub conflict_resolution: ConflictResolutionMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnknownRuleMode {
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnknownFactMode {
    Warn,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolutionMode {
    MostRestrictive,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyRule {
    pub id: String,
    pub class: PolicyClass,
    pub severity: PolicySeverity,
    pub description: String,
    #[serde(default)]
    pub when: ConditionBlock,
    #[serde(default)]
    pub require: ConditionBlock,
    pub decision: RuleDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyClass {
    CiPolicy,
    RuntimePolicy,
    CapabilityPolicy,
    CeremonyPolicy,
    ActivationPolicy,
    ArtifactPolicy,
    TrainingPolicy,
    ConstitutionalPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicySeverity {
    Info,
    Warn,
    ReviewRequired,
    Deny,
    Fatal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    Allow,
    AllowWithWarnings,
    RequireReview,
    Deny,
    Fatal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuleDecision {
    pub on_missing: DecisionKind,
    pub message: String,
    #[serde(default)]
    pub required_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum FactValue {
    Bool(bool),
    Integer(i64),
    Text(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyFact {
    pub fact_type: String,
    pub value: FactValue,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub state_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyInput {
    pub schema_version: String,
    pub state_hash: String,
    pub actor: String,
    pub action: String,
    #[serde(default)]
    pub target_paths: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub risk: u64,
    #[serde(default)]
    pub facts: Vec<PolicyFact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConditionBlock {
    #[serde(default)]
    pub all: Vec<Condition>,
    #[serde(default)]
    pub any: Vec<Condition>,
    #[serde(default, rename = "not")]
    pub not_conditions: Vec<Condition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Condition {
    #[serde(default)]
    pub fact: Option<String>,
    #[serde(default)]
    pub equals: Option<FactValue>,
    #[serde(default)]
    pub present: Option<bool>,
    #[serde(default)]
    pub evidence: Option<String>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub path_matches: Option<String>,
    #[serde(default)]
    pub risk_gte: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyEngineDecision {
    pub schema_version: String,
    pub active_policy_set: String,
    pub policy_hash: String,
    pub input_state_hash: String,
    pub decision: DecisionKind,
    #[serde(default)]
    pub matched_rules: Vec<String>,
    #[serde(default)]
    pub required_actions: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub findings: Vec<PolicyEvaluationFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyEvaluationFinding {
    pub rule_id: String,
    pub severity: PolicySeverity,
    pub decision: DecisionKind,
    pub message: String,
    #[serde(default)]
    pub missing_facts: Vec<String>,
    #[serde(default)]
    pub missing_evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuleProposal {
    pub schema_version: String,
    pub policy_update_type: PolicyUpdateType,
    pub proposed_rule: PolicyRule,
    pub proposer: String,
    #[serde(default)]
    pub rationale: String,
    #[serde(default)]
    pub approval_capability: Option<String>,
    #[serde(default)]
    pub examples: Vec<PolicyRuleExample>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyUpdateType {
    NewRule,
    RuleTightening,
    RuleRelaxation,
    Clarification,
    ScopeChange,
    SeverityChange,
    ConstitutionalChange,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProsePolicyConcern {
    pub event_type: String,
    pub ceremony_id: String,
    pub actor: String,
    pub prior_policy_hash: String,
    pub prior_state_hash: String,
    pub prose: String,
    pub target_area: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyIntentExtraction {
    pub event_type: String,
    pub ceremony_id: String,
    pub intent: PolicyIntent,
    #[serde(default)]
    pub uncertainties: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyIntent {
    pub policy_class: PolicyClass,
    pub action: String,
    #[serde(default)]
    pub required_evidence: Vec<String>,
    pub default_decision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyRuleExample {
    pub kind: PolicyRuleExampleKind,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PolicyRuleExampleKind {
    Allow,
    Deny,
    ReviewRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyConflictAnalysis {
    pub event_type: String,
    pub candidate_rule_id: String,
    #[serde(default)]
    pub conflicts: Vec<PolicyConflict>,
    #[serde(default)]
    pub warnings: Vec<PolicyConflict>,
    pub recommendation: ConflictRecommendation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyConflict {
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictRecommendation {
    Allow,
    AllowWithReview,
    Block,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyCapability {
    pub capability_type: String,
    #[serde(default)]
    pub allowed_policy_files: Vec<String>,
    #[serde(default)]
    pub allowed_rule_ids: Vec<String>,
    pub prior_policy_hash: String,
    pub prior_state_hash: String,
    pub expires_at: String,
    #[serde(default)]
    pub required_checks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyUpdateCeremonyTrace {
    pub schema_version: String,
    pub ceremony_id: String,
    #[serde(default)]
    pub policy_update_type: Option<PolicyUpdateType>,
    #[serde(default)]
    pub events: Vec<PolicyUpdateCeremonyEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyUpdateCeremonyEvent {
    pub event_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyUpdateCeremonyReport {
    pub schema_version: String,
    pub ceremony_id: String,
    pub passed: bool,
    #[serde(default)]
    pub missing_phases: Vec<String>,
    #[serde(default)]
    pub out_of_order_phases: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyWeakeningEvidence {
    pub schema_version: String,
    pub ceremony_id: String,
    pub authority_reduced: String,
    pub too_strict_rationale_hash: String,
    #[serde(default)]
    pub false_positive_example_hashes: Vec<String>,
    pub hostile_path_regression_test_hash: String,
    pub rollback_path_hash: String,
    pub human_approval_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyApproval {
    pub schema_version: String,
    pub event_type: String,
    pub ceremony_id: String,
    pub approver: String,
    pub approved_rule_id: String,
    pub prior_policy_hash: String,
    pub prior_state_hash: String,
    pub conflict_analysis_passed: bool,
    pub shadow_evaluation_completed: bool,
    pub policy_tests_passed: bool,
    pub replay_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyApplyRequest {
    pub proposal: RuleProposal,
    pub approval: PolicyApproval,
    pub capability: PolicyCapability,
    pub target_policy_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyApplyReceipt {
    pub schema_version: String,
    pub event_type: String,
    pub rule_id: String,
    pub policy_file: String,
    pub prior_policy_hash: String,
    pub new_policy_hash: String,
    pub prior_state_hash: String,
    pub applied_by_capability: String,
    pub required_checks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyTestSuite {
    #[serde(default)]
    pub case: Vec<PolicyTestCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyTestCase {
    pub name: String,
    pub action: String,
    #[serde(default)]
    pub facts: BTreeMap<String, FactValue>,
    #[serde(default)]
    pub evidence: BTreeMap<String, bool>,
    pub expect: PolicyTestExpectation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyTestExpectation {
    pub decision: DecisionKind,
    #[serde(default)]
    pub rule_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyTestReport {
    pub schema_version: String,
    pub active_policy_set: String,
    pub policy_hash: String,
    #[serde(default)]
    pub cases: Vec<PolicyTestCaseReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyTestCaseReport {
    pub name: String,
    pub passed: bool,
    pub expected_decision: DecisionKind,
    pub actual_decision: DecisionKind,
    #[serde(default)]
    pub expected_rule_id: Option<String>,
    #[serde(default)]
    pub matched_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShadowEvalResult {
    pub schema_version: String,
    pub active_policy_set: String,
    pub policy_hash: String,
    pub baseline_decision: PolicyEngineDecision,
    pub proposal_decision: PolicyEngineDecision,
    pub changed_decision: bool,
    pub introduced_required_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyShadowEvalReport {
    pub schema_version: String,
    pub active_policy_set: String,
    pub policy_hash: String,
    pub candidate_rule_id: String,
    pub results: PolicyShadowEvalSummary,
    pub recommendation: ShadowEvalRecommendation,
    #[serde(default)]
    pub cases: Vec<PolicyShadowEvalCaseReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PolicyShadowEvalSummary {
    pub would_block_previously_accepted: u64,
    pub would_catch_known_bad_cases: u64,
    pub would_change_review_to_deny: u64,
    pub ambiguous_cases: u64,
    pub changed_cases: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyShadowEvalCaseReport {
    pub name: String,
    pub expected_decision: DecisionKind,
    pub baseline_decision: DecisionKind,
    pub proposal_decision: DecisionKind,
    pub changed_decision: bool,
    #[serde(default)]
    pub matched_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShadowEvalRecommendation {
    Accept,
    AcceptWithWarning,
    RequiresHumanReview,
    Block,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PolicyIndexFile {
    pub schema_version: String,
    pub active_policy_set: PolicySet,
    pub mode: PolicyMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PolicyPackFile {
    pub schema_version: String,
    #[serde(default)]
    pub rule: Vec<PolicyRule>,
}

#[derive(Debug, Clone)]
pub(crate) enum RuleDispatchClass {
    Global,
    ActionBucket(String),
}

/// Conservative eligibility classifier for [`PolicyEngine`] (exact `when.all` conjunct semantics).
///
/// Bucketing is **sound only if** [`Condition::action`] is enforced with **`==`** against
/// [`PolicyInput::action`] (exact string equality). Wildcards/prefix/aliases would require widening
/// or rebuilding dispatch.
///
/// OR/NOT, zero or multiple distinct action literals → global bucket.
pub(crate) fn compile_rule_dispatch_class(when: &ConditionBlock) -> RuleDispatchClass {
    if !when.any.is_empty() || !when.not_conditions.is_empty() {
        return RuleDispatchClass::Global;
    }

    let mut actions = BTreeSet::new();
    for condition in &when.all {
        if let Some(action) = condition.action.clone() {
            actions.insert(action);
        }
    }

    match actions.len() {
        1 => {
            RuleDispatchClass::ActionBucket(actions.iter().next().expect("length checked").clone())
        }
        _ => RuleDispatchClass::Global,
    }
}

/// Compiled **action eligibility** over rules (derived cache — not serialized governance state).
#[derive(Debug, Clone)]
struct CompiledActionDispatch {
    global_indices: Vec<usize>,
    by_action: HashMap<String, Vec<usize>>,
}

impl CompiledActionDispatch {
    fn compile(rules: &[PolicyRule]) -> Self {
        let mut global_indices = Vec::new();
        let mut by_action: HashMap<String, Vec<usize>> = HashMap::new();

        for (idx, rule) in rules.iter().enumerate() {
            match compile_rule_dispatch_class(&rule.when) {
                RuleDispatchClass::Global => global_indices.push(idx),
                RuleDispatchClass::ActionBucket(action) => {
                    by_action.entry(action).or_default().push(idx)
                }
            }
        }

        global_indices.sort_unstable();
        for indices in by_action.values_mut() {
            indices.sort_unstable();
            indices.dedup();
        }

        Self {
            global_indices,
            by_action,
        }
    }

    fn merged_eligible_index_list(&self, input_action: &str) -> Vec<usize> {
        let bucket = self
            .by_action
            .get(input_action)
            .map(|indices| indices.as_slice())
            .unwrap_or(&[]);
        merge_sorted_unique(&self.global_indices, bucket)
    }
}

/// Merge two ascending, dedupe-stable slices without scanning full rule cardinality.
pub(crate) fn merge_sorted_unique(lhs: &[usize], rhs: &[usize]) -> Vec<usize> {
    let mut out = Vec::with_capacity(lhs.len() + rhs.len());
    let mut i = 0usize;
    let mut j = 0usize;
    while i < lhs.len() && j < rhs.len() {
        match lhs[i].cmp(&rhs[j]) {
            Ordering::Less => {
                out.push(lhs[i]);
                i += 1;
            }
            Ordering::Greater => {
                out.push(rhs[j]);
                j += 1;
            }
            Ordering::Equal => {
                out.push(lhs[i]);
                i += 1;
                j += 1;
            }
        }
    }
    out.extend_from_slice(&lhs[i..]);
    out.extend_from_slice(&rhs[j..]);
    out
}

enum EligibilityPlan<'a> {
    Merged(&'a [usize]),
    // Only constructed from `evaluate_full_scan_for_test`; unused in production lib builds.
    #[allow(dead_code)]
    FullScan,
}

pub struct PolicyEngine {
    registry: PolicyRegistry,
    dispatch: CompiledActionDispatch,
}

pub fn load_policy_registry(index_path: &Path) -> Result<PolicyRegistry, PolicyLoadError> {
    let root = index_path
        .parent()
        .ok_or_else(|| PolicyLoadError::MissingPolicyRoot(index_path.display().to_string()))?;
    let index_text = read_policy_text(index_path)?;
    let index: PolicyIndexFile =
        toml::from_str(&index_text).map_err(|source| PolicyLoadError::Toml {
            path: index_path.display().to_string(),
            source,
        })?;
    if index.schema_version != "larql.governance.policy_index.v1" {
        return Err(PolicyLoadError::Invalid(
            "unsupported policy index schema".to_string(),
        ));
    }
    if index.active_policy_set.id.trim().is_empty() {
        return Err(PolicyLoadError::Invalid(
            "active policy set requires id".to_string(),
        ));
    }

    let mut rules = Vec::new();
    let mut hash_material = String::new();
    hash_material.push_str(&index_text);
    for include in &index.active_policy_set.includes {
        validate_policy_include_segment(include)?;
        let include_path = normalize_policy_include(root, include)?;
        let text = read_policy_text(&include_path)?;
        let pack: PolicyPackFile =
            toml::from_str(&text).map_err(|source| PolicyLoadError::Toml {
                path: include_path.display().to_string(),
                source,
            })?;
        if pack.schema_version != "larql.governance.policy_pack.v1" {
            return Err(PolicyLoadError::Invalid(format!(
                "unsupported policy pack schema: {}",
                include_path.display()
            )));
        }
        hash_material.push_str(include);
        hash_material.push_str(&text);
        rules.extend(pack.rule);
    }

    validate_rule_ids(&rules)?;
    let policy_hash = crate::hash::hash_text(&hash_material);
    Ok(PolicyRegistry {
        schema_version: "larql.governance.policy_registry.v1".to_string(),
        active_policy_set: index.active_policy_set,
        policy_hash,
        mode: index.mode,
        rules,
    })
}

fn validate_policy_include_segment(include: &str) -> Result<(), PolicyLoadError> {
    if include.trim().is_empty() {
        return Err(PolicyLoadError::Invalid(
            "policy include path must be non-empty".to_string(),
        ));
    }
    if include.contains("..") {
        return Err(PolicyLoadError::Invalid(format!(
            "policy include must not contain '..': {include}"
        )));
    }
    if Path::new(include).is_absolute() {
        return Err(PolicyLoadError::Invalid(format!(
            "policy include must be relative: {include}"
        )));
    }
    Ok(())
}

fn target_policy_file_is_active_include(registry: &PolicyRegistry, target: &str) -> bool {
    active_policy_include_for_target(registry, target).is_some()
}

fn active_policy_include_for_target<'a>(
    registry: &'a PolicyRegistry,
    target: &str,
) -> Option<&'a str> {
    let Some(target) = normalize_policy_logical_path(target) else {
        return None;
    };
    registry
        .active_policy_set
        .includes
        .iter()
        .find(|include| {
            normalize_policy_logical_path(include).is_some_and(|normalized| {
                target == normalized || target == format!("governance/policies/{normalized}")
            })
        })
        .map(String::as_str)
}

fn normalize_policy_logical_path(path: &str) -> Option<String> {
    let path = path.trim();
    if path.is_empty() || Path::new(path).is_absolute() {
        return None;
    }
    let mut components = Vec::new();
    for component in Path::new(path).components() {
        match component {
            std::path::Component::Normal(value) => components.push(value.to_str()?),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => return None,
        }
    }
    if components.is_empty() {
        None
    } else {
        Some(components.join("/"))
    }
}

/// Load a policy registry from in-memory index and pack contents. Keys in `packs` must be the
/// **exact** `active_policy_set.includes` strings from the index (same as on disk under the index
/// parent directory), not basenames alone — so `packs/security/ci.toml` and `ci.toml` do not
/// collide.
///
/// `hash_material` matches [`load_policy_registry`] when file bytes equal the provided strings.
pub fn load_policy_registry_from_material(
    index_logical_path: &str,
    index_text: &str,
    packs: &BTreeMap<String, String>,
) -> Result<PolicyRegistry, PolicyLoadError> {
    let index: PolicyIndexFile =
        toml::from_str(index_text).map_err(|source| PolicyLoadError::Toml {
            path: index_logical_path.to_string(),
            source,
        })?;
    if index.schema_version != "larql.governance.policy_index.v1" {
        return Err(PolicyLoadError::Invalid(
            "unsupported policy index schema".to_string(),
        ));
    }
    if index.active_policy_set.id.trim().is_empty() {
        return Err(PolicyLoadError::Invalid(
            "active policy set requires id".to_string(),
        ));
    }

    let mut rules = Vec::new();
    let mut hash_material = String::new();
    hash_material.push_str(index_text);
    for include in &index.active_policy_set.includes {
        validate_policy_include_segment(include)?;
        let text = packs.get(include).ok_or_else(|| {
            PolicyLoadError::Invalid(format!(
                "missing policy pack contents for include {include:?} (index {index_logical_path})"
            ))
        })?;
        let pack_log_path = format!("{index_logical_path}::{include}");
        let pack: PolicyPackFile =
            toml::from_str(text).map_err(|source| PolicyLoadError::Toml {
                path: pack_log_path,
                source,
            })?;
        if pack.schema_version != "larql.governance.policy_pack.v1" {
            return Err(PolicyLoadError::Invalid(format!(
                "unsupported policy pack schema for include {include}"
            )));
        }
        hash_material.push_str(include);
        hash_material.push_str(text);
        rules.extend(pack.rule);
    }

    validate_rule_ids(&rules)?;
    let policy_hash = crate::hash::hash_text(&hash_material);
    Ok(PolicyRegistry {
        schema_version: "larql.governance.policy_registry.v1".to_string(),
        active_policy_set: index.active_policy_set,
        policy_hash,
        mode: index.mode,
        rules,
    })
}

impl PolicyEngine {
    pub fn new(registry: PolicyRegistry) -> Result<Self, PolicyLoadError> {
        validate_rule_ids(&registry.rules)?;
        let dispatch = CompiledActionDispatch::compile(&registry.rules);
        Ok(Self { registry, dispatch })
    }

    pub fn registry(&self) -> &PolicyRegistry {
        &self.registry
    }

    pub fn evaluate(&self, input: &PolicyInput) -> PolicyEngineDecision {
        let eligible = self.dispatch.merged_eligible_index_list(&input.action);
        self.evaluate_inner(input, EligibilityPlan::Merged(&eligible))
    }

    #[cfg(test)]
    pub(crate) fn evaluate_full_scan_for_test(&self, input: &PolicyInput) -> PolicyEngineDecision {
        self.evaluate_inner(input, EligibilityPlan::FullScan)
    }

    #[cfg(test)]
    pub(crate) fn merged_eligible_indices_for_action(&self, input_action: &str) -> Vec<usize> {
        self.dispatch.merged_eligible_index_list(input_action)
    }

    fn evaluate_inner(
        &self,
        input: &PolicyInput,
        plan: EligibilityPlan<'_>,
    ) -> PolicyEngineDecision {
        let facts = fact_map(&input.facts);
        let mut decision = DecisionKind::Allow;
        let mut matched_rules = Vec::new();
        let mut required_actions = BTreeSet::new();
        let mut evidence = BTreeSet::new();
        let mut findings = Vec::new();

        match plan {
            EligibilityPlan::FullScan => {
                for rule in &self.registry.rules {
                    Self::evaluate_rule_at_index(
                        rule,
                        input,
                        &facts,
                        &mut decision,
                        &mut matched_rules,
                        &mut required_actions,
                        &mut evidence,
                        &mut findings,
                    );
                }
            }
            EligibilityPlan::Merged(indices) => {
                for &idx in indices {
                    let Some(rule) = self.registry.rules.get(idx) else {
                        continue;
                    };
                    Self::evaluate_rule_at_index(
                        rule,
                        input,
                        &facts,
                        &mut decision,
                        &mut matched_rules,
                        &mut required_actions,
                        &mut evidence,
                        &mut findings,
                    );
                }
            }
        }

        if matched_rules.is_empty() {
            match self.registry.mode.unknown_rule {
                UnknownRuleMode::Deny => {
                    decision = DecisionKind::Deny;
                    let required_action =
                        format!("add active policy rule covering {}", input.action);
                    required_actions.insert(required_action);
                    findings.push(PolicyEvaluationFinding {
                        rule_id: "policy.unknown_action.deny".to_string(),
                        severity: PolicySeverity::Deny,
                        decision: DecisionKind::Deny,
                        message: format!(
                            "No active policy rule matched action '{}'.",
                            input.action
                        ),
                        missing_facts: Vec::new(),
                        missing_evidence: Vec::new(),
                    });
                }
            }
        }

        PolicyEngineDecision {
            schema_version: "larql.governance.policy_decision.v1".to_string(),
            active_policy_set: format!(
                "{}@{}",
                self.registry.active_policy_set.id, self.registry.active_policy_set.version
            ),
            policy_hash: self.registry.policy_hash.clone(),
            input_state_hash: input.state_hash.clone(),
            decision,
            matched_rules,
            required_actions: required_actions.into_iter().collect(),
            evidence: evidence.into_iter().collect(),
            findings,
        }
    }

    fn evaluate_rule_at_index(
        rule: &PolicyRule,
        input: &PolicyInput,
        facts: &HashMap<String, &PolicyFact>,
        decision: &mut DecisionKind,
        matched_rules: &mut Vec<String>,
        required_actions: &mut BTreeSet<String>,
        evidence: &mut BTreeSet<String>,
        findings: &mut Vec<PolicyEvaluationFinding>,
    ) {
        let when = evaluate_block(&rule.when, input, facts);
        if !when.matched {
            return;
        }
        matched_rules.push(rule.id.clone());
        let require = evaluate_block(&rule.require, input, facts);
        if require.matched {
            if rule.severity == PolicySeverity::Warn {
                *decision = most_restrictive(decision.clone(), DecisionKind::AllowWithWarnings);
            }
            for item in require.evidence {
                evidence.insert(item);
            }
            return;
        }

        *decision = most_restrictive(decision.clone(), rule.decision.on_missing.clone());
        for action in &rule.decision.required_actions {
            required_actions.insert(action.clone());
        }
        for item in when.evidence.into_iter().chain(require.evidence) {
            evidence.insert(item);
        }
        findings.push(PolicyEvaluationFinding {
            rule_id: rule.id.clone(),
            severity: rule.severity.clone(),
            decision: rule.decision.on_missing.clone(),
            message: rule.decision.message.clone(),
            missing_facts: require.missing_facts,
            missing_evidence: require.missing_evidence,
        });
    }

    pub fn shadow_eval(
        &self,
        proposal: RuleProposal,
        input: &PolicyInput,
    ) -> Result<ShadowEvalResult, PolicyLoadError> {
        validate_rule_proposal(&proposal)?;
        validate_rule_ids(std::slice::from_ref(&proposal.proposed_rule))?;
        let baseline_decision = self.evaluate(input);
        let mut registry = self.registry.clone();
        registry.rules.push(proposal.proposed_rule);
        validate_rule_ids(&registry.rules)?;
        registry.policy_hash =
            crate::hash::hash_text(&serde_json::to_string(&registry).map_err(|err| {
                PolicyLoadError::Invalid(format!("cannot hash proposed policy registry: {err}"))
            })?);
        let proposal_engine = PolicyEngine::new(registry)?;
        let proposal_decision = proposal_engine.evaluate(input);
        let changed_decision = baseline_decision.decision != proposal_decision.decision;
        let mut introduced = BTreeSet::new();
        for action in &proposal_decision.required_actions {
            if !baseline_decision.required_actions.contains(action) {
                introduced.insert(action.clone());
            }
        }
        Ok(ShadowEvalResult {
            schema_version: "larql.governance.shadow_eval.v1".to_string(),
            active_policy_set: baseline_decision.active_policy_set.clone(),
            policy_hash: baseline_decision.policy_hash.clone(),
            baseline_decision,
            proposal_decision,
            changed_decision,
            introduced_required_actions: introduced.into_iter().collect(),
        })
    }

    pub fn shadow_eval_suite(
        &self,
        proposal: RuleProposal,
        suite: &PolicyTestSuite,
    ) -> Result<PolicyShadowEvalReport, PolicyLoadError> {
        validate_rule_proposal(&proposal)?;
        validate_rule_ids(std::slice::from_ref(&proposal.proposed_rule))?;
        let candidate_rule_id = proposal.proposed_rule.id.clone();
        let mut registry = self.registry.clone();
        registry.rules.push(proposal.proposed_rule);
        validate_rule_ids(&registry.rules)?;
        registry.policy_hash =
            crate::hash::hash_text(&serde_json::to_string(&registry).map_err(|err| {
                PolicyLoadError::Invalid(format!("cannot hash proposed policy registry: {err}"))
            })?);
        let proposal_engine = PolicyEngine::new(registry)?;
        let mut summary = PolicyShadowEvalSummary::default();
        let mut cases = Vec::new();

        for case in &suite.case {
            let input = policy_input_from_test_case(case);
            let baseline = self.evaluate(&input);
            let proposed = proposal_engine.evaluate(&input);
            let changed_decision = baseline.decision != proposed.decision;
            if changed_decision {
                summary.changed_cases += 1;
            }
            if is_allowing_decision(&case.expect.decision)
                && is_allowing_decision(&baseline.decision)
                && is_denying_decision(&proposed.decision)
            {
                summary.would_block_previously_accepted += 1;
            }
            if is_denying_decision(&case.expect.decision)
                && !is_denying_decision(&baseline.decision)
                && is_denying_decision(&proposed.decision)
            {
                summary.would_catch_known_bad_cases += 1;
            }
            if baseline.decision == DecisionKind::RequireReview
                && is_denying_decision(&proposed.decision)
            {
                summary.would_change_review_to_deny += 1;
            }
            if changed_decision
                && matches!(
                    proposed.decision,
                    DecisionKind::RequireReview | DecisionKind::AllowWithWarnings
                )
            {
                summary.ambiguous_cases += 1;
            }
            cases.push(PolicyShadowEvalCaseReport {
                name: case.name.clone(),
                expected_decision: case.expect.decision.clone(),
                baseline_decision: baseline.decision,
                proposal_decision: proposed.decision,
                changed_decision,
                matched_rules: proposed.matched_rules,
            });
        }

        let recommendation = if summary.would_block_previously_accepted > 0 {
            ShadowEvalRecommendation::Block
        } else if summary.would_change_review_to_deny > 0 || summary.ambiguous_cases > 0 {
            ShadowEvalRecommendation::RequiresHumanReview
        } else if summary.would_catch_known_bad_cases > 0 || summary.changed_cases > 0 {
            ShadowEvalRecommendation::AcceptWithWarning
        } else {
            ShadowEvalRecommendation::Accept
        };

        Ok(PolicyShadowEvalReport {
            schema_version: "larql.governance.shadow_eval_report.v1".to_string(),
            active_policy_set: format!(
                "{}@{}",
                self.registry.active_policy_set.id, self.registry.active_policy_set.version
            ),
            policy_hash: self.registry.policy_hash.clone(),
            candidate_rule_id,
            results: summary,
            recommendation,
            cases,
        })
    }

    pub fn conflict_analysis(
        &self,
        proposal: &RuleProposal,
    ) -> Result<PolicyConflictAnalysis, PolicyLoadError> {
        validate_rule_proposal(proposal)?;
        let candidate = &proposal.proposed_rule;
        let mut conflicts = Vec::new();
        let mut warnings = Vec::new();

        for rule in &self.registry.rules {
            if rule.id == candidate.id {
                conflicts.push(PolicyConflict {
                    kind: "duplicate_rule_id".to_string(),
                    message: format!("rule id already active: {}", candidate.id),
                });
            }
            if rule.class == candidate.class && rule.when == candidate.when {
                let active_rank = severity_rank(&rule.severity);
                let candidate_rank = severity_rank(&candidate.severity);
                if candidate_rank < active_rank {
                    conflicts.push(PolicyConflict {
                        kind: "rule_weakening".to_string(),
                        message: format!(
                            "candidate weakens existing rule {} from {:?} to {:?}",
                            rule.id, rule.severity, candidate.severity
                        ),
                    });
                } else if candidate_rank > active_rank {
                    warnings.push(PolicyConflict {
                        kind: "stricter_than_existing_policy".to_string(),
                        message: format!("candidate is stricter than existing rule {}", rule.id),
                    });
                }
            }
            if rule.class == PolicyClass::ConstitutionalPolicy
                && candidate.class != PolicyClass::ConstitutionalPolicy
                && candidate.decision.on_missing < rule.decision.on_missing
            {
                conflicts.push(PolicyConflict {
                    kind: "constitutional_rule_override".to_string(),
                    message: format!("candidate may weaken constitutional rule {}", rule.id),
                });
            }
        }

        match proposal.policy_update_type {
            PolicyUpdateType::RuleRelaxation => conflicts.push(PolicyConflict {
                kind: "policy_weakening_requires_ceremony".to_string(),
                message: "rule relaxation must enter PolicyWeakeningCeremony".to_string(),
            }),
            PolicyUpdateType::ConstitutionalChange => warnings.push(PolicyConflict {
                kind: "constitutional_change_requires_hard_path".to_string(),
                message:
                    "constitutional changes alter policy authority and require hard-path review"
                        .to_string(),
            }),
            _ => {}
        }

        if candidate.class == PolicyClass::ConstitutionalPolicy
            && proposal.policy_update_type != PolicyUpdateType::ConstitutionalChange
        {
            conflicts.push(PolicyConflict {
                kind: "constitutional_policy_requires_constitutional_change".to_string(),
                message: "constitutional policy rules require constitutional_change update type"
                    .to_string(),
            });
        }

        match candidate.class {
            PolicyClass::CiPolicy => warnings.push(PolicyConflict {
                kind: "changes_ci_behavior".to_string(),
                message: "candidate changes CI policy behavior".to_string(),
            }),
            PolicyClass::RuntimePolicy
            | PolicyClass::CapabilityPolicy
            | PolicyClass::ActivationPolicy => warnings.push(PolicyConflict {
                kind: "changes_runtime_capability_behavior".to_string(),
                message: "candidate changes runtime capability or activation behavior".to_string(),
            }),
            _ => {}
        }

        if creates_circular_evidence(candidate) {
            conflicts.push(PolicyConflict {
                kind: "circular_evidence_requirement".to_string(),
                message: "candidate requires evidence with same name as its action".to_string(),
            });
        }

        let recommendation = if conflicts.is_empty() {
            if warnings.is_empty() {
                ConflictRecommendation::Allow
            } else {
                ConflictRecommendation::AllowWithReview
            }
        } else {
            ConflictRecommendation::Block
        };
        Ok(PolicyConflictAnalysis {
            event_type: "PolicyConflictAnalysisCompleted".to_string(),
            candidate_rule_id: candidate.id.clone(),
            conflicts,
            warnings,
            recommendation,
        })
    }

    pub fn verify_policy_apply_request(
        &self,
        request: &PolicyApplyRequest,
    ) -> Result<(), PolicyLoadError> {
        validate_policy_apply_request(&self.registry, request)
    }

    pub fn test_suite(&self, suite: &PolicyTestSuite) -> PolicyTestReport {
        let cases = suite
            .case
            .iter()
            .map(|case| {
                let input = policy_input_from_test_case(case);
                let decision = self.evaluate(&input);
                let rule_pass = case
                    .expect
                    .rule_id
                    .as_ref()
                    .is_none_or(|rule_id| decision.matched_rules.contains(rule_id));
                let passed = decision.decision == case.expect.decision && rule_pass;
                PolicyTestCaseReport {
                    name: case.name.clone(),
                    passed,
                    expected_decision: case.expect.decision.clone(),
                    actual_decision: decision.decision,
                    expected_rule_id: case.expect.rule_id.clone(),
                    matched_rules: decision.matched_rules,
                }
            })
            .collect();
        PolicyTestReport {
            schema_version: "larql.governance.policy_test_report.v1".to_string(),
            active_policy_set: format!(
                "{}@{}",
                self.registry.active_policy_set.id, self.registry.active_policy_set.version
            ),
            policy_hash: self.registry.policy_hash.clone(),
            cases,
        }
    }
}

pub fn draft_rule_proposal_from_intent(
    concern: &ProsePolicyConcern,
    extraction: &PolicyIntentExtraction,
    policy_update_type: PolicyUpdateType,
    proposer: impl Into<String>,
) -> Result<RuleProposal, PolicyLoadError> {
    if concern.ceremony_id != extraction.ceremony_id {
        return Err(PolicyLoadError::Invalid(
            "concern and intent ceremony ids differ".to_string(),
        ));
    }
    if concern.prose.trim().is_empty() {
        return Err(PolicyLoadError::Invalid(
            "prose concern cannot be empty".to_string(),
        ));
    }
    if extraction.intent.required_evidence.is_empty() {
        return Err(PolicyLoadError::Invalid(
            "formal rule draft requires at least one evidence requirement".to_string(),
        ));
    }
    let action = extraction.intent.action.trim();
    if action.is_empty() {
        return Err(PolicyLoadError::Invalid(
            "formal rule draft requires action".to_string(),
        ));
    }
    let target = concern
        .target_area
        .rsplit(':')
        .next()
        .unwrap_or(&concern.target_area)
        .replace('-', "_");
    let evidence_slug = extraction.intent.required_evidence[0].replace('-', "_");
    let id = format!("{}.requires_{}", target, evidence_slug);
    let required_actions = extraction
        .intent
        .required_evidence
        .iter()
        .map(|evidence| format!("attach {evidence} evidence"))
        .collect();
    let required_conditions = extraction
        .intent
        .required_evidence
        .iter()
        .map(|evidence| Condition {
            evidence: Some(evidence.clone()),
            present: Some(true),
            ..Condition::default()
        })
        .collect();
    let proposal = RuleProposal {
        schema_version: "larql.governance.rule_proposal.v1".to_string(),
        policy_update_type,
        proposed_rule: PolicyRule {
            id,
            class: extraction.intent.policy_class.clone(),
            severity: PolicySeverity::Deny,
            description: concern.prose.clone(),
            when: ConditionBlock {
                all: vec![Condition {
                    action: Some(action.to_string()),
                    ..Condition::default()
                }],
                ..ConditionBlock::default()
            },
            require: ConditionBlock {
                all: required_conditions,
                ..ConditionBlock::default()
            },
            decision: RuleDecision {
                on_missing: DecisionKind::Deny,
                message: format!(
                    "{} requires formal evidence: {}.",
                    action,
                    extraction.intent.required_evidence.join(", ")
                ),
                required_actions,
            },
        },
        proposer: proposer.into(),
        rationale: format!("Formalized from prose concern {}", concern.ceremony_id),
        approval_capability: None,
        examples: vec![
            PolicyRuleExample {
                kind: PolicyRuleExampleKind::Deny,
                description: "Required evidence is absent.".to_string(),
            },
            PolicyRuleExample {
                kind: PolicyRuleExampleKind::Allow,
                description: "All required evidence is present.".to_string(),
            },
            PolicyRuleExample {
                kind: PolicyRuleExampleKind::ReviewRequired,
                description: "Evidence is present but boundary or scope remains ambiguous."
                    .to_string(),
            },
        ],
    };
    validate_rule_proposal(&proposal)?;
    Ok(proposal)
}

pub fn validate_policy_apply_request(
    registry: &PolicyRegistry,
    request: &PolicyApplyRequest,
) -> Result<(), PolicyLoadError> {
    validate_rule_proposal(&request.proposal)?;
    if request.approval.schema_version != "larql.governance.policy_approval.v1" {
        return Err(PolicyLoadError::Invalid(
            "unsupported policy approval schema".to_string(),
        ));
    }
    if request.approval.event_type != "PolicyChangeApproved" {
        return Err(PolicyLoadError::Invalid(
            "policy approval event_type must be PolicyChangeApproved".to_string(),
        ));
    }
    if request.capability.capability_type != "PolicyUpdate" {
        return Err(PolicyLoadError::Invalid(
            "capability_type must be PolicyUpdate".to_string(),
        ));
    }
    let rule_id = &request.proposal.proposed_rule.id;
    if request.approval.approved_rule_id != *rule_id {
        return Err(PolicyLoadError::Invalid(
            "approval does not name proposed rule id".to_string(),
        ));
    }
    if request.approval.prior_policy_hash != registry.policy_hash
        || request.capability.prior_policy_hash != registry.policy_hash
    {
        return Err(PolicyLoadError::Invalid(
            "prior policy hash is stale".to_string(),
        ));
    }
    if request.approval.prior_state_hash != request.capability.prior_state_hash {
        return Err(PolicyLoadError::Invalid(
            "approval and capability prior state hashes differ".to_string(),
        ));
    }
    if !target_policy_file_is_active_include(registry, &request.target_policy_file) {
        return Err(PolicyLoadError::Invalid(
            "target policy file is not included by active policy set".to_string(),
        ));
    }
    if !request
        .capability
        .allowed_rule_ids
        .iter()
        .any(|id| id == rule_id)
    {
        return Err(PolicyLoadError::Invalid(
            "capability does not allow proposed rule id".to_string(),
        ));
    }
    if !request
        .capability
        .allowed_policy_files
        .iter()
        .any(|path| path == &request.target_policy_file)
    {
        return Err(PolicyLoadError::Invalid(
            "capability does not allow target policy file".to_string(),
        ));
    }
    for check in [
        "governor policy test",
        "governor policy shadow-eval",
        "governor replay",
    ] {
        if !request
            .capability
            .required_checks
            .iter()
            .any(|required| required == check)
        {
            return Err(PolicyLoadError::Invalid(format!(
                "capability missing required check: {check}"
            )));
        }
    }
    if !request.approval.conflict_analysis_passed
        || !request.approval.shadow_evaluation_completed
        || !request.approval.policy_tests_passed
        || !request.approval.replay_verified
    {
        return Err(PolicyLoadError::Invalid(
            "approval lacks completed policy-update checks".to_string(),
        ));
    }
    if matches!(
        request.proposal.policy_update_type,
        PolicyUpdateType::RuleRelaxation
    ) {
        return Err(PolicyLoadError::Invalid(
            "rule relaxation must use PolicyWeakeningCeremony".to_string(),
        ));
    }
    let mut candidate_rules = registry.rules.clone();
    candidate_rules.push(request.proposal.proposed_rule.clone());
    validate_rule_ids(&candidate_rules)?;
    Ok(())
}

pub fn apply_policy_update(
    policy_index_path: &Path,
    request: &PolicyApplyRequest,
) -> Result<PolicyApplyReceipt, PolicyLoadError> {
    let registry = load_policy_registry(policy_index_path)?;
    validate_policy_apply_request(&registry, request)?;
    let policy_root = policy_index_path.parent().ok_or_else(|| {
        PolicyLoadError::MissingPolicyRoot(policy_index_path.display().to_string())
    })?;
    let active_include = active_policy_include_for_target(&registry, &request.target_policy_file)
        .ok_or_else(|| {
        PolicyLoadError::Invalid(
            "target policy file is not included by active policy set".to_string(),
        )
    })?;
    validate_policy_include_segment(active_include)?;
    let policy_file = policy_root.join(active_include);

    let mut file = OpenOptions::new()
        .create(false)
        .append(true)
        .open(&policy_file)
        .map_err(|source| PolicyLoadError::Io {
            path: policy_file.display().to_string(),
            source,
        })?;
    writeln!(file).map_err(|source| PolicyLoadError::Io {
        path: policy_file.display().to_string(),
        source,
    })?;
    write!(file, "{}", rule_as_toml(&request.proposal.proposed_rule)?).map_err(|source| {
        PolicyLoadError::Io {
            path: policy_file.display().to_string(),
            source,
        }
    })?;
    drop(file);

    let new_registry = load_policy_registry(policy_index_path)?;
    if new_registry.policy_hash == registry.policy_hash {
        return Err(PolicyLoadError::Invalid(
            "policy apply did not advance active policy hash".to_string(),
        ));
    }
    if !new_registry
        .rules
        .iter()
        .any(|rule| rule.id == request.proposal.proposed_rule.id)
    {
        return Err(PolicyLoadError::Invalid(
            "policy apply did not activate proposed rule".to_string(),
        ));
    }

    Ok(PolicyApplyReceipt {
        schema_version: "larql.governance.policy_apply_receipt.v1".to_string(),
        event_type: "PolicyFileUpdated".to_string(),
        rule_id: request.proposal.proposed_rule.id.clone(),
        policy_file: request.target_policy_file.clone(),
        prior_policy_hash: registry.policy_hash,
        new_policy_hash: new_registry.policy_hash,
        prior_state_hash: request.capability.prior_state_hash.clone(),
        applied_by_capability: request.capability.capability_type.clone(),
        required_checks: request.capability.required_checks.clone(),
    })
}

pub fn mint_policy_update_capability(
    registry: &PolicyRegistry,
    proposal: &RuleProposal,
    approval: &PolicyApproval,
    target_policy_file: impl Into<String>,
    expires_at: impl Into<String>,
) -> Result<PolicyCapability, PolicyLoadError> {
    validate_rule_proposal(proposal)?;
    if matches!(
        proposal.policy_update_type,
        PolicyUpdateType::RuleRelaxation
    ) {
        return Err(PolicyLoadError::Invalid(
            "rule relaxation must use PolicyWeakeningCeremony".to_string(),
        ));
    }
    if approval.approved_rule_id != proposal.proposed_rule.id {
        return Err(PolicyLoadError::Invalid(
            "approval does not name proposed rule id".to_string(),
        ));
    }
    if approval.prior_policy_hash != registry.policy_hash {
        return Err(PolicyLoadError::Invalid(
            "approval prior policy hash is stale".to_string(),
        ));
    }
    if !approval.conflict_analysis_passed
        || !approval.shadow_evaluation_completed
        || !approval.policy_tests_passed
        || !approval.replay_verified
    {
        return Err(PolicyLoadError::Invalid(
            "approval lacks completed policy-update checks".to_string(),
        ));
    }
    let target_policy_file = target_policy_file.into();
    if !target_policy_file_is_active_include(registry, &target_policy_file) {
        return Err(PolicyLoadError::Invalid(
            "target policy file is not included by active policy set".to_string(),
        ));
    }

    Ok(PolicyCapability {
        capability_type: "PolicyUpdate".to_string(),
        allowed_policy_files: vec![target_policy_file],
        allowed_rule_ids: vec![proposal.proposed_rule.id.clone()],
        prior_policy_hash: registry.policy_hash.clone(),
        prior_state_hash: approval.prior_state_hash.clone(),
        expires_at: expires_at.into(),
        required_checks: vec![
            "governor policy test".to_string(),
            "governor policy shadow-eval".to_string(),
            "governor replay".to_string(),
        ],
    })
}

pub fn validate_policy_update_ceremony_trace(
    trace: &PolicyUpdateCeremonyTrace,
) -> PolicyUpdateCeremonyReport {
    let mut report = PolicyUpdateCeremonyReport {
        schema_version: "larql.governance.policy_update_ceremony_report.v1".to_string(),
        ceremony_id: trace.ceremony_id.clone(),
        passed: true,
        missing_phases: Vec::new(),
        out_of_order_phases: Vec::new(),
        errors: Vec::new(),
    };

    if trace.schema_version != "larql.governance.policy_update_trace.v1" {
        report
            .errors
            .push("unsupported policy update trace schema".to_string());
    }
    if matches!(
        trace.policy_update_type,
        Some(PolicyUpdateType::RuleRelaxation)
    ) {
        report
            .errors
            .push("rule relaxation must use PolicyWeakeningCeremony".to_string());
    }

    let required = policy_update_required_phases();
    let event_types: Vec<&str> = trace
        .events
        .iter()
        .map(|event| event.event_type.as_str())
        .collect();
    let mut last_index = 0usize;
    for phase in required {
        match event_types.iter().position(|event| event == phase) {
            Some(index) if index >= last_index => {
                last_index = index;
            }
            Some(_) => report.out_of_order_phases.push(phase.to_string()),
            None => report.missing_phases.push(phase.to_string()),
        }
    }

    report.passed = report.missing_phases.is_empty()
        && report.out_of_order_phases.is_empty()
        && report.errors.is_empty();
    report
}

pub fn validate_policy_weakening_evidence(
    evidence: &PolicyWeakeningEvidence,
) -> Result<(), PolicyLoadError> {
    if evidence.schema_version != "larql.governance.policy_weakening_evidence.v1" {
        return Err(PolicyLoadError::Invalid(
            "unsupported policy weakening evidence schema".to_string(),
        ));
    }
    if evidence.ceremony_id.trim().is_empty() {
        return Err(PolicyLoadError::Invalid(
            "policy weakening evidence requires ceremony_id".to_string(),
        ));
    }
    if evidence.authority_reduced.trim().is_empty() {
        return Err(PolicyLoadError::Invalid(
            "policy weakening evidence must name authority being reduced".to_string(),
        ));
    }
    crate::policy::require_hash(
        "too_strict_rationale_hash",
        &evidence.too_strict_rationale_hash,
    )
    .map_err(|err| PolicyLoadError::Invalid(err.to_string()))?;
    crate::policy::require_hashes(
        "false_positive_example_hashes",
        &evidence.false_positive_example_hashes,
    )
    .map_err(|err| PolicyLoadError::Invalid(err.to_string()))?;
    crate::policy::require_hash(
        "hostile_path_regression_test_hash",
        &evidence.hostile_path_regression_test_hash,
    )
    .map_err(|err| PolicyLoadError::Invalid(err.to_string()))?;
    crate::policy::require_hash("rollback_path_hash", &evidence.rollback_path_hash)
        .map_err(|err| PolicyLoadError::Invalid(err.to_string()))?;
    crate::policy::require_hash("human_approval_hash", &evidence.human_approval_hash)
        .map_err(|err| PolicyLoadError::Invalid(err.to_string()))?;
    Ok(())
}

fn policy_update_required_phases() -> &'static [&'static str] {
    &[
        "ProsePolicyConcernSubmitted",
        "PolicyIntentExtracted",
        "RuleCandidateDrafted",
        "RuleExamplesAttached",
        "ConflictAnalysisCompleted",
        "ShadowEvaluationCompleted",
        "PolicyChangeApproved",
        "PolicyCapabilityMinted",
        "PolicyFileUpdated",
        "PolicyHashAdvanced",
        "ReplayVerified",
        "PolicyUpdateClosed",
    ]
}

fn validate_rule_ids(rules: &[PolicyRule]) -> Result<(), PolicyLoadError> {
    let mut ids = BTreeSet::new();
    for rule in rules {
        if rule.id.trim().is_empty() {
            return Err(PolicyLoadError::Invalid(
                "policy rule requires id".to_string(),
            ));
        }
        if !ids.insert(rule.id.clone()) {
            return Err(PolicyLoadError::Invalid(format!(
                "duplicate policy rule id: {}",
                rule.id
            )));
        }
    }
    Ok(())
}

fn validate_rule_proposal(proposal: &RuleProposal) -> Result<(), PolicyLoadError> {
    if proposal.schema_version != "larql.governance.rule_proposal.v1" {
        return Err(PolicyLoadError::Invalid(
            "unsupported rule proposal schema".to_string(),
        ));
    }
    let mut kinds = BTreeSet::new();
    for example in &proposal.examples {
        kinds.insert(example.kind.clone());
    }
    for required in [
        PolicyRuleExampleKind::Allow,
        PolicyRuleExampleKind::Deny,
        PolicyRuleExampleKind::ReviewRequired,
    ] {
        if !kinds.contains(&required) {
            return Err(PolicyLoadError::Invalid(format!(
                "rule proposal missing {:?} example",
                required
            )));
        }
    }
    Ok(())
}

fn rule_as_toml(rule: &PolicyRule) -> Result<String, PolicyLoadError> {
    #[derive(Serialize)]
    struct RuleAppend<'a> {
        rule: Vec<&'a PolicyRule>,
    }
    let mut text = toml::to_string_pretty(&RuleAppend { rule: vec![rule] })?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

fn creates_circular_evidence(rule: &PolicyRule) -> bool {
    required_evidence(&rule.require)
        .into_iter()
        .any(|evidence| {
            rule.when
                .all
                .iter()
                .any(|condition| condition.action == Some(evidence.clone()))
        })
}

fn required_evidence(block: &ConditionBlock) -> Vec<String> {
    block
        .all
        .iter()
        .chain(block.any.iter())
        .filter_map(|condition| condition.evidence.clone())
        .collect()
}

fn severity_rank(severity: &PolicySeverity) -> u8 {
    match severity {
        PolicySeverity::Info => 0,
        PolicySeverity::Warn => 1,
        PolicySeverity::ReviewRequired => 2,
        PolicySeverity::Deny => 3,
        PolicySeverity::Fatal => 4,
    }
}

fn policy_input_from_test_case(case: &PolicyTestCase) -> PolicyInput {
    PolicyInput {
        schema_version: "larql.governance.policy_input.v1".to_string(),
        state_hash: crate::hash::hash_text(&case.name),
        actor: "policy-test".to_string(),
        action: case.action.clone(),
        target_paths: Vec::new(),
        evidence: case
            .evidence
            .iter()
            .filter_map(|(key, present)| present.then_some(key.clone()))
            .collect(),
        risk: 0,
        facts: case
            .facts
            .iter()
            .map(|(fact_type, value)| PolicyFact {
                fact_type: fact_type.clone(),
                value: value.clone(),
                subject: None,
                evidence: Vec::new(),
                source: Some("policy-test".to_string()),
                state_hash: None,
            })
            .collect(),
    }
}

fn read_policy_text(path: &Path) -> Result<String, PolicyLoadError> {
    std::fs::read_to_string(path).map_err(|source| PolicyLoadError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn normalize_policy_include(root: &Path, include: &str) -> Result<PathBuf, PolicyLoadError> {
    let path = root.join(include);
    if include.contains("..") || Path::new(include).is_absolute() {
        return Err(PolicyLoadError::Invalid(format!(
            "policy include must stay under policy root: {include}"
        )));
    }
    Ok(path)
}

#[derive(Debug, Default)]
pub(crate) struct ConditionOutcome {
    matched: bool,
    missing_facts: Vec<String>,
    missing_evidence: Vec<String>,
    evidence: Vec<String>,
}

pub(crate) fn fact_map(facts: &[PolicyFact]) -> HashMap<String, &PolicyFact> {
    let mut map = HashMap::new();
    for fact in facts {
        map.insert(fact.fact_type.clone(), fact);
    }
    map
}

pub(crate) fn evaluate_block(
    block: &ConditionBlock,
    input: &PolicyInput,
    facts: &HashMap<String, &PolicyFact>,
) -> ConditionOutcome {
    let mut outcome = ConditionOutcome {
        matched: true,
        ..ConditionOutcome::default()
    };

    for condition in &block.all {
        let condition_outcome = evaluate_condition(condition, input, facts);
        merge_outcome(&mut outcome, &condition_outcome);
        outcome.matched &= condition_outcome.matched;
    }

    if !block.any.is_empty() {
        let mut any_matched = false;
        for condition in &block.any {
            let condition_outcome = evaluate_condition(condition, input, facts);
            merge_outcome(&mut outcome, &condition_outcome);
            any_matched |= condition_outcome.matched;
        }
        outcome.matched &= any_matched;
    }

    for condition in &block.not_conditions {
        let condition_outcome = evaluate_condition(condition, input, facts);
        merge_outcome(&mut outcome, &condition_outcome);
        outcome.matched &= !condition_outcome.matched;
    }

    outcome
}

fn evaluate_condition(
    condition: &Condition,
    input: &PolicyInput,
    facts: &HashMap<String, &PolicyFact>,
) -> ConditionOutcome {
    let mut matched = true;
    let mut missing_facts = Vec::new();
    let mut missing_evidence = Vec::new();
    let mut evidence = Vec::new();

    if let Some(action) = &condition.action {
        matched &= input.action == *action;
    }
    if let Some(pattern) = &condition.path_matches {
        matched &= input
            .target_paths
            .iter()
            .any(|path| path_matches(path, pattern));
    }
    if let Some(threshold) = condition.risk_gte {
        matched &= input.risk >= threshold;
    }
    if let Some(evidence_id) = &condition.evidence {
        let present = input.evidence.iter().any(|value| value == evidence_id)
            || facts
                .values()
                .any(|fact| fact.evidence.iter().any(|value| value == evidence_id));
        if !present {
            missing_evidence.push(evidence_id.clone());
        } else {
            evidence.push(evidence_id.clone());
        }
        matched &= condition.present.unwrap_or(true) == present;
    }
    if let Some(fact_id) = &condition.fact {
        match facts.get(fact_id) {
            Some(fact) => {
                if let Some(expected) = &condition.equals {
                    matched &= fact.value == *expected;
                }
                if let Some(present) = condition.present {
                    matched &= present;
                }
                evidence.extend(fact.evidence.clone());
            }
            None => {
                missing_facts.push(fact_id.clone());
                matched &= condition.present == Some(false);
            }
        }
    }

    ConditionOutcome {
        matched,
        missing_facts,
        missing_evidence,
        evidence,
    }
}

fn merge_outcome(target: &mut ConditionOutcome, source: &ConditionOutcome) {
    target.missing_facts.extend(source.missing_facts.clone());
    target
        .missing_evidence
        .extend(source.missing_evidence.clone());
    target.evidence.extend(source.evidence.clone());
}

fn path_matches(path: &str, pattern: &str) -> bool {
    if pattern == "*" || pattern == path {
        return true;
    }
    if let Some((prefix, suffix)) = pattern.split_once('*') {
        return path.starts_with(prefix) && path.ends_with(suffix);
    }
    path.contains(pattern)
}

fn most_restrictive(left: DecisionKind, right: DecisionKind) -> DecisionKind {
    if restriction_rank(&right) > restriction_rank(&left) {
        right
    } else {
        left
    }
}

fn restriction_rank(decision: &DecisionKind) -> u8 {
    match decision {
        DecisionKind::Allow => 0,
        DecisionKind::AllowWithWarnings => 1,
        DecisionKind::RequireReview => 2,
        DecisionKind::Deny => 3,
        DecisionKind::Fatal => 4,
    }
}

fn is_allowing_decision(decision: &DecisionKind) -> bool {
    matches!(
        decision,
        DecisionKind::Allow | DecisionKind::AllowWithWarnings
    )
}

fn is_denying_decision(decision: &DecisionKind) -> bool {
    matches!(decision, DecisionKind::Deny | DecisionKind::Fatal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn engine() -> PolicyEngine {
        PolicyEngine::new(PolicyRegistry {
            schema_version: "larql.governance.policy_registry.v1".to_string(),
            active_policy_set: PolicySet {
                id: "test".to_string(),
                version: "1".to_string(),
                includes: vec!["machine-runtime.toml".to_string()],
            },
            policy_hash: crate::hash::hash_text("test-policy"),
            mode: PolicyMode {
                unknown_rule: UnknownRuleMode::Deny,
                unknown_fact: UnknownFactMode::Warn,
                conflict_resolution: ConflictResolutionMode::MostRestrictive,
            },
            rules: vec![PolicyRule {
                id: "machine_creation.requires_overlap_check".to_string(),
                class: PolicyClass::CeremonyPolicy,
                severity: PolicySeverity::Deny,
                description: "requires overlap evidence".to_string(),
                when: ConditionBlock {
                    all: vec![Condition {
                        action: Some("create_machine".to_string()),
                        ..Condition::default()
                    }],
                    ..ConditionBlock::default()
                },
                require: ConditionBlock {
                    all: vec![Condition {
                        evidence: Some("existing_machine_overlap_check".to_string()),
                        present: Some(true),
                        ..Condition::default()
                    }],
                    ..ConditionBlock::default()
                },
                decision: RuleDecision {
                    on_missing: DecisionKind::Deny,
                    message: "missing overlap".to_string(),
                    required_actions: vec!["attach overlap check".to_string()],
                },
            }],
        })
        .unwrap()
    }

    #[test]
    fn material_registry_matches_disk_for_repo_policies() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let index_path = manifest.join("../../governance/policies/index.toml");
        if !index_path.exists() {
            return;
        }
        let index_path = fs::canonicalize(index_path).unwrap();
        let disk = load_policy_registry(&index_path).unwrap();
        let index_text = fs::read_to_string(&index_path).unwrap();
        let root = index_path.parent().unwrap();
        let mut packs = BTreeMap::new();
        for include in &disk.active_policy_set.includes {
            let pack_path = root.join(include);
            packs.insert(include.clone(), fs::read_to_string(&pack_path).unwrap());
        }
        let from_material =
            load_policy_registry_from_material("repo_index.toml", &index_text, &packs).unwrap();
        assert_eq!(disk.policy_hash, from_material.policy_hash);
        assert_eq!(disk.rules.len(), from_material.rules.len());
        assert_eq!(disk.active_policy_set, from_material.active_policy_set);
        assert_eq!(disk.mode, from_material.mode);
    }

    fn proposal() -> RuleProposal {
        RuleProposal {
            schema_version: "larql.governance.rule_proposal.v1".to_string(),
            policy_update_type: PolicyUpdateType::NewRule,
            proposed_rule: PolicyRule {
                id: "machine_creation.requires_boundary".to_string(),
                class: PolicyClass::CeremonyPolicy,
                severity: PolicySeverity::Deny,
                description: "requires boundary evidence".to_string(),
                when: ConditionBlock {
                    all: vec![Condition {
                        action: Some("create_machine".to_string()),
                        ..Condition::default()
                    }],
                    ..ConditionBlock::default()
                },
                require: ConditionBlock {
                    all: vec![Condition {
                        evidence: Some("machine_responsibility_boundary".to_string()),
                        present: Some(true),
                        ..Condition::default()
                    }],
                    ..ConditionBlock::default()
                },
                decision: RuleDecision {
                    on_missing: DecisionKind::Deny,
                    message: "missing boundary".to_string(),
                    required_actions: vec!["attach boundary".to_string()],
                },
            },
            proposer: "human:test".to_string(),
            rationale: "test".to_string(),
            approval_capability: None,
            examples: vec![
                PolicyRuleExample {
                    kind: PolicyRuleExampleKind::Deny,
                    description: "missing evidence".to_string(),
                },
                PolicyRuleExample {
                    kind: PolicyRuleExampleKind::Allow,
                    description: "evidence present".to_string(),
                },
                PolicyRuleExample {
                    kind: PolicyRuleExampleKind::ReviewRequired,
                    description: "ambiguous evidence".to_string(),
                },
            ],
        }
    }

    fn apply_request_for(registry: &PolicyRegistry) -> PolicyApplyRequest {
        let proposal = proposal();
        let state_hash = crate::hash::hash_text("state");
        PolicyApplyRequest {
            approval: PolicyApproval {
                schema_version: "larql.governance.policy_approval.v1".to_string(),
                event_type: "PolicyChangeApproved".to_string(),
                ceremony_id: "policy_update_test".to_string(),
                approver: "human:test".to_string(),
                approved_rule_id: proposal.proposed_rule.id.clone(),
                prior_policy_hash: registry.policy_hash.clone(),
                prior_state_hash: state_hash.clone(),
                conflict_analysis_passed: true,
                shadow_evaluation_completed: true,
                policy_tests_passed: true,
                replay_verified: true,
            },
            capability: PolicyCapability {
                capability_type: "PolicyUpdate".to_string(),
                allowed_policy_files: vec!["governance/policies/machine-runtime.toml".to_string()],
                allowed_rule_ids: vec![proposal.proposed_rule.id.clone()],
                prior_policy_hash: registry.policy_hash.clone(),
                prior_state_hash: state_hash,
                expires_at: "2026-05-09T00:00:00Z".to_string(),
                required_checks: vec![
                    "governor policy test".to_string(),
                    "governor policy shadow-eval".to_string(),
                    "governor replay".to_string(),
                ],
            },
            proposal,
            target_policy_file: "governance/policies/machine-runtime.toml".to_string(),
        }
    }

    fn unique_temp_policy_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("larql-policy-apply-{}-{nanos}", std::process::id()))
    }

    fn write_minimal_policy_index(root: &Path) -> PathBuf {
        fs::create_dir_all(root).unwrap();
        fs::write(
            root.join("index.toml"),
            r#"schema_version = "larql.governance.policy_index.v1"

[active_policy_set]
id = "test-policy-set"
version = "1"
includes = ["machine-runtime.toml"]

[mode]
unknown_rule = "deny"
unknown_fact = "warn"
conflict_resolution = "most_restrictive"
"#,
        )
        .unwrap();
        fs::write(
            root.join("machine-runtime.toml"),
            r#"schema_version = "larql.governance.policy_pack.v1"
"#,
        )
        .unwrap();
        root.join("index.toml")
    }

    #[test]
    fn conflict_analysis_flags_runtime_ci_and_constitutional_authority_changes() {
        let engine = engine();

        let mut ci_proposal = proposal();
        ci_proposal.proposed_rule.id = "ci.fixture_requires_review".to_string();
        ci_proposal.proposed_rule.class = PolicyClass::CiPolicy;
        let ci_report = engine.conflict_analysis(&ci_proposal).unwrap();
        assert!(ci_report
            .warnings
            .iter()
            .any(|warning| warning.kind == "changes_ci_behavior"));

        let mut runtime_proposal = proposal();
        runtime_proposal.proposed_rule.id = "runtime.fixture_requires_review".to_string();
        runtime_proposal.proposed_rule.class = PolicyClass::CapabilityPolicy;
        let runtime_report = engine.conflict_analysis(&runtime_proposal).unwrap();
        assert!(runtime_report
            .warnings
            .iter()
            .any(|warning| warning.kind == "changes_runtime_capability_behavior"));

        let mut constitutional_proposal = proposal();
        constitutional_proposal.proposed_rule.id =
            "constitutional.fixture_requires_review".to_string();
        constitutional_proposal.proposed_rule.class = PolicyClass::ConstitutionalPolicy;
        let constitutional_report = engine.conflict_analysis(&constitutional_proposal).unwrap();
        assert!(constitutional_report.conflicts.iter().any(|conflict| {
            conflict.kind == "constitutional_policy_requires_constitutional_change"
        }));
    }

    #[test]
    fn policy_engine_denies_missing_required_evidence() {
        let decision = engine().evaluate(&PolicyInput {
            schema_version: "larql.governance.policy_input.v1".to_string(),
            state_hash: crate::hash::hash_text("state"),
            actor: "test".to_string(),
            action: "create_machine".to_string(),
            target_paths: Vec::new(),
            evidence: Vec::new(),
            risk: 0,
            facts: Vec::new(),
        });
        assert_eq!(decision.decision, DecisionKind::Deny);
        assert_eq!(
            decision.matched_rules,
            vec!["machine_creation.requires_overlap_check"]
        );
    }

    #[test]
    fn policy_engine_allows_when_required_evidence_present() {
        let decision = engine().evaluate(&PolicyInput {
            schema_version: "larql.governance.policy_input.v1".to_string(),
            state_hash: crate::hash::hash_text("state"),
            actor: "test".to_string(),
            action: "create_machine".to_string(),
            target_paths: Vec::new(),
            evidence: vec!["existing_machine_overlap_check".to_string()],
            risk: 0,
            facts: Vec::new(),
        });
        assert_eq!(decision.decision, DecisionKind::Allow);
    }

    #[test]
    fn policy_engine_denies_unknown_action_in_deny_mode() {
        let decision = engine().evaluate(&PolicyInput {
            schema_version: "larql.governance.policy_input.v1".to_string(),
            state_hash: crate::hash::hash_text("state"),
            actor: "test".to_string(),
            action: "unknown_governed_action".to_string(),
            target_paths: Vec::new(),
            evidence: Vec::new(),
            risk: 0,
            facts: Vec::new(),
        });
        assert_eq!(decision.decision, DecisionKind::Deny);
        assert!(decision.matched_rules.is_empty());
        assert_eq!(decision.findings[0].rule_id, "policy.unknown_action.deny");
    }

    #[test]
    fn policy_apply_request_requires_capability_scope() {
        let proposal = proposal();
        let engine = engine();
        let hash = engine.registry().policy_hash.clone();
        let request = PolicyApplyRequest {
            proposal,
            approval: PolicyApproval {
                schema_version: "larql.governance.policy_approval.v1".to_string(),
                event_type: "PolicyChangeApproved".to_string(),
                ceremony_id: "policy_update_test".to_string(),
                approver: "human:test".to_string(),
                approved_rule_id: "machine_creation.requires_boundary".to_string(),
                prior_policy_hash: hash.clone(),
                prior_state_hash: crate::hash::hash_text("state").to_string(),
                conflict_analysis_passed: true,
                shadow_evaluation_completed: true,
                policy_tests_passed: true,
                replay_verified: true,
            },
            capability: PolicyCapability {
                capability_type: "PolicyUpdate".to_string(),
                allowed_policy_files: vec!["governance/policies/machine-runtime.toml".to_string()],
                allowed_rule_ids: vec!["machine_creation.requires_boundary".to_string()],
                prior_policy_hash: hash,
                prior_state_hash: crate::hash::hash_text("state").to_string(),
                expires_at: "2026-05-09T00:00:00Z".to_string(),
                required_checks: vec![
                    "governor policy test".to_string(),
                    "governor policy shadow-eval".to_string(),
                    "governor replay".to_string(),
                ],
            },
            target_policy_file: "governance/policies/machine-runtime.toml".to_string(),
        };
        assert!(engine.verify_policy_apply_request(&request).is_ok());
    }

    #[test]
    fn policy_apply_request_rejects_inactive_policy_file() {
        let proposal = proposal();
        let engine = engine();
        let hash = engine.registry().policy_hash.clone();
        let request = PolicyApplyRequest {
            proposal,
            approval: PolicyApproval {
                schema_version: "larql.governance.policy_approval.v1".to_string(),
                event_type: "PolicyChangeApproved".to_string(),
                ceremony_id: "policy_update_test".to_string(),
                approver: "human:test".to_string(),
                approved_rule_id: "machine_creation.requires_boundary".to_string(),
                prior_policy_hash: hash.clone(),
                prior_state_hash: crate::hash::hash_text("state").to_string(),
                conflict_analysis_passed: true,
                shadow_evaluation_completed: true,
                policy_tests_passed: true,
                replay_verified: true,
            },
            capability: PolicyCapability {
                capability_type: "PolicyUpdate".to_string(),
                allowed_policy_files: vec!["governance/policies/inactive.toml".to_string()],
                allowed_rule_ids: vec!["machine_creation.requires_boundary".to_string()],
                prior_policy_hash: hash,
                prior_state_hash: crate::hash::hash_text("state").to_string(),
                expires_at: "2026-05-09T00:00:00Z".to_string(),
                required_checks: vec![
                    "governor policy test".to_string(),
                    "governor policy shadow-eval".to_string(),
                    "governor replay".to_string(),
                ],
            },
            target_policy_file: "governance/policies/inactive.toml".to_string(),
        };
        let err = engine.verify_policy_apply_request(&request).unwrap_err();
        assert!(err.to_string().contains("active policy set"));
    }

    #[test]
    fn policy_apply_runtime_appends_active_rule_and_advances_hash() {
        let root = unique_temp_policy_root();
        let index_path = write_minimal_policy_index(&root);
        let registry = load_policy_registry(&index_path).unwrap();
        let request = apply_request_for(&registry);

        let receipt = apply_policy_update(&index_path, &request).unwrap();
        assert_eq!(receipt.rule_id, "machine_creation.requires_boundary");
        assert_eq!(receipt.prior_policy_hash, registry.policy_hash);
        assert_ne!(receipt.new_policy_hash, registry.policy_hash);

        let new_registry = load_policy_registry(&index_path).unwrap();
        assert_eq!(receipt.new_policy_hash, new_registry.policy_hash);
        assert!(new_registry
            .rules
            .iter()
            .any(|rule| rule.id == "machine_creation.requires_boundary"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn shadow_eval_suite_reports_policy_impact_counts() {
        let suite = PolicyTestSuite {
            case: vec![PolicyTestCase {
                name: "accepted_machine_creation_without_boundary".to_string(),
                action: "create_machine".to_string(),
                facts: BTreeMap::new(),
                evidence: BTreeMap::from([("existing_machine_overlap_check".to_string(), true)]),
                expect: PolicyTestExpectation {
                    decision: DecisionKind::Allow,
                    rule_id: None,
                },
            }],
        };

        let report = engine().shadow_eval_suite(proposal(), &suite).unwrap();
        assert_eq!(
            report.candidate_rule_id,
            "machine_creation.requires_boundary"
        );
        assert_eq!(report.results.changed_cases, 1);
        assert_eq!(report.results.would_block_previously_accepted, 1);
        assert_eq!(report.recommendation, ShadowEvalRecommendation::Block);
    }

    #[test]
    fn policy_update_trace_requires_full_ordered_ceremony() {
        let trace = PolicyUpdateCeremonyTrace {
            schema_version: "larql.governance.policy_update_trace.v1".to_string(),
            ceremony_id: "policy_update_test".to_string(),
            policy_update_type: Some(PolicyUpdateType::NewRule),
            events: policy_update_required_phases()
                .iter()
                .map(|event_type| PolicyUpdateCeremonyEvent {
                    event_type: event_type.to_string(),
                })
                .collect(),
        };
        let report = validate_policy_update_ceremony_trace(&trace);
        assert!(report.passed);
    }

    #[test]
    fn policy_update_trace_blocks_rule_relaxation() {
        let trace = PolicyUpdateCeremonyTrace {
            schema_version: "larql.governance.policy_update_trace.v1".to_string(),
            ceremony_id: "policy_update_test".to_string(),
            policy_update_type: Some(PolicyUpdateType::RuleRelaxation),
            events: Vec::new(),
        };
        let report = validate_policy_update_ceremony_trace(&trace);
        assert!(!report.passed);
        assert!(report
            .errors
            .contains(&"rule relaxation must use PolicyWeakeningCeremony".to_string()));
    }

    #[test]
    fn policy_update_capability_is_minted_from_completed_approval() {
        let engine = engine();
        let proposal = proposal();
        let approval = PolicyApproval {
            schema_version: "larql.governance.policy_approval.v1".to_string(),
            event_type: "PolicyChangeApproved".to_string(),
            ceremony_id: "policy_update_test".to_string(),
            approver: "human:test".to_string(),
            approved_rule_id: proposal.proposed_rule.id.clone(),
            prior_policy_hash: engine.registry().policy_hash.clone(),
            prior_state_hash: crate::hash::hash_text("state"),
            conflict_analysis_passed: true,
            shadow_evaluation_completed: true,
            policy_tests_passed: true,
            replay_verified: true,
        };
        let capability = mint_policy_update_capability(
            engine.registry(),
            &proposal,
            &approval,
            "governance/policies/machine-runtime.toml",
            "2026-05-09T00:00:00Z",
        )
        .unwrap();
        assert_eq!(capability.capability_type, "PolicyUpdate");
        assert_eq!(
            capability.allowed_rule_ids,
            vec!["machine_creation.requires_boundary".to_string()]
        );
    }

    #[test]
    fn policy_update_capability_rejects_inactive_policy_file() {
        let engine = engine();
        let proposal = proposal();
        let approval = PolicyApproval {
            schema_version: "larql.governance.policy_approval.v1".to_string(),
            event_type: "PolicyChangeApproved".to_string(),
            ceremony_id: "policy_update_test".to_string(),
            approver: "human:test".to_string(),
            approved_rule_id: proposal.proposed_rule.id.clone(),
            prior_policy_hash: engine.registry().policy_hash.clone(),
            prior_state_hash: crate::hash::hash_text("state"),
            conflict_analysis_passed: true,
            shadow_evaluation_completed: true,
            policy_tests_passed: true,
            replay_verified: true,
        };
        let err = mint_policy_update_capability(
            engine.registry(),
            &proposal,
            &approval,
            "governance/policies/inactive.toml",
            "2026-05-09T00:00:00Z",
        )
        .unwrap_err();
        assert!(err.to_string().contains("active policy set"));
    }

    #[test]
    fn policy_weakening_requires_hash_backed_evidence() {
        let evidence = PolicyWeakeningEvidence {
            schema_version: "larql.governance.policy_weakening_evidence.v1".to_string(),
            ceremony_id: "policy_weakening_test".to_string(),
            authority_reduced: "machine_creation deny".to_string(),
            too_strict_rationale_hash: crate::hash::hash_text("rationale"),
            false_positive_example_hashes: vec![crate::hash::hash_text("false-positive")],
            hostile_path_regression_test_hash: crate::hash::hash_text("hostile-path"),
            rollback_path_hash: crate::hash::hash_text("rollback"),
            human_approval_hash: crate::hash::hash_text("approval"),
        };
        assert!(validate_policy_weakening_evidence(&evidence).is_ok());
    }

    #[test]
    fn merge_sorted_unique_merges_and_dedupes() {
        assert_eq!(
            merge_sorted_unique(&[1, 3, 5], &[2, 3, 6]),
            vec![1, 2, 3, 5, 6]
        );
        assert_eq!(merge_sorted_unique(&[], &[1, 2]), vec![1, 2]);
        assert_eq!(merge_sorted_unique(&[1, 2], &[]), vec![1, 2]);
    }

    #[test]
    fn dispatch_classifier_empty_when_is_global() {
        let when = ConditionBlock::default();
        assert!(matches!(
            compile_rule_dispatch_class(&when),
            RuleDispatchClass::Global
        ));
    }

    #[test]
    fn dispatch_classifier_single_action_bucket() {
        let when = ConditionBlock {
            all: vec![Condition {
                action: Some("a".into()),
                ..Condition::default()
            }],
            ..ConditionBlock::default()
        };
        assert!(matches!(
            compile_rule_dispatch_class(&when),
            RuleDispatchClass::ActionBucket(action) if action == "a"
        ));
    }

    #[test]
    fn dispatch_classifier_action_and_fact_still_one_action_bucket() {
        let when = ConditionBlock {
            all: vec![
                Condition {
                    action: Some("verify_governance_rule_profile".into()),
                    ..Condition::default()
                },
                Condition {
                    fact: Some("profile.x".into()),
                    equals: Some(FactValue::Bool(true)),
                    ..Condition::default()
                },
            ],
            ..ConditionBlock::default()
        };
        assert!(matches!(
            compile_rule_dispatch_class(&when),
            RuleDispatchClass::ActionBucket(action) if action == "verify_governance_rule_profile"
        ));
    }

    #[test]
    fn dispatch_classifier_fact_only_is_global() {
        let when = ConditionBlock {
            all: vec![Condition {
                fact: Some("public_api_changed".into()),
                equals: Some(FactValue::Bool(true)),
                ..Condition::default()
            }],
            ..ConditionBlock::default()
        };
        assert!(matches!(
            compile_rule_dispatch_class(&when),
            RuleDispatchClass::Global
        ));
    }

    #[test]
    fn dispatch_classifier_any_forces_global() {
        let when = ConditionBlock {
            any: vec![Condition {
                action: Some("a".into()),
                ..Condition::default()
            }],
            ..ConditionBlock::default()
        };
        assert!(matches!(
            compile_rule_dispatch_class(&when),
            RuleDispatchClass::Global
        ));
    }

    #[test]
    fn dispatch_classifier_not_forces_global() {
        let when = ConditionBlock {
            not_conditions: vec![Condition {
                action: Some("a".into()),
                ..Condition::default()
            }],
            ..ConditionBlock::default()
        };
        assert!(matches!(
            compile_rule_dispatch_class(&when),
            RuleDispatchClass::Global
        ));
    }

    #[test]
    fn dispatch_classifier_duplicate_action_literals_bucket() {
        let when = ConditionBlock {
            all: vec![
                Condition {
                    action: Some("create_machine".into()),
                    ..Condition::default()
                },
                Condition {
                    action: Some("create_machine".into()),
                    ..Condition::default()
                },
            ],
            ..ConditionBlock::default()
        };
        assert!(matches!(
            compile_rule_dispatch_class(&when),
            RuleDispatchClass::ActionBucket(action) if action == "create_machine"
        ));
    }

    #[test]
    fn dispatch_classifier_two_distinct_actions_is_global() {
        let when = ConditionBlock {
            all: vec![
                Condition {
                    action: Some("a".into()),
                    ..Condition::default()
                },
                Condition {
                    action: Some("b".into()),
                    ..Condition::default()
                },
            ],
            ..ConditionBlock::default()
        };
        assert!(matches!(
            compile_rule_dispatch_class(&when),
            RuleDispatchClass::Global
        ));
    }

    #[test]
    fn fact_map_last_duplicate_wins() {
        let input = PolicyInput {
            schema_version: "larql.governance.policy_input.v1".to_string(),
            state_hash: crate::hash::hash_text("dup-map"),
            actor: "test".into(),
            action: "noop".into(),
            target_paths: Vec::new(),
            evidence: Vec::new(),
            risk: 0,
            facts: Vec::new(),
        };

        let when = ConditionBlock {
            all: vec![Condition {
                fact: Some("x".into()),
                equals: Some(FactValue::Integer(2)),
                ..Condition::default()
            }],
            ..ConditionBlock::default()
        };

        let facts_ordered = vec![
            PolicyFact {
                fact_type: "x".into(),
                value: FactValue::Integer(1),
                subject: None,
                evidence: Vec::new(),
                source: None,
                state_hash: None,
            },
            PolicyFact {
                fact_type: "x".into(),
                value: FactValue::Integer(2),
                subject: None,
                evidence: Vec::new(),
                source: None,
                state_hash: None,
            },
        ];
        let map_last_wins = fact_map(&facts_ordered);
        assert!(
            evaluate_block(&when, &input, &map_last_wins).matched,
            "last duplicate wins for lookups"
        );

        let facts_both_old = vec![
            PolicyFact {
                fact_type: "x".into(),
                value: FactValue::Integer(1),
                subject: None,
                evidence: Vec::new(),
                source: None,
                state_hash: None,
            },
            PolicyFact {
                fact_type: "x".into(),
                value: FactValue::Integer(1),
                subject: None,
                evidence: Vec::new(),
                source: None,
                state_hash: None,
            },
        ];
        assert!(
            !evaluate_block(&when, &input, &fact_map(&facts_both_old)).matched,
            "when last duplicate mismatches expectation, guard should fail"
        );
    }

    #[test]
    fn compiled_eval_parity_matches_full_scan_for_engine_fixture() {
        let engine = engine();
        let inputs = vec![
            PolicyInput {
                schema_version: "larql.governance.policy_input.v1".to_string(),
                state_hash: crate::hash::hash_text("s1"),
                actor: "test".into(),
                action: "create_machine".into(),
                target_paths: Vec::new(),
                evidence: Vec::new(),
                risk: 0,
                facts: Vec::new(),
            },
            PolicyInput {
                schema_version: "larql.governance.policy_input.v1".to_string(),
                state_hash: crate::hash::hash_text("s2"),
                actor: "test".into(),
                action: "create_machine".into(),
                target_paths: Vec::new(),
                evidence: vec!["existing_machine_overlap_check".into()],
                risk: 0,
                facts: Vec::new(),
            },
            PolicyInput {
                schema_version: "larql.governance.policy_input.v1".to_string(),
                state_hash: crate::hash::hash_text("s3"),
                actor: "test".into(),
                action: "unknown_governed_action".into(),
                target_paths: Vec::new(),
                evidence: Vec::new(),
                risk: 0,
                facts: Vec::new(),
            },
        ];

        for input in inputs {
            let merged = engine.evaluate(&input);
            let full = engine.evaluate_full_scan_for_test(&input);
            assert_eq!(
                serde_json::to_value(&merged).unwrap(),
                serde_json::to_value(&full).unwrap(),
                "parity failure for action {}",
                input.action
            );
        }
    }

    #[test]
    fn unknown_action_eligible_paths_are_globals_only_when_bucket_missing() {
        let engine = engine();
        let globals = engine.merged_eligible_indices_for_action("__not_a_real_action__");
        assert!(
            globals.is_empty(),
            "fixture engine has only bucketed rules; unknown actions evaluate nothing beyond unknown_rule handling"
        );
    }

    fn try_repo_policy_engine() -> Option<PolicyEngine> {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let index_path = manifest.join("../../governance/policies/index.toml");
        if !index_path.exists() {
            return None;
        }
        let index_path = fs::canonicalize(index_path).ok()?;
        let registry = load_policy_registry(&index_path).ok()?;
        PolicyEngine::new(registry).ok()
    }

    #[test]
    fn repo_dispatch_soundness_skip_implies_when_unmatched() {
        let Some(engine) = try_repo_policy_engine() else {
            return;
        };

        let rules = &engine.registry().rules;

        let mut actions: BTreeSet<String> = BTreeSet::new();
        actions.insert("__synthetic_unknown__".into());
        actions.insert(String::new());
        for rule in rules {
            for condition in rule.when.all.iter() {
                if let Some(action) = &condition.action {
                    actions.insert(action.clone());
                }
            }
        }

        for action in actions {
            for risk in [0u64, 9] {
                for with_fact in [false, true] {
                    let facts = if with_fact {
                        vec![PolicyFact {
                            fact_type: "public_api_changed".into(),
                            value: FactValue::Bool(true),
                            subject: None,
                            evidence: Vec::new(),
                            source: Some("soundness-grid".into()),
                            state_hash: None,
                        }]
                    } else {
                        Vec::new()
                    };

                    let input = PolicyInput {
                        schema_version: "larql.governance.policy_input.v1".to_string(),
                        state_hash: crate::hash::hash_text(&format!(
                            "{}|{risk}|{with_fact}",
                            action.len()
                        )),
                        actor: "soundness".into(),
                        action: action.clone(),
                        target_paths: vec!["pkg/src/lib.rs".into()],
                        evidence: Vec::new(),
                        risk,
                        facts,
                    };

                    let eligible: BTreeSet<usize> = engine
                        .merged_eligible_indices_for_action(&input.action)
                        .into_iter()
                        .collect();
                    let fact_map_input = fact_map(&input.facts);

                    for idx in 0..rules.len() {
                        if !eligible.contains(&idx) {
                            let when_outcome =
                                evaluate_block(&rules[idx].when, &input, &fact_map_input);
                            assert!(
                                !when_outcome.matched,
                                "soundness violated for rule {} on action `{}` idx {idx}",
                                rules[idx].id, action
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn repo_policy_eval_parity_matches_full_scan_samples() {
        let Some(engine) = try_repo_policy_engine() else {
            return;
        };

        let samples = vec![
            PolicyInput {
                schema_version: "larql.governance.policy_input.v1".to_string(),
                state_hash: crate::hash::hash_text("repo-sample-1"),
                actor: "test".into(),
                action: "ci_governance_check".into(),
                target_paths: Vec::new(),
                evidence: vec![
                    "invariant_registry_verified".into(),
                    "decision_surface_validated".into(),
                ],
                risk: 0,
                facts: Vec::new(),
            },
            PolicyInput {
                schema_version: "larql.governance.policy_input.v1".to_string(),
                state_hash: crate::hash::hash_text("repo-sample-2"),
                actor: "test".into(),
                action: "verify_governance_rule_profile".into(),
                target_paths: Vec::new(),
                evidence: vec!["profile_forbidden_authority_review".into()],
                risk: 0,
                facts: vec![PolicyFact {
                    fact_type: "profile.allow_machine_edit_arbitrary_files_freely".into(),
                    value: FactValue::Bool(true),
                    subject: None,
                    evidence: Vec::new(),
                    source: None,
                    state_hash: None,
                }],
            },
        ];

        for input in samples {
            let merged_path = engine.evaluate(&input);
            let full_scan = engine.evaluate_full_scan_for_test(&input);
            assert_eq!(
                serde_json::to_value(&merged_path).unwrap(),
                serde_json::to_value(&full_scan).unwrap(),
            );
        }
    }
}
