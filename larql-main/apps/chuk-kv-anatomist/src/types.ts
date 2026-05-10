// ── Model metadata ──────────────────────────────────────────────
export interface ModelConfig {
  model_id: string;
  num_layers: number;
  hidden_dim: number;
  num_attention_heads: number;
  num_kv_heads: number;
  head_dim: number;
  vocab_size: number;
  max_position_embeddings: number;
  architecture: string;
  family: string;
}

// ── Layer 1: DLA Heatmap ────────────────────────────────────────
// Matches BatchDlaScanResult from Lazarus

export interface DlaHeadEntry {
  head: number;
  dla: number;
  fraction_of_layer: number;
  top_token: string;
}

export interface DlaLayerEntry {
  layer: number;
  is_decomposable: boolean;
  heads: DlaHeadEntry[];
  layer_total_dla: number;
}

export interface HotCell {
  layer: number;
  head: number;
  dla: number;
  top_token: string;
  fraction_of_layer: number;
  abs_rank: number;
}

export interface DLAScanResult {
  prompt: string;
  target_token: string;
  target_token_id: number;
  position: number;
  num_layers_scanned: number;
  num_heads: number;
  layers: DlaLayerEntry[];
  hot_cells: HotCell[];
  summary: Record<string, unknown>;
}

export type BatchDlaAnalysisMode = 'fact_probe' | 'workflow_probe';

export interface BatchDlaAnalysisRequest {
  mode: BatchDlaAnalysisMode;
  truth_spans: string[];
  materially_false_spans: string[];
  coherence_markers: string[];
  max_generated_tokens?: number | null;
  ridge_dead_zone?: number | null;
}

export function defaultBatchDlaAnalysisRequest(): BatchDlaAnalysisRequest {
  return {
    mode: 'fact_probe',
    truth_spans: [],
    materially_false_spans: [],
    coherence_markers: [],
    max_generated_tokens: 1,
    ridge_dead_zone: 0.05,
  };
}

// Flat cell for the heatmap rendering (derived from DLAScanResult)
export interface DLACell {
  layer: number;
  head: number;
  dla: number;
  fraction_of_layer: number;
  top_token: string;
  is_decomposable: boolean;
}

// ── Layer 2: Content Projection ─────────────────────────────────
// Matches AttentionOutputResult from Lazarus

export interface TokenProjection {
  token: string;
  token_id: number;
  coefficient: number;
  fraction: number;
}

export interface ContentProjectionResult {
  prompt: string;
  layer: number;
  head: number;
  position: number;
  vector_norm: number;
  top_projections: TokenProjection[];
  dimensionality: {
    dims_for_95pct: number;
    dims_for_99pct: number;
    dims_for_999pct: number;
    top1_fraction: number;
    top2_fraction: number;
    is_one_dimensional: boolean;
  };
  // Orthogonality test (computed client-side or from decode_residual)
  orthogonality?: {
    token: string;
    cosine: number;
  }[];
}

// ── Layer 3: Infrastructure Cost ────────────────────────────────
export interface InfrastructureCost {
  total_kv_bytes: number;
  total_kv_display: string;
  k_bytes: number;
  v_bytes: number;
  n_positions: number;
  n_layers: number;
  n_kv_heads: number;
  head_dim: number;
  dtype_bytes: number;
  factual_content_bytes: number;
  factual_index_bytes: number;
  n_facts: number;
  ratio: number;
}

// ── Layer 4: K-Space Crowding (Phase 2) ─────────────────────────
export interface Point2D {
  x: number;
  y: number;
  label: string;
  entity: string;
  template: string;
}

export interface KSpaceResult {
  hidden_points: Point2D[];
  k_points: Point2D[];
  pairwise_hidden: { pair: string; angle: number }[];
  pairwise_k: { pair: string; angle: number }[];
  preservation_score: number;
}

// ── Layer 5: Injection Test ─────────────────────────────────────
// Matches KvInjectTestResult from Lazarus

export interface TokenProbEntry {
  token: string;
  token_id: number;
  full_prob: number;
  injected_prob: number;
  delta: number;
}

export interface InjectionResult {
  prompt: string;
  token: string;
  token_id: number;
  coefficient: number;
  inject_layer: number;
  position: number;
  full_target_prob: number;
  injected_target_prob: number;
  target_prob_delta: number;
  kl_divergence: number;
  top_k_comparison: TokenProbEntry[];
  summary: Record<string, unknown>;
}

// ── Context Map ─────────────────────────────────────────────────

export interface ContextMapPrediction {
  token: string;
  token_id: number;
  probability: number;
}

export interface ContextMapEntry {
  position: number;
  token: string;
  tokenId: number;

  // Logit lens predictions
  predictions: ContextMapPrediction[];

  // Specificity metrics
  entropy: number;
  specificity: number;      // 1 - normalised entropy (0=generic, 1=specific)
  topProbability: number;

  // Residual properties
  residualNorm: number;
  tokenResidualAngle: number;  // degrees

  // Query overlay (optional, when query provided)
  queryAttention?: {
    h5: number;
    h4: number;
    h2: number;
    copyHeadRank: number;
  };
}

export interface SemanticField {
  start: number;
  end: number;
  tokens: string[];
  label: string;
}

// ── Knowledge Store (from Lazarus / search backends) ─────────────

export interface StoreInfo {
  store_path: string;
  model_id: string;
  n_windows: number;
  window_size: number;
  entries_per_window: number;
  total_entries: number;
  has_boundary_residual: boolean;
  has_window_tokens: boolean;
  version: number;
}

export interface WindowData {
  store_path: string;
  window_id: number;
  n_windows: number;
  text: string;
  token_ids: number[];
  num_tokens: number;
}

export interface BoundaryData {
  store_path: string;
  window_id: number;
  hidden_dim: number;
  residual: number[];
}

export type ContextMapColorBy = 'specificity' | 'entropy' | 'queryAttention' | 'residualNorm';

// ── Application state ───────────────────────────────────────────
export type LayerTab = 'dla' | 'content' | 'cost' | 'kspace' | 'injection';
export type AppMode = 'tokenAnalysis' | 'contextMap';

export interface SelectedCell {
  layer: number;
  head: number;
}

export interface AppState {
  mode: AppMode;
  activeTab: LayerTab;
  prompt: string;
  isRunning: boolean;
  modelConfig: ModelConfig | null;
  selectedCell: SelectedCell | null;

  // Layer results
  dlaResult: DLAScanResult | null;
  contentResult: ContentProjectionResult | null;
  infrastructureCost: InfrastructureCost | null;
  kspaceResult: KSpaceResult | null;
  injectionResult: InjectionResult | null;

  // Context Map results
  contextMapResult: ContextMapEntry[] | null;
  contextMapLayer: number;
  contextMapTopK: number;
  contextMapQuery: string;

  error: string | null;
}
