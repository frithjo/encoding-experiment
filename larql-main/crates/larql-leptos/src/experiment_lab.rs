use leptos::*;

use super::*;

#[component]
pub(crate) fn ExperimentLab() -> impl IntoView {
    let (workspace_path, set_workspace_path) = create_signal(String::new());
    let (top_k_raw, set_top_k_raw) = create_signal("40".to_string());
    let (output_path, set_output_path) = create_signal(String::new());
    let (loading, set_loading) = create_signal(false);
    let (error, set_error) = create_signal(None::<String>);
    let (result, set_result) = create_signal(None::<BitPerfectEraserRunResponse>);

    let run = move |_| {
        let workspace = workspace_path.get().trim().to_string();
        let output = output_path.get().trim().to_string();
        let top_k = match top_k_raw.get().trim().parse::<usize>() {
            Ok(value) if value > 0 => value,
            _ => {
                set_error.set(Some("top_k must be a positive integer".to_string()));
                return;
            }
        };
        if workspace.is_empty() {
            set_error.set(Some("Workspace path is required".to_string()));
            return;
        }

        set_loading.set(true);
        set_error.set(None);
        set_result.set(None);

        wasm_bindgen_futures::spawn_local(async move {
            let request = BitPerfectEraserRunRequest {
                workspace_path: workspace,
                top_k,
                output_path: if output.is_empty() {
                    None
                } else {
                    Some(output)
                },
            };
            match invoke_tauri_bit_perfect_eraser(request).await {
                Ok(resp) => {
                    set_result.set(Some(resp));
                    set_loading.set(false);
                }
                Err(e) => {
                    set_error.set(Some(e));
                    set_loading.set(false);
                }
            }
        });
    };

    view! {
        <>
            <WorkbenchChrome active_task="Experiment Lab"/>
            <section class="workspace">
                <div class="work-grid">
                    <div class="panel">
                        <div class="eyebrow">"Rust-native experiment"</div>
                        <h1>"Bit-Perfect Eraser Audit"</h1>
                        <p class="muted">
                            "Single-task facade for running the Rust experiment engine and projecting its evidence artifact."
                        </p>
                        <div class="status-strip">
                            <div class="status-card">
                                <strong>"Execution"</strong>
                                <span>"Tauri command -> larql-workbench-core -> larql-inference"</span>
                            </div>
                            <div class="status-card">
                                <strong>"Model scope"</strong>
                                <span>"Real vindex workspace, CPU/Linux path"</span>
                            </div>
                            <div class="status-card">
                                <strong>"UI role"</strong>
                                <span>"Projection only; no experiment semantics live here"</span>
                            </div>
                        </div>

                        <details class="drawer-panel" open>
                            <summary>"Protocol controls"</summary>
                            <div class="field-stack" style="margin-top: 10px;">
                                <label>
                                    "Workspace (.vindex path)"
                                    <input
                                        type="text"
                                        prop:value=workspace_path
                                        on:input=move |e| set_workspace_path.set(event_target_value(&e))
                                        placeholder="/path/to/model.vindex"
                                    />
                                </label>
                                <label>
                                    "Top-k probe count"
                                    <input
                                        type="number"
                                        min="1"
                                        prop:value=top_k_raw
                                        on:input=move |e| set_top_k_raw.set(event_target_value(&e))
                                    />
                                </label>
                                <label>
                                    "Output evidence JSON (optional)"
                                    <input
                                        type="text"
                                        prop:value=output_path
                                        on:input=move |e| set_output_path.set(event_target_value(&e))
                                        placeholder="docs/scientific-alignment/bit-perfect-eraser.json"
                                    />
                                </label>
                            </div>
                            <div class="toolbar">
                                <button class="primary-button" on:click=run disabled=loading>
                                    {move || {
                                        if loading.get() {
                                            "Running protocol..."
                                        } else {
                                            "Run experiment"
                                        }
                                    }}
                                </button>
                            </div>
                        </details>

                        {move || {
                            if let Some(err) = error.get() {
                                view! { <div class="error">{err}</div> }.into_view()
                            } else {
                                view! {}.into_view()
                            }
                        }}

                        <details class="drawer-panel">
                            <summary>"Immutable hypothesis under test"</summary>
                            <p class="muted" style="margin-top: 10px;">
                                "\"Surgical Isolation: By zeroing only 3 specific slots (24:1618, 25:3262, 23:1532) in a 4B parameter model, we absolutely erase the fact \"Atlantis -> Poseidon.\"\""
                            </p>
                            <p class="muted">
                                "\"Bit-Perfect Invariance: Because these slots never fire for \"France,\" the output for \"France\" remains binary identical.\""
                            </p>
                        </details>
                    </div>

                    <div class="result-panel">
                        <div class="eyebrow">"Evidence projection"</div>
                        <h2>"Run output"</h2>
                        {move || {
                            if let Some(resp) = result.get() {
                                let artifact_json = serde_json::to_string_pretty(&resp.artifact)
                                    .unwrap_or_else(|_| "{}".to_string());
                                let claim = extract_claim_assessment(&resp.artifact);
                                let facts = extract_mechanistic_facts(&resp.artifact);
                                let comparisons = extract_experiment_comparisons(&resp.artifact);
                                let slots = extract_experiment_slots(&resp.artifact);
                                view! {
                                    <>
                                        <div class="status-strip">
                                            <div class="status-card">
                                                <strong>"Summary"</strong>
                                                <span>{resp.summary.clone()}</span>
                                            </div>
                                            <div class="status-card">
                                                <strong>"Artifact"</strong>
                                                <span>{resp.output_path.clone()}</span>
                                            </div>
                                            <div class="status-card">
                                                <strong>"Duration"</strong>
                                                <span>{resp.duration_ms}" ms"</span>
                                            </div>
                                        </div>

                                        <h2>"Claim assessment"</h2>
                                        <div class="attention-grid">
                                            <div class="layer-card">
                                                <h3>"Verdict"</h3>
                                                <p>"Claim supported: "{claim.claim_supported.to_string()}</p>
                                                <p>"Claim refuted: "{claim.claim_refuted.to_string()}</p>
                                            </div>
                                            <div class="layer-card">
                                                <h3>"Criteria"</h3>
                                                <p>"Slots edited: "{claim.target_slots_all_edited.to_string()}</p>
                                                <p>"Atlantis changed: "{claim.atlantis_output_changed.to_string()}</p>
                                                <p>"Atlantis->Poseidon erased: "{claim.atlantis_poseidon_erased.to_string()}</p>
                                                <p>"France bit-invariant: "{claim.france_bit_perfect_invariant.to_string()}</p>
                                                <p>"France output changed: "{claim.france_output_changed.to_string()}</p>
                                                <p>"France hidden changed: "{claim.france_hidden_changed.to_string()}</p>
                                            </div>
                                        </div>
                                        {if claim.notes.is_empty() {
                                            view! {}.into_view()
                                        } else {
                                            view! {
                                                <div class="status-card" style="margin-top: 12px;">
                                                    <strong>"Assessment notes"</strong>
                                                    <span>{claim.notes.join(" ")}</span>
                                                </div>
                                            }.into_view()
                                        }}
                                        {if facts.is_empty() {
                                            view! {}.into_view()
                                        } else {
                                            view! {
                                                <div class="status-card" style="margin-top: 12px;">
                                                    <strong>"Mechanistic facts"</strong>
                                                    <span>{facts.join(" ")}</span>
                                                </div>
                                            }.into_view()
                                        }}

                                        <h2>"Comparisons"</h2>
                                        <div class="attention-grid">
                                            {comparisons.into_iter().map(|item| {
                                                view! {
                                                    <div class="layer-card">
                                                        <h3>{item.prompt}</h3>
                                                        <p>"Top predictions equal: "{item.top_equal.to_string()}</p>
                                                        <p>"Final hidden bits equal: "{item.bits_equal.to_string()}</p>
                                                        <p>"Max abs delta: "{format!("{:.6e}", item.max_abs_delta)}</p>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>

                                        <h2 style="margin-top: 14px;">"Slot edits"</h2>
                                        <div class="attention-grid">
                                            {slots.into_iter().map(|slot| {
                                                view! {
                                                    <div class="layer-card">
                                                        <h3>{format!("Layer {} Feature {}", slot.layer, slot.feature)}</h3>
                                                        <p>"Edited: "{slot.edited.to_string()}</p>
                                                        <p>"In range: "{(slot.in_layer_range && slot.in_feature_range).to_string()}</p>
                                                        <p>{slot.note}</p>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>

                                        <details class="drawer-panel" style="margin-top: 16px;">
                                            <summary>"Full evidence JSON"</summary>
                                            <pre style="margin-top: 12px;">{artifact_json}</pre>
                                        </details>
                                    </>
                                }
                                .into_view()
                            } else {
                                view! {
                                    <div class="status-card">
                                        <strong>"No run loaded"</strong>
                                        <span class="muted">
                                            "Choose a real extracted vindex workspace, then run the protocol. This panel only renders the Rust-produced artifact."
                                        </span>
                                    </div>
                                }
                                .into_view()
                            }
                        }}
                    </div>
                </div>
            </section>
            <ArtifactBar status="projection: Experiment Lab facade over Rust evidence artifact".to_string()/>
        </>
    }
}

#[derive(Clone)]
struct ExperimentComparisonView {
    prompt: String,
    top_equal: bool,
    bits_equal: bool,
    max_abs_delta: f64,
}

#[derive(Clone)]
struct ExperimentSlotView {
    layer: usize,
    feature: usize,
    in_layer_range: bool,
    in_feature_range: bool,
    edited: bool,
    note: String,
}

#[derive(Clone, Default)]
struct ClaimAssessmentView {
    target_slots_all_edited: bool,
    atlantis_output_changed: bool,
    atlantis_poseidon_erased: bool,
    france_bit_perfect_invariant: bool,
    france_output_changed: bool,
    france_hidden_changed: bool,
    claim_supported: bool,
    claim_refuted: bool,
    notes: Vec<String>,
}

fn extract_claim_assessment(artifact: &serde_json::Value) -> ClaimAssessmentView {
    let Some(value) = artifact.get("claim_assessment") else {
        return ClaimAssessmentView::default();
    };
    ClaimAssessmentView {
        target_slots_all_edited: bool_field(value, "target_slots_all_edited"),
        atlantis_output_changed: bool_field(value, "atlantis_output_changed"),
        atlantis_poseidon_erased: bool_field(value, "atlantis_poseidon_erased"),
        france_bit_perfect_invariant: bool_field(value, "france_bit_perfect_invariant"),
        france_output_changed: bool_field(value, "france_output_changed"),
        france_hidden_changed: bool_field(value, "france_hidden_changed"),
        claim_supported: bool_field(value, "claim_supported"),
        claim_refuted: bool_field(value, "claim_refuted"),
        notes: value
            .get("notes")
            .and_then(|notes| notes.as_array())
            .map(|notes| {
                notes
                    .iter()
                    .filter_map(|note| note.as_str().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn extract_mechanistic_facts(artifact: &serde_json::Value) -> Vec<String> {
    artifact
        .get("mechanistic_facts")
        .and_then(|facts| facts.as_array())
        .map(|facts| {
            facts
                .iter()
                .filter_map(|fact| fact.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn bool_field(value: &serde_json::Value, field: &str) -> bool {
    value
        .get(field)
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

fn extract_experiment_comparisons(artifact: &serde_json::Value) -> Vec<ExperimentComparisonView> {
    artifact
        .get("comparisons")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .map(|item| ExperimentComparisonView {
                    prompt: item
                        .get("prompt")
                        .and_then(|value| value.as_str())
                        .unwrap_or("")
                        .to_string(),
                    top_equal: item
                        .get("baseline_top_predictions_equal")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false),
                    bits_equal: item
                        .get("baseline_final_hidden_bits_equal")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false),
                    max_abs_delta: item
                        .get("max_abs_delta")
                        .and_then(|value| value.as_f64())
                        .unwrap_or(0.0),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn extract_experiment_slots(artifact: &serde_json::Value) -> Vec<ExperimentSlotView> {
    artifact
        .get("slots")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .map(|item| ExperimentSlotView {
                    layer: item
                        .get("layer")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(0) as usize,
                    feature: item
                        .get("feature")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(0) as usize,
                    in_layer_range: item
                        .get("in_layer_range")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false),
                    in_feature_range: item
                        .get("in_feature_range")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false),
                    edited: item
                        .get("edited")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false),
                    note: item
                        .get("note")
                        .and_then(|value| value.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}
