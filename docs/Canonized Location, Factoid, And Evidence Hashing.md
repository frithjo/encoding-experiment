  # Canonized Location, Factoid, And Evidence Hashing

  ## Status

  Historical plan artifact. Verify against live `Waightslander/crates/governor-*`
  code before using any command list or schema statement as current authority.

  ## Summary

  Implement a hard semantic cleanup:

  - location_hash identifies canonical place.
  - factoid_hash identifies canonical claim bound to a location.
  - evidence_hash identifies the capsule/receipt proving where or why admission happened.
  - event_hash identifies the governed observation/admission event.
  - provenance remains the replay graph, not a standalone identity primitive.

  The Rust path belongs in the location descriptor. factoid_hash must not equal raw path hash; it must include location_hash plus claim identity so multiple
  claims at one Rust item do not collide.

  ## Key Changes

  - Add one canonical Rust-owned identity module in governor-core:
      - CanonicalLocationDescriptor
      - CanonicalFactoidClaim
      - EvidenceReference
      - deterministic hash builders for location_hash, factoid_hash, evidence_hash
  - Define active schema v1:
      - location_hash = hash(repo_path + module_path + item_kind + item_name + occurrence)
      - factoid_hash = hash(location_hash + claim_kind + claim_payload_hash + claim_schema_version)
      - evidence_hash = hash(capsule_or_receipt_descriptor)
  - Update active event/fact graph handling:
      - LocationIdentityObserved carries location_hash and evidence_hash
      - FactoidStateObserved carries factoid_hash, location_hash, factoid_class, evidence_hash
      - no new active authority path should require provenance_hash
  - Keep historical provenance_hash records as legacy schema facts only. New regime commands emit evidence_hash / admission_hash semantics instead of
    promoting provenance_hash.
  - Add CLI projections:
      - governor governance locations index
      
      
      

  - location_hash: canonical place, ceremony-admitted.
  - factoid_hash: pure Rust path claim identity first.
  - Rust path claim cannot be valid/admitted without provisional/actual location binding.
  - evidence_hash: capsule/receipt for admission reason/place.
  - event_hash: governed observation/admission event.
  - provenance: replay graph, not identity primitive.

      - governor governance factoids inventory
      - governor governance commit receipt
      - governor governance commit verify
  - Commit receipt binds:
      - touched paths
      - touched location_hashes
      - factoid deltas
      - parent inventory hash
      - new inventory hash
      - receipt hash
      - optional git trailer pointers only, never trailer authority.

  ## Enforcement

  - Reject factoids without known canonical location_hash.
  - Reject factoid hash collisions at same location unless claim payload hash matches.
  - Reject raw admitted prose in evidence/capsule records.
  - Reject active authority from archive-only records.
  - Reject commit verification when touched tracked files lack location coverage.
  - Reject use of provenance_hash as active identity in new commands.

  ## Tests

  - Unit tests for stable location_hash from same Rust item descriptor.
  - Unit tests proving path/module/item changes alter location_hash.
  - Unit tests proving two different claims at same location produce different factoid_hashes.
  - Unit tests proving same claim with new evidence keeps same factoid_hash.
  - Replay test builds factoid_inventory from events only.
  - Negative tests:
      - unknown location for factoid fails
      - missing evidence hash fails
      - raw prose evidence fails
      - archive-only evidence fails
      - commit receipt missing touched location fails
      - old provenance_hash cannot satisfy new active evidence field

  ## Assumptions

  - No new dependencies.
  - Native Rust only.
  - No compatibility shim for new active commands.
  - Existing historical records may remain readable as historical evidence, but cannot fund new active governance without re-admission into the new evidence
    schema.
