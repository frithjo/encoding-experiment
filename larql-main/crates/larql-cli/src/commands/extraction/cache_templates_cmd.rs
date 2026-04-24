//! Cache template residuals for fast inference.
//!
//! This command computes pre-computed layer residuals for common templates
//! and stores them in cached_residuals.bin for use during inference.

use std::path::PathBuf;

use clap::Parser;

use larql_inference::layer_graph::CachedLayerGraph;
use larql_models::load_model_dir;
use larql_tokenizer::Tokenizer;

/// Cache template residuals for fast inference.
#[derive(Parser, Debug)]
#[command(name = "cache-templates")]
pub struct CacheTemplatesCmd {
    /// Path to the vindex directory
    #[arg(long)]
    pub vindex: PathBuf,

    /// Path to the model directory (safetensors)
    #[arg(long)]
    pub model: PathBuf,

    /// Number of templates to cache (default: all)
    #[arg(long, default_value = "0")]
    pub num_templates: usize,

    /// Use f16 encoding for residuals (default: true)
    #[arg(long, default_value = "true")]
    pub f16: bool,

    /// Layer range to cache (default: 0-12)
    #[arg(long, value_parser = parse_layer_range)]
    pub layer_range: Option<std::ops::RangeInclusive<usize>>,
}

fn parse_layer_range(s: &str) -> Result<std::ops::RangeInclusive<usize>, String> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 2 {
        return Err("Expected format: START-END (e.g., 0-12)".to_string());
    }
    let start: usize = parts[0]
        .parse()
        .map_err(|e| format!("Invalid start: {}", e))?;
    let end: usize = parts[1]
        .parse()
        .map_err(|e| format!("Invalid end: {}", e))?;
    if start > end {
        return Err("Start must be <= end".to_string());
    }
    Ok(start..=end)
}

impl CacheTemplatesCmd {
    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        eprintln!("Loading vindex from: {}", self.vindex.display());
        eprintln!("Loading model from: {}", self.model.display());

        // Load vindex
        let mut callbacks = larql_vindex::SilentLoadCallbacks;
        let vindex = larql_vindex::VectorIndex::load_vindex(&self.vindex, &mut callbacks)?;

        // Check extract level
        let config = larql_vindex::load_vindex_config(&self.vindex)?;
        if config.extract_level < larql_vindex::ExtractLevel::Inference {
            return Err("Vindex must be at Inference level or higher to cache templates".into());
        }

        // Load model
        let weights = load_model_dir(&self.model)?;

        // Load tokenizer
        let tokenizer = larql_vindex::load_vindex_tokenizer(&self.vindex)?;

        // Get templates
        let templates = if self.num_templates > 0 {
            &larql_vindex::extract::DEFAULT_TEMPLATES[..self.num_templates.min(larql_vindex::extract::DEFAULT_TEMPLATES.len())]
        } else {
            larql_vindex::extract::DEFAULT_TEMPLATES
        };

        let layer_range = self.layer_range.clone().unwrap_or(0..=12);

        eprintln!("Caching {} templates with layer range {:?}", templates.len(), layer_range);

        // Create cache writer
        let cache_path = self.vindex.join("cached_residuals.bin");
        let dtype = if self.f16 {
            larql_vindex::CacheDtype::F16
        } else {
            larql_vindex::CacheDtype::F32
        };

        let mut cache_writer = larql_vindex::CacheWriter::create(
            &cache_path,
            config.hidden_size,
            config.num_layers,
            dtype,
            templates.len(),
        )?;

        // Compute residuals for each template
        for (template_id, template_def) in templates.iter().enumerate() {
            eprintln!("Computing residuals for template {}: {}", template_id, template_def.name);

            // Tokenize template
            let prompt = template_def.pattern;
            let encoding = tokenizer.encode(prompt, true)?;
            let token_ids: Vec<u32> = encoding.ids.clone();

            // Build cached residuals using CachedLayerGraph
            let cached_layers: Vec<usize> = layer_range.clone().collect();
            let cache = CachedLayerGraph::build(
                &weights,
                &token_ids,
                &cached_layers,
                &larql_inference::ffn::DenseFfn,
            );

            // Convert to storage format
            let mut residuals = Vec::new();
            for layer in layer_range.clone() {
                if let Some(residual) = cache.cache.get(&layer) {
                    // Convert Array2 to Vec<f32>
                    let residual_vec = residual.as_slice().to_vec();
                    residuals.push((layer, residual_vec));
                }
            }

            // Write to cache
            cache_writer.append_template(
                template_id,
                token_ids.len(),
                *layer_range.start(),
                *layer_range.end(),
                &residuals,
            )?;

            eprintln!("  Cached {} layers for template {}", residuals.len(), template_def.name);
        }

        // Finish writing
        cache_writer.finish()?;

        eprintln!("Cached residuals written to: {}", cache_path.display());
        eprintln!("Total templates cached: {}", cache_writer.n_templates());

        Ok(())
    }
}
