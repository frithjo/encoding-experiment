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
import { McpSearchAdapter } from './mcp-adapter';
import { NativeSearchAdapter } from './native-adapter';

export type SearchBackend = 'mcp' | 'native';

export interface SearchClient {
  getModelInfo(): Promise<ModelConfig>;
  loadModel(modelId: string): Promise<ModelConfig>;
  batchDlaScan(prompt: string, targetToken?: string): Promise<DLAScanResult>;
  extractAttentionOutput(
    prompt: string,
    layer: number,
    head: number,
  ): Promise<ContentProjectionResult>;
  runInjectionTest(
    prompt: string,
    token: string,
    coefficient: number,
    injectLayer: number,
  ): Promise<InjectionResult>;
  fetchContextMap(
    prompt: string,
    layer: number,
    topK?: number,
    initialResidual?: number[],
  ): Promise<ContextMapEntry[]>;
  fetchContextMapWithQuery(
    prompt: string,
    query: string,
    layer: number,
    topK?: number,
    initialResidual?: number[],
  ): Promise<ContextMapEntry[]>;
  fetchStoreInfo(storePath: string): Promise<StoreInfo>;
  fetchStoreWindow(storePath: string, windowId: number): Promise<WindowData>;
  fetchBoundaryResidual(storePath: string, windowId?: number): Promise<BoundaryData>;
}

function resolveBackend(): SearchBackend {
  const raw = import.meta.env.VITE_SEARCH_BACKEND;
  if (raw === 'native') return 'native';
  return 'mcp';
}

/**
 * Factory: MCP (default) uses VITE_MCP_URL; native uses VITE_NATIVE_SEARCH_URL for direct JSON tool calls.
 */
export function createSearchClient(): SearchClient {
  const backend = resolveBackend();
  if (backend === 'native') {
    return new NativeSearchAdapter();
  }
  const mcpUrl = import.meta.env.VITE_MCP_URL || 'http://localhost:8765';
  return new McpSearchAdapter(mcpUrl);
}
