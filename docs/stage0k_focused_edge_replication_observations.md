# Stage 0k: Focused Edge Replication Observations

## Scientific intent

Stage `0j` compared four preregistered edges under constructed twins. Two stood out:

1. **`benign_assessed -> clarify`** — aggregate positive delta on strict `H3` (present minus absent twin rates), concentrated in `learning_seed=0`.
2. **`risky_assessed -> refuse`** — strong aggregate negative delta on strict `H3`.

Stage `0k` **does not redefine hypotheses**. It holds H1–H5 and thresholds fixed and asks a narrower question:

> Under constructed twins that fix all geometry except one edge, do those two directional hints **replicate** when we expand learning seeds and learned-K sample sizes?

## Preregistered protocol

Fixed before inspecting expanded results (same defaults as `experiments/stage0k_focused_edge_replication/run.py`):

| Field | Value |
|--------|--------|
| Target edges | `benign_assessed->clarify`, `risky_assessed->refuse` |
| Learning seeds | `0` through `7` |
| `n_samples_per_state` | `600`, `1200`, `2400` |
| Template seeds | `2000`–`2015` (16 templates) |
| `eval_rollouts` | `360` |
| `p_delete` / `p_add` | `0.12` / `0.06` |

Total rows expected in `constructed_pair_evaluations.csv`: **768** (`2 × 8 × 3 × 16`).

Authoritative snapshot: `experiments/stage0k_focused_edge_replication/results/protocol.json` after a completed run.

## Execution note

A full-grid run can take tens of minutes (many `learn_kernel` calls and twin evaluations). The Stage `0k` runner appends each `(edge, learning_seed, n_samples)` chunk to CSV as it completes so partial data survive interruption; see `experiments/stage0k_focused_edge_replication/README.md`.

## Aggregate results

Source files (completed run):

- `experiments/stage0k_focused_edge_replication/results/constructed_pair_evaluations.csv`
- `experiments/stage0k_focused_edge_replication/results/constructed_pair_summary.csv`
- `experiments/stage0k_focused_edge_replication/results/summary.json`

Row count check: **768** data rows (**2** edges × **8** learning seeds × **3** sample sizes × **16** templates), matching `summary.json`.

### 1) Pool over all 768 twin comparisons

Rates are **strict `H3_rollout_shift_without_exclusion`** pass rates (Stage `0b` definition unchanged).

| Target edge | Mean present `H3` | Mean absent `H3` | Mean (present − absent) | Pairs: present passes, absent fails | Pairs: present fails, absent passes |
|-------------|-------------------|------------------|-------------------------|---------------------------------------|-------------------------------------|
| `benign_assessed -> clarify` | **0.331** | **0.307** | **+0.023** | **15** | **6** |
| `risky_assessed -> refuse` | **0.320** | **0.503** | **−0.182** | **2** | **72** |

**Read:** Over the expanded grid, `benign_assessed -> clarify` stays **mildly favorable** on average for strict `H3` on the present twin. `risky_assessed -> refuse` remains **strongly unfavorable** on average: when that edge is present, strict `H3` is less likely than on the absent twin, and asymmetric flips overwhelmingly favor absent.

These are **observations under this protocol**, not edits to H3 wording or thresholds.

### 2) Stability across the summary grid (`edge` × `learning_seed` × `n_samples`)

`constructed_pair_summary.csv` has **24** rows per edge (**8** seeds × **3** sample sizes). “**Present minus absent**” here is **`present_minus_absent_H3_rate`** (difference of *rates* within that cell, each rate averaged over **16** templates).

**`benign_assessed -> clarify`**

- Cells with **strictly positive** `present_minus_absent_H3_rate`: **4 / 24**.
- Largest positive deltas occur at **`learning_seed = 0`** across sample sizes (**+0.25** to **+0.375** in rate units).
- Several seeds show **no separation** (`present_minus_absent_H3_rate = 0`) because **both** twins share the same `H3` outcome pattern (including cases where **both** sides pass `H3` for all **16** templates, e.g. **`learning_seed` 4** and parts of **7** — in those cells the toggle does not change strict `H3` at all).
- **`learning_seed = 5`** shows **negative** deltas (**−0.125** to **−0.062**), so the mild **positive** pooled mean should not be read as a universal sign.

**`risky_assessed -> refuse`**

- Cells with **strictly negative** `present_minus_absent_H3_rate`: **8 / 24**.
- The negative pooled mean is driven especially by **`learning_seed` 0** and **5**, where absent twins can reach **`absent_H3_rate = 1.0`** while present twins stay near **0.25–0.375**.
- Many seeds again show **no separation** (`0`) when both twins fail `H3` everywhere, or both pass everywhere (**`learning_seed` 4**, **7**).

**Sensitivity to `n_samples_per_state`:** Within a fixed `(edge, learning_seed)`, deltas usually move modestly across **600 / 1200 / 2400**. There is **no** simple story that doubling or tripling `K_hat` sample size uniformly flips the sign of the twin gap; the dominant pattern remains **seed-dependent**.

### 3) Honest interpretation (still not a hypothesis change)

- **Pooled replication:** The Stage `0j` **direction** is broadly **recovered** in the mean: **`benign_assessed -> clarify`** slightly helps strict `H3` on the present twin; **`risky_assessed -> refuse`** hurts it strongly on average.
- **Fine-grained replication:** Neither edge produces a **uniform sign** across all **24** summary cells. Zeros from **tied outcomes** are common; strong positives for **`benign`** concentrate in **`learning_seed = 0`**; strong negatives for **`risky`** concentrate in **`learning_seed = 0`** and **`5`**.
- **Scientific takeaway:** Single-edge toggles can matter under constructed twins, but **they are regime-dependent** (learned-K seed and, weakly, sample size). That continues to point at **geometry plus calibration** as the main bottleneck for strict trajectory-level criteria, rather than a single universal local lever.

Hypotheses **H1–H5** remain fixed; this section records **where** strict `H3` differs between twins, not a revised prediction statement.

## Relation to the main claim

This stage tests **local support toggles** under fixed resistance and learned-K protocols. It does not prove or disprove the global sentence about resistance versus exclusion; it narrows **when** strict trajectory-level criteria respond to one-edge differences under the preregistered Stage `0b` tests.
