//! GET /v1/layers — layer introspection for remote backends.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;

use crate::error::ServerError;
use crate::state::{AppState, LoadedModel};

#[derive(Deserialize)]
pub struct LayersRequest {
    #[serde(default)]
    pub start: Option<u32>,
    #[serde(default)]
    pub end: Option<u32>,
}

pub async fn handle_layers(
    State(state): State<Arc<AppState>>,
    Query(req): Query<LayersRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    state.bump_requests();
    let model = state
        .model(None)
        .ok_or_else(|| ServerError::NotFound("no model loaded".into()))?;
    let model = Arc::clone(model);
    let result = tokio::task::spawn_blocking(move || {
        run_layers(&model, &req)
    })
    .await
    .map_err(|e| ServerError::Internal(e.to_string()))??;
    Ok(Json(result))
}

fn run_layers(
    model: &LoadedModel,
    req: &LayersRequest,
) -> Result<serde_json::Value, ServerError> {
    let patched = model.patched.blocking_read();
    let all_layers = patched.loaded_layers();

    let show_layers: Vec<usize> = if let (Some(start), Some(end)) = (req.start, req.end) {
        (start as usize..=end as usize)
            .filter(|l| all_layers.contains(l))
            .collect()
    } else {
        all_layers
    };

    let layers: Vec<serde_json::Value> = show_layers
        .iter()
        .map(|layer| {
            let gate_count = patched
                .gate_vectors_at(*layer)
                .map(|m| m.shape()[0])
                .unwrap_or(0);
            let (meta_count, top_tok) = if let Some(metas) = patched.down_meta_at(*layer) {
                let count = metas.iter().filter(|m| m.is_some()).count();
                let mut freq: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
                for m in metas.iter().flatten() {
                    *freq.entry(&m.top_token).or_default() += 1;
                }
                let top = freq
                    .into_iter()
                    .max_by_key(|(_, c)| *c)
                    .map(|(t, _)| t.to_string())
                    .unwrap_or_default();
                (count, top)
            } else {
                (0, String::new())
            };

            serde_json::json!({
                "layer": *layer,
                "features": gate_count,
                "with_meta": meta_count,
                "top_token": top_tok,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "layers": layers,
    }))
}

pub async fn handle_layers_multi(
    State(state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
    Query(req): Query<LayersRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    state.bump_requests();
    let model = state
        .model(Some(&model_id))
        .ok_or_else(|| ServerError::NotFound(format!("model '{}' not found", model_id)))?;
    let model = Arc::clone(model);
    let result = tokio::task::spawn_blocking(move || {
        run_layers(&model, &req)
    })
    .await
    .map_err(|e| ServerError::Internal(e.to_string()))??;
    Ok(Json(result))
}
