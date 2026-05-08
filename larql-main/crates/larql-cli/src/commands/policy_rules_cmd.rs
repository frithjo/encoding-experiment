use clap::{Args, Subcommand};
use larql_governance::{
    draft_rule_proposal_from_intent, hash_text, load_policy_registry,
    mint_policy_update_capability, validate_invariant_registry, validate_policy_apply_request,
    validate_policy_update_ceremony_trace, ConditionBlock, DecisionKind,
    GovernanceInvariantRegistry, PolicyApplyReceipt, PolicyApplyRequest, PolicyApproval,
    PolicyCapability, PolicyClass, PolicyEngine, PolicyEngineDecision, PolicyInput, PolicyIntent,
    PolicyIntentExtraction, PolicyRegistry, PolicyRule, PolicyTestReport, PolicyTestSuite,
    PolicyUpdateCeremonyTrace, PolicyUpdateType, ProsePolicyConcern, RuleProposal,
};
use serde::Serialize;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Args)]
pub struct RulesArgs {
    #[command(subcommand)]
    command: RulesCommand,
}

#[derive(Subcommand)]
enum RulesCommand {
    /// Verify the invariant registry and emit a hash-backed receipt.
    Verify {
        #[arg(long)]
        registry: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Evaluate facts against the active policy index.
    Eval {
        #[arg(long)]
        facts: PathBuf,
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        ledger: Option<PathBuf>,
    },
    /// Run TOML policy cases against the active policy index.
    Test {
        #[arg(long)]
        cases: PathBuf,
        #[arg(long)]
        policy: PathBuf,
    },
    /// Run a proposed rule against a fact packet without activating it.
    ShadowEval {
        #[arg(long)]
        proposal: PathBuf,
        #[arg(long)]
        facts: PathBuf,
        #[arg(long)]
        policy: PathBuf,
    },
    /// Check a proposed rule against the active rule set.
    ConflictCheck {
        #[arg(long)]
        proposal: PathBuf,
        #[arg(long)]
        policy: PathBuf,
    },
    /// Record the exact prose concern that starts a policy update.
    Propose {
        #[arg(long = "from-prose")]
        from_prose: String,
        #[arg(long = "class")]
        policy_class: Option<String>,
        #[arg(long)]
        target: String,
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        prior_state_hash: String,
        #[arg(long, default_value = "human:unknown")]
        actor: String,
        #[arg(long)]
        ceremony_id: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Convert a prose concern plus operator facts into formal intent.
    ExtractIntent {
        #[arg(long)]
        concern: PathBuf,
        #[arg(long = "class")]
        policy_class: String,
        #[arg(long)]
        action: String,
        #[arg(long = "required-evidence")]
        required_evidence: Vec<String>,
        #[arg(long, default_value = "deny_if_missing")]
        default_decision: String,
        #[arg(long)]
        uncertainty: Vec<String>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Draft a formal rule proposal from concern and intent records.
    Draft {
        #[arg(long)]
        concern: PathBuf,
        #[arg(long)]
        intent: PathBuf,
        #[arg(long, default_value = "new_rule")]
        update_type: String,
        #[arg(long, default_value = "policy-clerk")]
        proposer: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Validate a full policy-update ceremony trace.
    Ceremony {
        #[arg(long)]
        trace: PathBuf,
    },
    /// Mint a scoped update capability after approval.
    MintCapability {
        #[arg(long)]
        proposal: PathBuf,
        #[arg(long)]
        approval: PathBuf,
        #[arg(long)]
        policy: PathBuf,
        #[arg(long = "policy-file")]
        policy_file: String,
        #[arg(long)]
        expires_at: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Apply an approved rule through a scoped capability.
    Apply {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        policy: PathBuf,
    },
}

#[derive(Args)]
pub struct ReplayArgs {
    #[arg(long)]
    pub ledger: PathBuf,
}

pub fn run(args: RulesArgs) -> Result<(), Box<dyn Error>> {
    match args.command {
        RulesCommand::Verify { registry, out } => run_verify(&registry, out.as_deref()),
        RulesCommand::Eval {
            facts,
            policy,
            ledger,
        } => run_eval(&facts, &policy, ledger.as_deref()),
        RulesCommand::Test { cases, policy } => run_test(&cases, &policy),
        RulesCommand::ShadowEval {
            proposal,
            facts,
            policy,
        } => run_shadow_eval(&proposal, &facts, &policy),
        RulesCommand::ConflictCheck { proposal, policy } => run_conflict_check(&proposal, &policy),
        RulesCommand::Propose {
            from_prose,
            policy_class,
            target,
            policy,
            prior_state_hash,
            actor,
            ceremony_id,
            out,
        } => run_propose(
            from_prose,
            policy_class,
            target,
            &policy,
            prior_state_hash,
            actor,
            ceremony_id,
            out.as_deref(),
        ),
        RulesCommand::ExtractIntent {
            concern,
            policy_class,
            action,
            required_evidence,
            default_decision,
            uncertainty,
            out,
        } => run_extract_intent(
            &concern,
            policy_class,
            action,
            required_evidence,
            default_decision,
            uncertainty,
            out.as_deref(),
        ),
        RulesCommand::Draft {
            concern,
            intent,
            update_type,
            proposer,
            out,
        } => run_draft(&concern, &intent, update_type, proposer, out.as_deref()),
        RulesCommand::Ceremony { trace } => run_ceremony(&trace),
        RulesCommand::MintCapability {
            proposal,
            approval,
            policy,
            policy_file,
            expires_at,
            out,
        } => run_mint_capability(
            &proposal,
            &approval,
            &policy,
            policy_file,
            expires_at,
            out.as_deref(),
        ),
        RulesCommand::Apply { request, policy } => run_apply(&request, &policy),
    }
}

pub fn run_replay(args: ReplayArgs) -> Result<(), Box<dyn Error>> {
    let file = fs::File::open(&args.ledger)?;
    let reader = BufReader::new(file);
    let mut count = 0usize;
    let mut denied = 0usize;
    for (line_number, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let decision: PolicyEngineDecision = serde_json::from_str(&line).map_err(|err| {
            format!(
                "invalid policy decision at {}:{}: {err}",
                args.ledger.display(),
                line_number + 1
            )
        })?;
        if decision.schema_version != "larql.governance.policy_decision.v1" {
            return Err(format!(
                "unsupported policy decision schema at {}:{}",
                args.ledger.display(),
                line_number + 1
            )
            .into());
        }
        if decision.policy_hash.trim().is_empty() || decision.input_state_hash.trim().is_empty() {
            return Err(format!(
                "decision missing replay hash at {}:{}",
                args.ledger.display(),
                line_number + 1
            )
            .into());
        }
        if matches!(decision.decision, DecisionKind::Deny | DecisionKind::Fatal) {
            denied += 1;
        }
        count += 1;
    }
    write_json_or_print(
        None,
        &serde_json::json!({
            "schema_version": "larql.governance.replay_report.v1",
            "ledger": args.ledger.display().to_string(),
            "decisions": count,
            "denied_or_fatal": denied,
            "passed": true
        }),
    )
}

fn run_verify(registry: &Path, out: Option<&Path>) -> Result<(), Box<dyn Error>> {
    let registry_value: GovernanceInvariantRegistry =
        serde_json::from_str(&fs::read_to_string(registry)?)?;
    let decision = validate_invariant_registry(&registry_value)?;
    let receipt = larql_governance::PolicyReceipt {
        schema_version: "larql.governance.policy.receipt.v1".to_string(),
        event_type: "InvariantRegistryVerified".to_string(),
        profile: decision.profile.clone(),
        spec_sha256: Some(larql_governance::hash_bytes(&fs::read(registry)?)),
        decision,
    };
    let capsule = larql_governance::make_policy_receipt_capsule(receipt)?;
    match out {
        Some(path) => {
            ensure_repo_relative_path(path)?;
            larql_core::capsule::write_capsule_json(path, &capsule)?;
        }
        None => {
            let mut bytes = larql_core::capsule::canonical_json_bytes(&capsule)?;
            bytes.push(b'\n');
            print!("{}", String::from_utf8(bytes)?);
        }
    }
    Ok(())
}

fn run_eval(facts: &Path, policy: &Path, ledger: Option<&Path>) -> Result<(), Box<dyn Error>> {
    let registry = load_policy_registry(policy)?;
    let engine = PolicyEngine::new(registry)?;
    let input: PolicyInput = serde_json::from_str(&fs::read_to_string(facts)?)?;
    let decision = engine.evaluate(&input);
    if let Some(path) = ledger {
        append_ledger(path, &decision)?;
    }
    write_json_or_print(None, &decision)?;
    if matches!(decision.decision, DecisionKind::Deny | DecisionKind::Fatal) {
        return Err("policy evaluation denied".into());
    }
    Ok(())
}

fn run_test(cases: &Path, policy: &Path) -> Result<(), Box<dyn Error>> {
    let registry = load_policy_registry(policy)?;
    let engine = PolicyEngine::new(registry)?;
    let suite: PolicyTestSuite = toml::from_str(&fs::read_to_string(cases)?)?;
    let report = engine.test_suite(&suite);
    let failed = report.cases.iter().any(|case| !case.passed);
    write_json_or_print(None, &report)?;
    if failed {
        return Err(format!("policy test failed: {}", cases.display()).into());
    }
    Ok(())
}

fn run_shadow_eval(proposal: &Path, facts: &Path, policy: &Path) -> Result<(), Box<dyn Error>> {
    let registry = load_policy_registry(policy)?;
    let engine = PolicyEngine::new(registry)?;
    let proposal: RuleProposal = serde_json::from_str(&fs::read_to_string(proposal)?)?;
    let input: PolicyInput = serde_json::from_str(&fs::read_to_string(facts)?)?;
    let result = engine.shadow_eval(proposal, &input)?;
    write_json_or_print(None, &result)
}

fn run_conflict_check(proposal: &Path, policy: &Path) -> Result<(), Box<dyn Error>> {
    let registry = load_policy_registry(policy)?;
    let engine = PolicyEngine::new(registry)?;
    let proposal: RuleProposal = serde_json::from_str(&fs::read_to_string(proposal)?)?;
    let result = engine.conflict_analysis(&proposal)?;
    write_json_or_print(None, &result)
}

fn run_propose(
    from_prose: String,
    policy_class: Option<String>,
    target: String,
    policy: &Path,
    prior_state_hash: String,
    actor: String,
    ceremony_id: Option<String>,
    out: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    if let Some(value) = policy_class {
        parse_policy_class(&value)?;
    }
    let registry = load_policy_registry(policy)?;
    let concern_hash = hash_text(&format!("{actor}:{target}:{from_prose}"));
    let suffix = concern_hash
        .split(':')
        .next_back()
        .unwrap_or(&concern_hash)
        .chars()
        .take(12)
        .collect::<String>();
    let concern = ProsePolicyConcern {
        event_type: "ProsePolicyConcernSubmitted".to_string(),
        ceremony_id: ceremony_id.unwrap_or_else(|| format!("policy_update_{suffix}")),
        actor,
        prior_policy_hash: registry.policy_hash,
        prior_state_hash,
        prose: from_prose,
        target_area: target,
        status: "submitted".to_string(),
    };
    write_json_or_print(out, &concern)
}

fn run_extract_intent(
    concern: &Path,
    policy_class: String,
    action: String,
    required_evidence: Vec<String>,
    default_decision: String,
    uncertainty: Vec<String>,
    out: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    if required_evidence.is_empty() {
        return Err("intent requires at least one --required-evidence".into());
    }
    let concern: ProsePolicyConcern = serde_json::from_str(&fs::read_to_string(concern)?)?;
    let extraction = PolicyIntentExtraction {
        event_type: "PolicyIntentExtracted".to_string(),
        ceremony_id: concern.ceremony_id,
        intent: PolicyIntent {
            policy_class: parse_policy_class(&policy_class)?,
            action,
            required_evidence,
            default_decision,
        },
        uncertainties: uncertainty,
    };
    write_json_or_print(out, &extraction)
}

fn run_draft(
    concern: &Path,
    intent: &Path,
    update_type: String,
    proposer: String,
    out: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    let concern: ProsePolicyConcern = serde_json::from_str(&fs::read_to_string(concern)?)?;
    let intent: PolicyIntentExtraction = serde_json::from_str(&fs::read_to_string(intent)?)?;
    let proposal = draft_rule_proposal_from_intent(
        &concern,
        &intent,
        parse_update_type(&update_type)?,
        proposer,
    )?;
    write_json_or_print(out, &proposal)
}

fn run_ceremony(trace: &Path) -> Result<(), Box<dyn Error>> {
    let trace: PolicyUpdateCeremonyTrace = serde_json::from_str(&fs::read_to_string(trace)?)?;
    let report = validate_policy_update_ceremony_trace(&trace);
    let passed = report.passed;
    write_json_or_print(None, &report)?;
    if !passed {
        return Err("policy update ceremony failed".into());
    }
    Ok(())
}

fn run_mint_capability(
    proposal: &Path,
    approval: &Path,
    policy: &Path,
    policy_file: String,
    expires_at: String,
    out: Option<&Path>,
) -> Result<(), Box<dyn Error>> {
    let registry = load_policy_registry(policy)?;
    let proposal: RuleProposal = serde_json::from_str(&fs::read_to_string(proposal)?)?;
    let approval: PolicyApproval = serde_json::from_str(&fs::read_to_string(approval)?)?;
    let capability =
        mint_policy_update_capability(&registry, &proposal, &approval, policy_file, expires_at)?;
    write_json_or_print(out, &capability)
}

fn run_apply(request: &Path, policy: &Path) -> Result<(), Box<dyn Error>> {
    let registry = load_policy_registry(policy)?;
    let request: PolicyApplyRequest = serde_json::from_str(&fs::read_to_string(request)?)?;
    validate_policy_apply_request(&registry, &request)?;
    let policy_file = PathBuf::from(&request.target_policy_file);
    ensure_repo_relative_path(&policy_file)?;
    let mut file = OpenOptions::new()
        .create(false)
        .append(true)
        .open(&policy_file)?;
    writeln!(file)?;
    writeln!(file, "{}", rule_as_toml(&request.proposal.proposed_rule)?)?;
    drop(file);

    let new_registry = load_policy_registry(policy)?;
    let receipt = PolicyApplyReceipt {
        schema_version: "larql.governance.policy_apply_receipt.v1".to_string(),
        event_type: "PolicyFileUpdated".to_string(),
        rule_id: request.proposal.proposed_rule.id,
        policy_file: request.target_policy_file,
        prior_policy_hash: registry.policy_hash,
        new_policy_hash: new_registry.policy_hash,
        prior_state_hash: request.capability.prior_state_hash,
        applied_by_capability: request.capability.capability_type,
        required_checks: request.capability.required_checks,
    };
    write_json_or_print(None, &receipt)
}

fn append_ledger(path: &Path, decision: &PolicyEngineDecision) -> Result<(), Box<dyn Error>> {
    ensure_repo_relative_path(path)?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    serde_json::to_writer(&mut file, decision)?;
    writeln!(file)?;
    Ok(())
}

fn rule_as_toml(rule: &PolicyRule) -> Result<String, Box<dyn Error>> {
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

fn parse_policy_class(value: &str) -> Result<PolicyClass, Box<dyn Error>> {
    match value {
        "ceremony_policy" => Ok(PolicyClass::CeremonyPolicy),
        "runtime_policy" => Ok(PolicyClass::RuntimePolicy),
        "ci_policy" => Ok(PolicyClass::CiPolicy),
        "capability_policy" => Ok(PolicyClass::CapabilityPolicy),
        "artifact_policy" => Ok(PolicyClass::ArtifactPolicy),
        "activation_policy" => Ok(PolicyClass::ActivationPolicy),
        "constitutional_policy" => Ok(PolicyClass::ConstitutionalPolicy),
        "training_policy" => Ok(PolicyClass::TrainingPolicy),
        _ => Err(format!("unknown policy class: {value}").into()),
    }
}

fn parse_update_type(value: &str) -> Result<PolicyUpdateType, Box<dyn Error>> {
    match value {
        "new_rule" => Ok(PolicyUpdateType::NewRule),
        "rule_tightening" => Ok(PolicyUpdateType::RuleTightening),
        "rule_relaxation" => Ok(PolicyUpdateType::RuleRelaxation),
        "clarification" => Ok(PolicyUpdateType::Clarification),
        "scope_change" => Ok(PolicyUpdateType::ScopeChange),
        "severity_change" => Ok(PolicyUpdateType::SeverityChange),
        "constitutional_change" => Ok(PolicyUpdateType::ConstitutionalChange),
        _ => Err(format!("unknown policy update type: {value}").into()),
    }
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

#[allow(dead_code)]
fn _keep_types_reachable(
    _: &PolicyRegistry,
    _: &PolicyCapability,
    _: &ConditionBlock,
    _: &PolicyTestReport,
) {
}
