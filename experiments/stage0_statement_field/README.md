# Stage 0 — Toy statement field (local, reproducible)

This experiment instantiates the **resistance-modulated softmax** transition model from the archived discussion (`chat_download_transformer_llm_layers/07_phase_stage0_execution.md`). It is a **controlled toy**: results depend on hand-specified matrices `K` and `R`.

## Scientific intent

**Claim under test (toy level):** policy-like behavior can appear as **reweighted continuation probabilities** via a resistance term, without setting any transition probability to zero.

**Not claimed:** alignment in real LLMs, identification of “true” statement nodes, or robustness under adversarial inputs.

## Formal rule

For capacitance parameter \(q \in [0,1]\):

\[
P(\text{next} \mid \text{current}, q) = \mathrm{softmax}\bigl(K[\text{current},:] - q \cdot R[\text{current},:]\bigr)
\]

- `K`: base compatibility (learned or hand-set in Stage 0).
- `R`: resistance profile (structural; in Stage 0, hand-set).
- `q`: scalar “charge” modulating resistance (not a truth label).

## Reproducibility

1. Create a virtual environment and install dependencies from the repo root:

   ```bash
   pip install -r requirements.txt
   ```

2. Run the experiment (writes CSV/JSON under `results/`):

   ```bash
   cd /path/to/encoding-experiment
   python -m experiments.stage0_statement_field.run
   ```

3. Lock numerics with tests:

   ```bash
   pytest tests/test_stage0_statement_field.py -v
   ```

Outputs include:

- `manifest.json` — environment, git revision (if available), sweep definition.
- `sweep_by_source.csv` — full \(q\) sweep for sources S4, S5, S6, S13.
- `sweep_key_q.csv` — rows at \(q \in \{0, 0.5, 1\}\) for quick comparison to the transcript.
- `top6_key_q.txt` — top-6 next-state distributions at key \(q\).
- `paths_from_S2.txt` — beam-searched length-4 paths from S2.
- `path_metrics.json` — canonical path probability \(S2 \to S6 \to S9 \to S10\) vs \(q\).

## Honest limitations (from the source discussion)

- **Partly built in:** interesting gradients are encoded in `K` and `R`; Stage 0 shows **internal consistency**, not discovery from data.
- **No learning:** Stage 1 would replace hand-`K` with a learned kernel and re-apply `R` at inference.

## Changing the experiment

- Edit only `matrices.py` to change the frozen specification, then re-run and update tests if you intentionally change the science.
