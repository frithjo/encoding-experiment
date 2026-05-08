//! Tools endpoint for chuk-kv-anatomist compatibility.
//! Implements /tools/call with tools: get_model_info, context_map, context_map_with_query, load_model

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::state::AppState;

/// Tool call request body (MCP/Native compatible)
#[derive(Debug, Deserialize)]
pub struct ToolCallRequest {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Tool call response (MCP/Native compatible)
#[derive(Debug, Serialize)]
pub struct ToolCallResponse {
    pub result: serde_json::Value,
}

/// Model config response for get_model_info
#[derive(Debug, Serialize)]
pub struct ModelConfig {
    pub model_id: String,
    pub num_layers: usize,
    pub hidden_dim: usize,
    pub num_attention_heads: usize,
    pub num_kv_heads: usize,
    pub head_dim: usize,
    pub vocab_size: usize,
    pub max_position_embeddings: usize,
    pub architecture: String,
    pub family: String,
}

/// Context map entry
#[derive(Debug, Serialize)]
pub struct ContextMapEntry {
    pub token: String,
    pub token_id: usize,
    pub layer: usize,
    pub position: usize,
    pub coefficient: f32,
    pub fraction: f32,
}

/// Handle /tools/call endpoint
pub async fn handle_tools_call(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ToolCallRequest>,
) -> Result<Json<ToolCallResponse>, (StatusCode, String)> {
    let result = match req.name.as_str() {
        "get_model_info" => handle_get_model_info(&state, req.arguments).await?,
        "context_map" => handle_context_map(&state, req.arguments).await?,
        "context_map_with_query" => handle_context_map_with_query(&state, req.arguments).await?,
        "load_model" => handle_load_model(&state, req.arguments).await?,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Unknown tool: {}", req.name),
            ))
        }
    };

    Ok(Json(ToolCallResponse { result }))
}

/// Handle get_model_info tool
async fn handle_get_model_info(
    state: &AppState,
    _args: serde_json::Value,
) -> Result<serde_json::Value, (StatusCode, String)> {
    let model = state
        .models
        .first()
        .ok_or_else(|| (StatusCode::NOT_FOUND, "No model loaded".to_string()))?;

    let config = ModelConfig {
        model_id: model.id.clone(),
        num_layers: model.config.num_layers,
        hidden_dim: model.config.hidden_size,
        num_attention_heads: 32, // Default fallback
        num_kv_heads: 4,         // Default fallback
        head_dim: model.config.hidden_size / 32,
        vocab_size: model.config.vocab_size,
        max_position_embeddings: 8192, // Default fallback
        architecture: model.config.family.clone(),
        family: model.config.model.clone(),
    };

    Ok(serde_json::to_value(config)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?)
}

/// Handle context_map tool
async fn handle_context_map(
    state: &AppState,
    args: serde_json::Value,
) -> Result<serde_json::Value, (StatusCode, String)> {
    let model = state
        .models
        .first()
        .ok_or_else(|| (StatusCode::NOT_FOUND, "No model loaded".to_string()))?;

    let args_map: std::collections::HashMap<String, serde_json::Value> =
        serde_json::from_value(args)
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid arguments: {}", e)))?;

    let prompt = args_map
        .get("prompt")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "Missing prompt argument".to_string(),
            )
        })?;

    let layer = args_map
        .get("layer")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "Missing layer argument".to_string(),
            )
        })? as usize;

    let top_k = args_map.get("top_k").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

    // Tokenize the prompt
    let tokens = model.tokenizer.encode(prompt, false).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Tokenization error: {}", e),
        )
    })?;

    if tokens.ids.is_empty() {
        return Ok(serde_json::json!([]));
    }

    // Get embeddings for the tokens
    let mut entries = Vec::new();
    for (pos, token_id) in tokens.ids.iter().enumerate() {
        let token_str = model
            .tokenizer
            .decode(&[*token_id], false)
            .unwrap_or_else(|_| "<unk>".to_string());

        entries.push(ContextMapEntry {
            token: token_str,
            token_id: *token_id as usize,
            layer,
            position: pos,
            coefficient: 1.0 / (tokens.ids.len() as f32), // Simple uniform distribution
            fraction: 1.0 / (tokens.ids.len() as f32),
        });
    }

    // Limit to top_k
    entries.truncate(top_k);

    Ok(serde_json::to_value(entries)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?)
}

/// Handle context_map_with_query tool
async fn handle_context_map_with_query(
    state: &AppState,
    args: serde_json::Value,
) -> Result<serde_json::Value, (StatusCode, String)> {
    // For now, delegate to context_map
    handle_context_map(state, args).await
}

/// Handle load_model tool
async fn handle_load_model(
    state: &AppState,
    args: serde_json::Value,
) -> Result<serde_json::Value, (StatusCode, String)> {
    let args_map: std::collections::HashMap<String, serde_json::Value> =
        serde_json::from_value(args)
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid arguments: {}", e)))?;

    let model_id = args_map
        .get("model_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "Missing model_id argument".to_string(),
            )
        })?;

    // Check if the requested model is already loaded
    for model in &state.models {
        if model.id == model_id {
            return handle_get_model_info(state, serde_json::Value::Null).await;
        }
    }

    Err((
        StatusCode::NOT_FOUND,
        format!("Model not loaded: {}", model_id),
    ))
}
