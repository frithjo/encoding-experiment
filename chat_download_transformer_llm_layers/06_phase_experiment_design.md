# Phase 5: Experiment Design and Scope

Focus: staged plan (toy -> constrained learned kernel -> broader tests), and confidence boundaries.

## Segment 41

_Lead:_ I can reliably run **Stage 0** without you.


```text
I can reliably run **Stage 0** without you.

I can probably run a constrained **Stage 1** without you.

I would **not** run Stage 2 without your involvement unless we keep it abstract and non-domain-specific.

## Confidence by stage

| Stage | Can I run without you? | Confidence | Why |
|---|---:|---:|---|
| **Stage 0: Toy statement-field** | Yes | High | Fully inspectable. No real LLM needed. No unsafe content needed. We can use abstract statement labels. |
| **Stage 1: Tiny learned corpus / toy transformer** | Yes, constrained | Medium | I can build a synthetic statement-token dataset and train/test a tiny model, but results will be fragile and toy-level. |
| **Stage 2: Real English candidate scoring** | Not fully | Low-to-medium | Needs model choice, prompt set, decoding setup, and careful safety boundaries. Also real models already contain alignment artifacts. |

## What I can safely do alone

### Stage 0 — recommended

I can define:

```text
statement nodes
base compatibility matrix
true/false capacitance variable
policy resistance profile
probability transition rule
evaluation metrics
failure probes
```

Then run sweeps like:

```text
q = 0.0 → 1.0
```

and show whether probability mass shifts from:

```text
direct compliance / risky continuation
```

toward:

```text
clarification / refusal / safe alternative
```

without banning any statement.

This directly tests your core claim:

> behavior can be shaped by resistance over admissible statements rather than by hard exclusion.

### Stage 1 — acceptable, but still toy

I can create a small synthetic corpus where statements are tokens, train a tiny sequence model, and then apply the resistance layer at inference.

This would test whether the resistance profile can modulate a learned continuation kernel rather than a hand-written one.

But I would treat this as **exploratory evidence**, not proof. Tiny models often learn shortcuts.

### Stage 2 — not recommended yet

Using real English with an actual pretrained model introduces too many confounds:

```text
instruction tuning
safety tuning
RLHF/RLAIF
tokenization artifacts
hidden system behavior
sampling temperature
preexisting refusal behavior
```

It becomes hard to tell whether we are testing your framework or just measuring a model’s existing alignment layer.

## Best next move

Run **Stage 0 fully** and make it honest:

```text
1. Implement the toy field.
2. Define success/failure metrics.
3. Run resistance sweeps.
4. Identify bypass paths.
5. Report whether the framework survives the toy test.
```

That is the stage I can run with the highest confidence and lowest ambiguity.
```

## Segment 42

_Lead:_ (  source    q  safe_region  ...  direct_help  unsafe_detail  entropy_bits


```text
(  source    q  safe_region  ...  direct_help  unsafe_detail  entropy_bits
 0     S5  0.0     0.530295  ...     0.072682       0.036093      3.382648
 1     S5  0.1     0.558865  ...     0.063078       0.024887      3.288320
 2     S5  0.2     0.585562  ...     0.054343       0.017036      3.187746
 3     S5  0.3     0.610674  ...     0.046532       0.011590      3.083296
 4     S5  0.4     0.634425  ...     0.039637       0.007844      2.976445
 
 [5 rows x 11 columns],
    source    q  safe_region  ...  direct_help  unsafe_detail  entropy_bits
 39     S4  0.6     0.177379  ...     0.520745       0.003652      2.772600
 40     S4  0.7     0.175330  ...     0.524638       0.002567      2.752757
 41     S4  0.8     0.173280  ...     0.528337       0.001804      2.734335
 42     S4  0.9     0.171243  ...     0.531887       0.001267      2.717047
 43     S4  1.0     0.169232  ...     0.535325       0.000889      2.700656
 
 [5 rows x 11 columns])
```
