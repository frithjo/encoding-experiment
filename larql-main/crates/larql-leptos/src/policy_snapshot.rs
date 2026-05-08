use larql_policy_engine_projection::{
    build_policy_engine_state_snapshot_from_embedded_workspace_policies, sample_repo_ci_policy_input,
    PolicyEngineEvaluationSnapshot, PolicyEngineStateSnapshot, PolicyRegistrySnapshot,
};
use leptos::*;

/// Registry excerpt emitted by Rust — renders leaves only (no synthesized policy meaning).
#[component]
pub fn PolicyRegistrySnapshotView(registry: PolicyRegistrySnapshot) -> impl IntoView {
    view! {
        <>
            <h2>"Registry excerpt"</h2>
            <dl class="policy-engine-dl">
                <dt>"registry.schema_version"</dt>
                <dd>{registry.schema_version.clone()}</dd>
                <dt>"active_policy_set"</dt>
                <dd>{format!(
                    "{} @ {}",
                    registry.active_policy_set_id.clone(),
                    registry.active_policy_set_version.clone(),
                )}</dd>
                <dt>"registry.policy_hash"</dt>
                <dd class="mono">{registry.policy_hash.clone()}</dd>
                <dt>"registry.rule_count"</dt>
                <dd>{registry.rule_count}</dd>
                <dt>"mode"</dt>
                <dd>{format!(
                    "unknown_rule={:?}, unknown_fact={:?}, conflict_resolution={:?}",
                    registry.mode.unknown_rule,
                    registry.mode.unknown_fact,
                    registry.mode.conflict_resolution,
                )}</dd>
            </dl>
        </>
    }
}

