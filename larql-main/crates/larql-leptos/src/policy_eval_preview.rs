use crate::policy_snapshot::{PolicyEngineEvaluationSnapshotView, PolicyRegistrySnapshotView};
use larql_policy_engine_projection::{
    build_policy_eval_preview_embedded, policy_input_from_preview_draft, preview_draft_from_ci_fixture,
    PolicyEvalPreview, PolicyEvalPreviewDraft, PreviewFactsSpecification,
};
use leptos::*;
use web_sys::{HtmlInputElement, HtmlTextAreaElement};

#[component]
pub fn PolicyEvalPreviewView(preview: PolicyEvalPreview) -> impl IntoView {
    let PolicyEvalPreview {
        schema_version,
        preview_content_sha256,
        registry,
        evaluation,
    } = preview;

    view! {
        <section class="policy-engine-preview">
            <header>
                <h1>"Policy eval preview"</h1>
                <p class="policy-engine-hint">"Rust-evaluated preview typed payload — UI never synthesizes governance outcomes."</p>
            </header>

            <dl class="policy-engine-dl">
                <dt>"preview schema_version"</dt>
                <dd>{schema_version.clone()}</dd>
                <dt>"preview_content_sha256"</dt>
                <dd class="mono">{preview_content_sha256.clone()}</dd>
            </dl>

            <PolicyRegistrySnapshotView registry/>
            <PolicyEngineEvaluationSnapshotView evaluation/>
        </section>
    }
}

