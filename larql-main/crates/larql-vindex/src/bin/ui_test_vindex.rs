//! Emit a minimal `.vindex` directory with full model weights for Python UI tests
//! (`larql-ui` trace / WalkModel). Same shape as `tests/test_vindex.rs` `make_synthetic_model`.
//!
//! Usage:
//!   cargo run -p larql-vindex --bin ui_test_vindex /path/to/out/dir
//!
//! The printed directory can be copied to `crates/larql-python/tests/fixtures/ui_walk_trace_vindex/`.

use std::collections::HashMap;
use std::env;
use std::path::Path;

use larql_tokenizer::HfTokenizer;
use ndarray::ArcArray2;

fn make_synthetic_model() -> larql_models::ModelWeights {
    let num_layers = 2;
    let hidden = 8;
    let intermediate = 4;
    let vocab_size = 16;

    let mut tensors: HashMap<String, ArcArray2<f32>> = HashMap::new();
    let mut vectors: HashMap<String, Vec<f32>> = HashMap::new();

    for layer in 0..num_layers {
        let mut gate = ndarray::Array2::<f32>::zeros((intermediate, hidden));
        for i in 0..intermediate {
            gate[[i, i % hidden]] = 1.0 + layer as f32;
        }
        tensors.insert(
            format!("layers.{layer}.mlp.gate_proj.weight"),
            gate.into_shared(),
        );

        let mut up = ndarray::Array2::<f32>::zeros((intermediate, hidden));
        for i in 0..intermediate {
            up[[i, (i + 1) % hidden]] = 0.5;
        }
        tensors.insert(
            format!("layers.{layer}.mlp.up_proj.weight"),
            up.into_shared(),
        );

        let mut down = ndarray::Array2::<f32>::zeros((hidden, intermediate));
        for i in 0..intermediate {
            down[[i % hidden, i]] = 0.3;
        }
        tensors.insert(
            format!("layers.{layer}.mlp.down_proj.weight"),
            down.into_shared(),
        );

        for suffix in &["q_proj", "k_proj", "v_proj", "o_proj"] {
            let mut attn = ndarray::Array2::<f32>::zeros((hidden, hidden));
            for i in 0..hidden {
                attn[[i, i]] = 1.0;
            }
            tensors.insert(
                format!("layers.{layer}.self_attn.{suffix}.weight"),
                attn.into_shared(),
            );
        }

        vectors.insert(
            format!("layers.{layer}.input_layernorm.weight"),
            vec![1.0; hidden],
        );
        vectors.insert(
            format!("layers.{layer}.post_attention_layernorm.weight"),
            vec![1.0; hidden],
        );
    }

    vectors.insert("norm.weight".into(), vec![1.0; hidden]);

    let mut embed = ndarray::Array2::<f32>::zeros((vocab_size, hidden));
    for i in 0..vocab_size {
        embed[[i, i % hidden]] = 1.0;
    }

    let embed = embed.into_shared();
    let lm_head = embed.clone();

    let arch = larql_models::detect_from_json(&serde_json::json!({
        "model_type": "llama",
        "hidden_size": hidden,
        "num_hidden_layers": num_layers,
        "intermediate_size": intermediate,
        "head_dim": hidden,
        "num_attention_heads": 1,
        "num_key_value_heads": 1,
        "rope_theta": 10000.0,
        "vocab_size": vocab_size,
    }));

    larql_models::ModelWeights {
        tensors,
        vectors,
        embed,
        lm_head,
        num_layers,
        hidden_size: hidden,
        intermediate_size: intermediate,
        vocab_size,
        head_dim: hidden,
        num_q_heads: 1,
        num_kv_heads: 1,
        rope_base: 10000.0,
        arch,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = env::args()
        .nth(1)
        .ok_or("usage: ui_test_vindex <output_dir>")?;
    let dir = Path::new(&out);
    if dir.exists() {
        return Err(format!("output path already exists: {}", dir.display()).into());
    }
    std::fs::create_dir_all(dir)?;

    let weights = make_synthetic_model();
    // Must match `vocab_size` (16) and produce non-empty encodings for trace (`WalkModel.trace`).
    let tok_json = include_str!("../../assets/ui_trace_tokenizer.json");
    std::fs::write(dir.join("tokenizer.json"), tok_json)?;

    let tokenizer = HfTokenizer::from_bytes(tok_json.as_bytes())?;
    let mut cb = larql_vindex::SilentBuildCallbacks;
    larql_vindex::build_vindex(
        &weights,
        &tokenizer,
        "test/synthetic-ui-trace",
        dir,
        5,
        larql_vindex::ExtractLevel::All,
        larql_vindex::StorageDtype::F32,
        &mut cb,
    )?;

    eprintln!("wrote test vindex with weights to {}", dir.display());
    Ok(())
}
