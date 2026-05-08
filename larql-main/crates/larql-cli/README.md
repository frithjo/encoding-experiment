# larql-cli

## Crate Role

- Role: User CLI surface
- Zone: interface
- Release impact: critical
- Stability target: stable

Command-line entrypoint for LARQL workflows (extract, query, serve, build, and
verification orchestration).

## Scope

- Parse user commands and route to crate-level implementations.
- Expose stable command UX for release-facing operations.
- Keep business logic in core crates (`larql-lql`, `larql-vindex`,
  `larql-inference`, `larql-core`) and treat this crate as interface glue.

## Public Contract

- Stable user-facing command semantics are release-critical.
- Breaking command behavior/flags require migration notes in PR/release notes.
- New command logic should prefer delegating to core crates rather than adding
  product logic directly in the CLI layer.
