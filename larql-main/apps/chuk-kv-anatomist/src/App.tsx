import { useState, useCallback, useEffect } from 'react';
import { Header } from './components/Header';
import { PromptBar } from './components/PromptBar';
import { TabBar } from './components/TabBar';
import { ModelStatus } from './components/ModelStatus';
import { DLAHeatmap } from './components/DLAHeatmap';
import { ContentProjection } from './components/ContentProjection';
import { InfrastructureCost } from './components/InfrastructureCost';
import { KSpacePlaceholder } from './components/KSpacePlaceholder';
import { InjectionTest } from './components/InjectionTest';
import { ContextMap } from './components/context-map/ContextMap';
import { searchClient } from './search';
import { computeInfrastructureCost } from './search/helpers';
import {
  defaultBatchDlaAnalysisRequest,
  type AppState,
  type LayerTab,
  type SelectedCell,
  type AppMode,
  type ContextMapEntry,
  type StoreInfo,
} from './types';

const INITIAL_STATE: AppState = {
  mode: 'tokenAnalysis',
  activeTab: 'dla',
  prompt: 'The capital of France is',
  isRunning: false,
  modelConfig: null,
  selectedCell: null,
  dlaResult: null,
  contentResult: null,
  infrastructureCost: null,
  kspaceResult: null,
  injectionResult: null,
  contextMapResult: null,
  contextMapLayer: 29,
  contextMapTopK: 5,
  contextMapQuery: '',
  error: null,
};

