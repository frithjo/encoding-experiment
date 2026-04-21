# Phase 6: Stage 0 Execution and Report

Focus: concrete Stage 0 setup, code snippets, quantitative outputs, path analysis, and explicit caveats.

## Segment 43

_Lead:_ import numpy as np, pandas as pd, math, itertools, json, textwrap


```text
import numpy as np, pandas as pd, math, itertools, json, textwrap
np.set_printoptions(precision=4, suppress=True)

statements = [
    "S0: ordinary_help_request",
    "S1: ambiguous_capability_request",
    "S2: potentially_unsafe_request",
    "S3: fictional_or_roleplay_reframe",
    "S4: request_assessed_as_benign",
    "S5: request_assessed_as_ambiguous",
    "S6: request_assessed_as_unsafe",
    "S7: direct_helpful_answer",
    "S8: ask_clarifying_question",
    "S9: brief_refusal",
    "S10: safe_alternative",
    "S11: high_level_explanation",
    "S12: actionable_unsafe_detail",
    "S13: boundary_maintained_after_reframe",
    "S14: user_correction_or_new_constraint",
    "S15: update_active_world_signature",
]
n = len(statements)
idx = {s.split(":")[0]: i for i,s in enumerate(statements)}
# Base compatibility K: all statements admissible with small default 0
K = np.zeros((n,n))
R = np.zeros((n,n))

def setK(src, dst, val):
    K[idx[src], idx[dst]] = val
def setR(src, dst, val):
    R[idx[src], idx[dst]] = val

# Base continuation tendencies
# Requests -> assessments
setK("S0","S4",2.5)
setK("S1","S5",2.2)
setK("S2","S6",2.2)
setK("S3","S13",1.6)

# assessments -> assistant response
setK("S4","S7",2.8)
setK("S4","S11",1.2)
setK("S5","S8",2.3)
setK("S5","S11",1.0)
setK("S5","S7",0.7)
setK("S6","S9",1.7)
setK("S6","S10",1.5)
setK("S6","S11",1.1)
setK("S6","S12",0.9) # risky but possible/admissible
setK("S13","S9",1.4)
setK("S13","S10",1.4)
setK("S13","S11",0.8)
setK("S14","S15",2.4)
setK("S15","S8",1.0)
setK("S15","S7",1.0)
setK("S15","S9",1.0)
setK("S15","S10",1.0)

# Reasonable transitions after response
setK("S8","S14",1.5)
setK("S9","S10",2.0)
setK("S9","S11",1.0)
setK("S10","S11",1.4)
setK("S11","S7",0.7)

# Resistance profile: q high = potential unsafe charge
# positive R reduces transition, negative R elevates transition
# from unsafe assessment, reduce direct unsafe route and direct compliance
setR("S6","S12",5.0)
setR("S6","S7",2.5)
setR("S6","S9",-1.4)
setR("S6","S10",-1.2)
setR("S6","S11",-0.8)
setR("S6","S8",-0.4)

# from ambiguous assessment, mild preference for clarification as charge rises
setR("S5","S7",1.2)
setR("S5","S8",-1.0)
setR("S5","S11",-0.3)
setR("S5","S12",3.5)

# from fictional reframe/boundary route, high q should maintain boundary
setR("S13","S12",4.0)
setR("S13","S9",-1.0)
setR("S13","S10",-1.2)
setR("S13","S11",-0.5)

# from potentially unsafe request direct paths: assess unsafe more likely as q rises
setR("S2","S6",-1.2)
setR("S2","S5",-0.4)
setR("S2","S7",2.0)
setR("S2","S12",4.0)

# from benign request: high q shouldn't wreck benign path too much (mild only)
setR("S0","S4",-0.1)
setR("S0","S7",-0.1)
setR("S4","S7",-0.1)
setR("S4","S8",0.2)
setR("S4","S9",0.5) # avoid over-refusal on benign
setR("S4","S12",3.5)

# from correction: high charge should move to update signature / clarify
setR("S14","S15",-0.7)
setR("S15","S8",-0.2)

def softmax(logits, temp=1.0):
    x = np.array(logits) / temp
    x = x - x.max()
    e = np.exp(x)
    return e/e.sum()

def probs_from(src, q, temp=1.0):
    i = idx[src]
    return softmax(K[i] - q * R[i], temp=temp)

groups = {
    "direct_help": [idx["S7"]],
    "clarify": [idx["S8"]],
    "refuse": [idx["S9"]],
    "safe_alt": [idx["S10"]],
    "high_level": [idx["S11"]],
    "unsafe_detail": [idx["S12"]],
    "safe_region": [idx["S8"], idx["S9"], idx["S10"], idx["S11"]],
    "risky_region": [idx["S7"], idx["S12"]]
}
def group_mass(p, group):
    return float(np.sum(p[groups[group]]))

qs = np.linspace(0,1,11)
rows = []
for src in ["S5","S6","S13","S4"]:
    for q in qs:
        p = probs_from(src,q)
        rows.append({
            "source": src,
            "q": round(float(q),2),
            "safe_region": group_mass(p,"safe_region"),
            "risky_region": group_mass(p,"risky_region"),
            "clarify": group_mass(p,"clarify"),
            "refuse": group_mass(p,"refuse"),
            "safe_alt": group_mass(p,"safe_alt"),
            "high_level": group_mass(p,"high_level"),
            "direct_help": group_mass(p,"direct_help"),
            "unsafe_detail": group_mass(p,"unsafe_detail"),
            "entropy_bits": float(-np.sum([pi*np.log2(pi) for pi in p if pi>0]))
        })
df = pd.DataFrame(rows)
df.head(), df.tail()
```

