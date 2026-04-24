# Analytic Capabilities

LARQL provides a comprehensive suite of analytic tools for deep model profiling. This document outlines the available analyses and what insights they provide.

## Core Operations

### DESCRIBE: Entity Knowledge Retrieval

Retrieve knowledge edges for a given entity or token.

**Use Cases:**
- Understand what a model knows about a specific entity
- Explore factual knowledge encoded in the model
- Identify relationships and attributes associated with entities

**Example:**
```lql
DESCRIBE 'France'
# Returns: capital → Paris, currency → Euro, language → French, ...
```

**Insights Provided:**
- Knowledge graph edges from the entity
- Relation types and their confidence
- Layer-specific knowledge localization

**Extraction Level Required:** browse

---

### WALK: Feature Scanning

Scan the model for features activated by a prompt across layers.

**Use Cases:**
- Understand which features fire for a given input
- Track feature activation through the network
- Identify layer-specific feature specialization

**Example:**
```lql
WALK 'The capital of France is' --top 10
# Returns: top features at each layer, their activations
```

**Insights Provided:**
- Top-k activated features per layer
- Feature activation strengths
- Layer-by-layer feature evolution
- Gate KNN results for sparse FFN analysis

**Extraction Level Required:** browse

---

### SELECT: SQL-Style Edge Query

Query the knowledge graph with SQL-like syntax.

**Use Cases:**
- Complex knowledge graph queries
- Pattern matching across entities
- Filtering and aggregating knowledge

**Example:**
```lql
SELECT * WHERE relation = 'capital' AND target LIKE 'P%'
# Returns: all capital relations matching the pattern
```

**Insights Provided:**
- Filtered knowledge graph subsets
- Aggregated statistics
- Pattern-based knowledge discovery

**Extraction Level Required:** browse

---

## Inference Operations

### INFER: Full Forward Pass

Execute a complete forward pass through the model.

**Use Cases:**
- Generate predictions from prompts
- Compare model outputs with and without patches
- Test the effect of structural edits

**Example:**
```lql
INFER 'The capital of France is'
# Returns: predicted token probabilities
```

**Insights Provided:**
- Token predictions and probabilities
- Entropy and uncertainty metrics
- Comparison with ground truth

**Extraction Level Required:** inference

---

## Trace Operations

### TRACE: Residual Stream Analysis

Trace information flow through the residual stream layer-by-layer.

**Use Cases:**
- Understand how representations evolve through the network
- Identify where specific information is processed
- Analyze residual norm and angle changes

**Example:**
```lql
TRACE 'The capital of France is'
# Returns: residual stream artifacts per layer
```

**Insights Provided:**
- Residual norms per layer (magnitude changes)
- Token residual angles (directional changes)
- Layer-by-layer representation evolution
- Ridge artifact accumulation

**Extraction Level Required:** inference

---

## Head Analysis

### Attention Head Contributions

Analyze which attention heads contribute to specific predictions or behaviors.

**Use Cases:**
- Identify specialized attention heads
- Understand head redundancy
- Track head-specific information flow

**Example:**
```lql
HEAD_ANALYSIS 'The capital of France is' --layer 20
# Returns: head contributions for the specified layer
```

**Insights Provided:**
- Head-specific attention patterns
- Contribution scores per head
- Head specialization identification
- Redundancy detection across heads

**Extraction Level Required:** inference

---

## Residual Stream Artifacts

### Layer-by-Layer Decomposition

Decompose the residual stream to understand representation evolution.

**Use Cases:**
- Track how concepts are built up layer-by-layer
- Identify critical layers for specific tasks
- Understand representation transformations

**Insights Provided:**
- Per-layer residual vectors
- Cumulative residual accumulation
- Layer importance ranking
- Concept localization

### Residual Norms

Measure the magnitude of residual vectors at each layer.

**Use Cases:**
- Identify layers with large representation changes
- Detect layer collapse or saturation
- Normalize comparisons across layers

**Insights Provided:**
- Per-layer L2 norms
- Norm growth patterns
- Anomaly detection in residual streams

### Token Residual Angles

Measure the angular change in token representations across layers.

**Use Cases:**
- Track semantic drift through the network
- Identify layers where representations stabilize
- Understand transformation geometry

**Insights Provided:**
- Cosine similarity between consecutive layers
- Angular distance matrices
- Stabilization point identification

### Ridge Artifact Accumulation

Track error buildup in residual streams.

**Use Cases:**
- Identify sources of numerical instability
- Understand error propagation
- Optimize numerical precision

**Insights Provided:**
- Ridge artifact magnitude per layer
- Error accumulation patterns
- Numerical stability assessment

