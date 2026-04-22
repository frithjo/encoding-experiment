import type {
  ModelConfig,
  DLAScanResult,
  ContentProjectionResult,
  InjectionResult,
  ContextMapEntry,
  StoreInfo,
  WindowData,
  BoundaryData,
} from '../types';
import type { SearchClient } from './client';
import {
  mapContextMapWithQuery,
  mapPositions,
  type RawContextMapResult,
  type RawContextMapWithQueryResult,
} from './context-map-mappers';
import { assertToolPayload, checkLazarusError, normalizeNativeToolResult } from './tool-result';

/**
 * Direct HTTP search backend: POST {base}/tools/call with JSON body
 * `{ name, arguments }` returning the same tool payload shape as MCP (after JSON parse),
 * optionally wrapped as `{ result: ... }`.
 *
 * Ops and parity checks are **terminal-first**: configure `NATIVE_SEARCH_URL` / `VITE_NATIVE_SEARCH_URL`
 * in the shell, hit the same endpoint with `scripts/tools-call-example.sh`, then point the UI at the same base.
 *
 * Set `VITE_NATIVE_SEARCH_URL` (e.g. `http://127.0.0.1:8766`) when using `VITE_SEARCH_BACKEND=native`.
 */
export class NativeSearchAdapter implements SearchClient {
  private readonly baseUrl: string;

  /**
   * @param baseUrlOverride - optional base URL (for tests); otherwise `VITE_NATIVE_SEARCH_URL` is required when using native backend.
   */
  constructor(baseUrlOverride?: string) {
    const base = baseUrlOverride ?? import.meta.env.VITE_NATIVE_SEARCH_URL;
    if (!base || typeof base !== 'string') {
      throw new Error(
        'VITE_NATIVE_SEARCH_URL must be set when VITE_SEARCH_BACKEND=native (e.g. http://127.0.0.1:8766)',
      );
    }
    this.baseUrl = base.replace(/\/$/, '');
  }

  private async invokeTool(name: string, args: Record<string, unknown>): Promise<unknown> {
    const res = await fetch(`${this.baseUrl}/tools/call`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name, arguments: args }),
    });

    if (!res.ok) {
      const text = await res.text();
      throw new Error(`Native search failed (${res.status}): ${text.slice(0, 500)}`);
    }

    const body = await res.json();
    const normalized = normalizeNativeToolResult(body);
    assertToolPayload(normalized);
    checkLazarusError(normalized);
    return normalized;
  }

  async getModelInfo(): Promise<ModelConfig> {
    return (await this.invokeTool('get_model_info', {})) as ModelConfig;
  }

  async loadModel(modelId: string): Promise<ModelConfig> {
    return (await this.invokeTool('load_model', { model_id: modelId })) as ModelConfig;
  }

  async batchDlaScan(prompt: string, targetToken?: string): Promise<DLAScanResult> {
    const args: Record<string, unknown> = { prompt };
    if (targetToken) args.target_token = targetToken;
    return (await this.invokeTool('batch_dla_scan', args)) as DLAScanResult;
  }

  async extractAttentionOutput(
    prompt: string,
    layer: number,
    head: number,
  ): Promise<ContentProjectionResult> {
    return (await this.invokeTool('extract_attention_output', {
      prompt,
      layer,
      head,
      top_k_tokens: 20,
    })) as ContentProjectionResult;
  }

  async runInjectionTest(
    prompt: string,
    token: string,
    coefficient: number,
    injectLayer: number,
  ): Promise<InjectionResult> {
    return (await this.invokeTool('kv_inject_test', {
      prompt,
      token,
      coefficient,
      inject_layer: injectLayer,
      top_k: 15,
    })) as InjectionResult;
  }

  async fetchContextMap(
    prompt: string,
    layer: number,
    topK: number = 5,
    initialResidual?: number[],
  ): Promise<ContextMapEntry[]> {
    const args: Record<string, unknown> = { prompt, layer, top_k: topK };
    if (initialResidual) args.initial_residual = initialResidual;
    const raw = (await this.invokeTool('context_map', args)) as RawContextMapResult;
    return mapPositions(raw);
  }

  async fetchContextMapWithQuery(
    prompt: string,
    query: string,
    layer: number,
    topK: number = 5,
    initialResidual?: number[],
  ): Promise<ContextMapEntry[]> {
    const args: Record<string, unknown> = { prompt, query, layer, top_k: topK };
    if (initialResidual) args.initial_residual = initialResidual;
    const raw = (await this.invokeTool(
      'context_map_with_query',
      args,
    )) as RawContextMapWithQueryResult;
    return mapContextMapWithQuery(raw);
  }

  async fetchStoreInfo(storePath: string): Promise<StoreInfo> {
    return (await this.invokeTool('knowledge_store_info', {
      store_path: storePath,
    })) as StoreInfo;
  }

  async fetchStoreWindow(storePath: string, windowId: number): Promise<WindowData> {
    return (await this.invokeTool('knowledge_store_window', {
      store_path: storePath,
      window_id: windowId,
    })) as WindowData;
  }

  async fetchBoundaryResidual(
    storePath: string,
    windowId: number = -1,
  ): Promise<BoundaryData> {
    return (await this.invokeTool('load_boundary_residual', {
      store_path: storePath,
      window_id: windowId,
    })) as BoundaryData;
  }
}