export default function App() {
  const [state, setState] = useState<AppState>(INITIAL_STATE);
  const [nPositions, setNPositions] = useState(370778);
  const [nFacts, setNFacts] = useState(3625);

  // Context map comparison state
  const [comparisonMap, setComparisonMap] = useState<ContextMapEntry[] | null>(null);
  const [comparisonLayer, setComparisonLayer] = useState(14);
  const [cachedLayers, setCachedLayers] = useState<Set<number>>(new Set());
  const [layerCache, setLayerCache] = useState<Map<number, ContextMapEntry[]>>(new Map());

  // Knowledge store state
  const [storeInfo, setStoreInfo] = useState<StoreInfo | null>(null);
  const [currentWindow, setCurrentWindow] = useState(0);
  const [boundaryResidual, setBoundaryResidual] = useState<number[] | null>(null);
  const [boundaryLoaded, setBoundaryLoaded] = useState(false);

  const setTab = useCallback((tab: LayerTab) => {
    setState(s => ({ ...s, activeTab: tab }));
  }, []);

  const setMode = useCallback((mode: AppMode) => {
    setState(s => ({ ...s, mode }));
  }, []);

  const setError = useCallback((error: string | null) => {
    setState(s => ({ ...s, error, isRunning: false }));
  }, []);

  const ensureModel = useCallback(async (forceRecheck = false) => {
    if (state.modelConfig && !forceRecheck) return state.modelConfig;
    try {
      const config = await searchClient.getModelInfo();
      setState(s => ({ ...s, modelConfig: config }));
      return config;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (msg.includes('ModelNotLoaded')) {
        try {
          setState(s => ({ ...s, error: null }));
          const config = await searchClient.loadModel('google/gemma-3-4b-it');
          setState(s => ({ ...s, modelConfig: config }));
          return config;
        } catch (loadErr) {
          const loadMsg = loadErr instanceof Error ? loadErr.message : String(loadErr);
          setError(`Failed to load model: ${loadMsg}`);
          return null;
        }
      }
      setError(`Cannot connect to Lazarus MCP server (${msg}). Is it running on localhost:8765?`);
      return null;
    }
  }, [state.modelConfig, setError]);

  const handleModelNotLoaded = useCallback(async (err: unknown): Promise<boolean> => {
    const msg = err instanceof Error ? err.message : String(err);
    if (msg.includes('ModelNotLoaded')) {
      setState(s => ({ ...s, modelConfig: null }));
      const config = await ensureModel(true);
      return config !== null;
    }
    return false;
  }, [ensureModel]);

  // ── Token Analysis run ────────────────────────────────────────
  const runAnalysis = useCallback(async () => {
    setState(s => ({ ...s, isRunning: true, error: null }));
    const config = await ensureModel();
    if (!config) return;

    try {
      switch (state.activeTab) {
        case 'dla': {
          const result = await searchClient.batchDlaScan(
            state.prompt,
            defaultBatchDlaAnalysisRequest(),
          );
          const hotCell = result.hot_cells?.[0];
          setState(s => ({
            ...s,
            dlaResult: result,
            selectedCell: hotCell
              ? { layer: hotCell.layer, head: hotCell.head }
              : s.selectedCell,
            isRunning: false,
          }));
          break;
        }
        case 'content': {
          const cell = state.selectedCell ?? { layer: 29, head: 4 };
          const result = await searchClient.extractAttentionOutput(
            state.prompt,
            cell.layer,
            cell.head,
          );
          setState(s => ({ ...s, contentResult: result, isRunning: false }));
          break;
        }
        case 'cost': {
          const cost = computeInfrastructureCost(config, nPositions, nFacts);
          setState(s => ({ ...s, infrastructureCost: cost, isRunning: false }));
          break;
        }
        case 'injection': {
          const contentResult = state.contentResult;
          const topProj = contentResult?.top_projections?.[0];
          if (!topProj) {
            setError('Run Layer 1 and Layer 2 first to identify the copy head and target token.');
            return;
          }
          const cell = state.selectedCell ?? { layer: 29, head: 4 };
          const result = await searchClient.runInjectionTest(
            state.prompt,
            topProj.token,
            topProj.coefficient,
            cell.layer + 1,
          );
          setState(s => ({ ...s, injectionResult: result, isRunning: false }));
          break;
        }
        default:
          setState(s => ({ ...s, isRunning: false }));
      }
    } catch (err) {
      const retried = await handleModelNotLoaded(err);
      if (!retried) {
        setError(err instanceof Error ? err.message : String(err));
      } else {
        setError('Model was reloaded after server restart. Please click Run again.');
      }
    }
  }, [state.activeTab, state.prompt, state.selectedCell, state.contentResult, state.modelConfig, ensureModel, handleModelNotLoaded, setError, nPositions, nFacts]);

  // ── Context Map run ───────────────────────────────────────────
  const runContextMap = useCallback(async () => {
    setState(s => ({ ...s, isRunning: true, error: null }));
    const config = await ensureModel();
    if (!config) return;

    const doFetch = async () => {
      const result = await searchClient.fetchContextMap(
        state.prompt,
        state.contextMapLayer,
        state.contextMapTopK,
        boundaryResidual ?? undefined,
      );
      setState(s => ({ ...s, contextMapResult: result, isRunning: false }));
      setLayerCache(prev => new Map(prev).set(state.contextMapLayer, result));
      setCachedLayers(prev => new Set(prev).add(state.contextMapLayer));
    };

    try {
      await doFetch();
    } catch (err) {
      const retried = await handleModelNotLoaded(err);
      if (retried) {
        try { await doFetch(); } catch (err2) {
          setError(err2 instanceof Error ? err2.message : String(err2));
        }
      } else {
        setError(err instanceof Error ? err.message : String(err));
      }
    }
  }, [state.prompt, state.contextMapLayer, state.contextMapTopK, state.modelConfig, boundaryResidual, ensureModel, handleModelNotLoaded, setError]);

  const runContextMapWithQuery = useCallback(async () => {
    if (!state.contextMapQuery.trim()) return;
    setState(s => ({ ...s, isRunning: true, error: null }));
    const config = await ensureModel();
    if (!config) return;

    const doFetch = async () => {
      const result = await searchClient.fetchContextMapWithQuery(
        state.prompt,
        state.contextMapQuery,
        state.contextMapLayer,
        state.contextMapTopK,
        boundaryResidual ?? undefined,
      );
      setState(s => ({ ...s, contextMapResult: result, isRunning: false }));
    };

    try {
      await doFetch();
    } catch (err) {
      const retried = await handleModelNotLoaded(err);
      if (retried) {
        try { await doFetch(); } catch (err2) {
          setError(err2 instanceof Error ? err2.message : String(err2));
        }
      } else {
        setError(err instanceof Error ? err.message : String(err));
      }
    }
  }, [state.prompt, state.contextMapQuery, state.contextMapLayer, state.contextMapTopK, state.modelConfig, boundaryResidual, ensureModel, handleModelNotLoaded, setError]);

  const runComparisonMap = useCallback(async () => {
    const cached = layerCache.get(comparisonLayer);
    if (cached) {
      setComparisonMap(cached);
      return;
    }

    setState(s => ({ ...s, isRunning: true, error: null }));
    const config = await ensureModel();
    if (!config) return;

    try {
      const result = await searchClient.fetchContextMap(
        state.prompt,
        comparisonLayer,
        state.contextMapTopK,
        boundaryResidual ?? undefined,
      );
      setComparisonMap(result);
      setLayerCache(prev => new Map(prev).set(comparisonLayer, result));
      setCachedLayers(prev => new Set(prev).add(comparisonLayer));
      setState(s => ({ ...s, isRunning: false }));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [comparisonLayer, state.prompt, state.contextMapTopK, state.modelConfig, boundaryResidual, ensureModel, setError, layerCache]);

  // Handle layer change — use cache if available
  const onContextMapLayerChange = useCallback((layer: number) => {
    setState(s => ({ ...s, contextMapLayer: layer }));
    const cached = layerCache.get(layer);
    if (cached) {
      setState(s => ({ ...s, contextMapResult: cached }));
    }
  }, [layerCache]);

  // Switch from Context Map to Token Analysis
  const onSwitchToTokenAnalysis = useCallback((position: number) => {
    void position;
    setState(s => ({
      ...s,
      mode: 'tokenAnalysis',
      activeTab: 'dla',
    }));
  }, []);

  // ── Knowledge Store ───────────────────────────────────────────
  const onLoadStore = useCallback(async (storePath: string) => {
    setState(s => ({ ...s, isRunning: true, error: null }));
    try {
      const info = await searchClient.fetchStoreInfo(storePath);
      setStoreInfo(info);
      setCurrentWindow(0);
      setBoundaryResidual(null);
      setBoundaryLoaded(false);

      // Auto-load first window text
      if (info.has_window_tokens && info.n_windows > 0) {
        const config = await ensureModel();
        if (config) {
          const windowData = await searchClient.fetchStoreWindow(storePath, 0);
          setState(s => ({
            ...s,
            prompt: windowData.text,
            contextMapResult: null,
            isRunning: false,
            error: null,
          }));
          // Clear caches when loading new window
          setLayerCache(new Map());
          setCachedLayers(new Set());
          setComparisonMap(null);
        } else {
          setState(s => ({ ...s, isRunning: false }));
        }
      } else {
        setState(s => ({ ...s, isRunning: false }));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [ensureModel, setError]);

  const onWindowChange = useCallback(async (windowId: number) => {
    setCurrentWindow(windowId);
    if (!storeInfo) return;

    setState(s => ({ ...s, isRunning: true, error: null }));
    try {
      const config = await ensureModel();
      if (!config) return;

      const windowData = await searchClient.fetchStoreWindow(storeInfo.store_path, windowId);
      setState(s => ({
        ...s,
        prompt: windowData.text,
        contextMapResult: null,
        isRunning: false,
        error: null,
      }));
      // Clear caches when switching windows
      setLayerCache(new Map());
      setCachedLayers(new Set());
      setComparisonMap(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [storeInfo, ensureModel, setError]);

  const onLoadBoundary = useCallback(async () => {
    if (!storeInfo) return;
    setState(s => ({ ...s, isRunning: true, error: null }));
    try {
      const data = await searchClient.fetchBoundaryResidual(storeInfo.store_path, -1);
      setBoundaryResidual(data.residual);
      setBoundaryLoaded(true);
      setState(s => ({ ...s, isRunning: false }));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [storeInfo, setError]);

  // Auto-compute infrastructure cost
  useEffect(() => {
    if (state.activeTab === 'cost' && state.modelConfig && !state.infrastructureCost) {
      const cost = computeInfrastructureCost(state.modelConfig, nPositions, nFacts);
      setState(s => ({ ...s, infrastructureCost: cost }));
    }
  }, [state.activeTab, state.modelConfig, state.infrastructureCost, nPositions, nFacts]);

  const onCellClick = useCallback((cell: SelectedCell) => {
    setState(s => ({
      ...s,
      selectedCell: cell,
      activeTab: 'content',
      contentResult: null,
    }));
  }, []);

  const setPrompt = useCallback((prompt: string) => {
    setState(s => ({
      ...s,
      prompt,
      dlaResult: null,
      contentResult: null,
      injectionResult: null,
      contextMapResult: null,
      selectedCell: null,
      error: null,
    }));
    setLayerCache(new Map());
    setCachedLayers(new Set());
    setComparisonMap(null);
  }, []);

  const onCostParamsChange = useCallback((positions: number, facts: number) => {
    setNPositions(positions);
    setNFacts(facts);
    if (state.modelConfig) {
      const cost = computeInfrastructureCost(state.modelConfig, positions, facts);
      setState(s => ({ ...s, infrastructureCost: cost }));
    }
  }, [state.modelConfig]);

  const maxLayer = state.modelConfig ? state.modelConfig.num_layers - 1 : 33;

  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100 flex flex-col">
      <Header />
      <div className="flex-1 flex flex-col max-w-7xl mx-auto w-full px-4 pb-6">
        <ModelStatus config={state.modelConfig} />

        {/* Mode toggle */}
        <div className="mt-4 flex items-center gap-2">
          <span className="text-xs text-zinc-500 mr-1">Mode:</span>
          <button
            onClick={() => setMode('tokenAnalysis')}
            className={`
              px-4 py-2 text-sm font-medium rounded-lg transition-colors
              ${state.mode === 'tokenAnalysis'
                ? 'bg-amber-600 text-white'
                : 'bg-zinc-800 text-zinc-400 hover:text-zinc-200 hover:bg-zinc-700'
              }
            `}
          >
            Token Analysis
          </button>
          <button
            onClick={() => setMode('contextMap')}
            className={`
              px-4 py-2 text-sm font-medium rounded-lg transition-colors
              ${state.mode === 'contextMap'
                ? 'bg-amber-600 text-white'
                : 'bg-zinc-800 text-zinc-400 hover:text-zinc-200 hover:bg-zinc-700'
              }
            `}
          >
            Context Map
          </button>
        </div>

        {/* Token Analysis mode */}
        {state.mode === 'tokenAnalysis' && (
          <>
            <PromptBar
              prompt={state.prompt}
              onPromptChange={setPrompt}
              onRun={runAnalysis}
              isRunning={state.isRunning}
            />
            <TabBar activeTab={state.activeTab} onTabChange={setTab} />

            {state.error && (
              <div className="mt-4 p-4 bg-red-900/30 border border-red-700 rounded-lg text-red-300 text-sm">
                {state.error}
              </div>
            )}

            <div className="mt-4 flex-1">
              {state.activeTab === 'dla' && (
                <DLAHeatmap result={state.dlaResult} onCellClick={onCellClick} />
              )}
              {state.activeTab === 'content' && (
                <ContentProjection
                  result={state.contentResult}
                  selectedCell={state.selectedCell}
                />
              )}
              {state.activeTab === 'cost' && (
                <InfrastructureCost
                  cost={state.infrastructureCost}
                  nPositions={nPositions}
                  nFacts={nFacts}
                  onParamsChange={onCostParamsChange}
                />
              )}
              {state.activeTab === 'kspace' && <KSpacePlaceholder />}
              {state.activeTab === 'injection' && (
                <InjectionTest result={state.injectionResult} />
              )}
            </div>
          </>
        )}

        {/* Context Map mode */}
        {state.mode === 'contextMap' && (
          <>
            <PromptBar
              prompt={state.prompt}
              onPromptChange={setPrompt}
              onRun={runContextMap}
              isRunning={state.isRunning}
            />

            {state.error && (
              <div className="mt-4 p-4 bg-red-900/30 border border-red-700 rounded-lg text-red-300 text-sm">
                {state.error}
              </div>
            )}

            <div className="mt-4 flex-1">
              <ContextMap
                contextMap={state.contextMapResult}
                currentLayer={state.contextMapLayer}
                maxLayer={maxLayer}
                topK={state.contextMapTopK}
                query={state.contextMapQuery}
                isLoading={state.isRunning}
                onLayerChange={onContextMapLayerChange}
                onTopKChange={(topK) => setState(s => ({ ...s, contextMapTopK: topK }))}
                onQueryChange={(query) => setState(s => ({ ...s, contextMapQuery: query }))}
                onRun={runContextMap}
                onRunWithQuery={runContextMapWithQuery}
                onSwitchToTokenAnalysis={onSwitchToTokenAnalysis}
                comparisonMap={comparisonMap}
                comparisonLayer={comparisonLayer}
                onComparisonLayerChange={setComparisonLayer}
                onRunComparison={runComparisonMap}
                cachedLayers={cachedLayers}
                onModeChange={setMode}
                storeInfo={storeInfo}
                currentWindow={currentWindow}
                boundaryLoaded={boundaryLoaded}
                onLoadStore={onLoadStore}
                onWindowChange={onWindowChange}
                onLoadBoundary={onLoadBoundary}
              />
            </div>
          </>
        )}
      </div>
    </div>
  );
}
