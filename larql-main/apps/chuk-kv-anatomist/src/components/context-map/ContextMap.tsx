import { useState, useCallback, useMemo, useRef } from 'react';
import type { ContextMapEntry, ContextMapColorBy, SemanticField, AppMode } from '../../types';
import type { StoreInfo } from '../../types';
import { detectSemanticFields } from '../../search/helpers';
import { DocumentView } from './DocumentView';
import { TokenDetail } from './TokenDetail';
import { LayerSlider } from './LayerSlider';
import { SemanticFieldView } from './SemanticFieldView';
import { ComparisonView } from './ComparisonView';
import { StoreBrowser } from './StoreBrowser';

type SubView = 'document' | 'comparison';

interface Props {
  contextMap: ContextMapEntry[] | null;
  currentLayer: number;
  maxLayer: number;
  topK: number;
  query: string;
  isLoading: boolean;
  onLayerChange: (layer: number) => void;
  onTopKChange: (topK: number) => void;
  onQueryChange: (query: string) => void;
  onRun: () => void;
  onRunWithQuery: () => void;
  onSwitchToTokenAnalysis: (position: number) => void;
  // Comparison mode
  comparisonMap: ContextMapEntry[] | null;
  comparisonLayer: number;
  onComparisonLayerChange: (layer: number) => void;
  onRunComparison: () => void;
  cachedLayers: Set<number>;
  onModeChange: (mode: AppMode) => void;
  // Store browser
  storeInfo: StoreInfo | null;
  currentWindow: number;
  boundaryLoaded: boolean;
  onLoadStore: (storePath: string) => void;
  onWindowChange: (windowId: number) => void;
  onLoadBoundary: () => void;
}

const COLOR_OPTIONS: { value: ContextMapColorBy; label: string }[] = [
  { value: 'specificity', label: 'Specificity' },
  { value: 'entropy', label: 'Entropy' },
  { value: 'queryAttention', label: 'Query Attention' },
  { value: 'residualNorm', label: 'Residual Norm' },
];

