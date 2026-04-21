//! GET /v1/tokens — structured token summary backing remote `SHOW TOKENS`.
//!
//! Returns pre-aggregated shape/kind rows grouped by layer, band, or flat.
//! Rendering to text/CSV/JSON happens client-side so local and remote paths
//! share formatters.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;

use larql_vindex::token_summary::{
    collect_token_hits, token_shape_name, TokenFilters, TokenHit,
};

use crate::error::ServerError;
use crate::state::{AppState, LoadedModel};

#[derive(Deserialize)]
pub struct TokensParams {
    /// Single-layer restriction; overrides `group_by` selection for `band`.
    #[serde(default)]
    pub layer: Option<u32>,
    /// "layer" | "band" | unset (flat).
    #[serde(default)]
    pub group_by: Option<String>,
    /// WHERE token/entity LIKE.
    #[serde(default)]
    pub token_filter: Option<String>,
    /// WHERE shape = ...
    #[serde(default)]
    pub shape_filter: Option<String>,
    /// WHERE type/kind = ...
    #[serde(default)]
    pub type_filter: Option<String>,
    /// WHERE band = ...
    #[serde(default)]
    pub band_filter: Option<String>,
}

fn build_filters(p: &TokensParams) -> TokenFilters {
    TokenFilters {
        token_filter: p.token_filter.clone(),
        shape_filter: p.shape_filter.as_ref().map(|s| s.to_lowercase()),
        type_filter: p.type_filter.as_ref().map(|s| s.to_lowercase()),
        band_filter: p.band_filter.as_ref().map(|s| s.to_lowercase()),
    }
}

/// Encode one per-token hit in a portable, language-agnostic form so the
/// client can reconstruct `HashMap<String, TokenHit>` and call local
/// formatters verbatim.
fn token_row(token: &str, hit: &TokenHit) -> serde_json::Value {
    serde_json::json!({
        "token": token,
        "shape": token_shape_name(hit.shape),
        "kind": hit.kind,
        "hits": hit.hits,
        "syntax_hits": hit.syntax_hits,
        "knowledge_hits": hit.knowledge_hits,
        "output_hits": hit.output_hits,
        "max_score": hit.max_score,
    })
}

fn describe_group(
    label: String,
    token_hits: std::collections::HashMap<String, TokenHit>,
) -> serde_json::Value {
    let mut sorted: Vec<_> = token_hits.into_iter().collect();
    // Sort by hits (desc), then max_score (desc), then token (asc) to match
    // the local renderer's sorting logic for byte-identical output.
    sorted.sort_by(|a, b| {
        b.1.hits
            .cmp(&a.1.hits)
            .then_with(|| {
                b.1.max_score
                    .partial_cmp(&a.1.max_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.0.cmp(&b.0))
    });
    let rows: Vec<serde_json::Value> = sorted
        .iter()
        .map(|(tok, hit)| token_row(tok, hit))
        .collect();
    serde_json::json!({
        "label": label,
        "token_hits": rows,
        "token_count": rows.len(),
    })
}

fn run_tokens(
    model: &LoadedModel,
    params: &TokensParams,
) -> Result<serde_json::Value, ServerError> {
    let start = std::time::Instant::now();
    let config = &model.config;
    let last = config.num_layers.saturating_sub(1);
    let bands = config
        .layer_bands
        .clone()
        .or_else(|| larql_vindex::LayerBands::for_family(&config.family, config.num_layers))
        .unwrap_or(larql_vindex::LayerBands {
            syntax: (0, last),
            knowledge: (0, last),
            output: (0, last),
        });

    let patched = model.patched.blocking_read();

    let scan_layers: Vec<usize> = if let Some(l) = params.layer {
        vec![l as usize]
    } else {
        (0..config.num_layers).collect()
    };

    let filters = build_filters(params);
    let mode = params.group_by.as_deref().unwrap_or("");

    let mut groups: Vec<serde_json::Value> = Vec::new();
    let mut skipped = 0usize;

    match mode {
        "layer" => {
            for layer in &scan_layers {
                let hits = collect_token_hits(&patched, &[*layer], &bands, &filters);
                if hits.is_empty() {
                    skipped += 1;
                    continue;
                }
                groups.push(describe_group(format!("at layer {}", layer), hits));
            }
        }
        "band" => {
            let band_groups = [
                ("syntax", bands.syntax.0, bands.syntax.1),
                ("knowledge", bands.knowledge.0, bands.knowledge.1),
                ("output", bands.output.0, bands.output.1),
            ];
            for (name, s, e) in band_groups {
                let group_layers: Vec<usize> = scan_layers
                    .iter()
                    .copied()
                    .filter(|l| *l >= s && *l <= e)
                    .collect();
                let hits = collect_token_hits(&patched, &group_layers, &bands, &filters);
                if hits.is_empty() {
                    skipped += 1;
                    continue;
                }
                groups.push(describe_group(
                    format!("for {} band (L{}-{})", name, s, e),
                    hits,
                ));
            }
        }
        _ => {
            let hits = collect_token_hits(&patched, &scan_layers, &bands, &filters);
            let label = if let Some(l) = params.layer {
                format!("at layer {}", l)
            } else {
                format!("across {} layers", scan_layers.len())
            };
            groups.push(describe_group(label, hits));
        }
    }

    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    Ok(serde_json::json!({
        "model": config.model,
        "group_by": mode,
        "groups": groups,
        "skipped": skipped,
        "scan_layers": scan_layers.len(),
        "latency_ms": (latency_ms * 10.0).round() / 10.0,
    }))
}

pub async fn handle_tokens(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TokensParams>,
) -> Result<Json<serde_json::Value>, ServerError> {
    state.bump_requests();
    let model = state
        .model(None)
        .ok_or_else(|| ServerError::NotFound("no model loaded".into()))?;
    let model = Arc::clone(model);
    let result = tokio::task::spawn_blocking(move || run_tokens(&model, &params))
        .await
        .map_err(|e| ServerError::Internal(e.to_string()))??;
    Ok(Json(result))
}

pub async fn handle_tokens_multi(
    State(state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
    Query(params): Query<TokensParams>,
) -> Result<Json<serde_json::Value>, ServerError> {
    state.bump_requests();
    let model = state
        .model(Some(&model_id))
        .ok_or_else(|| ServerError::NotFound(format!("model '{}' not found", model_id)))?;
    let model = Arc::clone(model);
    let result = tokio::task::spawn_blocking(move || run_tokens(&model, &params))
        .await
        .map_err(|e| ServerError::Internal(e.to_string()))??;
    Ok(Json(result))
}
