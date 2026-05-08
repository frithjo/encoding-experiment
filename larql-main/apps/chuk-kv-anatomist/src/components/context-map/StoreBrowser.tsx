import { useState, useCallback } from 'react';
import type { StoreInfo } from '../../types';

interface Props {
  storeInfo: StoreInfo | null;
  currentWindow: number;
  isLoading: boolean;
  onLoadStore: (storePath: string) => void;
  onWindowChange: (windowId: number) => void;
  onLoadBoundary: () => void;
  hasBoundary: boolean;
  boundaryLoaded: boolean;
}

export function StoreBrowser({
  storeInfo,
  currentWindow,
  isLoading,
  onLoadStore,
  onWindowChange,
  onLoadBoundary,
  hasBoundary,
  boundaryLoaded,
}: Props) {
  const [storePath, setStorePath] = useState('');

  const handleLoadStore = useCallback(() => {
    if (storePath.trim()) {
      onLoadStore(storePath.trim());
    }
  }, [storePath, onLoadStore]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter') handleLoadStore();
  }, [handleLoadStore]);

  return (
    <div className="bg-zinc-900/50 rounded-xl p-4 border border-zinc-800 space-y-3">
      <h3 className="text-sm font-medium text-zinc-300">Knowledge Store</h3>

      {/* Store path input */}
      <div className="flex gap-2">
        <input
          type="text"
          value={storePath}
          onChange={e => setStorePath(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="/path/to/knowledge_store"
          className="flex-1 bg-zinc-800 border border-zinc-700 rounded-lg px-3 py-1.5 text-sm text-zinc-300 placeholder:text-zinc-600 font-mono"
        />
        <button
          onClick={handleLoadStore}
          disabled={isLoading || !storePath.trim()}
          className="px-3 py-1.5 text-xs font-medium bg-zinc-700 hover:bg-zinc-600 disabled:bg-zinc-800 disabled:text-zinc-600 text-zinc-300 rounded-lg transition-colors border border-zinc-600"
        >
          Load
        </button>
      </div>

      {/* Store info + window selector */}
      {storeInfo && (
        <div className="space-y-3">
          {/* Store metadata */}
          <div className="flex items-center gap-4 text-[11px] text-zinc-500">
            <span>{storeInfo.n_windows} windows</span>
            <span>{storeInfo.window_size} tokens/window</span>
            <span>{storeInfo.total_entries} entries</span>
            <span className="text-zinc-600">v{storeInfo.version}</span>
            {storeInfo.model_id && (
              <span className="text-zinc-600 truncate">{storeInfo.model_id}</span>
            )}
          </div>

          {/* Window slider */}
          <div className="flex items-center gap-3">
            <label className="text-xs text-zinc-500 whitespace-nowrap">Window</label>
            <input
              type="range"
              min={0}
              max={storeInfo.n_windows - 1}
              value={currentWindow}
              onChange={e => onWindowChange(parseInt(e.target.value))}
              className="flex-1 h-1.5 accent-amber-500 cursor-pointer"
            />
            <span className="text-sm font-mono text-amber-400 w-12 text-right">
              {currentWindow}/{storeInfo.n_windows - 1}
            </span>
          </div>

          {/* Boundary residual */}
          {hasBoundary && (
            <div className="flex items-center gap-2">
              <button
                onClick={onLoadBoundary}
                disabled={isLoading || boundaryLoaded}
                className={`px-3 py-1.5 text-[11px] font-medium rounded-lg transition-colors border ${
                  boundaryLoaded
                    ? 'bg-emerald-900/30 border-emerald-700/50 text-emerald-400 cursor-default'
                    : 'bg-zinc-700 hover:bg-zinc-600 border-zinc-600 text-zinc-300'
                }`}
              >
                {boundaryLoaded ? 'Boundary loaded' : 'Load boundary residual'}
              </button>
              <span className="text-[10px] text-zinc-600">
                {boundaryLoaded
                  ? 'Context map will include full document context'
                  : 'Final boundary — carries accumulated document context'
                }
              </span>
            </div>
          )}
          {!hasBoundary && (
            <div className="text-[10px] text-zinc-600">
              No boundary residuals saved in this store
            </div>
          )}
        </div>
      )}
    </div>
  );
}
