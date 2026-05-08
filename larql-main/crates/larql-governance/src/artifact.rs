use crate::policy::{require_hash, require_hashes, PolicyDecision, PolicyError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GovernanceArtifactRole {
    Machine,
    NonMachine,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorityOperation {
    GovernedTransition,
    StableDeclarativeConstraint,
    EvidenceRecord,
    TestFixture,
    ExampleRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NonMachineArtifactKind {
    PolicyFile,
    CeremonyDefinition,
    Schema,
    Fixture,
    TestCase,
    DocAdr,
    Manifest,
    KnownLocationHashes,
    GeneratedReceipt,
    EventLog,
    TrainingExample,
    ProposalExample,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GovernanceArtifactDefinition {
    pub name: String,
    pub role: GovernanceArtifactRole,
    pub purpose_hash: String,
    pub input_schema_hash: Option<String>,
    pub output_schema_hash: Option<String>,
    #[serde(default)]
    pub consumes_evidence_hashes: Vec<String>,
    #[serde(default)]
    pub emits_evidence_hashes: Vec<String>,
    #[serde(default)]
    pub test_hashes: Vec<String>,
    pub deterministic: bool,
    pub ledger_visible_effects: bool,
    pub non_machine_kind: Option<NonMachineArtifactKind>,
}

pub fn validate_authority_operation(
    role: &GovernanceArtifactRole,
    operation: &AuthorityOperation,
) -> Result<PolicyDecision, PolicyError> {
    match (role, operation) {
        (GovernanceArtifactRole::Machine, AuthorityOperation::StableDeclarativeConstraint) => {
            Err(PolicyError::Rejected {
                field: "authority_operation",
                reason:
                    "machines apply constraints; non-machines define stable declarative constraints",
            })
        }
        (GovernanceArtifactRole::NonMachine, AuthorityOperation::GovernedTransition) => {
            Err(PolicyError::Rejected {
                field: "authority_operation",
                reason: "only machines may perform governed transitions",
            })
        }
        _ => Ok(PolicyDecision::passed("artifact-authority")),
    }
}

pub fn validate_governance_artifact_definition(
    definition: &GovernanceArtifactDefinition,
) -> Result<PolicyDecision, PolicyError> {
    if definition.name.trim().is_empty() {
        return Err(PolicyError::Rejected {
            field: "name",
            reason: "artifact definition requires a stable name",
        });
    }
    require_hash("purpose_hash", &definition.purpose_hash)?;

    match definition.role {
        GovernanceArtifactRole::Machine => validate_machine_definition(definition)?,
        GovernanceArtifactRole::NonMachine => validate_non_machine_definition(definition)?,
    }

    Ok(PolicyDecision::passed("artifact-definition"))
}

fn validate_machine_definition(
    definition: &GovernanceArtifactDefinition,
) -> Result<(), PolicyError> {
    require_hash(
        "input_schema_hash",
        definition
            .input_schema_hash
            .as_deref()
            .ok_or(PolicyError::Rejected {
                field: "input_schema_hash",
                reason: "machine requires typed input schema hash",
            })?,
    )?;
    require_hash(
        "output_schema_hash",
        definition
            .output_schema_hash
            .as_deref()
            .ok_or(PolicyError::Rejected {
                field: "output_schema_hash",
                reason: "machine requires typed output schema hash",
            })?,
    )?;
    require_hashes(
        "consumes_evidence_hashes",
        &definition.consumes_evidence_hashes,
    )?;
    require_hashes("emits_evidence_hashes", &definition.emits_evidence_hashes)?;
    require_hashes("test_hashes", &definition.test_hashes)?;

    if definition.non_machine_kind.is_some() {
        return Err(PolicyError::Rejected {
            field: "non_machine_kind",
            reason: "machine cannot carry non-machine artifact kind",
        });
    }

    Ok(())
}

fn validate_non_machine_definition(
    definition: &GovernanceArtifactDefinition,
) -> Result<(), PolicyError> {
    if definition.non_machine_kind.is_none() {
        return Err(PolicyError::Rejected {
            field: "non_machine_kind",
            reason: "non-machine requires declarative artifact kind",
        });
    }
    if definition.ledger_visible_effects {
        return Err(PolicyError::Rejected {
            field: "ledger_visible_effects",
            reason: "non-machines may be authoritative but are not active",
        });
    }
    if definition.input_schema_hash.is_some() || definition.output_schema_hash.is_some() {
        return Err(PolicyError::Rejected {
            field: "schema_hash",
            reason: "non-machines define or record constraints; they do not own executable input/output contracts",
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash_ref(label: &str) -> String {
        crate::hash::hash_text(label)
    }

    fn machine_definition() -> GovernanceArtifactDefinition {
        GovernanceArtifactDefinition {
            name: "policy_checker".to_string(),
            role: GovernanceArtifactRole::Machine,
            purpose_hash: hash_ref("purpose"),
            input_schema_hash: Some(hash_ref("input")),
            output_schema_hash: Some(hash_ref("output")),
            consumes_evidence_hashes: vec![hash_ref("consumes")],
            emits_evidence_hashes: vec![hash_ref("emits")],
            test_hashes: vec![hash_ref("tests")],
            deterministic: true,
            ledger_visible_effects: true,
            non_machine_kind: None,
        }
    }

    #[test]
    fn machine_definition_requires_active_contract() {
        assert!(validate_governance_artifact_definition(&machine_definition()).is_ok());
    }

    #[test]
    fn non_machine_cannot_perform_transition() {
        let err = validate_authority_operation(
            &GovernanceArtifactRole::NonMachine,
            &AuthorityOperation::GovernedTransition,
        )
        .unwrap_err();
        assert!(err.to_string().contains("only machines"));
    }

    #[test]
    fn machine_cannot_define_stable_declarative_constraint() {
        let err = validate_authority_operation(
            &GovernanceArtifactRole::Machine,
            &AuthorityOperation::StableDeclarativeConstraint,
        )
        .unwrap_err();
        assert!(err.to_string().contains("non-machines define"));
    }

    #[test]
    fn non_machine_definition_is_declarative_only() {
        let definition = GovernanceArtifactDefinition {
            name: "machine_creation.toml".to_string(),
            role: GovernanceArtifactRole::NonMachine,
            purpose_hash: hash_ref("purpose"),
            input_schema_hash: None,
            output_schema_hash: None,
            consumes_evidence_hashes: Vec::new(),
            emits_evidence_hashes: Vec::new(),
            test_hashes: Vec::new(),
            deterministic: true,
            ledger_visible_effects: false,
            non_machine_kind: Some(NonMachineArtifactKind::PolicyFile),
        };

        assert!(validate_governance_artifact_definition(&definition).is_ok());
    }

    #[test]
    fn non_machine_definition_rejects_ledger_effects() {
        let mut definition = machine_definition();
        definition.role = GovernanceArtifactRole::NonMachine;
        definition.non_machine_kind = Some(NonMachineArtifactKind::Schema);

        let err = validate_governance_artifact_definition(&definition).unwrap_err();
        assert!(err.to_string().contains("not active"));
    }
}
