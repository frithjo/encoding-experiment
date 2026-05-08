import { PRESETS } from '../presets/experiments';

interface Props {
  prompt: string;
  onPromptChange: (prompt: string) => void;
  onRun: () => void;
  isRunning: boolean;
}

export function PromptBar({ prompt, onPromptChange, onRun, isRunning }: Props) {
  return (
    <div className="mt-4 flex gap-2 items-center">
      <div className="relative flex-1">
        <input
          type="text"
          value={prompt}
          onChange={e => onPromptChange(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && !isRunning && onRun()}
          placeholder="Enter a factual prompt..."
          className="w-full bg-zinc-900 border border-zinc-700 rounded-lg px-4 py-2.5 text-sm
                     focus:outline-none focus:border-amber-500/50 focus:ring-1 focus:ring-amber-500/25
                     placeholder-zinc-500"
        />
      </div>
      <select
        onChange={e => {
          if (e.target.value) onPromptChange(e.target.value);
          e.target.value = '';
        }}
        defaultValue=""
        className="bg-zinc-900 border border-zinc-700 rounded-lg px-3 py-2.5 text-sm text-zinc-400
                   focus:outline-none focus:border-amber-500/50"
      >
        <option value="" disabled>Presets</option>
        {PRESETS.map(p => (
          <option key={p.name} value={p.prompt}>{p.name}</option>
        ))}
      </select>
      <button
        onClick={onRun}
        disabled={isRunning || !prompt.trim()}
        className="px-5 py-2.5 bg-amber-600 hover:bg-amber-500 disabled:bg-zinc-700 disabled:text-zinc-500
                   text-sm font-medium rounded-lg transition-colors"
      >
        {isRunning ? (
          <span className="flex items-center gap-2">
            <span className="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
            Running
          </span>
        ) : (
          'Run'
        )}
      </button>
    </div>
  );
}
