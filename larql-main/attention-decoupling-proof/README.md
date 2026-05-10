# Attention Decoupling Proof

Minimal proof target: real GGUF, Rust runtime, separated attention execution.

This directory keeps proof drivers and run notes only. The GGUF input, extracted
tensor payloads, and runtime proof capsule are local generated artifacts and are
ignored by git.

Runtime contract:

- admission sites accept capture JSON only;
- emitting sites send capsule JSON only;
- weight side owns GGUF tensors and Q/K/V projection;
- attention side owns only causal GQA over supplied Q/K/V activations;
- no Python participates in proof logic.

Proof boundary:

- proves separated runtime transport/admission/emission contract;
- proves child process computes attention from Q/K/V activation payload only;
- proves parent keeps learned GGUF weights and O/residual work local;
- does not prove independent attention algorithm correctness;
- does not prove full-model end-to-end generation equivalence.

## Real GGUF Proof

```bash
cargo run -q -p larql-cli -- attention-runtime proof \
  --gguf attention-decoupling-proof/qwen3.5-0.8b.gguf \
  --layer 3 \
  --seq-len 8 \
  --output attention-decoupling-proof/separated-attention-runtime-gguf-layer-proof.json
```

Output capsule:

- `separated-attention-runtime-gguf-layer-proof.json`
- capsule kind: `gguf_separated_attention_runtime_proof_capsule`
- capture kind: `gguf_separated_attention_runtime_proof`

Current real run:

- GGUF: `qwen3.5-0.8b.gguf`
- runnable layer: `3`
- sequence length: `8`
- max absolute diff: `0.0`
- attention output max absolute diff: `0.0`
- O-projected attention max absolute diff: `0.0`
- post-attention residual max absolute diff: `0.0`
- attention weight max absolute diff: `0.0`
- request capture kind: `separated_attention_request`
- request payload fields match contract: `true`
- learned weight identifiers sent on wire: `false`
- child attention runtime loaded model weights: `false`
- child process exit status: `0`

## Raw GGUF Attention Tensor Isolation

```bash
cargo run -q -p larql-cli -- attention-runtime extract-gguf \
  --gguf attention-decoupling-proof/qwen3.5-0.8b.gguf \
  --scope all \
  --out attention-decoupling-proof/qwen3.5-0.8b-attention-rust
```

Output capsules:

- `qwen3.5-0.8b-attention-rust/attention_manifest.json`
- `qwen3.5-0.8b-attention-rust/tensor-capsules/*.capsule.json`
- `qwen3.5-0.8b-attention-rust/tensors/*.bin`

Current real extraction:

- GGUF file size: `989M`
- selected attention tensors: `224`
- manifest tensor bytes: `272013312`
- extracted directory size: `261M`
- manifest capsule kind: `gguf_attention_extraction_report_capsule`
- manifest capture kind: `gguf_attention_extraction_report`

## Verification

Last verified locally:

```bash
cargo check -p larql-cli
cargo test -p larql-core capsule
cargo test -p larql-models gguf
cargo run -q -p larql-cli -- attention-runtime proof \
  --gguf attention-decoupling-proof/qwen3.5-0.8b.gguf \
  --layer 3 \
  --seq-len 8 \
  --output attention-decoupling-proof/separated-attention-runtime-gguf-layer-proof.json
```

## Python

`attention_decoupling_proof.py` and `extract_attention_tensors.py` are thin activators for the Rust CLI. They do not parse GGUF, validate capsules, build tensors, or run attention.