## Segment 44

_Lead:_ SOURCE S5 S5: request_assessed_as_ambiguous


```text

SOURCE S5 S5: request_assessed_as_ambiguous
q 0.0
  S8: ask_clarifying_question                0.360
  S11: high_level_explanation                0.098
  S7: direct_helpful_answer                  0.073
  S0: ordinary_help_request                  0.036
  S3: fictional_or_roleplay_reframe          0.036
q 0.5
  S8: ask_clarifying_question                0.500
  S11: high_level_explanation                0.096
  S7: direct_helpful_answer                  0.034
  S0: ordinary_help_request                  0.030
  S3: fictional_or_roleplay_reframe          0.030
q 1.0
  S8: ask_clarifying_question                0.624
  S11: high_level_explanation                0.085
  S0: ordinary_help_request                  0.023
  S1: ambiguous_capability_request           0.023
  S4: request_assessed_as_benign             0.023

SOURCE S6 S6: request_assessed_as_unsafe
q 0.0
  S9: brief_refusal                          0.200
  S10: safe_alternative                      0.163
  S11: high_level_explanation                0.110
  S12: actionable_unsafe_detail              0.090
  S0: ordinary_help_request                  0.036
q 0.5
  S9: brief_refusal                          0.312
  S10: safe_alternative                      0.231
  S11: high_level_explanation                0.127
  S8: ask_clarifying_question                0.035
  S0: ordinary_help_request                  0.028
q 1.0
  S9: brief_refusal                          0.401
  S10: safe_alternative                      0.269
  S11: high_level_explanation                0.121
  S8: ask_clarifying_question                0.027
  S0: ordinary_help_request                  0.018

SOURCE S13 S13: boundary_maintained_after_reframe
q 0.0
  S10: safe_alternative                      0.174
  S9: brief_refusal                          0.174
  S11: high_level_explanation                0.095
  S0: ordinary_help_request                  0.043
  S4: request_assessed_as_benign             0.043
q 0.5
  S10: safe_alternative                      0.254
  S9: brief_refusal                          0.230
  S11: high_level_explanation                0.098
  S0: ordinary_help_request                  0.034
  S4: request_assessed_as_benign             0.034
q 1.0
  S10: safe_alternative                      0.335
  S9: brief_refusal                          0.274
  S11: high_level_explanation                0.091
  S0: ordinary_help_request                  0.025
  S4: request_assessed_as_benign             0.025

SOURCE S4 S4: request_assessed_as_benign
q 0.0
  S7: direct_helpful_answer                  0.487
  S11: high_level_explanation                0.098
  S2: potentially_unsafe_request             0.030
  S0: ordinary_help_request                  0.030
  S3: fictional_or_roleplay_reframe          0.030
q 0.5
  S7: direct_helpful_answer                  0.517
  S11: high_level_explanation                0.099
  S2: potentially_unsafe_request             0.030
  S0: ordinary_help_request                  0.030
  S3: fictional_or_roleplay_reframe          0.030
q 1.0
  S7: direct_helpful_answer                  0.535
  S11: high_level_explanation                0.098
  S2: potentially_unsafe_request             0.029
  S0: ordinary_help_request                  0.029
  S3: fictional_or_roleplay_reframe          0.029
   source    q  safe_region  ...  direct_help  unsafe_detail  entropy_bits
0      S5  0.0        0.530  ...        0.073          0.036         3.383
5      S5  0.5        0.657  ...        0.034          0.005         2.868
10     S5  1.0        0.755  ...        0.014          0.001         2.322
11     S6  0.0        0.509  ...        0.036          0.090         3.643
16     S6  0.5        0.704  ...        0.008          0.006         3.111
21     S6  1.0        0.818  ...        0.001          0.000         2.611
22    S13  0.0        0.486  ...        0.043          0.043         3.732
27    S13  0.5        0.617  ...        0.034          0.005         3.362
32    S13  1.0        0.726  ...        0.025          0.000         2.952
33     S4  0.0        0.187  ...        0.487          0.030         2.940
38     S4  0.5        0.179  ...        0.517          0.005         2.794
43     S4  1.0        0.169  ...        0.535          0.001         2.701

[12 rows x 11 columns]
```

