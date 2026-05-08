# Vision

LARQL views transformer models as graph-like databases, enabling analytic tools that provide deep insights into model internals. The framework decompiles model weights into a queryable format (vindex) and provides a SQL-like query language (LQL) for exploring, analyzing, and profiling neural network architectures.

## Core Concept

**The model IS the database.**

Instead of treating transformer models as opaque black boxes, LARQL exposes their internal structure as a graph database where:
- Gate vectors and embeddings are queryable nodes
- Attention heads and FFN features are edges
- Residual streams are traceable artifacts
- Model weights are inspectable and mutable through patch overlays

## Purpose

Design analytic tools that provide comprehensive insights into transformer models, enabling researchers and engineers to:
- Understand how knowledge is encoded across layers and heads
- Trace information flow through the residual stream
- Identify which components contribute to specific outputs
- Edit model behavior through structural patches (not fine-tuning)
- Profile models on the deepest levels reachable

## Scope

LARQL provides analysis capabilities across multiple dimensions:

### Model Weights
- Gate vectors: Input projection weights for each layer
- Down weights: Output projection weights for each layer
- Attention weights: Query/key/value projections
- FFN weights: Intermediate and output projections

### Token Attributes
- Embeddings: Token representations in the model's space
- Token-to-token relationships: Attention patterns
- Feature activation: Which features fire for specific tokens

### Edges and Graph Structure
- Knowledge edges: Entity relationships (e.g., "France" → "capital" → "Paris")
- Feature edges: Connections between layers and heads
- Semantic edges: Relationships discovered through graph algorithms

### Heads Analysis
- Attention head contributions: Which heads drive specific predictions
- Head specialization: What each head learns to detect
- Head redundancy: Overlapping functionality across heads

### Residual Stream Artifacts
- Layer-by-layer decomposition: How representations evolve
- Residual norms: Magnitude changes across layers
- Token residual angles: Directional changes in representation space
- Ridge artifact accumulation: Error buildup in residual streams

## Goals

1. **Full Model Profiling**: Provide tools to analyze every component of a transformer model, from individual weights to high-level behaviors
2. **Deep Inspection**: Enable analysis at the deepest levels reachable, including individual neurons, attention heads, and residual stream positions
3. **Editable Knowledge**: Allow structural edits through patch overlays that modify behavior without retraining
4. **Multi-Model Support**: Work across different model architectures (BitNet, Gemma, etc.) with consistent interfaces
5. **Queryable Interface**: Provide SQL-like queries (LQL) for complex analysis tasks

## Framework

The LARQL framework enables this vision through:

- **Vindex**: A directory of mmap'd files representing a decompiled transformer model, queryable like a graph database
- **LQL (Lazarus Query Language)**: SQL-like language for browsing, mutating, and recompiling model knowledge
- **Extraction Levels**: Three extraction levels (browse, inference, all) that gate which LQL statements work based on available weights
- **Patches**: Overlay system (.vlp JSON files) that stack onto a readonly base vindex for structural edits

## Interfaces

The core analytic framework is accessible through multiple interfaces:
- **CLI**: Command-line tools for extraction, querying, and serving
- **Python SDK**: Programmatic access for data scientists and researchers
- **Server**: HTTP/gRPC API for remote access
- **Terminal Browser**: Primary interface for interactive exploration in the terminal
- **Python Workbench**: Web-based UI for recipe-driven analysis

See `docs/ui/README.md` for detailed information on each interface and their current status.

## Related Documentation

- `README.md`: Project overview and getting started
- `docs/analytic-capabilities.md`: Detailed overview of all available analyses
- `docs/ui/README.md`: Interface strategies and status
- `docs/lql-spec.md`: LQL language specification
- `docs/vindex-format-spec.md`: Vindex file format specification
