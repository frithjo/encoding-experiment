import type { ContentProjectionResult, SelectedCell } from '../types';

interface Props {
  result: ContentProjectionResult | null;
  selectedCell: SelectedCell | null;
}

export function ContentProjection({ result, selectedCell }: Props) {
  if (!result) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-zinc-500">
        <p className="text-lg font-medium mb-2">Layer 2: Content Projection</p>
        <p className="text-sm">What does the hot head actually retrieve?</p>
        <p className="text-xs mt-4 text-zinc-600">
          {selectedCell
            ? `Selected: L${selectedCell.layer} H${selectedCell.head} — click Run to project`
            : 'Click a cell in the DLA heatmap first, or click Run to use defaults'}
        </p>
      </div>
    );
  }

  const projections = result.top_projections ?? [];
  const maxCoeff = Math.max(...projections.map(p => Math.abs(p.coefficient)), 1);
  const dim = result.dimensionality;

  return (
    <div>
      <div className="mb-4">
        <div className="flex items-center gap-3">
          <h2 className="text-lg font-semibold">
            Content Projection
            <span className="text-zinc-500 font-normal ml-2 text-sm">
              L{result.layer} H{result.head}
            </span>
          </h2>
          {dim?.is_one_dimensional && (
            <span className="px-2 py-0.5 bg-amber-900/30 border border-amber-700/40 rounded text-xs text-amber-400 font-medium">
              1-Dimensional
            </span>
          )}
        </div>
        <p className="text-sm text-zinc-400 mt-1">
          Attention output projected onto token embeddings | norm: {result.vector_norm?.toFixed(2)}
        </p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Bar chart */}
        <div className="bg-zinc-900/50 rounded-xl p-5 border border-zinc-800">
          <h3 className="text-sm font-medium text-zinc-300 mb-4">Token Projections</h3>
          <div className="space-y-2.5">
            {projections.slice(0, 12).map((proj, i) => {
              const width = Math.abs(proj.coefficient) / maxCoeff * 100;
              const isTop = i === 0;
              return (
                <div key={proj.token_id} className="flex items-center gap-3">
                  <div className={`w-20 text-right font-mono text-sm truncate ${
                    isTop ? 'text-amber-400' : 'text-zinc-400'
                  }`}>
                    "{proj.token}"
                  </div>
                  <div className="flex-1 h-5 bg-zinc-800 rounded overflow-hidden">
                    <div
                      className={`h-full rounded transition-all ${
                        proj.coefficient > 0
                          ? isTop ? 'bg-amber-500' : 'bg-amber-700/60'
                          : 'bg-blue-700/40'
                      }`}
                      style={{ width: `${width}%` }}
                    />
                  </div>
                  <div className={`w-24 text-right font-mono text-xs ${
                    isTop ? 'text-amber-400' : 'text-zinc-500'
                  }`}>
                    {proj.coefficient > 0 ? '+' : ''}{proj.coefficient.toFixed(0)}
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        {/* Dimensionality + 12 bytes callout */}
        <div className="space-y-4">
          {dim && (
            <div className="bg-zinc-900/50 rounded-xl p-5 border border-zinc-800">
              <h3 className="text-sm font-medium text-zinc-300 mb-3">Dimensionality of Factual Signal</h3>
              <div className="space-y-2 text-sm">
                <div className="flex justify-between text-zinc-400">
                  <span>Directions for 95% of DLA:</span>
                  <span className="font-mono text-amber-400">{dim.dims_for_95pct}</span>
                </div>
                <div className="flex justify-between text-zinc-400">
                  <span>Directions for 99% of DLA:</span>
                  <span className="font-mono text-amber-400">{dim.dims_for_99pct}</span>
                </div>
                <div className="flex justify-between text-zinc-400">
                  <span>Directions for 99.9% of DLA:</span>
                  <span className="font-mono">{dim.dims_for_999pct}</span>
                </div>
                <div className="flex justify-between text-zinc-400">
                  <span>Top-1 fraction:</span>
                  <span className="font-mono text-amber-400">
                    {(dim.top1_fraction * 100).toFixed(2)}%
                  </span>
                </div>
                {dim.top2_fraction !== undefined && (
                  <div className="flex justify-between text-zinc-400">
                    <span>Top-2 fraction:</span>
                    <span className="font-mono">
                      {(dim.top2_fraction * 100).toFixed(2)}%
                    </span>
                  </div>
                )}
              </div>

              {dim.top1_fraction > 0.95 && (
                <div className="mt-4 p-3 bg-amber-900/20 rounded-lg border border-amber-700/30">
                  <p className="text-sm text-amber-300 font-medium">
                    The factual content is 1D.
                  </p>
                  <p className="text-xs text-zinc-500 mt-1">
                    One scalar. One direction. 12 bytes.
                  </p>
                  <p className="text-xs text-zinc-600 mt-1">
                    token_id (4B) + coefficient (8B) = 12 bytes carries {(dim.top1_fraction * 100).toFixed(1)}% of the factual signal.
                  </p>
                </div>
              )}
            </div>
          )}

          {result.orthogonality && result.orthogonality.length > 0 && (
            <div className="bg-zinc-900/50 rounded-xl p-5 border border-zinc-800">
              <h3 className="text-sm font-medium text-zinc-300 mb-3">
                Orthogonality Test
              </h3>
              <p className="text-xs text-zinc-500 mb-3">
                Cosine between bare residual and answer embeddings (should be near zero)
              </p>
              <div className="space-y-1.5">
                {result.orthogonality.map(o => (
                  <div key={o.token} className="flex justify-between text-sm">
                    <span className="text-zinc-400 font-mono">"{o.token}"</span>
                    <span className={`font-mono ${
                      Math.abs(o.cosine) < 0.05 ? 'text-emerald-400' : 'text-red-400'
                    }`}>
                      {o.cosine >= 0 ? '+' : ''}{o.cosine.toFixed(4)}
                    </span>
                  </div>
                ))}
              </div>
              <p className="text-xs text-zinc-600 mt-3">
                Near-zero = bare query has no opinion about the answer.
                Injection is purely additive.
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
