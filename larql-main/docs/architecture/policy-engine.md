# Policy Engine

SME source: `../../../docs/engine-rules.md`.

Policy authority has one root, many packs, one evaluator, one ledgered
admittance path, and one CI/runtime integration contract.

Active policy lives in:

- `governance/engine.toml`;
- `governance/policies/index.toml`;
- included `governance/policies/*.toml` packs;
- `governance/profiles/*.toml`;
- `governance/flows/*.toml`;
- `governance/recipes/*.toml`;
- `governance/tests/*_cases.toml` policy tests.

The authored files are not the engine. They are loaded into a
`PolicyRegistry`, validated, and compiled into a `CompiledPolicyPlan`. Runtime
decisions use `PolicyEngine`, which owns the registry and compiled plan. Active
runtime state is represented by `ActivePolicy`, an immutable `Arc<PolicyEngine>`
plus policy hash and generation.

```text
authored config/packs/profiles/flows/recipes
-> validated PolicyRegistry
-> CompiledPolicyPlan
-> immutable PolicyEngine
-> decisions / receipts / capabilities / activation
```

The config selects active packs, profiles, flows, recipes, and mode. It does
not contain executable policy logic. Rules remain in policy packs. Profiles
attach rule sets to artifact classes. Flows define lawful transition paths.
Recipes collect structured questions, factoids, derived facts, and required
outputs for governed work.

**Key Invariant: Recipes gather, Policies authorize.**
Recipes are responsible for structuring the evidence and facts required for a
given intent. They do not decide authority. Final admissibility is always
determined by the policy engine evaluating the gathered facts against the
active rule set.

The engine accepts facts and emits decisions:

- `PolicyInput`;
- `PolicyEngineDecision`;
- active policy set id/version;
- policy hash;
- matched rules;
- required actions;
- evidence;
- findings.

Conflict resolution is most restrictive:

```text
fatal > deny > require_review > allow_with_warnings > allow
```

CI uses the same evaluator as runtime:

```bash
./scripts/check_governance_rules.sh
```

That script first compiles active policy into
`target/governance/policy-compile-report.json`, then runs policy evaluation,
replay, policy tests, shadow evaluation, and decision-surface validation through
the same Rust engine:

```bash
governor policy compile --policy governance/policies/index.toml
governor policy test governance/tests/machine_creation_cases.toml
governor policy shadow-eval governance/fixtures/shadow_eval_cases.toml
```

Governance source-file shape is also checked in CI:

```bash
./scripts/verify-source-file-policy.sh
```

The 999-line split rule applies to governance and machine authority Rust source.
It does not apply to the rules engine, registries, or ledgers. Those files are
governed authority surfaces, and splitting them only to satisfy a line-count
heuristic can make audit harder. The gate therefore exempts `rules_engine*.rs`,
`*registry*.rs`, `*ledger*.rs`, and Rust files under registry/ledger
directories.

The governance-oriented CLI alias exposes the same evaluator directly:

```bash
governor policy eval \
  --facts governance/facts/ci_governance_rules.json \
  --policy governance/policies/index.toml \
  --ledger target/governance/policy-decisions.jsonl
governor replay --ledger target/governance/policy-decisions.jsonl
```

The old default machine-rule profile is no longer a separate source of CI
authority. Its prohibitions are declarative rules in
`governance/policies/machine-profile.toml`, included by the active policy
index, and `larql machine rules verify` evaluates the profile through that
policy set before emitting the invariant-registry receipt.

Rule profiles are now first-class config:

- `governance/profiles/machine.toml`;
- `governance/profiles/governing_artifact.toml`;
- `governance/profiles/policy_file.toml`;
- `governance/profiles/rust_struct.toml`;
- `governance/profiles/event_schema.toml`.

Policy flows are first-class config:

- `governance/flows/machine_creation.toml`;
- `governance/flows/policy_update.toml`;
- `governance/flows/policy_weakening.toml`;
- `governance/flows/patch_application.toml`;
- `governance/flows/capability_minting.toml`;
- `governance/flows/replay_closure.toml`.

Policy recipes are first-class config:

- `governance/recipes/struct_minting.toml`.

Decision surface:

- recurring LLM choices are registry entries in
  `governance/decision_surface/registry.toml`;
- unknown choices emit `UnknownDecisionEncountered`;
- promoted choices cite the deterministic artifact now owning them;
- ceremony definitions in `governance/ceremonies/*.toml` own phase order and
  required evidence for policy updates, machine creation, and capability
  minting;
- ceremony budgets in `governance/ceremonies/budgets.toml` cap unresolved LLM
  decisions before high-authority paths proceed.
- LLM autonomy levels in `governance/defaults/llm_roles.toml` declare what a
  bootstrap agent may decide and explicitly forbid policy activation,
  capability grant, and runtime mutation authority.

Validation:

```bash
larql machine decision-surface validate \
  --registry governance/decision_surface/registry.toml \
  --budgets governance/ceremonies/budgets.toml \
  --roles governance/defaults/llm_roles.toml
```

Unknown recurring decisions are recorded before they become implicit authority:

```bash
larql machine decision-surface record-unknown \
  --id error_decomposition.output_shape \
  --question "Should error decomposition extend existing errors or mint a new output type?" \
  --context machine_creation \
  --target-owner schema \
  --promotion-path schema \
  --risk medium \
  --option "extend existing error enum" \
  --option "mint a new output type"
```

Policy update path:

```text
prose concern
-> policy intent extraction
-> rule proposal
-> examples / counterexamples
-> formal rule draft
-> conflict check
-> shadow evaluation
-> approval
-> policy activation
-> decision receipts
-> replay
```

First-version command flow:

```bash
governor policy propose \
  --from-prose "New machines require overlap analysis before creation." \
  --class ceremony_policy \
  --target machine_creation \
  --policy governance/policies/index.toml \
  --prior-state-hash sha256:... \
  --out governance/proposals/policy_update_042.concern.json

governor policy extract-intent \
  --concern governance/proposals/policy_update_042.concern.json \
  --class ceremony_policy \
  --action create_machine \
  --required-evidence existing_machine_overlap_check \
  --required-evidence machine_responsibility_boundary \
  --uncertainty "Should this block all machine creation or only high-risk machine creation?" \
  --out governance/proposals/policy_update_042.intent.json

governor policy draft \
  --concern governance/proposals/policy_update_042.concern.json \
  --intent governance/proposals/policy_update_042.intent.json \
  --out governance/proposals/policy_update_042.proposal.json
```

Compact invariant:

```text
A prose policy becomes active only after it has been translated into a formal rule, supplied with examples, checked for conflicts, shadow-evaluated against history, approved through the policy-update ceremony, applied by scoped capability, and recorded under a new policy hash.
```

Decision-surface invariant:

```text
The LLM may make a decision only when no deterministic policy, template, schema, fixture, machine, ceremony, or default owns that decision. Every such decision must be recorded as decision-surface debt unless it is one-off and non-governing.
```
