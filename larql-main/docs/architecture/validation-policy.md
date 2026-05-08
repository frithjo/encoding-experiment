# Validation Policy

This workspace tracks two validation outcomes separately:

1. **Hard failures** (must be zero): command exits non-zero.
2. **Non-fatal warnings** (must be disclosed): command exits zero but prints warnings.

## Rule

Validation summary must not claim "clean" if warnings are present.

- Allowed phrasing: "passed with warnings"
- Disallowed phrasing: "fully clean" when warning output exists

## Enforcement

Use `scripts/check_validation_disclosure.py`:

- runs `cargo check --workspace`
- collects runtime warning lines (`warning: ...`)
- compares against `docs/architecture/validation-warnings.md`
- fails on warning drift:
  - runtime warning not documented
  - documented warning no longer present

## Update workflow

When warning set changes:

1. run `cargo check --workspace`
2. update `docs/architecture/validation-warnings.md`
3. re-run `python3 scripts/check_validation_disclosure.py`

Keep warning entries exact. Do not paraphrase warning text.
