use super::{decision_surface_cmd, policy_rules_cmd};
use clap::{Args, Subcommand};
use larql_core::capsule::{validate_capsule, write_capsule_json, Capsule};
use larql_governance::{
    hash_text, make_denial_capsule, make_mint_receipt_capsule, make_patch_envelope_capsule,
    make_policy_receipt_capsule, mint_struct_with_profile, plan_governed_patch,
    validate_machine_profile, verify_struct_spec_with_profile, CargoCheckStatus, IntegrationMode,
    MachineDenial, MachineRuleProfile, MintReceipt, PatchEnvelopeReceipt, PatchIntentSpec,
    PolicyReceipt, StructSpec, DENIAL_CAPSULE_KIND, DENIAL_CAPTURE_KIND, MINT_RECEIPT_CAPSULE_KIND,
    MINT_RECEIPT_CAPTURE_KIND, PATCH_ENVELOPE_CAPSULE_KIND, PATCH_ENVELOPE_CAPTURE_KIND,
    POLICY_RECEIPT_CAPSULE_KIND, POLICY_RECEIPT_CAPTURE_KIND,
};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Args)]
pub struct MachineArgs {
    #[command(subcommand)]
    command: MachineCommand,
}

#[derive(Subcommand)]
enum MachineCommand {
    /// Verify machine policy inputs.
    Policy(PolicyArgs),
    /// Plan governed patch envelopes from hash-backed intent.
    Patch(PatchArgs),
    /// Mint or verify governed Rust structs.
    Struct(StructArgs),
    /// Verify governance receipt capsules.
    Receipt(ReceiptArgs),
    /// Evaluate invariant, policy, and rule registries.
    Rules(policy_rules_cmd::RulesArgs),
    /// Validate and record decision-surface debt.
    DecisionSurface(decision_surface_cmd::DecisionSurfaceArgs),
}

#[derive(Args)]
struct PolicyArgs {
    #[command(subcommand)]
    command: PolicyCommand,
}

