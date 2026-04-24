Granular Attribution (DLA)
   * Direct Logit Attribution (DLA) per Head: Currently, the tool shows
     attention heatmaps. Enrich BatchDlaResult to include how much each
     individual attention head (and MLP) contributes to the final logit of
     the predicted token.
   * Logit Lens per Layer: Expose the "Logit Lens" (decoding residuals at
     every layer) in the TUI. This allows seeing where a "materially false"
     conclusion begins to dominate the residual stream.

  2. Experiment-Specific Metadata
   * Sincerity/Calibration Metrics: Add a "sincerity" weight to predictions,
     specifically flagging the "ridge artifacts" (near-zero weights used for
     justification) mentioned in GEMINI.md.
   * Coherence vs. Truth Annotations: Allow the tool to ingest a "Ground
     Truth" overlay. If the LLM generates a workflow step that is
     "materially false," the tool should highlight the specific attention
     heads that were active during that generation, contrasting them with
     "materially true" steps.

  3. Visual & Interactive Capacity
   * Attention Flow (Cross-Layer): Instead of static per-layer heatmaps,
     implement a "flow" visualization showing how a specific "false" token's
     influence propagates through the architecture across layers.
   * Interactive Interventions: Allow "hot-swapping" tokens in the prompt
     within the TUI to see how the "material coherence" shifts
     (Counterfactual Probing).

  4. Structural Data Enrichment
   * Workflow DAG Parsing: If the prompt generates a multi-step workflow,
     parse it into a Directed Acyclic Graph (DAG). Enrich the tool to show
     the model's confidence/attention on the edges between steps to identify
     where the "materially false" logic chain breaks.
   * Information Flux: Track the magnitude of change in the residual stream.
     High flux in middle layers often correlates with "hallucination" or
     "materially false" restructuring of facts.
