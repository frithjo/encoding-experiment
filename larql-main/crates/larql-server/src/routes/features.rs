//! GET /v1/features — feature introspection for remote backends.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;

use crate::error::ServerError;
use crate::state::{AppState, LoadedModel};

#[derive(Deserialize)]
pub struct FeaturesRequest {
    pub layer: u32,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub min_score: Option<f32>,
    #[serde(default)]
    pub limit: Option<u32>,
}

pub async fn handle_features(
    State(state): State<Arc<AppState>>,
    Query(req): Query<FeaturesRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    state.bump_requests();
    let model = state
        .model(None)
        .ok_or_else(|| ServerError::NotFound("no model loaded".into()))?;
    let model = Arc::clone(model);
    let result = tokio::task::spawn_blocking(move || {
        run_features(&model, &req)
    })
    .await
    .map_err(|e| ServerError::Internal(e.to_string()))??;
    Ok(Json(result))
}

fn run_features(
    model: &LoadedModel,
    req: &FeaturesRequest,
) -> Result<serde_json::Value, ServerError> {
    let patched = model.patched.blocking_read();
    let config = &model.config;

    let limit = req.limit.unwrap_or(config.num_layers as u32) as usize;

    let nf = patched.num_features(req.layer as usize);
    if nf == 0 {
        return Ok(serde_json::json!({
            "error": format!("no features at layer {}", req.layer),
        }));
    }

    let mut features: Vec<serde_json::Value> = Vec::new();
    let mut count = 0;

    for feat_idx in 0..nf {
        if count >= limit {
            break;
        }
        if let Some(meta) = patched.feature_meta(req.layer as usize, feat_idx) {
            // Apply WHERE filters
            if let Some(ref tf) = req.token {
                if !meta.top_token.to_lowercase().contains(&tf.to_lowercase()) {
                    continue;
                }
            }
            if let Some(ms) = req.min_score {
                if meta.c_score < ms {
                    continue;
                }
            }

            let down_outputs = if !meta.top_k.is_empty() {
                meta.top_k.iter()
                    .map(|t| format!("{} ({:.1})", t.token, t.logit))
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                String::new()
            };

            features.push(serde_json::json!({
                "feature": feat_idx,
                "top_token": meta.top_token,
                "score": meta.c_score,
                "down_outputs": down_outputs,
            }));
            count += 1;
        }
    }

    Ok(serde_json::json!({
        "layer": req.layer,
        "features": features,
    }))
}

pub async fn handle_features_multi(
    State(state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
    Query(req): Query<FeaturesRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    state.bump_requests();
    let model = state
        .model(Some(&model_id))
        .ok_or_else(|| ServerError::NotFound(format!("model '{}' not found", model_id)))?;
    let model = Arc::clone(model);
    let result = tokio::task::spawn_blocking(move || {
        run_features(&model, &req)
    })
    .await
    .map_err(|e| ServerError::Internal(e.to_string()))??;
    Ok(Json(result))
}
