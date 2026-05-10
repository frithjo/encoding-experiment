import type { ContextMapEntry } from '../../types';

interface Props {
  entry: ContextMapEntry | null;
  onInject?: (position: number) => void;
  onTrace?: (position: number) => void;
}

export function TokenDetail({ entry, onInject, onTrace }: Props) {
  if (!entry) {
    return (
      <div className="bg-zinc-900/50 rounded-xl p-6 border border-zinc-800 text-zinc-500 text-sm">
        Hover over a token to see details
      </div>
    );
  }

  return (
    <div className="bg-zinc-900/50 rounded-xl p-5 border border-zinc-800 space-y-4">
      <div className="flex items-baseline justify-between">
        <h3 className="text-sm font-medium text-zinc-300">
          Token: <span className="text-amber-400 font-mono">"{entry.token}"</span>
        </h3>
        <span className="text-[10px] text-zinc-600">position {entry.position}</span>
      </div>

      {/* Token metadata */}
      <div className="grid grid-cols-2 gap-2 text-xs">
        <div className="bg-zinc-800/50 rounded-lg p-2.5">
          <div className="text-zinc-500 mb-0.5">Token ID</div>
          <div className="text-zinc-200 font-mono">{entry.tokenId}</div>
        </div>
        <div className="bg-zinc-800/50 rounded-lg p-2.5">
          <div className="text-zinc-500 mb-0.5">Specificity</div>
          <div className="text-zinc-200 font-mono">{entry.specificity.toFixed(3)}</div>
        </div>
        <div className="bg-zinc-800/50 rounded-lg p-2.5">
          <div className="text-zinc-500 mb-0.5">Residual norm</div>
          <div className="text-zinc-200 font-mono">{entry.residualNorm.toLocaleString(undefined, { maximumFractionDigits: 0 })}</div>
        </div>
        <div className="bg-zinc-800/50 rounded-lg p-2.5">
          <div className="text-zinc-500 mb-0.5">Token↔Residual angle</div>
          <div className="text-zinc-200 font-mono">{entry.tokenResidualAngle.toFixed(2)}°</div>
        </div>
      </div>

      {/* Logit lens predictions */}
      <div>
        <div className="text-xs text-zinc-500 mb-2">Model thinks (logit lens top-{entry.predictions.length}):</div>
        <div className="space-y-1">
          {entry.predictions.map((pred, i) => (
            <div key={i} className="flex items-center gap-2 text-xs">
              <span className="text-zinc-600 w-4 text-right">{i + 1}.</span>
              <span className="font-mono text-zinc-200 flex-1 truncate">"{pred.token}"</span>
              <div className="w-24 bg-zinc-800 rounded-full h-1.5 overflow-hidden">
                <div
                  className="h-full bg-amber-500/70 rounded-full"
                  style={{ width: `${Math.min(pred.probability * 100, 100)}%` }}
                />
              </div>
              <span className="text-zinc-400 font-mono w-12 text-right">
                {(pred.probability * 100).toFixed(1)}%
              </span>
            </div>
          ))}
        </div>
      </div>

      {/* Entropy */}
      <div className="text-xs">
        <span className="text-zinc-500">Entropy: </span>
        <span className="text-zinc-300 font-mono">{entry.entropy.toFixed(3)}</span>
        <span className="text-zinc-600 ml-2">
          ({entry.specificity > 0.7 ? 'content-rich' : entry.specificity > 0.3 ? 'moderate' : 'structural/common'})
        </span>
      </div>

      {/* Query attention overlay */}
      {entry.queryAttention && (
        <div>
          <div className="text-xs text-zinc-500 mb-2">Query attention:</div>
          <div className="grid grid-cols-2 gap-2 text-xs">
            <div className="bg-zinc-800/50 rounded-lg p-2.5">
              <div className="text-zinc-500 mb-0.5">H5 attention</div>
              <div className="text-zinc-200 font-mono">{(entry.queryAttention.h5 * 100).toFixed(2)}%</div>
            </div>
            <div className="bg-zinc-800/50 rounded-lg p-2.5">
              <div className="text-zinc-500 mb-0.5">H4 attention</div>
              <div className="text-zinc-200 font-mono">{(entry.queryAttention.h4 * 100).toFixed(2)}%</div>
            </div>
            <div className="bg-zinc-800/50 rounded-lg p-2.5">
              <div className="text-zinc-500 mb-0.5">H2 attention</div>
              <div className="text-zinc-200 font-mono">{(entry.queryAttention.h2 * 100).toFixed(2)}%</div>
            </div>
            <div className="bg-zinc-800/50 rounded-lg p-2.5">
              <div className="text-zinc-500 mb-0.5">Copy head rank</div>
              <div className="text-zinc-200 font-mono">#{entry.queryAttention.copyHeadRank}</div>
            </div>
          </div>
        </div>
      )}

      {/* Action buttons */}
      <div className="flex gap-2 pt-1">
        {onInject && (
          <button
            onClick={() => onInject(entry.position)}
            className="px-3 py-1.5 text-[11px] font-medium bg-zinc-800 hover:bg-zinc-700 text-zinc-300 rounded-lg transition-colors border border-zinc-700"
          >
            Inject from this position
          </button>
        )}
        {onTrace && (
          <button
            onClick={() => onTrace(entry.position)}
            className="px-3 py-1.5 text-[11px] font-medium bg-zinc-800 hover:bg-zinc-700 text-zinc-300 rounded-lg transition-colors border border-zinc-700"
          >
            Trace circuit
          </button>
        )}
      </div>
    </div>
  );
}
