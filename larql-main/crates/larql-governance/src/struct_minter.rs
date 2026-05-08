use crate::hash::{hash_json, hash_text};
use crate::policy::{
    require_hash, require_hashes, MachinePolicyEngine, MachineRuleProfile, PolicyDecision,
    PolicyError, StructClassifications,
};
use crate::receipt::MintReceipt;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path};
use syn::parse_str;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StructMinterError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Policy(#[from] PolicyError),
    #[error("invalid Rust identifier in {field}: {value}")]
    Identifier { field: &'static str, value: String },
    #[error("invalid Rust type expression in field {field}: {message}")]
    TypeExpr { field: String, message: String },
    #[error("invalid target path: {0}")]
    TargetPath(String),
    #[error("generated Rust item did not parse: {0}")]
    GeneratedItem(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StructSpec {
    pub schema_version: String,
    pub item: StructItemSpec,
    #[serde(default)]
    pub derives: Vec<String>,
    pub layout: LayoutSpec,
    pub fields: Vec<FieldSpec>,
    pub governance: GovernanceSpec,
    #[serde(default)]
    pub dependency_additions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StructItemSpec {
    pub name: String,
    pub visibility: String,
    pub target_path: String,
    pub module_path: Vec<String>,
    pub integration_mode: IntegrationMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LayoutSpec {
    pub repr: String,
    pub repr_c_justification_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldSpec {
    pub name: String,
    #[serde(rename = "type")]
    pub type_expr: String,
    pub visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GovernanceSpec {
    pub principle_hash: String,
    pub invariant_hash: String,
    pub type_need_hash: String,
    pub evidence_hashes: Vec<String>,
    pub classifications: StructClassifications,
    pub public_api_policy_hash: Option<String>,
    pub test_policy_hash: Option<String>,
    pub doc_policy_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IntegrationMode {
    PatchOnly,
    GeneratedFile,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CargoCheckStatus {
    NotRun,
    NotApplicablePatchOnly,
    Passed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MintChecks {
    pub spec: String,
    pub generated_item_parse: String,
    pub policy: PolicyDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MintOutcome {
    pub generated_source: String,
    pub receipt: MintReceipt,
}

pub fn verify_struct_spec(spec: &StructSpec) -> Result<PolicyDecision, StructMinterError> {
    verify_struct_spec_with_profile(spec, &MachineRuleProfile::default_starter())
}

pub fn verify_struct_spec_with_profile(
    spec: &StructSpec,
    profile: &MachineRuleProfile,
) -> Result<PolicyDecision, StructMinterError> {
    if spec.schema_version != "larql.governance.struct_spec.v1" {
        return Err(PolicyError::Rejected {
            field: "schema_version",
            reason: "unsupported struct spec schema",
        }
        .into());
    }

    let engine = MachinePolicyEngine::new(profile.clone())?;
    engine.verify_profile()?;
    validate_identifier("item.name", &spec.item.name)?;
    validate_visibility("item.visibility", &spec.item.visibility)?;
    validate_target_path(&spec.item.target_path, &spec.item.integration_mode)?;
    validate_layout(&spec.layout)?;
    require_hash("governance.principle_hash", &spec.governance.principle_hash)?;
    require_hash("governance.invariant_hash", &spec.governance.invariant_hash)?;
    require_hash("governance.type_need_hash", &spec.governance.type_need_hash)?;
    require_hashes(
        "governance.evidence_hashes",
        &spec.governance.evidence_hashes,
    )?;
    validate_classification_policy(spec)?;

    if !spec.dependency_additions.is_empty() {
        return Err(PolicyError::Rejected {
            field: "dependency_additions",
            reason: "starter profile forbids auto-added dependencies",
        }
        .into());
    }

    if spec.fields.is_empty() {
        return Err(PolicyError::Rejected {
            field: "fields",
            reason: "struct minting requires explicit reviewed fields",
        }
        .into());
    }

    for field in &spec.fields {
        validate_identifier("field.name", &field.name)?;
        validate_visibility("field.visibility", &field.visibility)?;
        if field.type_expr.contains("unsafe") {
            return Err(PolicyError::Rejected {
                field: "field.type",
                reason: "starter profile forbids unsafe code",
            }
            .into());
        }
        parse_str::<syn::Type>(&field.type_expr).map_err(|err| StructMinterError::TypeExpr {
            field: field.name.clone(),
            message: err.to_string(),
        })?;
    }

    for derive in &spec.derives {
        validate_identifier("derive", derive)?;
    }

    Ok(PolicyDecision::passed(&engine.profile.name))
}

pub fn mint_struct(spec: &StructSpec) -> Result<MintOutcome, StructMinterError> {
    mint_struct_with_profile(spec, &MachineRuleProfile::default_starter())
}

pub fn mint_struct_with_profile(
    spec: &StructSpec,
    profile: &MachineRuleProfile,
) -> Result<MintOutcome, StructMinterError> {
    let policy = verify_struct_spec_with_profile(spec, profile)?;
    let generated_source = render_struct(spec);
    parse_str::<syn::ItemStruct>(&generated_source)
        .map_err(|err| StructMinterError::GeneratedItem(err.to_string()))?;
    let spec_sha256 = hash_json(spec)?;
    let generated_item_sha256 = hash_text(&generated_source);
    let checks = MintChecks {
        spec: "passed".to_string(),
        generated_item_parse: "passed".to_string(),
        policy,
    };
    let cargo_check = match spec.item.integration_mode {
        IntegrationMode::PatchOnly => CargoCheckStatus::NotApplicablePatchOnly,
        IntegrationMode::GeneratedFile => CargoCheckStatus::NotRun,
    };
    let receipt = MintReceipt {
        schema_version: "larql.governance.struct_mint.receipt.v1".to_string(),
        event_type: "StructMinted".to_string(),
        machine: "struct_minter".to_string(),
        spec_sha256,
        type_need_hash: spec.governance.type_need_hash.clone(),
        evidence_hashes: spec.governance.evidence_hashes.clone(),
        generated_item_sha256,
        target_path: spec.item.target_path.clone(),
        integration_mode: spec.item.integration_mode.clone(),
        checks,
        cargo_check,
    };

    Ok(MintOutcome {
        generated_source,
        receipt,
    })
}

fn validate_classification_policy(spec: &StructSpec) -> Result<(), PolicyError> {
    let class = &spec.governance.classifications;
    if spec.item.visibility == "pub" && !class.public_api {
        return Err(PolicyError::Rejected {
            field: "governance.classifications.public_api",
            reason: "public visibility requires public API classification",
        });
    }
    if class.public_api {
        require_hash(
            "governance.public_api_policy_hash",
            spec.governance
                .public_api_policy_hash
                .as_deref()
                .unwrap_or_default(),
        )?;
        require_hash(
            "governance.test_policy_hash",
            spec.governance
                .test_policy_hash
                .as_deref()
                .unwrap_or_default(),
        )?;
        require_hash(
            "governance.doc_policy_hash",
            spec.governance
                .doc_policy_hash
                .as_deref()
                .unwrap_or_default(),
        )?;
    }
    if class.ffi_boundary && spec.layout.repr != "C" {
        return Err(PolicyError::Rejected {
            field: "layout.repr",
            reason: "FFI boundary requires explicit repr C policy",
        });
    }
    Ok(())
}

fn validate_layout(layout: &LayoutSpec) -> Result<(), PolicyError> {
    match layout.repr.as_str() {
        "Rust" => Ok(()),
        "C" => {
            let value = layout
                .repr_c_justification_hash
                .as_deref()
                .unwrap_or_default();
            require_hash("layout.repr_c_justification_hash", value)
        }
        _ => Err(PolicyError::Rejected {
            field: "layout.repr",
            reason: "only Rust and C layouts are supported",
        }),
    }
}

fn validate_visibility(field: &'static str, value: &str) -> Result<(), StructMinterError> {
    match value {
        "pub" | "private" => Ok(()),
        _ => Err(StructMinterError::Identifier {
            field,
            value: value.to_string(),
        }),
    }
}

fn validate_identifier(field: &'static str, value: &str) -> Result<(), StructMinterError> {
    parse_str::<syn::Ident>(value)
        .map(|_| ())
        .map_err(|_| StructMinterError::Identifier {
            field,
            value: value.to_string(),
        })
}

fn validate_target_path(path: &str, mode: &IntegrationMode) -> Result<(), StructMinterError> {
    let path_ref = Path::new(path);
    if path_ref.is_absolute() {
        return Err(StructMinterError::TargetPath(
            "target path must be repo-relative".to_string(),
        ));
    }
    if path_ref
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return Err(StructMinterError::TargetPath(
            "target path cannot escape the repo".to_string(),
        ));
    }
    if path_ref.extension().and_then(|ext| ext.to_str()) != Some("rs") {
        return Err(StructMinterError::TargetPath(
            "target path must be a Rust source file".to_string(),
        ));
    }
    if matches!(mode, IntegrationMode::GeneratedFile) && !is_generated_path(path_ref) {
        return Err(StructMinterError::TargetPath(
            "generated-file mode requires a generated path".to_string(),
        ));
    }
    Ok(())
}

fn is_generated_path(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(value) => value == "generated",
        _ => false,
    }) || path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("governed_"))
}

fn render_struct(spec: &StructSpec) -> String {
    let mut out = String::new();
    if !spec.derives.is_empty() {
        out.push_str("#[derive(");
        out.push_str(&spec.derives.join(", "));
        out.push_str(")]\n");
    }
    if spec.layout.repr == "C" {
        out.push_str("#[repr(C)]\n");
    }
    if spec.item.visibility == "pub" {
        out.push_str("pub ");
    }
    out.push_str("struct ");
    out.push_str(&spec.item.name);
    out.push_str(" {\n");
    for field in &spec.fields {
        out.push_str("    ");
        if field.visibility == "pub" {
            out.push_str("pub ");
        }
        out.push_str(&field.name);
        out.push_str(": ");
        out.push_str(&field.type_expr);
        out.push_str(",\n");
    }
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::hash_text;

    fn valid_spec() -> StructSpec {
        let h = hash_text("evidence");
        StructSpec {
            schema_version: "larql.governance.struct_spec.v1".to_string(),
            item: StructItemSpec {
                name: "ExecutionToken".to_string(),
                visibility: "pub".to_string(),
                target_path: "src/generated/governed_types.rs".to_string(),
                module_path: vec!["generated".to_string()],
                integration_mode: IntegrationMode::PatchOnly,
            },
            derives: vec!["Debug".to_string(), "Clone".to_string()],
            layout: LayoutSpec {
                repr: "Rust".to_string(),
                repr_c_justification_hash: None,
            },
            fields: vec![FieldSpec {
                name: "run_id".to_string(),
                type_expr: "RunId".to_string(),
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
    fn mints_deterministic_struct_receipt() {
        let spec = valid_spec();
        let first = mint_struct(&spec).unwrap();
        let second = mint_struct(&spec).unwrap();
        assert_eq!(
            first.receipt.generated_item_sha256,
            second.receipt.generated_item_sha256
        );
        assert!(first.generated_source.contains("pub struct ExecutionToken"));
    }

    #[test]
    fn rejects_raw_prose_governance() {
        let mut spec = valid_spec();
        spec.governance.principle_hash = "scope execution authority".to_string();
        assert!(verify_struct_spec(&spec).is_err());
    }

    #[test]
    fn rejects_public_struct_without_policy_hashes() {
        let mut spec = valid_spec();
        spec.governance.test_policy_hash = None;
        assert!(verify_struct_spec(&spec).is_err());
    }
}
