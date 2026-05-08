use crate::hash::is_hash_ref;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("policy rejected {field}: {reason}")]
    Rejected {
        field: &'static str,
        reason: &'static str,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyDecision {
    pub status: String,
    pub profile: String,
    pub denied_rule: Option<String>,
}

impl PolicyDecision {
    pub fn passed(profile: impl Into<String>) -> Self {
        Self {
            status: "passed".to_string(),
            profile: profile.into(),
            denied_rule: None,
        }
    }

    pub fn denied(profile: impl Into<String>, rule: impl Into<String>) -> Self {
        Self {
            status: "denied".to_string(),
            profile: profile.into(),
            denied_rule: Some(rule.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MachineRuleProfile {
    pub name: String,
    pub edit_arbitrary_files_freely: bool,
    pub infer_field_types_from_prose_without_review: bool,
    pub generate_unsafe_code: bool,
    pub generate_repr_c_without_justification: bool,
    pub overwrite_hand_written_code: bool,
    pub auto_add_dependencies: bool,
    pub auto_commit_without_receipt: bool,
    pub modify_public_api_without_test_doc_policy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MachinePolicyEngine {
    pub profile: MachineRuleProfile,
}

impl MachineRuleProfile {
    pub fn default_starter() -> Self {
        Self {
            name: "default-starter".to_string(),
            edit_arbitrary_files_freely: false,
            infer_field_types_from_prose_without_review: false,
            generate_unsafe_code: false,
            generate_repr_c_without_justification: false,
            overwrite_hand_written_code: false,
            auto_add_dependencies: false,
            auto_commit_without_receipt: false,
            modify_public_api_without_test_doc_policy: false,
        }
    }
}

impl MachinePolicyEngine {
    pub fn new(profile: MachineRuleProfile) -> Result<Self, PolicyError> {
        validate_machine_profile(&profile)?;
        Ok(Self { profile })
    }

    pub fn default_starter() -> Self {
        Self {
            profile: MachineRuleProfile::default_starter(),
        }
    }

    pub fn verify_profile(&self) -> Result<PolicyDecision, PolicyError> {
        validate_machine_profile(&self.profile)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StructClassifications {
    pub internal_state: bool,
    pub public_api: bool,
    pub serialized: bool,
    pub persisted: bool,
    pub ffi_boundary: bool,
    pub authority_bearing: bool,
    pub stable_contract: bool,
}

pub fn validate_machine_profile(
    profile: &MachineRuleProfile,
) -> Result<PolicyDecision, PolicyError> {
    let checks = [
        (
            profile.edit_arbitrary_files_freely,
            "edit_arbitrary_files_freely",
        ),
        (
            profile.infer_field_types_from_prose_without_review,
            "infer_field_types_from_prose_without_review",
        ),
        (profile.generate_unsafe_code, "generate_unsafe_code"),
        (
            profile.generate_repr_c_without_justification,
            "generate_repr_c_without_justification",
        ),
        (
            profile.overwrite_hand_written_code,
            "overwrite_hand_written_code",
        ),
        (profile.auto_add_dependencies, "auto_add_dependencies"),
        (
            profile.auto_commit_without_receipt,
            "auto_commit_without_receipt",
        ),
        (
            profile.modify_public_api_without_test_doc_policy,
            "modify_public_api_without_test_doc_policy",
        ),
    ];

    for (enabled, rule) in checks {
        if enabled {
            return Err(PolicyError::Rejected {
                field: rule,
                reason: "starter profile forbids this authority",
            });
        }
    }

    Ok(PolicyDecision::passed(&profile.name))
}

pub fn require_hash(field: &'static str, value: &str) -> Result<(), PolicyError> {
    if is_hash_ref(value) {
        Ok(())
    } else {
        Err(PolicyError::Rejected {
            field,
            reason: "admitted records must carry hash references, not raw prose",
        })
    }
}

pub fn require_hashes(field: &'static str, values: &[String]) -> Result<(), PolicyError> {
    if values.is_empty() {
        return Err(PolicyError::Rejected {
            field,
            reason: "at least one hash reference is required",
        });
    }
    for value in values {
        require_hash(field, value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_profile_passes() {
        let profile = MachineRuleProfile::default_starter();
        assert!(validate_machine_profile(&profile).is_ok());
    }

    #[test]
    fn starter_profile_rejects_forbidden_authority() {
        let mut profile = MachineRuleProfile::default_starter();
        profile.generate_unsafe_code = true;
        assert!(validate_machine_profile(&profile).is_err());
    }

    #[test]
    fn policy_engine_loads_valid_profile() {
        let profile = MachineRuleProfile::default_starter();
        let engine = MachinePolicyEngine::new(profile).unwrap();
        assert_eq!(engine.profile.name, "default-starter");
    }
}
