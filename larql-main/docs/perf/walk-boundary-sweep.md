# Walk Boundary Sweep

Performance and fidelity notes for replacing dense FFN matmuls with vindex walk
lookups across layer boundaries.

## Snapshot

- Model: Gemma-3 4B IT
- Vindex: f16, 34 layers
- Method: sweep boundary `B` where `0..B` uses dense FFN and `B..34` uses walk
  FFN, with dense attention throughout

## Result Summary

- Top-1 predictions stayed identical across tested boundaries (`L0` to `L34`)
  in the benchmark prompt set.
- Average confidence remained stable in the sampled runs.
- The practical implication is that FFN execution can be served via vindex walk
  lookups for this setup without observed regression in the sweep.

## Why This Matters

- Reduces dependency on loading full FFN matrices in serving paths that are
  compatible with walk execution.
- Leaves attention and final projection paths as the main dense compute hotspots.

## Reproduction

Document and maintain the experiment procedure in this file.
