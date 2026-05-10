use std::path::PathBuf;
use std::time::Instant;

use clap::Args;
use indicatif::{ProgressBar, ProgressStyle};
use larql_inference::InferenceModel;
use larql_vindex::write_model_weights;
use larql_vindex::IndexBuildCallbacks;

#[derive(Args)]
pub struct ExtractIndexArgs {
    /// Model path or HuggingFace model ID (extracts directly from weights).
    /// Not needed if --from-vectors is used.
    model: Option<String>,

    /// Output path for the .vindex directory.
    #[arg(short, long)]
    output: PathBuf,

    /// Build from already-extracted NDJSON vector files instead of model weights.
    /// Point to the directory containing ffn_gate.vectors.jsonl, etc.
    #[arg(long)]
    from_vectors: Option<PathBuf>,

    /// Top-K tokens to store per feature in down metadata (only for model extraction).
    #[arg(long, default_value = "10")]
    down_top_k: usize,

    /// Extract level: browse (gate+embed+down_meta), inference (+attention+norms),
    /// all (+up+down+lm_head for COMPILE).
    #[arg(long, default_value = "browse", value_parser = parse_extract_level)]
    level: larql_vindex::ExtractLevel,

    /// Include full model weights. Alias for --level all (deprecated, use --level instead).
    #[arg(long)]
    include_weights: bool,

    /// Store weights in f16 (half precision). Halves file sizes with negligible accuracy loss.
    #[arg(long)]
    f16: bool,

    /// Skip stages that already have output files (resume interrupted builds).
    #[arg(long)]
    resume: bool,
}

fn parse_extract_level(s: &str) -> Result<larql_vindex::ExtractLevel, String> {
    match s.to_lowercase().as_str() {
        "browse" => Ok(larql_vindex::ExtractLevel::Browse),
        "inference" => Ok(larql_vindex::ExtractLevel::Inference),
        "all" => Ok(larql_vindex::ExtractLevel::All),
        _ => Err(format!(
            "unknown extract level: {s} (expected: browse, inference, all)"
        )),
    }
}

fn has_all_files(dir: &std::path::Path, names: &[&str]) -> bool {
    names.iter().all(|name| dir.join(name).exists())
}

fn has_browse_outputs_from_vectors(dir: &std::path::Path) -> bool {
    has_all_files(
        dir,
        &[
            "gate_vectors.bin",
            "embeddings.bin",
            "down_meta.jsonl",
            "index.json",
        ],
    )
}

fn has_browse_outputs_from_model(dir: &std::path::Path) -> bool {
    has_all_files(
        dir,
        &[
            "gate_vectors.bin",
            "embeddings.bin",
            "down_meta.bin",
            "tokenizer.json",
            "index.json",
        ],
    )
}

fn has_model_weight_outputs(dir: &std::path::Path) -> bool {
    dir.join("weight_manifest.json").exists()
}

fn merge_extract_level(
    existing: larql_vindex::ExtractLevel,
    requested: larql_vindex::ExtractLevel,
) -> larql_vindex::ExtractLevel {
    use larql_vindex::ExtractLevel;

    match (existing, requested) {
        (ExtractLevel::All, _) | (_, ExtractLevel::All) => ExtractLevel::All,
        (ExtractLevel::Inference, _) | (_, ExtractLevel::Inference) => ExtractLevel::Inference,
        _ => ExtractLevel::Browse,
    }
}

fn refresh_index_metadata(
    output_dir: &std::path::Path,
    requested_level: larql_vindex::ExtractLevel,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = larql_vindex::load_vindex_config(output_dir)?;
    config.extract_level = merge_extract_level(config.extract_level, requested_level);
    if config.extract_level != larql_vindex::ExtractLevel::Browse {
        config.has_model_weights = has_model_weight_outputs(output_dir);
    }
    config.checksums = larql_vindex::checksums::compute_checksums(output_dir).ok();
    let config_json = serde_json::to_string_pretty(&config)?;
    std::fs::write(output_dir.join("index.json"), config_json)?;
    Ok(())
}

