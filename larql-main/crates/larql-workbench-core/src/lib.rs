use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use larql_inference::{run_bit_perfect_eraser_experiment, BitPerfectEraserRequest};
use larql_lql::ast::quote_string;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LqlRunRequest {
    pub workspace_path: String,
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LqlRunResponse {
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescribeRunRequest {
    pub workspace_path: String,
    pub entity: String,
    pub band: Option<String>,
    pub verbose: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescribeRunResponse {
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeInferRequest {
    pub server_url: String,
    pub prompt: String,
    pub top_k: usize,
    pub mode: String,
    pub truth_spans: Vec<String>,
    pub materially_false_spans: Vec<String>,
    pub coherence_markers: Vec<String>,
    pub max_generated_tokens: Option<usize>,
    pub ridge_dead_zone: Option<f32>,
}

pub type AnalyzeInferResponse = larql_inference::AnalysisResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitPerfectEraserRunRequest {
    pub workspace_path: String,
    pub top_k: usize,
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitPerfectEraserRunResponse {
    pub summary: String,
    pub output_path: String,
    pub duration_ms: u64,
    pub artifact: larql_inference::ExperimentReport,
}

#[derive(Debug, Error)]
pub enum WorkbenchCoreError {
    #[error("workspace path is required")]
    MissingWorkspacePath,
    #[error("query is required")]
    MissingQuery,
    #[error("workspace path does not exist: {0}")]
    WorkspaceMissing(String),
    #[error("workspace path is not a directory: {0}")]
    WorkspaceNotDirectory(String),
    #[error("LQL parse failed: {0}")]
    Parse(String),
    #[error("LQL execution failed: {0}")]
    Execute(String),
    #[error("entity is required")]
    MissingEntity,
    #[error("unsupported band: {0} (expected: all, syntax, knowledge, output)")]
    InvalidBand(String),
    #[error("server url is required")]
    MissingServerUrl,
    #[error("prompt is required")]
    MissingPrompt,
    #[error("top_k must be greater than 0")]
    InvalidTopK,
    #[error("analysis mode is required (fact_probe or workflow_probe)")]
    MissingAnalysisMode,
    #[error("unsupported analysis mode: {0} (expected fact_probe or workflow_probe)")]
    InvalidAnalysisMode(String),
    #[error("MODE FACT_PROBE requires at least TRUTH_SPANS or FALSE_SPANS")]
    MissingFactProbeSpans,
    #[error("MODE WORKFLOW_PROBE requires FALSE_SPANS")]
    MissingWorkflowProbeFalseSpans,
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("top_k must be greater than 0")]
    InvalidExperimentTopK,
    #[error("experiment failed: {0}")]
    Experiment(String),
    #[error("failed to write experiment artifact: {0}")]
    ArtifactWrite(String),
}

pub fn run_lql_query(req: &LqlRunRequest) -> Result<LqlRunResponse, WorkbenchCoreError> {
    let workspace_path = req.workspace_path.trim();
    let query = req.query.trim();

    if workspace_path.is_empty() {
        return Err(WorkbenchCoreError::MissingWorkspacePath);
    }
    if query.is_empty() {
        return Err(WorkbenchCoreError::MissingQuery);
    }

    let workspace = Path::new(workspace_path);
    if !workspace.exists() {
        return Err(WorkbenchCoreError::WorkspaceMissing(
            workspace_path.to_string(),
        ));
    }
    if !workspace.is_dir() {
        return Err(WorkbenchCoreError::WorkspaceNotDirectory(
            workspace_path.to_string(),
        ));
    }

    let mut session = larql_lql::Session::new();

    let use_stmt = format!("USE {};", quote_string(workspace_path));
    let use_parsed =
        larql_lql::parse(&use_stmt).map_err(|e| WorkbenchCoreError::Parse(e.to_string()))?;
    session
        .execute(&use_parsed)
        .map_err(|e| WorkbenchCoreError::Execute(e.to_string()))?;

    let normalized_query = if query.ends_with(';') {
        query.to_string()
    } else {
        format!("{query};")
    };
    let parsed = larql_lql::parse(&normalized_query)
        .map_err(|e| WorkbenchCoreError::Parse(e.to_string()))?;
    let lines = session
        .execute(&parsed)
        .map_err(|e| WorkbenchCoreError::Execute(e.to_string()))?;

    Ok(LqlRunResponse { lines })
}

pub fn run_describe(req: &DescribeRunRequest) -> Result<DescribeRunResponse, WorkbenchCoreError> {
    let workspace_path = req.workspace_path.trim();
    let entity = req.entity.trim();

    if workspace_path.is_empty() {
        return Err(WorkbenchCoreError::MissingWorkspacePath);
    }
    if entity.is_empty() {
        return Err(WorkbenchCoreError::MissingEntity);
    }

    validate_workspace_path(workspace_path)?;

    let mut session = larql_lql::Session::new();
    open_workspace(&mut session, workspace_path)?;

    let mut statement = format!("DESCRIBE {}", quote_string(entity));
    if let Some(band_raw) = req.band.as_deref() {
        let band = band_raw.trim().to_ascii_lowercase();
        match band.as_str() {
            "all" => statement.push_str(" ALL LAYERS"),
            "syntax" => statement.push_str(" SYNTAX"),
            "knowledge" => statement.push_str(" KNOWLEDGE"),
            "output" => statement.push_str(" OUTPUT"),
            "" => {}
            other => return Err(WorkbenchCoreError::InvalidBand(other.to_string())),
        }
    }
    if req.verbose {
        statement.push_str(" VERBOSE");
    }
    statement.push(';');

    let parsed =
        larql_lql::parse(&statement).map_err(|e| WorkbenchCoreError::Parse(e.to_string()))?;
    let lines = session
        .execute(&parsed)
        .map_err(|e| WorkbenchCoreError::Execute(e.to_string()))?;

    Ok(DescribeRunResponse { lines })
}

pub fn run_analyze_infer(
    req: &AnalyzeInferRequest,
) -> Result<AnalyzeInferResponse, WorkbenchCoreError> {
    let server_url = req.server_url.trim();
    let prompt = req.prompt.trim();
    let mode = req.mode.trim().to_ascii_lowercase();

    if server_url.is_empty() {
        return Err(WorkbenchCoreError::MissingServerUrl);
    }
    if prompt.is_empty() {
        return Err(WorkbenchCoreError::MissingPrompt);
    }
    if req.top_k == 0 {
        return Err(WorkbenchCoreError::InvalidTopK);
    }
    if mode.is_empty() {
        return Err(WorkbenchCoreError::MissingAnalysisMode);
    }
    match mode.as_str() {
        "fact_probe" => {
            if req.truth_spans.is_empty() && req.materially_false_spans.is_empty() {
                return Err(WorkbenchCoreError::MissingFactProbeSpans);
            }
        }
        "workflow_probe" => {
            if req.materially_false_spans.is_empty() {
                return Err(WorkbenchCoreError::MissingWorkflowProbeFalseSpans);
            }
        }
        _ => return Err(WorkbenchCoreError::InvalidAnalysisMode(mode)),
    }

    let mut session = larql_lql::Session::new();
    let use_remote_stmt = format!("USE REMOTE {};", quote_string(server_url));
    let use_remote_parsed =
        larql_lql::parse(&use_remote_stmt).map_err(|e| WorkbenchCoreError::Parse(e.to_string()))?;
    session
        .execute(&use_remote_parsed)
        .map_err(|e| WorkbenchCoreError::Execute(e.to_string()))?;

    let mode_clause = match mode.as_str() {
        "fact_probe" => "FACT_PROBE",
        "workflow_probe" => "WORKFLOW_PROBE",
        _ => unreachable!("mode validated above"),
    };
    let mut statement = format!(
        "ANALYZE INFER {} MODE {}",
        quote_string(prompt),
        mode_clause
    );
    if !req.truth_spans.is_empty() {
        statement.push_str(" TRUTH_SPANS (");
        statement.push_str(
            &req.truth_spans
                .iter()
                .map(|s| quote_string(s))
                .collect::<Vec<_>>()
                .join(", "),
        );
        statement.push(')');
    }
    if !req.materially_false_spans.is_empty() {
        statement.push_str(" FALSE_SPANS (");
        statement.push_str(
            &req.materially_false_spans
                .iter()
                .map(|s| quote_string(s))
                .collect::<Vec<_>>()
                .join(", "),
        );
        statement.push(')');
    }
    if !req.coherence_markers.is_empty() {
        statement.push_str(" COHERENCE_MARKERS (");
        statement.push_str(
            &req.coherence_markers
                .iter()
                .map(|s| quote_string(s))
                .collect::<Vec<_>>()
                .join(", "),
        );
        statement.push(')');
    }
    if let Some(max_generated_tokens) = req.max_generated_tokens {
        statement.push_str(&format!(" MAX_GENERATED_TOKENS {}", max_generated_tokens));
    }
    if let Some(ridge_dead_zone) = req.ridge_dead_zone {
        statement.push_str(&format!(" RIDGE_DEAD_ZONE {}", ridge_dead_zone));
    }
    statement.push_str(&format!(" TOP {} FORMAT JSON;", req.top_k));

    let parsed =
        larql_lql::parse(&statement).map_err(|e| WorkbenchCoreError::Parse(e.to_string()))?;
    let lines = session
        .execute(&parsed)
        .map_err(|e| WorkbenchCoreError::Execute(e.to_string()))?;
    let analysis_json = lines.join("\n");
    serde_json::from_str::<AnalyzeInferResponse>(&analysis_json)
        .map_err(|e| WorkbenchCoreError::InvalidResponse(e.to_string()))
}

pub fn run_bit_perfect_eraser(
    req: &BitPerfectEraserRunRequest,
) -> Result<BitPerfectEraserRunResponse, WorkbenchCoreError> {
    let workspace_path = req.workspace_path.trim();
    if workspace_path.is_empty() {
        return Err(WorkbenchCoreError::MissingWorkspacePath);
    }
    if req.top_k == 0 {
        return Err(WorkbenchCoreError::InvalidExperimentTopK);
    }
    validate_workspace_path(workspace_path)?;

    let artifact_path = resolve_experiment_output_path(workspace_path, req.top_k, &req.output_path);
    if let Some(parent) = artifact_path.parent() {
        fs::create_dir_all(parent).map_err(|e| WorkbenchCoreError::ArtifactWrite(e.to_string()))?;
    }

    let started = Instant::now();
    let mut artifact = run_bit_perfect_eraser_experiment(&BitPerfectEraserRequest {
        vindex_path: PathBuf::from(workspace_path),
        top_k: req.top_k,
    })
    .map_err(|e| WorkbenchCoreError::Experiment(e.to_string()))?;
    artifact.environment.insert(
        "LARQL_BIT_PERFECT_OUTPUT".into(),
        artifact_path.display().to_string(),
    );
    let duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;

    let artifact_bytes = serde_json::to_vec_pretty(&artifact)
        .map_err(|e| WorkbenchCoreError::InvalidResponse(e.to_string()))?;
    fs::write(&artifact_path, artifact_bytes)
        .map_err(|e| WorkbenchCoreError::ArtifactWrite(e.to_string()))?;

    let outcomes = artifact
        .comparisons
        .iter()
        .map(|comparison| {
            format!(
                "{}: top={} bits={} max_delta={:.6e}",
                comparison.prompt,
                comparison.baseline_top_predictions_equal,
                comparison.baseline_final_hidden_bits_equal,
                comparison.max_abs_delta
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let summary = if outcomes.is_empty() {
        format!(
            "Bit-Perfect Eraser completed. top_k={}; output={}",
            req.top_k,
            artifact_path.display()
        )
    } else {
        format!(
            "Bit-Perfect Eraser completed. top_k={}; output={}; {}",
            req.top_k,
            artifact_path.display(),
            outcomes
        )
    };

    Ok(BitPerfectEraserRunResponse {
        summary,
        output_path: artifact_path.display().to_string(),
        duration_ms,
        artifact,
    })
}

fn validate_workspace_path(workspace_path: &str) -> Result<(), WorkbenchCoreError> {
    let workspace = Path::new(workspace_path);
    if !workspace.exists() {
        return Err(WorkbenchCoreError::WorkspaceMissing(
            workspace_path.to_string(),
        ));
    }
    if !workspace.is_dir() {
        return Err(WorkbenchCoreError::WorkspaceNotDirectory(
            workspace_path.to_string(),
        ));
    }
    Ok(())
}

fn open_workspace(
    session: &mut larql_lql::Session,
    workspace_path: &str,
) -> Result<(), WorkbenchCoreError> {
    let use_stmt = format!("USE {};", quote_string(workspace_path));
    let use_parsed =
        larql_lql::parse(&use_stmt).map_err(|e| WorkbenchCoreError::Parse(e.to_string()))?;
    session
        .execute(&use_parsed)
        .map_err(|e| WorkbenchCoreError::Execute(e.to_string()))?;
    Ok(())
}

fn resolve_experiment_output_path(
    workspace_path: &str,
    top_k: usize,
    output_path: &Option<String>,
) -> PathBuf {
    let repo_root = workspace_root();
    if let Some(raw) = output_path.as_deref() {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            let mut path = PathBuf::from(trimmed);
            if !path.is_absolute() {
                path = repo_root.join(path);
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                path.set_extension("json");
            }
            return path;
        }
    }

    let workspace_name = Path::new(workspace_path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(safe_slug)
        .unwrap_or_else(|| "vindex".to_string());
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    repo_root.join(format!(
        "docs/scientific-alignment/bit-perfect-eraser-{workspace_name}-{timestamp}-top{top_k}.json"
    ))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn safe_slug(value: &str) -> String {
    let slug = value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches(['-', '_'])
        .to_string();
    if slug.is_empty() {
        "item".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::{
        run_analyze_infer, run_bit_perfect_eraser, run_describe, run_lql_query,
        AnalyzeInferRequest, BitPerfectEraserRunRequest, DescribeRunRequest, LqlRunRequest,
        WorkbenchCoreError,
    };

    fn fixture_workspace_path() -> String {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        manifest
            .join("../larql-python/tests/fixtures/ui_walk_trace_vindex")
            .to_string_lossy()
            .to_string()
    }

    #[test]
    fn run_lql_query_requires_workspace() {
        let err = run_lql_query(&LqlRunRequest {
            workspace_path: "".into(),
            query: "STATS".into(),
        })
        .unwrap_err();
        assert!(matches!(err, WorkbenchCoreError::MissingWorkspacePath));
    }

    #[test]
    fn run_lql_query_requires_query() {
        let err = run_lql_query(&LqlRunRequest {
            workspace_path: fixture_workspace_path(),
            query: " ".into(),
        })
        .unwrap_err();
        assert!(matches!(err, WorkbenchCoreError::MissingQuery));
    }

    #[test]
    fn run_lql_query_executes_against_workspace() {
        let out = run_lql_query(&LqlRunRequest {
            workspace_path: fixture_workspace_path(),
            query: "STATS".into(),
        })
        .expect("query should run");
        assert!(!out.lines.is_empty());
    }

    #[test]
    fn run_describe_requires_entity() {
        let err = run_describe(&DescribeRunRequest {
            workspace_path: fixture_workspace_path(),
            entity: "".into(),
            band: None,
            verbose: false,
        })
        .unwrap_err();
        assert!(matches!(err, WorkbenchCoreError::MissingEntity));
    }

    #[test]
    fn run_describe_rejects_invalid_band() {
        let err = run_describe(&DescribeRunRequest {
            workspace_path: fixture_workspace_path(),
            entity: "France".into(),
            band: Some("invalid".into()),
            verbose: false,
        })
        .unwrap_err();
        assert!(matches!(err, WorkbenchCoreError::InvalidBand(_)));
    }

    #[test]
    fn run_describe_executes_against_workspace() {
        let out = run_describe(&DescribeRunRequest {
            workspace_path: fixture_workspace_path(),
            entity: "France".into(),
            band: Some("knowledge".into()),
            verbose: false,
        })
        .expect("describe should run");
        assert!(!out.lines.is_empty());
    }

    #[test]
    fn run_analyze_infer_requires_server_url() {
        let err = run_analyze_infer(&AnalyzeInferRequest {
            server_url: "".into(),
            prompt: "test".into(),
            top_k: 5,
            mode: "fact_probe".into(),
            truth_spans: vec![],
            materially_false_spans: vec![],
            coherence_markers: vec![],
            max_generated_tokens: None,
            ridge_dead_zone: None,
        })
        .unwrap_err();
        assert!(matches!(err, WorkbenchCoreError::MissingServerUrl));
    }

    #[test]
    fn run_analyze_infer_requires_prompt() {
        let err = run_analyze_infer(&AnalyzeInferRequest {
            server_url: "http://127.0.0.1:8080".into(),
            prompt: " ".into(),
            top_k: 5,
            mode: "fact_probe".into(),
            truth_spans: vec![],
            materially_false_spans: vec![],
            coherence_markers: vec![],
            max_generated_tokens: None,
            ridge_dead_zone: None,
        })
        .unwrap_err();
        assert!(matches!(err, WorkbenchCoreError::MissingPrompt));
    }

    #[test]
    fn run_analyze_infer_requires_mode() {
        let err = run_analyze_infer(&AnalyzeInferRequest {
            server_url: "http://127.0.0.1:8080".into(),
            prompt: "test".into(),
            top_k: 5,
            mode: "".into(),
            truth_spans: vec!["truth".into()],
            materially_false_spans: vec![],
            coherence_markers: vec![],
            max_generated_tokens: None,
            ridge_dead_zone: None,
        })
        .unwrap_err();
        assert!(matches!(err, WorkbenchCoreError::MissingAnalysisMode));
    }

    #[test]
    fn run_analyze_infer_fact_probe_requires_spans() {
        let err = run_analyze_infer(&AnalyzeInferRequest {
            server_url: "http://127.0.0.1:8080".into(),
            prompt: "test".into(),
            top_k: 5,
            mode: "fact_probe".into(),
            truth_spans: vec![],
            materially_false_spans: vec![],
            coherence_markers: vec![],
            max_generated_tokens: None,
            ridge_dead_zone: None,
        })
        .unwrap_err();
        assert!(matches!(err, WorkbenchCoreError::MissingFactProbeSpans));
    }

    #[test]
    fn run_analyze_infer_workflow_probe_requires_false_spans() {
        let err = run_analyze_infer(&AnalyzeInferRequest {
            server_url: "http://127.0.0.1:8080".into(),
            prompt: "test".into(),
            top_k: 5,
            mode: "workflow_probe".into(),
            truth_spans: vec!["truth".into()],
            materially_false_spans: vec![],
            coherence_markers: vec![],
            max_generated_tokens: None,
            ridge_dead_zone: None,
        })
        .unwrap_err();
        assert!(matches!(
            err,
            WorkbenchCoreError::MissingWorkflowProbeFalseSpans
        ));
    }

    #[test]
    fn run_analyze_infer_rejects_unknown_mode() {
        let err = run_analyze_infer(&AnalyzeInferRequest {
            server_url: "http://127.0.0.1:8080".into(),
            prompt: "test".into(),
            top_k: 5,
            mode: "other_probe".into(),
            truth_spans: vec!["truth".into()],
            materially_false_spans: vec![],
            coherence_markers: vec![],
            max_generated_tokens: None,
            ridge_dead_zone: None,
        })
        .unwrap_err();
        assert!(matches!(err, WorkbenchCoreError::InvalidAnalysisMode(_)));
    }

    #[test]
    fn run_analyze_infer_requires_positive_top_k() {
        let err = run_analyze_infer(&AnalyzeInferRequest {
            server_url: "http://127.0.0.1:8080".into(),
            prompt: "test".into(),
            top_k: 0,
            mode: "fact_probe".into(),
            truth_spans: vec!["truth".into()],
            materially_false_spans: vec![],
            coherence_markers: vec![],
            max_generated_tokens: None,
            ridge_dead_zone: None,
        })
        .unwrap_err();
        assert!(matches!(err, WorkbenchCoreError::InvalidTopK));
    }

    #[test]
    fn run_bit_perfect_eraser_requires_workspace() {
        let err = run_bit_perfect_eraser(&BitPerfectEraserRunRequest {
            workspace_path: "".into(),
            top_k: 40,
            output_path: None,
        })
        .unwrap_err();
        assert!(matches!(err, WorkbenchCoreError::MissingWorkspacePath));
    }

    #[test]
    fn run_bit_perfect_eraser_requires_positive_top_k() {
        let err = run_bit_perfect_eraser(&BitPerfectEraserRunRequest {
            workspace_path: fixture_workspace_path(),
            top_k: 0,
            output_path: None,
        })
        .unwrap_err();
        assert!(matches!(err, WorkbenchCoreError::InvalidExperimentTopK));
    }
}
