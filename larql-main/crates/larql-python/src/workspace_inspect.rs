//! Workbench workspace inspection — implemented in Rust so UI Python stays thin.
//!
//! Returns a dict compatible with `larql_ui.ui.models.WorkspaceSummary` (same keys/types).

use std::path::Path;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use larql_lql::{parse, Session};

use crate::vindex::PyVindex;
use crate::walk::probe_walk_model;

const RELATION_LABEL_PREVIEW_CAP: usize = 60;
const TOP_TOKENS_PER_RELATION: usize = 8;
const PROBE_RELATION_PREVIEW_CAP: usize = 60;
const WALK_MODEL_PROBE_TOP_K: usize = 64;

fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut prev_sep = false;
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_sep = false;
        } else if !out.is_empty() && !prev_sep {
            out.push('-');
            prev_sep = true;
        }
    }
    let s = out.trim_matches('-').to_string();
    if s.is_empty() {
        "item".into()
    } else {
        s
    }
}

fn probe_lql(path: &str) -> Result<(), String> {
    let mut session = Session::new();
    let use_stmt = format!("USE \"{}\";", path.replace('"', "\\\""));
    let stmt = parse(&use_stmt).map_err(|e| format!("LQL parse (USE): {e}"))?;
    session
        .execute(&stmt)
        .map_err(|e| format!("LQL USE: {e}"))?;
    let stmt = parse("STATS;").map_err(|e| format!("LQL parse (STATS): {e}"))?;
    session
        .execute(&stmt)
        .map_err(|e| format!("LQL STATS: {e}"))?;
    Ok(())
}

