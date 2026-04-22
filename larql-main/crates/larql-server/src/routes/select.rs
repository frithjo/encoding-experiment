//! POST /v1/select — SQL-style edge query.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;

use crate::error::ServerError;
use crate::state::{AppState, LoadedModel};

#[derive(Deserialize)]
pub struct SelectRequest {
    #[serde(default)]
    pub entity: Option<String>,
    /// Filter by probe-confirmed relation label.
    #[serde(default)]
    pub relation: Option<String>,
    #[serde(default)]
    pub layer: Option<usize>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub confidence_floor: Option<f32>,
    #[serde(default)]
    pub order_by: Option<String>,
    #[serde(default = "default_order")]
    pub order: String,
    #[serde(default)]
    pub fields: Option<Vec<String>>,
    #[serde(default)]
    pub nearest: Option<NearestRequest>,
}

#[derive(Deserialize)]
pub struct NearestRequest {
    pub entity: String,
    pub layer: u32,
}

fn default_limit() -> usize {
    20
}
fn default_order() -> String {
    "desc".into()
}

fn select_edges(
    model: &LoadedModel,
    req: &SelectRequest,
) -> Result<serde_json::Value, ServerError> {
    let start = std::time::Instant::now();

    // Handle NEAREST clause - KNN lookup
    if let Some(ref nearest) = req.nearest {
        return select_nearest(model, nearest, req.limit);
    }

    let patched = model.patched.blocking_read();
    let all_layers = patched.loaded_layers();

    let scan_layers: Vec<usize> = if let Some(l) = req.layer {
        vec![l]
    } else {
        all_layers
    };

    struct Row {
        layer: usize,
        feature: usize,
        top_token: String,
        c_score: f32,
        relation: Option<String>,
    }

    let mut rows: Vec<Row> = Vec::new();

    for &layer in &scan_layers {
        let num_features = patched.num_features(layer);
        for feat_idx in 0..num_features {
            // Check probe label first (fast filter when relation is specified)
            let relation = model.probe_labels.get(&(layer, feat_idx)).cloned();
            if let Some(ref rel_filter) = req.relation {
                match &relation {
                    Some(r) if r.to_lowercase().contains(&rel_filter.to_lowercase()) => {}
                    _ => continue,
                }
            }

            // Get feature metadata (handles both heap and mmap down_meta)
            if let Some(meta) = patched.feature_meta(layer, feat_idx) {
                if let Some(ref ent) = req.entity {
                    if !meta.top_token.to_lowercase().contains(&ent.to_lowercase()) {
                        continue;
                    }
                }
                if let Some(min_c) = req.confidence_floor {
                    if meta.c_score < min_c {
                        continue;
                    }
                }
                rows.push(Row {
                    layer,
                    feature: feat_idx,
                    top_token: meta.top_token.clone(),
                    c_score: meta.c_score,
                    relation,
                });
            }
        }
    }

    let descending = req.order == "desc";
    match req.order_by.as_deref() {
        Some("gate_score") | Some("confidence") | Some("c_score") => {
            rows.sort_by(|a, b| {
                let cmp = a
                    .c_score
                    .partial_cmp(&b.c_score)
                    .unwrap_or(std::cmp::Ordering::Equal);
                if descending {
                    cmp.reverse()
                } else {
                    cmp
                }
            });
        }
        Some("layer") => {
            rows.sort_by(|a, b| {
                let cmp = a.layer.cmp(&b.layer);
                if descending {
                    cmp.reverse()
                } else {
                    cmp
                }
            });
        }
        _ => {
            rows.sort_by(|a, b| {
                let cmp = a
                    .c_score
                    .partial_cmp(&b.c_score)
                    .unwrap_or(std::cmp::Ordering::Equal);
                cmp.reverse()
            });
        }
    }

    let total = rows.len();
    rows.truncate(req.limit);

    let edges: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            let mut edge = serde_json::json!({
                "layer": r.layer,
                "feature": r.feature,
                "target": r.top_token.trim(),
                "c_score": r.c_score,
            });
            if let Some(ref rel) = r.relation {
                edge["relation"] = serde_json::json!(rel);
            }
            // Apply field filtering if specified
            if let Some(ref fields) = req.fields {
                let mut filtered = serde_json::Map::new();
                for field in fields {
                    if let Some(val) = edge.get(field) {
                        filtered.insert(field.clone(), val.clone());
                    }
                }
                edge = serde_json::Value::Object(filtered);
            }
            edge
        })
        .collect();

    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

    Ok(serde_json::json!({
        "edges": edges,
        "total": total,
        "latency_ms": (latency_ms * 10.0).round() / 10.0,
    }))
}

