import type { ContextMapEntry } from '../../types';

interface Props {
  leftLayer: number;
  rightLayer: number;
  contextMapLeft: ContextMapEntry[] | null;
  contextMapRight: ContextMapEntry[] | null;
  selectedPosition: number | null;
}

export function ComparisonView({
  leftLayer,
  rightLayer,
  contextMapLeft,
  contextMapRight,
  selectedPosition,
}: Props) {
  // Find the selected position in both maps
  const leftEntry = contextMapLeft?.find(e => e.position === selectedPosition);
  const rightEntry = contextMapRight?.find(e => e.position === selectedPosition);

  if (!contextMapLeft || !contextMapRight) {
    return (
      <div className="bg-zinc-900/50 rounded-xl p-6 border border-zinc-800 text-center text-zinc-500 text-sm">
        Select two layers and run analysis to compare
      </div>
    );
  }

  if (!selectedPosition && selectedPosition !== 0) {
    return (
      <div className="bg-zinc-900/50 rounded-xl p-6 border border-zinc-800 text-center text-zinc-500 text-sm">
        Click a token in the document view to compare across layers
      </div>
    );
  }

  return (
    <div className="bg-zinc-900/50 rounded-xl p-5 border border-zinc-800">
      <h3 className="text-sm font-medium text-zinc-300 mb-4">
        Layer Comparison — Token: <span className="text-amber-400 font-mono">"{leftEntry?.token ?? rightEntry?.token ?? '?'}"</span>
        <span className="text-zinc-600 text-xs ml-2">position {selectedPosition}</span>
      </h3>

      <div className="grid grid-cols-2 gap-4">
        {/* Left layer */}
        <div className="space-y-3">
          <div className="text-xs font-medium text-zinc-400 bg-zinc-800/50 rounded-lg px-3 py-2 text-center">
            L{leftLayer}
          </div>
          {leftEntry ? (
            <>
              <div className="text-xs text-zinc-500">
                Specificity: <span className="text-zinc-300 font-mono">{leftEntry.specificity.toFixed(3)}</span>
              </div>
              <div className="space-y-1">
                {leftEntry.predictions.map((pred, i) => (
                  <div key={i} className="flex items-center gap-2 text-xs">
                    <span className="text-zinc-600 w-4 text-right">{i + 1}.</span>
                    <span className="font-mono text-zinc-300 flex-1 truncate">"{pred.token}"</span>
                    <span className="text-zinc-500 font-mono w-12 text-right">
                      {(pred.probability * 100).toFixed(1)}%
                    </span>
                  </div>
                ))}
              </div>
              <div className="text-[10px] text-zinc-600">
                {leftEntry.specificity < 0.3
                  ? 'Generic — model hasn\'t formed understanding yet'
                  : leftEntry.specificity < 0.6
                    ? 'Emerging — some content signal'
                    : 'Specific — model has clear prediction'}
              </div>
            </>
          ) : (
            <div className="text-xs text-zinc-600">No data for this position</div>
          )}
        </div>

        {/* Right layer */}
        <div className="space-y-3">
          <div className="text-xs font-medium text-zinc-400 bg-zinc-800/50 rounded-lg px-3 py-2 text-center">
            L{rightLayer}
          </div>
          {rightEntry ? (
            <>
              <div className="text-xs text-zinc-500">
                Specificity: <span className="text-zinc-300 font-mono">{rightEntry.specificity.toFixed(3)}</span>
              </div>
              <div className="space-y-1">
                {rightEntry.predictions.map((pred, i) => (
                  <div key={i} className="flex items-center gap-2 text-xs">
                    <span className="text-zinc-600 w-4 text-right">{i + 1}.</span>
                    <span className="font-mono text-zinc-300 flex-1 truncate">"{pred.token}"</span>
                    <span className="text-zinc-500 font-mono w-12 text-right">
                      {(pred.probability * 100).toFixed(1)}%
                    </span>
                  </div>
                ))}
              </div>
              <div className="text-[10px] text-zinc-600">
                {rightEntry.specificity < 0.3
                  ? 'Generic — model hasn\'t formed understanding yet'
                  : rightEntry.specificity < 0.6
                    ? 'Emerging — some content signal'
                    : 'Specific — model has clear prediction'}
              </div>
            </>
          ) : (
            <div className="text-xs text-zinc-600">No data for this position</div>
          )}
        </div>
      </div>

      {/* Delta summary */}
      {leftEntry && rightEntry && (
        <div className="mt-4 pt-3 border-t border-zinc-800 text-xs text-zinc-500">
          Specificity change: {' '}
          <span className={`font-mono ${rightEntry.specificity > leftEntry.specificity ? 'text-emerald-400' : 'text-red-400'}`}>
            {rightEntry.specificity > leftEntry.specificity ? '+' : ''}
            {(rightEntry.specificity - leftEntry.specificity).toFixed(3)}
          </span>
          <span className="mx-3">|</span>
          Top prediction change: {' '}
          <span className="font-mono text-zinc-300">
            "{leftEntry.predictions[0]?.token}" → "{rightEntry.predictions[0]?.token}"
          </span>
        </div>
      )}
    </div>
  );
}