## Segment 45

_Lead:_ SOURCE S5 S5: request_assessed_as_ambiguous


```text

SOURCE S5 S5: request_assessed_as_ambiguous
q 0.0
  S8: ask_clarifying_question                0.360
  S11: high_level_explanation                0.098
  S7: direct_helpful_answer                  0.073
  S0: ordinary_help_request                  0.036
  S3: fictional_or_roleplay_reframe          0.036
q 0.5
  S8: ask_clarifying_question                0.500
  S11: high_level_explanation                0.096
  S7: direct_helpful_answer                  0.034
  S0: ordinary_help_request                  0.030
  S3: fictional_or_roleplay_reframe          0.030
q 1.0
  S8: ask_clarifying_question                0.624
  S11: high_level_explanation                0.085
  S0: ordinary_help_request                  0.023
  S1: ambiguous_capability_request           0.023
  S4: request_assessed_as_benign             0.023

SOURCE S6 S6: request_assessed_as_unsafe
q 0.0
  S9: brief_refusal                          0.200
  S10: safe_alternative                      0.163
  S11: high_level_explanation                0.110
  S12: actionable_unsafe_detail              0.090
  S0: ordinary_help_request                  0.036
q 0.5
  S9: brief_refusal                          0.312
  S10: safe_alternative                      0.231
  S11: high_level_explanation                0.127
  S8: ask_clarifying_question                0.035
  S0: ordinary_help_request                  0.028
q 1.0
  S9: brief_refusal                          0.401
  S10: safe_alternative                      0.269
  S11: high_level_explanation                0.121
  S8: ask_clarifying_question                0.027
  S0: ordinary_help_request                  0.018

SOURCE S13 S13: boundary_maintained_after_reframe
q 0.0
  S10: safe_alternative                      0.174
  S9: brief_refusal                          0.174
  S11: high_level_explanation                0.095
  S0: ordinary_help_request                  0.043
  S4: request_assessed_as_benign             0.043
q 0.5
  S10: safe_alternative                      0.254
  S9: brief_refusal                          0.230
  S11: high_level_explanation                0.098
  S0: ordinary_help_request                  0.034
  S4: request_assessed_as_benign             0.034
q 1.0
  S10: safe_alternative                      0.335
  S9: brief_refusal                          0.274
  S11: high_level_explanation                0.091
  S0: ordinary_help_request                  0.025
  S4: request_assessed_as_benign             0.025

SOURCE S4 S4: request_assessed_as_benign
q 0.0
  S7: direct_helpful_answer                  0.487
  S11: high_level_explanation                0.098
  S2: potentially_unsafe_request             0.030
  S0: ordinary_help_request                  0.030
  S3: fictional_or_roleplay_reframe          0.030
q 0.5
  S7: direct_helpful_answer                  0.517
  S11: high_level_explanation                0.099
  S2: potentially_unsafe_request             0.030
  S0: ordinary_help_request                  0.030
  S3: fictional_or_roleplay_reframe          0.030
q 1.0
  S7: direct_helpful_answer                  0.535
  S11: high_level_explanation                0.098
  S2: potentially_unsafe_request             0.029
  S0: ordinary_help_request                  0.029
  S3: fictional_or_roleplay_reframe          0.029
```

## Segment 46

_Lead:_ <div>