fn maybe_resume_weight_stage(
    model_name: &str,
    output_dir: &std::path::Path,
    requested_level: larql_vindex::ExtractLevel,
    callbacks: &mut dyn IndexBuildCallbacks,
) -> Result<bool, Box<dyn std::error::Error>> {
    if requested_level == larql_vindex::ExtractLevel::Browse {
        return Ok(false);
    }

    if has_model_weight_outputs(output_dir) {
        eprintln!("Resuming: existing model weight outputs found, skipping weight stage.");
        refresh_index_metadata(output_dir, requested_level)?;
        return Ok(true);
    }

    eprintln!("\nLoading model for weights: {}", model_name);
    let model = InferenceModel::load(model_name)?;
    write_model_weights(model.weights(), output_dir, callbacks)?;
    refresh_index_metadata(output_dir, requested_level)?;
    Ok(true)
}

struct CliBuildCallbacks {
    stage_start: Option<Instant>,
    feature_bar: ProgressBar,
}

impl CliBuildCallbacks {
    fn new() -> Self {
        let feature_bar = ProgressBar::new(0);
        feature_bar.set_style(
            ProgressStyle::default_bar()
                .template("  {spinner} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("█▓░"),
        );
        feature_bar.set_draw_target(indicatif::ProgressDrawTarget::stderr());

        Self {
            stage_start: None,
            feature_bar,
        }
    }
}

impl IndexBuildCallbacks for CliBuildCallbacks {
    fn on_stage(&mut self, stage: &str) {
        self.feature_bar.finish_and_clear();
        eprintln!("\n── {stage} ──");
        self.stage_start = Some(Instant::now());
    }

    fn on_layer_start(&mut self, component: &str, layer: usize, total: usize) {
        self.feature_bar.reset();
        self.feature_bar
            .set_message(format!("{component} L{layer} ({}/{})", layer + 1, total));
    }

    fn on_feature_progress(&mut self, component: &str, _layer: usize, done: usize, total: usize) {
        if total > 0 {
            self.feature_bar.set_length(total as u64);
        }
        self.feature_bar.set_position(done as u64);
        if total == 0 {
            self.feature_bar
                .set_message(format!("{component} {done} records"));
        }
    }

    fn on_layer_done(&mut self, component: &str, layer: usize, elapsed_ms: f64) {
        self.feature_bar.finish_and_clear();
        eprintln!("  {component} L{layer:2}: {:.1}s", elapsed_ms / 1000.0);
    }

