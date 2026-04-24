 Yes, reintegration makes sense.

  But not as “make LQL call the MCP tool.”
  Do it as: move the scientific analysis contract
  into core LQL/Inference, then let server/TUI
  consume that same engine.

  Verdict

  - batch_dla_scan is no longer just a UI helper.
  - It now owns experiment-critical semantics:
      - analysis.mode
      - exact truth/false span resolution
      - coherence vs false-content attribution
      - ridge accumulation across trace
      - analysis_summary, token_analysis,
        generation_trace, ridge_by_layer
  - LQL already owns the adjacent semantic surface:
      - INFER in larql-main/crates/larql-lql/src/
        executor/query.rs
      - EXPLAIN INFER in larql-main/crates/larql-
        lql/src/executor/query.rs
  - So keeping them separate long-term is
    duplication debt.

  Main finding
  Current split is structurally wrong for your
  experiment.

  Why:

  - LQL path returns formatted text trace only.
  - batch_dla_scan path returns structured
    scientific payload in larql-main/crates/larql-
    server/src/routes/tools.rs:946.
  - Both run inference-like logic.
  - That means future changes to false-origin
    semantics can drift between:
      - LQL EXPLAIN INFER
      - server tool batch_dla_scan
      - TUI expectations

  For this repo, that is the exact kind of
  divergence you do not want.

  What I would do
  Hard-cut reintegration in 2 layers.

  1. Core analysis engine

  - Extract batch_dla_scan scientific logic out of
    server route code.
  - New home should be a core crate, likely:
      - larql-inference if you want inference-first
        ownership
      - or larql-lql only if you believe this is
        language semantics first
  - My recommendation: larql-inference
      - because the analysis is over model behavior,
        tokens, head DLA, residuals
      - not over parsing/execution formatting

  2. LQL language surface

  - Add a real LQL statement for this analysis,
    instead of overloading plain EXPLAIN INFER.
  - Reason: the payload and semantics are not just
    “explain inference”; they are experimental
    attribution analysis with explicit truth/
    coherence annotations.

  Do not

  - Do not make larql-lql shell out to
    batch_dla_scan
  - Do not keep server route as canonical and add a
    thin LQL adapter
  - Do not stuff this into EXPLAIN INFER with hidden
    magic defaults
  - Do not parse recipe prose or natural-language
    hints back into semantics

  That would recreate the same split under a
  different name.

  Recommended language shape
  Best option:

  - new statement family, something like:

  ANALYZE INFER "The capital of Freedonia is"
  MODE FACT_PROBE
  TRUTH_SPANS ["Markov"]
  FALSE_SPANS ["Paris", "London"]
  COHERENCE_MARKERS ["The", "capital", "of", "is"]
  TOP 5;

  or:

  EXPLAIN ANALYZE INFER ...

  I would not overload current EXPLAIN INFER unless
  you want permanent ambiguity between:

  - per-layer feature trace
  - scientific false/coherence attribution run

  Those are related, but not identical products.

  Why new syntax is better

  - explicit experiment contract
  - explicit annotations
  - no fake inference that “guesses” truth
    conditions
  - cleaner parser/executor separation
  - better REPL / CLI / remote-server stability
  - preserves current EXPLAIN INFER expectations for
    users who only want attention + trace

  Concrete refactor seam
  Current files that matter:

  Language / executor:

  - larql-main/crates/larql-lql/src/ast.rs
  - larql-main/crates/larql-lql/src/executor/
    query.rs
  - parser files under larql-main/crates/larql-lql/
    src/parser

  Current scientific tool surface:

  - larql-main/crates/larql-server/src/routes/
    tools.rs:946

  Inference capture:

  - larql-main/crates/larql-inference/src/forward/
    predict.rs
  - larql-main/crates/larql-inference/src/forward/
    mod.rs

  TUI consumer:

  - larql-main/crates/larql-terminal-batch-dla/src/
    lib.rs

  Spec already points this direction:

  - larql-main/docs/lql-spec.md:1428
      - “machinery exists, language doesn't”

  That is basically the repo already telling you
  this seam should close in LQL.

  Recommended architecture
  Canonical flow should become:

  - larql-inference
      - exposes structured analysis API
  - larql-lql
      - parses new analysis statement
      - executes via structured analysis API
      - formats human-readable output for REPL/CLI
  - larql-server
      - tool route becomes transport adapter over
        same analysis API
  - larql-terminal-batch-dla
      - either:
          - keeps calling server JSON endpoint, or
          - eventually runs LQL statement and
            consumes structured JSON if LQL gets
            machine-readable output mode

  Important design decision
  You need one canonical result type.

  Right now the scientific contract effectively
  lives in the tool response:

  - predictions
  - logit_lens
  - head_dla
  - generation_trace
  - token_analysis
  - analysis_summary
  - ridge_by_layer

  That should become a real shared type in core
  code, not route-local JSON assembly.

  If you do not do that, reintegration will look
  complete but still drift.

  Biggest risks

  1. Overloading EXPLAIN INFER

  - risk: semantic confusion
  - result: users cannot tell if they are getting
    plain trace or experimental truth-analysis

  2. Putting experiment semantics in parser/executor
     only

  - risk: server tool and TUI still duplicate logic
    elsewhere

  3. Keeping route-local JSON as canonical

  - risk: LQL remains second-class and will drift
    again

  4. Formatting-first implementation

  - risk: REPL text becomes source of truth instead
    of structured data
  - bad fit for a scientific experiment

  5. Recipe-driven semantics leaking into language

  - recipes are UI convenience
  - LQL should accept explicit analysis clauses, not
    import TUI recipe assumptions

  My recommendation
  Proceed with reintegration, but with this
  boundary:

  - canonical semantics in larql-inference
  - canonical typed result in core crate
  - new explicit LQL analysis statement
  - server batch_dla_scan rewritten as adapter over
    same core API
  - TUI recipes compile to explicit LQL analysis
    clauses, not custom hidden JSON semantics

  That gives you:

  - one experiment engine
  - one language surface
  - one transport surface
  - less drift
  - no shimmy compatibility layer

  Success criteria for reintegration
  It is only truly closed when all are true:

  1. same prompt + same analysis spec yields same
     first_false_origin through both LQL and server
  2. ridge totals match between LQL and
     batch_dla_scan
  3. truth/false span resolution is implemented once
  4. TUI recipe export emits a real executable LQL
     analysis statement, not lossy EXPLAIN INFER
     "..." TOP 5;
  5. batch_dla_scan route becomes a thin adapter,
     not its own analysis owner
  6. EXPLAIN INFER behavior remains stable unless
     you intentionally redefine it in spec and code
     together

  Bottom line

  - Reintegration: yes
  - Reintegrate into LQL as first-class analysis
    language, not as wrapper glue
  - Keep batch_dla_scan only as a transport/UI
    adapter after the cutover
  - Use a new statement, not hidden behavior inside
    existing EXPLAIN INFER
