import { useMemo } from 'react';
import type { ContextMapEntry, ContextMapColorBy } from '../../types';

interface Props {
  contextMap: ContextMapEntry[];
  colorBy: ContextMapColorBy;
  hoveredPosition: number | null;
  selectedPositions: number[];
  onTokenHover: (entry: ContextMapEntry | null) => void;
  onTokenClick: (entry: ContextMapEntry) => void;
}

function getColor(entry: ContextMapEntry, colorBy: ContextMapColorBy): string {
  let value: number;
  switch (colorBy) {
    case 'specificity':
      value = entry.specificity;
      break;
    case 'entropy':
      // Invert: low entropy = bright
      value = 1 - Math.min(entry.entropy / 10, 1);
      break;
    case 'queryAttention':
      value = entry.queryAttention ? Math.min(entry.queryAttention.h5 * 5, 1) : 0;
      break;
    case 'residualNorm':
      // Normalize to 0-1 range (norms can be very large)
      value = Math.min(entry.residualNorm / 100000, 1);
      break;
  }

  // Grey → Amber → Bright orange/white scale
  if (value < 0.2) {
    const t = value / 0.2;
    const r = Math.round(63 + t * 30);
    const g = Math.round(63 + t * 20);
    const b = Math.round(70 + t * 5);
    return `rgb(${r}, ${g}, ${b})`;
  } else if (value < 0.6) {
    const t = (value - 0.2) / 0.4;
    const r = Math.round(93 + t * 127);
    const g = Math.round(83 + t * 60);
    const b = Math.round(75 - t * 55);
    return `rgb(${r}, ${g}, ${b})`;
  } else {
    const t = (value - 0.6) / 0.4;
    const r = Math.round(220 + t * 35);
    const g = Math.round(143 + t * 80);
    const b = Math.round(20 + t * 40);
    return `rgb(${r}, ${g}, ${b})`;
  }
}

function getBgColor(entry: ContextMapEntry, colorBy: ContextMapColorBy): string {
  let value: number;
  switch (colorBy) {
    case 'specificity':
      value = entry.specificity;
      break;
    case 'entropy':
      value = 1 - Math.min(entry.entropy / 10, 1);
      break;
    case 'queryAttention':
      value = entry.queryAttention ? Math.min(entry.queryAttention.h5 * 5, 1) : 0;
      break;
    case 'residualNorm':
      value = Math.min(entry.residualNorm / 100000, 1);
      break;
  }

  const alpha = Math.max(0.05, value * 0.35);
  if (value < 0.3) return `rgba(161, 161, 170, ${alpha})`;  // zinc
  if (value < 0.6) return `rgba(245, 158, 11, ${alpha})`;   // amber
  return `rgba(251, 191, 36, ${alpha})`;                      // bright amber
}

export function DocumentView({
  contextMap,
  colorBy,
  hoveredPosition,
  selectedPositions,
  onTokenHover,
  onTokenClick,
}: Props) {
  const selectedSet = useMemo(() => new Set(selectedPositions), [selectedPositions]);

  if (contextMap.length === 0) {
    return (
      <div className="bg-zinc-900/50 rounded-xl p-8 border border-zinc-800 text-center text-zinc-500">
        Run a context map analysis to see the document view
      </div>
    );
  }

  return (
    <div className="bg-zinc-900/50 rounded-xl p-6 border border-zinc-800">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-sm font-medium text-zinc-300">Document View</h3>
        <div className="flex items-center gap-4 text-[10px] text-zinc-500">
          <span className="flex items-center gap-1">
            <span className="inline-block w-3 h-3 rounded" style={{ background: 'rgba(161,161,170,0.2)' }} />
            low specificity
          </span>
          <span className="flex items-center gap-1">
            <span className="inline-block w-3 h-3 rounded" style={{ background: 'rgba(245,158,11,0.25)' }} />
            medium
          </span>
          <span className="flex items-center gap-1">
            <span className="inline-block w-3 h-3 rounded" style={{ background: 'rgba(251,191,36,0.35)' }} />
            high specificity
          </span>
        </div>
      </div>

      <div className="font-mono text-sm leading-relaxed flex flex-wrap">
        {contextMap.map(entry => {
          const isHovered = hoveredPosition === entry.position;
          const isSelected = selectedSet.has(entry.position);

          return (
            <span
              key={entry.position}
              className="cursor-pointer transition-all duration-100 rounded-sm px-px"
              style={{
                color: getColor(entry, colorBy),
                backgroundColor: isHovered
                  ? 'rgba(245, 158, 11, 0.3)'
                  : isSelected
                    ? 'rgba(59, 130, 246, 0.25)'
                    : getBgColor(entry, colorBy),
                outline: isHovered ? '1px solid rgba(245, 158, 11, 0.5)' : 'none',
                textDecoration: isSelected ? 'underline' : 'none',
                textDecorationColor: 'rgba(59, 130, 246, 0.5)',
              }}
              onMouseEnter={() => onTokenHover(entry)}
              onMouseLeave={() => onTokenHover(null)}
              onClick={() => onTokenClick(entry)}
              title={`pos ${entry.position}: ${entry.predictions[0]?.token ?? '?'} (${(entry.topProbability * 100).toFixed(1)}%)`}
            >
              {entry.token === '\n' ? '↵\n' : entry.token}
            </span>
          );
        })}
      </div>
    </div>
  );
}
