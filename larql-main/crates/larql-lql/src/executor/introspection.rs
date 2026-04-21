//! Introspection executor: SHOW RELATIONS, SHOW LAYERS, SHOW FEATURES, SHOW MODELS.

use std::collections::HashMap;

use crate::ast::*;
use crate::error::LqlError;
use super::Session;
use super::helpers::{format_number, format_bytes, dir_size, is_content_token};
use super::helpers::{token_shape_name, TokenShape};
use super::helpers::{collect_token_hits, KindInfo, ShapeInfo, TokenFilters, TokenHit};

impl Session {
    pub(crate) fn exec_show_relations(
        &self,
        layer_filter: Option<u32>,
        with_examples: bool,
        mode: DescribeMode,
    ) -> Result<Vec<String>, LqlError> {
        let (_path, _config, patched) = self.require_vindex()?;

        let all_layers = patched.loaded_layers();
        let scan_layers: Vec<usize> = if let Some(l) = layer_filter {
            vec![l as usize]
        } else {
            all_layers.iter().copied().filter(|l| *l >= 14 && *l <= 27).collect()
        };

        // ── Probe-confirmed relations (skip for Raw mode) ──
        let classifier = self.relation_classifier();
        let mut probe_relations: HashMap<String, usize> = HashMap::new();
        if mode != DescribeMode::Raw {
            if let Some(rc) = classifier {
                for &layer in &scan_layers {
                    let num_features = patched.num_features(layer);
                    for feat in 0..num_features {
                        if let Some(label) = rc.label_for_feature(layer, feat) {
                            *probe_relations.entry(label.to_string()).or_insert(0) += 1;
                        }
                    }
                }
            }
        }

        // ── Raw token relations (skip for Brief mode unless no probes) ──
        struct TokenInfo {
            count: usize,
            max_score: f32,
            min_layer: usize,
            max_layer: usize,
            original: String,
            examples: Vec<String>,
        }

        let show_raw = mode == DescribeMode::Raw
            || mode == DescribeMode::Verbose
            || probe_relations.is_empty();

        let mut tokens: HashMap<String, TokenInfo> = HashMap::new();
        if show_raw {
            for &layer in &scan_layers {
                if let Some(metas) = patched.down_meta_at(layer) {
                    for meta in metas.iter().flatten() {
                        let tok = meta.top_token.trim();
                        if !is_content_token(tok) {
                            continue;
                        }
                        if meta.c_score < 0.2 {
                            continue;
                        }
                        let key = tok.to_lowercase();
                        let examples: Vec<String> = meta.top_k.iter()
                            .filter(|t| t.token.trim() != tok && is_content_token(t.token.trim()))
                            .take(3)
                            .map(|t| t.token.trim().to_string())
                            .collect();
                        let entry = tokens.entry(key).or_insert(TokenInfo {
                            count: 0,
                            max_score: 0.0,
                            min_layer: layer,
                            max_layer: layer,
                            original: tok.to_string(),
                            examples,
                        });
                        entry.count += 1;
                        if meta.c_score > entry.max_score {
                            entry.max_score = meta.c_score;
                        }
                        if layer < entry.min_layer {
                            entry.min_layer = layer;
                        }
                        if layer > entry.max_layer {
                            entry.max_layer = layer;
                        }
                    }
                }
            }
        }

        let mut out = Vec::new();
        let layer_label = if let Some(l) = layer_filter {
            format!("L{}", l)
        } else {
            "L14-27".into()
        };

        // ── Probe-confirmed section ──
        if !probe_relations.is_empty() {
            let total_labels: usize = probe_relations.values().sum();
            out.push(format!("Probe-confirmed relations ({} labels):", total_labels));
            out.push(format!("{:<25} {:>8}", "Relation", "Features"));
            out.push("-".repeat(35));

            let mut probe_sorted: Vec<(&String, &usize)> = probe_relations.iter().collect();
            probe_sorted.sort_by(|a, b| b.1.cmp(a.1));

            let limit = if mode == DescribeMode::Brief { 30 } else { probe_sorted.len() };
            for (name, count) in probe_sorted.into_iter().take(limit) {
                out.push(format!("{:<25} {:>8}", name, count));
            }
        }

        // ── Raw token section ──
        if show_raw && !tokens.is_empty() {
            if !probe_relations.is_empty() {
                out.push(String::new());
            }

            let mut sorted: Vec<(&str, &TokenInfo)> = tokens.values()
                .map(|info| (info.original.as_str(), info))
                .collect();
            sorted.sort_by(|a, b| b.1.count.cmp(&a.1.count));

            let limit = if mode == DescribeMode::Verbose { 50 } else { 30 };
            sorted.truncate(limit);

            out.push(format!(
                "Top output tokens ({}):",
                layer_label
            ));
            out.push(format!(
                "{:<25} {:>8} {:>8} {:>10}",
                "Token", "Count", "Score", "Layers"
            ));
            out.push("-".repeat(55));

            for (tok, info) in &sorted {
                let examples_str = if with_examples && !info.examples.is_empty() {
                    format!("  e.g. {}", info.examples.join(", "))
                } else {
                    String::new()
                };
                out.push(format!(
                    "{:<25} {:>8} {:>8.2} {:>5}-{}{}",
                    tok,
                    info.count,
                    info.max_score,
                    info.min_layer,
                    info.max_layer,
                    examples_str,
                ));
            }
        }

        if out.is_empty() || (probe_relations.is_empty() && tokens.is_empty()) {
            out.push("  (no relations found)".into());
        }

        Ok(out)
    }

