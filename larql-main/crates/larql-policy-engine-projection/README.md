# larql-policy-engine-projection

## Crate Role

- Role: Policy engine snapshot projection
- Zone: interface
- Release impact: medium
- Stability target: iterative

Rust-authored projection layer for policy engine snapshots used by UI surfaces.

## Scope

- Build deterministic policy registry, evaluation, and ceremony-debt snapshots.
- Keep UI render inputs derived from typed governance state, not raw prose.
- Pin golden hashes when embedded governance fixtures intentionally change.
