//! Remote executor — forwards LQL queries to a larql-server via HTTP.

use super::Backend;
use super::Session;
use crate::ast::*;
use crate::error::LqlError;

use larql_core::{Client as HttpClient, Response};

fn patch_timestamp_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

fn condition_usize(conditions: &[crate::ast::Condition], field: &str) -> Option<usize> {
    conditions
        .iter()
        .find(|c| c.field == field)
        .and_then(|c| match &c.value {
            crate::ast::Value::Integer(n) => Some(*n as usize),
            _ => None,
        })
}

fn condition_string(conditions: &[crate::ast::Condition], field: &str) -> Option<String> {
    conditions
        .iter()
        .find(|c| c.field == field)
        .and_then(|c| match &c.value {
            crate::ast::Value::String(s) => Some(s.clone()),
            _ => None,
        })
}

fn rows_to_layer_features(rows: &[serde_json::Value]) -> Result<Vec<(usize, usize)>, LqlError> {
    rows.iter()
        .map(|row| {
            let layer = row.get("layer").and_then(|v| v.as_u64()).ok_or_else(|| {
                LqlError::Execution("remote SELECT row missing numeric 'layer'".into())
            })?;
            let feature = row.get("feature").and_then(|v| v.as_u64()).ok_or_else(|| {
                LqlError::Execution("remote SELECT row missing numeric 'feature'".into())
            })?;
            Ok((layer as usize, feature as usize))
        })
        .collect()
}

