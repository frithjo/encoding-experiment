# Leptos Migration Guide: Model-Specific Tools

**Status:** Active Migration

This document describes the migration patterns for moving model-specific tools from React (chuk-kv-anatomist) to Leptos with larql-server backend. See [docs/ui/README.md](docs/ui/README.md) for overall UI strategy and interface status.

## Architecture Overview

**Before:**
- React frontend (chuk-kv-anatomist)
- Separate backend for model-specific tools (MCP server)
- larql-server for universal tools only

**After:**
- Leptos frontend (Rust/WASM)
- Single larql-server backend handling all tools
- larql-inference for model-specific computations
- Optional Tauri shell for desktop-native command invocation
- Rust command core (`crates/larql-workbench-core`) owns validation/execution for shell-invoked flows

## Rust-Only Command Boundary (Tauri + WASM)

When running under Tauri:
- WASM UI submits typed command requests only.
- Tauri command handlers call Rust command-core functions.
- Command-core owns workspace path validation, parsing, execution, and error semantics.
- UI renders returned lines/state and does not duplicate business logic.

First migrated flow:
- `run_lql_query_command` (Tauri) -> `larql_workbench_core::run_lql_query` -> `larql_lql::Session`.
- `run_describe_command` (Tauri) -> `larql_workbench_core::run_describe` -> `larql_lql::Session`.
- `run_analyze_infer_command` (Tauri) -> `larql_workbench_core::run_analyze_infer` -> strict LQL `ANALYZE INFER ... FORMAT JSON`.

## Migration Pattern: batch_dla_scan (Historical Proof of Concept)

> Historical note: the section below captures an early transport-first migration
> pattern. The current canonical scientific path is LQL-first (`ANALYZE INFER`
> via `larql-lql`), with `/v1/analyze-infer` retained as a transport adapter.

### Step 1: Implement Tool Handler in larql-server

**File:** `larql-server/src/routes/tools.rs`

```rust
use larql_inference::forward::{predict_with_ffn_attention, PredictResultWithAttention};
use larql_inference::ffn::WeightFfn;

// Add to handle_tools_call match
"batch_dla_scan" => handle_batch_dla_scan(&state, req.arguments).await?,

// Implement handler
async fn handle_batch_dla_scan(
    state: &AppState,
    args: serde_json::Value,
) -> Result<serde_json::Value, (StatusCode, String)> {
    let model = state.models.first()?;
    let args_map: HashMap<String, serde_json::Value> = serde_json::from_value(args)?;
    let prompt = args_map.get("prompt").and_then(|v| v.as_str())?;
    
    let tokens = model.tokenizer.encode(prompt, false)?;
    let weights = model.get_or_load_weights()?;
    let ffn = WeightFfn { weights };
    
    let result = predict_with_ffn_attention(weights, &*model.tokenizer, &tokens.ids, 5, &ffn);
    
    // Convert to JSON-serializable format
    let attention_data: Vec<serde_json::Value> = result.attention
        .into_iter()
        .map(|layer_capture| serde_json::json!({
            "layer": layer_capture.layer,
            "heads": layer_capture.weights.heads
        }))
        .collect();
    
    Ok(serde_json::json!({
        "attention": attention_data,
        "num_layers": model.config.num_layers,
        "seq_len": tokens.ids.len(),
        "tokens": tokens.ids.iter().map(|&id| id as usize).collect::<Vec<_>>(),
        "predictions": result.predictions
    }))
}
```

