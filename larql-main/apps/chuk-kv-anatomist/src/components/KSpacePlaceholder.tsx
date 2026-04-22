export function KSpacePlaceholder() {
  return (
    <div className="flex flex-col items-center justify-center py-20 text-zinc-500">
      <p className="text-lg font-medium mb-2">Layer 4: K-Space Crowding</p>
      <p className="text-sm">Why can't the addressing system distinguish entities?</p>
      <div className="mt-6 p-4 bg-zinc-900/50 rounded-xl border border-zinc-800 max-w-md text-center">
        <p className="text-sm text-zinc-400">
          Phase 2: Side-by-side PCA scatter plots of hidden space (2560D) vs K-space (256D).
        </p>
        <p className="text-xs text-zinc-600 mt-2">
          Requires extract_k_vector and extract_q_vector tools.
        </p>
      </div>
    </div>
  );
}
