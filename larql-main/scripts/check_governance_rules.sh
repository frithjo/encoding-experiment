#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

mkdir -p target/governance
rm -f \
  target/governance/invariant-registry-receipt.json \
  target/governance/policy-decisions.jsonl \
  target/governance/policy-compile-report.json \
  target/governance/policy-shadow-eval-report.json
cargo run -q -p larql-cli --bin larql -- machine rules verify \
  --registry governance/invariants/registry.json \
  --out target/governance/invariant-registry-receipt.json >/dev/null
cargo run -q -p larql-cli --bin governor -- policy governance-compile \
  --policy governance/policies/index.toml \
  > target/governance/policy-compile-report.json
cargo run -q -p larql-cli --bin larql -- machine rules eval \
  --facts governance/facts/ci_governance_rules.json \
  --policy governance/policies/index.toml \
  --ledger target/governance/policy-decisions.jsonl >/dev/null
cargo run -q -p larql-cli --bin governor -- replay \
  --ledger target/governance/policy-decisions.jsonl >/dev/null
for cases in governance/policies/*_cases.toml; do
  cargo run -q -p larql-cli --bin larql -- machine rules test \
    --cases "$cases" \
    --policy governance/policies/index.toml >/dev/null
done
for cases in governance/tests/*_cases.toml; do
  cargo run -q -p larql-cli --bin governor -- policy test \
    --cases "$cases" \
    --policy governance/policies/index.toml >/dev/null
done
cargo run -q -p larql-cli --bin larql -- machine rules shadow-eval \
  --proposal governance/proposals/policy_shadow_eval_fixture.json \
  --cases governance/proposals/policy_shadow_eval_fixture_cases.toml \
  --policy governance/policies/index.toml \
  > target/governance/policy-shadow-eval-report.json
cargo run -q -p larql-cli --bin larql -- machine decision-surface validate \
  --registry governance/decision_surface/registry.toml \
  --budgets governance/ceremonies/budgets.toml \
  --roles governance/defaults/llm_roles.toml >/dev/null