    pub(crate) fn exec_show_layers(&self, range: Option<&Range>) -> Result<Vec<String>, LqlError> {
        let (_path, _config, patched) = self.require_vindex()?;

        let all_layers = patched.loaded_layers();
        let show_layers: Vec<usize> = if let Some(r) = range {
            (r.start as usize..=r.end as usize)
                .filter(|l| all_layers.contains(l))
                .collect()
        } else {
            all_layers
        };

        let mut out = Vec::new();
        out.push(format!(
            "{:<8} {:>10} {:>10} {:>15}",
            "Layer", "Features", "With Meta", "Top Token"
        ));
        out.push("-".repeat(48));

        for layer in &show_layers {
            let gate_count = patched
                .gate_vectors_at(*layer)
                .map(|m| m.shape()[0])
                .unwrap_or(0);
            let (meta_count, top_tok) = if let Some(metas) = patched.down_meta_at(*layer) {
                let count = metas.iter().filter(|m| m.is_some()).count();
                let mut freq: HashMap<&str, usize> = HashMap::new();
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

            out.push(format!(
                "L{:<7} {:>10} {:>10} {:>15}",
                layer,
                format_number(gate_count),
                format_number(meta_count),
                top_tok
            ));
        }

        Ok(out)
    }

    pub(crate) fn exec_show_features(
        &self,
        layer: u32,
        conditions: &[Condition],
        limit: Option<u32>,
    ) -> Result<Vec<String>, LqlError> {
        let (_path, config, patched) = self.require_vindex()?;
        // Default to num_layers — a manageable screenful that matches
        // the model's depth. Use LIMIT for more or fewer.
        let limit = limit.unwrap_or(config.num_layers as u32) as usize;

        // Extract filters from WHERE conditions
        let token_filter = conditions.iter().find(|c| c.field == "relation" || c.field == "token").and_then(|c| {
            if let Value::String(ref s) = c.value { Some(s.as_str()) } else { None }
        });
        let confidence_floor = conditions.iter().find(|c| c.field == "confidence" || c.field == "c_score").and_then(|c| {
            match &c.value {
                Value::Number(n) => Some(*n as f32),
                Value::Integer(n) => Some(*n as f32),
                _ => None,
            }
        });

        let nf = patched.num_features(layer as usize);
        if nf == 0 {
            return Err(LqlError::Execution(format!("no features at layer {layer}")));
        }

        let mut out = Vec::new();
        out.push(format!(
            "{:<8} {:<20} {:>10} {:>30}",
            "Feature", "Top Token", "Score", "Down outputs"
        ));
        out.push("-".repeat(72));

        let mut count = 0;
        for feat_idx in 0..nf {
            if count >= limit {
                break;
            }
            if let Some(meta) = patched.feature_meta(layer as usize, feat_idx) {
                // Apply WHERE filters
                if let Some(tf) = token_filter {
                    if !meta.top_token.to_lowercase().contains(&tf.to_lowercase()) {
                        continue;
                    }
                }
                if let Some(ms) = confidence_floor {
                    if meta.c_score < ms {
                        continue;
                    }
                }

                let down_tokens: String = meta
                    .top_k
                    .iter()
                    .take(5)
                    .map(|t| t.token.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");

                out.push(format!(
                    "F{:<7} {:<20} {:>10.4} {:>30}",
                    feat_idx, meta.top_token, meta.c_score, down_tokens
                ));
                count += 1;
            }
        }

        Ok(out)
    }

    pub(crate) fn exec_show_entities(
        &self,
        layer_filter: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<String>, LqlError> {
        let (_path, config, patched) = self.require_vindex()?;
        let limit = limit.unwrap_or(50) as usize;

        let scan_layers: Vec<usize> = if let Some(l) = layer_filter {
            vec![l as usize]
        } else {
            (0..config.num_layers).collect()
        };

        // Collect distinct top_tokens across all scanned features.
        let mut entity_counts: std::collections::HashMap<String, (usize, f32)> =
            std::collections::HashMap::new();

        for layer in &scan_layers {
            let nf = patched.num_features(*layer);
            for feat in 0..nf {
                if let Some(meta) = patched.feature_meta(*layer, feat) {
                    let tok = meta.top_token.trim().to_string();
                    // Filter to named entities: starts with uppercase
                    // ASCII, 3+ chars, all alphabetic. This skips
                    // function words (the, of, in), subword fragments,
                    // and non-Latin scripts that are typically noise
                    // from polysemantic features.
                    if tok.len() < 3 {
                        continue;
                    }
                    let first = tok.chars().next().unwrap_or(' ');
                    if !first.is_ascii_uppercase() {
                        continue;
                    }
                    if !tok.chars().all(|c| c.is_alphabetic()) {
                        continue;
                    }
                    let entry = entity_counts.entry(tok).or_insert((0, 0.0));
                    entry.0 += 1;
                    if meta.c_score > entry.1 {
                        entry.1 = meta.c_score;
                    }
                }
            }
        }

        // Sort by feature count descending.
        let mut entities: Vec<(String, usize, f32)> = entity_counts
            .into_iter()
            .map(|(tok, (count, max_score))| (tok, count, max_score))
            .collect();
        entities.sort_by(|a, b| b.1.cmp(&a.1));
        entities.truncate(limit);

        let mut out = Vec::new();
        let layer_note = if let Some(l) = layer_filter {
            format!(" at layer {l}")
        } else {
            format!(" across {} layers", scan_layers.len())
        };
        out.push(format!("Distinct entities{layer_note} ({} total, showing top {limit}):",
            entities.len().max(limit)));
        out.push(format!(
            "{:<24} {:>10} {:>10}",
            "Entity", "Features", "Max Score"
        ));
        out.push("-".repeat(48));

        for (tok, count, max_score) in &entities {
            out.push(format!(
                "{:<24} {:>10} {:>10.4}",
                tok, count, max_score
            ));
        }

        if entities.is_empty() {
            out.push("  (no entities found)".into());
        }

        Ok(out)
    }

    pub(crate) fn exec_show_tokens(
        &self,
        layer_filter: Option<u32>,
        conditions: &[Condition],
        verbose: bool,
        group_by: Option<TokenGroupBy>,
        order_by: Option<TokenSortBy>,
        limit: Option<u32>,
        export_format: Option<crate::ast::ExportFormat>,
    ) -> Result<Vec<String>, LqlError> {
        let (_path, config, patched) = self.require_vindex()?;

        let scan_layers: Vec<usize> = if let Some(l) = layer_filter {
            vec![l as usize]
        } else {
            (0..config.num_layers).collect()
        };
    let bands = super::query::describe_resolve_bands(config);
    let filters = parse_token_filters(conditions);

    // If export format is specified, collect all data and return in specified format
    if let Some(fmt) = export_format {
        let hits = collect_token_hits(patched, &scan_layers, &bands, &filters);
        return Ok(export_token_summary(hits, fmt, verbose, order_by, limit));
    }

    match group_by {
        Some(TokenGroupBy::Layer) => {
            let mut out = Vec::new();
            let mut rendered = 0usize;
            let mut skipped = 0usize;
            for layer in &scan_layers {
                let hits = collect_token_hits(patched, &[*layer], &bands, &filters);
                if hits.is_empty() {
                    skipped += 1;
                    continue;
                }
                if rendered > 0 {
                    out.push(String::new());
                }
                out.extend(render_token_summary(
                    format!("at layer {}", layer),
                    hits,
                    verbose,
                    order_by,
                    limit,
                ));
                rendered += 1;
            }
            if rendered == 0 {
                return Ok(vec!["  (no tokens found)".into()]);
            }
            if skipped > 0 {
                out.push(String::new());
                out.push(format!("Skipped {} empty layer summaries.", skipped));
            }
            Ok(out)
        }
        Some(TokenGroupBy::Band) => {
            let mut out = Vec::new();
            let mut rendered = 0usize;
            let mut skipped = 0usize;
            let groups = [
                ("syntax", bands.syntax.0, bands.syntax.1),
                ("knowledge", bands.knowledge.0, bands.knowledge.1),
                ("output", bands.output.0, bands.output.1),
            ];
            for (name, start, end) in groups {
                let group_layers: Vec<usize> = scan_layers
                    .iter()
                    .copied()
                    .filter(|l| *l >= start && *l <= end)
                    .collect();
                let hits = collect_token_hits(patched, &group_layers, &bands, &filters);
                if hits.is_empty() {
                    skipped += 1;
                    continue;
                }
                if rendered > 0 {
                    out.push(String::new());
                }
                out.extend(render_token_summary(
                    format!("for {} band (L{}-{})", name, start, end),
                    hits,
                    verbose,
                    order_by,
                    limit,
                ));
                rendered += 1;
            }
            if rendered == 0 {
                return Ok(vec!["  (no tokens found)".into()]);
            }
            if skipped > 0 {
                out.push(String::new());
                out.push(format!("Skipped {} empty band summaries.", skipped));
            }
            Ok(out)
        }
        None => {
            let hits = collect_token_hits(patched, &scan_layers, &bands, &filters);
            let scope = if let Some(l) = layer_filter {
                format!("at layer {}", l)
            } else {
                format!("across {} layers", scan_layers.len())
            };
            Ok(render_token_summary(scope, hits, verbose, order_by, limit))
        }
    }
}

    pub(crate) fn exec_show_models(&self) -> Result<Vec<String>, LqlError> {
        let mut out = Vec::new();
        out.push(format!(
            "{:<35} {:>10} {:>8} {:>12}",
            "Model", "Size", "Layers", "Status"
        ));
        out.push("-".repeat(70));

        let cwd = std::env::current_dir().unwrap_or_default();
        if let Ok(entries) = std::fs::read_dir(&cwd) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let index_json = path.join("index.json");
                    if index_json.exists() {
                        if let Ok(config) = larql_vindex::load_vindex_config(&path) {
                            let size = dir_size(&path);
                            out.push(format!(
                                "{:<35} {:>10} {:>8} {:>12}",
                                path.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy(),
                                format_bytes(size),
                                config.num_layers,
                                "ready",
                            ));
                        }
                    }
                }
            }
        }

        if out.len() == 2 {
            out.push("  (no vindexes found in current directory)".into());
        }

        Ok(out)
    }
}

