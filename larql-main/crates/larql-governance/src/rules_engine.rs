use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
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

pub struct PolicyEngine {
    registry: PolicyRegistry,
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
        let pack: PolicyPackFile = toml::from_str(text).map_err(|source| PolicyLoadError::Toml {
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
        Ok(Self { registry })
    }

    pub fn registry(&self) -> &PolicyRegistry {
        &self.registry
    }

    pub fn evaluate(&self, input: &PolicyInput) -> PolicyEngineDecision {
        let facts = fact_map(&input.facts);
        let mut decision = DecisionKind::Allow;
        let mut matched_rules = Vec::new();
        let mut required_actions = BTreeSet::new();
        let mut evidence = BTreeSet::new();
        let mut findings = Vec::new();

        for rule in &self.registry.rules {
            let when = evaluate_block(&rule.when, input, &facts);
            if !when.matched {
                continue;
            }
            matched_rules.push(rule.id.clone());
            let require = evaluate_block(&rule.require, input, &facts);
            if require.matched {
                if rule.severity == PolicySeverity::Warn {
                    decision = most_restrictive(decision, DecisionKind::AllowWithWarnings);
                }
                for item in require.evidence {
                    evidence.insert(item);
                }
                continue;
            }

            decision = most_restrictive(decision, rule.decision.on_missing.clone());
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

    Ok(PolicyCapability {
        capability_type: "PolicyUpdate".to_string(),
        allowed_policy_files: vec![target_policy_file.into()],
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
struct ConditionOutcome {
    matched: bool,
    missing_facts: Vec<String>,
    missing_evidence: Vec<String>,
    evidence: Vec<String>,
}

fn fact_map(facts: &[PolicyFact]) -> BTreeMap<String, &PolicyFact> {
    facts
        .iter()
        .map(|fact| (fact.fact_type.clone(), fact))
        .collect()
}

fn evaluate_block(
    block: &ConditionBlock,
    input: &PolicyInput,
    facts: &BTreeMap<String, &PolicyFact>,
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
    facts: &BTreeMap<String, &PolicyFact>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn engine() -> PolicyEngine {
        PolicyEngine::new(PolicyRegistry {
            schema_version: "larql.governance.policy_registry.v1".to_string(),
            active_policy_set: PolicySet {
                id: "test".to_string(),
                version: "1".to_string(),
                includes: Vec::new(),
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
        let from_material = load_policy_registry_from_material(
            "repo_index.toml",
            &index_text,
            &packs,
        )
        .unwrap();
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
}