    fn on_stage_done(&mut self, stage: &str, _elapsed_ms: f64) {
        self.feature_bar.finish_and_clear();
        if let Some(start) = self.stage_start.take() {
            eprintln!("  {stage}: {:.1}s", start.elapsed().as_secs_f64());
        }
    }
}

pub fn run(args: ExtractIndexArgs) -> Result<(), Box<dyn std::error::Error>> {
    let mut callbacks = CliBuildCallbacks::new();
    let build_start = Instant::now();

    // Resolve extract level: --include-weights upgrades to All (backwards compat)
    let level = if args.include_weights {
        larql_vindex::ExtractLevel::All
    } else {
        args.level
    };

    let dtype = if args.f16 {
        larql_vindex::StorageDtype::F16
    } else {
        larql_vindex::StorageDtype::F32
    };

    if let Some(ref vectors_dir) = args.from_vectors {
        // Build from existing NDJSON files
        eprintln!("Building vindex from vectors: {}", vectors_dir.display());
        eprintln!("Output: {}", args.output.display());

        let browse_done = args.resume && has_browse_outputs_from_vectors(&args.output);
        if browse_done {
            eprintln!("Resuming: existing vector-built browse outputs found, skipping pack stage.");
        } else {
            larql_vindex::build_vindex_from_vectors(vectors_dir, &args.output, &mut callbacks)?;
        }

        if let Some(model_name) = args.model.as_deref() {
            let _ = maybe_resume_weight_stage(model_name, &args.output, level, &mut callbacks)?;
        } else if matches!(
            level,
            larql_vindex::ExtractLevel::Inference | larql_vindex::ExtractLevel::All
        ) {
            return Err(
                "--model required with --level inference/all (need model to extract weights)"
                    .into(),
            );
        } else if browse_done {
            refresh_index_metadata(&args.output, level)?;
        }
    } else {
        // Build from model — streaming mode (mmap safetensors, no full model load)
        let model_name = args
            .model
            .as_deref()
            .ok_or("Either provide a model name or use --from-vectors")?;

        let model_path = larql_models::resolve_model_path(model_name)?;

        let level_str = match level {
            larql_vindex::ExtractLevel::Browse => "browse",
            larql_vindex::ExtractLevel::Inference => "inference",
            larql_vindex::ExtractLevel::All => "all",
        };
        let dtype_str = match dtype {
            larql_vindex::StorageDtype::F32 => "f32",
            larql_vindex::StorageDtype::F16 => "f16",
        };
        eprintln!(
            "Extracting: {} → {} (level={}, dtype={})",
            model_path.display(),
            args.output.display(),
            level_str,
            dtype_str
        );

        let output = &args.output;

        if args.resume && has_browse_outputs_from_model(output) {
            eprintln!(
                "Resuming: existing model-built browse outputs found, skipping extraction stage."
            );
            let _ = maybe_resume_weight_stage(model_name, output, level, &mut callbacks)?;
            if level == larql_vindex::ExtractLevel::Browse {
                refresh_index_metadata(output, level)?;
            }
            callbacks.feature_bar.finish_and_clear();
            let build_elapsed = build_start.elapsed();

            // Print summary
            eprintln!("\n── Summary ──");
            eprintln!("  Output: {}", args.output.display());

            if build_elapsed.as_secs() >= 60 {
                eprintln!("  Build time: {:.1}min", build_elapsed.as_secs_f64() / 60.0);
            } else {
                eprintln!("  Build time: {:.1}s", build_elapsed.as_secs_f64());
            }

            for name in &[
                "index.json",
                "gate_vectors.bin",
                "embeddings.bin",
                "down_meta.jsonl",
                "down_meta.bin",
                "tokenizer.json",
                "attn_weights.bin",
                "up_weights.bin",
                "down_weights.bin",
                "norms.bin",
                "lm_head.bin",
                "weight_manifest.json",
            ] {
                let path = args.output.join(name);
                if let Ok(meta) = std::fs::metadata(&path) {
                    let size_mb = meta.len() as f64 / (1024.0 * 1024.0);
                    if size_mb > 1024.0 {
                        eprintln!("  {name}: {:.2} GB", size_mb / 1024.0);
                    } else if size_mb > 0.1 {
                        eprintln!("  {name}: {:.1} MB", size_mb);
                    } else {
                        let size_kb = meta.len() as f64 / 1024.0;
                        eprintln!("  {name}: {:.1} KB", size_kb);
                    }
                } else {
                    eprintln!("  {name}: (not found)");
                }
            }

            let total_size: u64 = std::fs::read_dir(&args.output)
                .ok()
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .filter_map(|e| e.metadata().ok())
                        .map(|m| m.len())
                        .sum()
                })
                .unwrap_or(0);
            eprintln!(
                "  Total: {:.2} GB",
                total_size as f64 / (1024.0 * 1024.0 * 1024.0)
            );

            eprintln!("\nUsage:");
            eprintln!(
                "  larql walk --index {} -p \"The capital of France is\"",
                args.output.display()
            );

            return Ok(());
        }

