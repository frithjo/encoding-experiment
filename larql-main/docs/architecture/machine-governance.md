# Machine Governance

LARQL treats machine action as governed state transition, not direct codegen.
Prose can propose intent, but runtime authority must be a capsule-backed record
with hashes, policy checks, and receipts.

## Runtime Rule

Admitted governance records carry hashes and capsules. Raw prose can remain as
external context or evidence material, but admitted records reference its hash.

## First Machine

The first implemented machine is `struct_minter` in `larql-governance`.

It accepts a JSON `larql.governance.struct_spec.v1` record and can:

- verify starter policy;
- synthesize a Rust struct from explicit reviewed fields;
- reject unsafe code, unreviewed prose authority, arbitrary edits, dependency
  additions, unjustified `repr(C)`, and public API changes without test/doc
  evidence hashes;
- require `governance.type_need_hash`, recording the repo-grounded analysis that
  an existing type cannot legally or cleanly express the needed state;
- emit mint, policy, or denial receipt capsules.

## CLI Surface

```bash
larql machine policy verify --spec specs/example.json --profile specs/profile.json
larql machine struct verify --spec specs/example.json --profile specs/profile.json
larql machine struct mint --spec specs/example.json --profile specs/profile.json --mode patch-only --source-out target/example.rs
larql machine receipt verify --path target/example.receipt.json
```

Generated-file mode writes only to generated Rust paths and then runs a focused
`cargo check` unless `--skip-cargo-check` is explicitly set.
If `--profile` is omitted, the default starter profile is used.

## Authority

Rust struct authority is derived from parsed Rust source:

- repo-relative path;
- module path;
- item kind/name;
- visibility;
- canonical item source hash;
- canonical location hash.

Path or item movement changes the binding hash and therefore changes the
factoid relationship that can fund future governance.

## Governed Patch Envelope

The broader machine entry point is `prose_to_governed_patch`, exposed as:

```bash
larql machine patch plan --spec specs/patch-intent.json --profile specs/profile.json
```

It accepts `larql.governance.patch_intent.v1`, which carries intent,
proposed-change, and evidence hashes instead of raw admitted prose. The machine
then:

- scans Rust repo facts and derives struct binding hashes;
- verifies proposed struct specs through the starter rule profile;
- resolves placement, API, serialization, storage, migration, caller,
  invariant, and test stages;
- emits a `larql.governance.patch_envelope_capsule.v1` receipt.

Patch-envelope stage answers are system-defined tokens, not free-form prose.
This keeps the runtime record replayable: auditors can compare hashes,
capsules, stage tokens, and repo facts without accepting unbounded language as
authority.
