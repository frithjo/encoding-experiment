import type {
  BatchDlaAnalysisRequest,
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
import { createMcpTransport } from './mcp-transport';
import {
  mapContextMapWithQuery,
  mapPositions,
  type RawContextMapResult,
  type RawContextMapWithQueryResult,
} from './context-map-mappers';

export class McpSearchAdapter implements SearchClient {
  private readonly callTool: ReturnType<typeof createMcpTransport>['callTool'];

  constructor(mcpUrl: string) {
    const transport = createMcpTransport(mcpUrl);
    this.callTool = transport.callTool.bind(transport);
  }

  async getModelInfo(): Promise<ModelConfig> {
    return (await this.callTool('get_model_info', {})) as ModelConfig;
  }

  async loadModel(modelId: string): Promise<ModelConfig> {
    return (await this.callTool('load_model', { model_id: modelId })) as ModelConfig;
  }

  async batchDlaScan(
    prompt: string,
    analysis: BatchDlaAnalysisRequest,
    _targetToken?: string,
  ): Promise<DLAScanResult> {
    const request = {
      prompt,
      top_k: 5,
      mode: analysis.mode,
      truth_spans: analysis.truth_spans,
      materially_false_spans: analysis.materially_false_spans,
      coherence_markers: analysis.coherence_markers,
      max_generated_tokens: analysis.max_generated_tokens ?? null,
      ridge_dead_zone: analysis.ridge_dead_zone ?? null,
    };
    return (await this.callTool('analyze-infer', request)) as DLAScanResult;
  }

  async extractAttentionOutput(
    prompt: string,
    layer: number,
    head: number,
  ): Promise<ContentProjectionResult> {
    return (await this.callTool('extract_attention_output', {
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
    return (await this.callTool('kv_inject_test', {
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
    const raw = (await this.callTool('context_map', args)) as RawContextMapResult;
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
    const raw = (await this.callTool(
      'context_map_with_query',
      args,
    )) as RawContextMapWithQueryResult;
    return mapContextMapWithQuery(raw);
  }

  async fetchStoreInfo(storePath: string): Promise<StoreInfo> {
    return (await this.callTool('knowledge_store_info', {
      store_path: storePath,
    })) as StoreInfo;
  }

  async fetchStoreWindow(storePath: string, windowId: number): Promise<WindowData> {
    return (await this.callTool('knowledge_store_window', {
      store_path: storePath,
      window_id: windowId,
    })) as WindowData;
  }

  async fetchBoundaryResidual(
    storePath: string,
    windowId: number = -1,
  ): Promise<BoundaryData> {
    return (await this.callTool('load_boundary_residual', {
      store_path: storePath,
      window_id: windowId,
    })) as BoundaryData;
  }
}