        // Find or create tokenizer
        let tok_path = model_path.join("tokenizer.json");
        let tokenizer = if tok_path.exists() {
            larql_tokenizer::load_tokenizer(&tok_path)
                .map_err(|e| format!("failed to load tokenizer: {e}"))?
        } else {
            return Err(format!("tokenizer.json not found at {}", model_path.display()).into());
        };

        larql_vindex::build_vindex_streaming(
            &model_path,
            &*tokenizer,
            model_name,
            output,
            args.down_top_k,
            level,
            dtype,
            &mut callbacks,
        )?;
    }

    callbacks.feature_bar.finish_and_clear();
    let build_elapsed = build_start.elapsed();

    // Print summary
    eprintln!("\n── Summary ──");
    eprintln!("  Output: {}", args.output.display());

    if build_elapsed.as_secs() >= 60 {
        eprintln!("  Build time: {:.1}min", build_elapsed.as_secs_f64() / 60.0);
    } else {
        eprintln!("  Build time: {:.1}s", build_elapsed.as_secs_f64());
    }

    for name in &[
        "index.json",
        "gate_vectors.bin",
        "embeddings.bin",
        "down_meta.jsonl",
        "down_meta.bin",
        "tokenizer.json",
        "attn_weights.bin",
        "up_weights.bin",
        "down_weights.bin",
        "norms.bin",
        "lm_head.bin",
        "weight_manifest.json",
    ] {
        let path = args.output.join(name);
        if let Ok(meta) = std::fs::metadata(&path) {
            let size_mb = meta.len() as f64 / (1024.0 * 1024.0);
            if size_mb > 1024.0 {
                eprintln!("  {name}: {:.2} GB", size_mb / 1024.0);
            } else if size_mb > 0.1 {
                eprintln!("  {name}: {:.1} MB", size_mb);
            } else {
                let size_kb = meta.len() as f64 / 1024.0;
                eprintln!("  {name}: {:.1} KB", size_kb);
            }
        } else {
            eprintln!("  {name}: (not found)");
        }
    }

    // Total: sum all files in the directory
    let total_size: u64 = std::fs::read_dir(&args.output)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0);
    eprintln!(
        "  Total: {:.2} GB",
        total_size as f64 / (1024.0 * 1024.0 * 1024.0)
    );

    eprintln!("\nUsage:");
    eprintln!(
        "  larql walk --index {} -p \"The capital of France is\"",
        args.output.display()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        has_browse_outputs_from_model, has_browse_outputs_from_vectors, has_model_weight_outputs,
        merge_extract_level,
    };
    use std::path::PathBuf;

    fn tempdir() -> PathBuf {
        let unique = format!(
            "larql-cli-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&path).expect("create tempdir");
        path
    }

    #[test]
    fn resume_detection_checks_vector_outputs() {
        let dir = tempdir();
        for name in [
            "gate_vectors.bin",
            "embeddings.bin",
            "down_meta.jsonl",
            "index.json",
        ] {
            std::fs::write(dir.join(name), []).expect("write fixture");
        }
        assert!(has_browse_outputs_from_vectors(&dir));
        assert!(!has_browse_outputs_from_model(&dir));
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn resume_detection_checks_model_outputs_and_weights() {
        let dir = tempdir();
        for name in [
            "gate_vectors.bin",
            "embeddings.bin",
            "down_meta.bin",
            "tokenizer.json",
            "index.json",
            "weight_manifest.json",
        ] {
            std::fs::write(dir.join(name), []).expect("write fixture");
        }
        assert!(has_browse_outputs_from_model(&dir));
        assert!(has_model_weight_outputs(&dir));
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn merge_extract_level_preserves_highest_capability() {
        use larql_vindex::ExtractLevel;

        assert_eq!(
            merge_extract_level(ExtractLevel::Browse, ExtractLevel::Inference),
            ExtractLevel::Inference
        );
        assert_eq!(
            merge_extract_level(ExtractLevel::All, ExtractLevel::Browse),
            ExtractLevel::All
        );
    }
}
