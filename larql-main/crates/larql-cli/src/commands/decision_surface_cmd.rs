use clap::{Args, Subcommand};
use larql_governance::{
    load_ceremony_decision_budgets, load_decision_surface_registry, load_llm_autonomy_roles,
    validate_decision_surface, UnknownDecisionEncountered,
};
use serde::Serialize;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Args)]
pub struct DecisionSurfaceArgs {
    #[command(subcommand)]
    command: DecisionSurfaceCommand,
}

#[derive(Subcommand)]
enum DecisionSurfaceCommand {
    /// Validate the decision surface registry, budgets, and LLM autonomy defaults.
    Validate {
        #[arg(long)]
        registry: PathBuf,
        #[arg(long)]
        budgets: Option<PathBuf>,
        #[arg(long)]
        roles: Option<PathBuf>,
    },
    /// Emit an unknown-decision event before recurring LLM judgment becomes authority.
    RecordUnknown {
        #[arg(long)]
        id: String,
        #[arg(long)]
        question: String,
        #[arg(long)]
        context: String,
        #[arg(long = "target-owner")]
        target_owner: Option<String>,
        #[arg(long = "promotion-path")]
        promotion_path: Option<String>,
        #[arg(long)]
        risk: Option<String>,
        #[arg(long = "option")]
        option: Vec<String>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

pub fn run(args: DecisionSurfaceArgs) -> Result<(), Box<dyn Error>> {
    match args.command {
        DecisionSurfaceCommand::Validate {
            registry,
            budgets,
            roles,
        } => run_validate(&registry, budgets.as_deref(), roles.as_deref()),
        DecisionSurfaceCommand::RecordUnknown {
            id,
            question,
            context,
            target_owner,
            promotion_path,
            risk,
            option,
            out,
        } => run_record_unknown(
            id,
            question,
            context,
            target_owner,
            promotion_path,
            risk,
            option,
            out.as_deref(),
        ),
    }
}

fn run_validate(
    registry_path: &Path,
    budget_path: Option<&Path>,
    roles_path: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    let registry = load_decision_surface_registry(registry_path)?;
    let budgets = budget_path
        .map(load_ceremony_decision_budgets)
        .transpose()?;
    let roles = roles_path.map(load_llm_autonomy_roles).transpose()?;
    let report = validate_decision_surface(
        registry_path,
        &registry,
        budget_path,
        budgets.as_ref(),
        roles_path,
        roles.as_ref(),
    );
    let passed = report.passed;
    write_json_or_print(None, &report)?;
    if !passed {
        return Err("decision surface validation failed".into());
    }
    Ok(())
}

fn run_record_unknown(
    id: String,
    question: String,
    context: String,
    target_owner: Option<String>,
    promotion_path: Option<String>,
    risk: Option<String>,
    option: Vec<String>,
    out: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    let mut context_parts = vec![format!("id={id}"), format!("context={context}")];
    if let Some(value) = target_owner {
        context_parts.push(format!("target_owner={value}"));
    }
    if let Some(value) = promotion_path {
        context_parts.push(format!("promotion_path={value}"));
    }
    if let Some(value) = risk {
        context_parts.push(format!("risk={value}"));
    }
    let event = UnknownDecisionEncountered {
        event_type: "UnknownDecisionEncountered".to_string(),
        decision_question: question,
        context: context_parts.join("; "),
        proposed_options: option,
        recommended_next_step: "record in governance/decision_surface/registry.toml or promote to deterministic artifact".to_string(),
    };
    write_json_or_print(out, &event)
}

fn write_json_or_print<T: Serialize>(path: Option<&Path>, value: &T) -> Result<(), Box<dyn Error>> {
    let text = serde_json::to_string_pretty(value)?;
    match path {
        Some(path) => {
            ensure_repo_relative_path(path)?;
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)?;
                }
            }
            fs::write(path, format!("{text}\n"))?;
        }
        None => println!("{text}"),
    }
    Ok(())
}

fn ensure_repo_relative_path(path: &Path) -> Result<(), Box<dyn Error>> {
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(format!("path must be repo-relative: {}", path.display()).into());
    }
    Ok(())
}
