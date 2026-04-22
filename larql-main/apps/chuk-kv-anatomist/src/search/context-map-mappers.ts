import type { ContextMapEntry } from '../types';

export interface RawContextMapResult {
  prompt: string;
  layer: number;
  top_k: number;
  positions: {
    position: number;
    token: string;
    token_id: number;
    predictions: { token: string; token_id: number; probability: number }[];
    entropy: number;
    specificity: number;
    residual_norm: number;
    token_residual_angle: number;
  }[];
}

export interface RawContextMapWithQueryResult extends RawContextMapResult {
  query: string;
  positions: (RawContextMapResult['positions'][number] & {
    h5_attention: number;
    h4_attention: number;
    h2_attention: number;
    copy_head_rank: number;
  })[];
}

export function mapPositions(raw: RawContextMapResult): ContextMapEntry[] {
  return raw.positions.map(p => ({
    position: p.position,
    token: p.token,
    tokenId: p.token_id,
    predictions: p.predictions,
    entropy: p.entropy,
    specificity: p.specificity,
    topProbability: p.predictions[0]?.probability ?? 0,
    residualNorm: p.residual_norm,
    tokenResidualAngle: p.token_residual_angle,
  }));
}

export function mapContextMapWithQuery(raw: RawContextMapWithQueryResult): ContextMapEntry[] {
  return raw.positions.map(p => ({
    position: p.position,
    token: p.token,
    tokenId: p.token_id,
    predictions: p.predictions,
    entropy: p.entropy,
    specificity: p.specificity,
    topProbability: p.predictions[0]?.probability ?? 0,
    residualNorm: p.residual_norm,
    tokenResidualAngle: p.token_residual_angle,
    queryAttention: {
      h5: p.h5_attention,
      h4: p.h4_attention,
      h2: p.h2_attention,
      copyHeadRank: p.copy_head_rank,
    },
  }));
}
