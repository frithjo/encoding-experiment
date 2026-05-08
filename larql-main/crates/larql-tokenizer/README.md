# larql-tokenizer

## Crate Role

- Role: Tokenization contract and implementations
- Zone: core
- Release impact: high
- Stability target: stable

Tokenizer contract crate for LARQL with feature-gated implementations (for
example Hugging Face tokenizers) and shared encode/decode integration points.

## Scope

- Define tokenizer-facing APIs used by runtime/interface crates.
- Provide optional concrete backends behind crate features.
- Keep model/inference/query logic in their respective core crates.

## Public vs Internal Surface

- Public: tokenizer trait/contracts and encode/decode behavior expected by
  interface crates.
- Internal: backend adapters and feature-flag wiring may evolve without
  changing tokenizer semantics.
