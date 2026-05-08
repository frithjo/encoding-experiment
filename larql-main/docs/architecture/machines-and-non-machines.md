# Machines And Non-Machines

SME source: `../../../docs/machins-non-machines.md`.

## Rule

Machines perform governed work.
Non-machines define, constrain, record, or test governed work.

Only machines may perform governed transitions.
Only non-machines may define stable declarative constraints.

## Machine

A machine is an executable unit with responsibility for a governed operation.
It must have:

- named purpose;
- typed input;
- typed output;
- evidence it consumes;
- evidence it emits;
- deterministic behavior where the operation permits it;
- tests;
- ledger-visible effects when it affects governance state.

Current machine examples:

- `struct_minter`;
- `prose_to_governed_patch`;
- future `policy_checker`, `proposal_intake`, `replay_verifier`,
  `activation_funder`, and `migration_checker` units.

Machine contracts are applied by `larql-governance::PolicyEngine` from
declarative policy packs and described by `larql-governance::artifact`:

- `GovernanceArtifactRole::Machine`;
- `AuthorityOperation::GovernedTransition`;
- `GovernanceArtifactDefinition`;
- `PolicyEngine`;
- `PolicyInput`;
- `PolicyEngineDecision`;
- `validate_governance_artifact_definition`;
- `validate_authority_operation`.

Machine governance also has a first-class rule profile:
`governance/profiles/machine.toml`. That profile attaches the active machine
rule sets to the `machine` artifact class and selects
`governance/flows/machine_creation.toml` as the lawful default flow.
Structured machine work may also use a recipe, such as
`governance/recipes/struct_minting.toml`, to collect factoids and outputs
before the engine evaluates authority.

## Non-Machine

A non-machine can be authoritative, but it is not active. It does not execute
or mutate governance state on its own. A machine reads it, verifies it, applies
it, emits from it, or records against it.

Non-machine examples:

- policy files;
- ceremony definitions;
- schemas;
- fixtures;
- test cases;
- docs and ADRs;
- manifests;
- known-location hashes;
- generated receipts;
- event logs;
- training examples;
- accepted and rejected proposal examples.

Non-machine contracts are applied by the same policy engine:

- `GovernanceArtifactRole::NonMachine`;
- `NonMachineArtifactKind`;
- `AuthorityOperation::StableDeclarativeConstraint`;
- `GovernanceInvariantRegistry`;
- declarative-only validation in `validate_governance_artifact_definition`.

Non-machine profiles live beside machine profiles:

- `governance/profiles/governing_artifact.toml`;
- `governance/profiles/policy_file.toml`;
- `governance/profiles/rust_struct.toml`;
- `governance/profiles/event_schema.toml`.

These files select rule sets and flows. They do not execute policy. Runtime
authority starts only after `PolicyRegistry` validation and `CompiledPolicyPlan`
compilation.

## Invariant Registry

Repo invariants are non-machine artifacts. They become runtime authority only
after admission into `governance/invariants/registry.json`.

The registry is a non-machine input to the active policy set. Each admitted
invariant must carry:

- stable id;
- subject;
- status;
- statement hash;
- source hashes;
- evidence hashes;
- admission receipt hash;
- CI gate flag.

The CI path is the same policy-engine path:

```bash
larql machine rules eval \
  --facts governance/facts/ci_governance_rules.json \
  --policy governance/policies/index.toml \
  --ledger target/governance/policy-decisions.jsonl
governor replay --ledger target/governance/policy-decisions.jsonl
```

Adding an invariant means adding a hash-backed registry entry and keeping the CI
gate green. Static docs can propose or explain invariants, but only the registry
admits them.

Registry and ledger source files are exempt from the 999-line source split
heuristic. They are governed authority surfaces, so auditability favors keeping
their registry/ledger boundary intact. `scripts/verify-source-file-policy.sh`
enforces this by exempting `*registry*.rs`, `*ledger*.rs`, registry/ledger
directories, and the rules engine.

## Prose Policy Updates

Prose may initiate policy and explain policy. It is never executable policy.
A prose-originated policy becomes active only after it is translated into a
formal rule, supplied with allow/deny/review examples, checked for conflicts,
shadow-evaluated, approved, applied by scoped capability, recorded under a new
policy hash, and replayed.

The first recorded event preserves exact prose:

```bash
larql machine rules propose \
  --from-prose "New machines require overlap analysis before creation." \
  --class ceremony_policy \
  --target machine_creation \
  --policy governance/policies/index.toml \
  --prior-state-hash sha256:...
```

The same ceremony entry point is available as:

```bash
governor policy propose \
  --from-prose "New machines require overlap analysis before creation." \
  --class ceremony_policy \
  --target machine_creation \
  --policy governance/policies/index.toml \
  --prior-state-hash sha256:...
```

The next event must make extracted intent explicit before any formal rule draft:

```bash
governor policy extract-intent \
  --concern governance/proposals/policy_update_042.concern.json \
  --class ceremony_policy \
  --action create_machine \
  --required-evidence existing_machine_overlap_check \
  --required-evidence machine_responsibility_boundary
```

Candidate rules use `governance/proposals/*.json`, policy tests use
`governance/tests/*_cases.toml`, and active rules live under
`governance/policies/*.toml`. CI runs those tests through
`./scripts/check_governance_rules.sh` after compiling the active policy set.

## Boundary

Do not inflate static artifacts into actors. A policy file is not a
`policy_machine`; a ceremony definition is not a `ceremony_machine`; a fixture
is not a `fixture_machine`.

Do not leave active authority implicit in static files. If a policy, schema, or
ceremony changes governance state, the active step must be owned by a machine
with typed inputs, typed outputs, evidence hashes, emitted evidence, tests, and
ledger-visible receipts.
