use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;

#[cfg(target_arch = "wasm32")]
use gloo_net::http::Request;
#[cfg(target_arch = "wasm32")]
use js_sys::{Function, Object, Promise, Reflect};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;

mod experiment_lab;
mod lql_console;

use experiment_lab::ExperimentLab;
use lql_console::LqlConsole;

#[derive(Clone, Serialize, Deserialize)]
struct AttentionData {
    layer: usize,
    heads: Vec<Vec<f32>>,
}

#[derive(Clone, Serialize, Deserialize)]
struct LqlRunRequest {
    workspace_path: String,
    query: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct LqlRunResponse {
    lines: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct DescribeRunRequest {
    workspace_path: String,
    entity: String,
    band: Option<String>,
    verbose: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct DescribeRunResponse {
    lines: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct AnalyzeInferRequest {
    server_url: String,
    prompt: String,
    top_k: usize,
    mode: String,
    truth_spans: Vec<String>,
    materially_false_spans: Vec<String>,
    coherence_markers: Vec<String>,
    max_generated_tokens: Option<usize>,
    ridge_dead_zone: Option<f32>,
}

#[derive(Clone, Serialize, Deserialize)]
struct BitPerfectEraserRunRequest {
    workspace_path: String,
    top_k: usize,
    output_path: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct BitPerfectEraserRunResponse {
    summary: String,
    output_path: String,
    duration_ms: u64,
    artifact: serde_json::Value,
}

#[derive(Clone, Serialize, Deserialize)]
struct AnalyzeHeadContribution {
    layer: usize,
    head: usize,
    source_token: usize,
    contribution: f64,
}

#[derive(Clone, Serialize, Deserialize)]
struct AnalyzeStepTopHeadSummary {
    false_content: Vec<AnalyzeHeadContribution>,
    material_coherence: Vec<AnalyzeHeadContribution>,
}

#[derive(Clone, Serialize, Deserialize)]
struct AnalyzeGeneratedStep {
    position: usize,
    token_id: u32,
    token: String,
    probability: f64,
}

#[derive(Clone, Serialize, Deserialize)]
struct AnalyzeTokenAnalysis {
    position: usize,
    token_id: u32,
    token: String,
    probability: f64,
    label: String,
    truth_mass: f64,
    false_mass: f64,
    coherence_mass: f64,
    ridge: f64,
    top_heads: AnalyzeStepTopHeadSummary,
}

#[derive(Clone, Serialize, Deserialize)]
struct AnalyzeFirstFalseOrigin {
    position: usize,
    token_id: u32,
    token: String,
    layer: usize,
    head: usize,
    source_token: usize,
    contribution: f64,
}

#[derive(Clone, Serialize, Deserialize)]
struct AnalyzeSummary {
    first_false_position: Option<usize>,
    first_false_token: Option<String>,
    first_false_origin: Option<AnalyzeFirstFalseOrigin>,
    top_coherence_heads: Vec<AnalyzeHeadContribution>,
    top_false_content_heads: Vec<AnalyzeHeadContribution>,
    materially_false_detected: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct AnalyzeLayerRidge {
    layer: usize,
    ridge: f64,
}

#[derive(Clone, Serialize, Deserialize)]
struct AnalyzeAttentionLayer {
    layer: usize,
    heads: Vec<Vec<f32>>,
}

#[derive(Clone, Serialize, Deserialize)]
struct AnalyzeLogitLensLayer {
    layer: usize,
    predictions: Vec<(String, f64)>,
}

#[derive(Clone, Serialize, Deserialize)]
struct AnalyzeHeadDlaLayer {
    layer: usize,
    heads: Vec<Vec<f32>>,
}

#[derive(Clone, Serialize, Deserialize)]
struct AnalyzeInferResponse {
    attention: Vec<AnalyzeAttentionLayer>,
    logit_lens: Vec<AnalyzeLogitLensLayer>,
    head_dla: Vec<AnalyzeHeadDlaLayer>,
    num_layers: usize,
    seq_len: usize,
    tokens: Vec<u32>,
    strings: Vec<String>,
    predictions: Vec<(String, f64)>,
    generation_trace: Vec<AnalyzeGeneratedStep>,
    token_analysis: Vec<AnalyzeTokenAnalysis>,
    analysis_summary: Option<AnalyzeSummary>,
    ridge_by_layer: Vec<AnalyzeLayerRidge>,
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Title text="LARQL Workbench"/>
        <Router>
            <RouteBootstrap/>
            <main class="app-shell">
                <Routes>
                    <Route path="/" view=StackedWorkbench/>
                    <Route path="/tasks" view=TaskChooser/>
                    <Route path="/lql" view=|| view! { <LqlConsole/> }/>
                    <Route path="/explorer" view=ExplorerDescribe/>
                    <Route path="/batch-dla" view=BatchDlaScan/>
                    <Route path="/experiments" view=|| view! { <ExperimentLab/> }/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn RouteBootstrap() -> impl IntoView {
    #[cfg(target_arch = "wasm32")]
    {
        let navigate = use_navigate();
        create_effect(move |_| {
            let Ok(search) = window().location().search() else {
                return;
            };
            let query = search.strip_prefix('?').unwrap_or(search.as_str());
            let target = query.split('&').find_map(|pair| {
                let mut parts = pair.splitn(2, '=');
                let key = parts.next().unwrap_or_default();
                let value = parts.next().unwrap_or_default();
                if key != "route" {
                    return None;
                }
                match value {
                    "lql" => Some("/lql"),
                    "experiments" => Some("/experiments"),
                    "explorer" => Some("/explorer"),
                    "batch-dla" => Some("/batch-dla"),
                    _ => None,
                }
            });

            if let Some(target) = target {
                navigate(
                    target,
                    NavigateOptions {
                        resolve: false,
                        replace: true,
                        ..Default::default()
                    },
                );
            }
        });
    }

    view! {}
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen(start))]
pub fn main() {
    if console_log::init_with_level(log::Level::Debug).is_ok() {
        log::info!("LARQL WASM logging enabled on activation (level=debug)");
    }
    console_error_panic_hook::set_once();
    log::info!("LARQL WASM mounting app shell");
    leptos::mount_to_body(App);
}

#[component]
fn WorkbenchChrome(active_task: &'static str) -> impl IntoView {
    view! {
        <header class="chrome-bar">
            <RouteAnchor class="brand" href="/">"LARQL"</RouteAnchor>
            <span class="chrome-title">{active_task}</span>
            <span class="chip">"CPU/Linux"</span>
            <span class="chrome-spacer"></span>
            <div class="chrome-actions">
                <RouteAnchor class="nav-button" href="/">"Tasks"</RouteAnchor>
                <RouteAnchor class="nav-button" href="/experiments">"Experiment Lab"</RouteAnchor>
                <RouteAnchor class="nav-button" href="/lql">"LQL Console"</RouteAnchor>
                <details class="task-menu">
                    <summary class="ghost-button">"Tools"</summary>
                    <div class="task-menu-panel">
                        <RouteAnchor class="nav-button" href="/explorer">"Explorer Describe"</RouteAnchor>
                        <RouteAnchor class="nav-button" href="/batch-dla">"Batch DLA"</RouteAnchor>
                    </div>
                </details>
            </div>
        </header>
    }
}

#[component]
fn RouteAnchor(class: &'static str, href: &'static str, children: Children) -> impl IntoView {
    let navigate = use_navigate();

    view! {
        <a
            class=class
            href=href
            on:click=move |ev| {
                if ev.default_prevented()
                    || ev.button() != 0
                    || ev.meta_key()
                    || ev.ctrl_key()
                    || ev.shift_key()
                    || ev.alt_key()
                {
                    return;
                }
                ev.prevent_default();
                navigate(
                    href,
                    NavigateOptions {
                        resolve: false,
                        ..Default::default()
                    },
                );
            }
        >
            {children()}
        </a>
    }
}

#[component]
fn ArtifactBar(status: String) -> impl IntoView {
    view! {
        <footer class="artifact-bar">
            <span>{status}</span>
            <span>"launcher: lq"</span>
        </footer>
    }
}

#[component]
fn TaskChooser() -> impl IntoView {
    view! {
        <>
            <WorkbenchChrome active_task="Task chooser"/>
            <section class="task-chooser">
                <div class="task-chooser-inner">
                    <div class="eyebrow">"Projected work surfaces"</div>
                    <h1>"LARQL Workbench"</h1>
                    <p class="muted">
                        "Pick the task in front of you. Secondary analysis tools stay behind the Tools menu."
                    </p>
                    <div class="task-grid">
                        <RouteAnchor class="task-card" href="/experiments">
                            <span class="metric">"Scientific protocol"</span>
                            <h2>"Experiment Lab"</h2>
                            <p class="muted">
                                "Run Rust-native experiments and inspect evidence artifacts."
                            </p>
                        </RouteAnchor>
                        <RouteAnchor class="task-card" href="/lql">
                            <span class="metric">"Query workspace"</span>
                            <h2>"LQL Console"</h2>
                            <p class="muted">
                                "Write, run, save, and inspect LQL queries against a vindex."
                            </p>
                        </RouteAnchor>
                    </div>
                </div>
            </section>
            <ArtifactBar status="projection: choose one focused work surface".to_string()/>
        </>
    }
}

#[component]
fn StackedWorkbench() -> impl IntoView {
    let (active_task, set_active_task) = create_signal("Experiment Lab");

    #[cfg(not(target_arch = "wasm32"))]
    let _ = set_active_task;

    #[cfg(target_arch = "wasm32")]
    {
        let set_active_task = set_active_task;
        create_effect(move |_| {
            let win = window();

            let update_active: std::rc::Rc<dyn Fn()> = {
                let win = win.clone();
                std::rc::Rc::new(move || {
                    let viewport_mid = win
                        .inner_height()
                        .ok()
                        .and_then(|height| height.as_f64())
                        .unwrap_or(0.0)
                        / 2.0;
                    let scroll_y = win.scroll_y().ok().unwrap_or(0.0);
                    let viewport_mid_in_doc = scroll_y + viewport_mid;
                    let shared_taskbar_mid = win
                        .inner_height()
                        .ok()
                        .and_then(|height| height.as_f64())
                        .map(|height| height - 22.0)
                        .unwrap_or(0.0);
                    if viewport_mid_in_doc < shared_taskbar_mid {
                        set_active_task.set("Experiment Lab");
                    } else {
                        set_active_task.set("LQL Console");
                    }
                })
            };

            update_active();

            let scroll_update = update_active.clone();
            let scroll_listener = Closure::<dyn FnMut()>::new(move || {
                scroll_update();
            });
            let resize_update = update_active.clone();
            let resize_listener = Closure::<dyn FnMut()>::new(move || {
                resize_update();
            });

            let _ = win.add_event_listener_with_callback(
                "scroll",
                scroll_listener.as_ref().unchecked_ref(),
            );
            let _ = win.add_event_listener_with_callback(
                "resize",
                resize_listener.as_ref().unchecked_ref(),
            );

            on_cleanup(move || {
                let _ = win.remove_event_listener_with_callback(
                    "scroll",
                    scroll_listener.as_ref().unchecked_ref(),
                );
                let _ = win.remove_event_listener_with_callback(
                    "resize",
                    resize_listener.as_ref().unchecked_ref(),
                );
                drop(scroll_listener);
                drop(resize_listener);
            });
        });
    }

    view! {
        <div class="stacked-workbench">
            <ExperimentLab embedded=true/>
            <header class="chrome-bar shared-taskbar">
                <RouteAnchor class="brand" href="/">"LARQL"</RouteAnchor>
                <span class="chrome-title">{move || active_task.get()}</span>
                <span class="chip">"CPU/Linux"</span>
                <span class="chrome-spacer"></span>
                <div class="chrome-actions">
                    <RouteAnchor class="nav-button" href="/tasks">"Tasks"</RouteAnchor>
                    <RouteAnchor class="nav-button" href="/experiments">"Experiment Lab"</RouteAnchor>
                    <RouteAnchor class="nav-button" href="/lql">"LQL Console"</RouteAnchor>
                    <details class="task-menu">
                        <summary class="ghost-button">"Tools"</summary>
                        <div class="task-menu-panel">
                            <RouteAnchor class="nav-button" href="/explorer">"Explorer Describe"</RouteAnchor>
                            <RouteAnchor class="nav-button" href="/batch-dla">"Batch DLA"</RouteAnchor>
                        </div>
                    </details>
                </div>
            </header>
            <LqlConsole embedded=true/>
        </div>
    }
}

#[component]
#[allow(non_snake_case)]
fn ExplorerDescribe() -> impl IntoView {
    let (workspace_path, set_workspace_path) = create_signal(String::new());
    let (entity, set_entity) = create_signal(String::new());
    let (band, set_band) = create_signal("knowledge".to_string());
    let (verbose, set_verbose) = create_signal(false);
    let (loading, set_loading) = create_signal(false);
    let (error, set_error) = create_signal(None::<String>);
    let (lines, set_lines) = create_signal(Vec::<String>::new());

    let run = move |_| {
        let workspace = workspace_path.get().trim().to_string();
        let describe_entity = entity.get().trim().to_string();
        let band_value = band.get();
        let include_verbose = verbose.get();

        set_loading.set(true);
        set_error.set(None);
        set_lines.set(Vec::new());

        wasm_bindgen_futures::spawn_local(async move {
            let req = DescribeRunRequest {
                workspace_path: workspace,
                entity: describe_entity,
                band: if band_value == "none" {
                    None
                } else {
                    Some(band_value)
                },
                verbose: include_verbose,
            };
            match invoke_tauri_describe(req).await {
                Ok(resp) => {
                    set_lines.set(resp.lines);
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
            <WorkbenchChrome active_task="Explorer Describe"/>
            <section class="workspace">
                <div class="work-grid">
                    <div class="panel">
                        <div class="eyebrow">"Secondary tool"</div>
                        <h1>"Explorer Describe"</h1>
                        <p class="muted">"Projected facade over the Rust describe executor."</p>
                        <div class="field-stack">
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
                                "Entity"
                                <input
                                    type="text"
                                    prop:value=entity
                                    on:input=move |e| set_entity.set(event_target_value(&e))
                                    placeholder="France"
                                />
                            </label>
                            <label>
                                "Band"
                                <select
                                    prop:value=band
                                    on:change=move |e| set_band.set(event_target_value(&e))
                                >
                                    <option value="knowledge">"knowledge"</option>
                                    <option value="all">"all"</option>
                                    <option value="syntax">"syntax"</option>
                                    <option value="output">"output"</option>
                                    <option value="none">"none"</option>
                                </select>
                            </label>
                            <label>
                                <span>
                                    <input
                                        type="checkbox"
                                        prop:checked=verbose
                                        on:change=move |e| set_verbose.set(event_target_checked(&e))
                                    />
                                    " Verbose"
                                </span>
                            </label>
                        </div>
                        <div class="toolbar">
                            <button class="primary-button" on:click=run disabled=loading>
                                {move || if loading.get() { "Running..." } else { "Describe" }}
                            </button>
                        </div>
                        {move || {
                            if let Some(err) = error.get() {
                                view! { <div class="error">{err}</div> }.into_view()
                            } else {
                                view! {}.into_view()
                            }
                        }}
                    </div>

                    <div class="result-panel">
                        <h2>"Result"</h2>
                        {move || {
                            let current = lines.get();
                            if current.is_empty() {
                                view! {
                                    <div class="status-card">
                                        <strong>"No describe output"</strong>
                                        <span class="muted">"Run DESCRIBE against a real vindex workspace."</span>
                                    </div>
                                }
                                .into_view()
                            } else {
                                view! { <pre>{current.join("\n")}</pre> }.into_view()
                            }
                        }}
                    </div>
                </div>
            </section>
            <ArtifactBar status="projection: Explorer facade over LQL describe".to_string()/>
        </>
    }
}

#[component]
#[allow(non_snake_case)]
fn BatchDlaScan() -> impl IntoView {
    let (server_url, set_server_url) = create_signal("http://127.0.0.1:8080".to_string());
    let (prompt, set_prompt) = create_signal("".to_string());
    let (mode, set_mode) = create_signal("fact_probe".to_string());
    let (truth_spans_raw, set_truth_spans_raw) = create_signal(String::new());
    let (false_spans_raw, set_false_spans_raw) = create_signal(String::new());
    let (coherence_markers_raw, set_coherence_markers_raw) = create_signal(String::new());
    let (max_generated_tokens_raw, set_max_generated_tokens_raw) = create_signal("1".to_string());
    let (ridge_dead_zone_raw, set_ridge_dead_zone_raw) = create_signal("0.05".to_string());
    let (loading, set_loading) = create_signal(false);
    let (error, set_error) = create_signal(None::<String>);
    let (attention_data, set_attention_data) = create_signal(None::<Vec<AttentionData>>);
    let (num_layers, set_num_layers) = create_signal(0usize);
    let (tokens, set_tokens) = create_signal(Vec::<usize>::new());
    let (analysis_dump, set_analysis_dump) = create_signal(String::new());

    let parse_csv = |raw: &str| {
        raw.split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    };

    let run_scan = move |_| {
        let server_url_val = server_url.get().trim().to_string();
        let prompt_val = prompt.get().trim().to_string();
        let mode_val = mode.get().trim().to_ascii_lowercase();
        let truth_spans = parse_csv(&truth_spans_raw.get());
        let materially_false_spans = parse_csv(&false_spans_raw.get());
        let coherence_markers = parse_csv(&coherence_markers_raw.get());
        let max_generated_tokens = max_generated_tokens_raw.get().trim().parse::<usize>().ok();
        let ridge_dead_zone = ridge_dead_zone_raw.get().trim().parse::<f32>().ok();

        if prompt_val.is_empty() {
            set_error.set(Some("Please enter a prompt".to_string()));
            return;
        }
        if mode_val != "fact_probe" && mode_val != "workflow_probe" {
            set_error.set(Some(
                "Mode must be fact_probe or workflow_probe".to_string(),
            ));
            return;
        }
        if mode_val == "fact_probe" && truth_spans.is_empty() && materially_false_spans.is_empty() {
            set_error.set(Some(
                "FACT_PROBE requires at least one TRUTH_SPAN or FALSE_SPAN".to_string(),
            ));
            return;
        }
        if mode_val == "workflow_probe" && materially_false_spans.is_empty() {
            set_error.set(Some("WORKFLOW_PROBE requires FALSE_SPANS".to_string()));
            return;
        }

        set_loading.set(true);
        set_error.set(None);
        set_analysis_dump.set(String::new());
        set_attention_data.set(None);
        set_tokens.set(Vec::new());
        set_num_layers.set(0);

        let server_url_clone = server_url_val.clone();
        let prompt_clone = prompt_val.clone();
        let mode_clone = mode_val.clone();
        let truth_spans_clone = truth_spans.clone();
        let false_spans_clone = materially_false_spans.clone();
        let coherence_markers_clone = coherence_markers.clone();
        let max_generated_tokens_clone = max_generated_tokens;
        let ridge_dead_zone_clone = ridge_dead_zone;

        wasm_bindgen_futures::spawn_local(async move {
            let request = AnalyzeInferRequest {
                server_url: server_url_clone,
                prompt: prompt_clone,
                top_k: 5,
                mode: mode_clone,
                truth_spans: truth_spans_clone,
                materially_false_spans: false_spans_clone,
                coherence_markers: coherence_markers_clone,
                max_generated_tokens: max_generated_tokens_clone,
                ridge_dead_zone: ridge_dead_zone_clone,
            };

            match invoke_tauri_analyze_infer(request).await {
                Ok(result) => {
                    let dump =
                        serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());
                    set_analysis_dump.set(dump);
                    let parsed_attention: Vec<AttentionData> = result
                        .attention
                        .into_iter()
                        .map(|layer| AttentionData {
                            layer: layer.layer,
                            heads: layer.heads,
                        })
                        .collect();
                    set_attention_data.set(Some(parsed_attention));
                    set_num_layers.set(result.num_layers);
                    let parsed_tokens: Vec<usize> = result
                        .tokens
                        .into_iter()
                        .map(|token| token as usize)
                        .collect();
                    set_tokens.set(parsed_tokens);
                    set_loading.set(false);
                }
                Err(e) => {
                    set_loading.set(false);
                    set_error.set(Some(e));
                }
            }
        });
    };

    view! {
        <>
            <WorkbenchChrome active_task="Batch DLA"/>
            <section class="workspace">
                <div class="panel">
                    <div class="eyebrow">"Secondary tool"</div>
                    <h1>"Batch DLA Scan"</h1>

            <div class="input-section">
                <label>"Server URL:"</label>
                <input
                    type="text"
                    prop:value=server_url
                    on:input=move |e| set_server_url.set(event_target_value(&e))
                    placeholder="http://127.0.0.1:8080"
                />

                <label>"Prompt:"</label>
                <textarea
                    prop:value=prompt
                    on:input=move |e| set_prompt.set(event_target_value(&e))
                    placeholder="Enter text to analyze..."
                    rows="4"
                />

                <label style="margin-top: 12px;">"Mode:"</label>
                <select
                    prop:value=mode
                    on:change=move |e| set_mode.set(event_target_value(&e))
                >
                    <option value="fact_probe">"fact_probe"</option>
                    <option value="workflow_probe">"workflow_probe"</option>
                </select>

                <label style="margin-top: 12px;">"Truth Spans (comma-separated):"</label>
                <input
                    type="text"
                    prop:value=truth_spans_raw
                    on:input=move |e| set_truth_spans_raw.set(event_target_value(&e))
                    placeholder="Markov"
                />

                <label style="margin-top: 12px;">"False Spans (comma-separated):"</label>
                <input
                    type="text"
                    prop:value=false_spans_raw
                    on:input=move |e| set_false_spans_raw.set(event_target_value(&e))
                    placeholder="Paris, London"
                />

                <label style="margin-top: 12px;">"Coherence Markers (comma-separated):"</label>
                <input
                    type="text"
                    prop:value=coherence_markers_raw
                    on:input=move |e| set_coherence_markers_raw.set(event_target_value(&e))
                    placeholder="capital, is"
                />

                <label style="margin-top: 12px;">"Max Generated Tokens:"</label>
                <input
                    type="number"
                    min="1"
                    prop:value=max_generated_tokens_raw
                    on:input=move |e| set_max_generated_tokens_raw.set(event_target_value(&e))
                />

                <label style="margin-top: 12px;">"Ridge Dead Zone:"</label>
                <input
                    type="text"
                    prop:value=ridge_dead_zone_raw
                    on:input=move |e| set_ridge_dead_zone_raw.set(event_target_value(&e))
                    placeholder="0.05"
                />
                <button
                    on:click=run_scan
                    disabled=loading
                >
                    {move || if loading.get() { "Scanning..." } else { "Scan" } }
                </button>
            </div>

            {move || {
                if let Some(err) = error.get() {
                    view! {
                        <div class="error">{err}</div>
                    }
                    .into_view()
                } else {
                    view! {}.into_view()
                }
            }}

            {move || {
                if let Some(data) = attention_data.get() {
                    view! {
                        <div class="results">
                            <h2>"Attention Matrices"</h2>
                            <div class="stats">
                                <p>"Layers: "{num_layers}</p>
                                <p>"Tokens: "{tokens.with(|t| t.len())}</p>
                            </div>
                            <div class="attention-grid">
                                {data.iter().map(|layer_data| {
                                    view! {
                                        <div class="layer-card">
                                            <h3>"Layer "{layer_data.layer}</h3>
                                            <div class="attention-preview">
                                                <p>"Heads: "{layer_data.heads.len()}</p>
                                                <p>"Positions: "{layer_data.heads.first().map(|h| h.len()).unwrap_or(0)}</p>
                                            </div>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                            <details style="margin-top: 16px;">
                                <summary>"Full Analysis JSON"</summary>
                                <pre style="white-space: pre-wrap; margin-top: 12px;">{move || analysis_dump.get()}</pre>
                            </details>
                        </div>
                    }
                    .into_view()
                } else {
                    view! {}.into_view()
                }
            }}
                </div>
            </section>
            <ArtifactBar status="projection: Batch DLA facade over analysis transport".to_string()/>
        </>
    }
}

async fn invoke_tauri_lql(request: LqlRunRequest) -> Result<LqlRunResponse, String> {
    if !has_tauri_runtime() {
        return invoke_http_lql(request).await;
    }
    let request_js = serde_wasm_bindgen::to_value(&request).map_err(|e| e.to_string())?;
    let result = invoke_tauri_command("run_lql_query_command", request_js).await?;
    serde_wasm_bindgen::from_value(result).map_err(|e| e.to_string())
}

async fn invoke_tauri_describe(request: DescribeRunRequest) -> Result<DescribeRunResponse, String> {
    if !has_tauri_runtime() {
        return invoke_http_describe(request).await;
    }
    let request_js = serde_wasm_bindgen::to_value(&request).map_err(|e| e.to_string())?;
    let result = invoke_tauri_command("run_describe_command", request_js).await?;
    serde_wasm_bindgen::from_value(result).map_err(|e| e.to_string())
}

async fn invoke_tauri_analyze_infer(
    request: AnalyzeInferRequest,
) -> Result<AnalyzeInferResponse, String> {
    if !has_tauri_runtime() {
        return invoke_http_analyze_infer(request).await;
    }
    let request_js = serde_wasm_bindgen::to_value(&request).map_err(|e| e.to_string())?;
    let result = invoke_tauri_command("run_analyze_infer_command", request_js).await?;
    serde_wasm_bindgen::from_value(result).map_err(|e| e.to_string())
}

async fn invoke_tauri_bit_perfect_eraser(
    request: BitPerfectEraserRunRequest,
) -> Result<BitPerfectEraserRunResponse, String> {
    if !has_tauri_runtime() {
        return Err("Bit-Perfect Eraser is only available in the Tauri workbench".to_string());
    }
    let request_js = serde_wasm_bindgen::to_value(&request).map_err(|e| e.to_string())?;
    let result = invoke_tauri_command("run_bit_perfect_eraser_command", request_js).await?;
    serde_wasm_bindgen::from_value(result).map_err(|e| e.to_string())
}

#[cfg(target_arch = "wasm32")]
fn has_tauri_runtime() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(tauri_obj) = Reflect::get(&window, &JsValue::from_str("__TAURI__")) else {
        return false;
    };
    !(tauri_obj.is_undefined() || tauri_obj.is_null())
}

#[cfg(not(target_arch = "wasm32"))]
fn has_tauri_runtime() -> bool {
    false
}

#[cfg(target_arch = "wasm32")]
fn trim_trailing_slashes(raw: &str) -> String {
    raw.trim_end_matches('/').to_string()
}

#[cfg(target_arch = "wasm32")]
async fn invoke_http_lql(request: LqlRunRequest) -> Result<LqlRunResponse, String> {
    let payload = serde_json::json!({
        "query": request.query,
        "workspace_path": request.workspace_path,
    });
    let response = Request::post("/api/lql/query")
        .json(&payload)
        .map_err(|e| format!("failed to create LQL request: {e}"))?
        .send()
        .await
        .map_err(|e| format!("LQL request failed: {e}"))?;

    if !response.ok() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("LQL request failed ({status}): {body}"));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("failed to parse LQL response: {e}"))?;

    if let Some(lines) = payload
        .get("run")
        .and_then(|run| run.get("raw"))
        .and_then(|raw| raw.get("lines"))
        .and_then(|lines| lines.as_array())
    {
        let parsed_lines = lines
            .iter()
            .map(|line| line.as_str().unwrap_or("").to_string())
            .collect::<Vec<_>>();
        return Ok(LqlRunResponse {
            lines: parsed_lines,
        });
    }

    if let Some(err) = payload.get("error").and_then(|e| e.as_str()) {
        return Err(err.to_string());
    }

    Err("unexpected LQL response shape from /api/lql/query".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
async fn invoke_http_lql(_request: LqlRunRequest) -> Result<LqlRunResponse, String> {
    Err("HTTP LQL fallback is only available in wasm32 builds".to_string())
}

#[cfg(target_arch = "wasm32")]
async fn invoke_http_describe(request: DescribeRunRequest) -> Result<DescribeRunResponse, String> {
    let payload = serde_json::json!({
        "entity": request.entity,
        "band": request.band.unwrap_or_else(|| "knowledge".to_string()),
        "verbose": request.verbose,
        "workspace_path": request.workspace_path,
    });
    let response = Request::post("/api/explorer/describe")
        .json(&payload)
        .map_err(|e| format!("failed to create describe request: {e}"))?
        .send()
        .await
        .map_err(|e| format!("describe request failed: {e}"))?;

    if !response.ok() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("describe request failed ({status}): {body}"));
    }

    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("failed to parse describe response: {e}"))?;

    if let Some(edges) = payload
        .get("run")
        .and_then(|run| run.get("raw"))
        .and_then(|raw| raw.get("edges"))
        .and_then(|edges| edges.as_array())
    {
        let lines = edges
            .iter()
            .map(|edge| serde_json::to_string_pretty(edge).unwrap_or_else(|_| edge.to_string()))
            .collect::<Vec<_>>();
        return Ok(DescribeRunResponse { lines });
    }

    if let Some(err) = payload.get("error").and_then(|e| e.as_str()) {
        return Err(err.to_string());
    }

    Err("unexpected describe response shape from /api/explorer/describe".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
async fn invoke_http_describe(_request: DescribeRunRequest) -> Result<DescribeRunResponse, String> {
    Err("HTTP describe fallback is only available in wasm32 builds".to_string())
}

#[cfg(target_arch = "wasm32")]
async fn invoke_http_analyze_infer(
    request: AnalyzeInferRequest,
) -> Result<AnalyzeInferResponse, String> {
    let base = trim_trailing_slashes(&request.server_url);
    let url = format!("{base}/v1/analyze-infer");
    let response = Request::post(&url)
        .json(&request)
        .map_err(|e| format!("failed to create analyze request: {e}"))?
        .send()
        .await
        .map_err(|e| format!("analyze request failed: {e}"))?;

    if !response.ok() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("analyze request failed ({status}): {body}"));
    }

    response
        .json::<AnalyzeInferResponse>()
        .await
        .map_err(|e| format!("failed to parse analyze response: {e}"))
}

#[cfg(not(target_arch = "wasm32"))]
async fn invoke_http_analyze_infer(
    _request: AnalyzeInferRequest,
) -> Result<AnalyzeInferResponse, String> {
    Err("HTTP analyze fallback is only available in wasm32 builds".to_string())
}

async fn invoke_tauri_command(command: &str, payload: JsValue) -> Result<JsValue, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (command, payload);
        Err("Tauri invoke is only available in wasm32 builds".to_string())
    }

    #[cfg(target_arch = "wasm32")]
    {
        let window = web_sys::window().ok_or_else(|| "window not available".to_string())?;
        let tauri_obj =
            Reflect::get(&window, &JsValue::from_str("__TAURI__")).map_err(|e| format!("{e:?}"))?;

        if tauri_obj.is_undefined() || tauri_obj.is_null() {
            return Err("Tauri runtime not detected (window.__TAURI__ missing)".to_string());
        }

        let core_obj =
            Reflect::get(&tauri_obj, &JsValue::from_str("core")).unwrap_or(JsValue::UNDEFINED);
        let holder = if core_obj.is_undefined() || core_obj.is_null() {
            tauri_obj.clone()
        } else {
            core_obj
        };

        let invoke_fn = Reflect::get(&holder, &JsValue::from_str("invoke"))
            .map_err(|e| format!("{e:?}"))?
            .dyn_into::<Function>()
            .map_err(|_| "tauri invoke function missing".to_string())?;

        let args = Object::new();
        Reflect::set(&args, &JsValue::from_str("request"), &payload)
            .map_err(|e| format!("{e:?}"))?;

        let promise_val = invoke_fn
            .call2(&holder, &JsValue::from_str(command), &JsValue::from(args))
            .map_err(|e| format!("{e:?}"))?;
        let promise = promise_val
            .dyn_into::<Promise>()
            .map_err(|_| "tauri invoke did not return Promise".to_string())?;
        JsFuture::from(promise)
            .await
            .map_err(|e| format!("invoke failed: {e:?}"))
    }
}