fn select_nearest(
    model: &LoadedModel,
    nearest: &NearestRequest,
    limit: usize,
) -> Result<serde_json::Value, ServerError> {
    let patched = model.patched.blocking_read();
    let path = &model.path;

    // Load embeddings and tokenizer
    let (embed, embed_scale) = larql_vindex::load_vindex_embeddings(path)
        .map_err(|e| ServerError::Internal(format!("failed to load embeddings: {}", e)))?;
    let tokenizer = larql_vindex::load_vindex_tokenizer(path)
        .map_err(|e| ServerError::Internal(format!("failed to load tokenizer: {}", e)))?;

    let encoding = tokenizer
        .encode(nearest.entity.as_str(), false)
        .map_err(|e| ServerError::Internal(format!("tokenize error: {}", e)))?;
    let token_ids: Vec<u32> = encoding.ids.clone();

    if token_ids.is_empty() {
        return Ok(serde_json::json!({
            "edges": [],
            "total": 0,
            "message": "entity not found",
        }));
    }

    // Build query from entity embedding
    let hidden = embed.shape()[1];
    let query = if token_ids.len() == 1 {
        embed.row(token_ids[0] as usize).mapv(|v| v * embed_scale)
    } else {
        let mut avg = larql_vindex::ndarray::Array1::<f32>::zeros(hidden);
        for &tok in &token_ids {
            avg += &embed.row(tok as usize).mapv(|v| v * embed_scale);
        }
        avg.mapv(|v| v / token_ids.len() as f32)
    };

    // Scan features at specified layer and compute cosine similarity
    let layer = nearest.layer as usize;
    let num_features = patched.num_features(layer);
    let mut similarities: Vec<(usize, f32)> = Vec::new();

    for feat_idx in 0..num_features {
        if let Some(meta) = patched.feature_meta(layer, feat_idx) {
            let encoding = tokenizer
                .encode(meta.top_token.trim(), false)
                .map_err(|e| ServerError::Internal(format!("tokenize error: {}", e)))?;
            let feat_token_ids: Vec<u32> = encoding.ids.clone();

            if !feat_token_ids.is_empty() {
                let feat_embed = if feat_token_ids.len() == 1 {
                    embed
                        .row(feat_token_ids[0] as usize)
                        .mapv(|v| v * embed_scale)
                } else {
                    let mut avg = larql_vindex::ndarray::Array1::<f32>::zeros(hidden);
                    for &tok in &feat_token_ids {
                        avg += &embed.row(tok as usize).mapv(|v| v * embed_scale);
                    }
                    avg.mapv(|v| v / feat_token_ids.len() as f32)
                };

                // Cosine similarity
                let dot = query.dot(&feat_embed);
                let norm_q = query.iter().map(|x| x * x).sum::<f32>().sqrt();
                let norm_f = feat_embed.iter().map(|x| x * x).sum::<f32>().sqrt();
                let sim = if norm_q > 0.0 && norm_f > 0.0 {
                    dot / (norm_q * norm_f)
                } else {
                    0.0
                };

                similarities.push((feat_idx, sim));
            }
        }
    }

    // Sort by similarity descending
    similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    similarities.truncate(limit);

    let edges: Vec<serde_json::Value> = similarities
        .iter()
        .map(|(feat_idx, sim)| {
            let meta = patched.feature_meta(layer, *feat_idx);
            let top_token = meta.as_ref().map(|m| m.top_token.trim()).unwrap_or("?");
            let c_score = meta.as_ref().map(|m| m.c_score).unwrap_or(0.0);
            let relation = model.probe_labels.get(&(layer, *feat_idx)).cloned();

            let mut edge = serde_json::json!({
                "layer": layer,
                "feature": feat_idx,
                "target": top_token,
                "similarity": sim,
                "c_score": c_score,
            });
            if let Some(ref rel) = relation {
                edge["relation"] = serde_json::json!(rel);
            }
            edge
        })
        .collect();

    Ok(serde_json::json!({
        "edges": edges,
        "total": similarities.len(),
    }))
}

pub async fn handle_select(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SelectRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    state.bump_requests();
    let model = state
        .model(None)
        .ok_or_else(|| ServerError::NotFound("no model loaded".into()))?;
    let model = Arc::clone(model);
    let result = tokio::task::spawn_blocking(move || select_edges(&model, &req))
        .await
        .map_err(|e| ServerError::Internal(e.to_string()))??;
    Ok(Json(result))
}

pub async fn handle_select_multi(
    State(state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
    Json(req): Json<SelectRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    state.bump_requests();
    let model = state
        .model(Some(&model_id))
        .ok_or_else(|| ServerError::NotFound(format!("model '{}' not found", model_id)))?;
    let model = Arc::clone(model);
    let result = tokio::task::spawn_blocking(move || select_edges(&model, &req))
        .await
        .map_err(|e| ServerError::Internal(e.to_string()))??;
    Ok(Json(result))
}