// `TokenFilters`, `TokenHit`, `ShapeInfo`, `KindInfo` now live in
// `larql_vindex::token_summary`; see `executor::helpers` for the re-exports
// that keep existing `pub(crate)` call sites unchanged.

fn parse_token_filters(conditions: &[Condition]) -> TokenFilters {
    TokenFilters {
        token_filter: conditions
            .iter()
            .find(|c| c.field == "token" || c.field == "entity")
            .and_then(|c| {
                if let Value::String(ref s) = c.value { Some(s.clone()) } else { None }
            }),
        shape_filter: conditions.iter().find(|c| c.field == "shape").and_then(|c| {
            if let Value::String(ref s) = c.value { Some(s.to_lowercase()) } else { None }
        }),
        type_filter: conditions.iter().find(|c| c.field == "type" || c.field == "kind").and_then(|c| {
            if let Value::String(ref s) = c.value { Some(s.to_lowercase()) } else { None }
        }),
        band_filter: conditions.iter().find(|c| c.field == "band").and_then(|c| {
            if let Value::String(ref s) = c.value { Some(s.to_lowercase()) } else { None }
        }),
    }
}

// `band_name_for_layer` and `collect_token_hits` now live in
// `larql_vindex::token_summary`; see `executor::helpers`.

pub(crate) fn export_token_summary(
    token_hits: HashMap<String, TokenHit>,
    format: crate::ast::ExportFormat,
    _verbose: bool,
    order_by: Option<TokenSortBy>,
    limit: Option<u32>,
) -> Vec<String> {
    let mut shapes: HashMap<TokenShape, ShapeInfo> = HashMap::new();

    for (tok, hit) in &token_hits {
        let shape_info = shapes.entry(hit.shape).or_default();
        shape_info.distinct += 1;
        shape_info.feature_hits += hit.hits;
        if hit.kind.is_some() {
            shape_info.entity_like += 1;
        }
        if shape_info.examples.len() < 4 && !tok.is_empty() {
            shape_info.examples.push(tok.clone());
        }
    }

    // Sort examples within each shape for deterministic output
    for shape_info in shapes.values_mut() {
        shape_info.examples.sort();
    }

    let mut shape_rows: Vec<(TokenShape, ShapeInfo)> = shapes.into_iter().collect();
    match order_by {
        Some(TokenSortBy::MaxScore) => {
            let mut shape_max_scores: HashMap<TokenShape, f32> = HashMap::new();
            for hit in token_hits.values() {
                let entry = shape_max_scores.entry(hit.shape).or_insert(0.0);
                *entry = (*entry).max(hit.max_score);
            }
            shape_rows.sort_by(|a, b| {
                let score_a = shape_max_scores.get(&a.0).unwrap_or(&0.0);
                let score_b = shape_max_scores.get(&b.0).unwrap_or(&0.0);
                score_b.partial_cmp(score_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| token_shape_name(a.0).cmp(token_shape_name(b.0)))
            });
        }
        Some(TokenSortBy::Distinct) => {
            shape_rows.sort_by(|a, b| {
                b.1.distinct.cmp(&a.1.distinct)
                    .then_with(|| token_shape_name(a.0).cmp(token_shape_name(b.0)))
            });
        }
        Some(TokenSortBy::EntityLike) => {
            shape_rows.sort_by(|a, b| {
                b.1.entity_like.cmp(&a.1.entity_like)
                    .then_with(|| token_shape_name(a.0).cmp(token_shape_name(b.0)))
            });
        }
        Some(TokenSortBy::Shape) => {
            shape_rows.sort_by(|a, b| token_shape_name(a.0).cmp(token_shape_name(b.0)));
        }
        None => {
            shape_rows.sort_by(|a, b| {
                b.1.feature_hits.cmp(&a.1.feature_hits)
                    .then_with(|| token_shape_name(a.0).cmp(token_shape_name(b.0)))
            });
        }
    }
    if let Some(lim) = limit {
        shape_rows.truncate(lim as usize);
    }

    match format {
        crate::ast::ExportFormat::Csv => {
            let mut out = vec!["shape,distinct,feature_hits,entity_like,examples".to_string()];
            for (shape, info) in shape_rows {
                let examples = info.examples.iter()
                    .map(|e| e.replace(',', "\\,"))
                    .collect::<Vec<_>>()
                    .join(",");
                out.push(format!("{},{},{},{},{}",
                    token_shape_name(shape),
                    info.distinct,
                    info.feature_hits,
                    info.entity_like,
                    examples
                ));
            }
            out
        }
        crate::ast::ExportFormat::Json => {
            let rows: Vec<serde_json::Value> = shape_rows.iter().map(|(shape, info)| {
                serde_json::json!({
                    "shape": token_shape_name(*shape),
                    "distinct": info.distinct,
                    "feature_hits": info.feature_hits,
                    "entity_like": info.entity_like,
                    "examples": info.examples,
                })
            }).collect();
            vec![serde_json::to_string_pretty(&rows).unwrap_or("[]".to_string())]
        }
    }
}

