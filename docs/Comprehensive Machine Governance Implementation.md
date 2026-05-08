  # Comprehensive Machine Governance Implementation

  ## Status

  Historical plan artifact, not current implementation state. Live tree already
  contains `larql-main/crates/larql-governance`, `larql machine ...`, and
  `larql-main/docs/architecture/machine-governance.md`.

  ## Summary

  Implement a Rust-only governed machine layer around the repo’s existing capsule primitives. The first concrete machine will be a struct/change minter that
  turns governed specs into compiler-checked, policy-checked, hash-addressed proposals, while refusing raw prose as admitted runtime authority.

  Original baseline claim was materially false after commit `f7521d5`: a
  dedicated governance crate and machine CLI exist. Treat remaining sections as
  prior intent, not proof of missing implementation.

  ## Key Changes

  - Add a new core crate, larql-governance, and wire it into the workspace.
  - Move reusable governance primitives there:
      - capsule-backed records
      - deterministic hashing helpers
      - machine rule profiles
      - ceremony/capability records
      - denial receipts
      - struct binding authority records
  - Add a CLI surface under larql machine ...:
      - machine policy verify
      - machine struct mint --spec <path> --mode patch-only|generated-file
      - machine struct verify --spec <path>
      - machine receipt verify --path <path>
  - Specs and receipts are JSON capsules by default to avoid adding TOML parsing dependency and to align with existing capsule/hash machinery.
  - Add one justified direct dependency only if needed: syn for Rust AST parsing, because std/native Rust cannot parse Rust item structure or derive stable
    struct-binding authority.
  - Enforce the default starter rule profile:
      - reject arbitrary unscoped edits
      - reject prose-derived field inference without typed review
      - reject unsafe code
      - reject repr(C) unless explicitly justified
      - reject overwriting hand-written code
      - reject auto-added dependencies unless policy-approved
      - reject auto-commit without receipt
      - reject public API mutation without test/doc policy evidence
  - Add struct classification gates for every minted public struct:
      - internal state
      - public API
      - serialized
      - persisted
      - FFI boundary
      - security/authority-bearing
      - expected stable contract
  - Make factoid/path authority derive from Rust struct bindings, not raw paths or prose:
      - repo-relative path
      - module path
      - item kind/name
      - visibility
      - canonical item source hash
      - canonical location hash
  - Add strict CLI validation that admitted records carry hashes/capsules, not raw prose payloads.

  ## Implementation Shape

  - larql-governance owns the machine runtime library.
  - larql-cli only dispatches commands and prints receipts.
  - larql-core::capsule remains usable, but governance-specific schemas move behind the new governance crate API.
  - Minted code is limited to generated files or patch-only output; no arbitrary existing-module mutation in this phase.
  - Denials are first-class JSON capsule receipts, not plain errors.
  - Update AGENTS.md with the operating rule: provenance funds governance; machine actions must enter through capsules, rules, capabilities, and receipts.

  ## Test Plan

  - Unit: capsule validation rejects mutated payload/hash pairs.
  - Unit: machine profile rejects each forbidden starter rule.
  - Unit: struct spec validation rejects missing authority classification.
  - Unit: generated struct output is deterministic for the same spec.
  - Unit: Rust struct binding hash changes on path/name/source movement.
  - Negative: raw prose in admitted records fails CLI verification.
  - Negative: public struct without test/doc policy evidence fails.
  - Positive: patch-only mint emits spec hash, generated item hash, policy result, and receipt.
  - Integration: cargo check -p larql-governance -p larql-cli.
  - Integration: larql machine struct verify --spec <fixture> passes/fails deterministically.

  ## Assumptions

  - The implementation should be Rust-only for governance/runtime paths.
  - JSON capsule specs are preferred over TOML to minimize dependencies.
  - syn is acceptable only for AST authority derivation, because native Rust has no stable equivalent.
  - The first machine mints governed structs/change proposals only; self-patching and broader machine teams come after this foundation is enforced.