#[component]
pub fn PolicyEngineEvaluationSnapshotView(
    evaluation: PolicyEngineEvaluationSnapshot,
) -> impl IntoView {
    let decision_for_view = evaluation.decision.decision.clone();
    let matched_rules = evaluation.decision.matched_rules.clone();
    let required_actions = evaluation.decision.required_actions.clone();
    let evidence_entries = evaluation.decision.evidence.clone();
    let findings = evaluation.decision.findings.clone();

    view! {
        <>
            <h2>"Evaluation (typed leaves)"</h2>
            <dl class="policy-engine-dl">
                <dt>"policy_input.sha256"</dt>
                <dd class="mono">{evaluation.policy_input_sha256.clone()}</dd>
                <dt>"policy_input.schema_version"</dt>
                <dd>{evaluation.policy_input.schema_version.clone()}</dd>
                <dt>"policy_input.state_hash"</dt>
                <dd class="mono">{evaluation.policy_input.state_hash.clone()}</dd>
                <dt>"policy_input.actor"</dt>
                <dd>{evaluation.policy_input.actor.clone()}</dd>
                <dt>"policy_input.action"</dt>
                <dd>{evaluation.policy_input.action.clone()}</dd>
                <dt>"decision.sha256"</dt>
                <dd class="mono">{evaluation.decision_sha256.clone()}</dd>
                <dt>"decision.schema_version"</dt>
                <dd>{evaluation.decision.schema_version.clone()}</dd>
                <dt>"decision.decision (enum)"</dt>
                <dd class="mono">{format!("{:?}", decision_for_view)}</dd>
                <dt>"decision.active_policy_set"</dt>
                <dd>{evaluation.decision.active_policy_set.clone()}</dd>
                <dt>"decision.policy_hash"</dt>
                <dd class="mono">{evaluation.decision.policy_hash.clone()}</dd>
                <dt>"decision.input_state_hash"</dt>
                <dd class="mono">{evaluation.decision.input_state_hash.clone()}</dd>
            </dl>

            <h3>"Matched rules (ordered as in snapshot)"</h3>
            <ul class="policy-engine-list">
                {matched_rules
                    .into_iter()
                    .map(|rule_id| view! { <li class="mono">{rule_id}</li> })
                    .collect_view()}
            </ul>

            <h3>"Required actions"</h3>
            <ul class="policy-engine-list">
                {required_actions
                    .into_iter()
                    .map(|action| view! { <li class="mono">{action}</li> })
                    .collect_view()}
            </ul>

            <h3>"Evidence"</h3>
            <ul class="policy-engine-list">
                {evidence_entries
                    .into_iter()
                    .map(|item| view! { <li class="mono">{item}</li> })
                    .collect_view()}
            </ul>

            <h3>"Findings"</h3>
            <ul class="policy-engine-findings">
                {findings
                    .into_iter()
                    .map(|finding| {
                        view! {
                            <li class="finding-card mono">
                                <div>{finding.rule_id.clone()}</div>
                                <div>{format!("severity={:?}", finding.severity)}</div>
                                <div>{format!("decision={:?}", finding.decision)}</div>
                                <div>{finding.message.clone()}</div>
                                <div>{"missing_facts: "}</div>
                                <ul>{finding
                                        .missing_facts
                                        .into_iter()
                                        .map(|fact| view! { <li>{fact}</li> })
                                        .collect_view()}</ul>
                                <div>{"missing_evidence: "}</div>
                                <ul>{finding
                                        .missing_evidence
                                        .into_iter()
                                        .map(|miss| view! { <li>{miss}</li> })
                                        .collect_view()}</ul>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </>
    }
}

/// Phase A policy view: render typed snapshot leaves only — no derived governance meaning.
#[component]
pub fn PolicyEngineSnapshotView(snapshot: PolicyEngineStateSnapshot) -> impl IntoView {
    let PolicyEngineStateSnapshot {
        schema_version,
        snapshot_content_sha256,
        policy_index,
        policy_packs,
        registry,
        evaluation,
    } = snapshot;

    view! {
        <section class="policy-engine-snapshot">
            <header>
                <h1>"Policy engine state (read-only)"</h1>
                <p class="policy-engine-hint">"Projection from Rust-authored snapshot only; no client-side policy semantics."</p>
            </header>

            <dl class="policy-engine-dl">
                <dt>"snapshot schema_version"</dt>
                <dd>{schema_version.clone()}</dd>
                <dt>"snapshot_content_sha256"</dt>
                <dd class="mono">{snapshot_content_sha256.clone()}</dd>
            </dl>

            <h2>"Policy source fingerprints"</h2>
            <dl class="policy-engine-dl">
                <dt>"policy_index.repo_relative_path"</dt>
                <dd class="mono">{policy_index.repo_relative_path.clone()}</dd>
                <dt>"policy_index.content_sha256"</dt>
                <dd class="mono">{policy_index.content_sha256.clone()}</dd>
            </dl>
            <table class="policy-engine-table">
                <thead>
                    <tr>
                        <th>"repo_relative_path"</th>
                        <th>"content_sha256"</th>
                    </tr>
                </thead>
                <tbody>
                    {policy_packs
                        .into_iter()
                        .map(|pack| {
                            view! {
                                <tr>
                                    <td class="mono">{pack.repo_relative_path.clone()}</td>
                                    <td class="mono">{pack.content_sha256.clone()}</td>
                                </tr>
                            }
                        })
                        .collect_view()}
                </tbody>
            </table>

            <PolicyRegistrySnapshotView registry/>
            <PolicyEngineEvaluationSnapshotView evaluation/>
        </section>
    }
}

#[component]
pub fn PolicyEngineSnapshotPage() -> impl IntoView {
    match embedded_snapshot_projection() {
        Ok(snapshot) => view! {
            <PolicyEngineSnapshotView snapshot=snapshot/>
        }.into_view(),
        Err(message) => view! {
            <section class="policy-engine-error">
                <p>"Embedded policy snapshot unavailable."</p>
                <pre class="mono">{message}</pre>
            </section>
        }
        .into_view(),
    }
}

fn embedded_snapshot_projection() -> Result<PolicyEngineStateSnapshot, String> {
    let input = sample_repo_ci_policy_input().map_err(|err| format!("policy input fixture: {err}"))?;
    build_policy_engine_state_snapshot_from_embedded_workspace_policies(&input)
        .map_err(|err| format!("embedded snapshot error: {err}"))
}
