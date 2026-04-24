use crate::ffn::WeightFfn;
use crate::forward::{
    hidden_vec_to_token_predictions, hidden_vec_token_probability,
    incremental_forward_append_token, predict_with_ffn_attention,
    prepare_incremental_forward_state, IncrementalForwardState, LayerAttentionCapture,
    PredictResultWithAttention, TokenPrediction,
};
use crate::model::ModelWeights;
use larql_tokenizer::Tokenizer;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

const ANALYSIS_TOP_K: usize = 5;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AnalysisRequest {
    pub prompt: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub truth_spans: Vec<String>,
    #[serde(default)]
    pub materially_false_spans: Vec<String>,
    #[serde(default)]
    pub coherence_markers: Vec<String>,
    #[serde(default)]
    pub max_generated_tokens: Option<usize>,
    #[serde(default)]
    pub ridge_dead_zone: Option<f32>,
}

const fn default_top_k() -> usize {
    5
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisMode {
    FactProbe,
    WorkflowProbe,
}

impl AnalysisMode {
    pub fn parse(mode: &str) -> Result<Self, String> {
        match mode {
            "" | "fact_probe" => Ok(Self::FactProbe),
            "workflow_probe" => Ok(Self::WorkflowProbe),
            other => Err(format!(
                "Unsupported analysis mode '{other}'. Expected 'fact_probe' or 'workflow_probe'"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadContribution {
    pub layer: usize,
    pub head: usize,
    pub source_token: usize,
    pub contribution: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepTopHeadSummary {
    pub false_content: Vec<HeadContribution>,
    pub material_coherence: Vec<HeadContribution>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedStep {
    pub position: usize,
    pub token_id: u32,
    pub token: String,
    pub probability: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenAnalysis {
    pub position: usize,
    pub token_id: u32,
    pub token: String,
    pub probability: f64,
    pub label: String,
    pub truth_mass: f64,
    pub false_mass: f64,
    pub coherence_mass: f64,
    pub ridge: f64,
    pub top_heads: StepTopHeadSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirstFalseOrigin {
    pub position: usize,
    pub token_id: u32,
    pub token: String,
    pub layer: usize,
    pub head: usize,
    pub source_token: usize,
    pub contribution: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisSummary {
    pub first_false_position: Option<usize>,
    pub first_false_token: Option<String>,
    pub first_false_origin: Option<FirstFalseOrigin>,
    pub top_coherence_heads: Vec<HeadContribution>,
    pub top_false_content_heads: Vec<HeadContribution>,
    pub materially_false_detected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerRidge {
    pub layer: usize,
    pub ridge: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionLayer {
    pub layer: usize,
    pub heads: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogitLensLayer {
    pub layer: usize,
    pub predictions: Vec<(String, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadDlaLayer {
    pub layer: usize,
    pub heads: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub attention: Vec<AttentionLayer>,
    pub logit_lens: Vec<LogitLensLayer>,
    pub head_dla: Vec<HeadDlaLayer>,
    pub num_layers: usize,
    pub seq_len: usize,
    pub tokens: Vec<u32>,
    pub strings: Vec<String>,
    pub predictions: Vec<(String, f64)>,
    pub generation_trace: Vec<GeneratedStep>,
    pub token_analysis: Vec<TokenAnalysis>,
    pub analysis_summary: Option<AnalysisSummary>,
    pub ridge_by_layer: Vec<LayerRidge>,
}

#[derive(Debug, Clone)]
struct EncodedSpan {
    normalized_text: String,
    token_ids: Vec<u32>,
}

#[derive(Debug, Clone)]
struct ProjectedSpanCompletion {
    branch_token_ids: Vec<u32>,
    first_token_id: u32,
    first_token_text: String,
    first_token_probability: f64,
    log_probability: f64,
}

#[derive(Debug, Clone)]
struct ExactSpanResolution {
    total_probability: f64,
    best_completion: Option<ProjectedSpanCompletion>,
}

#[derive(Debug, Clone)]
struct StepAnalysis {
    token: TokenAnalysis,
    resolved_truth: bool,
    resolved_false: bool,
}

pub fn analyze_infer(
    weights: &ModelWeights,
    tokenizer: &dyn Tokenizer,
    num_layers: usize,
    request: &AnalysisRequest,
) -> Result<AnalysisResult, String> {
    let tokens = tokenizer
        .encode(&request.prompt, false)
        .map_err(|e| format!("Tokenization error: {e}"))?;

    if tokens.ids.is_empty() {
        return Err("Empty prompt results in no tokens".to_string());
    }

    let strings: Vec<String> = tokens
        .ids
        .iter()
        .map(|&id| tokenizer.decode(&[id], true).unwrap_or_else(|_| "?".to_string()))
        .collect();

    let ffn = WeightFfn { weights };
    let mut current_tokens = tokens.ids.clone();
    let mut final_result: Option<PredictResultWithAttention> = None;
    let mut generation_trace = Vec::new();
    let mut token_analysis = Vec::new();
    let mut first_false_origin: Option<FirstFalseOrigin> = None;
    let mut ridge_by_layer_totals: BTreeMap<usize, f64> = BTreeMap::new();
    let mut generated_token_ids = Vec::new();

    let (analysis_enabled, mode, truth_spans, false_spans, coherence_spans, max_steps, dead_zone) =
        if request.mode.is_empty()
            && request.truth_spans.is_empty()
            && request.materially_false_spans.is_empty()
            && request.coherence_markers.is_empty()
            && request.max_generated_tokens.is_none()
            && request.ridge_dead_zone.is_none()
        {
            (
                false,
                AnalysisMode::FactProbe,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                1,
                0.05,
            )
        } else {
            let mode = AnalysisMode::parse(&request.mode)?;
            let truth_spans = encode_spans(tokenizer, &request.truth_spans)?;
            let false_spans = encode_spans(tokenizer, &request.materially_false_spans)?;
            let coherence_spans = encode_spans(tokenizer, &request.coherence_markers)?;
            (
                true,
                mode,
                truth_spans,
                false_spans,
                coherence_spans,
                request.max_generated_tokens.unwrap_or(1).max(1),
                request.ridge_dead_zone.unwrap_or(0.05) as f64,
            )
        };

    for position in 0..max_steps {
        let result =
            predict_with_ffn_attention(weights, tokenizer, &current_tokens, request.top_k, &ffn);

        if analysis_enabled {
            if let Some(step) = analyze_step(
                mode,
                weights,
                tokenizer,
                &result,
                &truth_spans,
                &false_spans,
                &coherence_spans,
                dead_zone,
                position,
                &current_tokens,
                &generated_token_ids,
            ) {
                generated_token_ids.push(step.token.token_id);
                if first_false_origin.is_none() && step.token.label == "materially_false" {
                    if let Some(origin) = step.token.top_heads.false_content.first() {
                        first_false_origin = Some(FirstFalseOrigin {
                            position: step.token.position,
                            token_id: step.token.token_id,
                            token: step.token.token.clone(),
                            layer: origin.layer,
                            head: origin.head,
                            source_token: origin.source_token,
                            contribution: origin.contribution,
                        });
                    }
                }

                for (layer, ridge) in layer_ridge_values(
                    weights,
                    &result.head_dla,
                    &truth_spans,
                    &false_spans,
                    &coherence_spans,
                    dead_zone,
                ) {
                    *ridge_by_layer_totals.entry(layer).or_insert(0.0) += ridge;
                }

                generation_trace.push(GeneratedStep {
                    position: step.token.position,
                    token_id: step.token.token_id,
                    token: step.token.token.clone(),
                    probability: step.token.probability,
                });
                token_analysis.push(step.token.clone());

                if mode == AnalysisMode::FactProbe && (step.resolved_truth || step.resolved_false) {
                    final_result = Some(result);
                    break;
                }

                current_tokens.push(step.token.token_id);
            }
        }

        final_result = Some(result);
        if !analysis_enabled {
            break;
        }
    }

    let result = final_result.expect("analyze_infer should produce at least one result");
    let ridge_by_layer = ridge_by_layer_totals
        .into_iter()
        .map(|(layer, ridge)| LayerRidge { layer, ridge })
        .collect();
    let attention = result
        .attention
        .iter()
        .map(|layer_capture| AttentionLayer {
            layer: layer_capture.layer,
            heads: layer_capture.weights.heads.clone(),
        })
        .collect();
    let logit_lens = result
        .logit_lens
        .iter()
        .map(|(layer, preds)| LogitLensLayer {
            layer: *layer,
            predictions: preds.clone(),
        })
        .collect();
    let head_dla = result
        .head_dla
        .iter()
        .map(|(layer, heads)| HeadDlaLayer {
            layer: *layer,
            heads: heads.clone(),
        })
        .collect();

    let analysis_summary = if analysis_enabled {
        let mut top_coherence_heads = token_analysis
            .iter()
            .flat_map(|step| step.top_heads.material_coherence.clone())
            .collect::<Vec<_>>();
        top_coherence_heads.sort_by(|a, b| b.contribution.partial_cmp(&a.contribution).unwrap());
        top_coherence_heads.truncate(5);

        let mut top_false_heads = token_analysis
            .iter()
            .flat_map(|step| step.top_heads.false_content.clone())
            .collect::<Vec<_>>();
        top_false_heads.sort_by(|a, b| b.contribution.partial_cmp(&a.contribution).unwrap());
        top_false_heads.truncate(5);

        Some(AnalysisSummary {
            first_false_position: first_false_origin.as_ref().map(|origin| origin.position),
            first_false_token: first_false_origin.as_ref().map(|origin| origin.token.clone()),
            first_false_origin,
            top_coherence_heads,
            top_false_content_heads: top_false_heads,
            materially_false_detected: token_analysis
                .iter()
                .any(|step| step.label == "materially_false"),
        })
    } else {
        None
    };

    Ok(AnalysisResult {
        attention,
        logit_lens,
        head_dla,
        num_layers,
        seq_len: tokens.ids.len(),
        tokens: tokens.ids,
        strings,
        predictions: result.predictions,
        generation_trace,
        token_analysis,
        analysis_summary,
        ridge_by_layer,
    })
}

fn encode_spans(tokenizer: &dyn Tokenizer, spans: &[String]) -> Result<Vec<EncodedSpan>, String> {
    spans
        .iter()
        .filter(|span| !span.trim().is_empty())
        .map(|span| {
            tokenizer
                .encode(span, false)
                .map_err(|e| format!("Failed to encode analysis span '{span}': {e}"))
                .and_then(|enc| {
                    if enc.ids.is_empty() {
                        Err(format!("Analysis span '{span}' tokenized to empty"))
                    } else {
                        Ok(EncodedSpan {
                            normalized_text: normalize_text(span),
                            token_ids: enc.ids,
                        })
                    }
                })
        })
        .collect()
}

fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn token_set(spans: &[EncodedSpan]) -> BTreeSet<u32> {
    spans
        .iter()
        .flat_map(|span| span.token_ids.iter().copied())
        .collect()
}

fn max_mass_for_tokens(weights: &ModelWeights, hidden: &[f32], token_ids: &BTreeSet<u32>) -> f64 {
    token_ids
        .iter()
        .filter_map(|&token_id| hidden_vec_token_probability(weights, hidden, token_id))
        .fold(0.0, f64::max)
}

fn max_mass_for_spans(weights: &ModelWeights, hidden: &[f32], spans: &[EncodedSpan]) -> f64 {
    spans
        .iter()
        .map(|span| {
            let total: f64 = span
                .token_ids
                .iter()
                .filter_map(|&token_id| hidden_vec_token_probability(weights, hidden, token_id))
                .sum();
            total / span.token_ids.len() as f64
        })
        .fold(0.0, f64::max)
}

fn generated_tokens_complete_span(
    tokenizer: &dyn Tokenizer,
    generated_token_ids: &[u32],
    span: &EncodedSpan,
) -> bool {
    if generated_token_ids.ends_with(&span.token_ids) {
        return true;
    }

    let decoded_generated = tokenizer
        .decode(generated_token_ids, true)
        .ok()
        .map(|text| normalize_text(&text));
    if decoded_generated
        .as_deref()
        .is_some_and(|text| text.ends_with(&span.normalized_text))
    {
        return true;
    }

    let max_window = generated_token_ids
        .len()
        .min(span.token_ids.len().saturating_mul(4).max(span.token_ids.len()));
    (1..=max_window).any(|window_len| {
        tokenizer
            .decode(
                &generated_token_ids[generated_token_ids.len() - window_len..],
                true,
            )
            .ok()
            .map(|decoded| {
                let normalized = normalize_text(&decoded);
                normalized == span.normalized_text || normalized.ends_with(&span.normalized_text)
            })
            .unwrap_or(false)
    })
}

fn generated_tokens_complete_any_span(
    tokenizer: &dyn Tokenizer,
    generated_token_ids: &[u32],
    spans: &[EncodedSpan],
) -> bool {
    spans
        .iter()
        .any(|span| generated_tokens_complete_span(tokenizer, generated_token_ids, span))
}

fn projected_completion_better(
    candidate: &ProjectedSpanCompletion,
    current_best: &ProjectedSpanCompletion,
) -> bool {
    candidate.log_probability > current_best.log_probability
        || (candidate.log_probability == current_best.log_probability
            && candidate.branch_token_ids.len() < current_best.branch_token_ids.len())
}

fn dominant_exact_resolution<'a>(
    truth: Option<&'a ExactSpanResolution>,
    falsehood: Option<&'a ExactSpanResolution>,
) -> Option<(&'static str, &'a ExactSpanResolution)> {
    match (
        truth.filter(|resolution| resolution.total_probability > 0.0),
        falsehood.filter(|resolution| resolution.total_probability > 0.0),
    ) {
        (Some(truth), Some(falsehood)) => {
            if falsehood.total_probability > truth.total_probability {
                Some(("materially_false", falsehood))
            } else {
                Some(("truth_support", truth))
            }
        }
        (Some(truth), None) => Some(("truth_support", truth)),
        (None, Some(falsehood)) => Some(("materially_false", falsehood)),
        (None, None) => None,
    }
}

fn normalized_prefix_lengths(text: &str) -> Vec<usize> {
    let mut boundaries = text.char_indices().map(|(idx, _)| idx).collect::<Vec<_>>();
    boundaries.push(text.len());
    boundaries
}

fn span_overlap_prefix_lengths(current_text: &str, target_text: &str) -> Vec<usize> {
    let mut overlaps = normalized_prefix_lengths(target_text)
        .into_iter()
        .filter(|&len| current_text.ends_with(&target_text[..len]))
        .collect::<Vec<_>>();
    overlaps.sort_unstable();
    overlaps.dedup();
    overlaps
}

fn normalized_token_candidates(
    tokenizer: &dyn Tokenizer,
    spans: &[EncodedSpan],
) -> Vec<(u32, String)> {
    let mut candidates = Vec::new();
    for token_id in 0..tokenizer.vocab_size(false) as u32 {
        let Ok(decoded) = tokenizer.decode(&[token_id], true) else {
            continue;
        };
        let normalized = normalize_text(&decoded);
        if normalized.is_empty() {
            continue;
        }
        if spans
            .iter()
            .any(|span| span.normalized_text.contains(&normalized))
        {
            candidates.push((token_id, decoded));
        }
    }
    candidates
}

fn enumerate_exact_span_tokenizations(
    tokenizer: &dyn Tokenizer,
    remainder: &str,
    token_candidates: &[(u32, String)],
) -> Vec<Vec<u32>> {
    fn walk(
        tokenizer: &dyn Tokenizer,
        remainder: &str,
        token_candidates: &[(u32, String)],
        current_tokens: &mut Vec<u32>,
        current_normalized: &str,
        out: &mut BTreeSet<Vec<u32>>,
    ) {
        if current_normalized == remainder {
            out.insert(current_tokens.clone());
            return;
        }

        for (token_id, _) in token_candidates {
            current_tokens.push(*token_id);
            let next_normalized = tokenizer
                .decode(current_tokens, true)
                .ok()
                .map(|decoded| normalize_text(&decoded));

            if let Some(next_normalized) = next_normalized {
                if next_normalized.len() > current_normalized.len()
                    && remainder.starts_with(&next_normalized)
                {
                    walk(
                        tokenizer,
                        remainder,
                        token_candidates,
                        current_tokens,
                        &next_normalized,
                        out,
                    );
                }
            }
            current_tokens.pop();
        }
    }

    if remainder.is_empty() {
        return vec![Vec::new()];
    }

    let mut out = BTreeSet::new();
    walk(
        tokenizer,
        remainder,
        token_candidates,
        &mut Vec::new(),
        "",
        &mut out,
    );
    out.into_iter().collect()
}

fn exact_span_resolution<F>(
    tokenizer: &dyn Tokenizer,
    generated_token_prefix: &[u32],
    spans: &[EncodedSpan],
    token_candidates: &[(u32, String)],
    mut branch_probability: F,
) -> ExactSpanResolution
where
    F: FnMut(&[u32]) -> Option<f64>,
{
    if spans.is_empty() {
        return ExactSpanResolution {
            total_probability: 0.0,
            best_completion: None,
        };
    }

    let generated_text = tokenizer
        .decode(generated_token_prefix, true)
        .ok()
        .map(|text| normalize_text(&text))
        .unwrap_or_default();

    let mut completion_paths = BTreeSet::new();
    for span in spans {
        for overlap_len in span_overlap_prefix_lengths(&generated_text, &span.normalized_text) {
            let remainder = &span.normalized_text[overlap_len..];
            for completion in
                enumerate_exact_span_tokenizations(tokenizer, remainder, token_candidates)
            {
                completion_paths.insert(completion);
            }
        }
    }

    let mut total_probability = 0.0;
    let mut best_completion: Option<ProjectedSpanCompletion> = None;

    for branch_token_ids in completion_paths {
        if branch_token_ids.is_empty() {
            continue;
        }
        let Some(probability) = branch_probability(&branch_token_ids) else {
            continue;
        };
        if probability <= 0.0 {
            continue;
        }

        total_probability += probability;

        let first_token_id = branch_token_ids[0];
        let first_token_text = tokenizer
            .decode(&[first_token_id], true)
            .unwrap_or_else(|_| String::new());
        let candidate = ProjectedSpanCompletion {
            branch_token_ids: branch_token_ids.clone(),
            first_token_id,
            first_token_text,
            first_token_probability: branch_probability(&branch_token_ids[..1])
                .unwrap_or(probability),
            log_probability: probability.max(1e-12).ln(),
        };
        if best_completion
            .as_ref()
            .is_none_or(|current| projected_completion_better(&candidate, current))
        {
            best_completion = Some(candidate);
        }
    }

    ExactSpanResolution {
        total_probability,
        best_completion,
    }
}

fn label_for_step(
    mode: AnalysisMode,
    tokenizer: &dyn Tokenizer,
    generated_token_ids: &[u32],
    truth_spans: &[EncodedSpan],
    false_spans: &[EncodedSpan],
    coherence_tokens: &BTreeSet<u32>,
    token_id: u32,
    truth_mass: f64,
    false_mass: f64,
    coherence_mass: f64,
    dead_zone: f64,
    projected_truth: Option<&ExactSpanResolution>,
    projected_false: Option<&ExactSpanResolution>,
) -> String {
    if generated_tokens_complete_any_span(tokenizer, generated_token_ids, false_spans) {
        return "materially_false".to_string();
    }
    if generated_tokens_complete_any_span(tokenizer, generated_token_ids, truth_spans) {
        return "truth_support".to_string();
    }

    match mode {
        AnalysisMode::FactProbe => {
            if let Some((label, _)) = dominant_exact_resolution(projected_truth, projected_false) {
                label.to_string()
            } else if coherence_tokens.contains(&token_id) && coherence_mass > dead_zone {
                "material_coherence".to_string()
            } else {
                "unlabeled".to_string()
            }
        }
        AnalysisMode::WorkflowProbe => {
            if false_mass > truth_mass + dead_zone && coherence_mass > dead_zone {
                "materially_false".to_string()
            } else if coherence_mass > dead_zone || coherence_tokens.contains(&token_id) {
                "material_coherence".to_string()
            } else {
                "unlabeled".to_string()
            }
        }
    }
}

fn top_head_contributions(
    weights: &ModelWeights,
    head_dla: &[(usize, Vec<Vec<f32>>)],
    attention: &[LayerAttentionCapture],
    token_ids: &BTreeSet<u32>,
    limit: usize,
) -> Vec<HeadContribution> {
    let mut out = Vec::new();
    for (layer, heads) in head_dla {
        let layer_attention = attention.iter().find(|entry| entry.layer == *layer);
        for (head_idx, hidden) in heads.iter().enumerate() {
            let contribution = max_mass_for_tokens(weights, hidden, token_ids);
            let source_token = layer_attention
                .and_then(|entry| entry.weights.heads.get(head_idx))
                .and_then(|vals| {
                    vals.iter()
                        .copied()
                        .enumerate()
                        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                        .map(|(idx, _)| idx)
                })
                .unwrap_or(0);
            out.push(HeadContribution {
                layer: *layer,
                head: head_idx,
                source_token,
                contribution,
            });
        }
    }
    out.sort_by(|a, b| b.contribution.partial_cmp(&a.contribution).unwrap());
    out.truncate(limit);
    out
}

fn layer_ridge_values(
    weights: &ModelWeights,
    head_dla: &[(usize, Vec<Vec<f32>>)],
    truth_spans: &[EncodedSpan],
    false_spans: &[EncodedSpan],
    coherence_spans: &[EncodedSpan],
    dead_zone: f64,
) -> Vec<(usize, f64)> {
    head_dla
        .iter()
        .map(|(layer, heads)| {
            let layer_ridge = heads
                .iter()
                .map(|head| {
                    let false_mass = max_mass_for_spans(weights, head, false_spans);
                    let truth_mass = max_mass_for_spans(weights, head, truth_spans);
                    let coherence_mass = max_mass_for_spans(weights, head, coherence_spans);
                    (coherence_mass - dead_zone).max(0.0)
                        * (false_mass - truth_mass - dead_zone).max(0.0)
                })
                .fold(0.0, f64::max);
            (*layer, layer_ridge)
        })
        .collect()
}

fn analyze_step(
    mode: AnalysisMode,
    weights: &ModelWeights,
    tokenizer: &dyn Tokenizer,
    result: &PredictResultWithAttention,
    truth_spans: &[EncodedSpan],
    false_spans: &[EncodedSpan],
    coherence_spans: &[EncodedSpan],
    dead_zone: f64,
    position: usize,
    current_tokens: &[u32],
    generated_token_prefix: &[u32],
) -> Option<StepAnalysis> {
    let top_predictions =
        hidden_vec_to_token_predictions(weights, tokenizer, &result.final_hidden, ANALYSIS_TOP_K);
    let top_pred = top_predictions.first()?.clone();

    let coherence_tokens = token_set(coherence_spans);
    let truth_mass = max_mass_for_spans(weights, &result.final_hidden, truth_spans);
    let false_mass = max_mass_for_spans(weights, &result.final_hidden, false_spans);
    let coherence_mass = max_mass_for_spans(weights, &result.final_hidden, coherence_spans);
    let ridge =
        (coherence_mass - dead_zone).max(0.0) * (false_mass - truth_mass - dead_zone).max(0.0);
    let false_heads = top_head_contributions(
        weights,
        &result.head_dla,
        &result.attention,
        &token_set(false_spans),
        3,
    );
    let coherence_heads = top_head_contributions(
        weights,
        &result.head_dla,
        &result.attention,
        &coherence_tokens,
        3,
    );

    let (projected_truth, projected_false) = if mode == AnalysisMode::FactProbe {
        let token_candidates =
            normalized_token_candidates(tokenizer, &[truth_spans, false_spans].concat());
        let ffn = WeightFfn { weights };
        let mut state_cache: HashMap<Vec<u32>, IncrementalForwardState> = HashMap::new();
        if let Some(root_state) = prepare_incremental_forward_state(weights, current_tokens, &ffn) {
            state_cache.insert(Vec::new(), root_state);
        }
        (
            Some(exact_span_resolution(
                tokenizer,
                generated_token_prefix,
                truth_spans,
                &token_candidates,
                |branch_token_ids| {
                    branch_probability(weights, &mut state_cache, &ffn, branch_token_ids)
                },
            )),
            Some(exact_span_resolution(
                tokenizer,
                generated_token_prefix,
                false_spans,
                &token_candidates,
                |branch_token_ids| {
                    branch_probability(weights, &mut state_cache, &ffn, branch_token_ids)
                },
            )),
        )
    } else {
        (None, None)
    };

    let selected_prediction =
        dominant_exact_resolution(projected_truth.as_ref(), projected_false.as_ref())
            .and_then(|(_, resolution)| resolution.best_completion.as_ref())
            .map(|completion| TokenPrediction {
                token_id: completion.first_token_id,
                text: completion.first_token_text.clone(),
                probability: completion.first_token_probability,
            })
            .unwrap_or(top_pred);

    let mut generated_token_ids = generated_token_prefix.to_vec();
    generated_token_ids.push(selected_prediction.token_id);

    let label = label_for_step(
        mode,
        tokenizer,
        &generated_token_ids,
        truth_spans,
        false_spans,
        &coherence_tokens,
        selected_prediction.token_id,
        truth_mass,
        false_mass,
        coherence_mass,
        dead_zone,
        projected_truth.as_ref(),
        projected_false.as_ref(),
    );

    let resolved_truth = if mode == AnalysisMode::FactProbe {
        matches!(
            dominant_exact_resolution(projected_truth.as_ref(), projected_false.as_ref()),
            Some(("truth_support", _))
        ) || generated_tokens_complete_any_span(tokenizer, &generated_token_ids, truth_spans)
    } else {
        false
    };
    let resolved_false = if mode == AnalysisMode::FactProbe {
        matches!(
            dominant_exact_resolution(projected_truth.as_ref(), projected_false.as_ref()),
            Some(("materially_false", _))
        ) || generated_tokens_complete_any_span(tokenizer, &generated_token_ids, false_spans)
    } else {
        false
    };

    Some(StepAnalysis {
        token: TokenAnalysis {
            position,
            token_id: selected_prediction.token_id,
            token: selected_prediction.text,
            probability: selected_prediction.probability,
            label,
            truth_mass,
            false_mass,
            coherence_mass,
            ridge,
            top_heads: StepTopHeadSummary {
                false_content: false_heads,
                material_coherence: coherence_heads,
            },
        },
        resolved_truth,
        resolved_false,
    })
}

fn branch_probability(
    weights: &ModelWeights,
    state_cache: &mut HashMap<Vec<u32>, IncrementalForwardState>,
    ffn: &WeightFfn<'_>,
    branch_token_ids: &[u32],
) -> Option<f64> {
    let mut probability = 1.0;
    for step in 0..branch_token_ids.len() {
        let prefix = &branch_token_ids[..step];
        let hidden = if let Some(state) = state_cache.get(prefix) {
            state.final_hidden().to_vec()
        } else {
            let parent_prefix = &branch_token_ids[..step.saturating_sub(1)];
            let parent_state = state_cache.get(parent_prefix)?.clone();
            let next_state =
                incremental_forward_append_token(weights, &parent_state, branch_token_ids[step - 1], ffn)?;
            let hidden = next_state.final_hidden().to_vec();
            state_cache.insert(prefix.to_vec(), next_state);
            hidden
        };
        let step_probability =
            hidden_vec_token_probability(weights, &hidden, branch_token_ids[step])?;
        probability *= step_probability;
    }
    Some(probability)
}

#[cfg(test)]
mod tests {
    use super::{
        dominant_exact_resolution, encode_spans, exact_span_resolution,
        generated_tokens_complete_any_span, normalize_text, normalized_token_candidates,
    };
    use larql_tokenizer::{Encoded, Tokenizer, TokenizerError};
    use std::collections::HashMap;

    struct FakeTokenizer {
        encode_map: HashMap<String, Vec<u32>>,
        decode_map: HashMap<Vec<u32>, String>,
        id_map: HashMap<u32, String>,
    }

    impl FakeTokenizer {
        fn new() -> Self {
            let mut encode_map = HashMap::new();
            encode_map.insert("Birnin Zana".to_string(), vec![1, 2]);
            encode_map.insert("Freedonia Capital".to_string(), vec![10, 11]);

            let mut decode_map = HashMap::new();
            decode_map.insert(vec![1], "Birnin".to_string());
            decode_map.insert(vec![2], " Zana".to_string());
            decode_map.insert(vec![7], "Birnin Zana".to_string());
            decode_map.insert(vec![8], " Birnin".to_string());
            decode_map.insert(vec![9], " Zana".to_string());
            decode_map.insert(vec![8, 9], " Birnin Zana".to_string());
            decode_map.insert(vec![10], "Freedonia".to_string());
            decode_map.insert(vec![11], " Capital".to_string());
            decode_map.insert(vec![12], "Freedonia Capital".to_string());

            let mut id_map = HashMap::new();
            id_map.insert(1, "Birnin".to_string());
            id_map.insert(2, " Zana".to_string());
            id_map.insert(7, "Birnin Zana".to_string());
            id_map.insert(8, " Birnin".to_string());
            id_map.insert(9, " Zana".to_string());
            id_map.insert(10, "Freedonia".to_string());
            id_map.insert(11, " Capital".to_string());
            id_map.insert(12, "Freedonia Capital".to_string());

            Self {
                encode_map,
                decode_map,
                id_map,
            }
        }
    }

    impl Tokenizer for FakeTokenizer {
        fn encode(
            &self,
            text: &str,
            _add_special_tokens: bool,
        ) -> Result<Encoded, TokenizerError> {
            self.encode_map
                .get(text)
                .cloned()
                .map(|ids| Encoded {
                    tokens: ids
                        .iter()
                        .map(|id| self.id_to_token(*id).unwrap_or_else(|| format!("[{id}]")))
                        .collect(),
                    ids,
                })
                .ok_or_else(|| TokenizerError::Parse("missing".to_string()))
        }

        fn decode(&self, ids: &[u32], _skip_special_tokens: bool) -> Result<String, TokenizerError> {
            if let Some(text) = self.decode_map.get(ids) {
                return Ok(text.clone());
            }
            if ids.len() == 1 {
                return self
                    .id_map
                    .get(&ids[0])
                    .cloned()
                    .ok_or_else(|| TokenizerError::Parse("missing".to_string()));
            }
            Err(TokenizerError::Parse("missing".to_string()))
        }

        fn id_to_token(&self, id: u32) -> Option<String> {
            self.id_map.get(&id).cloned()
        }

        fn token_to_id(&self, tok: &str) -> Option<u32> {
            self.id_map
                .iter()
                .find_map(|(id, value)| if value == tok { Some(*id) } else { None })
        }

        fn vocab_size(&self, _with_added_tokens: bool) -> usize {
            16
        }

        fn backend_name(&self) -> &'static str {
            "fake"
        }
    }

    #[test]
    fn normalize_text_collapses_whitespace() {
        assert_eq!(normalize_text(" Birnin   Zana "), "Birnin Zana");
    }

    #[test]
    fn generated_tokens_complete_any_span_accepts_alternate_segmentation() {
        let tokenizer = FakeTokenizer::new();
        let spans = encode_spans(&tokenizer, &[String::from("Birnin Zana")]).unwrap();
        assert!(generated_tokens_complete_any_span(&tokenizer, &[8, 9], &spans));
        assert!(generated_tokens_complete_any_span(&tokenizer, &[7], &spans));
    }

    #[test]
    fn exact_span_resolution_sums_all_matching_tokenizations() {
        let tokenizer = FakeTokenizer::new();
        let spans = encode_spans(&tokenizer, &[String::from("Birnin Zana")]).unwrap();
        let candidates = normalized_token_candidates(&tokenizer, &spans);
        let resolution = exact_span_resolution(&tokenizer, &[], &spans, &candidates, |branch| {
            match branch {
                [7] => Some(0.3),
                [8, 9] => Some(0.4),
                _ => None,
            }
        });

        assert!((resolution.total_probability - 0.7).abs() < 1e-9);
        assert_eq!(
            resolution.best_completion.as_ref().unwrap().branch_token_ids,
            vec![8, 9]
        );
    }

    #[test]
    fn dominant_exact_resolution_uses_total_probability() {
        let truth = super::ExactSpanResolution {
            total_probability: 0.45,
            best_completion: None,
        };
        let falsehood = super::ExactSpanResolution {
            total_probability: 0.55,
            best_completion: None,
        };

        let (label, _) =
            dominant_exact_resolution(Some(&truth), Some(&falsehood)).expect("expected winner");
        assert_eq!(label, "materially_false");
    }
}