pub(crate) fn render_token_summary(
    scope: String,
    token_hits: HashMap<String, TokenHit>,
    verbose: bool,
    order_by: Option<TokenSortBy>,
    limit: Option<u32>,
) -> Vec<String> {
    let has_tokens = !token_hits.is_empty();
    let mut shapes: HashMap<TokenShape, ShapeInfo> = HashMap::new();
    let mut kinds: HashMap<&'static str, KindInfo> = HashMap::new();

    for (tok, hit) in &token_hits {
        let shape_info = shapes.entry(hit.shape).or_default();
        shape_info.distinct += 1;
        shape_info.feature_hits += hit.hits;
        if hit.kind.is_some() {
            shape_info.entity_like += 1;
        }
        if shape_info.examples.len() < 4 && !tok.is_empty() {
            shape_info.examples.push(tok.clone());
        }

        if let Some(kind_name) = hit.kind {
            let kind_info = kinds.entry(kind_name).or_default();
            kind_info.distinct += 1;
            kind_info.feature_hits += hit.hits;
            if kind_info.examples.len() < 4 && !tok.is_empty() {
                kind_info.examples.push(tok.clone());
            }
        }
    }

    // Sort examples within each shape/kind for deterministic output
    for shape_info in shapes.values_mut() {
        shape_info.examples.sort();
    }
    for kind_info in kinds.values_mut() {
        kind_info.examples.sort();
    }

    let mut out = Vec::new();
    out.push(format!(
        "Token summary {scope} ({} distinct top tokens):",
        format_number(token_hits.len())
    ));
    out.push(format!(
        "{:<12} {:>10} {:>12} {:>12} {:<24}",
        "Shape", "Distinct", "FeatHits", "EntityLike", "Examples"
    ));
    out.push("-".repeat(76));

    let mut shape_rows: Vec<(TokenShape, ShapeInfo)> = shapes.into_iter().collect();
    match order_by {
        Some(TokenSortBy::MaxScore) => {
            // Sort by max_score from token_hits (need to aggregate max per shape)
            let mut shape_max_scores: HashMap<TokenShape, f32> = HashMap::new();
            for hit in token_hits.values() {
                let entry = shape_max_scores.entry(hit.shape).or_insert(0.0);
                *entry = (*entry).max(hit.max_score);
            }
            shape_rows.sort_by(|a, b| {
                let score_a = shape_max_scores.get(&a.0).unwrap_or(&0.0);
                let score_b = shape_max_scores.get(&b.0).unwrap_or(&0.0);
                score_b.partial_cmp(score_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| token_shape_name(a.0).cmp(token_shape_name(b.0)))
            });
        }
        Some(TokenSortBy::Distinct) => {
            shape_rows.sort_by(|a, b| {
                b.1.distinct.cmp(&a.1.distinct)
                    .then_with(|| token_shape_name(a.0).cmp(token_shape_name(b.0)))
            });
        }
        Some(TokenSortBy::EntityLike) => {
            shape_rows.sort_by(|a, b| {
                b.1.entity_like.cmp(&a.1.entity_like)
                    .then_with(|| token_shape_name(a.0).cmp(token_shape_name(b.0)))
            });
        }
        Some(TokenSortBy::Shape) => {
            shape_rows.sort_by(|a, b| token_shape_name(a.0).cmp(token_shape_name(b.0)));
        }
        None => {
            shape_rows.sort_by(|a, b| {
                b.1.feature_hits.cmp(&a.1.feature_hits)
                    .then_with(|| token_shape_name(a.0).cmp(token_shape_name(b.0)))
            });
        }
    }
    if let Some(lim) = limit {
        shape_rows.truncate(lim as usize);
    }
    for (shape, info) in shape_rows {
        out.push(format!(
            "{:<12} {:>10} {:>12} {:>12} {:<24}",
            token_shape_name(shape),
            format_number(info.distinct),
            format_number(info.feature_hits),
            format_number(info.entity_like),
            info.examples.join(", ")
        ));
    }

    if !kinds.is_empty() {
        out.push(String::new());
        out.push("Entity-capable token classes:".into());
        out.push(format!(
            "{:<12} {:>10} {:>12} {:<24}",
            "Type", "Distinct", "FeatHits", "Examples"
        ));
        out.push("-".repeat(62));
        let mut kind_rows: Vec<(&'static str, KindInfo)> = kinds.into_iter().collect();
        kind_rows.sort_by(|a, b| {
            b.1.feature_hits.cmp(&a.1.feature_hits)
                .then_with(|| b.1.distinct.cmp(&a.1.distinct))
                .then_with(|| a.0.cmp(&b.0))
        });
        for (kind, info) in kind_rows {
            out.push(format!(
                "{:<12} {:>10} {:>12} {:<24}",
                kind,
                format_number(info.distinct),
                format_number(info.feature_hits),
                info.examples.join(", ")
            ));
        }
    }

    if verbose && has_tokens {
        out.push(String::new());
        out.push("Top matching tokens:".into());
        out.push(format!(
            "{:<24} {:<12} {:<10} {:<8} {:>10} {:>10}",
            "Token", "Shape", "EntityType", "Bands", "Features", "Max Score"
        ));
        out.push("-".repeat(82));

        let mut tokens: Vec<_> = token_hits.into_iter().collect();
        tokens.sort_by(|a, b| {
            b.1.hits
                .cmp(&a.1.hits)
                .then_with(|| {
                    b.1.max_score
                        .partial_cmp(&a.1.max_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.0.cmp(&b.0))
        });
        for (tok, hit) in tokens.into_iter().take(30) {
            let bands_seen = format!(
                "{}{}{}",
                if hit.syntax_hits > 0 { "S" } else { "-" },
                if hit.knowledge_hits > 0 { "K" } else { "-" },
                if hit.output_hits > 0 { "O" } else { "-" },
            );
            out.push(format!(
                "{:<24} {:<12} {:<10} {:<8} {:>10} {:>10.4}",
                if tok.is_empty() { "∅" } else { tok.as_str() },
                token_shape_name(hit.shape),
                hit.kind.unwrap_or("none"),
                bands_seen,
                hit.hits,
                hit.max_score
            ));
        }
    }

    if !has_tokens {
        out.push("  (no tokens found)".into());
    }

    out
}
