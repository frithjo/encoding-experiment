# larql-tensor

## Crate Role

- Role: Tensor facade and sgemm utilities
- Zone: core
- Release impact: medium
- Stability target: managed

Tensor abstraction layer for LARQL that centralizes ndarray/matrixmultiply
interaction and BLAS-backed SGEMM integration.

## Scope

- Provide a consistent tensor facade for core/runtime crates.
- Own low-level SGEMM helpers and matrix interop points.
- Keep high-level model, inference, and API concerns out of this crate.

## Public vs Internal Surface

- Public: tensor facade APIs and SGEMM utility behavior used by core crates.
- Internal: backend-specific micro-optimizations and helper layout are
  implementation details.