#[component]
pub fn PolicyEvalPreviewPage() -> impl IntoView {
    let seeded = preview_draft_from_ci_fixture().unwrap_or_default();

    let actor = create_rw_signal(seeded.actor);
    let action = create_rw_signal(seeded.action);
    let state_hash = create_rw_signal(seeded.state_hash);
    let risk = create_rw_signal(seeded.risk.to_string());
    let target_paths_raw = create_rw_signal(seeded.target_paths_raw);
    let evidence_raw = create_rw_signal(seeded.evidence_raw);
    let fixture_facts_only = create_rw_signal(matches!(seeded.facts, PreviewFactsSpecification::Fixture));
    let facts_custom_json = create_rw_signal(match seeded.facts {
        PreviewFactsSpecification::Fixture => String::new(),
        PreviewFactsSpecification::CustomJson(blob) => blob,
    });
    let last_outcome =
        create_rw_signal::<Option<Result<PolicyEvalPreview, String>>>(None);

    let reset_fixture = move |_| {
        match preview_draft_from_ci_fixture() {
            Ok(next) => {
                actor.set(next.actor.clone());
                action.set(next.action.clone());
                state_hash.set(next.state_hash.clone());
                risk.set(next.risk.to_string());
                target_paths_raw.set(next.target_paths_raw.clone());
                evidence_raw.set(next.evidence_raw.clone());
                fixture_facts_only.set(true);
                facts_custom_json.set(String::new());
                last_outcome.set(None);
            }
            Err(err) => {
                last_outcome.set(Some(Err(format!("fixture load error: {err}"))));
            }
        }
    };

    let run_preview = move |_| {
        let risk_value = match risk.get().trim().parse::<u64>() {
            Ok(value) => value,
            Err(_) => {
                last_outcome.set(Some(Err("risk must be a non-negative integer".into())));
                return;
            }
        };

        let draft = PolicyEvalPreviewDraft {
            actor: actor.get(),
            action: action.get(),
            state_hash: state_hash.get(),
            risk: risk_value,
            target_paths_raw: target_paths_raw.get(),
            evidence_raw: evidence_raw.get(),
            facts: if fixture_facts_only.get() {
                PreviewFactsSpecification::Fixture
            } else {
                PreviewFactsSpecification::CustomJson(facts_custom_json.get())
            },
        };

        let input = match policy_input_from_preview_draft(&draft) {
            Ok(value) => value,
            Err(err) => {
                last_outcome.set(Some(Err(format!("{err}"))));
                return;
            }
        };

        match build_policy_eval_preview_embedded(&input) {
            Ok(preview) => last_outcome.set(Some(Ok(preview))),
            Err(err) => last_outcome.set(Some(Err(format!("{err}")))),
        }
    };

    view! {
        <section class="policy-engine-preview-shell">
            <header>
                <h1>"Build a policy preview"</h1>
                <p class="policy-engine-hint">"Structured drafting only; Rust owns parsing, hashing, and policy evaluation."</p>
            </header>

            <form class="policy-engine-form" on:submit=|ev| ev.prevent_default()>
                <label for="preview-actor">"Actor"</label>
                <textarea
                    id="preview-actor"
                    prop:value=move || actor.get()
                    rows="1"
                    on:input=move |ev| {
                        actor.set(event_target::<HtmlTextAreaElement>(&ev).value())
                    }
                />

                <label for="preview-action">"Action"</label>
                <textarea
                    id="preview-action"
                    prop:value=move || action.get()
                    rows="2"
                    on:input=move |ev| {
                        action.set(event_target::<HtmlTextAreaElement>(&ev).value())
                    }
                />

                <label for="preview-state-hash">"State hash"</label>
                <textarea
                    id="preview-state-hash"
                    prop:value=move || state_hash.get()
                    rows="2"
                    on:input=move |ev| {
                        state_hash.set(event_target::<HtmlTextAreaElement>(&ev).value())
                    }
                />

                <label for="preview-risk">"Risk"</label>
                <input
                    id="preview-risk"
                    type="text"
                    prop:value=move || risk.get()
                    on:input=move |ev| {
                        risk.set(event_target::<HtmlInputElement>(&ev).value())
                    }
                />

                <label for="preview-targets">"Target paths (one line each)"</label>
                <textarea
                    id="preview-targets"
                    prop:value=move || target_paths_raw.get()
                    rows="4"
                    on:input=move |ev| {
                        target_paths_raw.set(event_target::<HtmlTextAreaElement>(&ev).value())
                    }
                />

                <label for="preview-evidence">"Evidence IDs (one line each)"</label>
                <textarea
                    id="preview-evidence"
                    prop:value=move || evidence_raw.get()
                    rows="3"
                    on:input=move |ev| {
                        evidence_raw.set(event_target::<HtmlTextAreaElement>(&ev).value())
                    }
                />

                <fieldset class="preview-facts">
                    <legend>"Facts"</legend>
                    <label>
                        <input
                            type="checkbox"
                            prop:checked=move || fixture_facts_only.get()
                            on:change=move |ev| {
                                fixture_facts_only.set(event_target::<HtmlInputElement>(&ev).checked())
                            }
                        /> "Use bundled CI governance facts"
                    </label>
                    <label for="preview-facts-json">"Overrides (PolicyFact JSON array only — parsed strictly in Rust)"</label>
                    <textarea
                        id="preview-facts-json"
                        prop:value=move || facts_custom_json.get()
                        rows="6"
                        prop:disabled=move || fixture_facts_only.get()
                        on:input=move |ev| {
                            facts_custom_json.set(event_target::<HtmlTextAreaElement>(&ev).value())
                        }
                    />
                </fieldset>

                <div class="policy-engine-buttons">
                    <button type="button" class="brand-button" on:click=reset_fixture>"Reset CI draft"</button>
                    <button type="button" class="brand-button" on:click=run_preview>"Run embedded preview"</button>
                </div>
            </form>

            <div class="policy-engine-preview-results">
                {move || match last_outcome.get() {
                    Some(Ok(payload)) => view! {
                        <PolicyEvalPreviewView preview=payload.clone()/>
                    }.into_view(),
                    Some(Err(message)) => view! {
                        <section class="policy-engine-error">
                            <p>"Preview failed."</p>
                            <pre class="mono">{message}</pre>
                        </section>
                    }.into_view(),
                    None => view! {
                        <p class="policy-engine-muted">"No preview executed yet."</p>
                    }.into_view(),
                }}
            </div>
        </section>
    }
}