/// Format `/v1/explain-infer` JSON (standalone or embedded as `followup_infer` on `/v1/describe`).
fn format_explain_infer_json_value(
    result: &serde_json::Value,
    prompt: &str,
    band: Option<LayerBand>,
    relations_only: bool,
    with_attention: bool,
) -> Vec<String> {
    let band_label = match band {
        Some(LayerBand::Syntax) => " (syntax)",
        Some(LayerBand::Knowledge) => " (knowledge)",
        Some(LayerBand::Output) => " (output)",
        _ => "",
    };

    let mut out = Vec::new();
    out.push(format!("Inference trace for {:?}{}:", prompt, band_label));

    if let Some(preds) = result["predictions"].as_array() {
        if let Some(first) = preds.first() {
            let tok = first["token"].as_str().unwrap_or("?");
            let prob = first["probability"].as_f64().unwrap_or(0.0);
            out.push(format!("Prediction: {} ({:.2}%)", tok, prob * 100.0));
        }
    }
    out.push(String::new());

    if let Some(layers) = result["trace"].as_array() {
        for layer_obj in layers {
            let layer = layer_obj["layer"].as_u64().unwrap_or(0);
            let features = layer_obj["features"].as_array();

            if with_attention {
                let feat = features.and_then(|f| f.first());
                let feature_str = if let Some(feat) = feat {
                    let relation = feat["relation"]
                        .as_str()
                        .or_else(|| feat["relation"].as_null().map(|_| ""))
                        .unwrap_or("");
                    if relations_only && relation.is_empty() {
                        None
                    } else {
                        let gate = feat["gate_score"].as_f64().unwrap_or(0.0);
                        let top_token = feat["top_token"].as_str().unwrap_or("?");
                        let name = if !relation.is_empty() {
                            relation
                        } else {
                            top_token
                        };
                        Some(format!("{:<14} {:+.1}", name, gate))
                    }
                } else {
                    None
                };
                let empty = format!("{:19}", "");
                let feature_part = feature_str.as_deref().unwrap_or(&empty);

                let attn_part = layer_obj
                    .get("attention")
                    .and_then(|a| a.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|v| {
                        let tok = v["token"].as_str()?;
                        let w = v["weight"].as_f64()?;
                        Some(format!("{}({:.0}%)", tok, w * 100.0))
                    })
                    .unwrap_or_default();

                let lens_part = layer_obj
                    .get("lens")
                    .and_then(|l| {
                        let tok = l["token"].as_str()?;
                        let prob = l["probability"].as_f64()?;
                        Some(format!("{} ({:.1}%)", tok, prob * 100.0))
                    })
                    .unwrap_or_default();

                if feature_str.is_some() || !lens_part.is_empty() {
                    out.push(format!(
                        "  L{:2}  {:<19}  {:<16} → {}",
                        layer, feature_part, attn_part, lens_part,
                    ));
                }
            } else if let Some(features) = features {
                for feat in features {
                    let feature = feat["feature"].as_u64().unwrap_or(0);
                    let gate = feat["gate_score"].as_f64().unwrap_or(0.0);
                    let relation = feat["relation"]
                        .as_str()
                        .or_else(|| feat["relation"].as_null().map(|_| ""))
                        .unwrap_or("");
                    if relations_only && relation.is_empty() {
                        continue;
                    }
                    let label_str = if relation.is_empty() {
                        format!("{:14}", "")
                    } else {
                        format!("{:<14}", relation)
                    };
                    let top_token = feat["top_token"].as_str().unwrap_or("?");
                    let top_tokens: String = feat["top_tokens"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_default();
                    out.push(format!(
                        "  L{:2}: {} F{:<5} gate={:+.1}  → {:15} [{}]",
                        layer, label_str, feature, gate, top_token, top_tokens,
                    ));
                }
            }
        }
    }

    if let Some(ms) = result["latency_ms"].as_f64() {
        out.push(format!("\n{:.0}ms (remote)", ms));
    }

    out
}

impl Session {
    /// Connect to a remote larql-server.
    pub(crate) fn exec_use_remote(&mut self, url: &str) -> Result<Vec<String>, LqlError> {
        let url = url.trim_end_matches('/').to_string();

        let client = HttpClient::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| LqlError::exec("failed to create HTTP client", e))?;

        // Verify the server is reachable by hitting /v1/stats.
        let stats_url = format!("{url}/v1/stats");
        let resp = client
            .get(&stats_url)
            .send()
            .map_err(|e| LqlError::exec("failed to connect to {url}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp
                .text()
                .map_err(|e| LqlError::exec("failed to read error response", e))?;
            return Err(LqlError::Execution(format!(
                "server returned {}: {}",
                status, text
            )));
        }

        let stats: serde_json::Value = resp
            .json()
            .map_err(|e| LqlError::exec("invalid response from server", e))?;

        let model = stats["model"].as_str().unwrap_or("unknown");
        let layers = stats["layers"].as_u64().unwrap_or(0);
        let features = stats["features"].as_u64().unwrap_or(0);

        // Generate a unique session ID for this connection.
        let mut hasher = std::collections::hash_map::DefaultHasher::default();
        std::hash::Hash::hash(&std::process::id(), &mut hasher);
        std::hash::Hash::hash(&std::time::SystemTime::now(), &mut hasher);
        let random_bytes = std::hash::Hasher::finish(&hasher).to_le_bytes();
        let random_suffix = u64::from_le_bytes(random_bytes);
        let session_id = format!(
            "larql-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            random_suffix
        );

        self.backend = Backend::Remote {
            url: url.clone(),
            client,
            local_patches: Vec::new(),
            session_id,
        };
        self.patch_recording = None;
        self.auto_patch = false;

        Ok(vec![format!(
            "Connected: {} ({} layers, {} features)\n  Remote: {}",
            model, layers, features, url,
        )])
    }

    /// Check if the backend is remote.
    pub(crate) fn is_remote(&self) -> bool {
        matches!(&self.backend, Backend::Remote { .. })
    }

    /// Get the remote URL, client, and session ID, or error.
    fn require_remote(&self) -> Result<(&str, &HttpClient, &str), LqlError> {
        match &self.backend {
            Backend::Remote {
                url,
                client,
                session_id,
                ..
            } => Ok((url, client, session_id)),
            _ => Err(LqlError::Execution(
                "not connected to a remote server".into(),
            )),
        }
    }

    // ── Generic HTTP forwarding helpers ──
    //
    // Every `remote_*` method ends up doing the same `build URL → send →
    // status check → parse JSON` dance. These helpers consolidate the
    // pattern so the per-statement methods only have to assemble the
    // request shape and process the response body.

    /// Helper to convert Vec<(String, String)> to Vec<(&str, &str)> for query parameters
    fn to_query_ref(params: &[(String, String)]) -> Vec<(&str, &str)> {
        params
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect()
    }

    /// GET `{remote_url}{endpoint}` with optional query parameters,
    /// check the response status, and parse the body as JSON.
    fn remote_get_json(
        &self,
        endpoint: &str,
        query: &[(&str, &str)],
    ) -> Result<serde_json::Value, LqlError> {
        let (url, client, _sid) = self.require_remote()?;
        let resp = client
            .get(format!("{url}{endpoint}"))
            .query(query)
            .send()
            .map_err(|e| LqlError::exec("request failed", e))?;
        Self::check_and_parse(endpoint, resp)
    }

    /// POST `{remote_url}{endpoint}` with a JSON body, check status,
    /// and parse the response. When `with_session` is true, the
    /// `x-session-id` header is added (server-side patch sessions).
    fn remote_post_json(
        &self,
        endpoint: &str,
        body: &serde_json::Value,
        with_session: bool,
    ) -> Result<serde_json::Value, LqlError> {
        let (url, client, sid) = self.require_remote()?;
        let mut req = client.post(format!("{url}{endpoint}")).json(body);
        if with_session {
            req = req.header("x-session-id", sid);
        }
        let resp = req
            .send()
            .map_err(|e| LqlError::exec("request failed", e))?;
        Self::check_and_parse(endpoint, resp)
    }

    /// Validate the response status, parse the body as JSON, and turn
    /// any failure into a tagged `LqlError`.
    fn check_and_parse(endpoint: &str, resp: Response) -> Result<serde_json::Value, LqlError> {
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp
                .text()
                .map_err(|e| LqlError::exec("failed to read error response", e))?;
            return Err(LqlError::Execution(format!(
                "{endpoint} failed ({status}): {text}"
            )));
        }
        resp.json::<serde_json::Value>()
            .map_err(|e| LqlError::exec("invalid response", e))
    }

    // ── Remote query forwarding ──

    pub(crate) fn remote_describe(
        &self,
        entity: &str,
        band: Option<LayerBand>,
        layer: Option<u32>,
        relations_only: bool,
        mode: crate::ast::DescribeMode,
    ) -> Result<Vec<String>, LqlError> {
        let verbose = mode == crate::ast::DescribeMode::Verbose;
        let show_also = matches!(
            mode,
            crate::ast::DescribeMode::Verbose | crate::ast::DescribeMode::Raw
        );

        // Match local `exec_describe`: default band is all layers (`None` → "all" on the wire).
        let band_str = match band {
            Some(LayerBand::Syntax) => "syntax",
            Some(LayerBand::Knowledge) => "knowledge",
            Some(LayerBand::Output) => "output",
            Some(LayerBand::All) => "all",
            None => "all",
        };

        let verbose_s = if verbose { "true" } else { "false" };
        let mode_s = match mode {
            crate::ast::DescribeMode::Verbose => "verbose",
            crate::ast::DescribeMode::Brief => "brief",
            crate::ast::DescribeMode::Raw => "raw",
        };
        let layer_s = layer.map(|l| l.to_string());
        let mut q: Vec<(String, String)> = vec![
            ("entity".into(), entity.to_string()),
            ("band".into(), band_str.to_string()),
            ("verbose".into(), verbose_s.to_string()),
            ("mode".into(), mode_s.to_string()),
        ];
        if let Some(ref ls) = layer_s {
            q.push(("layer".into(), ls.clone()));
        }
        if relations_only {
            q.push(("relations_only".into(), "true".into()));
        }
        let qref = Self::to_query_ref(&q);

        let body = self.remote_get_json("/v1/describe", &qref)?;

        let mut out = vec![entity.to_string()];

        if let Some(edges) = body["edges"].as_array() {
            if edges.is_empty() {
                out.push("  (no edges found)".into());
                if let Some(note) = body["followup_note"].as_str() {
                    out.push(String::new());
                    for line in note.lines() {
                        if !line.is_empty() {
                            out.push(format!("  {line}"));
                        }
                    }
                }
                if let Some(arr) = body["followup_walk"].as_array() {
                    if !arr.is_empty() {
                        out.push(String::new());
                        out.push("  EXPLAIN WALK —".into());
                        for v in arr {
                            if let Some(s) = v.as_str() {
                                out.push(format!("    {s}"));
                            }
                        }
                    }
                }
                if body.get("followup_infer").is_some() && !body["followup_infer"].is_null() {
                    out.push(String::new());
                    out.push("  EXPLAIN INFER —".into());
                    let infer_lines = format_explain_infer_json_value(
                        &body["followup_infer"],
                        entity,
                        band,
                        false,
                        false,
                    );
                    for line in infer_lines {
                        out.push(format!("    {line}"));
                    }
                } else if let Some(err) = body["followup_infer_error"].as_str() {
                    out.push(String::new());
                    out.push(format!("  EXPLAIN INFER — skipped ({err})"));
                }
            } else {
                for edge in edges {
                    let target = edge["target"].as_str().unwrap_or("?");
                    let gate = edge["gate_score"].as_f64().unwrap_or(0.0);
                    let layer = edge["layer"].as_u64().unwrap_or(0);
                    let relation = edge["relation"].as_str().unwrap_or("");
                    let source = edge["source"].as_str().unwrap_or("");

                    let show_labels = mode != crate::ast::DescribeMode::Raw;
                    let label = if show_labels && !relation.is_empty() {
                        format!("{:<12}", relation)
                    } else {
                        format!("{:<12}", "")
                    };

                    let tag = if show_labels && source == "probe" {
                        "  (probe)"
                    } else {
                        ""
                    };

                    let also_str = if show_also {
                        edge["also"]
                            .as_array()
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .filter(|s| !s.is_empty())
                            .map(|s| format!("  also: {s}"))
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };

                    out.push(format!(
                        "    {} → {:20} {:>7.1}  L{:<3}{}{}",
                        label, target, gate, layer, tag, also_str,
                    ));
                }
            }
        }

        if let Some(ms) = body["latency_ms"].as_f64() {
            out.push(format!("\n{:.1}ms (remote)", ms));
        }

        // Overlay local patch edges.
        if let Backend::Remote { local_patches, .. } = &self.backend {
            if !local_patches.is_empty() {
                let entity_lower = entity.to_lowercase();
                let mut local_edges = Vec::new();
                for patch in local_patches {
                    for op in &patch.operations {
                        if let larql_vindex::PatchOp::Insert {
                            entity: ent,
                            target,
                            relation,
                            layer,
                            confidence,
                            ..
                        } = op
                        {
                            // Cache lowercase comparison to avoid repeated allocations
                            let ent_lower = ent.to_lowercase();
                            if ent_lower == entity_lower {
                                local_edges.push((
                                    relation.as_deref().unwrap_or(""),
                                    target.as_str(),
                                    *layer,
                                    confidence.unwrap_or(0.9),
                                ));
                            }
                        }
                    }
                }
                if !local_edges.is_empty() {
                    out.push("  Local patch edges:".into());
                    for (relation, target, layer, conf) in &local_edges {
                        let label = if relation.is_empty() {
                            format!("{:<12}", "")
                        } else {
                            format!("{:<12}", relation)
                        };
                        out.push(format!(
                            "    {} → {:20} {:>7.2}  L{:<3}  (local)",
                            label, target, conf, layer,
                        ));
                    }
                }
            }
        }

        Ok(out)
    }

    pub(crate) fn remote_walk(
        &self,
        prompt: &str,
        top: Option<u32>,
        layers: Option<&Range>,
        mode: Option<WalkMode>,
        compare: bool,
    ) -> Result<Vec<String>, LqlError> {
        let top_k = top.unwrap_or(10).to_string();
        let layers_str = layers.map(|r| format!("{}-{}", r.start, r.end));
        let mode_s = mode.map(|m| match m {
            WalkMode::Hybrid => "hybrid",
            WalkMode::Pure => "pure",
            WalkMode::Dense => "dense",
        });
        let compare_s = if compare { "true" } else { "false" };

        let mut params: Vec<(&str, &str)> = vec![
            ("prompt", prompt),
            ("top", top_k.as_str()),
            ("compare", compare_s),
        ];
        if let Some(ref s) = layers_str {
            params.push(("layers", s.as_str()));
        }
        if let Some(ref s) = mode_s {
            params.push(("mode", s));
        }

        let body = self.remote_get_json("/v1/walk", &params)?;

        let token = body["token"].as_str().unwrap_or("?");
        let layers_count = body["layers_count"].as_u64().unwrap_or(0);
        let mode_name = body["mode"]
            .as_str()
            .map(|m| match m {
                "pure" => "pure (sparse KNN only)",
                "dense" => "dense (full matmul)",
                _ => "hybrid (default)",
            })
            .unwrap_or("hybrid (default)");

        let mut out = Vec::new();
        out.push(format!(
            "Feature scan for {:?} (token {:?}, {} layers, mode={})",
            prompt, token, layers_count, mode_name,
        ));
        out.push(String::new());

        if let Some(hits) = body["hits"].as_array() {
            let mut current_layer = None;
            let mut layer_hits = 0;
            let max_per_layer = if compare { 5 } else { 3 };

            for hit in hits {
                let layer = hit["layer"].as_u64().unwrap_or(0);
                if current_layer != Some(layer) {
                    current_layer = Some(layer);
                    layer_hits = 0;
                }

                if layer_hits >= max_per_layer {
                    continue;
                }
                layer_hits += 1;

                let feature = hit["feature"].as_u64().unwrap_or(0);
                let gate = hit["gate_score"].as_f64().unwrap_or(0.0);
                let target = hit["target"].as_str().unwrap_or("?");
                let down_tokens: String = hit["down"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();

                out.push(format!(
                    "  L{:2}: F{:<5} gate={:+.1}  top={:15}  down=[{}]",
                    layer,
                    feature,
                    gate,
                    format!("{:?}", target),
                    down_tokens
                ));
            }
        }

        if let Some(ms) = body["latency_ms"].as_f64() {
            out.push(format!("\n{:.1}ms (remote)", ms));
        }

        if compare {
            out.push(String::new());
            out.push(
                "Note: COMPARE shows more features per layer. For inference use INFER.".into(),
            );
        } else {
            out.push(String::new());
            out.push("Note: pure vindex scan (no attention). For inference use INFER.".into());
        }

        Ok(out)
    }

    pub(crate) fn remote_infer(
        &self,
        prompt: &str,
        top: Option<u32>,
        compare: bool,
    ) -> Result<Vec<String>, LqlError> {
        let mode = if compare { "compare" } else { "walk" };
        let request = serde_json::json!({
            "prompt": prompt,
            "top": top.unwrap_or(5),
            "mode": mode,
        });

        let result = self.remote_post_json("/v1/infer", &request, true)?;

        let mut out = Vec::new();

        if compare {
            for mode in &["walk", "dense"] {
                if let Some(preds) = result[mode].as_array() {
                    out.push(format!("Predictions ({mode}):"));
                    for (i, p) in preds.iter().enumerate() {
                        let tok = p["token"].as_str().unwrap_or("?");
                        let prob = p["probability"].as_f64().unwrap_or(0.0);
                        out.push(format!("  {:2}. {:20} ({:.2}%)", i + 1, tok, prob * 100.0));
                    }
                    if let Some(ms) = result[format!("{mode}_ms")].as_f64() {
                        out.push(format!("  {:.0}ms", ms));
                    }
                    out.push(String::new());
                }
            }
        } else if let Some(preds) = result["predictions"].as_array() {
            out.push("Predictions (walk FFN):".into());
            for (i, p) in preds.iter().enumerate() {
                let tok = p["token"].as_str().unwrap_or("?");
                let prob = p["probability"].as_f64().unwrap_or(0.0);
                out.push(format!("  {:2}. {:20} ({:.2}%)", i + 1, tok, prob * 100.0));
            }
        }

        if let Some(ms) = result["latency_ms"].as_f64() {
            out.push(format!("{:.0}ms (remote)", ms));
        }

        Ok(out)
    }

    pub(crate) fn remote_explain_infer(
        &self,
        prompt: &str,
        top: Option<u32>,
        band: Option<LayerBand>,
        relations_only: bool,
        with_attention: bool,
    ) -> Result<Vec<String>, LqlError> {
        let per_layer = top.unwrap_or(3);
        let band_str = match band {
            Some(LayerBand::Syntax) => "syntax",
            Some(LayerBand::Knowledge) => "knowledge",
            Some(LayerBand::Output) => "output",
            Some(LayerBand::All) => "all",
            None => "all",
        };

        let request = serde_json::json!({
            "prompt": prompt,
            "top": top.unwrap_or(5),
            "per_layer": per_layer,
            "band": band_str,
            "relations_only": relations_only,
            "with_attention": with_attention,
        });

        let result = self.remote_post_json("/v1/explain-infer", &request, false)?;
        Ok(format_explain_infer_json_value(
            &result,
            prompt,
            band,
            relations_only,
            with_attention,
        ))
    }

    pub(crate) fn remote_stats(&self) -> Result<Vec<String>, LqlError> {
        let body = self.remote_get_json("/v1/stats", &[])?;
        let url = match &self.backend {
            Backend::Remote { url, .. } => url.as_str(),
            _ => "?",
        };

        let mut out = Vec::new();
        out.push(format!("Model: {}", body["model"].as_str().unwrap_or("?")));
        out.push(format!(
            "Family: {}",
            body["family"].as_str().unwrap_or("?")
        ));
        out.push(format!("Layers: {}", body["layers"].as_u64().unwrap_or(0)));
        out.push(format!(
            "Features: {}",
            body["features"].as_u64().unwrap_or(0)
        ));
        out.push(format!(
            "Hidden: {}",
            body["hidden_size"].as_u64().unwrap_or(0)
        ));
        out.push(format!("Dtype: {}", body["dtype"].as_str().unwrap_or("?")));
        out.push(format!(
            "Extract level: {}",
            body["extract_level"].as_str().unwrap_or("?")
        ));

        if let Some(bands) = body.get("layer_bands") {
            if let (Some(s), Some(k), Some(o)) = (
                bands.get("syntax"),
                bands.get("knowledge"),
                bands.get("output"),
            ) {
                out.push(format!(
                    "Bands: syntax {}-{}, knowledge {}-{}, output {}-{}",
                    s[0], s[1], k[0], k[1], o[0], o[1]
                ));
            }
        }

        if let Some(loaded) = body.get("loaded") {
            out.push(format!(
                "Loaded: browse={}, inference={}",
                loaded["browse"].as_bool().unwrap_or(false),
                loaded["inference"].as_bool().unwrap_or(false),
            ));
        }

        out.push(format!("Remote: {url}"));

        Ok(out)
    }

    pub(crate) fn remote_show_relations(
        &self,
        mode: crate::ast::DescribeMode,
        with_examples: bool,
    ) -> Result<Vec<String>, LqlError> {
        use crate::ast::DescribeMode;
        let body = self.remote_get_json("/v1/relations", &[])?;

        let mut out = Vec::new();

        // Probe-confirmed relations (skip for Raw mode)
        if mode != DescribeMode::Raw {
            if let Some(probes) = body["probe_relations"].as_array() {
                if !probes.is_empty() {
                    let probe_count = body["probe_count"].as_u64().unwrap_or(0);
                    out.push(format!("Probe-confirmed relations ({probe_count} labels):"));
                    out.push(format!("{:<25} {:>8}", "Relation", "Features"));
                    out.push("-".repeat(35));
                    for rel in probes {
                        let name = rel["name"].as_str().unwrap_or("?");
                        let count = rel["count"].as_u64().unwrap_or(0);
                        out.push(format!("{:<25} {:>8}", name, count));
                    }
                    out.push(String::new());
                }
            }
        }

        // Raw token relations (show for Verbose, Raw, or when no probes)
        let show_raw = mode == DescribeMode::Raw || mode == DescribeMode::Verbose || out.is_empty();

        if show_raw {
            if let Some(rels) = body["relations"].as_array() {
                if !rels.is_empty() {
                    out.push("Top output tokens:".to_string());
                    out.push(format!(
                        "{:<25} {:>8} {:>8} {:>10}",
                        "Token", "Count", "Score", "Layers"
                    ));
                    out.push("-".repeat(55));
                    for rel in rels {
                        let name = rel["name"].as_str().unwrap_or("?");
                        let count = rel["count"].as_u64().unwrap_or(0);
                        let score = rel["max_score"].as_f64().unwrap_or(0.0);
                        let min_l = rel["min_layer"].as_u64().unwrap_or(0);
                        let max_l = rel["max_layer"].as_u64().unwrap_or(0);
                        let examples_str = if with_examples {
                            if let Some(arr) = rel["examples"].as_array() {
                                let ex: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
                                if ex.is_empty() {
                                    String::new()
                                } else {
                                    format!("  e.g. {}", ex.join(", "))
                                }
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        };
                        out.push(format!(
                            "{:<25} {:>8} {:>8.2} {:>5}-{}{}",
                            name, count, score, min_l, max_l, examples_str,
                        ));
                    }
                }
            }
        }

        Ok(out)
    }

    // ── Remote mutations (forwarded to server as patches) ──

    pub(crate) fn remote_insert(
        &self,
        entity: &str,
        relation: &str,
        target: &str,
        layer: Option<u32>,
        confidence: Option<f32>,
        alpha: Option<f32>,
    ) -> Result<Vec<String>, LqlError> {
        let request = serde_json::json!({
            "entity": entity,
            "relation": relation,
            "target": target,
            "layer": layer,
            "confidence": confidence.unwrap_or(0.9),
            "alpha": alpha.unwrap_or(0.25),
        });

        let result = self.remote_post_json("/v1/insert", &request, true)?;

        let inserted = result["inserted"].as_u64().unwrap_or(0);
        let mode = result["mode"].as_str().unwrap_or("unknown");
        let ms = result["latency_ms"].as_f64().unwrap_or(0.0);

        let mut out = Vec::new();
        out.push(format!(
            "Inserted: {} —[{}]→ {} ({} layers, mode: {})",
            entity, relation, target, inserted, mode,
        ));
        out.push(format!("{:.0}ms (remote)", ms));

        Ok(out)
    }

    pub(crate) fn remote_delete(
        &self,
        conditions: &[crate::ast::Condition],
    ) -> Result<Vec<String>, LqlError> {
        let mut ops = Vec::new();
        let layer = condition_usize(conditions, "layer");
        let feature = condition_usize(conditions, "feature");

        if let (Some(layer), Some(feature)) = (layer, feature) {
            ops.push(larql_vindex::PatchOp::Delete {
                layer,
                feature,
                reason: Some("remote DELETE".into()),
            });
        } else {
            let entity = condition_string(conditions, "entity").ok_or_else(|| {
                LqlError::Execution("DELETE requires either layer+feature or entity".into())
            })?;
            let mut body = serde_json::Map::new();
            body.insert("entity".into(), serde_json::json!(entity));
            body.insert("limit".into(), serde_json::json!(10000));
            body.insert("fields".into(), serde_json::json!(["layer", "feature"]));
            if let Some(layer) = layer {
                body.insert("layer".into(), serde_json::json!(layer));
            }

            let result =
                self.remote_post_json("/v1/select", &serde_json::Value::Object(body), true)?;
            let rows = result["rows"].as_array().cloned().unwrap_or_default();
            let matches = rows_to_layer_features(&rows)?;
            if matches.is_empty() {
                return Ok(vec!["  (no matching features found)".into()]);
            }
            for (layer, feature) in &matches {
                ops.push(larql_vindex::PatchOp::Delete {
                    layer: *layer,
                    feature: *feature,
                    reason: Some("remote DELETE".into()),
                });
            }
        }

        // Use sensible defaults for patch metadata since remote patches
        // don't have a local base model reference
        let now = patch_timestamp_now();
        let patch = larql_vindex::VindexPatch {
            version: 1,
            base_model: "remote".to_string(),
            base_checksum: None,
            created_at: now,
            description: Some(format!("DELETE {} feature(s)", ops.len())),
            author: None,
            tags: vec![],
            operations: ops,
        };

        let _result = self.remote_post_json(
            "/v1/patches/apply",
            &serde_json::json!({"patch": patch}),
            false,
        )?;

        Ok(vec![format!(
            "Deleted: {} feature(s) → remote server",
            patch.operations.len()
        )])
    }

    pub(crate) fn remote_update(
        &self,
        set: &[crate::ast::Assignment],
        conditions: &[crate::ast::Condition],
    ) -> Result<Vec<String>, LqlError> {
        let layer = condition_usize(conditions, "layer");
        let feature = condition_usize(conditions, "feature");

        // Build down_meta from SET assignments.
        let target = set
            .iter()
            .find(|a| a.field == "target" || a.field == "top_token")
            .and_then(|a| match &a.value {
                crate::ast::Value::String(s) => Some(s.clone()),
                _ => None,
            });
        let confidence = set
            .iter()
            .find(|a| a.field == "confidence" || a.field == "c_score")
            .and_then(|a| match &a.value {
                crate::ast::Value::Number(n) => Some(*n as f32),
                crate::ast::Value::Integer(n) => Some(*n as f32),
                _ => None,
            });

        let down_meta = target
            .as_ref()
            .map(|t| larql_vindex::patch::core::PatchDownMeta {
                top_token: t.clone(),
                top_token_id: 0,
                c_score: confidence.unwrap_or(0.9),
            });

        let mut ops = Vec::new();
        if let (Some(layer), Some(feature)) = (layer, feature) {
            ops.push(larql_vindex::PatchOp::Update {
                layer,
                feature,
                gate_vector_b64: None,
                down_meta: down_meta.clone(),
            });
        } else {
            let entity = condition_string(conditions, "entity").ok_or_else(|| {
                LqlError::Execution("UPDATE requires either layer+feature or entity".into())
            })?;
            let mut body = serde_json::Map::new();
            body.insert("entity".into(), serde_json::json!(entity));
            body.insert("limit".into(), serde_json::json!(10000));
            body.insert("fields".into(), serde_json::json!(["layer", "feature"]));
            if let Some(layer) = layer {
                body.insert("layer".into(), serde_json::json!(layer));
            }

            let result =
                self.remote_post_json("/v1/select", &serde_json::Value::Object(body), true)?;
            let rows = result["rows"].as_array().cloned().unwrap_or_default();
            let matches = rows_to_layer_features(&rows)?;
            if matches.is_empty() {
                return Ok(vec!["  (no matching features found)".into()]);
            }
            for (layer, feature) in matches {
                ops.push(larql_vindex::PatchOp::Update {
                    layer,
                    feature,
                    gate_vector_b64: None,
                    down_meta: down_meta.clone(),
                });
            }
        }

        // Use sensible defaults for patch metadata since remote patches
        // don't have a local base model reference
        let now = patch_timestamp_now();
        let patch = larql_vindex::VindexPatch {
            version: 1,
            base_model: "remote".to_string(),
            base_checksum: None,
            created_at: now,
            description: Some(format!("UPDATE {} feature(s)", ops.len())),
            author: None,
            tags: vec![],
            operations: ops,
        };

        let _result = self.remote_post_json(
            "/v1/patches/apply",
            &serde_json::json!({"patch": patch}),
            false,
        )?;

        let desc = target
            .as_deref()
            .map(|t| format!(" target={t}"))
            .unwrap_or_default();
        Ok(vec![format!(
            "Updated: {} feature(s){desc} → remote server",
            patch.operations.len()
        )])
    }

    // ── Remote SELECT ──

    pub(crate) fn remote_select(
        &self,
        conditions: &[crate::ast::Condition],
        limit: Option<u32>,
        fields: &[String],
        nearest: Option<&crate::ast::NearestClause>,
        order: Option<&crate::ast::OrderBy>,
    ) -> Result<Vec<String>, LqlError> {
        let mut body = serde_json::Map::new();
        body.insert("limit".into(), serde_json::json!(limit.unwrap_or(20)));

        // Add order support
        if let Some(o) = order {
            body.insert(
                "order".into(),
                serde_json::json!(if o.descending { "desc" } else { "asc" }),
            );
            body.insert("order_by".into(), serde_json::json!(&o.field));
        }

        // Add fields support
        if !fields.is_empty() {
            body.insert("fields".into(), serde_json::json!(fields));
        }

        // Add nearest support
        if let Some(n) = nearest {
            body.insert(
                "nearest".into(),
                serde_json::json!({
                    "entity": n.entity,
                    "layer": n.layer,
                }),
            );
        }

        // Track unknown fields for warning
        let supported_fields = ["entity", "relation", "layer", "confidence", "c_score"];
        let mut unknown_fields = Vec::new();

        for cond in conditions {
            match cond.field.as_str() {
                "entity" => {
                    if let crate::ast::Value::String(s) = &cond.value {
                        body.insert("entity".into(), serde_json::json!(s));
                    }
                }
                "relation" => {
                    if let crate::ast::Value::String(s) = &cond.value {
                        body.insert("relation".into(), serde_json::json!(s));
                    }
                }
                "layer" => {
                    if let crate::ast::Value::Integer(n) = &cond.value {
                        body.insert("layer".into(), serde_json::json!(n));
                    }
                }
                "confidence" | "c_score" => match &cond.value {
                    crate::ast::Value::Number(n) => {
                        body.insert("confidence_floor".into(), serde_json::json!(n));
                    }
                    crate::ast::Value::Integer(n) => {
                        body.insert("confidence_floor".into(), serde_json::json!(n));
                    }
                    _ => {}
                },
                _ => {
                    if !supported_fields.contains(&cond.field.as_str()) {
                        unknown_fields.push(cond.field.clone());
                    }
                }
            }
        }

        // Warn about unknown fields
        if !unknown_fields.is_empty() {
            eprintln!(
                "Warning: SELECT ignored unknown condition fields: {}",
                unknown_fields.join(", ")
            );
        }

        let result =
            self.remote_post_json("/v1/select", &serde_json::Value::Object(body), false)?;

        let mut out = Vec::new();

        if let Some(edges) = result["edges"].as_array() {
            if edges.is_empty() {
                out.push("  (no matching edges)".into());
            } else {
                out.push(format!(
                    "  {:<20} {:<15} {:>6}  {:<6} {}",
                    "Target", "Relation", "Score", "Layer", "Feature"
                ));
                out.push(format!("  {}", "-".repeat(65)));
                for edge in edges {
                    let layer = edge["layer"].as_u64().unwrap_or(0);
                    let feature = edge["feature"].as_u64().unwrap_or(0);
                    let target = edge["target"].as_str().unwrap_or("?");
                    let score = edge["c_score"].as_f64().unwrap_or(0.0);
                    let relation = edge["relation"].as_str().unwrap_or("");
                    out.push(format!(
                        "  {:<20} {:<15} {:>6.3}  L{:<5} F{}",
                        target, relation, score, layer, feature
                    ));
                }
            }
        }

        if let Some(total) = result["total"].as_u64() {
            out.push(format!("\n{} total", total));
        }

        Ok(out)
    }

    // ── Local patch management (client-side overlay) ──

    pub(crate) fn remote_apply_local_patch(&mut self, path: &str) -> Result<Vec<String>, LqlError> {
        let patch_path = std::path::PathBuf::from(path);
        if !patch_path.exists() {
            return Err(LqlError::Execution(format!("patch not found: {path}")));
        }

        let patch = larql_vindex::VindexPatch::load(&patch_path)
            .map_err(|e| LqlError::exec("failed to load patch", e))?;

        let (ins, upd, del) = patch.counts();
        let total = patch.len();

        match &mut self.backend {
            Backend::Remote { local_patches, .. } => {
                local_patches.push(patch);
                Ok(vec![format!(
                    "Applied locally: {path} ({total} ops: {ins} ins, {upd} upd, {del} del)\n\
                     Patch stays client-side — server never sees it."
                )])
            }
            _ => Err(LqlError::Execution(
                "not connected to a remote server".into(),
            )),
        }
    }

    pub(crate) fn remote_show_patches(&self) -> Result<Vec<String>, LqlError> {
        let local_patches = match &self.backend {
            Backend::Remote { local_patches, .. } => local_patches,
            _ => {
                return Err(LqlError::Execution(
                    "not connected to a remote server".into(),
                ))
            }
        };

        let mut out = Vec::new();
        if local_patches.is_empty() {
            out.push("  (no local patches)".into());
        } else {
            out.push("Local patches (client-side only):".into());
            for p in local_patches {
                let (ins, upd, del) = p.counts();
                let desc = p.description.as_deref().unwrap_or("unnamed");
                out.push(format!(
                    "  - {} (ins={}, upd={}, del={})",
                    desc, ins, upd, del
                ));
            }
        }
        Ok(out)
    }

    pub(crate) fn remote_show_tokens(
        &self,
        layer: Option<u32>,
        conditions: &[crate::ast::Condition],
        verbose: bool,
        group_by: Option<crate::ast::TokenGroupBy>,
        order_by: Option<crate::ast::TokenSortBy>,
        limit: Option<u32>,
        export_format: Option<crate::ast::ExportFormat>,
    ) -> Result<Vec<String>, LqlError> {
        use larql_vindex::token_summary::{TokenHit, TokenShape};
        use std::collections::HashMap;

        // ── Build query params ──
        let layer_s = layer.map(|l| l.to_string());
        let group_s = match group_by {
            Some(crate::ast::TokenGroupBy::Layer) => Some("layer"),
            Some(crate::ast::TokenGroupBy::Band) => Some("band"),
            None => None,
        };

        // Condition extractors mirror `parse_token_filters` in the local path.
        fn cond_string<'a>(
            conds: &'a [crate::ast::Condition],
            fields: &[&str],
            lower: bool,
        ) -> Option<String> {
            for c in conds {
                if fields.contains(&c.field.as_str()) {
                    if let crate::ast::Value::String(ref s) = c.value {
                        return Some(if lower { s.to_lowercase() } else { s.clone() });
                    }
                }
            }
            None
        }
        let token_filter = cond_string(conditions, &["token", "entity"], false);
        let shape_filter = cond_string(conditions, &["shape"], true);
        let type_filter = cond_string(conditions, &["type", "kind"], true);
        let band_filter = cond_string(conditions, &["band"], true);

        let mut q: Vec<(String, String)> = Vec::new();
        if let Some(ref ls) = layer_s {
            q.push(("layer".into(), ls.clone()));
        }
        if let Some(gs) = group_s {
            q.push(("group_by".into(), gs.into()));
        }
        if let Some(ref v) = token_filter {
            q.push(("token_filter".into(), v.clone()));
        }
        if let Some(ref v) = shape_filter {
            q.push(("shape_filter".into(), v.clone()));
        }
        if let Some(ref v) = type_filter {
            q.push(("type_filter".into(), v.clone()));
        }
        if let Some(ref v) = band_filter {
            q.push(("band_filter".into(), v.clone()));
        }
        let qref = Self::to_query_ref(&q);

        let body = self.remote_get_json("/v1/tokens", &qref)?;

        // ── Parse server response into `(label, HashMap<String, TokenHit>)` groups ──
        fn shape_from_name(name: &str) -> TokenShape {
            match name {
                "empty" => TokenShape::Empty,
                "lower" => TokenShape::LowerWord,
                "title" => TokenShape::TitleWord,
                "upper" => TokenShape::UpperWord,
                "mixed" => TokenShape::MixedWord,
                "number" => TokenShape::Number,
                "alnum" => TokenShape::AlphaNumeric,
                "punct" => TokenShape::Punctuation,
                "symbol" => TokenShape::Symbolic,
                _ => TokenShape::Other,
            }
        }
        // Freeze `kind` strings to `&'static str` by interning the fixed set
        // produced by `entity_token_kind`.
        fn kind_from_str(s: &str) -> Option<&'static str> {
            match s {
                "title" => Some("title"),
                "mixed" => Some("mixed"),
                "acronym" => Some("acronym"),
                _ => None,
            }
        }

        let mut parsed: Vec<(String, HashMap<String, TokenHit>)> = Vec::new();
        if let Some(groups) = body["groups"].as_array() {
            for g in groups {
                let label = g["label"].as_str().unwrap_or("").to_string();
                let mut hits: HashMap<String, TokenHit> = HashMap::new();
                if let Some(rows) = g["token_hits"].as_array() {
                    for r in rows {
                        let token = r["token"].as_str().unwrap_or("").to_string();
                        let shape = shape_from_name(r["shape"].as_str().unwrap_or("other"));
                        let kind = r["kind"].as_str().and_then(kind_from_str);
                        let hit = TokenHit {
                            shape,
                            kind,
                            hits: r["hits"].as_u64().unwrap_or(0) as usize,
                            syntax_hits: r["syntax_hits"].as_u64().unwrap_or(0) as usize,
                            knowledge_hits: r["knowledge_hits"].as_u64().unwrap_or(0) as usize,
                            output_hits: r["output_hits"].as_u64().unwrap_or(0) as usize,
                            max_score: r["max_score"].as_f64().unwrap_or(0.0) as f32,
                        };
                        hits.insert(token, hit);
                    }
                }
                parsed.push((label, hits));
            }
        }

        // ── Render by reusing local formatters verbatim ──
        if let Some(fmt) = export_format {
            // Export path: flatten all groups into a single map (export ignores
            // grouping, matching the local behaviour).
            let mut merged: HashMap<String, TokenHit> = HashMap::new();
            for (_, hits) in parsed {
                merged.extend(hits);
            }
            return Ok(super::introspection::export_token_summary(
                merged, fmt, verbose, order_by, limit,
            ));
        }

        // No groups (server could return one flat group) → single render pass.
        if group_by.is_none() {
            let (label, hits) = parsed
                .into_iter()
                .next()
                .unwrap_or_else(|| ("across 0 layers".into(), HashMap::new()));
            return Ok(super::introspection::render_token_summary(
                label, hits, verbose, order_by, limit,
            ));
        }

        // Grouped output: render each non-empty group, blank line between,
        // then a trailing "Skipped N ..." mirror of the local path.
        let skipped = body["skipped"].as_u64().unwrap_or(0) as usize;
        let mut out = Vec::new();
        let mut rendered = 0usize;
        for (label, hits) in parsed {
            if hits.is_empty() {
                continue;
            }
            if rendered > 0 {
                out.push(String::new());
            }
            out.extend(super::introspection::render_token_summary(
                label, hits, verbose, order_by, limit,
            ));
            rendered += 1;
        }
        if rendered == 0 {
            return Ok(vec!["  (no tokens found)".into()]);
        }
        if skipped > 0 {
            out.push(String::new());
            let kind = match group_by {
                Some(crate::ast::TokenGroupBy::Layer) => "layer",
                Some(crate::ast::TokenGroupBy::Band) => "band",
                None => "group",
            };
            out.push(format!("Skipped {} empty {} summaries.", skipped, kind));
        }
        Ok(out)
    }

    pub(crate) fn remote_show_models(&self) -> Result<Vec<String>, LqlError> {
        let body = self.remote_get_json("/v1/models", &[])?;

        let mut out = Vec::new();
        out.push(format!(
            "{:<35} {:>10} {:>8} {:>12}",
            "Model", "Features", "Status", "Path"
        ));
        out.push("-".repeat(70));

        if let Some(models) = body["models"].as_array() {
            for m in models {
                let id = m["id"].as_str().unwrap_or("?");
                let features = m["features"].as_u64().unwrap_or(0);
                let loaded = m["loaded"].as_bool().unwrap_or(false);
                let path = m["path"].as_str().unwrap_or("/v1");
                out.push(format!(
                    "{:<35} {:>10} {:>8} {:>12}",
                    id,
                    features,
                    if loaded { "loaded" } else { "unloaded" },
                    path
                ));
            }
        }

        Ok(out)
    }

    pub(crate) fn remote_show_layers(
        &self,
        range: Option<&crate::ast::Range>,
    ) -> Result<Vec<String>, LqlError> {
        let mut params: Vec<(String, String)> = vec![];
        if let Some(r) = range {
            params.push(("start".to_string(), r.start.to_string()));
            params.push(("end".to_string(), r.end.to_string()));
        }

        let qref = Self::to_query_ref(&params);
        let body = self.remote_get_json("/v1/layers", &qref)?;

        let mut out = Vec::new();
        out.push(format!(
            "{:<8} {:>10} {:>10} {:>15}",
            "Layer", "Features", "With Meta", "Top Token"
        ));
        out.push("-".repeat(48));

        if let Some(layers) = body["layers"].as_array() {
            for l in layers {
                let layer = l["layer"].as_u64().unwrap_or(0);
                let features = l["features"].as_u64().unwrap_or(0);
                let with_meta = l["with_meta"].as_u64().unwrap_or(0);
                let top_token = l["top_token"].as_str().unwrap_or("");
                out.push(format!(
                    "{:<8} {:>10} {:>10} {:>15}",
                    layer, features, with_meta, top_token
                ));
            }
        }

        Ok(out)
    }

    pub(crate) fn remote_show_features(
        &self,
        layer: u32,
        conditions: &[crate::ast::Condition],
        limit: Option<u32>,
    ) -> Result<Vec<String>, LqlError> {
        let mut params: Vec<(String, String)> = vec![("layer".to_string(), layer.to_string())];

        let token_filter = conditions
            .iter()
            .find(|c| c.field == "relation" || c.field == "token")
            .and_then(|c| {
                if let crate::ast::Value::String(ref s) = c.value {
                    Some(s.clone())
                } else {
                    None
                }
            });
        let confidence_floor = conditions
            .iter()
            .find(|c| c.field == "confidence" || c.field == "c_score")
            .and_then(|c| match &c.value {
                crate::ast::Value::Number(n) => Some(*n as f32),
                crate::ast::Value::Integer(n) => Some(*n as f32),
                _ => None,
            });

        if let Some(tf) = token_filter {
            params.push(("token".to_string(), tf));
        }
        if let Some(ms) = confidence_floor {
            params.push(("confidence_floor".to_string(), ms.to_string()));
        }
        if let Some(lim) = limit {
            params.push(("limit".to_string(), lim.to_string()));
        }

        let qref = Self::to_query_ref(&params);
        let body = self.remote_get_json("/v1/features", &qref)?;

        if let Some(error) = body.get("error") {
            return Err(LqlError::Execution(
                error.as_str().unwrap_or("unknown error").to_string(),
            ));
        }

        let mut out = Vec::new();
        out.push(format!(
            "{:<8} {:<20} {:>10} {:>30}",
            "Feature", "Top Token", "Score", "Down outputs"
        ));
        out.push("-".repeat(72));

        if let Some(features) = body["features"].as_array() {
            for f in features {
                let feature = f["feature"].as_u64().unwrap_or(0);
                let top_token = f["top_token"].as_str().unwrap_or("");
                let score = f["score"].as_f64().unwrap_or(0.0);
                let down_outputs = f["down_outputs"].as_str().unwrap_or("");
                out.push(format!(
                    "{:<8} {:<20} {:>10.1} {:>30}",
                    feature, top_token, score, down_outputs
                ));
            }
        }

        Ok(out)
    }

    pub(crate) fn remote_show_entities(
        &self,
        layer: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<String>, LqlError> {
        let mut params: Vec<(String, String)> = vec![];
        if let Some(l) = layer {
            params.push(("layer".to_string(), l.to_string()));
        }
        if let Some(lim) = limit {
            params.push(("limit".to_string(), lim.to_string()));
        }

        let qref = Self::to_query_ref(&params);
        let body = self.remote_get_json("/v1/entities", &qref)?;

        let mut out = Vec::new();
        out.push(format!("{:<30} {:>10} {:>10}", "Entity", "Count", "Score"));
        out.push("-".repeat(52));

        if let Some(entities) = body["entities"].as_array() {
            for e in entities {
                let entity = e["entity"].as_str().unwrap_or("");
                let count = e["count"].as_u64().unwrap_or(0);
                let score = e["score"].as_f64().unwrap_or(0.0);
                out.push(format!("{:<30} {:>10} {:>10.1}", entity, count, score));
            }
        }

        Ok(out)
    }

    pub(crate) fn remote_remove_local_patch(
        &mut self,
        name: &str,
    ) -> Result<Vec<String>, LqlError> {
        let local_patches = match &mut self.backend {
            Backend::Remote { local_patches, .. } => local_patches,
            _ => {
                return Err(LqlError::Execution(
                    "not connected to a remote server".into(),
                ))
            }
        };

        // Try to parse as index first, then fall back to description match
        let pos = if let Ok(idx) = name.parse::<usize>() {
            if idx < local_patches.len() {
                Some(idx)
            } else {
                None
            }
        } else {
            // Fallback to description match for backward compatibility
            local_patches
                .iter()
                .position(|p| p.description.as_deref().unwrap_or("unnamed") == name)
        };

        match pos {
            Some(i) => {
                let removed = local_patches.remove(i);
                let desc = removed.description.as_deref().unwrap_or("unnamed");
                Ok(vec![format!("Removed local patch #{}: {}", i, desc)])
            }
            None => Err(LqlError::Execution(format!(
                "local patch not found: {name} (use index or description)"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{condition_string, condition_usize, rows_to_layer_features};
    use crate::ast::{CompareOp, Condition, Value};
    use crate::error::LqlError;

    #[test]
    fn remote_condition_helpers_extract_expected_types() {
        let conditions = vec![
            Condition {
                field: "layer".into(),
                op: CompareOp::Eq,
                value: Value::Integer(12),
            },
            Condition {
                field: "entity".into(),
                op: CompareOp::Eq,
                value: Value::String("France".into()),
            },
        ];
        assert_eq!(condition_usize(&conditions, "layer"), Some(12));
        assert_eq!(
            condition_string(&conditions, "entity").as_deref(),
            Some("France")
        );
        assert_eq!(condition_usize(&conditions, "entity"), None);
    }

    #[test]
    fn rows_to_layer_features_rejects_malformed_rows() {
        let rows = vec![serde_json::json!({"layer": 3})];
        let err = rows_to_layer_features(&rows).unwrap_err();
        assert!(matches!(err, LqlError::Execution(_)));
    }
}