export function ContextMap({
  contextMap,
  currentLayer,
  maxLayer,
  topK,
  query,
  isLoading,
  onLayerChange,
  onTopKChange,
  onQueryChange,
  onRun,
  onRunWithQuery,
  onSwitchToTokenAnalysis,
  comparisonMap,
  comparisonLayer,
  onComparisonLayerChange,
  onRunComparison,
  cachedLayers,
  storeInfo,
  currentWindow,
  boundaryLoaded,
  onLoadStore,
  onWindowChange,
  onLoadBoundary,
}: Props) {
  const [colorBy, setColorBy] = useState<ContextMapColorBy>('specificity');
  const [hoveredEntry, setHoveredEntry] = useState<ContextMapEntry | null>(null);
  const [selectedPositions, setSelectedPositions] = useState<number[]>([]);
  const [subView, setSubView] = useState<SubView>('document');
  const [highlightedField, setHighlightedField] = useState<number | null>(null);
  const documentRef = useRef<HTMLDivElement>(null);

  const semanticFields = useMemo(() => {
    if (!contextMap) return [];
    return detectSemanticFields(contextMap);
  }, [contextMap]);

  const handleTokenHover = useCallback((entry: ContextMapEntry | null) => {
    setHoveredEntry(entry);
    if (entry && semanticFields.length > 0) {
      const fieldIdx = semanticFields.findIndex(
        f => entry.position >= f.start && entry.position <= f.end
      );
      setHighlightedField(fieldIdx >= 0 ? fieldIdx : null);
    } else {
      setHighlightedField(null);
    }
  }, [semanticFields]);

  const handleTokenClick = useCallback((entry: ContextMapEntry) => {
    setSelectedPositions(prev =>
      prev.includes(entry.position)
        ? prev.filter(p => p !== entry.position)
        : [...prev, entry.position]
    );
  }, []);

  const handleFieldClick = useCallback((field: SemanticField) => {
    const positions: number[] = [];
    for (let i = field.start; i <= field.end; i++) {
      positions.push(i);
    }
    setSelectedPositions(positions);
  }, []);

  const handleInject = useCallback((position: number) => {
    onSwitchToTokenAnalysis(position);
  }, [onSwitchToTokenAnalysis]);

  return (
    <div className="space-y-4">
      {/* Knowledge Store Browser */}
      <StoreBrowser
        storeInfo={storeInfo}
        currentWindow={currentWindow}
        isLoading={isLoading}
        onLoadStore={onLoadStore}
        onWindowChange={onWindowChange}
        onLoadBoundary={onLoadBoundary}
        hasBoundary={storeInfo?.has_boundary_residual ?? false}
        boundaryLoaded={boundaryLoaded}
      />

      {/* Controls bar */}
      <div className="bg-zinc-900/50 rounded-xl p-4 border border-zinc-800 space-y-3">
        <div className="flex items-center gap-4 flex-wrap">
          {/* Layer slider */}
          <div className="flex-1 min-w-48">
            <LayerSlider
              currentLayer={currentLayer}
              maxLayer={maxLayer}
              onChange={onLayerChange}
              cachedLayers={cachedLayers}
              isLoading={isLoading}
            />
          </div>

          {/* Top-K selector */}
          <div className="flex items-center gap-2">
            <label className="text-xs text-zinc-500">Top-K</label>
            <select
              value={topK}
              onChange={e => onTopKChange(parseInt(e.target.value))}
              className="bg-zinc-800 border border-zinc-700 rounded-lg px-2 py-1 text-xs text-zinc-300"
            >
              {[3, 5, 10, 15, 20].map(k => (
                <option key={k} value={k}>{k}</option>
              ))}
            </select>
          </div>

          {/* Color-by selector */}
          <div className="flex items-center gap-2">
            <label className="text-xs text-zinc-500">Color by</label>
            <select
              value={colorBy}
              onChange={e => setColorBy(e.target.value as ContextMapColorBy)}
              className="bg-zinc-800 border border-zinc-700 rounded-lg px-2 py-1 text-xs text-zinc-300"
            >
              {COLOR_OPTIONS.map(opt => (
                <option key={opt.value} value={opt.value}>{opt.label}</option>
              ))}
            </select>
          </div>

          {/* Sub-view toggle */}
          <div className="flex items-center gap-1 bg-zinc-800 rounded-lg p-0.5">
            <button
              onClick={() => setSubView('document')}
              className={`px-3 py-1 text-xs rounded-md transition-colors ${
                subView === 'document'
                  ? 'bg-zinc-700 text-amber-400'
                  : 'text-zinc-400 hover:text-zinc-300'
              }`}
            >
              Document
            </button>
            <button
              onClick={() => setSubView('comparison')}
              className={`px-3 py-1 text-xs rounded-md transition-colors ${
                subView === 'comparison'
                  ? 'bg-zinc-700 text-amber-400'
                  : 'text-zinc-400 hover:text-zinc-300'
              }`}
            >
              Compare
            </button>
          </div>

          {/* Boundary indicator */}
          {boundaryLoaded && (
            <span className="text-[10px] text-emerald-400 bg-emerald-900/30 px-2 py-1 rounded">
              + boundary
            </span>
          )}

          {/* Run button */}
          <button
            onClick={onRun}
            disabled={isLoading}
            className="px-4 py-1.5 text-sm font-medium bg-amber-600 hover:bg-amber-500 disabled:bg-zinc-700 disabled:text-zinc-500 text-white rounded-lg transition-colors"
          >
            {isLoading ? 'Running...' : 'Run'}
          </button>
        </div>

        {/* Query input for overlay mode */}
        <div className="flex items-center gap-2">
          <label className="text-xs text-zinc-500 whitespace-nowrap">Query overlay</label>
          <input
            type="text"
            value={query}
            onChange={e => onQueryChange(e.target.value)}
            placeholder="Enter query to overlay attention pattern..."
            className="flex-1 bg-zinc-800 border border-zinc-700 rounded-lg px-3 py-1.5 text-sm text-zinc-300 placeholder:text-zinc-600"
          />
          <button
            onClick={onRunWithQuery}
            disabled={isLoading || !query.trim()}
            className="px-3 py-1.5 text-xs font-medium bg-zinc-700 hover:bg-zinc-600 disabled:bg-zinc-800 disabled:text-zinc-600 text-zinc-300 rounded-lg transition-colors border border-zinc-600"
          >
            Overlay
          </button>
        </div>

        {/* Comparison layer controls */}
        {subView === 'comparison' && (
          <div className="flex items-center gap-3 pt-1 border-t border-zinc-800">
            <span className="text-xs text-zinc-500">Compare with</span>
            <div className="flex-1 max-w-64">
              <LayerSlider
                currentLayer={comparisonLayer}
                maxLayer={maxLayer}
                onChange={onComparisonLayerChange}
                cachedLayers={cachedLayers}
                isLoading={false}
              />
            </div>
            <button
              onClick={onRunComparison}
              disabled={isLoading}
              className="px-3 py-1 text-xs font-medium bg-zinc-700 hover:bg-zinc-600 disabled:bg-zinc-800 disabled:text-zinc-600 text-zinc-300 rounded-lg transition-colors border border-zinc-600"
            >
              Load comparison
            </button>
          </div>
        )}
      </div>

      {/* Main content area */}
      <div className="grid grid-cols-1 lg:grid-cols-4 gap-4">
        {/* Document / Comparison view (3 cols) */}
        <div className="lg:col-span-3 space-y-4" ref={documentRef}>
          {subView === 'document' ? (
            <DocumentView
              contextMap={contextMap ?? []}
              colorBy={colorBy}
              hoveredPosition={hoveredEntry?.position ?? null}
              selectedPositions={selectedPositions}
              onTokenHover={handleTokenHover}
              onTokenClick={handleTokenClick}
            />
          ) : (
            <ComparisonView
              leftLayer={currentLayer}
              rightLayer={comparisonLayer}
              contextMapLeft={contextMap}
              contextMapRight={comparisonMap}
              selectedPosition={selectedPositions[selectedPositions.length - 1] ?? null}
            />
          )}

          {subView === 'comparison' && contextMap && (
            <DocumentView
              contextMap={contextMap}
              colorBy={colorBy}
              hoveredPosition={hoveredEntry?.position ?? null}
              selectedPositions={selectedPositions}
              onTokenHover={handleTokenHover}
              onTokenClick={handleTokenClick}
            />
          )}
        </div>

        {/* Right sidebar (1 col) */}
        <div className="space-y-4">
          <TokenDetail
            entry={hoveredEntry}
            onInject={handleInject}
            onTrace={handleInject}
          />
          <SemanticFieldView
            fields={semanticFields}
            onFieldClick={handleFieldClick}
            highlightedField={highlightedField}
          />
        </div>
      </div>
    </div>
  );
}
