import type { InfrastructureCost as CostType } from '../types';

interface Props {
  cost: CostType | null;
  nPositions: number;
  nFacts: number;
  onParamsChange: (positions: number, facts: number) => void;
}

function formatBytes(b: number): string {
  if (b >= 1e9) return `${(b / 1e9).toFixed(1)} GB`;
  if (b >= 1e6) return `${(b / 1e6).toFixed(2)} MB`;
  if (b >= 1e3) return `${(b / 1e3).toFixed(1)} KB`;
  return `${b} B`;
}

function formatNumber(n: number): string {
  return n.toLocaleString();
}

export function InfrastructureCost({ cost, nPositions, nFacts, onParamsChange }: Props) {
  if (!cost) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-zinc-500">
        <p className="text-lg font-medium mb-2">Layer 3: Infrastructure Cost</p>
        <p className="text-sm">How much of the KV cache supports the 12-byte signal?</p>
        <p className="text-xs mt-4 text-zinc-600">
          Click Run to connect to the model and compute the infrastructure ratio.
        </p>
      </div>
    );
  }

  const contentPct = (cost.factual_content_bytes / cost.total_kv_bytes) * 100;
  const infraPct = 100 - contentPct;

  return (
    <div>
      <div className="mb-6 flex items-start justify-between">
        <div>
          <h2 className="text-lg font-semibold">KV Cache Anatomy</h2>
          <p className="text-sm text-zinc-400 mt-1">
            {formatNumber(cost.n_positions)} positions x {cost.n_layers} layers x {cost.n_kv_heads} KV heads x 2 (K+V) x {cost.head_dim}D x {cost.dtype_bytes}B
          </p>
        </div>
        {/* Parameter inputs */}
        <div className="flex gap-3 items-end">
          <div>
            <label className="block text-xs text-zinc-500 mb-1">Positions</label>
            <input
              type="number"
              value={nPositions}
              onChange={e => onParamsChange(Number(e.target.value) || 1, nFacts)}
              className="w-28 bg-zinc-900 border border-zinc-700 rounded px-2 py-1.5 text-sm text-zinc-300
                         focus:outline-none focus:border-amber-500/50"
            />
          </div>
          <div>
            <label className="block text-xs text-zinc-500 mb-1">Facts</label>
            <input
              type="number"
              value={nFacts}
              onChange={e => onParamsChange(nPositions, Number(e.target.value) || 1)}
              className="w-20 bg-zinc-900 border border-zinc-700 rounded px-2 py-1.5 text-sm text-zinc-300
                         focus:outline-none focus:border-amber-500/50"
            />
          </div>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Total KV Cache */}
        <div className="bg-zinc-900/50 rounded-xl p-6 border border-zinc-800 text-center">
          <p className="text-sm text-zinc-500 mb-2">Total KV Cache</p>
          <p className="text-3xl font-bold text-zinc-100">{cost.total_kv_display}</p>
          <div className="mt-3 text-xs text-zinc-500 space-y-1">
            <div className="flex justify-between">
              <span>K-vectors (addressing):</span>
              <span className="font-mono">{formatBytes(cost.k_bytes)}</span>
            </div>
            <div className="flex justify-between">
              <span>V-vectors (content):</span>
              <span className="font-mono">{formatBytes(cost.v_bytes)}</span>
            </div>
          </div>
        </div>

        {/* Factual Content */}
        <div className="bg-zinc-900/50 rounded-xl p-6 border border-amber-700/30 text-center">
          <p className="text-sm text-zinc-500 mb-2">Factual Content</p>
          <p className="text-3xl font-bold text-amber-400">{formatBytes(cost.factual_content_bytes)}</p>
          <p className="text-xs text-zinc-500 mt-2">
            {formatNumber(cost.n_facts)} facts x 12 bytes
          </p>
          <p className="text-xs text-zinc-600 mt-1">
            (token_id: 4B + coefficient: 8B)
          </p>
        </div>

        {/* Ratio */}
        <div className="bg-zinc-900/50 rounded-xl p-6 border border-zinc-800 text-center">
          <p className="text-sm text-zinc-500 mb-2">Ratio</p>
          <p className="text-3xl font-bold text-red-400">
            {formatNumber(Math.round(cost.ratio))} : 1
          </p>
          <p className="text-xs text-zinc-500 mt-2">
            infrastructure : content
          </p>
        </div>
      </div>

      {/* The stacked bar */}
      <div className="mt-6 bg-zinc-900/50 rounded-xl p-6 border border-zinc-800">
        <h3 className="text-sm font-medium text-zinc-300 mb-4">Structural vs Factual</h3>

        <div className="h-10 rounded-lg overflow-hidden flex">
          <div
            className="bg-zinc-700 flex items-center justify-center text-xs text-zinc-300"
            style={{ width: `${Math.max(infraPct, 99)}%` }}
          >
            Structural Infrastructure — {infraPct.toFixed(7)}%
          </div>
          <div
            className="bg-amber-500"
            style={{ width: `${Math.max(contentPct, 1)}%`, minWidth: '4px' }}
          />
        </div>

        <div className="mt-6 grid grid-cols-2 gap-8">
          <div className="space-y-2">
            <h4 className="text-sm font-medium text-zinc-400">STRUCTURAL INFRASTRUCTURE</h4>
            <ul className="text-xs text-zinc-500 space-y-1">
              <li>Grammar, coherence, register, tone</li>
              <li>Position, syntax, template</li>
              <li>{cost.n_layers - 1} other layers</li>
              <li>{cost.n_kv_heads - 1} other KV heads at copy layer</li>
              <li>Every position ever seen</li>
            </ul>
            <p className="text-lg font-mono text-zinc-300 mt-2">{infraPct.toFixed(7)}%</p>
          </div>
          <div className="space-y-2">
            <h4 className="text-sm font-medium text-amber-400">FACTUAL CONTENT</h4>
            <ul className="text-xs text-zinc-500 space-y-1">
              <li>One scalar</li>
              <li>One direction</li>
              <li>One head</li>
              <li>One layer</li>
            </ul>
            <p className="text-lg font-mono text-amber-400 mt-2">12 bytes</p>
          </div>
        </div>

        {cost.factual_index_bytes > 0 && (
          <div className="mt-6 pt-4 border-t border-zinc-800 text-xs text-zinc-500">
            With addressing index: {formatBytes(cost.factual_index_bytes)} ({formatNumber(cost.n_facts)} facts x {cost.head_dim * 2 + 12}B) — ratio {formatNumber(Math.round(cost.total_kv_bytes / cost.factual_index_bytes))} : 1
          </div>
        )}
      </div>

      <div className="mt-4 p-4 bg-zinc-900/30 rounded-lg border border-zinc-800/50">
        <p className="text-sm text-zinc-400 italic">
          The KV cache is working memory for structural processing.
          Facts hitchhike on it.
        </p>
      </div>
    </div>
  );
}