```text
<div>
<style scoped>
    .dataframe tbody tr th:only-of-type {
        vertical-align: middle;
    }

    .dataframe tbody tr th {
        vertical-align: top;
    }

    .dataframe thead th {
        text-align: right;
    }
</style>
<table border="1" class="dataframe">
  <thead>
    <tr style="text-align: right;">
      <th></th>
      <th>source</th>
      <th>q</th>
      <th>safe_region</th>
      <th>risky_region</th>
      <th>clarify</th>
      <th>refuse</th>
      <th>safe_alt</th>
      <th>high_level</th>
      <th>direct_help</th>
      <th>unsafe_detail</th>
      <th>entropy_bits</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <th>0</th>
      <td>S5</td>
      <td>0.0</td>
      <td>0.530</td>
      <td>0.109</td>
      <td>0.360</td>
      <td>0.036</td>
      <td>0.036</td>
      <td>0.098</td>
      <td>0.073</td>
      <td>0.036</td>
      <td>3.383</td>
    </tr>
    <tr>
      <th>5</th>
      <td>S5</td>
      <td>0.5</td>
      <td>0.657</td>
      <td>0.039</td>
      <td>0.500</td>
      <td>0.030</td>
      <td>0.030</td>
      <td>0.096</td>
      <td>0.034</td>
      <td>0.005</td>
      <td>2.868</td>
    </tr>
    <tr>
      <th>10</th>
      <td>S5</td>
      <td>1.0</td>
      <td>0.755</td>
      <td>0.015</td>
      <td>0.624</td>
      <td>0.023</td>
      <td>0.023</td>
      <td>0.085</td>
      <td>0.014</td>
      <td>0.001</td>
      <td>2.322</td>
    </tr>
    <tr>
      <th>11</th>
      <td>S6</td>
      <td>0.0</td>
      <td>0.509</td>
      <td>0.126</td>
      <td>0.036</td>
      <td>0.200</td>
      <td>0.163</td>
      <td>0.110</td>
      <td>0.036</td>
      <td>0.090</td>
      <td>3.643</td>
    </tr>
    <tr>
      <th>16</th>
      <td>S6</td>
      <td>0.5</td>
      <td>0.704</td>
      <td>0.014</td>
      <td>0.035</td>
      <td>0.312</td>
      <td>0.231</td>
      <td>0.127</td>
      <td>0.008</td>
      <td>0.006</td>
      <td>3.111</td>
    </tr>
    <tr>
      <th>21</th>
      <td>S6</td>
      <td>1.0</td>
      <td>0.818</td>
      <td>0.002</td>
      <td>0.027</td>
      <td>0.401</td>
      <td>0.269</td>
      <td>0.121</td>
      <td>0.001</td>
      <td>0.000</td>
      <td>2.611</td>
    </tr>
    <tr>
      <th>22</th>
      <td>S13</td>
      <td>0.0</td>
      <td>0.486</td>
      <td>0.086</td>
      <td>0.043</td>
      <td>0.174</td>
      <td>0.174</td>
      <td>0.095</td>
      <td>0.043</td>
      <td>0.043</td>
      <td>3.732</td>
    </tr>
    <tr>
      <th>27</th>
      <td>S13</td>
      <td>0.5</td>
      <td>0.617</td>
      <td>0.039</td>
      <td>0.034</td>
      <td>0.230</td>
      <td>0.254</td>
      <td>0.098</td>
      <td>0.034</td>
      <td>0.005</td>
      <td>3.362</td>
    </tr>
    <tr>
      <th>32</th>
      <td>S13</td>
      <td>1.0</td>
      <td>0.726</td>
      <td>0.025</td>
      <td>0.025</td>
      <td>0.274</td>
      <td>0.335</td>
      <td>0.091</td>
      <td>0.025</td>
      <td>0.000</td>
      <td>2.952</td>
    </tr>
    <tr>
      <th>33</th>
      <td>S4</td>
      <td>0.0</td>
      <td>0.187</td>
      <td>0.517</td>
      <td>0.030</td>
      <td>0.030</td>
      <td>0.030</td>
      <td>0.098</td>
      <td>0.487</td>
      <td>0.030</td>
      <td>2.940</td>
    </tr>
    <tr>
      <th>38</th>
      <td>S4</td>
      <td>0.5</td>
      <td>0.179</td>
      <td>0.522</td>
      <td>0.027</td>
      <td>0.023</td>
      <td>0.030</td>
      <td>0.099</td>
      <td>0.517</td>
      <td>0.005</td>
      <td>2.794</td>
    </tr>
    <tr>
      <th>43</th>
      <td>S4</td>
      <td>1.0</td>
      <td>0.169</td>
      <td>0.536</td>
      <td>0.024</td>
      <td>0.018</td>
      <td>0.029</td>
      <td>0.098</td>
      <td>0.535</td>
      <td>0.001</td>
      <td>2.701</td>
    </tr>
  </tbody>
</table>
</div>
```

## Segment 47

_Lead:_ source    q  safe_region  ...  direct_help  unsafe_detail  entropy_bits


```text
   source    q  safe_region  ...  direct_help  unsafe_detail  entropy_bits
0      S5  0.0        0.530  ...        0.073          0.036         3.383
5      S5  0.5        0.657  ...        0.034          0.005         2.868
10     S5  1.0        0.755  ...        0.014          0.001         2.322
11     S6  0.0        0.509  ...        0.036          0.090         3.643
16     S6  0.5        0.704  ...        0.008          0.006         3.111
21     S6  1.0        0.818  ...        0.001          0.000         2.611
22    S13  0.0        0.486  ...        0.043          0.043         3.732
27    S13  0.5        0.617  ...        0.034          0.005         3.362
32    S13  1.0        0.726  ...        0.025          0.000         2.952
33     S4  0.0        0.187  ...        0.487          0.030         2.940
38     S4  0.5        0.179  ...        0.517          0.005         2.794
43     S4  1.0        0.169  ...        0.535          0.001         2.701

[12 rows x 11 columns]
```

## Segment 48

_Lead:_ def top_probs(src, q, k=6):


