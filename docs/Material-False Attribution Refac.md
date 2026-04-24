 # Material-False Attribution Refactor

  ## Summary

  - Refactor the batch-DLA path from a prompt-only scanner into an
    analysis-aware pipeline that accepts explicit truth/coherence
    annotations, traces either one next-token step or a bounded
    continuation, and returns structured evidence for:
      - exact false-origin layer/head/token
      - coherence-vs-false-content attribution flow
      - ridge-artifact accumulation across layers
  - Keep the existing recipe export and footer feedback behavior, but
    rebase it on a typed recipe/manual analysis spec.
  - Make this a hard-cut refactor. No compatibility loader, no description
    parsing, no placeholder UI pretending a signal exists when it does
    not.

  ## Scope Definitions

  - In scope:
      - recipe schema refactor in larql-main/recipes/recipes.json
      - inference capture and traced generation in larql-main/crates/
        larql-inference/src/forward/predict.rs, larql-main/crates/larql-
        inference/src/forward/mod.rs, and larql-main/crates/larql-
        inference/src/layer_graph/generate.rs
      - server tool contract in larql-main/crates/larql-server/src/routes/
        tools.rs
      - TUI request/state/rendering/manual-target flow in larql-main/
        crates/larql-terminal-batch-dla/src/lib.rs
      - end-to-end proving tests in larql-main/crates/larql-server/tests/
        test_tools.rs and the existing test module in larql-main/crates/
        larql-terminal-batch-dla/src/lib.rs
  - Out of scope:
      - editing hypotheses or experiment docs
      - semantic truth discovery from arbitrary prompts without explicit
        annotations
      - compatibility parsing for old recipe entries
      - unrelated circuit-detection or renderer expansion
  - Required hard cuts:
      - recipe semantics move out of free-text description
      - sincerity_threshold is replaced by an explicit ridge dead-zone
        field in the typed analysis block
      - the current fake seq_len x seq_len attention heatmap is replaced
        with a truthful last-token source-strip view

  ## Public Interfaces

  - recipes/recipes.json becomes typed per entry:
      - name
      - prompt
      - description as display-only text
      - ui_defaults: { layer, head }
      - analysis: { mode, truth_span, materially_false_spans,
        coherence_markers, max_generated_tokens, ridge_dead_zone }
  - batch_dla_scan request gains analysis for both recipe-driven and
    manual-target scans.
  - batch_dla_scan response gains:
      - head_dla
      - generation_trace
      - token_analysis
      - analysis_summary
      - ridge_by_layer
  - token_analysis must include per analyzed position:
      - emitted/predicted token text
      - matched truth/false/coherence labels
      - true-vs-false mass
      - top contributing (layer, head, source_token, contribution) entries
        for both coherence and false-content flows
  - analysis_summary must include:
      - first_false_origin
      - first_false_position
      - first_false_token
      - top_coherence_heads
      - top_false_content_heads

  ## Implementation Checklist

  ### 1. Truth Origin Tracking

  Files: larql-main/recipes/recipes.json, larql-main/crates/larql-
  inference/src/forward/predict.rs, larql-main/crates/larql-inference/src/
  forward/mod.rs, larql-main/crates/larql-inference/src/layer_graph/
  generate.rs, larql-main/crates/larql-server/src/routes/tools.rs, larql-
  main/crates/larql-terminal-batch-dla/src/lib.rs

  - [ ] Replace loose recipe truth hints with an explicit analysis block
    on every recipe.
  - [ ] Add a multi-token span matcher so truths like Birnin Zana and Von
    Doom are matched as spans, not guessed from one token.
  - [ ] Extend the inference path to capture head_dla, logit_lens, and
    residual snapshots for every analyzed position.
  - [ ] Reuse the existing generation loop for workflow probes by adding a
    traced generation variant, not a second disconnected implementation.
  - [ ] Define first_false_origin as the earliest analyzed position where
    a materially false span becomes the emitted token or the dominant
    predicted continuation and its false mass exceeds true mass after
    dead-zone application.
  - [ ] Record the exact (position, token_text, layer, head, source_token,
    contribution) for that first false origin.
  - [ ] Return that structure from the server and render it directly in
    the TUI summary pane.

  ### 2. Attribution Flow

  Files: larql-main/crates/larql-inference/src/forward/predict.rs, larql-
  main/crates/larql-server/src/routes/tools.rs, larql-main/crates/larql-
  terminal-batch-dla/src/lib.rs

  - [ ] Classify attribution per token position, not once globally per
    head.
  - [ ] false_content means positive head contribution into tokens that
    match annotated materially false spans.
  - [ ] material_coherence means positive head contribution into
    structural or instructional tokens that are outside both truth and
    false spans.
  - [ ] Permit the same head to appear in both classes at different
    positions; report it as a timeline, not a hard global label.
  - [ ] Add TUI panels for top coherence heads and top false-content heads
    at the current analyzed position and for the first false event.

  ### 3. Residual Inspection / Ridge Artifact

  Files: larql-main/crates/larql-inference/src/forward/predict.rs, larql-
  main/crates/larql-inference/src/forward/mod.rs, larql-main/crates/larql-
  server/src/routes/tools.rs, larql-main/crates/larql-terminal-batch-dla/
  src/lib.rs

  - [ ] Compute three per-layer, per-position axes from the captured
    trace: true_axis, false_axis, coherence_axis.
  - [ ] Define ridge_artifact = coherence_axis * max(0, false_axis -
    true_axis - ridge_dead_zone).
  - [ ] Clamp near-zero coherence signals to zero before ridge
    accumulation so tiny sincerity-like noise does not count.
  - [ ] Return both ridge_by_layer and a token-position ridge timeline.
  - [ ] Render a compact ridge strip/bar view in the TUI and tie cursor
    movement to the same analyzed position as the attribution panels.

  ### 4. Recipes and Manual Target Mode

  Files: larql-main/recipes/recipes.json, larql-main/
  experiments/06_backprop_insert/results/ground_truth.json, larql-main/
  crates/larql-terminal-batch-dla/src/lib.rs

  - [ ] Rewrite the four initial recipes to the new typed schema.
  - [ ] Set exact truth spans from larql-main/
    experiments/06_backprop_insert/results/ground_truth.json: Freedonia
    capital Markov, Wakanda capital Birnin Zana, Latveria ruler Von Doom.
  - [ ] Update the workflow recipe to carry explicit workflow_probe
    annotations instead of relying on prose description.
  - [ ] Move target_layer and target_head under ui_defaults.
  - [ ] Add a manual-target mode in the TUI with fields for prompt, mode,
    truth_span, materially_false_spans, coherence_markers, and
    max_generated_tokens.
  - [ ] Make recipes and manual targets feed the same internal request
    struct.
  - [ ] Reject recipe entries that do not carry the new analysis block. No
    fallback parsing.

  ### 5. Existing Export / Status / Startup Behaviors

  Files: larql-main/crates/larql-terminal-batch-dla/src/lib.rs, larql-
  main/recipes/recipes.json

  - [ ] Keep C in the recipe list as the export key.
  - [ ] Export the exact currently selected recipe or manual prompt as
    EXPLAIN INFER "..." TOP 5;.
  - [ ] Keep the exported LQL visible in the footer/status bar immediately
    after export.
  - [ ] Preserve startup recipe preload so the four failure modes are
    ready on launch.
  - [ ] Rework the attention rendering so it shows the real last-token
    source strip rather than a fabricated square heatmap.

  ## Success Criteria

  - The TUI loads the four typed recipes at startup with no compatibility
    path.
  - Each recipe scan returns a populated analysis_summary, token_analysis,
    head_dla, and ridge_by_layer.
  - Fact probes report either:
      - an exact first_false_origin, or
      - an explicit “no materially false token detected” state.
  - Workflow probes run a bounded continuation trace and report the first
    materially false generated or dominant predicted token if one appears.
  - The analysis clearly separates coherence attribution from false-
    content attribution for the same run and the same token timeline.
  - Ridge output is zero when coherence contribution is below the
    configured dead-zone and non-zero only when coherence overlays
    structurally false pressure.
  - Wakanda and Latveria recipes use the exact multi-token truths from
    ground truth, not the current stale shorthand.
  - C export still works and the footer echoes the exact LQL immediately.
  - The UI no longer displays a fake square attention heatmap for last-
    token attention data.
  - No placeholder/stub fields remain in the new analysis path. If a field
    is present in the TUI, it must be backed by real returned data.

  ## Test Plan

  - cargo metadata --no-deps --format-version 1 after any manifest
    changes.
  - cargo test -p larql-inference with focused cases for:
      - multi-token span matching
      - first-false-origin detection
      - per-token coherence vs false-content classification
      - ridge dead-zone clamping
  - cargo test -p larql-server --test test_tools with a spawned server
    proving:
      - analysis request parsing
      - head_dla serialization
      - analysis_summary presence
      - workflow continuation traces in the tool response
  - cargo test -p larql-terminal-batch-dla proving:
      - typed recipe loading
      - truthful strip rendering dimensions and non-empty analysis panes
    pass.

  ## Assumptions and Defaults

  - Advanced false-origin/coherence/ridge analysis runs only when a recipe
    or manual target provides an explicit analysis block.
  - Default workflow trace budget is 32 generated tokens unless the recipe
    overrides it.
  - Truth and false detection are exact decoded-span matches against the
    supplied annotations, not open-ended semantic entailment.
  - Hypotheses and their docs remain read-only. Any contradiction
    discovered by the new analysis is recorded as an observation/anomaly,
    not “fixed” in the hypothesis text.
  - No shims: old recipe entries, old field names, and old fake
    visualization behavior are removed rather than silently supported.
