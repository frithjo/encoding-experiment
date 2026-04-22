//! Build interleaved Q4_K vindex file: gate + up + down packed per layer.
//!
//! Layout: [L0_gate | L0_up | L0_down | L1_gate | L1_up | L1_down | ...]
//! Each layer's 3 matrices are contiguous — one TLB region, one prefetch stream.
//! Quantized to Q4_K format (Ollama-compatible) for 7x size reduction.
//!
//! Reads from: gate_vectors.bin, up_features.bin, down_features.bin
//! Writes to:  interleaved_q4k.bin
//!
//! Usage:
//!   cargo run --release -p larql-vindex --example build_interleaved -- output/gemma3-4b-v2.vindex

use std::io::Write;
use std::path::Path;
use std::time::Instant;

use larql_core::mmap::Mmap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args()
        .nth(1)
        .ok_or("Usage: build_interleaved <vindex_dir>")?;
    let dir = Path::new(&dir);

    let config_text = std::fs::read_to_string(dir.join("index.json"))?;
    let config: serde_json::Value = serde_json::from_str(&config_text)?;
    let num_layers = config["num_layers"].as_u64().unwrap() as usize;
    let hidden_size = config["hidden_size"].as_u64().unwrap() as usize;
    let intermediate_size = config["intermediate_size"].as_u64().unwrap() as usize;

    let floats_per_matrix = intermediate_size * hidden_size;
    let bytes_per_matrix_f32 = floats_per_matrix * 4;
    let bytes_per_layer_f32 = bytes_per_matrix_f32 * 3; // gate + up + down

    println!("=== Build Interleaved Q4_K Vindex ===\n");
    println!("Layers: {num_layers}, hidden: {hidden_size}, intermediate: {intermediate_size}");
    println!(
        "Per matrix (f32): {:.1} MB, per layer (f32): {:.1} MB",
        bytes_per_matrix_f32 as f64 / 1e6,
        bytes_per_layer_f32 as f64 / 1e6
    );
    // Q4_K: 144 bytes per 256 floats (GGUF format)
    let bytes_per_matrix_q4k = (floats_per_matrix / 256) * 144;
    let bytes_per_layer_q4k = bytes_per_matrix_q4k * 3;
    println!(
        "Per matrix (Q4_K): {:.1} MB, per layer (Q4_K): {:.1} MB",
        bytes_per_matrix_q4k as f64 / 1e6,
        bytes_per_layer_q4k as f64 / 1e6
    );
    println!(
        "Total (Q4_K): {:.1} GB\n",
        (bytes_per_layer_q4k * num_layers) as f64 / 1e9
    );

    // Open source files
    let gate_file = std::fs::File::open(dir.join("gate_vectors.bin"))?;
    let gate_mmap = unsafe { Mmap::map(&gate_file)? };

    let up_file = std::fs::File::open(dir.join("up_features.bin"))?;
    let up_mmap = unsafe { Mmap::map(&up_file)? };

    let down_file = std::fs::File::open(dir.join("down_features.bin"))?;
    let down_mmap = unsafe { Mmap::map(&down_file)? };

    println!("Source files:");
    println!(
        "  gate_vectors.bin:  {:.1} MB",
        gate_mmap.len() as f64 / 1e6
    );
    println!("  up_features.bin:   {:.1} MB", up_mmap.len() as f64 / 1e6);
    println!(
        "  down_features.bin: {:.1} MB\n",
        down_mmap.len() as f64 / 1e6
    );

    // Gate vectors may be f32 already (same as features) or need dtype detection
    let gate_bytes_per_layer = bytes_per_matrix_f32; // gate is [intermediate, hidden] per layer
    let up_bytes_per_layer = bytes_per_matrix_f32;
    let down_bytes_per_layer = bytes_per_matrix_f32;

    // Verify sizes
    let expected_gate = gate_bytes_per_layer * num_layers;
    let _expected_up = up_bytes_per_layer * num_layers;
    let _expected_down = down_bytes_per_layer * num_layers;

    if gate_mmap.len() != expected_gate {
        println!(
            "WARNING: gate_vectors.bin size {} != expected {}",
            gate_mmap.len(),
            expected_gate
        );
        println!("  Gate may be f16 or have different layout. Checking...");
        // f16 gate vectors: half the size
        if gate_mmap.len() == expected_gate / 2 {
            println!("  Gate is f16 — will decode to f32 during interleave");
        } else {
            return Err(format!("gate_vectors.bin unexpected size: {}", gate_mmap.len()).into());
        }
    }

    let gate_is_f16 = gate_mmap.len() == expected_gate / 2;

    // Write interleaved Q4_K file
    let out_path = dir.join("interleaved_q4k.bin");
    let mut out = std::io::BufWriter::with_capacity(
        16 * 1024 * 1024, // 16MB buffer
        std::fs::File::create(&out_path)?,
    );

    let t0 = Instant::now();
    let mut total_bytes: u64 = 0;

    for layer in 0..num_layers {
        // Gate
        let gate_floats = if gate_is_f16 {
            let gate_offset = layer * gate_bytes_per_layer / 2;
            let gate_end = gate_offset + gate_bytes_per_layer / 2;
            let raw = &gate_mmap[gate_offset..gate_end];
            larql_models::quant::half::decode_f16(raw)
        } else {
            let gate_offset = layer * gate_bytes_per_layer;
            let gate_end = gate_offset + gate_bytes_per_layer;
            let raw = &gate_mmap[gate_offset..gate_end];
            let floats: Vec<f32> = unsafe {
                std::slice::from_raw_parts(raw.as_ptr() as *const f32, raw.len() / 4).to_vec()
            };
            floats
        };

        // Quantize gate to Q4_K (GGUF format)
        let gate_q4k = larql_compute::cpu::ops::q4_common::quantize_q4_k_gguf(&gate_floats);
        out.write_all(&gate_q4k)?;
        total_bytes += gate_q4k.len() as u64;

        // Up
        let up_offset = layer * up_bytes_per_layer;
        let up_end = up_offset + up_bytes_per_layer;
        let up_raw = &up_mmap[up_offset..up_end];
        let up_floats: Vec<f32> = unsafe {
            std::slice::from_raw_parts(up_raw.as_ptr() as *const f32, up_raw.len() / 4).to_vec()
        };
        let up_q4k = larql_compute::cpu::ops::q4_common::quantize_q4_k_gguf(&up_floats);
        out.write_all(&up_q4k)?;
        total_bytes += up_q4k.len() as u64;

        // Down
        let down_offset = layer * down_bytes_per_layer;
        let down_end = down_offset + down_bytes_per_layer;
        let down_raw = &down_mmap[down_offset..down_end];
        let down_floats: Vec<f32> = unsafe {
            std::slice::from_raw_parts(down_raw.as_ptr() as *const f32, down_raw.len() / 4).to_vec()
        };
        let down_q4k = larql_compute::cpu::ops::q4_common::quantize_q4_k_gguf(&down_floats);
        out.write_all(&down_q4k)?;
        total_bytes += down_q4k.len() as u64;

        if layer % 10 == 0 || layer == num_layers - 1 {
            println!(
                "  Layer {layer}: gate+up+down = {:.1} MB (Q4_K) @ offset {:.1} GB",
                (gate_q4k.len() + up_q4k.len() + down_q4k.len()) as f64 / 1e6,
                total_bytes as f64 / 1e9
            );
        }
    }

    out.flush()?;
    let elapsed = t0.elapsed();

    println!(
        "\nInterleaved Q4_K file: {:.1} GB ({:.1}s)",
        total_bytes as f64 / 1e9,
        elapsed.as_secs_f64()
    );
    println!("Layout: [gate|up|down] × {num_layers} layers, Q4_K quantized");
    println!("File: {}", out_path.display());
    println!("Done.");

    Ok(())
}