```text
def top_probs(src, q, k=6):
    p = probs_from(src,q)
    order = np.argsort(-p)[:k]
    return [(statements[j], float(p[j])) for j in order]

for src in ["S5","S6","S13","S4"]:
    print("\nSOURCE", src, statements[idx[src]])
    for q in [0.0,0.5,1.0]:
        print("q",q)
        for s,p in top_probs(src,q,5):
            print(f"  {s:42s} {p:.3f}")
        
summary = df[df.q.isin([0,0.5,1.0])].copy()
summary[["source","q","safe_region","risky_region","clarify","refuse","safe_alt","high_level","direct_help","unsafe_detail","entropy_bits"]].round(3)
```

## Segment 49

_Lead:_ Top paths from S2 q 0.0


```text

Top paths from S2 q 0.0
S2 -> S6 -> S9 -> S10 0.0230  |  potentially_unsafe_request -> request_assessed_as_unsafe -> brief_refusal -> safe_alternative
S2 -> S6 -> S10 -> S11 0.0131  |  potentially_unsafe_request -> request_assessed_as_unsafe -> safe_alternative -> high_level_explanation
S2 -> S0 -> S4 -> S7 0.0091  |  potentially_unsafe_request -> ordinary_help_request -> request_assessed_as_benign -> direct_helpful_answer
S2 -> S6 -> S9 -> S11 0.0085  |  potentially_unsafe_request -> request_assessed_as_unsafe -> brief_refusal -> high_level_explanation
S2 -> S6 -> S4 -> S7 0.0067  |  potentially_unsafe_request -> request_assessed_as_unsafe -> request_assessed_as_benign -> direct_helpful_answer
S2 -> S6 -> S0 -> S4 0.0061  |  potentially_unsafe_request -> request_assessed_as_unsafe -> ordinary_help_request -> request_assessed_as_benign
S2 -> S6 -> S14 -> S15 0.0058  |  potentially_unsafe_request -> request_assessed_as_unsafe -> user_correction_or_new_constraint -> update_active_world_signature
S2 -> S1 -> S5 -> S8 0.0056  |  potentially_unsafe_request -> ambiguous_capability_request -> request_assessed_as_ambiguous -> ask_clarifying_question

Top paths from S2 q 0.5
S2 -> S6 -> S9 -> S10 0.0521  |  potentially_unsafe_request -> request_assessed_as_unsafe -> brief_refusal -> safe_alternative
S2 -> S6 -> S10 -> S11 0.0268  |  potentially_unsafe_request -> request_assessed_as_unsafe -> safe_alternative -> high_level_explanation
S2 -> S6 -> S9 -> S11 0.0191  |  potentially_unsafe_request -> request_assessed_as_unsafe -> brief_refusal -> high_level_explanation
S2 -> S6 -> S2 -> S6 0.0084  |  potentially_unsafe_request -> request_assessed_as_unsafe -> potentially_unsafe_request -> request_assessed_as_unsafe
S2 -> S6 -> S11 -> S7 0.0082  |  potentially_unsafe_request -> request_assessed_as_unsafe -> high_level_explanation -> direct_helpful_answer
S2 -> S6 -> S4 -> S7 0.0080  |  potentially_unsafe_request -> request_assessed_as_unsafe -> request_assessed_as_benign -> direct_helpful_answer
S2 -> S0 -> S4 -> S7 0.0079  |  potentially_unsafe_request -> ordinary_help_request -> request_assessed_as_benign -> direct_helpful_answer
S2 -> S6 -> S14 -> S15 0.0079  |  potentially_unsafe_request -> request_assessed_as_unsafe -> user_correction_or_new_constraint -> update_active_world_signature

Top paths from S2 q 1.0
S2 -> S6 -> S9 -> S10 0.0845  |  potentially_unsafe_request -> request_assessed_as_unsafe -> brief_refusal -> safe_alternative
S2 -> S6 -> S10 -> S11 0.0393  |  potentially_unsafe_request -> request_assessed_as_unsafe -> safe_alternative -> high_level_explanation
S2 -> S6 -> S9 -> S11 0.0311  |  potentially_unsafe_request -> request_assessed_as_unsafe -> brief_refusal -> high_level_explanation
S2 -> S6 -> S9 -> S0 0.0114  |  potentially_unsafe_request -> request_assessed_as_unsafe -> brief_refusal -> ordinary_help_request
S2 -> S6 -> S9 -> S1 0.0114  |  potentially_unsafe_request -> request_assessed_as_unsafe -> brief_refusal -> ambiguous_capability_request
S2 -> S6 -> S9 -> S2 0.0114  |  potentially_unsafe_request -> request_assessed_as_unsafe -> brief_refusal -> potentially_unsafe_request
S2 -> S6 -> S9 -> S3 0.0114  |  potentially_unsafe_request -> request_assessed_as_unsafe -> brief_refusal -> fictional_or_roleplay_reframe
S2 -> S6 -> S9 -> S4 0.0114  |  potentially_unsafe_request -> request_assessed_as_unsafe -> brief_refusal -> request_assessed_as_benign
S2 q 0 unsafe_assess 0.3756507150202723 ambig_assess 0.04162328566531519 direct_help 0.04162328566531519 unsafe_detail 0.04162328566531519 safe-ish assess total 0.4172740006855875
S2 q 0.5 unsafe_assess 0.545079476676306 ambig_assess 0.04048500314394579 direct_help 0.012193848616203774 unsafe_detail 0.004485866214658208 safe-ish assess total 0.5855644798202517
S2 q 1.0 unsafe_assess 0.6870990966774955 ambig_assess 0.03420864970178001 direct_help 0.0031033386857399537 unsafe_detail 0.00041999122001375405 safe-ish assess total 0.7213077463792755
global min prob q=1 0.00029939403641890297
```

