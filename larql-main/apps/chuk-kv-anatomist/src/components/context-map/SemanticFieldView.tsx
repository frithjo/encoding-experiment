import type { SemanticField } from '../../types';

interface Props {
  fields: SemanticField[];
  onFieldClick: (field: SemanticField) => void;
  highlightedField: number | null;
}

export function SemanticFieldView({ fields, onFieldClick, highlightedField }: Props) {
  if (fields.length === 0) {
    return null;
  }

  return (
    <div className="bg-zinc-900/50 rounded-xl p-5 border border-zinc-800">
      <h3 className="text-sm font-medium text-zinc-300 mb-3">Semantic Fields</h3>
      <div className="space-y-1.5">
        {fields.map((field, i) => {
          const span = field.end - field.start + 1;
          return (
            <button
              key={i}
              onClick={() => onFieldClick(field)}
              className={`
                w-full text-left px-3 py-2 rounded-lg text-xs transition-colors
                ${highlightedField === i
                  ? 'bg-amber-900/30 border border-amber-700/50 text-amber-300'
                  : 'bg-zinc-800/50 border border-zinc-800 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-300'
                }
              `}
            >
              <div className="flex items-center justify-between">
                <span className="font-medium truncate">{field.label}</span>
                <span className="text-[10px] text-zinc-600 ml-2 shrink-0">
                  pos {field.start}–{field.end} ({span} tokens)
                </span>
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}
