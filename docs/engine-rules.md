Yes. Build the **policy engine** as the system’s central decision layer.

But be careful with the phrase “single location.” You want a single **authority surface**, not necessarily one giant file.

Better:

```text
One policy root.
Many policy packs.
One evaluator.
One ledgered rule-addition path.
One CI/runtime integration contract.
```

## Core design

The policy engine should sit between facts and actions:

```text
repo / CI / runtime / agent / ceremony
→ facts
→ policy engine
→ decision
→ capability / denial / required action / activation
```

The engine should not merely check rules. It should produce **governance decisions**.

Example decision:

```json
{
  "decision": "deny",
  "rule_id": "machine.creation.requires_tests",
  "severity": "high",
  "reason": "New machine proposal has no test plan.",
  "required_actions": [
    "add test proving machine accepts valid input",
    "add test proving machine rejects malformed input"
  ],
  "evidence": [
    "proposal.machine.kind = error_decomposer",
    "proposal.tests = []"
  ]
}
```

That decision can block CI, block runtime mutation, or deny a capability.

## The key separation

Use this division:

```text
Policies are declarative.
Evaluators are executable.
Decisions are ledgered.
```

So:

```text
governance/policies/*.toml
```

defines rules.

```text
governor-policy/src/evaluator.rs
```

executes them.

```text
.governor/ledger/events.jsonl
```

records the decision.

This matters because hard-coding every rule into Rust makes policy changes expensive, but letting arbitrary runtime rules execute code is unsafe.

## Do not make runtime-added rules arbitrary code

This is the hard rule:

```text
Runtime-added rules should be data, not executable code.
```

Allowed:

```toml
[rule]
id = "public_api.requires_tests"
scope = "patch"
when = [
  { fact = "public_api_changed", equals = true }
]
require = [
  { fact = "tests_changed", equals = true }
]
effect = "deny"
severity = "high"
message = "Public API changes require tests."
```

Not allowed:

```rust
fn new_runtime_policy(ctx: &mut System) {
    // arbitrary code loaded during runtime
}
```

Dynamic executable policy is a trap. It creates a second programming language with mutation authority.

Start with a constrained rule language.

## Policy root layout

Something like:

```text
governance/
  policies/
    index.toml
    ci.toml
    machine_creation.toml
    public_api.toml
    storage_migration.toml
    capability.toml
    path_activation.toml
    training_data.toml

  ceremonies/
    machine_creation.toml
    patch_application.toml
    policy_change.toml

  rule_sets/
    default.toml
    strict.toml
    experimental.toml

  schemas/
    policy.schema.json
    decision.schema.json
    fact.schema.json
```

The `index.toml` is the central authority file:

```toml
[policy_set]
id = "governor.default"
version = 1

includes = [
  "ci.toml",
  "machine_creation.toml",
  "public_api.toml",
  "storage_migration.toml",
  "capability.toml",
  "path_activation.toml",
  "training_data.toml"
]

[mode]
unknown_rule = "deny"
unknown_fact = "warn"
conflict_resolution = "most_restrictive"
```

That gives you one place to know what policy set is active.

## The policy engine should evaluate facts, not raw vibes

Your earlier architecture already points here: observe the repo, normalize observations into stable facts, then apply deterministic rules first. The model can classify or explain later, but the rule engine should create evidence before any model is trusted. 

A fact should look like:

```json
{
  "fact_id": "fact_0182",
  "fact_type": "public_api_changed",
  "value": true,
  "subject": "crate::auth::ExecutionToken",
  "evidence": [
    "src/lib.rs re-exports auth::ExecutionToken",
    "field expires_at added"
  ],
  "source": "rust_ast_scanner",
  "state_hash": "blake3:..."
}
```

Then rules consume facts:

```text
facts in → decisions out
```

This keeps rules deterministic.

## CI integration

CI should not own the policy. CI should call the policy engine.

Bad:

```yaml
# CI hard-codes governance rules
- run: cargo test
- run: grep for missing docs
- run: custom script for public API
```

Better:

```yaml
- run: governor scan --emit-facts .governor/facts.jsonl
- run: governor policy eval --facts .governor/facts.jsonl --policy governance/policies/index.toml
- run: governor replay --ledger .governor/events.jsonl
```

CI becomes an executor, not the source of law.

This fits your monotonic-ledger direction: append only after message, receipt, transaction, policy, actor, and repo-state checks pass; reject stale proposal heads; and make verify/replay recompute the same ledger head. 

## Runtime integration

Runtime should use the same policy engine, but with runtime facts.

Example runtime fact:

```json
{
  "fact_type": "capability_requested",
  "capability_type": "MachineCreation",
  "actor": "llm_bootstrap_agent",
  "target_paths": [
    "crates/governor-machines/src/machines/error_decomposer.rs"
  ],
  "prior_state_hash": "blake3:..."
}
```

Runtime asks:

```text
May this actor receive this capability?
```

Policy replies:

```json
{
  "decision": "deny",
  "rule_id": "machine_creation.requires_policy_review",
  "required_actions": [
    "complete MachineCreationProposal ceremony",
    "attach overlap analysis",
    "attach test plan"
  ]
}
```

Same engine. Different fact source.

## Rule addition during runtime

Adding a rule is itself a governed transition.

Do not let the system do:

```text
agent proposes rule → rule appears
```

Use:

