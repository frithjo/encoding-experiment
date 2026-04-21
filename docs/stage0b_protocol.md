# Stage 0b Protocol — Resistance vs Exclusion

## Claim under test

Stage 0b tests stronger statement than Stage 0a:

> Behavior can be shaped by resistance over admissible statements rather than by hard exclusion.

This stage no longer asks only whether hand-written reweighting can produce desired toy pattern. It asks whether **same admissible support** can be preserved while behavior still moves toward safer trajectories.

## World design

Stage 0b uses fixed finite synthetic state graph with four layers:

1. task start states
2. assessment states
3. assistant action states
4. terminal outcome states

Task classes:

- benign
- ambiguous
- risky
- boundary

Important structural rule:

- many transitions are **inadmissible by graph design**
- only subset of outgoing edges exist from each state
- risky assistant action (`risky_detail`) remains admissible in both `baseline` and `resistance`
- only `exclusion` may remove that edge from support

## Conditions

For state `s`, admissibility mask `A(s, next)` fixed across `baseline` and `resistance`.

### Baseline

\[
P_0(next \mid s) = \mathrm{softmax}(K[s,*]) \text{ over admissible edges only}
\]

### Resistance

\[
P_R(next \mid s) = \mathrm{softmax}(K[s,*] - q(s)R[s,*]) \text{ over same admissible edges}
\]

### Exclusion

\[
P_X(next \mid s) = \mathrm{softmax}(K[s,*]) \text{ over admissible edges with risky subset masked}
\]

This is core causal comparison:

- `baseline -> resistance`: same support, different weights
- `resistance -> exclusion`: similar behavioral target, different mechanism

## Hypotheses

- `H1`: risky action mass drops sharply in risky contexts under resistance vs baseline.
- `H2`: risky admissible edges remain nonzero under resistance.
- `H3`: rollout behavior under resistance approaches exclusion on safe trajectory rate while preserving nonzero risky support.
- `H4`: benign helpful behavior stays largely intact under resistance.
- `H5`: H1-H4 remain true under small perturbations to `K` and `R`.

## Preregistered thresholds

- minimum risky admissible probability under resistance: `>= 1e-4`
- mean risky action mass in risky contexts under resistance: `<= 35%` of baseline
- risky terminal rate under resistance: `<= 45%` of baseline
- safe-or-boundary terminal rate gain under resistance: `>= +0.15` absolute vs baseline
- safe-or-boundary terminal rate under resistance: within `0.12` absolute of exclusion
- benign helpful terminal rate under resistance: no more than `0.10` absolute below baseline
- benign refusal/safe-alt mass drift under resistance: `<= 0.08`
- perturbation pass rate for H1-H4 under sigma `0.05`: `>= 0.80`

These are calibration-dependent thresholds. They do not claim universality.

## Metrics

### Per-task first-action metrics

Computed from task start state by marginalizing over:

`start -> assessment -> first assistant action`

Reported masses:

- direct help
- clarify
- refuse
- safe alternative
- high-level explanation
- risky detail
- boundary reply
- risky action mass
- safe action mass
- refusal + safe alternative mass

### Rollout metrics

Monte Carlo rollouts from each task start state report:

- helpful terminal hit rate
- safe terminal hit rate
- risky terminal hit rate
- boundary terminal hit rate
- safe-or-boundary terminal hit rate
- mean steps to terminal

### Distance metrics

For first-action distributions:

- KL divergence
- total variation distance

Between:

- resistance vs baseline
- resistance vs exclusion

### Robustness

Add Gaussian noise to admissible entries of `K` and `R` only. Re-run H1-H4. Report fraction of draws that still pass.

## Falsifiers

Stage 0b fails if any of these hold:

- resistance effect only works by effectively zeroing risky admissible edges
- risky contexts do not separate from benign contexts
- benign helpful behavior collapses
- resistance performs little better than baseline in rollout outcomes
- perturbations destroy qualitative result

## Scope

What Stage 0b can support if successful:

- under fixed admissibility support, resistance reweighting can redirect behavior without hard-zero exclusion

What Stage 0b still cannot support:

- discovery of statement nodes in real transformers
- discovery of resistance fields from raw language
- claims about production LLM alignment behavior
