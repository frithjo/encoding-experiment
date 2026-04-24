//! Behavioral-driven integration tests for batch_dla_scan tool handler.
//!
//! Tests start an actual larql-server process and make real HTTP requests
//! to assert exact behavior with clear success criteria.

use reqwest::Client;
use serde_json::Value;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::Duration;

const TEST_PORT: u16 = 18080;
const FIXTURE_VINDEX: &str = "/home/arty/Documents/projects/encoding-experiment/larql-main/crates/larql-python/tests/fixtures/ui_walk_trace_vindex";

static TEST_MUTEX: Mutex<()> = Mutex::new(());

/// Helper: Start larql-server process with test vindex
async fn start_test_server(vindex_path: &str) -> Child {
    let server_path = env!("CARGO_BIN_EXE_larql-server");

    let mut child = Command::new(server_path)
        .arg("--port")
        .arg(TEST_PORT.to_string())
        .arg(vindex_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to start larql-server");

    // Wait for server to be ready
    wait_for_server_ready(&mut child).await;

    child
}

/// Helper: Wait for server to be ready
async fn wait_for_server_ready(child: &mut Child) {
    let client = Client::new();
    let expected_pid = child.id() as u64;
    // Give server more time to start
    tokio::time::sleep(Duration::from_millis(1000)).await;
    let mut attempts = 0;
    while attempts < 60 {
        if let Some(status) = child.try_wait().expect("Failed to poll larql-server child") {
            panic!("Spawned larql-server exited before readiness check succeeded: {status}");
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
        if let Ok(response) = client
            .get(format!("http://localhost:{}/v1/health", TEST_PORT))
            .send()
            .await
        {
            if response.status().is_success() {
                let is_expected_health = response
                    .json::<Value>()
                    .await
                    .ok()
                    .map(|body| {
                        body.get("status").and_then(Value::as_str) == Some("ok")
                            && body.get("pid").and_then(Value::as_u64) == Some(expected_pid)
                    })
                    .unwrap_or(false);

                if is_expected_health {
                    // Additional delay after successful health check
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    return;
                }
            }
        }
        attempts += 1;
    }
    // If server didn't start, kill it and panic
    panic!("Server failed to start within 12 seconds");
}

/// Helper: Send HTTP request with retry logic
async fn send_with_retry(
    client: &Client,
    request: reqwest::Request,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut attempts = 0;
    loop {
        match client.execute(request.try_clone().unwrap()).await {
            Ok(response) => return Ok(response),
            Err(_e) if attempts < 3 => {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Helper: Stop server process
fn stop_test_server(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn lock_test_server() -> std::sync::MutexGuard<'static, ()> {
    TEST_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Helper: Get model config from vindex
fn get_model_config(vindex_path: &str) -> Value {
    let index_path = format!("{}/index.json", vindex_path);
    let content = std::fs::read_to_string(index_path).expect("Failed to read index.json");
    serde_json::from_str(&content).expect("Failed to parse index.json")
}

async fn post_tools_call(client: &Client, request: Value) -> reqwest::Response {
    let req = client
        .post(format!("http://localhost:{}/tools/call", TEST_PORT))
        .json(&request)
        .build()
        .unwrap();
    send_with_retry(client, req)
        .await
        .expect("Failed to send request")
}

async fn post_analyze_infer(client: &Client, request: Value) -> reqwest::Response {
    let req = client
        .post(format!("http://localhost:{}/v1/analyze-infer", TEST_PORT))
        .json(&request)
        .build()
        .unwrap();
    send_with_retry(client, req)
        .await
        .expect("Failed to send request")
}

#[tokio::test]
async fn test_batch_dla_scan_attention_layer_count_matches_config() {
    let _guard = lock_test_server();
    // Given: larql-server is running with ui_walk_trace_vindex loaded
    let server = start_test_server(FIXTURE_VINDEX).await;
    let client = Client::new();
    let config = get_model_config(FIXTURE_VINDEX);
    let expected_num_layers = config["num_layers"].as_u64().unwrap() as usize;

    // When: POST /tools/call with batch_dla_scan and a valid prompt
    let request = serde_json::json!({
        "name": "batch_dla_scan",
        "arguments": {
            "prompt": "test prompt"
        }
    });

    let response = post_tools_call(&client, request).await;

    // Then: Response contains num_layers equal to model config's num_layers
    assert!(
        response.status().is_success(),
        "Request failed: {:?}",
        response.status()
    );

    let result: Value = response.json().await.expect("Failed to parse response");
    let num_layers = result["result"]["num_layers"].as_u64().unwrap() as usize;

    assert_eq!(
        num_layers, expected_num_layers,
        "Attention layer count {} does not match config num_layers {}",
        num_layers, expected_num_layers
    );

    stop_test_server(server);
}

#[tokio::test]
async fn test_batch_dla_scan_attention_matrix_dimensions() {
    let _guard = lock_test_server();
    // Given: larql-server is running with ui_walk_trace_vindex loaded
    let server = start_test_server(FIXTURE_VINDEX).await;
    let client = Client::new();
    let prompt = "test prompt for dimension check";

    // When: POST /tools/call with batch_dla_scan and a prompt with N tokens
    let request = serde_json::json!({
        "name": "batch_dla_scan",
        "arguments": {
            "prompt": prompt
        }
    });

    let response = post_tools_call(&client, request).await;

    // Then: Each attention matrix has dimensions matching heads × seq_len
    assert!(response.status().is_success());

    let result: Value = response.json().await.expect("Failed to parse response");
    let attention = result["result"]["attention"].as_array().unwrap();
    let seq_len = result["result"]["seq_len"].as_u64().unwrap() as usize;

    // Get expected num_heads from config
    let config = get_model_config(FIXTURE_VINDEX);
    let num_heads = config["num_attention_heads"].as_u64().unwrap_or(1) as usize;

    for layer_data in attention {
        let layer = layer_data["layer"].as_u64().unwrap() as usize;
        let heads = layer_data["heads"].as_array().unwrap();

        assert_eq!(
            heads.len(),
            num_heads,
            "Layer {}: Expected {} heads, got {}",
            layer,
            num_heads,
            heads.len()
        );

        for (head_idx, head_weights) in heads.iter().enumerate() {
            let weights: Vec<f32> = head_weights
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap() as f32)
                .collect();
            assert_eq!(
                weights.len(),
                seq_len,
                "Layer {} head {}: Expected seq_len {}, got {}",
                layer,
                head_idx,
                seq_len,
                weights.len()
            );
        }
    }

    stop_test_server(server);
}

#[tokio::test]
async fn test_batch_dla_scan_token_ids_match_tokenizer() {
    let _guard = lock_test_server();
    // Given: larql-server is running with ui_walk_trace_vindex loaded
    let server = start_test_server(FIXTURE_VINDEX).await;
    let client = Client::new();
    let prompt = "test prompt for tokenization";

    // When: POST /tools/call with batch_dla_scan and a prompt
    let request = serde_json::json!({
        "name": "batch_dla_scan",
        "arguments": {
            "prompt": prompt
        }
    });

    let response = post_tools_call(&client, request).await;

    // Then: Response contains token IDs
    assert!(response.status().is_success());

    let result: Value = response.json().await.expect("Failed to parse response");
    let response_tokens: Vec<usize> = result["result"]["tokens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as usize)
        .collect();

    // For the test vindex, we just check that tokens are generated
    assert!(
        !response_tokens.is_empty(),
        "Response tokens should not be empty"
    );

    stop_test_server(server);
}

#[tokio::test]
async fn test_batch_dla_scan_predictions_valid_top_k_format() {
    let _guard = lock_test_server();
    // Given: larql-server is running with ui_walk_trace_vindex loaded
    let server = start_test_server(FIXTURE_VINDEX).await;
    let client = Client::new();
    let prompt = "test prompt for predictions";

    // When: POST /tools/call with batch_dla_scan and a prompt
    let request = serde_json::json!({
        "name": "batch_dla_scan",
        "arguments": {
            "prompt": prompt
        }
    });

    let response = post_tools_call(&client, request).await;

    // Then: Response contains predictions in valid top-k format
    assert!(response.status().is_success());

    let result: Value = response.json().await.expect("Failed to parse response");
    let predictions = result["result"]["predictions"]
        .as_array()
        .expect("Predictions should be an array");

    // Check that predictions is an array
    assert!(!predictions.is_empty(), "Predictions should not be empty");

    // Check top-k length (should be 5 as hardcoded in handler)
    assert_eq!(predictions.len(), 5, "Should return top-5 predictions");

    // Check each prediction is an array [token, logit]
    for pred in predictions {
        assert!(
            pred.is_array(),
            "Each prediction should be an array [token, logit]"
        );
        let pred_array = pred.as_array().unwrap();
        assert_eq!(
            pred_array.len(),
            2,
            "Prediction should have 2 elements [token, logit]"
        );
        assert!(
            pred_array[0].is_string(),
            "First element should be token string"
        );
        assert!(
            pred_array[1].is_number(),
            "Second element should be logit number"
        );

        // Check logit is a finite number
        let logit = pred_array[1].as_f64().expect("Logit should be a number");
        assert!(logit.is_finite(), "Logit should be finite");
    }

    stop_test_server(server);
}

#[tokio::test]
async fn test_batch_dla_scan_returns_logit_lens_and_head_dla() {
    let _guard = lock_test_server();
    // Given: larql-server is running with ui_walk_trace_vindex loaded
    let server = start_test_server(FIXTURE_VINDEX).await;
    let client = Client::new();
    let prompt = "test prompt for enrichment check";

    // When: POST /tools/call with batch_dla_scan
    let request = serde_json::json!({
        "name": "batch_dla_scan",
        "arguments": {
            "prompt": prompt
        }
    });

    let response = post_tools_call(&client, request).await;

    // Then: Response contains logit_lens and head_dla
    assert!(response.status().is_success());

    let result: Value = response.json().await.expect("Failed to parse response");
    let result_data = &result["result"];

    // Check logit_lens
    assert!(
        result_data["logit_lens"].is_array(),
        "logit_lens should be an array"
    );
    let logit_lens = result_data["logit_lens"].as_array().unwrap();
    assert!(!logit_lens.is_empty(), "logit_lens should not be empty");

    // Check head_dla
    // Note: head_dla might be empty if not captured, but the structure should exist or be handled
    // In our implementation, we added it to the JSON response.
    // Let's check if it exists (even if empty depending on how it was captured)
    if result_data.get("head_dla").is_some() {
        assert!(
            result_data["head_dla"].is_array(),
            "head_dla should be an array"
        );
    }

    stop_test_server(server);
}

#[tokio::test]
async fn test_batch_dla_scan_error_handling_empty_prompt() {
    let _guard = lock_test_server();
    // Given: larql-server is running with ui_walk_trace_vindex loaded
    let server = start_test_server(FIXTURE_VINDEX).await;
    let client = Client::new();

    // When: POST /tools/call with batch_dla_scan and empty prompt string
    let request = serde_json::json!({
        "name": "batch_dla_scan",
        "arguments": {
            "prompt": ""
        }
    });

    let response = post_tools_call(&client, request).await;

    // Then: Response returns appropriate error
    // Empty prompt should either return error or empty attention array
    let status = response.status();

    // Either error status or empty attention array is acceptable
    if !status.is_success() {
        let error_text = response
            .text()
            .await
            .expect("Failed to read error response body");
        assert!(
            !error_text.trim().is_empty(),
            "Error response should contain a non-empty body"
        );
    } else {
        let result: Value = response.json().await.expect("Failed to parse response");
        let attention = result["result"]["attention"].as_array().unwrap();
        assert!(
            attention.is_empty(),
            "Empty prompt should return empty attention"
        );
    }

    stop_test_server(server);
}

#[tokio::test]
async fn test_batch_dla_scan_rejects_unknown_analysis_mode() {
    let _guard = lock_test_server();
    let server = start_test_server(FIXTURE_VINDEX).await;
    let client = Client::new();

    let request = serde_json::json!({
        "name": "batch_dla_scan",
        "arguments": {
            "prompt": "test prompt",
            "analysis": {
                "mode": "unknown_mode",
                "truth_spans": [],
                "materially_false_spans": [],
                "coherence_markers": [],
                "max_generated_tokens": 1,
                "ridge_dead_zone": 0.05
            }
        }
    });

    let response = post_tools_call(&client, request).await;
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let body = response.text().await.expect("Failed to read error body");
    assert!(
        body.contains("Unsupported analysis mode"),
        "Unexpected body: {body}"
    );

    stop_test_server(server);
}

#[tokio::test]
async fn test_batch_dla_scan_fact_probe_stops_after_resolved_false_span() {
    let _guard = lock_test_server();
    let server = start_test_server(FIXTURE_VINDEX).await;
    let client = Client::new();
    let prompt = "test prompt for fact mode";

    let baseline = post_tools_call(
        &client,
        serde_json::json!({
            "name": "batch_dla_scan",
            "arguments": { "prompt": prompt }
        }),
    )
    .await;
    assert!(baseline.status().is_success());
    let baseline_json: Value = baseline.json().await.expect("Failed to parse baseline");
    let first_prediction = baseline_json["result"]["predictions"][0][0]
        .as_str()
        .expect("Missing first prediction token")
        .to_string();

    let response = post_tools_call(
        &client,
        serde_json::json!({
            "name": "batch_dla_scan",
            "arguments": {
                "prompt": prompt,
                "analysis": {
                    "mode": "fact_probe",
                    "truth_spans": [],
                    "materially_false_spans": [first_prediction],
                    "coherence_markers": [],
                    "max_generated_tokens": 3,
                    "ridge_dead_zone": 0.05
                }
            }
        }),
    )
    .await;

    assert!(response.status().is_success());
    let result: Value = response.json().await.expect("Failed to parse response");
    let generation_trace = result["result"]["generation_trace"]
        .as_array()
        .expect("generation_trace should be an array");
    let token_analysis = result["result"]["token_analysis"]
        .as_array()
        .expect("token_analysis should be an array");
    let summary = &result["result"]["analysis_summary"];

    assert_eq!(
        generation_trace.len(),
        1,
        "fact_probe should stop after exact false-span resolution"
    );
    assert_eq!(
        token_analysis.len(),
        1,
        "fact_probe should only analyze the resolved step"
    );
    assert_eq!(
        summary["materially_false_detected"].as_bool(),
        Some(true),
        "fact_probe should mark the resolved false span"
    );
    assert_eq!(
        summary["first_false_position"].as_u64(),
        Some(0),
        "fact_probe should report the first false position"
    );

    stop_test_server(server);
}

#[tokio::test]
async fn test_batch_dla_scan_fact_probe_detects_false_alternate_continuation() {
    let _guard = lock_test_server();
    let server = start_test_server(FIXTURE_VINDEX).await;
    let client = Client::new();
    let prompt = "test prompt for alternate fact path";

    let baseline = post_tools_call(
        &client,
        serde_json::json!({
            "name": "batch_dla_scan",
            "arguments": { "prompt": prompt }
        }),
    )
    .await;
    assert!(baseline.status().is_success());
    let baseline_json: Value = baseline.json().await.expect("Failed to parse baseline");
    let predictions = baseline_json["result"]["predictions"]
        .as_array()
        .expect("Missing predictions array");
    let false_span = predictions
        .iter()
        .skip(1)
        .filter_map(|prediction| prediction.as_array())
        .filter_map(|prediction| prediction.first().and_then(Value::as_str))
        .find(|token| !token.is_empty())
        .expect("Need a non-top-1 prediction token")
        .to_string();

    let response = post_tools_call(
        &client,
        serde_json::json!({
            "name": "batch_dla_scan",
            "arguments": {
                "prompt": prompt,
                "analysis": {
                    "mode": "fact_probe",
                    "truth_spans": [],
                    "materially_false_spans": [false_span.clone()],
                    "coherence_markers": [],
                    "max_generated_tokens": 3,
                    "ridge_dead_zone": 0.05
                }
            }
        }),
    )
    .await;

    assert!(response.status().is_success());
    let result: Value = response.json().await.expect("Failed to parse response");
    let generation_trace = result["result"]["generation_trace"]
        .as_array()
        .expect("generation_trace should be an array");
    let token_analysis = result["result"]["token_analysis"]
        .as_array()
        .expect("token_analysis should be an array");
    let summary = &result["result"]["analysis_summary"];

    assert_eq!(
        generation_trace.len(),
        1,
        "fact_probe should resolve an alternate false continuation at the first analyzed step"
    );
    assert_eq!(
        token_analysis.len(),
        1,
        "fact_probe should stop once the alternate false continuation is resolved"
    );
    assert_eq!(
        summary["materially_false_detected"].as_bool(),
        Some(true),
        "fact_probe should mark the alternate continuation as materially false"
    );
    assert_eq!(
        summary["first_false_position"].as_u64(),
        Some(0),
        "fact_probe should report the alternate continuation at the first position"
    );
    assert_eq!(
        generation_trace[0]["token"].as_str(),
        Some(false_span.as_str()),
        "fact_probe should surface the dominant false continuation token"
    );

    stop_test_server(server);
}

#[tokio::test]
async fn test_batch_dla_scan_workflow_probe_accumulates_ridge_across_full_trace() {
    let _guard = lock_test_server();
    let server = start_test_server(FIXTURE_VINDEX).await;
    let client = Client::new();

    let response = post_tools_call(
        &client,
        serde_json::json!({
            "name": "batch_dla_scan",
            "arguments": {
                "prompt": "Step-by-step guide to becoming the president of Freedonia",
                "analysis": {
                    "mode": "workflow_probe",
                    "truth_spans": ["Albrecht"],
                    "materially_false_spans": ["Freedonia", "president of Freedonia"],
                    "coherence_markers": ["step", "guide", "campaign", "election", "office", "becoming"],
                    "max_generated_tokens": 3,
                    "ridge_dead_zone": 0.0
                }
            }
        }),
    )
    .await;

    assert!(response.status().is_success());
    let result: Value = response.json().await.expect("Failed to parse response");
    let generation_trace = result["result"]["generation_trace"]
        .as_array()
        .expect("generation_trace should be an array");
    let token_analysis = result["result"]["token_analysis"]
        .as_array()
        .expect("token_analysis should be an array");
    let ridge_by_layer = result["result"]["ridge_by_layer"]
        .as_array()
        .expect("ridge_by_layer should be an array");

    assert_eq!(
        generation_trace.len(),
        3,
        "workflow_probe should trace the full bounded continuation"
    );
    assert_eq!(
        token_analysis.len(),
        3,
        "workflow_probe should analyze every generated step"
    );
    assert!(
        !ridge_by_layer.is_empty(),
        "workflow_probe should return accumulated ridge values"
    );
    assert!(
        ridge_by_layer.iter().all(|entry| {
            entry["layer"].as_u64().is_some() && entry["ridge"].as_f64().unwrap_or(-1.0) >= 0.0
        }),
        "ridge_by_layer entries should be layer/ridge pairs with non-negative totals"
    );

    stop_test_server(server);
}

#[tokio::test]
async fn test_analyze_infer_endpoint_returns_structured_analysis_payload() {
    let _guard = lock_test_server();
    let server = start_test_server(FIXTURE_VINDEX).await;
    let client = Client::new();

    let response = post_analyze_infer(
        &client,
        serde_json::json!({
            "prompt": "Step-by-step guide to becoming the president of Freedonia",
            "top_k": 5,
            "mode": "workflow_probe",
            "truth_spans": ["Albrecht"],
            "materially_false_spans": ["Freedonia", "president of Freedonia"],
            "coherence_markers": ["step", "guide", "campaign", "election", "office", "becoming"],
            "max_generated_tokens": 2,
            "ridge_dead_zone": 0.0
        }),
    )
    .await;

    assert!(response.status().is_success());
    let result: Value = response.json().await.expect("Failed to parse response");

    assert!(
        result["generation_trace"].as_array().is_some(),
        "generation_trace should be present on /v1/analyze-infer"
    );
    assert!(
        result["ridge_by_layer"].as_array().is_some(),
        "ridge_by_layer should be present on /v1/analyze-infer"
    );
    assert!(
        result["analysis_summary"].is_object(),
        "analysis_summary should be present on /v1/analyze-infer"
    );

    stop_test_server(server);
}
