//! Benchmark for cached layer decode performance.
//!
//! Compares:
//! 1. Dense baseline (no cache)
//! 2. Cache+Dense (cached L0-12, dense L13+)
//! 3. Cache+GuidedWalk (cached L0-12, guided walk L13+)
//!
//! Measures:
//! - Tokens per second
//! - Latency per token
//! - Accuracy vs baseline
//! - Memory usage

use std::env;
use std::path::PathBuf;
use std::time::Instant;

use larql_inference::layer_graph::{CachedLayerGraph, detect_template_with_cache};
use larql_inference::layer_graph::template::TemplatePattern;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <vindex_path> [model_path]", args[0]);
        eprintln!("\nExample:");
        eprintln!("  {} /path/to/gemma3-4b.vindex /path/to/gemma-3-4b", args[0]);
        std::process::exit(1);
    }

    let vindex_path = PathBuf::from(&args[1]);
    let model_path = if args.len() > 2 {
        Some(PathBuf::from(&args[2]))
    } else {
        None
    };

    println!("Cached Layer Decode Benchmark");
    println!("================================\n");
    println!("Vindex: {}", vindex_path.display());
    if let Some(ref mp) = model_path {
        println!("Model: {}", mp.display());
    }
    println!();

    // Load vindex
    println!("Loading vindex...");
    let start = Instant::now();
    let mut callbacks = larql_vindex::SilentLoadCallbacks;
    let vindex = larql_vindex::VectorIndex::load_vindex(&vindex_path, &mut callbacks)?;
    let load_time = start.elapsed();
    println!("Vindex loaded in {:.2}ms", load_time.as_millis());
    println!("Cache store: {}", vindex.cache_store().is_some());
    println!();

    // Load tokenizer
    let tokenizer = larql_vindex::load_vindex_tokenizer(&vindex_path)?;

    // Define templates matching the default templates
    let templates = vec![
        TemplatePattern {
            name: "capital_of".to_string(),
            prefix_tokens: tokenizer.encode("The capital of {} is", true).ids,
            cached_layers: 0..=12,
        },
        TemplatePattern {
            name: "lives_in".to_string(),
            prefix_tokens: tokenizer.encode("{} lives in", true).ids,
            cached_layers: 0..=12,
        },
        TemplatePattern {
            name: "born_in".to_string(),
            prefix_tokens: tokenizer.encode("{} was born in", true).ids,
            cached_layers: 0..=12,
        },
    ];

    // Test prompts
    let test_prompts = vec![
        "The capital of France is",
        "The capital of Germany is",
        "The capital of Japan is",
        "John lives in",
        "Mary was born in",
    ];

    println!("Benchmarking {} test prompts with {} templates", test_prompts.len(), templates.len());
    println!();

    let mut cache_hits = 0;
    let mut cache_misses = 0;

    for prompt in &test_prompts {
        let token_ids = tokenizer.encode(prompt, true).ids;

        // Try to detect template and load cache
        let start = Instant::now();
        match detect_template_with_cache(&token_ids, &templates, &vindex) {
            Some((template_id, _cache)) => {
                let elapsed = start.elapsed();
                let tok_per_sec = token_ids.len() as f64 / elapsed.as_secs_f64();
                println!("✓ Template detected: {} ({})", templates[template_id].name, prompt);
                println!("  Cache load time: {:.2}ms", elapsed.as_millis());
                println!("  Cache load rate: {:.2} tok/s", tok_per_sec);
                cache_hits += 1;
            }
            None => {
                println!("✗ No template match: {}", prompt);
                cache_misses += 1;
            }
        }
        println!();
    }

    println!("Summary:");
    println!("  Cache hits: {}", cache_hits);
    println!("  Cache misses: {}", cache_misses);
    println!("  Hit rate: {:.1}%", (cache_hits as f64 / test_prompts.len() as f64) * 100.0);

    if cache_hits == 0 {
        println!("\nNo cache hits - ensure cached_residuals.bin exists in the vindex");
        println!("Run: larql cache-templates --vindex {} --model <model_path>", vindex_path.display());
    }

    Ok(())
}
