//! CLI wrapper for the Rust-native Bit-Perfect Eraser experiment.

use std::env;
use std::fs;
use std::path::PathBuf;

use larql_inference::{run_bit_perfect_eraser_experiment, BitPerfectEraserRequest};

type DynError = Box<dyn std::error::Error + Send + Sync + 'static>;

fn parse_vindex_path() -> Result<PathBuf, DynError> {
    if let Ok(path) = env::var("LARQL_VINDEX__PATH") {
        return Ok(PathBuf::from(path));
    }
    if let Some(arg) = env::args().nth(1) {
        return Ok(PathBuf::from(arg));
    }
    Err("LARQL_VINDEX__PATH is not set and no CLI path argument was provided".into())
}

fn output_path() -> PathBuf {
    env::var("LARQL_BIT_PERFECT_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("docs/scientific-alignment/bit-perfect-eraser-rust.json"))
}

fn top_k() -> usize {
    env::var("LARQL_BIT_PERFECT_TOP_K")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(40)
}

fn main() -> Result<(), DynError> {
    let output = output_path();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let report = run_bit_perfect_eraser_experiment(&BitPerfectEraserRequest {
        vindex_path: parse_vindex_path()?,
        top_k: top_k(),
    })?;
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    println!("Wrote evidence artifact: {}", output.display());
    Ok(())
}
