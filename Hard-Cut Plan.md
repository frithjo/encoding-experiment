Hard-Cut Plan

  Goal:

  - make LQL the first-class surface for batch DLA
    scientific analysis
  - remove duplicated semantics between LQL and
    batch_dla_scan
  - keep one canonical experiment engine
  - no shims, no dual ownership

  ## Scope

  In:

  - shared scientific analysis engine
  - shared typed result contract
  - new explicit LQL analysis statement
  - server tool rewritten as adapter
  - TUI recipe export upgraded to real LQL analysis
    statement
  - docs/spec updated to match parser and
    implementation

  Out:

  - backwards-compatible parser aliases
  - old recipe prose parsing
  - hidden fallback from plain EXPLAIN INFER into
    analysis mode
  - fake UI surfaces unsupported by real engine
    output

  ## Hard Cut Decisions

  1. batch_dla_scan stops owning analysis semantics.
  2. LQL gets a new statement, not an overloaded
     EXPLAIN INFER.
  3. Canonical analysis code lives in larql-
     inference.
  4. Canonical result types live in core code, not
     route-local JSON.
  5. TUI recipes export executable analysis LQL, not
     lossy prompt-only EXPLAIN INFER.
  6. Existing plain INFER and EXPLAIN INFER stay
     semantically stable.

  ## Proposed LQL Surface

  Recommended statement:

  ANALYZE INFER "The capital of Freedonia is"
  MODE FACT_PROBE
  TRUTH_SPANS ["Markov"]
  FALSE_SPANS ["Paris", "London"]
  COHERENCE_MARKERS ["The", "capital", "of", "is"]
  TOP 5;

  Workflow mode:

  ANALYZE INFER "Step-by-step guide to becoming the
  president of Freedonia"
  MODE WORKFLOW_PROBE
  FALSE_SPANS ["president of Freedonia"]
  COHERENCE_MARKERS ["Step", "guide", "becoming",
  "president"]
  MAX_GENERATED_TOKENS 32
  RIDGE_DEAD_ZONE 0.05
  TOP 5;

  Optional concise form later, not in first cut.

  ## Canonical Data Model

  Create one shared analysis request/result
  contract.

  ### Request

  Fields:

  - prompt
  - top_k
  - mode
  - truth_spans
  - materially_false_spans
  - coherence_markers
  - max_generated_tokens
  - ridge_dead_zone

  ### Result

  Fields:

  - predictions
  - attention
  - logit_lens
  - head_dla
  - generation_trace
  - token_analysis
  - analysis_summary
  - ridge_by_layer
  - tokens
  - strings
  - num_layers
  - seq_len

  This should be a real Rust type, not route-only
  JSON assembly.

  ## File-Level Plan

  ### 1. Move scientific engine into core inference

  Files:

  - larql-main/crates/larql-server/src/routes/
    tools.rs
  - larql-main/crates/larql-inference/src/forward/
    predict.rs
  - larql-main/crates/larql-inference/src/forward/
    mod.rs
  - likely new file:
      - larql-main/crates/larql-inference/src/
        analysis.rs

  Changes:

  - extract all analysis-specific types from server
    route
  - extract:
      - mode parsing enum
      - span encoding/normalization
      - exact span resolution
      - false/coherence attribution
      - ridge accumulation
      - generation loop analysis orchestration
  - expose one public API like:
      - analyze_infer(...) -> AnalysisResult

  Success criteria:

  - server no longer contains canonical false-
    origin/ridge logic
  - one call in core produces the full structured
    analysis result

  ### 2. Add LQL AST support

  Files:

  - larql-main/crates/larql-lql/src/ast.rs

  Changes:

  - add Statement::AnalyzeInfer { ... }
  - add AnalysisMode enum on LQL side if not reusing
    shared one
  - include explicit fields for spans, markers, max
    tokens, top, dead zone

  Success criteria:

  - AST can represent full scientific analysis
    without lossy side channels

  ### 3. Add parser support

  Files:

  - parser files under larql-main/crates/larql-lql/
    src/parser

  Changes:

  - parse ANALYZE INFER
  - parse clauses:
      - MODE
      - TRUTH_SPANS
      - FALSE_SPANS
      - COHERENCE_MARKERS
      - MAX_GENERATED_TOKENS
      - RIDGE_DEAD_ZONE
      - TOP
  - reject missing required clauses where mode needs
    them
  - no compatibility alias from old syntax

  Success criteria:

  - parser accepts the new grammar
  - invalid/missing analysis clauses fail clearly

  ### 4. Add executor path

  Files:

  - larql-main/crates/larql-lql/src/executor/
    query.rs
  - maybe larql-main/crates/larql-lql/src/executor/
    mod.rs

  Changes:

  - dispatch Statement::AnalyzeInfer
  - load weights/tokenizer through existing LQL
    backend path
  - call shared larql-inference analysis API
  - format readable REPL output from structured
    result
  - do not reimplement scientific logic in executor

  Success criteria:

  - LQL execution uses the same core analysis engine
    as server/TUI
  - text output is a renderer over shared structured
    data

  ### 5. Rewrite server tool as adapter

  Files:

  - larql-main/crates/larql-server/src/routes/
    tools.rs

  Changes:

  - keep request/response transport shape if useful
    for TUI
  - route parses args, calls shared analysis engine,
    serializes result
  - delete route-local analysis logic once migrated

  Success criteria:

  - handle_batch_dla_scan becomes thin transport
    glue
  - no scientific decision logic remains route-local

  ### 6. Upgrade TUI recipe export

  Files:

  - larql-main/crates/larql-terminal-batch-dla/src/
    lib.rs
  - larql-main/recipes/recipes.json

  Changes:

  - export real ANALYZE INFER ... statements from
    recipes/manual mode
  - include all analysis clauses, not only prompt +
    TOP 5
  - keep status bar feedback
  - keep startup recipe preload
  - do not preserve old lossy export format

  Success criteria:

  - pressing C yields executable analysis LQL
    preserving recipe semantics
  - no information loss from recipe to LQL export

  ### 7. Update docs/spec

  Files:

  - larql-main/docs/lql-spec.md
  - larql-main/docs/lql-guide.md
  - larql-main/README.md
  - optionally larql-main/docs/ui/terminal-browser-
    spec.md

  Changes:

  - document ANALYZE INFER
  - define semantics precisely:
      - fact probe vs workflow probe
      - exact span resolution
      - total-mass truth/false dominance
      - ridge dead-zone behavior
  - remove docs that imply recipe export is only
    EXPLAIN INFER "..." TOP 5;

  Success criteria:

  - parser, docs, and implementation say the same
    thing
  - no “machinery exists, language doesn't” gap for
    this feature

  ## Checklist By Feature

  ### Feature 1: Truth Origin Tracking

  - [ ] shared result type includes
    first_false_origin
  - [ ] core engine computes exact first materially
    false position
  - [ ] exact origin includes layer/head/source
    token/contribution
  - [ ] LQL renders it
  - [ ] server returns it unchanged
  - [ ] TUI consumes same field

  ### Feature 2: Attribution Flow

  - [ ] shared token-analysis type includes
    coherence vs false-content top heads
  - [ ] core engine computes both from same run
  - [ ] LQL renderer shows per-token and summary
    view
  - [ ] server serializes same structure
  - [ ] no duplicate attribution logic in executor/
    route

  ### Feature 3: Residual Ridge Accumulation

  - [ ] core engine computes ridge_by_layer across
    full trace
  - [ ] dead-zone semantics preserved exactly
  - [ ] LQL output includes ridge summary
  - [ ] server returns same structure
  - [ ] TUI remains truthful strip/ridge consumer

  ### Feature 4: Discovery to Query

  - [ ] recipe export emits real ANALYZE INFER
  - [ ] all recipe analysis fields map into LQL
    clauses
  - [ ] manual mode exports same statement shape
  - [ ] footer/status bar shows exported query
    verbatim

  ## Migration Order

  1. extract shared analysis types + engine into
     larql-inference
  2. port server tool to shared engine
  3. add LQL AST + parser
  4. add LQL executor path
  5. upgrade TUI export to ANALYZE INFER
  6. update docs/spec
  7. remove any now-dead route-local helpers

  This order keeps one canonical engine early and
  avoids building a second copy in LQL.

  ## Testing / Proof Plan

  ### Core

  - cargo test -p larql-inference
  - add tests for:
      - exact span resolution
      - total-mass label dominance
      - mode semantics
      - ridge accumulation
      - first false origin

  ### LQL

  - cargo test -p larql-lql
  - add tests for:
      - parser acceptance/rejection
      - executor output on ANALYZE INFER
      - same prompt/spec producing same semantic
        result as shared engine

  ### Server

  - cargo test -p larql-server --test test_tools
  - prove route output is same structured result
    from shared engine

  ### TUI

  - cargo test -p larql-terminal-batch-dla
  - prove recipe export now emits full ANALYZE
    INFER ...

  ## Comprehensive Success Criteria

  This refactor is only done when all are true:

  1. ANALYZE INFER exists in parser, AST, executor,
     and docs.
  2. LQL and batch_dla_scan use the same analysis
     engine.
  3. There is one canonical result type for
     scientific analysis.
  4. batch_dla_scan is transport glue, not semantic
     owner.
  5. same prompt + same analysis spec yields same
     false-origin/ridge/coherence results in:
      - LQL
      - server tool
      - TUI
  6. TUI export preserves full recipe semantics as
     LQL.
  7. no compatibility alias or hidden fallback
     remains.
  8. plain INFER and EXPLAIN INFER still mean what
     they meant before.

  ## Risks To Manage

  - executor reimplements logic instead of calling
    shared engine
  - route keeps partial legacy logic after migration
  - over-formatting LQL output and losing machine-
    traceability
  - recipe export omits fields and becomes lossy
    again
  - accidental semantic drift between plain EXPLAIN
    INFER and new ANALYZE INFER

  ## Recommendation

  Implement as a hard-cut language addition:

  - add ANALYZE INFER
  - centralize engine in larql-inference
  - demote batch_dla_scan to adapter
  - upgrade TUI export immediately after LQL lands

---

**Status (2026-04-24):** COMPLETED

All phases of the Hard-Cut Plan have been successfully implemented:

- ✅ Phase 0: Canonical shared request/result types defined in `larql-inference/src/analysis.rs`
- ✅ Phase 1: Scientific engine extracted and centralized in `larql-inference::analyze_infer`
- ✅ Phase 2: Server adapter cutover - `/v1/analyze-infer` uses shared engine
- ✅ Phase 3: AST node added - `Statement::AnalyzeInfer` in `larql-lql/src/ast.rs`
- ✅ Phase 4: Parser grammar implemented in `larql-lql/src/parser/query.rs`
- ✅ Phase 5: LQL executor wired in `larql-lql/src/executor/query.rs`
- ✅ Phase 6: TUI export cutover - exports full executable ANALYZE INFER statements
- ✅ Phase 7: Optional LQL machine-readable output (FORMAT JSON)
- ✅ Phase 8: Spec and docs hard cut - all documentation updated to reflect ANALYZE INFER as canonical interface
- ✅ Phase 9: Dead-code cleanup - deprecated route-local helpers removed

All success criteria met. No semantic drift between LQL, server, and TUI. One canonical experiment engine, one language surface, one transport surface.
