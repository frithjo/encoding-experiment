# SME Direction: Machines And Non-Machines

Status: codified in
`larql-main/docs/architecture/machines-and-non-machines.md` and
`larql-main/crates/larql-governance/src/artifact.rs`.

Yes. You should have **both machines and non-machines** in the repo.

The cleaner rule is:

```text
Machines perform governed work.
Non-machines define, constrain, record, or test governed work.
```

Trying to make everything a machine will make the system bloated and theatrical. Trying to make everything non-machine will leave too much authority implicit.

## The distinction I would use

A **machine** is an executable unit with responsibility for a governed operation.

It should have:

```text
- named purpose
- typed input
- typed output
- evidence it consumes
- evidence it emits
- deterministic behavior as much as possible
- tests
- ledger-visible effects, if it affects governance state
```

A **non-machine** is anything that informs or constrains the machines but does not itself act.

Examples:

```text
- policy files
- ceremony definitions
- schemas
- fixtures
- test cases
- docs / ADRs
- manifests
- known-location hashes
- generated receipts
- event logs
- training examples
- examples of accepted/rejected proposals
```

The key difference:

```text
Non-machines may be authoritative.
But they are not active.
```

A policy file can carry authority, but it does not execute. A machine reads it and applies it.

## Why both are necessary

If everything is a machine, you get **agency inflation**.

You end up with:

```text
policy_machine
schema_machine
ceremony_machine
readme_machine
test_machine
config_machine
```

That becomes hard to reason about because every artifact starts pretending to be an actor.

But if everything is just static files, you get **implicit authority**.

You end up asking:

```text
Who enforces this policy?
Who checks this schema?
Who decides this ceremony completed?
Who verifies this receipt?
```

So the division should be:

```text
non-machines state the law, evidence, examples, and memory
machines apply, verify, transform, or enforce them
```

## Practical repo shape

Something like this is grounded:

```text
crates/
  governor-core/
    machine.rs
    event.rs
    evidence.rs
    capability.rs
    activation.rs

  governor-machines/
    machines/
      path_resolver.rs
      proposal_intake.rs
      capability_minter.rs
      struct_minter.rs
      policy_checker.rs
      replay_verifier.rs

  governor-cli/
    main.rs

governance/
  policies/
    machine_creation.toml
    public_api.toml
    storage_migration.toml

  ceremonies/
    machine_creation.toml
    patch_application.toml
    corpus_promotion.toml

  schemas/
    proposal.schema.json
    capability.schema.json
    receipt.schema.json

  fixtures/
    denied_direct_machine_addition.json
    stale_capability_attempt.json
    accepted_machine_creation.json

  examples/
    proposals/
    receipts/
    denials/

  ledger/
    events.jsonl
```

In that layout:

```text
crates/governor-machines/ = active machinery
governance/ = declarative authority and memory
```

That separation is healthy.

## What should be a machine?

Make something a machine when it answers:

```text
Given state X and input Y, should the system produce output Z?
```

Good machine candidates:

```text
path_resolver
  Given intent, return lawful path.

proposal_intake
  Given proposal, validate shape and attach initial evidence.

capability_minter
  Given approved proposal, mint scoped capability.

policy_checker
  Given patch envelope, evaluate policy.

struct_minter
  Given approved type spec, generate Rust item.

replay_verifier
  Given event log, reconstruct state hash.

activation_funder
  Given provenance graph, decide whether a path may activate.

migration_checker
  Given type/schema change, decide whether storage review is required.
```

These are active transformations or decisions.

## What should not be a machine?

Do **not** make these machines early:

```text
policy itself
ceremony definition itself
schema itself
training example itself
receipt itself
fixture itself
ADR itself
```

Those are better as files.

For example:

```text
machine_creation.toml
```

is not a machine.

The machine is:

```text
ceremony_runner.rs
```

or:

```text
path_resolver.rs
```

which reads `machine_creation.toml`.

## The hard rule

I would enforce this:

```text
Only machines may perform transitions.
Only non-machines may define stable declarative constraints.
```

That prevents two bad outcomes.

Bad outcome 1:

```text
A policy file somehow mutates state.
```

Bad outcome 2:

```text
A machine secretly contains hard-coded policy that should have been declarative.
```

So if a rule is stable and inspectable, put it in a non-machine artifact.

If a rule needs execution, evaluation, transformation, or evidence checking, put that logic in a machine.

## Another useful distinction

Use three classes, not two:

```text
1. Machines
   active executors/verifiers

2. Governing artifacts
   policies, ceremonies, schemas, invariants

3. Evidence artifacts
   logs, receipts, tests, fixtures, examples, training traces
```

That is probably cleaner than just machine vs non-machine.

Example:

```text
Machine:
  policy_checker.rs

Governing artifact:
  policies/public_api.toml

Evidence artifact:
  receipts/public_api_change_042.json
```

The policy checker acts. The policy constrains. The receipt proves.

## Path activation implication

For path activation, this distinction matters.

A non-machine can fund activation:

```text
policy says public API changes require review
fixture shows similar past failure
ADR declares invariant
test failure proves behavior gap
```

But only a machine can activate the path:

```text
activation_funder reads provenance
path_resolver identifies ceremony
proposal_intake opens ceremony
```

So:

```text
non-machines supply provenance
machines convert provenance into action
```

That is the right split.

## The one thing to avoid

Avoid “machine” becoming a prestige label.

Not every important thing is a machine.

Some of the most important system objects should be inert:

```text
constitution
policy manifest
event log
capability receipt
schema
test fixture
denial trace
```

Their strength comes from being stable and inspectable, not active.

## My recommendation

Use both.

But formalize the boundary:

```text
Machine:
  executable governed actor

Governing artifact:
  declarative constraint or ceremony

Evidence artifact:
  proof, trace, receipt, fixture, example

Generated artifact:
  machine-produced output that is not authoritative until accepted
```

Then require every repo item to declare its class.

Example metadata:

```toml
[artifact]
class = "governing_artifact"
kind = "policy"
name = "machine_creation_policy"
```

or:

```toml
[artifact]
class = "machine"
kind = "path_resolver"
name = "path_resolver_machine"
```

## Bottom line

Your repo should contain both.

The real invariant should be:

```text
Machines act.
Non-machines constrain, remember, prove, or train.
No artifact changes governed state except through a machine-mediated transition.
No machine gets authority except from non-machine constraints and recorded provenance.
```

That gives you a repo that stays maintainable without turning every file into an actor.