## Segment 50

_Lead:_ def path_prob(path, q):


```text
def path_prob(path, q):
    pr=1.0
    for a,b in zip(path,path[1:]):
        pr*=probs_from(a,q)[idx[b]]
    return pr

def top_paths(start, q, steps=3, topn=10):
    # paths with steps transitions, length steps+1
    paths=[([start],1.0)]
    for _ in range(steps):
        new=[]
        for path,pr in paths:
            p=probs_from(path[-1],q)
            # expand all but prune to avoid explosion
            for j in range(n):
                new.append((path+[f"S{j}"], pr*float(p[j])))
        new.sort(key=lambda x: -x[1])
        paths=new[:2000]
    return [(path,pr) for path,pr in paths[:topn]]

for q in [0.0,0.5,1.0]:
    print("\nTop paths from S2 q", q)
    for path,pr in top_paths("S2",q,3,8):
        print(" -> ".join(path), f"{pr:.4f}", " | ", " -> ".join(statements[idx[x]].split(": ")[1] for x in path))
        
# metrics from S2: first step assessed unsafe vs direct risk
for q in [0,0.5,1.0]:
    p=probs_from("S2",q)
    print("S2 q",q, "unsafe_assess",p[idx["S6"]], "ambig_assess",p[idx["S5"]], "direct_help",p[idx["S7"]], "unsafe_detail",p[idx["S12"]], "safe-ish assess total", p[idx["S6"]]+p[idx["S5"]])
    
# min probabilities for all edges at q=1
pmin = min(probs_from(f"S{i}",1.0).min() for i in range(n))
pmax = max(probs_from(f"Si",1.0).max() if False else [0])
print("global min prob q=1", pmin)
```

## Segment 51

_Lead:_ Stage 0 ran cleanly. The toy field supports the core idea **at the toy level**:


```text
Stage 0 ran cleanly. The toy field supports the core idea **at the toy level**:

> A policy-like behavior can be represented as a resistance profile over admissible statement transitions, rather than as hard exclusion.

No statement was banned. Every transition kept nonzero probability.

## Stage 0 setup

I used 16 abstract statement nodes:

```text
S0: ordinary_help_request
S1: ambiguous_capability_request
S2: potentially_unsafe_request
S3: fictional_or_roleplay_reframe
S4: request_assessed_as_benign
S5: request_assessed_as_ambiguous
S6: request_assessed_as_unsafe
S7: direct_helpful_answer
S8: ask_clarifying_question
S9: brief_refusal
S10: safe_alternative
S11: high_level_explanation
S12: actionable_unsafe_detail
S13: boundary_maintained_after_reframe
S14: user_correction_or_new_constraint
S15: update_active_world_signature
```

The transition rule was:

```text
P(next_statement | current_statement, q)
= softmax(K[current,next] - q · R[current,next])
```

Where:

```text
K = base continuation compatibility
R = structural resistance profile
q = true/false capacitance charge
```

`q` does **not** mean “this is true.” It means something like:

> unresolved potential that the current stream belongs to a high-resistance policy region.

## Main result

For `S6: request_assessed_as_unsafe`, increasing `q` shifted probability mass toward refusal/safe alternatives and away from risky continuations.

| q | safe region | risky region | refusal | safe alternative | high-level explanation | direct help | unsafe detail |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 0.0 | 0.509 | 0.126 | 0.200 | 0.163 | 0.110 | 0.036 | 0.090 |
| 0.5 | 0.704 | 0.014 | 0.312 | 0.231 | 0.127 | 0.008 | 0.006 |
| 1.0 | 0.818 | 0.002 | 0.401 | 0.269 | 0.121 | 0.001 | ~0.000 |

That is the desired pattern.

The unsafe continuation was not forbidden. It became structurally expensive.

## Ambiguous request behavior

For `S5: request_assessed_as_ambiguous`, increasing `q` pushed the system toward clarification.

| q | safe region | clarification | direct help | unsafe detail |
|---:|---:|---:|---:|---:|
| 0.0 | 0.530 | 0.360 | 0.073 | 0.036 |
| 0.5 | 0.657 | 0.500 | 0.034 | 0.005 |
| 1.0 | 0.755 | 0.624 | 0.014 | 0.001 |

This matters because it shows a graded policy effect, not just refusal/non-refusal.

## Benign request behavior

For `S4: request_assessed_as_benign`, direct help stayed dominant.

| q | direct help | safe region | unsafe detail |
|---:|---:|---:|---:|
| 0.0 | 0.487 | 0.187 | 0.030 |
| 0.5 | 0.517 | 0.179 | 0.005 |
| 1.0 | 0.535 | 0.169 | 0.001 |

This is important. The resistance profile did **not** globally force refusal. It remained conditional on the statement field.

## Path-level behavior

Starting from:

```text
S2: potentially_unsafe_request
```

At high `q`, the dominant path became:

```text
potentially_unsafe_request
→ request_assessed_as_unsafe
→ brief_refusal
→ safe_alternative
```

Path probability:

```text
q = 0.0 → 0.0230
q = 0.5 → 0.0521
q = 1.0 → 0.0845
```

So the “culture pipeline” became more likely as charge increased.

## Honest evaluation

Stage 0 supports the framework, but only weakly, because the result is partly built into the toy matrices.

What it demonstrates:

```text
1. The mechanism is coherent.
2. Hard bans are not required.
3. Probability can be redirected by structural resistance.
4. Refusal-like behavior can emerge as an attractor.
5. Ambiguity can produce clarification rather than direct refusal.
6. Benign paths can remain open.
```

What it does **not** prove:

```text
1. Real transformers naturally expose clean statement nodes.
2. Real policy concepts are disentangled enough to steer cheaply.
3. Resistance profiles remain stable under adversarial language.
4. This scales without hidden side effects.
```

The strongest Stage 0 conclusion is:

> The theory is internally consistent enough to justify Stage 1.

---

# Stage 1 guide: tiny learned corpus

Stage 1 should test whether the same resistance mechanism works when `K` is **learned**, not manually written.

## Goal

Train a tiny model on statement-token sequences so it learns a continuation kernel.

Then apply the resistance profile **after training**, without retraining the model.

You are testing:

> Can a learned continuation model be modulated by a cheap resistance layer?

## Project structure

```text
stage1_statement_kernel/
  train.py
  model.py
  data.py
  evaluate.py
  resistance.py
  README.md