**Key Points:**
- Use `WeightFfn` for dense FFN computation (architecture-correct)
- `predict_with_ffn_attention` captures attention across all layers
- Convert `AttentionWeights` to JSON manually (struct doesn't implement Serialize)
- Return layer index, attention heads, sequence length, tokens, and predictions

### Step 2: Create Leptos Frontend

**Cargo.toml:**
```toml
[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
leptos = { version = "0.6", features = ["csr"] }
leptos_meta = { version = "0.6" }
leptos_router = { version = "0.6" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
web-sys = "0.3"
gloo-net = { version = "0.5", features = ["http", "json"] }
console_log = "1.0"
console_error_panic_hook = "0.1"
```

**lib.rs:**
```rust
use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct ToolCallRequest {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Clone, Serialize, Deserialize)]
struct ToolCallResponse {
    result: serde_json::Value,
}

#[derive(Clone, Serialize, Deserialize)]
struct AttentionData {
    layer: usize,
    heads: Vec<Vec<f32>>,
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();
    view! {
        <Title text="LARQL Leptos"/>
        <Router>
            <main class="container">
                <Routes>
                    <Route path="/" view=BatchDlaScan/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn BatchDlaScan() -> impl IntoView {
    let (prompt, set_prompt) = create_signal("".to_string());
    let (loading, set_loading) = create_signal(false);
    let (error, set_error) = create_signal(None::<String>);
    let (attention_data, set_attention_data) = create_signal(None::<Vec<AttentionData>>);

    let run_scan = move |_| {
        let prompt_val = prompt.get();
        if prompt_val.is_empty() {
            set_error.set(Some("Please enter a prompt".to_string()));
            return;
        }

        set_loading.set(true);
        set_error.set(None);

        let prompt_clone = prompt_val.clone();
        
        wasm_bindgen_futures::spawn_local(async move {
            let request = ToolCallRequest {
                name: "batch_dla_scan".to_string(),
                arguments: serde_json::json!({ "prompt": prompt_clone }),
            };

            match gloo_net::http::Request::post("http://localhost:8080/tools/call")
                .json(&request)
            {
                Ok(req) => {
                    match req.send().await {
                        Ok(response) => {
                            if response.ok() {
                                match response.json::<ToolCallResponse>().await {
                                    Ok(tool_response) => {
                                        // Parse and display results
                                        set_loading.set(false);
                                    }
                                    Err(e) => {
                                        set_loading.set(false);
                                        set_error.set(Some(format!("Parse error: {}", e)));
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
            <textarea prop:value=prompt on:input=move |e| set_prompt.set(event_target_value(&e))/>
            <button on:click=run_scan disabled=loading>"Scan"</button>
            {move || error.get().map(|e| view! { <div class="error">{e}</div> })}
            {move || attention_data.get().map(|data| {
                // Display attention matrices
            })}
        </div>
    }
}
```

**Key Points:**
- Use `wasm-bindgen-futures` for async operations in WASM
- Use `gloo-net` for HTTP requests
- Signals for reactive state management
- `create_signal` for state, `.set()` for updates
- `spawn_local` for async closures

### Step 3: Remaining Tools Migration

#### extract_attention_output
- Similar to batch_dla_scan but for specific layer/head
- Use same `predict_with_ffn_attention` function
- Filter results for specific layer/head index
- Return single attention matrix

#### kv_inject_test
- Requires KV cache manipulation functions from larql-inference
- Use `prefill_with_kv` or similar functions
- Test KV injection and return results
- More complex - may need additional larql-inference functions

#### context_map_with_query
- Replace placeholder with actual attention-based computation
- Use query-specific attention patterns
- May need custom attention computation
- Could use `trace_forward` with attention capture

## Testing Strategy

1. **Unit test tool handler**: Test larql-server handler directly with mock state
2. **Integration test**: Start larql-server, call endpoint, verify response
3. **WASM build test**: Build Leptos app, verify no compilation errors
4. **End-to-end test**: Run larql-server, serve Leptos app, test full flow

## Challenges and Solutions

**Challenge:** AttentionWeights doesn't implement Serialize
**Solution:** Manually convert to JSON-serializable format (access `.heads` field)

**Challenge:** WASM async requires special handling
**Solution:** Use `wasm-bindgen-futures::spawn_local` for async closures

**Challenge:** Leptos stable vs nightly features
**Solution:** Use stable-compatible Leptos 0.6 without nightly features

**Challenge:** Signal setter syntax
**Solution:** Use `.set()` method on WriteSignal, not direct function call

## Next Steps

1. Complete batch_dla_scan end-to-end testing
2. Migrate extract_attention_output using same pattern
3. Investigate larql-inference KV cache functions for kv_inject_test
4. Implement context_map_with_query with attention-based computation
5. Build comprehensive visualization for attention matrices
6. Add error handling and loading states
7. Consider adding a landing page for multiple tools
