# Stage 0c + Heldout Generalization — Observations (Honest Checkpoint)

Scope: synthetic fixed state graph family from Stage 0b, with resistance shaping tested under:

- **0b-learned** baseline kernel `K_hat` (learned baseline continuation geometry)
- **0c** learned resistance intensity `w` (scalar q-scale) with fixed resistance shape `R`
- **Heldout** admissibility-mask variants (new admissibility support, same semantics)

All results below come from the local implementations under `experiments/`.

## Stage 0c result (learned q-scale)

Run:
- `experiments/stage0c_learn_q_scale_resistance_vs_exclusion/run.py`
- `learning_seed=0`, `n_samples_per_state=900`, `w_grid` coarse step `0.5`
- `chosen_w = 0.5`

Hypotheses (all **True** under the *relaxed* Stage 0b-learned evaluation thresholds):
- `H1_risky_mass_drop`: passed (risky action mass ratio ≈ `0.1573`)
- `H2_nonzero_risky_support`: passed (`min_risky_probability_under_resistance ≈ 0.025216`)
- `H3_rollout_shift_without_exclusion`: passed
  - baseline risky terminal ≈ `0.2407` → resistance risky terminal ≈ `0.03733`
  - baseline safe-or-boundary terminal ≈ `0.50267` → resistance ≈ `0.62733`
- `H4_benign_help_retained`: passed
  - benign helpful terminal ≈ `0.22933` (baseline ≈ `0.25`)
- `H5_perturbation_robustness`: passed (pass rate `0.875` vs required `0.75`)

Observation:
Learning a scalar resistance intensity `w` is sufficient to recover strong-ish resistance shaping, while keeping risky admissible support nonzero (i.e., not requiring hard exclusion).

Limitation:
This is still a synthetic admissibility graph. Also, terminal-safe gain is sensitive to evaluation threshold calibration (strict Stage 0b thresholds are harsher than Stage 0b-learned thresholds).

## Heldout generalization result (admissibility variants)

Run:
- `experiments/stage0b_heldout_generalization/run.py`
- `n_variants = 14`, `rollouts_per_task = 140`
- admissibility variant parameters: `p_delete=0.12`, `p_add=0.06`

Summary (strict Stage 0b evaluation thresholds):
- `pass_all = True` for `7/14` variants
- `pass_all = False` for `7/14` variants
- Failures occur only in `H3_rollout_shift_without_exclusion`
  - `H1` always True
  - `H2` always True
  - `H4` always True

Additional numeric recomputation for terminal-level cause (using 120 rollouts/task to estimate H3 numerics):
- For **H3_passed=True** variants (7 variants):
  - mean safe-gain (resistance safe-or-boundary minus baseline) ≈ `0.145`
  - mean abs(safe_res - safe_exclusion) ≈ `0.046`
- For **H3_passed=False** variants (7 variants):
  - mean safe-gain ≈ `0.115`
  - mean abs(safe_res - safe_exclusion) ≈ `0.035`

Observation:
Resistance shaping over admissible risky actions remains robust at the *first-action* level (H1) and preserves risky admissible support (H2), and benign retention (H4) stays intact. The weak link under admissibility support shifts is the *terminal safe-or-boundary shift magnitude* required by the strict `H3` threshold.

Calibration-dependent note:
H3 is near its boundary for some variants; with different rollout counts, some borderline cases can flip.

## Strongest defensible next step

To strengthen the scientific checkpoint, the next target should be:
- either increase rollout counts for borderline heldout variants (reduce stochastic threshold flipping),
- or perform a Stage 0c-style learned intensity `w` *per variant family* to test whether resistance can adapt while still avoiding hard exclusion.