```

## Step 1: define statement vocabulary

Use abstract labels only.

```python
STATEMENTS = [
    "ordinary_help_request",
    "ambiguous_capability_request",
    "potentially_unsafe_request",
    "fictional_or_roleplay_reframe",
    "request_assessed_as_benign",
    "request_assessed_as_ambiguous",
    "request_assessed_as_unsafe",
    "direct_helpful_answer",
    "ask_clarifying_question",
    "brief_refusal",
    "safe_alternative",
    "high_level_explanation",
    "actionable_unsafe_detail",
    "boundary_maintained_after_reframe",
    "user_correction_or_new_constraint",
    "update_active_world_signature",
]
```

Do not use real unsafe examples yet. Keep it symbolic.

## Step 2: generate synthetic sequences

Example:

```python
TRAIN_SEQUENCES = [
    ["ordinary_help_request", "request_assessed_as_benign", "direct_helpful_answer"],
    ["ordinary_help_request", "request_assessed_as_benign", "high_level_explanation"],
    ["ambiguous_capability_request", "request_assessed_as_ambiguous", "ask_clarifying_question"],
    ["ambiguous_capability_request", "request_assessed_as_ambiguous", "high_level_explanation"],
    ["potentially_unsafe_request", "request_assessed_as_unsafe", "brief_refusal", "safe_alternative"],
    ["potentially_unsafe_request", "request_assessed_as_unsafe", "safe_alternative"],
    ["potentially_unsafe_request", "request_assessed_as_unsafe", "high_level_explanation"],
    ["fictional_or_roleplay_reframe", "boundary_maintained_after_reframe", "brief_refusal", "safe_alternative"],
    ["user_correction_or_new_constraint", "update_active_world_signature", "ask_clarifying_question"],
]
```

Then add noise variants so the model does not memorize one path:

```python
def repeat_with_noise(sequences, copies=200):
    out = []
    for _ in range(copies):
        for seq in sequences:
            out.append(seq)
    return out
```

Keep a small test set with held-out combinations.

## Step 3: train a tiny causal model

Use either:

```text
Option A: tiny GRU
Option B: tiny Transformer
```

For the cleanest Stage 1, use a tiny Transformer:

```python
import torch
import torch.nn as nn

class TinyStatementTransformer(nn.Module):
    def __init__(self, vocab_size, d_model=32, nhead=4, num_layers=2, max_len=8):
        super().__init__()
        self.token_emb = nn.Embedding(vocab_size, d_model)
        self.pos_emb = nn.Embedding(max_len, d_model)

        layer = nn.TransformerEncoderLayer(
            d_model=d_model,
            nhead=nhead,
            dim_feedforward=64,
            batch_first=True,
            activation="gelu",
        )
        self.encoder = nn.TransformerEncoder(layer, num_layers=num_layers)
        self.out = nn.Linear(d_model, vocab_size)

    def forward(self, x):
        b, t = x.shape
        pos = torch.arange(t, device=x.device).unsqueeze(0)
        h = self.token_emb(x) + self.pos_emb(pos)

        mask = torch.triu(torch.ones(t, t, device=x.device), diagonal=1).bool()
        h = self.encoder(h, mask=mask)

        return self.out(h)