/// Inspect a `.vindex` directory for the workbench: stats, relations, LQL/WalkModel probes, flags.
///
/// `mlx_available` must be supplied from Python (`importlib.util.find_spec`) — MLX is a Python stack.
#[pyfunction]
#[pyo3(signature = (path, mlx_available=false))]
pub fn inspect_workspace_for_ui(
    py: Python<'_>,
    path: &str,
    mlx_available: bool,
) -> PyResult<Py<PyDict>> {
    let norm_path = Path::new(path);
    if !norm_path.exists() {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
            "Workspace path does not exist: {path}"
        )));
    }
    if !norm_path.is_dir() {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
            "Workspace path is not directory: {path}"
        )));
    }
    if !norm_path.join("index.json").is_file() {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
            "Missing index.json in workspace: {path}"
        )));
    }

    let norm_str = norm_path
        .canonicalize()
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{e}")))?
        .to_string_lossy()
        .into_owned();

    let vindex = PyVindex::open(&norm_str)?;

    let display_name = norm_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("vindex")
        .to_string();

    let has_model_weights = vindex.config.has_model_weights;

    let all_rels = vindex.relations_list();
    let relation_count = all_rels.len();

    let relation_labels = PyList::empty(py);
    for rel in all_rels.into_iter().take(RELATION_LABEL_PREVIEW_CAP) {
        let d = PyDict::new(py);
        d.set_item("name", rel.name)?;
        d.set_item("count", rel.count)?;
        d.set_item("cluster_id", rel.cluster_id)?;
        let tops: Vec<String> = rel
            .top_tokens
            .into_iter()
            .take(TOP_TOKENS_PER_RELATION)
            .collect();
        d.set_item("top_tokens", tops)?;
        relation_labels.append(d)?;
    }

    let all_probe = vindex.probe_relations_list();
    let probe_relation_name_count = all_probe.len();

    let probe_relation_labels = PyList::empty(py);
    for pr in all_probe.into_iter().take(PROBE_RELATION_PREVIEW_CAP) {
        let d = PyDict::new(py);
        d.set_item("name", pr.name)?;
        d.set_item("count", pr.count)?;
        probe_relation_labels.append(d)?;
    }

    let num_clusters = vindex
        .classifier
        .as_ref()
        .map(|c| c.num_clusters())
        .unwrap_or(0);
    let num_probe_labels = vindex
        .classifier
        .as_ref()
        .map(|c| c.num_probe_labels())
        .unwrap_or(0);

    let has_relation_labels = relation_count > 0 || num_probe_labels > 0;
    let has_layer_bands = vindex.config.layer_bands.is_some();

    let mut supports_lql = false;
    let mut lql_probe_error: Option<String> = None;
    match probe_lql(&norm_str) {
        Ok(()) => supports_lql = true,
        Err(e) => lql_probe_error = Some(e),
    }

    let mut supports_walk_model = false;
    let mut walk_model_error: Option<String> = None;
    if has_model_weights {
        match probe_walk_model(&norm_str, WALK_MODEL_PROBE_TOP_K) {
            Ok(()) => supports_walk_model = true,
            Err(e) => walk_model_error = Some(e),
        }
    }

    let mut warnings: Vec<String> = Vec::new();
    if !supports_lql {
        let detail = lql_probe_error.unwrap_or_else(|| "unknown error".into());
        warnings.push(format!("LQL session probe failed: {detail}"));
    }
    if has_model_weights && !supports_walk_model {
        let detail = walk_model_error.unwrap_or_else(|| "unknown error".into());
        warnings.push(format!("WalkModel load failed: {detail}"));
    }
    if !has_model_weights {
        if supports_lql {
            warnings.push(
                "No model weights. Browse and LQL ready. Inference, trace, and mmap WalkModel disabled."
                    .into(),
            );
        } else {
            warnings
                .push("No model weights. Inference, trace, and mmap WalkModel disabled.".into());
        }
    }
    if relation_count == 0 && num_probe_labels == 0 {
        warnings.push(
            "No cluster relation catalogue (relations() empty) and no probe labels — describe edges may be sparse."
                .into(),
        );
    } else if relation_count == 0 && num_probe_labels > 0 {
        warnings.push(
            "No cluster catalogue (relations() empty); probe labels present — describe may still show relations with source=probe."
                .into(),
        );
    }
    if !mlx_available {
        warnings.push("MLX deps missing. MLX generation disabled.".into());
    }

    let warnings_py = PyList::empty(py);
    for w in &warnings {
        warnings_py.append(w)?;
    }

    let out = PyDict::new(py);
    out.set_item("id", slugify(&display_name))?;
    out.set_item("path", &norm_str)?;
    out.set_item("display_name", &display_name)?;
    out.set_item("model", &vindex.config.model)?;
    out.set_item("family", &vindex.config.family)?;
    out.set_item("num_layers", vindex.config.num_layers)?;
    out.set_item("hidden_size", vindex.config.hidden_size)?;
    out.set_item("vocab_size", vindex.config.vocab_size)?;
    out.set_item("extract_level", vindex.config.extract_level.to_string())?;
    out.set_item("has_model_weights", has_model_weights)?;
    out.set_item("has_relation_labels", has_relation_labels)?;
    out.set_item("has_layer_bands", has_layer_bands)?;
    out.set_item("supports_infer", has_model_weights)?;
    out.set_item("supports_trace", has_model_weights)?;
    out.set_item("supports_mlx", mlx_available && has_model_weights)?;
    out.set_item("supports_streaming", mlx_available && has_model_weights)?;
    out.set_item("supports_walk_ffn", mlx_available && has_model_weights)?;
    out.set_item("supports_lql", supports_lql)?;
    out.set_item("supports_walk_model", supports_walk_model)?;
    out.set_item("warnings", warnings_py)?;
    out.set_item("relation_count", relation_count)?;
    out.set_item("relation_labels", relation_labels)?;
    out.set_item("num_clusters", num_clusters)?;
    out.set_item("num_probe_labels", num_probe_labels)?;
    out.set_item("probe_relation_name_count", probe_relation_name_count)?;
    out.set_item("probe_relation_labels", probe_relation_labels)?;

    Ok(out.unbind())
}
