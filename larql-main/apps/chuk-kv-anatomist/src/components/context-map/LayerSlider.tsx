interface Props {
  currentLayer: number;
  maxLayer: number;
  onChange: (layer: number) => void;
  cachedLayers: Set<number>;
  isLoading: boolean;
}

export function LayerSlider({ currentLayer, maxLayer, onChange, cachedLayers, isLoading }: Props) {
  return (
    <div className="flex items-center gap-3">
      <label className="text-xs text-zinc-500 whitespace-nowrap">Layer</label>
      <input
        type="range"
        min={0}
        max={maxLayer}
        value={currentLayer}
        onChange={e => onChange(parseInt(e.target.value))}
        className="flex-1 h-1.5 accent-amber-500 cursor-pointer"
      />
      <span className="text-sm font-mono text-amber-400 w-8 text-right">
        L{currentLayer}
      </span>
      {isLoading && (
        <span className="text-[10px] text-zinc-500 animate-pulse">loading...</span>
      )}
      {!isLoading && cachedLayers.has(currentLayer) && (
        <span className="text-[10px] text-zinc-600">cached</span>
      )}
    </div>
  );
}
