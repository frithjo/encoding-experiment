 Execution Sequence

  Goal:

  - land ANALYZE INFER as first-class LQL
  - centralize scientific semantics once
  - reduce batch_dla_scan to adapter
  - preserve current INFER / EXPLAIN INFER behavior

  ## Phase 0: Freeze Contract

  Purpose:

  - define canonical shared request/result types
    before code motion

  Files:

  - new larql-main/crates/larql-inference/src/
    analysis.rs
  - update larql-main/crates/larql-inference/src/
    lib.rs
  - likely update larql-main/crates/larql-inference/
    src/forward/mod.rs

  Edits:

  - define:
      - AnalysisMode
      - AnalysisRequest
      - AnalysisResult
      - GeneratedStep
      - TokenAnalysis
      - AnalysisSummary
      - FirstFalseOrigin
      - TopHeadContribution
      - LayerRidge
  - move route-local structs into core
  - no behavior changes yet

  Checklist:

  - [ ] one shared request type exists
  - [ ] one shared result type exists
  - [ ] no JSON-only structs remain canonical

  Gate:

  - cargo test -p larql-inference --lib

  ## Phase 1: Extract Scientific Engine From Server

  Purpose:

  - move the real semantics out of route code

  Files:

  - larql-main/crates/larql-server/src/routes/
    tools.rs
  - new larql-main/crates/larql-inference/src/
    analysis.rs
  - maybe larql-main/crates/larql-inference/src/
    forward/predict.rs

  Edits:

  - migrate from server route into core:
      - span encoding
      - normalization
      - exact span resolution
      - projected/exact dominance logic
      - mode handling
      - first false origin
      - token attribution labeling
      - ridge accumulation
      - analysis loop over generation
  - add one public entrypoint, for example:
      - analyze_infer(weights, tokenizer, tokens,
        request, ffn) -> AnalysisResult
  - keep route behavior identical by calling the new
    engine

  Checklist:

  - [ ] server route no longer owns false-origin
    semantics
  - [ ] server route no longer owns ridge semantics
  - [ ] server route no longer owns span-resolution
    semantics
  - [ ] core engine returns same fields as existing
    tool response

  Gate:

  - cargo test -p larql-server --test test_tools
  - cargo test -p larql-inference --lib

  ## Phase 2: Thin Adapter Cutover For
  batch_dla_scan

  Purpose:

  - make handle_batch_dla_scan transport-only

  Files:

  - larql-main/crates/larql-server/src/routes/
    tools.rs

  Edits:

  - parse request args into AnalysisRequest
  - call shared analyze_infer
  - serialize AnalysisResult
  - delete dead helper logic from route
  - keep response shape stable for TUI unless
    intentionally changed later

  Checklist:

  - [ ] handle_batch_dla_scan only parses, calls,
    serializes
  - [ ] no scientific decision code remains in
    server route
  - [ ] all current server tests still validate same
    semantics

  Gate:

  - cargo test -p larql-server --test test_tools

  ## Phase 3: Add New LQL AST Node

  Purpose:

  - make scientific analysis first-class in language

  Files:

  - larql-main/crates/larql-lql/src/ast.rs

  Edits:

  - add:
      - Statement::AnalyzeInfer { ... }
  - fields:
      - prompt
      - top
      - mode
      - truth_spans
      - materially_false_spans
      - coherence_markers
      - max_generated_tokens
      - ridge_dead_zone

  Checklist:

  - [ ] AST can represent full analysis spec
  - [ ] AST does not rely on recipe semantics
  - [ ] no hidden fallback to Explain { mode:
    Infer }

  Gate:

  - cargo test -p larql-lql ast
      - or full crate if narrow target awkward

  ## Phase 4: Parser Grammar For ANALYZE INFER

  Purpose:

  - add explicit language entrypoint

  Files:

  - parser files under larql-main/crates/larql-lql/
    src/parser
  - maybe parser tests in existing parser test
    modules

  Edits:

  - parse:
      - ANALYZE INFER <prompt>
      - MODE FACT_PROBE|WORKFLOW_PROBE
      - TRUTH_SPANS [...]
      - FALSE_SPANS [...]
      - COHERENCE_MARKERS [...]
      - MAX_GENERATED_TOKENS <n>
      - RIDGE_DEAD_ZONE <f>
      - TOP <n>
  - reject:
      - unknown mode
      - malformed arrays
      - missing semicolon
      - invalid numeric clauses
  - decide required clauses strictly:
      - MODE required
      - FALSE_SPANS likely required for both modes
      - TRUTH_SPANS required for fact probes,
        optional for workflow probes
      - COHERENCE_MARKERS optional only if
        explicitly allowed by engine semantics

  Checklist:

  - [ ] grammar parses explicit analysis statement
  - [ ] invalid forms fail clearly
  - [ ] no alias from EXPLAIN INFER

  Gate:

  - cargo test -p larql-lql parser

  ## Phase 5: LQL Executor Wiring

  Purpose:

  - run scientific analysis through LQL backend

  Files:

  - larql-main/crates/larql-lql/src/executor/
    query.rs
  - maybe larql-main/crates/larql-lql/src/executor/
    mod.rs

  Edits:

  - dispatch Statement::AnalyzeInfer
  - load weights/tokenizer from current backend
  - call shared analyze_infer
  - render structured result into readable text
  - do not duplicate engine logic in executor

  Output should include:

  - prediction header
  - first false origin summary
  - top coherence heads
  - top false-content heads
  - per-token trace summary
  - ridge by layer summary

  Checklist:

  - [ ] LQL executor uses shared engine
  - [ ] no route helper reused from server
  - [ ] plain INFER and EXPLAIN INFER untouched

  Gate:

  - cargo test -p larql-lql

  ## Phase 6: TUI Export Cutover

  Purpose:

  - exported query becomes semantically complete

  Files:

  - larql-main/crates/larql-terminal-batch-dla/src/
    lib.rs
  - larql-main/recipes/recipes.json

  Edits:

  - replace export string:
      - from prompt-only EXPLAIN INFER "..." TOP 5;
      - to full ANALYZE INFER ...
  - include:
      - mode
      - truth spans
      - false spans
      - coherence markers
      - max generated tokens
      - ridge dead zone
      - top value
  - preserve footer/status message behavior

  Checklist:

  - [ ] C export yields executable ANALYZE INFER
  - [ ] recipe export is no longer lossy
  - [ ] manual mode export matches same shape
  - [ ] no old export format retained as fallback

  Gate:

  - cargo test -p larql-terminal-batch-dla

  ## Phase 7: Optional LQL Machine-Readable Output

  Purpose:

  - if you want TUI/server eventually consuming LQL
    directly

  Files:

  - likely LQL executor output modules
  - maybe CLI or remote surface files

  Edits:

  - add structured output mode only if needed
  - this is optional for first hard cut
  - do not block reintegration on this

  Checklist:

  - [ ] only do this if there is a real consumer
    need
  - [ ] avoid building a second structured contract
    separate from AnalysisResult

  Gate:

  - only if implemented

  ## Phase 8: Spec and Docs Hard Cut

  Purpose:

  - remove doc drift

  Files:

  - larql-main/docs/lql-spec.md
  - larql-main/docs/lql-guide.md
  - larql-main/README.md
  - maybe larql-main/docs/ui/terminal-browser-
    spec.md

  Edits:

  - document ANALYZE INFER
  - specify:
      - mode semantics
      - exact span resolution semantics
      - total-mass truth/false resolution
      - ridge dead-zone semantics
      - distinction from EXPLAIN INFER
  - remove stale export examples

  Checklist:

  - [ ] docs match parser
  - [ ] docs match implementation
  - [ ] no “machinery exists, language doesn't” for
    this surface anymore

  Gate:

  - no code gate, but do this before declaring
    closure

  ## Phase 9: Dead-Code Cleanup

  Purpose:

  - complete hard cut

  Files:

  - larql-main/crates/larql-server/src/routes/
    tools.rs
  - any old helper modules left in LQL/server/TUI

  Edits:

  - remove route-local helper leftovers
  - remove dead imports and tests tied to old
    ownership
  - keep only one engine

  Checklist:

  - [ ] no duplicate analysis helper remains
  - [ ] no stale export string remains
  - [ ] no compatibility shim remains

  Gate:

  - cargo test -p larql-inference
  - cargo test -p larql-lql
  - cargo test -p larql-server --test test_tools
  - cargo test -p larql-terminal-batch-dla

  ———

  Exact File Ownership Split

  Core engine owns:

  - larql-main/crates/larql-inference/src/
    analysis.rs
  - larql-main/crates/larql-inference/src/forward/
    predict.rs only where inference capture support
    is needed

  Language owns:

  - larql-main/crates/larql-lql/src/ast.rs
  - parser files
  - larql-main/crates/larql-lql/src/executor/
    query.rs

  Transport owns:

  - larql-main/crates/larql-server/src/routes/
    tools.rs

  UI owns:

  - larql-main/crates/larql-terminal-batch-dla/src/
    lib.rs
  - larql-main/recipes/recipes.json

  ———

  Recommended Validation Gates By Slice

  Slice 1:

  - core extraction only
  - prove with:
      - cargo test -p larql-inference --lib
      - cargo test -p larql-server --test test_tools

  Slice 2:

  - server adapter cutover
  - prove with:
      - cargo test -p larql-server --test test_tools

  Slice 3:

  - AST + parser
  - prove with:
      - cargo test -p larql-lql

  Slice 4:

  - executor
  - prove with:
      - cargo test -p larql-lql

  Slice 5:

  - TUI export
  - prove with:
      - cargo test -p larql-terminal-batch-dla

  Slice 6:

  - full closeout
  - prove with:
      - cargo test -p larql-inference
      - cargo test -p larql-lql
      - cargo test -p larql-server --test test_tools
      - cargo test -p larql-terminal-batch-dla

  ———

  Critical Non-Negotiables

  - no alias from EXPLAIN INFER to ANALYZE INFER
  - no server-owned semantic fallback
  - no recipe prose parsing
  - no second result schema
  - no TUI export that drops analysis clauses
  - no silent semantic drift between LQL and tool
    route

  ———

  What I’d implement first

  1. Phase 0
  2. Phase 1
  3. Phase 2

  Reason:

  - until the engine is centralized, any LQL work
    risks cementing duplicate semantics again
