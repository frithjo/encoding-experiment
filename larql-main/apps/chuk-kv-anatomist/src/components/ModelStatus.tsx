import type { ModelConfig } from '../types';

interface Props {
  config: ModelConfig | null;
}

export function ModelStatus({ config }: Props) {
  if (!config) {
    return (
      <div className="mt-3 text-xs text-zinc-500 flex items-center gap-2">
        <span className="w-2 h-2 rounded-full bg-zinc-600" />
        Not connected — run an analysis to connect to Lazarus MCP
      </div>
    );
  }

  return (
    <div className="mt-3 text-xs text-zinc-400 flex items-center gap-4">
      <span className="flex items-center gap-1.5">
        <span className="w-2 h-2 rounded-full bg-emerald-500" />
        {config.model_id}
      </span>
      <span className="text-zinc-600">|</span>
      <span>Layers: {config.num_layers}</span>
      <span>Heads: {config.num_attention_heads}</span>
      <span>KV Heads: {config.num_kv_heads}</span>
      <span>Hidden: {config.hidden_dim}D</span>
      <span>Head: {config.head_dim}D</span>
    </div>
  );
}
