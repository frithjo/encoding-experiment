use crate::artifact::{
    validate_authority_operation, validate_governance_artifact_definition, AuthorityOperation,
    GovernanceArtifactDefinition, GovernanceArtifactRole,
};
use crate::policy::{require_hash, require_hashes, PolicyDecision, PolicyError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GovernanceRuleProfile {
    pub name: String,
    pub allow_machine_edit_arbitrary_files_freely: bool,
    pub allow_machine_infer_field_types_from_prose_without_review: bool,
    pub allow_machine_generate_unsafe_code: bool,
    pub allow_machine_generate_repr_c_without_justification: bool,
    pub allow_machine_overwrite_hand_written_code: bool,
    pub allow_machine_auto_add_dependencies: bool,
    pub allow_machine_auto_commit_without_receipt: bool,
    pub allow_machine_modify_public_api_without_test_doc_policy: bool,
    pub allow_machine_define_stable_declarative_constraints: bool,
    pub allow_non_machine_perform_governed_transitions: bool,
    pub allow_unregistered_invariants: bool,
    pub allow_ci_without_invariant_registry: bool,
    pub allow_raw_prose_in_admitted_records: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GovernanceRulesEngine {
    pub profile: GovernanceRuleProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GovernanceInvariantRegistry {
    pub schema_version: String,
    pub registry_name: String,
    pub admission_policy_hash: String,
    pub ci_gate: InvariantCiGate,
    #[serde(default)]
    pub invariants: Vec<GovernanceInvariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvariantCiGate {
    pub name: String,
    pub command_hash: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GovernanceInvariant {
    pub id: String,
    pub subject: InvariantSubject,
    pub status: InvariantStatus,
    pub statement_hash: String,
    pub source_hashes: Vec<String>,
    pub evidence_hashes: Vec<String>,
    pub admission_receipt_hash: String,
    pub ci_gate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum InvariantSubject {
    Machine,
    NonMachine,
    MachinesAndNonMachines,
    Ci,
    Repository,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum InvariantStatus {
    Proposed,
    Admitted,
    Deprecated,
}

impl GovernanceRuleProfile {
    pub fn default_ci() -> Self {
        Self {
            name: "default-ci-rules".to_string(),
            allow_machine_edit_arbitrary_files_freely: false,
            allow_machine_infer_field_types_from_prose_without_review: false,
            allow_machine_generate_unsafe_code: false,
            allow_machine_generate_repr_c_without_justification: false,
            allow_machine_overwrite_hand_written_code: false,
            allow_machine_auto_add_dependencies: false,
            allow_machine_auto_commit_without_receipt: false,
            allow_machine_modify_public_api_without_test_doc_policy: false,
            allow_machine_define_stable_declarative_constraints: false,
            allow_non_machine_perform_governed_transitions: false,
            allow_unregistered_invariants: false,
            allow_ci_without_invariant_registry: false,
            allow_raw_prose_in_admitted_records: false,
        }
    }
}

impl GovernanceRulesEngine {
    pub fn new(profile: GovernanceRuleProfile) -> Result<Self, PolicyError> {
        validate_governance_rule_profile(&profile)?;
        Ok(Self { profile })
    }

    pub fn default_ci() -> Self {
        Self {
            profile: GovernanceRuleProfile::default_ci(),
        }
    }

    pub fn verify_profile(&self) -> Result<PolicyDecision, PolicyError> {
        validate_governance_rule_profile(&self.profile)
    }

    pub fn validate_artifact_definition(
        &self,
        definition: &GovernanceArtifactDefinition,
    ) -> Result<PolicyDecision, PolicyError> {
        validate_governance_artifact_definition(definition)
    }

    pub fn validate_authority_operation(
        &self,
        role: &GovernanceArtifactRole,
        operation: &AuthorityOperation,
    ) -> Result<PolicyDecision, PolicyError> {
        validate_authority_operation(role, operation)
    }

    pub fn validate_invariant_registry(
        &self,
        registry: &GovernanceInvariantRegistry,
    ) -> Result<PolicyDecision, PolicyError> {
        validate_invariant_registry_with_profile(registry, &self.profile)
    }
}

pub fn validate_governance_rule_profile(
    profile: &GovernanceRuleProfile,
) -> Result<PolicyDecision, PolicyError> {
    if profile.name.trim().is_empty() {
        return Err(PolicyError::Rejected {
            field: "name",
            reason: "rules profile requires a stable name",
        });
    }

    let checks = [
        (
            profile.allow_machine_edit_arbitrary_files_freely,
            "allow_machine_edit_arbitrary_files_freely",
        ),
        (
            profile.allow_machine_infer_field_types_from_prose_without_review,
            "allow_machine_infer_field_types_from_prose_without_review",
        ),
        (
            profile.allow_machine_generate_unsafe_code,
            "allow_machine_generate_unsafe_code",
        ),
        (
            profile.allow_machine_generate_repr_c_without_justification,
            "allow_machine_generate_repr_c_without_justification",
        ),
        (
            profile.allow_machine_overwrite_hand_written_code,
            "allow_machine_overwrite_hand_written_code",
        ),
        (
            profile.allow_machine_auto_add_dependencies,
            "allow_machine_auto_add_dependencies",
        ),
        (
            profile.allow_machine_auto_commit_without_receipt,
            "allow_machine_auto_commit_without_receipt",
        ),
        (
            profile.allow_machine_modify_public_api_without_test_doc_policy,
            "allow_machine_modify_public_api_without_test_doc_policy",
        ),
        (
            profile.allow_machine_define_stable_declarative_constraints,
            "allow_machine_define_stable_declarative_constraints",
        ),
        (
            profile.allow_non_machine_perform_governed_transitions,
            "allow_non_machine_perform_governed_transitions",
        ),
        (
            profile.allow_unregistered_invariants,
            "allow_unregistered_invariants",
        ),
        (
            profile.allow_ci_without_invariant_registry,
            "allow_ci_without_invariant_registry",
        ),
        (
            profile.allow_raw_prose_in_admitted_records,
            "allow_raw_prose_in_admitted_records",
        ),
    ];

    for (enabled, rule) in checks {
        if enabled {
            return Err(PolicyError::Rejected {
                field: rule,
                reason: "governance rules engine forbids this authority",
            });
        }
    }

    Ok(PolicyDecision::passed(&profile.name))
}

pub fn validate_invariant_registry(
    registry: &GovernanceInvariantRegistry,
) -> Result<PolicyDecision, PolicyError> {
    validate_invariant_registry_with_profile(registry, &GovernanceRuleProfile::default_ci())
}

pub fn validate_invariant_registry_with_profile(
    registry: &GovernanceInvariantRegistry,
    profile: &GovernanceRuleProfile,
) -> Result<PolicyDecision, PolicyError> {
    validate_governance_rule_profile(profile)?;
    if registry.schema_version != "larql.governance.invariant_registry.v1" {
        return Err(PolicyError::Rejected {
            field: "schema_version",
            reason: "unsupported invariant registry schema",
        });
    }
    if registry.registry_name.trim().is_empty() {
        return Err(PolicyError::Rejected {
            field: "registry_name",
            reason: "invariant registry requires a stable name",
        });
    }
    require_hash("admission_policy_hash", &registry.admission_policy_hash)?;
    validate_ci_gate(&registry.ci_gate)?;

    let mut ids = BTreeSet::new();
    for invariant in &registry.invariants {
        validate_invariant_admission(invariant)?;
        if !ids.insert(invariant.id.clone()) {
            return Err(PolicyError::Rejected {
                field: "invariants.id",
                reason: "invariant ids must be unique",
            });
        }
    }

    Ok(PolicyDecision::passed(&profile.name))
}

pub fn validate_invariant_admission(
    invariant: &GovernanceInvariant,
) -> Result<PolicyDecision, PolicyError> {
    if invariant.id.trim().is_empty() {
        return Err(PolicyError::Rejected {
            field: "invariants.id",
            reason: "invariant requires a stable id",
        });
    }
    require_hash("invariants.statement_hash", &invariant.statement_hash)?;
    require_hashes("invariants.source_hashes", &invariant.source_hashes)?;
    require_hashes("invariants.evidence_hashes", &invariant.evidence_hashes)?;
    require_hash(
        "invariants.admission_receipt_hash",
        &invariant.admission_receipt_hash,
    )?;

    if invariant.status == InvariantStatus::Admitted && !invariant.ci_gate {
        return Err(PolicyError::Rejected {
            field: "invariants.ci_gate",
            reason: "admitted invariants must be part of the CI gate",
        });
    }

    Ok(PolicyDecision::passed("invariant-admission"))
}

pub fn validate_ci_gate(gate: &InvariantCiGate) -> Result<PolicyDecision, PolicyError> {
    if gate.name.trim().is_empty() {
        return Err(PolicyError::Rejected {
            field: "ci_gate.name",
            reason: "CI gate requires a stable name",
        });
    }
    require_hash("ci_gate.command_hash", &gate.command_hash)?;
    if !gate.required {
        return Err(PolicyError::Rejected {
            field: "ci_gate.required",
            reason: "invariant registry CI gate must be required",
        });
    }
    Ok(PolicyDecision::passed("ci-gate"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash_ref(label: &str) -> String {
        crate::hash::hash_text(label)
    }

    fn admitted_invariant(id: &str) -> GovernanceInvariant {
        GovernanceInvariant {
            id: id.to_string(),
            subject: InvariantSubject::MachinesAndNonMachines,
            status: InvariantStatus::Admitted,
            statement_hash: hash_ref("statement"),
            source_hashes: vec![hash_ref("source")],
            evidence_hashes: vec![hash_ref("evidence")],
            admission_receipt_hash: hash_ref("receipt"),
            ci_gate: true,
        }
    }

    fn registry() -> GovernanceInvariantRegistry {
        GovernanceInvariantRegistry {
            schema_version: "larql.governance.invariant_registry.v1".to_string(),
            registry_name: "repo-invariants".to_string(),
            admission_policy_hash: hash_ref("policy"),
            ci_gate: InvariantCiGate {
                name: "governance-rules".to_string(),
                command_hash: hash_ref("cargo run -p larql-cli -- machine rules eval"),
                required: true,
            },
            invariants: vec![admitted_invariant("machines-and-non-machines")],
        }
    }

    #[test]
    fn default_ci_profile_passes() {
        let profile = GovernanceRuleProfile::default_ci();
        assert!(validate_governance_rule_profile(&profile).is_ok());
    }

    #[test]
    fn default_ci_profile_rejects_forbidden_authority() {
        let mut profile = GovernanceRuleProfile::default_ci();
        profile.allow_machine_generate_unsafe_code = true;
        assert!(validate_governance_rule_profile(&profile).is_err());
    }

    #[test]
    fn rules_engine_loads_valid_profile() {
        let profile = GovernanceRuleProfile::default_ci();
        let engine = GovernanceRulesEngine::new(profile).unwrap();
        assert_eq!(engine.profile.name, "default-ci-rules");
    }

    #[test]
    fn registry_requires_admitted_invariants_in_ci() {
        let mut registry = registry();
        registry.invariants[0].ci_gate = false;
        assert!(validate_invariant_registry(&registry).is_err());
    }

    #[test]
    fn registry_passes_with_hash_backed_invariant() {
        assert!(validate_invariant_registry(&registry()).is_ok());
    }
}
