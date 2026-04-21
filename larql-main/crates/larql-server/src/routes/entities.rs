//! GET /v1/entities — entity introspection for remote backends.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;

use crate::error::ServerError;
use crate::state::{AppState, LoadedModel};

#[derive(Deserialize)]
pub struct EntitiesRequest {
    #[serde(default)]
    pub layer: Option<u32>,
    #[serde(default)]
    pub limit: Option<u32>,
}

pub async fn handle_entities(
    State(state): State<Arc<AppState>>,
    Query(req): Query<EntitiesRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    state.bump_requests();
    let model = state
        .model(None)
        .ok_or_else(|| ServerError::NotFound("no model loaded".into()))?;
    let model = Arc::clone(model);
    let result = tokio::task::spawn_blocking(move || {
        run_entities(&model, &req)
    })
    .await
    .map_err(|e| ServerError::Internal(e.to_string()))??;
    Ok(Json(result))
}

fn run_entities(
    model: &LoadedModel,
    req: &EntitiesRequest,
) -> Result<serde_json::Value, ServerError> {
    let patched = model.patched.blocking_read();
    let config = &model.config;

    let limit = req.limit.unwrap_or(50) as usize;

    let scan_layers: Vec<usize> = if let Some(l) = req.layer {
        vec![l as usize]
    } else {
        (0..config.num_layers).collect()
    };

    // Collect distinct top_tokens across all scanned features
    let mut entity_counts: std::collections::HashMap<String, (usize, f32)> =
        std::collections::HashMap::new();

    for layer in &scan_layers {
        let nf = patched.num_features(*layer);
        for feat in 0..nf {
            if let Some(meta) = patched.feature_meta(*layer, feat) {
                let tok = meta.top_token.trim().to_string();
                // Filter to named entities: starts with uppercase
                // ASCII, 3+ chars, all alphabetic
                if tok.len() < 3 {
                    continue;
                }
                let first = tok.chars().next().unwrap_or('\0');
                if !first.is_ascii_uppercase() {
                    continue;
                }
                if !tok.chars().all(|c| c.is_ascii_alphabetic()) {
                    continue;
                }

                let entry = entity_counts.entry(tok).or_insert((0, 0.0));
                entry.0 += 1;
                entry.1 = entry.1.max(meta.c_score);
            }
        }
    }

    // Sort by count descending, then by score descending
    let mut entities: Vec<(String, usize, f32)> = entity_counts
        .into_iter()
        .map(|(k, (count, score))| (k, count, score))
        .collect();
    entities.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal))
    });
    entities.truncate(limit);

    let data: Vec<serde_json::Value> = entities
        .iter()
        .map(|(entity, count, score)| {
            serde_json::json!({
                "entity": entity,
                "count": count,
                "score": score,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "entities": data,
    }))
}

pub async fn handle_entities_multi(
    State(state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
    Query(req): Query<EntitiesRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    state.bump_requests();
    let model = state
        .model(Some(&model_id))
        .ok_or_else(|| ServerError::NotFound(format!("model '{}' not found", model_id)))?;
    let model = Arc::clone(model);
    let result = tokio::task::spawn_blocking(move || {
        run_entities(&model, &req)
    })
    .await
    .map_err(|e| ServerError::Internal(e.to_string()))??;
    Ok(Json(result))
}
