use larql_policy_engine_projection::{
    build_ceremony_debt_state_snapshot_embedded, CeremonyDebtStateSnapshot,
    CeremonyDecisionBudgets, CeremonyDefinitionLeaf, DecisionSurfaceItem, DecisionSurfaceRegistry,
    PolicySourceFingerprint,
};
use leptos::*;

#[component]
fn CeremonyDebtSourcesView(sources: Vec<PolicySourceFingerprint>) -> impl IntoView {
    view! {
        <h2>"Embedded source fingerprints"</h2>
        <p class="policy-engine-hint">"Each row is a repo-relative path with a content hash from the embedded bundle (no host paths)."</p>
        <table class="policy-engine-table">
            <thead>
                <tr>
                    <th>"repo_relative_path"</th>
                    <th>"content_sha256"</th>
                </tr>
            </thead>
            <tbody>
                {sources
                    .into_iter()
                    .map(|source| {
                        view! {
                            <tr>
                                <td class="mono">{source.repo_relative_path.clone()}</td>
                                <td class="mono">{source.content_sha256.clone()}</td>
                            </tr>
                        }
                    })
                    .collect_view()}
            </tbody>
        </table>
    }
}

#[component]
fn CeremonyBudgetsView(budgets: CeremonyDecisionBudgets) -> impl IntoView {
    view! {
        <h2>"Ceremony decision budgets"</h2>
        <dl class="policy-engine-dl">
            <dt>"budgets.schema_version"</dt>
            <dd>{budgets.schema_version.clone()}</dd>
        </dl>
        <table class="policy-engine-table">
            <thead>
                <tr>
                    <th>"ceremony id"</th>
                    <th>"max_open_llm_decisions"</th>
                </tr>
            </thead>
            <tbody>
                {budgets
                    .ceremony
                    .into_iter()
                    .map(|(id, budget)| {
                        view! {
                            <tr>
                                <td class="mono">{id.clone()}</td>
                                <td>{budget.max_open_llm_decisions.to_string()}</td>
                            </tr>
                        }
                    })
                    .collect_view()}
            </tbody>
        </table>
    }
}

#[component]
fn DecisionSurfaceRegistryHeader(registry: DecisionSurfaceRegistry) -> impl IntoView {
    let schema_version = registry.schema_version.clone();
    let decision_len = registry.decision.len();

    view! {
        <h2>"Decision surface registry (typed registry leaf)"</h2>
        <dl class="policy-engine-dl">
            <dt>"registry.schema_version"</dt>
            <dd>{schema_version}</dd>
            <dt>"registry.decisions.len"</dt>
            <dd>{decision_len.to_string()}</dd>
        </dl>
    }
}

#[component]
fn CeremonyDefinitionView(definition: CeremonyDefinitionLeaf) -> impl IntoView {
    let CeremonyDefinitionLeaf {
        repo_relative_path,
        content_sha256,
        schema_version,
        id,
        name,
        max_open_llm_decisions,
        phases,
        required_evidence,
        required_artifacts,
        authority,
    } = definition;

    view! {
        <article class="policy-engine-card">
            <header>
                <h3>{format!("Ceremony `{}`", id.clone())}</h3>
            </header>
            <dl class="policy-engine-dl">
                <dt>"definition.repo_relative_path"</dt>
                <dd class="mono">{repo_relative_path.clone()}</dd>
                <dt>"definition.content_sha256"</dt>
                <dd class="mono">{content_sha256.clone()}</dd>
                <dt>"definition.schema_version"</dt>
                <dd>{schema_version.clone()}</dd>
                <dt>"definition.name"</dt>
                <dd>{name.clone()}</dd>
                <dt>"definition.max_open_llm_decisions"</dt>
                <dd>{match max_open_llm_decisions {
                        Some(limit) => limit.to_string(),
                        None => "".to_string(),
                    }}</dd>
            </dl>

            <h4>"phases"</h4>
            <ol class="policy-engine-list">{phases.into_iter().map(|phase| {
                view! { <li class="mono">{phase}</li> }
            }).collect_view()}</ol>

            <h4>"required_evidence"</h4>
            <ul class="policy-engine-list">{required_evidence
                .into_iter()
                .map(|token| view! { <li class="mono">{token}</li> })
                .collect_view()}</ul>

            <h4>"required_artifacts"</h4>
            <ul class="policy-engine-list">{required_artifacts
                .into_iter()
                .map(|artifact| view! { <li class="mono">{artifact}</li> })
                .collect_view()}</ul>

            <h4>"authority (flattened TOML leaves)"</h4>
            <dl class="policy-engine-dl">
                {authority
                    .into_iter()
                    .map(|(key, value)| {
                        view! {
                            <dt class="mono">{key}</dt>
                            <dd>{value}</dd>
                        }
                    })
                    .collect_view()}
            </dl>
        </article>
    }
}

