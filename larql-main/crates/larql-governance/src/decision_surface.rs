use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum DecisionSurfaceError {
    #[error("decision surface IO failed for {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("decision surface TOML parse failed for {path}: {source}")]
    Toml {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("decision surface invalid: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecisionSurfaceRegistry {
    pub schema_version: String,
    #[serde(default)]
    pub decision: Vec<DecisionSurfaceItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecisionSurfaceItem {
    pub id: String,
    pub question: String,
    pub current_owner: DecisionOwner,
    pub target_owner: DecisionOwner,
    pub status: DecisionStatus,
    pub promotion_path: PromotionPath,
    pub risk: DecisionRisk,
    #[serde(default)]
    pub ceremony: Option<String>,
    #[serde(default)]
    pub promotion_artifact: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DecisionOwner {
    Llm,
    PolicyEngine,
    Template,
    Schema,
    Ceremony,
    Machine,
    Fixture,
    Default,
    Human,
    Retired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    Open,
    Assisted,
    Constrained,
    Promoted,
    Retired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum PromotionPath {
    PolicyRule,
    Template,
    Schema,
    Ceremony,
    Machine,
    Fixture,
    Default,
    MachineRulePlusTemplate,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DecisionRisk {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CeremonyDecisionBudgets {
    pub schema_version: String,
    #[serde(default)]
    pub ceremony: BTreeMap<String, CeremonyDecisionBudget>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CeremonyDecisionBudget {
    pub max_open_llm_decisions: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LlmAutonomyRoles {
    pub schema_version: String,
    #[serde(default)]
    pub llm_role: BTreeMap<String, LlmAutonomyRole>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LlmAutonomyRole {
    pub level: String,
    #[serde(default)]
    pub may_decide: Vec<String>,
    #[serde(default)]
    pub may_not_decide: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecisionSurfaceValidation {
    pub schema_version: String,
    pub registry_path: String,
    #[serde(default)]
    pub budget_path: Option<String>,
    #[serde(default)]
    pub autonomy_roles_path: Option<String>,
    pub total_decisions: usize,
    pub open_llm_decisions: usize,
    #[serde(default)]
    pub ceremony_open_llm_decisions: BTreeMap<String, usize>,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnknownDecisionEncountered {
    pub event_type: String,
    pub decision_question: String,
    pub context: String,
    #[serde(default)]
    pub proposed_options: Vec<String>,
    pub recommended_next_step: String,
}

/// Deserialize a [`DecisionSurfaceRegistry`] from UTF-8 TOML bytes (host-agnostic; embedded/WASM safe).
pub fn parse_decision_surface_registry_from_str(
    text: &str,
    path_label: impl Into<String>,
) -> Result<DecisionSurfaceRegistry, DecisionSurfaceError> {
    let path = path_label.into();
    let registry: DecisionSurfaceRegistry =
        toml::from_str(text).map_err(|source| DecisionSurfaceError::Toml {
            path: path.clone(),
            source,
        })?;
    if registry.schema_version != "larql.governance.decision_surface.v1" {
        return Err(DecisionSurfaceError::Invalid(
            "unsupported decision surface registry schema".to_string(),
        ));
    }
    Ok(registry)
}

/// Deserialize [`CeremonyDecisionBudgets`] from UTF-8 TOML bytes (host-agnostic; embedded/WASM safe).
pub fn parse_ceremony_decision_budgets_from_str(
    text: &str,
    path_label: impl Into<String>,
) -> Result<CeremonyDecisionBudgets, DecisionSurfaceError> {
    let path = path_label.into();
    let budgets: CeremonyDecisionBudgets =
        toml::from_str(text).map_err(|source| DecisionSurfaceError::Toml {
            path: path.clone(),
            source,
        })?;
    if budgets.schema_version != "larql.governance.ceremony_budgets.v1" {
        return Err(DecisionSurfaceError::Invalid(
            "unsupported ceremony budget schema".to_string(),
        ));
    }
    Ok(budgets)
}

pub fn load_decision_surface_registry(
    path: &Path,
) -> Result<DecisionSurfaceRegistry, DecisionSurfaceError> {
    let text = read_text(path)?;
    parse_decision_surface_registry_from_str(&text, path.display().to_string())
}

pub fn load_ceremony_decision_budgets(
    path: &Path,
) -> Result<CeremonyDecisionBudgets, DecisionSurfaceError> {
    let text = read_text(path)?;
    parse_ceremony_decision_budgets_from_str(&text, path.display().to_string())
}

pub fn load_llm_autonomy_roles(path: &Path) -> Result<LlmAutonomyRoles, DecisionSurfaceError> {
    let text = read_text(path)?;
    let roles: LlmAutonomyRoles =
        toml::from_str(&text).map_err(|source| DecisionSurfaceError::Toml {
            path: path.display().to_string(),
            source,
        })?;
    if roles.schema_version != "larql.governance.llm_autonomy_roles.v1" {
        return Err(DecisionSurfaceError::Invalid(
            "unsupported llm autonomy roles schema".to_string(),
        ));
    }
    Ok(roles)
}

pub fn validate_decision_surface(
    registry_path: &Path,
    registry: &DecisionSurfaceRegistry,
    budget_path: Option<&Path>,
    budgets: Option<&CeremonyDecisionBudgets>,
    autonomy_roles_path: Option<&Path>,
    autonomy_roles: Option<&LlmAutonomyRoles>,
) -> DecisionSurfaceValidation {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut ids = BTreeSet::new();
    let mut open_llm_decisions = 0usize;
    let mut ceremony_open_llm_decisions = BTreeMap::new();

    for item in &registry.decision {
        if item.id.trim().is_empty() {
            errors.push("decision id cannot be empty".to_string());
        }
        if item.question.trim().is_empty() {
            errors.push(format!("decision {} question cannot be empty", item.id));
        }
        if !ids.insert(item.id.clone()) {
            errors.push(format!("duplicate decision id: {}", item.id));
        }
        if item.status == DecisionStatus::Promoted {
            if matches!(
                item.current_owner,
                DecisionOwner::Llm | DecisionOwner::Human
            ) {
                errors.push(format!(
                    "promoted decision {} cannot remain owned by {:?}",
                    item.id, item.current_owner
                ));
            }
            if item.current_owner != item.target_owner {
                errors.push(format!(
                    "promoted decision {} current_owner must match target_owner",
                    item.id
                ));
            }
            let artifact = item.promotion_artifact.as_deref().unwrap_or("").trim();
            if artifact.is_empty() {
                errors.push(format!(
                    "promoted decision {} requires promotion_artifact",
                    item.id
                ));
            } else if let Some(repo_root) = registry_path
                .parent()
                .and_then(|path| path.parent())
                .and_then(|path| path.parent())
            {
                let artifact_path = artifact.split('#').next().unwrap_or(artifact);
                if !artifact_path.contains('*') && !repo_root.join(artifact_path).exists() {
                    errors.push(format!(
                        "promoted decision {} references missing artifact {}",
                        item.id, artifact_path
                    ));
                }
            }
        }
        validate_promotion_path(item, &mut errors);
        if item.status == DecisionStatus::Retired
            && item.promotion_path != PromotionPath::None
            && item.target_owner != DecisionOwner::Retired
        {
            warnings.push(format!(
                "retired decision {} still names active promotion path",
                item.id
            ));
        }
        if is_open_llm_decision(item) {
            if item.promotion_path == PromotionPath::None {
                errors.push(format!(
                    "open llm decision {} requires a promotion path",
                    item.id
                ));
            }
            if matches!(item.target_owner, DecisionOwner::Llm | DecisionOwner::Human) {
                errors.push(format!(
                    "open llm decision {} requires deterministic target_owner",
                    item.id
                ));
            }
            open_llm_decisions += 1;
            if let Some(ceremony) = &item.ceremony {
                *ceremony_open_llm_decisions
                    .entry(ceremony.clone())
                    .or_insert(0) += 1;
            } else {
                warnings.push(format!("open llm decision {} lacks ceremony", item.id));
            }
        }
    }

    if let Some(budgets) = budgets {
        for (ceremony, count) in &ceremony_open_llm_decisions {
            match budgets.ceremony.get(ceremony) {
                Some(budget) if *count as u64 > budget.max_open_llm_decisions => {
                    errors.push(format!(
                        "ceremony {ceremony} has {count} open llm decisions, budget {}",
                        budget.max_open_llm_decisions
                    ))
                }
                Some(_) => {}
                None => warnings.push(format!(
                    "ceremony {ceremony} has open llm decisions but no budget"
                )),
            }
        }
    }

    if let Some(roles) = autonomy_roles {
        validate_llm_autonomy_roles(roles, &mut errors, &mut warnings);
    }

    let passed = errors.is_empty();
    DecisionSurfaceValidation {
        schema_version: "larql.governance.decision_surface.validation.v1".to_string(),
        registry_path: registry_path.display().to_string(),
        budget_path: budget_path.map(|path| path.display().to_string()),
        autonomy_roles_path: autonomy_roles_path.map(|path| path.display().to_string()),
        total_decisions: registry.decision.len(),
        open_llm_decisions,
        ceremony_open_llm_decisions,
        errors,
        warnings,
        passed,
    }
}

fn validate_llm_autonomy_roles(
    roles: &LlmAutonomyRoles,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let forbidden_authority = [
        "policy.activation",
        "capability.grant",
        "runtime.mutation",
        "policy.approval",
    ];
    for (role_id, role) in &roles.llm_role {
        if role_id.trim().is_empty() {
            errors.push("llm role id cannot be empty".to_string());
        }
        if role.level.trim().is_empty() {
            errors.push(format!("llm role {role_id} requires level"));
        }
        let may_decide: BTreeSet<_> = role.may_decide.iter().collect();
        for decision in &role.may_not_decide {
            if may_decide.contains(decision) {
                errors.push(format!(
                    "llm role {role_id} both may_decide and may_not_decide {decision}"
                ));
            }
        }
        for decision in &role.may_decide {
            if forbidden_authority.contains(&decision.as_str()) {
                errors.push(format!(
                    "llm role {role_id} may not own high-authority decision {decision}"
                ));
            }
        }
        if role.may_decide.is_empty() && role.may_not_decide.is_empty() {
            warnings.push(format!("llm role {role_id} has no decision bindings"));
        }
    }
}

fn validate_promotion_path(item: &DecisionSurfaceItem, errors: &mut Vec<String>) {
    let expected_owner = match item.promotion_path {
        PromotionPath::PolicyRule => Some(DecisionOwner::PolicyEngine),
        PromotionPath::Template => Some(DecisionOwner::Template),
        PromotionPath::Schema => Some(DecisionOwner::Schema),
        PromotionPath::Ceremony => Some(DecisionOwner::Ceremony),
        PromotionPath::Machine => Some(DecisionOwner::Machine),
        PromotionPath::Fixture => Some(DecisionOwner::Fixture),
        PromotionPath::Default => Some(DecisionOwner::Default),
        PromotionPath::MachineRulePlusTemplate => Some(DecisionOwner::Machine),
        PromotionPath::None => Some(DecisionOwner::Retired),
    };
    if let Some(expected_owner) = expected_owner {
        if item.target_owner != expected_owner {
            errors.push(format!(
                "decision {} promotion_path {:?} expects target_owner {:?}, got {:?}",
                item.id, item.promotion_path, expected_owner, item.target_owner
            ));
        }
    }
}

fn is_open_llm_decision(item: &DecisionSurfaceItem) -> bool {
    item.current_owner == DecisionOwner::Llm
        && matches!(
            item.status,
            DecisionStatus::Open | DecisionStatus::Assisted | DecisionStatus::Constrained
        )
}

fn read_text(path: &Path) -> Result<String, DecisionSurfaceError> {
    std::fs::read_to_string(path).map_err(|source| DecisionSurfaceError::Io {
        path: path.display().to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promoted_decision_requires_non_llm_owner_and_artifact() {
        let registry = DecisionSurfaceRegistry {
            schema_version: "larql.governance.decision_surface.v1".to_string(),
            decision: vec![DecisionSurfaceItem {
                id: "policy.activation".to_string(),
                question: "Who activates policy?".to_string(),
                current_owner: DecisionOwner::Llm,
                target_owner: DecisionOwner::PolicyEngine,
                status: DecisionStatus::Promoted,
                promotion_path: PromotionPath::PolicyRule,
                risk: DecisionRisk::Critical,
                ceremony: Some("policy_update".to_string()),
                promotion_artifact: None,
            }],
        };
        let report = validate_decision_surface(
            Path::new("registry.toml"),
            &registry,
            None,
            None,
            None,
            None,
        );
        assert!(!report.passed);
        assert_eq!(report.errors.len(), 3);
    }

    #[test]
    fn ceremony_budget_blocks_too_many_open_llm_decisions() {
        let registry = DecisionSurfaceRegistry {
            schema_version: "larql.governance.decision_surface.v1".to_string(),
            decision: vec![DecisionSurfaceItem {
                id: "capability.scope".to_string(),
                question: "What scope?".to_string(),
                current_owner: DecisionOwner::Llm,
                target_owner: DecisionOwner::PolicyEngine,
                status: DecisionStatus::Open,
                promotion_path: PromotionPath::PolicyRule,
                risk: DecisionRisk::High,
                ceremony: Some("capability_minting".to_string()),
                promotion_artifact: None,
            }],
        };
        let mut ceremony = BTreeMap::new();
        ceremony.insert(
            "capability_minting".to_string(),
            CeremonyDecisionBudget {
                max_open_llm_decisions: 0,
            },
        );
        let budgets = CeremonyDecisionBudgets {
            schema_version: "larql.governance.ceremony_budgets.v1".to_string(),
            ceremony,
        };
        let report = validate_decision_surface(
            Path::new("registry.toml"),
            &registry,
            Some(Path::new("budgets.toml")),
            Some(&budgets),
            None,
            None,
        );
        assert!(!report.passed);
        assert!(report.errors[0].contains("capability_minting"));
    }

    #[test]
    fn autonomy_roles_cannot_grant_policy_authority_to_llm() {
        let roles = LlmAutonomyRoles {
            schema_version: "larql.governance.llm_autonomy_roles.v1".to_string(),
            llm_role: BTreeMap::from([(
                "bootstrap_l5".to_string(),
                LlmAutonomyRole {
                    level: "L5 policy critic".to_string(),
                    may_decide: vec!["policy.activation".to_string()],
                    may_not_decide: vec![],
                },
            )]),
        };
        let registry = DecisionSurfaceRegistry {
            schema_version: "larql.governance.decision_surface.v1".to_string(),
            decision: vec![],
        };
        let report = validate_decision_surface(
            Path::new("registry.toml"),
            &registry,
            None,
            None,
            Some(Path::new("llm_roles.toml")),
            Some(&roles),
        );
        assert!(!report.passed);
        assert!(report.errors[0].contains("policy.activation"));
    }
}
