/**
 * Back-compat barrel: delegates to {@link searchClient} and pure helpers.
 * Prefer importing from `./search` or `./types` in new code.
 */
import { searchClient } from './search';
import {
  computeInfrastructureCost,
  detectSemanticFields,
  flattenDlaMatrix,
} from './search/helpers';

export { computeInfrastructureCost, detectSemanticFields, flattenDlaMatrix };

export type {
  BatchDlaAnalysisRequest,
  StoreInfo,
  WindowData,
  BoundaryData,
} from './types';

export async function getModelInfo() {
  return searchClient.getModelInfo();
}

export async function loadModel(modelId: string) {
  return searchClient.loadModel(modelId);
}

export async function batchDlaScan(
  prompt: string,
  analysis: import('./types').BatchDlaAnalysisRequest,
  targetToken?: string,
) {
  return searchClient.batchDlaScan(prompt, analysis, targetToken);
}

export async function extractAttentionOutput(
  prompt: string,
  layer: number,
  head: number,
) {
  return searchClient.extractAttentionOutput(prompt, layer, head);
}

export async function runInjectionTest(
  prompt: string,
  token: string,
  coefficient: number,
  injectLayer: number,
) {
  return searchClient.runInjectionTest(prompt, token, coefficient, injectLayer);
}

export async function fetchContextMap(
  prompt: string,
  layer: number,
  topK?: number,
  initialResidual?: number[],
) {
  return searchClient.fetchContextMap(prompt, layer, topK, initialResidual);
}

export async function fetchContextMapWithQuery(
  prompt: string,
  query: string,
  layer: number,
  topK?: number,
  initialResidual?: number[],
) {
  return searchClient.fetchContextMapWithQuery(prompt, query, layer, topK, initialResidual);
}

export async function fetchStoreInfo(storePath: string) {
  return searchClient.fetchStoreInfo(storePath);
}

export async function fetchStoreWindow(storePath: string, windowId: number) {
  return searchClient.fetchStoreWindow(storePath, windowId);
}

export async function fetchBoundaryResidual(storePath: string, windowId?: number) {
  return searchClient.fetchBoundaryResidual(storePath, windowId);
}