#[component]
fn DebtDecisionLeafView(item: DecisionSurfaceItem) -> impl IntoView {
    view! {
        <article class="policy-engine-findings-item">
            <div class="mono">{item.id.clone()}</div>
            <dl class="policy-engine-dl">
                <dt>"question"</dt>
                <dd>{item.question.clone()}</dd>
                <dt>"current_owner"</dt>
                <dd class="mono">{format!("{:?}", item.current_owner)}</dd>
                <dt>"target_owner"</dt>
                <dd class="mono">{format!("{:?}", item.target_owner)}</dd>
                <dt>"status"</dt>
                <dd class="mono">{format!("{:?}", item.status)}</dd>
                <dt>"promotion_path"</dt>
                <dd class="mono">{format!("{:?}", item.promotion_path)}</dd>
                <dt>"risk"</dt>
                <dd class="mono">{format!("{:?}", item.risk)}</dd>
                <dt>"ceremony"</dt>
                <dd class="mono">{item.ceremony.clone().unwrap_or_default()}</dd>
                <dt>"promotion_artifact"</dt>
                <dd class="mono">{item.promotion_artifact.clone().unwrap_or_default()}</dd>
            </dl>
        </article>
    }
}

/// Phase C — ceremony/debt: render [`CeremonyDebtStateSnapshot`] leaves only (no client semantics).
#[component]
pub fn CeremonyDebtSnapshotView(snapshot: CeremonyDebtStateSnapshot) -> impl IntoView {
    let CeremonyDebtStateSnapshot {
        schema_version,
        snapshot_content_sha256,
        sources,
        ceremony_budgets,
        decision_surface_registry,
        ceremony_definitions,
        debt_decisions,
    } = snapshot;

    view! {
        <section class="policy-engine-preview">
            <header>
                <h1>"Ceremony & decision surface"</h1>
                <p class="policy-engine-hint">"Rust-authored ceremony/decision payloads; hosts never synthesize governance outcomes."</p>
            </header>

            <dl class="policy-engine-dl">
                <dt>"snapshot.schema_version"</dt>
                <dd>{schema_version.clone()}</dd>
                <dt>"snapshot_content_sha256"</dt>
                <dd class="mono">{snapshot_content_sha256.clone()}</dd>
            </dl>

            <CeremonyDebtSourcesView sources=sources/>
            <CeremonyBudgetsView budgets=ceremony_budgets.clone()/>
            <DecisionSurfaceRegistryHeader registry=decision_surface_registry.clone()/>

            <h2>"Embedded ceremony definitions"</h2>
            <div class="policy-engine-cards">
                {ceremony_definitions
                    .into_iter()
                    .map(|definition| {
                        view! { <CeremonyDefinitionView definition/> }
                    })
                    .collect_view()}
            </div>

            <h2>"Debt subset from snapshot (Rust: status ∈ {{open, assisted}})"</h2>
            <ul class="policy-engine-findings">
                {debt_decisions
                    .into_iter()
                    .map(|item| {
                        view! {
                            <li>
                                <DebtDecisionLeafView item/>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </section>
    }
}

#[component]
pub fn CeremonyDebtPage() -> impl IntoView {
    match build_ceremony_debt_state_snapshot_embedded().map_err(|err| format!("{err:?}")) {
        Ok(snapshot) => view! { <CeremonyDebtSnapshotView snapshot/> }.into_view(),
        Err(message) => view! {
            <section class="policy-engine-error">
                <p>"Embedded ceremony/debt projection unavailable."</p>
                <pre class="mono">{message}</pre>
            </section>
        }
        .into_view(),
    }
}
