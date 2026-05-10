import type {
  ModelConfig,
  DLAScanResult,
  DLACell,
  InfrastructureCost,
  ContextMapEntry,
  SemanticField,
} from '../types';

/** Flatten the nested BatchDlaScanResult into a flat cell array for the heatmap. */
export function flattenDlaMatrix(result: DLAScanResult): DLACell[] {
  const cells: DLACell[] = [];
  for (const layerEntry of result.layers) {
    for (const headEntry of layerEntry.heads) {
      cells.push({
        layer: layerEntry.layer,
        head: headEntry.head,
        dla: headEntry.dla,
        fraction_of_layer: headEntry.fraction_of_layer,
        top_token: headEntry.top_token,
        is_decomposable: layerEntry.is_decomposable,
      });
    }
  }
  return cells;
}

export function computeInfrastructureCost(
  config: ModelConfig,
  nPositions: number,
  nFacts: number,
  dtypeBytes: number = 2,
): InfrastructureCost {
  const totalKv =
    nPositions * config.num_layers * config.num_kv_heads * 2 * config.head_dim * dtypeBytes;
  const kBytes = totalKv / 2;
  const vBytes = totalKv / 2;
  const factualContent = nFacts * 12;
  const factualIndex = nFacts * (config.head_dim * 2 + 12);
  const ratio = totalKv / Math.max(factualContent, 1);

  const formatBytes = (b: number): string => {
    if (b >= 1e9) return `${(b / 1e9).toFixed(1)} GB`;
    if (b >= 1e6) return `${(b / 1e6).toFixed(2)} MB`;
    if (b >= 1e3) return `${(b / 1e3).toFixed(1)} KB`;
    return `${b} B`;
  };

  return {
    total_kv_bytes: totalKv,
    total_kv_display: formatBytes(totalKv),
    k_bytes: kBytes,
    v_bytes: vBytes,
    n_positions: nPositions,
    n_layers: config.num_layers,
    n_kv_heads: config.num_kv_heads,
    head_dim: config.head_dim,
    dtype_bytes: dtypeBytes,
    factual_content_bytes: factualContent,
    factual_index_bytes: factualIndex,
    n_facts: nFacts,
    ratio,
  };
}

function inferFieldLabel(tokens: string[]): string {
  const content = tokens.filter(t => t.trim().length > 1 && !/^[\n\r\t .,;:!?]+$/.test(t));
  if (content.length === 0) return 'structural';
  return content.slice(0, 3).map(t => t.trim()).join(', ');
}

/** Detect semantic fields from context map by grouping positions with shared predictions. */
export function detectSemanticFields(contextMap: ContextMapEntry[]): SemanticField[] {
  const fields: SemanticField[] = [];
  let currentTokens = new Set<string>();
  let start = 0;

  for (let i = 0; i < contextMap.length; i++) {
    const topPredictions = new Set(contextMap[i].predictions.slice(0, 3).map(p => p.token));

    const overlap = [...topPredictions].some(t => currentTokens.has(t));

    if (overlap && currentTokens.size > 0) {
      topPredictions.forEach(t => currentTokens.add(t));
    } else {
      if (currentTokens.size > 0 && i > start) {
        fields.push({
          start,
          end: i - 1,
          tokens: [...currentTokens],
          label: inferFieldLabel([...currentTokens]),
        });
      }
      start = i;
      currentTokens = topPredictions;
    }
  }

  if (currentTokens.size > 0) {
    fields.push({
      start,
      end: contextMap.length - 1,
      tokens: [...currentTokens],
      label: inferFieldLabel([...currentTokens]),
    });
  }

  return fields;
}
