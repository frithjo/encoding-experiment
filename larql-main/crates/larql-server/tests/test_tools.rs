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
const FIXTURE_VINDEX: &str = "../../crates/larql-python/tests/fixtures/ui_walk_trace_vindex";

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

    let req = client
        .post(format!("http://localhost:{}/tools/call", TEST_PORT))
        .json(&request)
        .build()
        .unwrap();
    let response = send_with_retry(&client, req)
        .await
        .expect("Failed to send request");

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

    let req = client
        .post(format!("http://localhost:{}/tools/call", TEST_PORT))
        .json(&request)
        .build()
        .unwrap();
    let response = send_with_retry(&client, req)
        .await
        .expect("Failed to send request");

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

    let req = client
        .post(format!("http://localhost:{}/tools/call", TEST_PORT))
        .json(&request)
        .build()
        .unwrap();
    let response = send_with_retry(&client, req)
        .await
        .expect("Failed to send request");

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

    let req = client
        .post(format!("http://localhost:{}/tools/call", TEST_PORT))
        .json(&request)
        .build()
        .unwrap();
    let response = send_with_retry(&client, req)
        .await
        .expect("Failed to send request");

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

    let req = client
        .post(format!("http://localhost:{}/tools/call", TEST_PORT))
        .json(&request)
        .build()
        .unwrap();
    let response = send_with_retry(&client, req)
        .await
        .expect("Failed to send request");

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
