//! POST /v1/analyze-infer - scientific attribution analysis for a prompt.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use larql_inference::{analyze_infer, AnalysisRequest, AnalysisResult};

use crate::error::ServerError;
use crate::state::{AppState, LoadedModel};

fn run_analyze_infer(
    model: &LoadedModel,
    req: &AnalysisRequest,
) -> Result<AnalysisResult, ServerError> {
    let weights = model
        .get_or_load_weights()
        .map_err(ServerError::InferenceUnavailable)?;
    analyze_infer(weights, &*model.tokenizer, model.config.num_layers, req)
        .map_err(ServerError::Internal)
}

pub async fn handle_analyze(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AnalysisRequest>,
) -> Result<Json<AnalysisResult>, ServerError> {
    state.bump_requests();
    let model = state
        .model(None)
        .ok_or_else(|| ServerError::NotFound("no model loaded".into()))?;
    let model = Arc::clone(model);
    let result = tokio::task::spawn_blocking(move || run_analyze_infer(&model, &req))
        .await
        .map_err(|e| ServerError::Internal(e.to_string()))??;
    Ok(Json(result))
}

pub async fn handle_analyze_multi(
    State(state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
    Json(req): Json<AnalysisRequest>,
) -> Result<Json<AnalysisResult>, ServerError> {
    state.bump_requests();
    let model = state
        .model(Some(&model_id))
        .ok_or_else(|| ServerError::NotFound(format!("model '{}' not found", model_id)))?;
    let model = Arc::clone(model);
    let result = tokio::task::spawn_blocking(move || run_analyze_infer(&model, &req))
        .await
        .map_err(|e| ServerError::Internal(e.to_string()))??;
    Ok(Json(result))
}
