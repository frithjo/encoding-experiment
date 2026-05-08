export function Header() {
  return (
    <header className="border-b border-zinc-800 bg-zinc-950/80 backdrop-blur-sm sticky top-0 z-10">
      <div className="max-w-7xl mx-auto px-4 py-3 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-amber-500 to-orange-600 flex items-center justify-center text-sm font-bold text-white">
            KV
          </div>
          <div>
            <h1 className="text-lg font-semibold tracking-tight">KV Anatomist</h1>
            <p className="text-xs text-zinc-500">Decomposing Transformer Working Memory</p>
          </div>
        </div>
        <div className="text-xs text-zinc-600">
          chuk-mcp-lazarus
        </div>
      </div>
    </header>
  );
}
