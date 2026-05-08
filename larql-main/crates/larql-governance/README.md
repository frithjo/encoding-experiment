# larql-governance

## Crate Role

- Role: Governed machine policy and receipts
- Zone: core
- Release impact: high
- Stability target: managed

Policy, receipt, and machine primitives for governed LARQL transitions.

## Scope

- Validate machine rule profiles before runtime admission.
- Mint hash-addressed receipts and denials through capsule records.
- Derive Rust struct binding authority from parsed Rust source.
- Require a `type_need_hash` before struct minting, so the machine records why
  a new struct is necessary instead of inflating types from prose.
- Keep CLI code as dispatch only; governance semantics live here.

## Public Contract

- Admitted records carry hashes and capsules, not raw prose authority.
- Generated Rust enters through explicit machine modes and policy checks.
- Rule profiles are structured inputs. The starter profile denies arbitrary
  writes, unsafe code, unreviewed type inference, dependency addition, public
  API mutation without test/doc policy, and auto-commit without receipt.
- New dependencies require policy justification because this crate is on the
  governed runtime path.
