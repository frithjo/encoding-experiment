//! Integration tests for /v1/analyze-infer endpoint.
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

#[tokio::test]
async fn test_cross_path_equivalence_local_vs_remote() {
    let _guard = lock_test_server();
    let server = start_test_server(FIXTURE_VINDEX).await;
    let client = Client::new();

    let request = larql_inference::AnalysisRequest {
        prompt: "The capital of France is Paris".to_string(),
        top_k: 5,
        mode: "fact_probe".to_string(),
        truth_spans: vec!["Paris".to_string(), "France".to_string()],
        materially_false_spans: vec!["London".to_string()],
        coherence_markers: vec![
            "The".to_string(),
            "capital".to_string(),
            "of".to_string(),
            "is".to_string(),
        ],
        max_generated_tokens: Some(1),
        ridge_dead_zone: Some(0.05),
    };

    // Get result from remote path via HTTP
    let remote_response = post_analyze_infer(
        &client,
        serde_json::to_value(&request).expect("Failed to serialize request"),
    )
    .await;

    assert!(remote_response.status().is_success());
    let remote_result: larql_inference::AnalysisResult = remote_response
        .json()
        .await
        .expect("Failed to parse remote response");

    // Load model weights and tokenizer for local path
    let vindex_path = std::path::Path::new(FIXTURE_VINDEX);
    let weights =
        larql_vindex::load_model_weights(vindex_path, &mut larql_vindex::SilentLoadCallbacks)
            .expect("Failed to load model weights");
    let tokenizer =
        larql_vindex::load_vindex_tokenizer(vindex_path).expect("Failed to load tokenizer");

    // Get result from local path via direct function call
    let num_layers = weights.num_layers;
    let local_result =
        larql_inference::analyze_infer(&weights, tokenizer.as_ref(), num_layers, &request)
            .expect("Local analysis failed");

    // Compare critical fields for equivalence
    assert_eq!(
        local_result.num_layers, remote_result.num_layers,
        "num_layers should match"
    );
    assert_eq!(
        local_result.seq_len, remote_result.seq_len,
        "seq_len should match"
    );
    assert_eq!(
        local_result.tokens, remote_result.tokens,
        "tokens should match"
    );
    assert_eq!(
        local_result.strings, remote_result.strings,
        "strings should match"
    );

    // Compare attention structure (layer count and dimensions)
    assert_eq!(
        local_result.attention.len(),
        remote_result.attention.len(),
        "attention layer count should match"
    );
    for (local_layer, remote_layer) in local_result
        .attention
        .iter()
        .zip(remote_result.attention.iter())
    {
        assert_eq!(
            local_layer.layer, remote_layer.layer,
            "attention layer index should match"
        );
        assert_eq!(
            local_layer.heads.len(),
            remote_layer.heads.len(),
            "head count should match per layer"
        );
    }

    // Compare head_dla structure
    assert_eq!(
        local_result.head_dla.len(),
        remote_result.head_dla.len(),
        "head_dla layer count should match"
    );

    // Compare generation_trace (if non-empty)
    if !local_result.generation_trace.is_empty() && !remote_result.generation_trace.is_empty() {
        assert_eq!(
            local_result.generation_trace.len(),
            remote_result.generation_trace.len(),
            "generation_trace length should match"
        );
        for (local_step, remote_step) in local_result
            .generation_trace
            .iter()
            .zip(remote_result.generation_trace.iter())
        {
            assert_eq!(
                local_step.position, remote_step.position,
                "generation step position should match"
            );
            assert_eq!(
                local_step.token_id, remote_step.token_id,
                "generation step token_id should match"
            );
            assert_eq!(
                local_step.token, remote_step.token,
                "generation step token should match"
            );
            // Probabilities may have minor floating point differences
            assert!(
                (local_step.probability - remote_step.probability).abs() < 1e-6,
                "generation step probability should match within tolerance"
            );
        }
    }

    // Compare ridge_by_layer totals (critical for scientific contract)
    assert_eq!(
        local_result.ridge_by_layer.len(),
        remote_result.ridge_by_layer.len(),
        "ridge_by_layer count should match"
    );
    let local_total_ridge: f64 = local_result.ridge_by_layer.iter().map(|r| r.ridge).sum();
    let remote_total_ridge: f64 = remote_result.ridge_by_layer.iter().map(|r| r.ridge).sum();
    assert!(
        (local_total_ridge - remote_total_ridge).abs() < 1e-6,
        "total ridge should match between local and remote paths"
    );

    stop_test_server(server);
}
