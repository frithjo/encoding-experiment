# Canonization Observation

Status: current live-code observation as of 2026-05-08. Verify against
`Waightslander/crates/governor-*` before treating as stable interface
documentation.

In live code, locations were defined as deterministic structural coordinates,
then materialized into a location-index.json.

  Core identity lives in Waightslander/crates/governor-core/src/identity.rs:14:

  CanonicalLocationDescriptor {
    repo_relative_path,
    module_path,
    item_kind,
    item_name,
    occurrence,
  }

  location_hash is:

  sha256(canonical_json({
    domain: "governor:v1:location_identity",
    descriptor: CanonicalLocationDescriptor
  }))

  Materializer currently creates two location kinds in Waightslander/crates/governor-cli/src/canonization.rs:133:

  1. file
      - source: git ls-files
      - excludes archive/, .run/, target/
      - descriptor:
          - repo_relative_path = tracked path
          - module_path = ""
          - item_kind = "file"
          - item_name = tracked path
          - occurrence = 0
      - evidence capsule:
          - evidence_kind = "tracked_file"
          - subject = location_hash
          - capsule = file content sha256
  2. rust_item
      - source: parsed Rust AST via syn
      - currently admits only struct and free/inline-module fn
      - descriptor:
          - repo_relative_path = Rust source path
          - module_path = path-derived Rust-ish module path
          - item_kind = "struct" or "fn"
          - item_name = identifier
          - occurrence = duplicate index for same kind/name
      - evidence capsule:
          - evidence_kind = "rust_item_span"
          - subject = location_hash
          - capsule = source span sha256

  Important distinction: materialized locations are candidate_unadmitted. They become admitted only through governor governance
  locations admit, which requires:

  - existing location_hash
  - matching evidence_hash
  - ceremony event_hash

  So location materialization means: “this canonical place exists in the tracked repo snapshot.”
  It does not yet mean: “governance has admitted this place.”
