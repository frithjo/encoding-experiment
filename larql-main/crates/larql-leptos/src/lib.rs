use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct AttentionData {
    layer: usize,
    heads: Vec<Vec<f32>>,
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Title text="LARQL Leptos - Batch DLA Scan"/>
        <Router>
            <main class="container">
                <Routes>
                    <Route path="/" view=BatchDlaScan/>
                </Routes>
            </main>
        </Router>
    }
}

pub fn main() {
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();
    leptos::mount_to_body(App);
}

#[component]
fn BatchDlaScan() -> impl IntoView {
    let (prompt, set_prompt) = create_signal("".to_string());
    let (loading, set_loading) = create_signal(false);
    let (error, set_error) = create_signal(None::<String>);
    let (attention_data, set_attention_data) = create_signal(None::<Vec<AttentionData>>);
    let (num_layers, set_num_layers) = create_signal(0usize);
    let (tokens, set_tokens) = create_signal(Vec::<usize>::new());

    let run_scan = move |_| {
        let prompt_val = prompt.get();
        if prompt_val.is_empty() {
            set_error.set(Some("Please enter a prompt".to_string()));
            return;
        }

        set_loading.set(true);
        set_error.set(None);

        let prompt_clone = prompt_val.clone();

        // Call larql-server /v1/analyze-infer endpoint (canonical path)
        wasm_bindgen_futures::spawn_local(async move {
            let request = serde_json::json!({
                "prompt": prompt_clone,
                "top_k": 5,
                "mode": "fact_probe",
                "truth_spans": [],
                "materially_false_spans": [],
                "coherence_markers": [],
                "max_generated_tokens": None,
                "ridge_dead_zone": None
            });

            match gloo_net::http::Request::post("http://localhost:8080/v1/analyze-infer")
                .json(&request)
            {
                Ok(req) => {
                    match req.send().await {
                        Ok(response) => {
                            if response.ok() {
                                match response.json::<serde_json::Value>().await {
                                    Ok(result) => {
                                        // Parse the attention data from the response
                                        if let Some(attention) = result.get("attention").and_then(|v| v.as_array()) {
                                            let parsed_attention: Vec<AttentionData> = attention
                                                .iter()
                                                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                                                .collect();

                                            set_attention_data.set(Some(parsed_attention));

                                            if let Some(layers) = result.get("num_layers").and_then(|v| v.as_u64()) {
                                                set_num_layers.set(layers as usize);
                                            }

                                            if let Some(tokens_array) = result.get("tokens").and_then(|v| v.as_array()) {
                                                let parsed_tokens: Vec<usize> = tokens_array
                                                    .iter()
                                                    .filter_map(|v| v.as_u64().map(|u| u as usize))
                                                    .collect();
                                                set_tokens.set(parsed_tokens);
                                            }
                                        }
                                        set_loading.set(false);
                                    }
                                    Err(e) => {
                                        set_loading.set(false);
                                        set_error.set(Some(format!("Failed to parse response: {}", e)));
                                    }
                                }
                            } else {
                                set_loading.set(false);
                                set_error.set(Some(format!("Server error: {}", response.status_text())));
                            }
                        }
                        Err(e) => {
                            set_loading.set(false);
                            set_error.set(Some(format!("Request failed: {}", e)));
                        }
                    }
                }
                Err(e) => {
                    set_loading.set(false);
                    set_error.set(Some(format!("Failed to create request: {}", e)));
                }
            }
        });
    };

    view! {
        <div class="batch-dla-scan">
            <h1>"Batch DLA Scan"</h1>
            
            <div class="input-section">
                <label>"Prompt:"</label>
                <textarea
                    prop:value=prompt
                    on:input=move |e| set_prompt.set(event_target_value(&e))
                    placeholder="Enter text to analyze..."
                    rows="4"
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
                    }.into_view()
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
                        </div>
                    }.into_view()
                } else {
                    view! {}.into_view()
                }
            }}
        </div>
    }
}