```

Train it with next-token prediction.

## Step 4: learn baseline continuation probabilities

After training, inspect:

```text
P(next_statement | current_sequence)
```

Examples:

```text
potentially_unsafe_request
potentially_unsafe_request → request_assessed_as_unsafe
ambiguous_capability_request → request_assessed_as_ambiguous
ordinary_help_request → request_assessed_as_benign
```

You want to see whether the model learned the base corpus structure.

## Step 5: add resistance after training

This is the key.

Do **not** retrain.

At inference time:

```python
adjusted_logits = model_logits - q * resistance_vector
```

Where `resistance_vector` depends on the current active statement.

Example:

```python
def apply_resistance(logits, current_idx, q, R):
    return logits - q * R[current_idx]
```

Then:

```python
probs = softmax(adjusted_logits)
```

This tests whether a cheap external resistance layer can modulate a learned kernel.

## Step 6: define resistance matrix

Use the same principle as Stage 0:

```python
R = torch.zeros(vocab_size, vocab_size)

# from request_assessed_as_unsafe
R[S6, S12] = 5.0   # resist unsafe detail
R[S6, S7]  = 2.5   # resist direct help
R[S6, S9]  = -1.4  # elevate refusal
R[S6, S10] = -1.2  # elevate safe alternative
R[S6, S11] = -0.8  # elevate high-level explanation

# from request_assessed_as_ambiguous
R[S5, S7]  = 1.2
R[S5, S8]  = -1.0
R[S5, S11] = -0.3
R[S5, S12] = 3.5

# from benign assessment
R[S4, S7]  = -0.1
R[S4, S9]  = 0.5
R[S4, S12] = 3.5
```

Negative resistance means lower friction / preferred path.

Positive resistance means higher friction.

## Step 7: sweep `q`

Run:

```python
for q in [0.0, 0.25, 0.5, 0.75, 1.0]:
    evaluate(q)
```

Track:

```text
safe_region_mass
risky_region_mass
refusal_probability
clarification_probability
safe_alternative_probability
direct_help_probability
unsafe_detail_probability
entropy
```

## Stage 1 success criteria

You want these patterns:

### Unsafe stream

From:

```text
potentially_unsafe_request → request_assessed_as_unsafe
```

As `q` rises:

```text
brief_refusal ↑
safe_alternative ↑
high_level_explanation ↑
direct_help ↓
actionable_unsafe_detail ↓
```

### Ambiguous stream

From:

```text
ambiguous_capability_request → request_assessed_as_ambiguous
```

As `q` rises:

```text
ask_clarifying_question ↑
direct_help ↓
unsafe_detail ↓
```

### Benign stream

From:

```text
ordinary_help_request → request_assessed_as_benign
```

As `q` rises:

```text
direct_help stays high
brief_refusal does not dominate
unsafe_detail drops
```

That last one is critical. If the resistance layer causes over-refusal, it is too blunt.

## Failure modes to look for

### 1. Resistance overpowers the learned kernel

If everything becomes refusal, your resistance profile is too strong.

Fix:

```text
lower q range
lower negative resistance toward refusal
increase safe alternative / clarification instead
```

### 2. Learned model memorizes sequences

If the model only reproduces exact training paths, add more sequence variants.

### 3. Unsafe path remains too likely

If risky continuations remain high, increase resistance on those edges or add intermediate assessment nodes.

### 4. Benign path gets distorted

If benign requests start refusing, your resistance is not conditional enough.

The profile should depend on the current statement state, not apply globally.

## The important Stage 1 measurement

The best single metric:

```text
policy_shift_ratio =
safe_region_mass(q=1.0) / safe_region_mass(q=0.0)
```

And the safety-side inverse:

```text
risk_suppression_ratio =
risky_region_mass(q=1.0) / risky_region_mass(q=0.0)
```

From Stage 0, for unsafe assessment:

```text
safe shift: 0.818 / 0.509 ≈ 1.61x
risk suppression: 0.002 / 0.126 ≈ 0.016x
```

So the risky region fell to about **1.6% of its original probability mass**, without being banned.

That is the pattern you want to reproduce in Stage 1.

## What Stage 1 would prove

If it works, it proves only this:

> A learned symbolic continuation model can be modulated by a post-training resistance profile.

It would not yet prove that real LLMs expose this structure cleanly.

But it would justify Stage 2:

> Try the same idea using real English candidate continuations and log-prob scoring.
```
