import type { InjectionResult } from '../types';

interface Props {
  result: InjectionResult | null;
}

export function InjectionTest({ result }: Props) {
  if (!result) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-zinc-500">
        <p className="text-lg font-medium mb-2">Layer 5: Injection Test</p>
        <p className="text-sm">Can you really replace the KV cache with 12 bytes?</p>
        <p className="text-xs mt-4 text-zinc-600 max-w-md text-center">
          Run Layers 1 and 2 first to identify the copy head and target token,
          then switch here and click Run to compare full-attention vs V-injection.
        </p>
      </div>
    );
  }

  const klStr = result.kl_divergence < 0.001
    ? result.kl_divergence.toExponential(3)
    : result.kl_divergence.toFixed(6);

  const faithful = result.kl_divergence < 0.01;

  return (
    <div>
      <div className="mb-4">
        <h2 className="text-lg font-semibold">Injection Test</h2>
        <p className="text-sm text-zinc-400 mt-1">
          Token: "<span className="text-amber-400">{result.token}</span>"
          {' | '}Coeff: {result.coefficient.toFixed(2)}
          {' | '}Layer: {result.inject_layer}
        </p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-6">
        {/* KL Divergence card */}
        <div className={`rounded-xl p-6 border text-center ${
          faithful
            ? 'bg-emerald-900/20 border-emerald-700/40'
            : 'bg-red-900/20 border-red-700/40'
        }`}>
          <p className="text-sm text-zinc-400 mb-2">KL Divergence</p>
          <p className={`text-2xl font-mono font-bold ${
            faithful ? 'text-emerald-400' : 'text-red-400'
          }`}>
            {klStr}
          </p>
          <div className="mt-3 p-2 rounded bg-zinc-900/50">
            <p className="text-xs text-zinc-400">
              {result.kl_divergence < 0.001
                ? 'Statistically indistinguishable.'
                : result.kl_divergence < 0.01
                  ? 'Very close match.'
                  : 'Divergence detected.'}
            </p>
          </div>
        </div>

        {/* Probability comparison */}
        <div className="rounded-xl p-6 border border-amber-700/30 bg-amber-900/10 text-center">
          <p className="text-sm text-zinc-400 mb-2">Full Attention</p>
          <p className="text-2xl font-mono font-bold text-zinc-100">
            {(result.full_target_prob * 100).toFixed(2)}%
          </p>
          <p className="text-xs text-zinc-500 mt-1">P("{result.token}")</p>
        </div>

        <div className="rounded-xl p-6 border border-amber-700/30 bg-amber-900/10 text-center">
          <p className="text-sm text-zinc-400 mb-2">12-Byte Injection</p>
          <p className="text-2xl font-mono font-bold text-amber-400">
            {(result.injected_target_prob * 100).toFixed(2)}%
          </p>
          <p className="text-xs text-zinc-500 mt-1">P("{result.token}")</p>
        </div>

        <div className="rounded-xl p-6 border border-zinc-800 bg-zinc-900/50 text-center">
          <p className="text-sm text-zinc-400 mb-2">Delta</p>
          <p className={`text-2xl font-mono font-bold ${
            Math.abs(result.target_prob_delta) < 0.01 ? 'text-emerald-400' : 'text-red-400'
          }`}>
            {result.target_prob_delta >= 0 ? '+' : ''}{(result.target_prob_delta * 100).toFixed(3)}%
          </p>
          <p className="text-xs text-zinc-500 mt-1">probability shift</p>
        </div>
      </div>

      {/* Side-by-side comparison table */}
      <div className="mt-6 bg-zinc-900/50 rounded-xl p-5 border border-zinc-800">
        <h3 className="text-sm font-medium text-zinc-300 mb-3">Token Probability Comparison</h3>
        <table className="w-full text-sm">
          <thead>
            <tr className="text-zinc-500 text-xs">
              <th className="text-left pb-2 font-medium">Token</th>
              <th className="text-right pb-2 font-medium">Full Attn</th>
              <th className="text-right pb-2 font-medium">Injected</th>
              <th className="text-right pb-2 font-medium">Delta</th>
            </tr>
          </thead>
          <tbody>
            {result.top_k_comparison.map((tp, i) => {
              const absDelta = Math.abs(tp.delta);
              const deltaColor = absDelta < 0.001
                ? 'text-zinc-600'
                : absDelta < 0.01
                  ? 'text-yellow-500'
                  : 'text-red-400';
              return (
                <tr key={tp.token_id} className={`border-t border-zinc-800/50 ${i === 0 ? 'text-amber-400' : 'text-zinc-400'}`}>
                  <td className="py-1.5 font-mono">"{tp.token}"</td>
                  <td className="py-1.5 text-right font-mono">{(tp.full_prob * 100).toFixed(2)}%</td>
                  <td className="py-1.5 text-right font-mono">{(tp.injected_prob * 100).toFixed(2)}%</td>
                  <td className={`py-1.5 text-right font-mono ${deltaColor}`}>
                    {tp.delta >= 0 ? '+' : ''}{(tp.delta * 100).toFixed(3)}%
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      {faithful && (
        <div className="mt-4 p-4 bg-emerald-900/10 rounded-lg border border-emerald-800/30">
          <p className="text-sm text-emerald-300 font-medium">
            12 bytes reproduces {(result.full_target_prob * 100).toFixed(0)}% accuracy for factual retrieval.
          </p>
          <p className="text-xs text-zinc-500 mt-1">
            The full KV cache and the 12-byte injection produce statistically indistinguishable distributions.
          </p>
        </div>
      )}
    </div>
  );
}
