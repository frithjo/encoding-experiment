//! Shared DESCRIBE edge aggregation from a [`WalkTrace`](crate::WalkTrace).
//!
//! Matches `larql-lql` `describe_collect_edges` so local LQL and HTTP `/v1/describe` stay aligned.

use std::collections::HashMap;

use crate::index::core::WalkTrace;
use crate::text::{is_content_token, is_readable_token};

/// Per-target accumulator after walk + dedupe (before relation labeling).
#[derive(Debug, Clone)]
pub struct CollectedDescribeEdge {
    pub gate: f32,
    pub layers: Vec<usize>,
    pub count: usize,
    pub original: String,
    pub also: Vec<String>,
    pub best_layer: usize,
    pub best_feature: usize,
}

/// Same per-layer walk breadth as `exec_describe` in larql-lql (`patched.walk(..., 20)`).
pub const DESCRIBE_WALK_TOP_K: usize = 20;

/// Default gate-floor filter for DESCRIBE edge collection (HTTP `/v1/describe`, LQL local path, gRPC).
pub const DESCRIBE_GATE_FLOOR_DEFAULT: f32 = 5.0;

/// Walk the trace, deduplicate by lowercased target token, and apply
/// content / coherence filters. Output is sorted descending by gate.
pub fn collect_describe_edges_from_trace(
    trace: &WalkTrace,
    entity: &str,
    gate_floor: f32,
) -> Vec<CollectedDescribeEdge> {
    let entity_lower = entity.to_lowercase();
    let mut edges: HashMap<String, CollectedDescribeEdge> = HashMap::new();

    for (layer_idx, hits) in &trace.layers {
        for hit in hits {
            if hit.gate_score < gate_floor {
                continue;
            }
            let tok = &hit.meta.top_token;
            if !is_content_token(tok) {
                continue;
            }
            if tok.to_lowercase() == entity_lower {
                continue;
            }

            let also_readable: Vec<String> = hit
                .meta
                .top_k
                .iter()
                .filter(|t| {
                    t.token.to_lowercase() != tok.to_lowercase()
                        && t.token.to_lowercase() != entity_lower
                        && is_readable_token(&t.token)
                        && t.logit > 0.0
                })
                .take(5)
                .map(|t| t.token.clone())
                .collect();

            let also: Vec<String> = also_readable
                .iter()
                .filter(|t| is_content_token(t))
                .take(3)
                .cloned()
                .collect();

            // Coherence filter: skip weak edges with no content secondaries
            if also.is_empty() && !also_readable.is_empty() && hit.gate_score < 20.0 {
                continue;
            }

            let key = tok.to_lowercase();
            let entry = edges.entry(key).or_insert_with(|| CollectedDescribeEdge {
                gate: 0.0,
                layers: Vec::new(),
                count: 0,
                original: tok.to_string(),
                also,
                best_layer: *layer_idx,
                best_feature: hit.feature,
            });

            if hit.gate_score > entry.gate {
                entry.gate = hit.gate_score;
                entry.best_layer = *layer_idx;
                entry.best_feature = hit.feature;
            }
            if !entry.layers.contains(layer_idx) {
                entry.layers.push(*layer_idx);
            }
            entry.count += 1;
        }
    }

    let mut ranked: Vec<CollectedDescribeEdge> = edges.into_values().collect();
    ranked.sort_by(|a, b| {
        b.gate
            .partial_cmp(&a.gate)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FeatureMeta, WalkHit, WalkTrace};
    use larql_models::TopKEntry;

    fn meta(top_token: &str, top_k: Vec<(&str, f32)>) -> FeatureMeta {
        FeatureMeta {
            top_token: top_token.to_string(),
            top_token_id: 0,
            c_score: 1.0,
            top_k: top_k
                .into_iter()
                .enumerate()
                .map(|(i, (t, logit))| TopKEntry {
                    token: t.to_string(),
                    token_id: i as u32,
                    logit,
                })
                .collect(),
        }
    }

    fn hit(layer: usize, feature: usize, gate: f32, meta: FeatureMeta) -> WalkHit {
        WalkHit {
            layer,
            feature,
            gate_score: gate,
            meta,
        }
    }

    #[test]
    fn rejects_non_content_primary_token() {
        let trace = WalkTrace {
            layers: vec![(0, vec![hit(0, 1, 10.0, meta("the", vec![("Paris", 1.0)]))])],
        };
        let out = collect_describe_edges_from_trace(&trace, "France", DESCRIBE_GATE_FLOOR_DEFAULT);
        assert!(out.is_empty());
    }

    #[test]
    fn coherence_skips_weak_gate_without_also() {
        // Primary "Berlin" gate 10; secondary "the" is readable but not content → also empty;
        // also_readable non-empty, gate < 20 → skip.
        let trace2 = WalkTrace {
            layers: vec![(1, vec![hit(1, 2, 10.0, meta("Berlin", vec![("the", 1.0)]))])],
        };
        let out = collect_describe_edges_from_trace(&trace2, "France", DESCRIBE_GATE_FLOOR_DEFAULT);
        assert!(out.is_empty());
    }

    #[test]
    fn dedupes_by_lower_target() {
        let m = meta("Paris", vec![("Lyon", 1.0), ("Nice", 1.0)]);
        let trace = WalkTrace {
            layers: vec![
                (0, vec![hit(0, 1, 8.0, m.clone())]),
                (1, vec![hit(1, 2, 12.0, m)]),
            ],
        };
        let out = collect_describe_edges_from_trace(&trace, "France", DESCRIBE_GATE_FLOOR_DEFAULT);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].original, "Paris");
        assert_eq!(out[0].gate, 12.0);
        assert_eq!(out[0].best_layer, 1);
        assert!(out[0].layers.contains(&0) && out[0].layers.contains(&1));
    }

    #[test]
    fn gate_floor() {
        let trace = WalkTrace {
            layers: vec![(
                0,
                vec![hit(0, 1, 4.0, meta("Berlin", vec![("Munich", 1.0)]))],
            )],
        };
        assert!(
            collect_describe_edges_from_trace(&trace, "France", DESCRIBE_GATE_FLOOR_DEFAULT)
                .is_empty()
        );
        assert!(!collect_describe_edges_from_trace(&trace, "France", 4.0).is_empty());
    }
}