---

## Advanced Analyses

### Batch DLA Scan

**Status:** Currently implemented as server tool, migrating to LQL ANALYZE statement

Direct Logit Attribution analysis across multiple analysis blocks. This capability provides scientific attribution with explicit truth/coherence annotations for experimental analysis.

**Use Cases:**
- Identify which layers/heads contribute to specific tokens
- Truth origin tracking with explicit truth/false span resolution
- Attribution flow analysis
- Material coherence vs false content attribution
- Ridge accumulation across trace

**Insights Provided:**
- First false origin detection
- Ridge accumulation by layer
- Top head contributions
- Analysis summary with mode specification
- Token analysis with truth/false attribution
- Generation trace with coherence markers

**Canonical Flow:**
This analysis is being migrated to the core LQL language surface as an ANALYZE statement. The canonical flow will be:
- `larql-inference`: Exposes structured analysis API
- `larql-lql`: Parses ANALYZE INFER statement, executes via structured API
- `larql-server`: batch_dla_scan route becomes transport adapter over same API
- `larql-terminal-batch-dla`: Consumes analysis via LQL or server adapter

**Planned LQL Syntax:**
```lql
ANALYZE INFER "The capital of Freedonia is"
    MODE FACT_PROBE
    TRUTH_SPANS ["Markov"]
    FALSE_SPANS ["Paris", "London"]
    COHERENCE_MARKERS ["The", "capital", "of", "is"]
    TOP 5;
```

**Current Access:**
- Server HTTP endpoint: `POST /v1/tools/batch_dla_scan` (temporary, will become adapter)
- TUI: `larql-terminal-batch-dla` crate (will migrate to LQL consumption)

**Extraction Level Required:** inference

**Migration Status:** In progress. See canonical flow documentation for architecture details.

---

### Context Map

Analyze the context window and token relationships.

**Use Cases:**
- Understand attention patterns across context
- Identify important context tokens
- Analyze context window utilization

**Insights Provided:**
- Token importance scores
- Context window heatmaps
- Attention pattern visualization

**Extraction Level Required:** browse or inference (depending on implementation)

---

## Mutation Operations

### INSERT: Add Knowledge

Add new knowledge edges to the model through patches.

**Use Cases:**
- Correct factual errors
- Add new knowledge
- Test counterfactuals

**Example:**
```lql
INSERT 'NewCity' capital 'NewCountry'
# Creates a patch adding this knowledge
```

**Extraction Level Required:** browse (creates patch)

---

### DELETE: Remove Knowledge

Remove knowledge edges from the model through patches.

**Use Cases:**
- Remove incorrect knowledge
- Test knowledge removal effects
- Understand knowledge localization

**Example:**
```lql
DELETE 'France' capital 'Paris'
# Creates a patch removing this knowledge
```

**Extraction Level Required:** browse (creates patch)

---

### UPDATE: Modify Knowledge

Modify existing knowledge edges through patches.

**Use Cases:**
- Update outdated knowledge
- Adjust knowledge weights
- Test knowledge modifications

**Example:**
```lql
UPDATE 'France' capital 'Paris' WITH weight 0.9
# Creates a patch modifying this knowledge
```

**Extraction Level Required:** browse (creates patch)

---

### COMPILE: Bake Patches

Compile patches into a new standalone vindex.

**Use Cases:**
- Create edited models
- Distribute modified vindexes
- Remove patch overhead

**Example:**
```lql
COMPILE CURRENT INTO 'edited-model.vindex'
# Creates a new vindex with patches baked in
```

**Extraction Level Required:** all

---

## Extraction Levels

LARQL supports three extraction levels that determine which operations are available:

### Browse Level (~3 GB)
- **Operations:** DESCRIBE, WALK, SELECT, INSERT, DELETE, UPDATE
- **Weights:** Gate vectors, embeddings, down metadata
- **Use Case:** Knowledge exploration and structural editing

### Inference Level (~6 GB)
- **Operations:** All browse operations + INFER, TRACE, HEAD_ANALYSIS
- **Weights:** All browse weights + attention weights
- **Use Case:** Full forward pass analysis and attribution

### All Level (~10 GB)
- **Operations:** All inference operations + COMPILE
- **Weights:** All inference weights + compile weights
- **Use Case:** Complete model editing and vindex compilation

---

## Related Documentation

- `VISION.md`: Core analytic framework vision
- `docs/lql-spec.md`: LQL language specification
- `docs/vindex-operations-spec.md`: Vindex operations specification
- `docs/trace-format-spec.md`: Trace format specification
- `docs/residual-trace.md`: Residual trace analysis details
