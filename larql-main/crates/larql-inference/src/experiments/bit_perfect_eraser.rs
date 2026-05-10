//! Rust-native verification of the "Bit-Perfect Eraser" claim.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::{forward, predict_with_ffn_attention, vindex::WalkFfn};
use larql_vindex::{
    load_model_weights, load_vindex_config, load_vindex_tokenizer, GateIndex, PatchedVindex,
    SilentLoadCallbacks, VectorIndex,
};
use ndarray::Array1;
use serde::{Deserialize, Serialize};

pub type ExperimentError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BitPerfectEraserRequest {
    pub vindex_path: PathBuf,
    pub top_k: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PredictionRecord {
    pub rank: usize,
    pub token: String,
    pub probability: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TargetTokenRecord {
    pub token: String,
    pub token_id: Option<u32>,
    pub probability: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InferenceRun {
    pub prompt: String,
    pub top_k: usize,
    pub token_count: usize,
    pub token_ids: Vec<u32>,
    pub duration_ms: f64,
    pub top_predictions: Vec<PredictionRecord>,
    pub target_tokens: Vec<TargetTokenRecord>,
    pub final_hidden_len: usize,
    pub final_hidden_fnv1a64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunComparison {
    pub prompt: String,
    pub baseline_top_predictions_equal: bool,
    pub baseline_final_hidden_bits_equal: bool,
    pub max_abs_delta: f64,
    pub l1_delta: f64,
    pub l2_delta: f64,
    pub target_token_deltas: Vec<TargetTokenDelta>,
    pub slot_observations: Vec<SlotObservation>,
    pub baseline: InferenceRun,
    pub patched: InferenceRun,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlotEdit {
    pub layer: usize,
    pub feature: usize,
    pub in_layer_range: bool,
    pub in_feature_range: bool,
    pub original_present: bool,
    pub override_length: Option<usize>,
    pub original_length: Option<usize>,
    pub original_sum: Option<f64>,
    pub original_l2: Option<f64>,
    pub edited: bool,
    pub note: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TargetTokenDelta {
    pub token: String,
    pub token_id: Option<u32>,
    pub baseline_probability: Option<f64>,
    pub patched_probability: Option<f64>,
    pub delta: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlotObservation {
    pub prompt: String,
    pub layer: usize,
    pub feature: usize,
    pub gate_score: Option<f64>,
    pub gate_abs_rank: Option<usize>,
    pub up_score: Option<f64>,
    pub activation: Option<f64>,
    pub activation_abs_gt_1e_10: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClaimAssessment {
    pub target_slots_all_edited: bool,
    pub atlantis_output_changed: bool,
    pub atlantis_poseidon_erased: bool,
    pub france_bit_perfect_invariant: bool,
    pub france_output_changed: bool,
    pub france_hidden_changed: bool,
    pub claim_supported: bool,
    pub claim_refuted: bool,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExperimentReport {
    pub hypothesis: String,
    pub mechanistic_facts: Vec<String>,
    pub claim_assessment: ClaimAssessment,
    pub environment: BTreeMap<String, String>,
    pub vindex_path: String,
    pub num_layers: usize,
    pub hidden_size: usize,
    pub vocab_size: usize,
    pub model_has_weights: bool,
    pub slots: Vec<SlotEdit>,
    pub comparisons: Vec<RunComparison>,
    pub toolchain: String,
}

#[derive(Clone, Copy)]
struct Slot {
    layer: usize,
    feature: usize,
}

struct RawInferenceRun {
    public: InferenceRun,
    final_hidden: Vec<f32>,
    ffn_inputs: Vec<(usize, Vec<f32>)>,
}

pub fn run_bit_perfect_eraser_experiment(
    request: &BitPerfectEraserRequest,
) -> Result<ExperimentReport, ExperimentError> {
    if request.top_k == 0 {
        return Err("top_k must be >= 1".into());
    }
    if !Path::new(&request.vindex_path).is_dir() {
        return Err(format!("vindex path does not exist: {:?}", request.vindex_path).into());
    }

    let config = load_vindex_config(&request.vindex_path)?;
    if !config.has_model_weights {
        return Err("vindex does not contain model weights; rebuild with --include-weights".into());
    }

    let mut callbacks = SilentLoadCallbacks;
    let weights = load_model_weights(&request.vindex_path, &mut callbacks)?;
    let tokenizer = load_vindex_tokenizer(&request.vindex_path)?;

    let index = VectorIndex::load_vindex(&request.vindex_path, &mut callbacks)?;
    let mut patched = PatchedVindex::new(index.clone());
    let slot_edits =
        slot_edits_and_apply(&index, &target_slot_ids(), config.hidden_size, &mut patched);

    let baseline = PatchedVindex::new(index);
    let comparisons = build_comparisons(
        &target_prompts(),
        request.top_k,
        &baseline,
        &patched,
        &weights,
        tokenizer.as_ref(),
        &patched.base,
        &target_slot_ids(),
    )?;
    let claim_assessment = assess_claim(&slot_edits, &comparisons);

    let mut env_vars = BTreeMap::new();
    env_vars.insert(
        "LARQL_VINDEX__PATH".into(),
        request.vindex_path.display().to_string(),
    );
    env_vars.insert("LARQL_BIT_PERFECT_TOP_K".into(), request.top_k.to_string());

    Ok(ExperimentReport {
        hypothesis:
            "Surgical Isolation by zeroing slots 24:1618, 25:3262, 23:1532 and Bit-Perfect Invariance for France."
                .to_string(),
        mechanistic_facts: mechanistic_facts(),
        claim_assessment,
        environment: env_report(env_vars),
        vindex_path: request.vindex_path.display().to_string(),
        num_layers: config.num_layers,
        hidden_size: config.hidden_size,
        vocab_size: config.vocab_size,
        model_has_weights: config.has_model_weights,
        slots: slot_edits,
        comparisons,
        toolchain: "larql-inference experiment module (Rust, no python)".to_string(),
    })
}

fn mechanistic_facts() -> Vec<String> {
    vec![
        "Model forward pass is local CPU attention + FFN compute.".to_string(),
        "HTTP transport is optional and only used for remote knowledge lookup surfaces.".to_string(),
        "Generation requires full attention compute per layer; it is not pure vector retrieval."
            .to_string(),
        "Knowledge probes in this protocol are evaluated through gate and logit computations."
            .to_string(),
        "FFN slots are writable via linear-algebra patching, but bit-perfect eraser invariance must be tested empirically."
            .to_string(),
    ]
}

fn target_slot_ids() -> Vec<Slot> {
    vec![
        Slot {
            layer: 24,
            feature: 1618,
        },
        Slot {
            layer: 25,
            feature: 3262,
        },
        Slot {
            layer: 23,
            feature: 1532,
        },
    ]
}

fn target_prompts() -> Vec<&'static str> {
    vec!["The capital of Atlantis is", "The capital of France is"]
}

fn target_tokens() -> Vec<&'static str> {
    vec!["Pose", "Poseid", "Poseidon", "Paris", "said"]
}

fn env_report(mut map: BTreeMap<String, String>) -> BTreeMap<String, String> {
    map.insert("OS".into(), std::env::consts::OS.to_string());
    map.insert("ARCH".into(), std::env::consts::ARCH.to_string());
    map.insert(
        "CC".into(),
        std::env::var("CC").unwrap_or_else(|_| "unset".into()),
    );
    map.insert(
        "CARGO_CFG_TARGET_ARCH".into(),
        std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "unknown".into()),
    );
    map
}

fn run_infer(
    prompt: &str,
    top_k: usize,
    patched: &PatchedVindex,
    weights: &crate::ModelWeights,
    tokenizer: &dyn larql_tokenizer::Tokenizer,
) -> Result<RawInferenceRun, ExperimentError> {
    let encoding = tokenizer.encode(prompt, true)?;
    let token_ids = encoding.ids;
    // Keep protocol runtime bounded on CPU while preserving sparse-walk behavior.
    let ffn_top_k = top_k.max(512);
    let walk_ffn = WalkFfn::new_with_trace(weights, patched, ffn_top_k);

    let start = Instant::now();
    let result = predict_with_ffn_attention(weights, tokenizer, &token_ids, top_k, &walk_ffn);
    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
    let ffn_inputs = walk_ffn.take_residuals();

    let mut top_predictions = Vec::with_capacity(result.predictions.len());
    for (rank, (token, probability)) in result.predictions.into_iter().enumerate() {
        top_predictions.push(PredictionRecord {
            rank: rank + 1,
            token,
            probability,
        });
    }

    let mut target_records = Vec::with_capacity(target_tokens().len());
    for token in target_tokens() {
        let token_id = tokenizer.token_to_id(token);
        let probability = token_id.and_then(|id| {
            forward::hidden_vec_token_probability(weights, &result.final_hidden, id)
        });
        target_records.push(TargetTokenRecord {
            token: token.to_string(),
            token_id,
            probability,
        });
    }

    let final_hidden_len = result.final_hidden.len();
    let final_hidden_fnv1a64 = fnv1a64_f32(&result.final_hidden);
    Ok(RawInferenceRun {
        public: InferenceRun {
            prompt: prompt.to_string(),
            top_k,
            token_count: token_ids.len(),
            token_ids,
            duration_ms,
            top_predictions,
            target_tokens: target_records,
            final_hidden_len,
            final_hidden_fnv1a64,
        },
        final_hidden: result.final_hidden,
        ffn_inputs,
    })
}

fn fnv1a64_f32(values: &[f32]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for value in values {
        for byte in value.to_bits().to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("{hash:016x}")
}

fn target_token_deltas(
    baseline: &[TargetTokenRecord],
    patched: &[TargetTokenRecord],
) -> Vec<TargetTokenDelta> {
    baseline
        .iter()
        .zip(patched.iter())
        .map(|(left, right)| TargetTokenDelta {
            token: left.token.clone(),
            token_id: left.token_id.or(right.token_id),
            baseline_probability: left.probability,
            patched_probability: right.probability,
            delta: left
                .probability
                .zip(right.probability)
                .map(|(base, patch)| patch - base),
        })
        .collect()
}

fn slot_observations(
    prompt: &str,
    index: &VectorIndex,
    weights: &crate::ModelWeights,
    ffn_inputs: &[(usize, Vec<f32>)],
    slots: &[Slot],
) -> Vec<SlotObservation> {
    let mut observations = Vec::with_capacity(slots.len());
    for slot in slots {
        let Some((_, input)) = ffn_inputs.iter().find(|(layer, _)| *layer == slot.layer) else {
            observations.push(empty_slot_observation(prompt, slot));
            continue;
        };
        let residual = Array1::from_vec(input.clone());
        let gate_score = index
            .gate_vector(slot.layer, slot.feature)
            .map(|gate| dot(&gate, input) as f64);
        let gate_abs_rank = None;
        let up_score = index
            .up_layer_matrix(slot.layer)
            .filter(|matrix| slot.feature < matrix.shape()[0])
            .map(|matrix| matrix.row(slot.feature).dot(&residual) as f64);
        let activation = gate_score.zip(up_score).map(|(gate, up)| {
            let activated_gate = if matches!(
                weights.arch.activation(),
                larql_models::Activation::GeluTanh | larql_models::Activation::Gelu
            ) {
                crate::ffn::gelu_tanh(gate as f32) as f64
            } else {
                let gate_f32 = gate as f32;
                (gate_f32 * crate::ffn::sigmoid(gate_f32)) as f64
            };
            if weights.arch.ffn_type() == larql_models::FfnType::Gated {
                activated_gate * up
            } else {
                activated_gate
            }
        });
        observations.push(SlotObservation {
            prompt: prompt.to_string(),
            layer: slot.layer,
            feature: slot.feature,
            gate_score,
            gate_abs_rank,
            up_score,
            activation,
            activation_abs_gt_1e_10: activation.is_some_and(|value| value.abs() > 1e-10),
        });
    }
    observations
}

fn empty_slot_observation(prompt: &str, slot: &Slot) -> SlotObservation {
    SlotObservation {
        prompt: prompt.to_string(),
        layer: slot.layer,
        feature: slot.feature,
        gate_score: None,
        gate_abs_rank: None,
        up_score: None,
        activation: None,
        activation_abs_gt_1e_10: false,
    }
}

fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right.iter())
        .map(|(a, b)| a * b)
        .sum::<f32>()
}

fn assess_claim(slots: &[SlotEdit], comparisons: &[RunComparison]) -> ClaimAssessment {
    let target_slots_all_edited = slots.iter().all(|slot| slot.edited);
    let atlantis = comparisons
        .iter()
        .find(|comparison| comparison.prompt.contains("Atlantis"));
    let france = comparisons
        .iter()
        .find(|comparison| comparison.prompt.contains("France"));
    let atlantis_output_changed = atlantis
        .map(|comparison| {
            !comparison.baseline_top_predictions_equal
                || !comparison.baseline_final_hidden_bits_equal
                || comparison.max_abs_delta > 0.0
        })
        .unwrap_or(false);
    let atlantis_poseidon_erased = atlantis.map(atlantis_poseidon_erased).unwrap_or(false);
    let france_bit_perfect_invariant = france
        .map(|comparison| comparison.baseline_final_hidden_bits_equal)
        .unwrap_or(false);
    let france_output_changed = france
        .map(|comparison| !comparison.baseline_top_predictions_equal)
        .unwrap_or(false);
    let france_hidden_changed = france
        .map(|comparison| !comparison.baseline_final_hidden_bits_equal)
        .unwrap_or(false);
    let claim_supported =
        target_slots_all_edited && atlantis_poseidon_erased && france_bit_perfect_invariant;

    let mut notes = Vec::new();
    if !target_slots_all_edited {
        notes.push("At least one target slot was not editable in the supplied vindex.".to_string());
    }
    if !atlantis_output_changed {
        notes
            .push("Zeroing the named slots did not change the Atlantis prompt output.".to_string());
    }
    if let Some(comparison) = atlantis {
        let (baseline_poseidon, patched_poseidon) = poseidon_probabilities(comparison);
        notes.push(format!(
            "Atlantis Poseidon max-prob baseline={:?} patched={:?}.",
            baseline_poseidon, patched_poseidon
        ));
        let baseline_top = top_prediction_token(&comparison.baseline).unwrap_or("<none>");
        let patched_top = top_prediction_token(&comparison.patched).unwrap_or("<none>");
        notes.push(format!(
            "Atlantis top prediction baseline='{}' patched='{}'.",
            baseline_top, patched_top
        ));
    }
    if !atlantis_poseidon_erased {
        notes.push("Atlantis->Poseidon was not erased by the strict criterion.".to_string());
    }
    if !france_bit_perfect_invariant {
        notes.push("France did not remain bit-perfect invariant.".to_string());
    }

    ClaimAssessment {
        target_slots_all_edited,
        atlantis_output_changed,
        atlantis_poseidon_erased,
        france_bit_perfect_invariant,
        france_output_changed,
        france_hidden_changed,
        claim_supported,
        claim_refuted: !claim_supported,
        notes,
    }
}

fn atlantis_poseidon_erased(comparison: &RunComparison) -> bool {
    let (baseline_poseidon, patched_poseidon) = poseidon_probabilities(comparison);
    let top_token = top_prediction_token(&comparison.patched).unwrap_or_default();
    let top_is_poseidon = token_looks_poseidon(top_token);
    match (baseline_poseidon, patched_poseidon) {
        (Some(base), Some(patch)) if base > 0.0 => {
            let strong_drop = patch <= base * 1e-3;
            let near_zero = patch <= 1e-9;
            (strong_drop || near_zero) && !top_is_poseidon
        }
        _ => false,
    }
}

fn poseidon_probabilities(comparison: &RunComparison) -> (Option<f64>, Option<f64>) {
    let baseline = comparison
        .target_token_deltas
        .iter()
        .filter(|delta| token_looks_poseidon(&delta.token))
        .filter_map(|delta| delta.baseline_probability)
        .max_by(f64::total_cmp);
    let patched = comparison
        .target_token_deltas
        .iter()
        .filter(|delta| token_looks_poseidon(&delta.token))
        .filter_map(|delta| delta.patched_probability)
        .max_by(f64::total_cmp);
    (baseline, patched)
}

fn top_prediction_token(run: &InferenceRun) -> Option<&str> {
    run.top_predictions.first().map(|item| item.token.as_str())
}

fn token_looks_poseidon(token: &str) -> bool {
    token.to_ascii_lowercase().contains("poseidon")
}

fn vector_delta(left: &[f32], right: &[f32]) -> (bool, f64, f64, f64) {
    if left.len() != right.len() {
        return (false, f64::INFINITY, f64::INFINITY, f64::INFINITY);
    }
    let mut bits_equal = true;
    let mut max_abs = 0.0_f64;
    let mut l1 = 0.0_f64;
    let mut l2 = 0.0_f64;
    for (a, b) in left.iter().zip(right.iter()) {
        let delta = *a as f64 - *b as f64;
        max_abs = max_abs.max(delta.abs());
        l1 += delta.abs();
        l2 += delta * delta;
        if a.to_bits() != b.to_bits() {
            bits_equal = false;
        }
    }
    (bits_equal, max_abs, l1, l2.sqrt())
}

fn predict_top_equal(left: &[PredictionRecord], right: &[PredictionRecord]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter().zip(right.iter()).all(|(a, b)| {
        a.rank == b.rank && a.token == b.token && a.probability.to_bits() == b.probability.to_bits()
    })
}

fn slot_edits_and_apply(
    base: &VectorIndex,
    slots: &[Slot],
    hidden_size: usize,
    patched: &mut PatchedVindex,
) -> Vec<SlotEdit> {
    let mut out = Vec::new();
    for slot in slots {
        let in_layer_range = slot.layer < base.loaded_layers().len();
        let num_features = if in_layer_range {
            patched.num_features(slot.layer)
        } else {
            0
        };
        let in_feature_range = in_layer_range && slot.feature < num_features;
        let mut edit = SlotEdit {
            layer: slot.layer,
            feature: slot.feature,
            in_layer_range,
            in_feature_range,
            original_present: false,
            override_length: None,
            original_length: None,
            original_sum: None,
            original_l2: None,
            edited: false,
            note: String::new(),
        };

        if !in_layer_range {
            edit.note = "layer outside index range".into();
            out.push(edit);
            continue;
        }

        if !in_feature_range {
            edit.note = format!(
                "feature {} outside num_features {} at layer {}",
                slot.feature, num_features, slot.layer
            );
            out.push(edit);
            continue;
        }

        if let Some(original) = patched.down_feature_vector(slot.layer, slot.feature) {
            edit.original_present = true;
            edit.original_length = Some(original.len());
            let (sum, l2) = summarize_vec(original);
            edit.original_sum = Some(sum);
            edit.original_l2 = Some(l2);
        } else {
            edit.note =
                "no base down vector fetched (expected for sparse/memmapped layouts); applying override anyway".into();
        }

        let zero_vec = vec![0.0; hidden_size];
        patched.set_down_vector(slot.layer, slot.feature, zero_vec);
        edit.override_length = Some(hidden_size);
        edit.edited = true;
        if edit.note.is_empty() {
            edit.note = "zero override applied".into();
        } else {
            edit.note = format!("{}, then zero override applied", edit.note);
        }

        out.push(edit);
    }
    out
}

fn summarize_vec(vector: &[f32]) -> (f64, f64) {
    let sum = vector.iter().map(|v| *v as f64).sum();
    let l2 = vector
        .iter()
        .map(|v| (*v as f64) * (*v as f64))
        .sum::<f64>()
        .sqrt();
    (sum, l2)
}

fn build_comparisons(
    prompts: &[&str],
    top_k: usize,
    baseline: &PatchedVindex,
    patched: &PatchedVindex,
    weights: &crate::ModelWeights,
    tokenizer: &dyn larql_tokenizer::Tokenizer,
    index: &VectorIndex,
    slots: &[Slot],
) -> Result<Vec<RunComparison>, ExperimentError> {
    let mut comparisons = Vec::with_capacity(prompts.len());
    for prompt in prompts {
        let run_base = run_infer(prompt, top_k, baseline, weights, tokenizer)?;
        let run_patched = run_infer(prompt, top_k, patched, weights, tokenizer)?;

        let (bits_equal, max_abs_delta, l1_delta, l2_delta) =
            vector_delta(&run_base.final_hidden, &run_patched.final_hidden);
        let top_equal = predict_top_equal(
            &run_base.public.top_predictions,
            &run_patched.public.top_predictions,
        );
        let target_token_deltas = target_token_deltas(
            &run_base.public.target_tokens,
            &run_patched.public.target_tokens,
        );
        let slot_observations =
            slot_observations(prompt, index, weights, &run_base.ffn_inputs, slots);

        comparisons.push(RunComparison {
            prompt: (*prompt).to_string(),
            baseline_top_predictions_equal: top_equal,
            baseline_final_hidden_bits_equal: bits_equal,
            max_abs_delta,
            l1_delta,
            l2_delta,
            target_token_deltas,
            slot_observations,
            baseline: run_base.public,
            patched: run_patched.public,
        });
    }
    Ok(comparisons)
}