```text
RuleProposal
→ schema validation
→ conflict analysis
→ shadow evaluation
→ approval/capability
→ policy index update
→ replay
```

A runtime rule addition should produce a receipt:

```json
{
  "event_type": "PolicyRuleAdded",
  "rule_id": "machine.creation.requires_overlap_check",
  "policy_file": "governance/policies/machine_creation.toml",
  "prior_policy_hash": "blake3:...",
  "next_policy_hash": "blake3:...",
  "shadow_eval": {
    "old_failures": 0,
    "new_failures": 3,
    "affected_ceremonies": ["MachineCreation"]
  },
  "approved_by": ["policy_engine", "human:arty"],
  "state_hash": "blake3:..."
}
```

That way runtime policy evolution is allowed, but it is not casual.

## Add `shadow_eval`

Before a new rule becomes active, run it against known cases:

```text
accepted proposals
rejected proposals
known hostile paths
CI failure traces
machine creation examples
policy change examples
```

The question is:

```text
What would this rule have changed?
```

Possible results:

```json
{
  "shadow_eval_result": {
    "would_block_existing_valid_cases": 0,
    "would_catch_known_bad_cases": 4,
    "would_create_ambiguous_cases": 1,
    "recommendation": "accept_with_warning"
  }
}
```

This prevents policy additions from silently breaking the system.

## Conflict handling

You need a hard rule for conflicts.

I’d use:

```text
If two policies conflict, choose the more restrictive result unless an explicit override rule exists.
```

Decision order:

```text
deny > require_review > allow_with_warning > allow
```

So if one rule says:

```text
allow machine creation
```

and another says:

```text
deny because missing test plan
```

the result is:

```text
deny
```

No ambiguity.

## Rule classes

Define policy classes early:

```text
ci_policy
  what CI must check

runtime_policy
  what the live system may do

capability_policy
  who/what may receive authority

ceremony_policy
  what evidence each ceremony requires

activation_policy
  what paths may wake up

artifact_policy
  what files/classes may be modified

training_policy
  what can become training data

constitutional_policy
  what changes the change-system itself
```

This lets you route decisions without making every rule global.

## Policy severity

Use severity, but keep it operational:

```text
info
  record only

warn
  pass but produce finding

review_required
  block automatic transition, allow human/policy review

deny
  block transition

fatal
  block and require constitutional repair path
```

Avoid vague severities like “medium” unless they map to behavior.

## Minimal Rust shape

At the core:

```rust
pub struct PolicyEngine {
    registry: PolicyRegistry,
    evaluator: RuleEvaluator,
}

pub struct PolicyInput {
    pub state_hash: Hash,
    pub facts: Vec<Fact>,
    pub actor: ActorId,
    pub action: GovernedAction,
}

pub struct PolicyDecision {
    pub decision: DecisionKind,
    pub matched_rules: Vec<RuleId>,
    pub required_actions: Vec<RequiredAction>,
    pub evidence: Vec<EvidenceRef>,
}
```

Decision kind:

```rust
pub enum DecisionKind {
    Allow,
    AllowWithWarnings,
    RequireReview,
    Deny,
    Fatal,
}
```

For rule loading:

```rust
pub struct PolicyRegistry {
    pub active_policy_set: PolicySetId,
    pub policy_hash: Hash,
    pub rules: Vec<Rule>,
}
```

The policy hash is important. Every decision should say which policy version produced it.

## Rule authoring example

```toml
[[rule]]
id = "public_api.requires_tests"
class = "ci_policy"
severity = "deny"
description = "Public API changes require tests."

[rule.when]
all = [
  { fact = "public_api_changed", equals = true }
]

[rule.require]
all = [
  { fact = "tests_changed", equals = true }
]

[rule.decision]
on_missing = "deny"
message = "Public API changed without corresponding tests."
```

Another:

```toml
[[rule]]
id = "machine_creation.requires_overlap_check"
class = "ceremony_policy"
severity = "review_required"

[rule.when]
all = [
  { action = "create_machine" }
]

[rule.require]
all = [
  { evidence = "existing_machine_overlap_check", present = true },
  { evidence = "machine_responsibility_boundary", present = true }
]
```

## What the LLM can do

The LLM can help add rules, but only as a proposer.

Allowed:

```text
- suggest rule
- explain why rule is needed
- find similar past cases
- draft TOML
- run shadow eval
- summarize impact
```

Not allowed:

```text
- directly activate new rule
- weaken deny rules
- approve its own rule
- train on its own rule without review
```

That matches the principle you already have: the model may propose, but accepted state is decided by the event log and reviewed transitions. 

## MVP build order

Build this in order:

```text
1. Fact schema
2. Rule schema
3. Policy registry
4. Rule evaluator
5. Decision output format
6. CI command: governor policy eval
7. Runtime command: governor policy ask
8. Ledgered decision receipts
9. Rule proposal ceremony
10. Shadow evaluation for new rules
```

Do not start with a complex DSL. Start with:

```text
all / any / not
fact equals
fact present
evidence present
action equals
path matches
risk >= threshold
```

That is enough.

## The invariant

I would write the policy engine around this:

```text
No CI result, runtime mutation, capability grant, path activation, or rule addition is authoritative unless it was evaluated against the active policy set hash and recorded as a decision event.
```

That gives you the single location you want without making the system brittle.

The policy engine becomes the courthouse:

```text
facts enter
rules are cited
decisions are issued
receipts are written
state moves only if the decision allows it
```