#[derive(Subcommand)]
enum PolicyCommand {
    /// Verify the starter profile and optional struct spec.
    Verify {
        /// Optional machine rule profile JSON. Defaults to default-starter.
        #[arg(long)]
        profile: Option<PathBuf>,
        /// Optional struct spec JSON to policy-check.
        #[arg(long)]
        spec: Option<PathBuf>,
        /// Optional receipt output path. Defaults to stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Args)]
struct PatchArgs {
    #[command(subcommand)]
    command: PatchCommand,
}

#[derive(Subcommand)]
enum PatchCommand {
    /// Plan a governed patch envelope without writing runtime code.
    Plan {
        /// Hash-backed patch intent JSON.
        #[arg(long)]
        spec: PathBuf,
        /// Optional machine rule profile JSON. Defaults to default-starter.
        #[arg(long)]
        profile: Option<PathBuf>,
        /// Optional envelope capsule output path. Defaults to stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Args)]
struct StructArgs {
    #[command(subcommand)]
    command: StructCommand,
}

#[derive(Subcommand)]
enum StructCommand {
    /// Mint a governed Rust struct from a JSON spec.
    Mint {
        /// Governed struct spec JSON.
        #[arg(long)]
        spec: PathBuf,
        /// Optional machine rule profile JSON. Defaults to default-starter.
        #[arg(long)]
        profile: Option<PathBuf>,
        /// Override integration mode: patch-only or generated-file.
        #[arg(long)]
        mode: Option<String>,
        /// Optional generated Rust output path for patch-only review.
        #[arg(long)]
        source_out: Option<PathBuf>,
        /// Optional receipt output path. Defaults to stdout.
        #[arg(long)]
        receipt_out: Option<PathBuf>,
        /// Do not run cargo check after generated-file writes.
        #[arg(long)]
        skip_cargo_check: bool,
    },
    /// Verify a governed Rust struct spec without writing code.
    Verify {
        /// Governed struct spec JSON.
        #[arg(long)]
        spec: PathBuf,
        /// Optional machine rule profile JSON. Defaults to default-starter.
        #[arg(long)]
        profile: Option<PathBuf>,
        /// Optional receipt output path. Defaults to stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Args)]
struct ReceiptArgs {
    #[command(subcommand)]
    command: ReceiptCommand,
}

#[derive(Subcommand)]
enum ReceiptCommand {
    /// Verify a governance receipt capsule.
    Verify {
        /// Receipt capsule JSON path.
        #[arg(long)]
        path: PathBuf,
    },
}

pub fn run(args: MachineArgs) -> Result<(), Box<dyn Error>> {
    match args.command {
        MachineCommand::Policy(args) => run_policy(args),
        MachineCommand::Patch(args) => run_patch(args),
        MachineCommand::Struct(args) => run_struct(args),
        MachineCommand::Receipt(args) => run_receipt(args),
        MachineCommand::Rules(args) => policy_rules_cmd::run(args),
        MachineCommand::DecisionSurface(args) => decision_surface_cmd::run(args),
    }
}

fn run_policy(args: PolicyArgs) -> Result<(), Box<dyn Error>> {
    match args.command {
        PolicyCommand::Verify { profile, spec, out } => {
            let profile = read_profile(profile.as_deref())?;
            let decision = match &spec {
                Some(path) => {
                    let spec = read_struct_spec(path)?;
                    verify_struct_spec_with_profile(&spec, &profile)?
                }
                None => validate_machine_profile(&profile)?,
            };
            let spec_sha256 = spec
                .as_ref()
                .map(|path| std::fs::read(path))
                .transpose()?
                .map(|bytes| larql_governance::hash_bytes(&bytes));
            let receipt = PolicyReceipt {
                schema_version: "larql.governance.policy.receipt.v1".to_string(),
                event_type: "PolicyVerified".to_string(),
                profile: profile.name,
                spec_sha256,
                decision,
            };
            let capsule = make_policy_receipt_capsule(receipt)?;
            write_or_print_capsule(out.as_deref(), &capsule)?;
            Ok(())
        }
    }
}

fn run_patch(args: PatchArgs) -> Result<(), Box<dyn Error>> {
    match args.command {
        PatchCommand::Plan { spec, profile, out } => {
            let profile = read_profile(profile.as_deref())?;
            let spec_value = read_patch_intent_spec(&spec)?;
            let envelope = plan_governed_patch(Path::new("."), &spec_value, &profile)?;
            let capsule = make_patch_envelope_capsule(envelope)?;
            write_or_print_capsule(out.as_deref(), &capsule)?;
            Ok(())
        }
    }
}

fn run_struct(args: StructArgs) -> Result<(), Box<dyn Error>> {
    match args.command {
        StructCommand::Mint {
            spec,
            profile,
            mode,
            source_out,
            receipt_out,
            skip_cargo_check,
        } => {
            let profile = read_profile(profile.as_deref())?;
            let mut spec_value = read_struct_spec(&spec)?;
            if let Some(mode) = mode {
                spec_value.item.integration_mode = parse_mode(&mode)?;
            }
            match mint_struct_with_profile(&spec_value, &profile) {
                Ok(mut outcome) => {
                    match spec_value.item.integration_mode {
                        IntegrationMode::PatchOnly => {
                            if let Some(path) = source_out {
                                write_review_source(&path, &outcome.generated_source)?;
                            }
                            outcome.receipt.cargo_check = CargoCheckStatus::NotApplicablePatchOnly;
                        }
                        IntegrationMode::GeneratedFile => {
                            write_generated_file(
                                Path::new(&spec_value.item.target_path),
                                &outcome.generated_source,
                            )?;
                            outcome.receipt.cargo_check = if skip_cargo_check {
                                CargoCheckStatus::NotRun
                            } else if run_cargo_check()? {
                                CargoCheckStatus::Passed
                            } else {
                                CargoCheckStatus::Failed
                            };
                        }
                    }
                    let capsule = make_mint_receipt_capsule(outcome.receipt)?;
                    write_or_print_capsule(receipt_out.as_deref(), &capsule)?;
                    Ok(())
                }
                Err(err) => {
                    let denial = MachineDenial {
                        schema_version: "larql.governance.denial.v1".to_string(),
                        event_type: "TransitionDenied".to_string(),
                        machine: "struct_minter".to_string(),
                        rule: "struct_minter_policy_rejection".to_string(),
                        denied_field: "struct_spec".to_string(),
                        required_ceremony: "MachineStructMintCeremony".to_string(),
                        evidence_hashes: vec![hash_text(&err.to_string())],
                    };
                    let capsule = make_denial_capsule(denial)?;
                    write_or_print_capsule(receipt_out.as_deref(), &capsule)?;
                    Err(format!("struct mint denied: {err}").into())
                }
            }
        }
        StructCommand::Verify { spec, profile, out } => {
            let profile = read_profile(profile.as_deref())?;
            let spec_value = read_struct_spec(&spec)?;
            let decision = verify_struct_spec_with_profile(&spec_value, &profile)?;
            let spec_sha256 = larql_governance::hash_bytes(&std::fs::read(&spec)?);
            let receipt = PolicyReceipt {
                schema_version: "larql.governance.policy.receipt.v1".to_string(),
                event_type: "StructSpecVerified".to_string(),
                profile: profile.name,
                spec_sha256: Some(spec_sha256),
                decision,
            };
            let capsule = make_policy_receipt_capsule(receipt)?;
            write_or_print_capsule(out.as_deref(), &capsule)?;
            Ok(())
        }
    }
}

fn run_receipt(args: ReceiptArgs) -> Result<(), Box<dyn Error>> {
    match args.command {
        ReceiptCommand::Verify { path } => verify_receipt(&path),
    }
}

fn verify_receipt(path: &Path) -> Result<(), Box<dyn Error>> {
    let text = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let kind = value
        .get("capsule_kind")
        .and_then(|kind| kind.as_str())
        .ok_or("receipt is missing capsule_kind")?;
    match kind {
        MINT_RECEIPT_CAPSULE_KIND => {
            let capsule: Capsule<MintReceipt> = serde_json::from_value(value)?;
            validate_capsule(
                &capsule,
                Some(MINT_RECEIPT_CAPSULE_KIND),
                Some(MINT_RECEIPT_CAPTURE_KIND),
            )?;
        }
        POLICY_RECEIPT_CAPSULE_KIND => {
            let capsule: Capsule<PolicyReceipt> = serde_json::from_value(value)?;
            validate_capsule(
                &capsule,
                Some(POLICY_RECEIPT_CAPSULE_KIND),
                Some(POLICY_RECEIPT_CAPTURE_KIND),
            )?;
        }
        PATCH_ENVELOPE_CAPSULE_KIND => {
            let capsule: Capsule<PatchEnvelopeReceipt> = serde_json::from_value(value)?;
            validate_capsule(
                &capsule,
                Some(PATCH_ENVELOPE_CAPSULE_KIND),
                Some(PATCH_ENVELOPE_CAPTURE_KIND),
            )?;
        }
        DENIAL_CAPSULE_KIND => {
            let capsule: Capsule<MachineDenial> = serde_json::from_value(value)?;
            validate_capsule(
                &capsule,
                Some(DENIAL_CAPSULE_KIND),
                Some(DENIAL_CAPTURE_KIND),
            )?;
        }
        other => return Err(format!("unsupported receipt capsule kind: {other}").into()),
    }
    println!("receipt verified: {}", path.display());
    Ok(())
}

fn read_profile(path: Option<&Path>) -> Result<MachineRuleProfile, Box<dyn Error>> {
    match path {
        Some(path) => Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?),
        None => Ok(MachineRuleProfile::default_starter()),
    }
}

fn read_struct_spec(path: &Path) -> Result<StructSpec, Box<dyn Error>> {
    let text = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

fn read_patch_intent_spec(path: &Path) -> Result<PatchIntentSpec, Box<dyn Error>> {
    let text = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

fn parse_mode(mode: &str) -> Result<IntegrationMode, Box<dyn Error>> {
    match mode {
        "patch-only" => Ok(IntegrationMode::PatchOnly),
        "generated-file" => Ok(IntegrationMode::GeneratedFile),
        _ => Err(format!("unknown integration mode: {mode}").into()),
    }
}

fn write_generated_file(path: &Path, text: &str) -> Result<(), Box<dyn Error>> {
    ensure_repo_relative_path(path)?;
    write_text(path, text)
}

fn write_review_source(path: &Path, text: &str) -> Result<(), Box<dyn Error>> {
    ensure_repo_relative_path(path)?;
    if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
        return Err("patch-only source output must be a Rust source file".into());
    }
    if !(path_starts_with(path, "target") || path_has_generated_component(path)) {
        return Err("patch-only source output must live under target/ or a generated path".into());
    }
    write_new_text(path, text)
}

fn write_receipt(
    path: &Path,
    capsule: &Capsule<impl serde::Serialize>,
) -> Result<(), Box<dyn Error>> {
    ensure_repo_relative_path(path)?;
    if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
        return Err("receipt output must be a JSON file".into());
    }
    if path.exists() {
        return Err(format!("refusing to overwrite existing receipt: {}", path.display()).into());
    }
    write_capsule_json(path, capsule)?;
    Ok(())
}

fn write_new_text(path: &Path, text: &str) -> Result<(), Box<dyn Error>> {
    if path.exists() {
        return Err(format!("refusing to overwrite existing output: {}", path.display()).into());
    }
    write_text(path, text)
}

fn write_text(path: &Path, text: &str) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, text)?;
    Ok(())
}

fn ensure_repo_relative_path(path: &Path) -> Result<(), Box<dyn Error>> {
    if path.is_absolute() {
        return Err(format!(
            "machine output path must be repo-relative: {}",
            path.display()
        )
        .into());
    }
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir | std::path::Component::RootDir
        )
    }) {
        return Err(format!("machine output path cannot escape repo: {}", path.display()).into());
    }
    Ok(())
}

fn path_starts_with(path: &Path, expected: &str) -> bool {
    path.components()
        .next()
        .is_some_and(|component| component.as_os_str() == expected)
}

fn path_has_generated_component(path: &Path) -> bool {
    path.components().any(|component| match component {
        std::path::Component::Normal(value) => value == "generated",
        _ => false,
    }) || path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("governed_"))
}

fn write_or_print_capsule<T: serde::Serialize>(
    path: Option<&Path>,
    capsule: &Capsule<T>,
) -> Result<(), Box<dyn Error>> {
    match path {
        Some(path) => write_receipt(path, capsule)?,
        None => {
            let mut bytes = larql_core::capsule::canonical_json_bytes(capsule)?;
            bytes.push(b'\n');
            print!("{}", String::from_utf8(bytes)?);
        }
    }
    Ok(())
}

fn run_cargo_check() -> Result<bool, Box<dyn Error>> {
    let status = Command::new("cargo")
        .args(["check", "-p", "larql-governance", "-p", "larql-cli"])
        .status()?;
    Ok(status.success())
}
