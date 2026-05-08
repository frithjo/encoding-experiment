import { useRef, useEffect, useMemo } from 'react';
import * as d3 from 'd3';
import type { DLAScanResult, DLACell, SelectedCell } from '../types';
import { flattenDlaMatrix } from '../search/helpers';

interface Props {
  result: DLAScanResult | null;
  onCellClick: (cell: SelectedCell) => void;
}

export function DLAHeatmap({ result, onCellClick }: Props) {
  const svgRef = useRef<SVGSVGElement>(null);

  const cells = useMemo(() => {
    if (!result) return [];
    return flattenDlaMatrix(result);
  }, [result]);

  const nLayers = result?.num_layers_scanned ?? 0;
  const nHeads = result?.num_heads ?? 0;
  const hotCells = result?.hot_cells ?? [];

  useEffect(() => {
    if (!result || !svgRef.current || cells.length === 0) return;

    const svg = d3.select(svgRef.current);
    svg.selectAll('*').remove();

    const margin = { top: 40, right: 20, bottom: 20, left: 50 };
    const cellSize = Math.min(28, Math.max(12, 800 / nHeads));
    const width = margin.left + nHeads * cellSize + margin.right;
    const height = margin.top + nLayers * cellSize + margin.bottom;

    svg.attr('viewBox', `0 0 ${width} ${height}`);

    const maxAbs = Math.max(...cells.map(c => Math.abs(c.dla)), 0.01);
    const colorScale = d3.scaleSequential(d3.interpolateYlOrRd).domain([0, maxAbs]);

    const g = svg.append('g');

    // Column headers (heads)
    for (let h = 0; h < nHeads; h++) {
      g.append('text')
        .attr('x', margin.left + h * cellSize + cellSize / 2)
        .attr('y', margin.top - 8)
        .attr('text-anchor', 'middle')
        .attr('fill', '#71717a')
        .attr('font-size', '10px')
        .text(`H${h}`);
    }

    // Row labels (layers) — show every Nth to avoid clutter
    const labelStep = nLayers > 40 ? 5 : nLayers > 20 ? 2 : 1;
    for (let l = 0; l < nLayers; l++) {
      if (l % labelStep !== 0 && l !== nLayers - 1) continue;
      g.append('text')
        .attr('x', margin.left - 8)
        .attr('y', margin.top + l * cellSize + cellSize / 2 + 3)
        .attr('text-anchor', 'end')
        .attr('fill', '#71717a')
        .attr('font-size', '10px')
        .text(`L${l}`);
    }

    // Cells
    cells.forEach((cell: DLACell) => {
      const fill = !cell.is_decomposable
        ? '#27272a' // zinc-800 for non-decomposable
        : cell.dla > 0
          ? colorScale(cell.dla)
          : '#18181b';

      const rect = g
        .append('rect')
        .attr('class', 'heatmap-cell')
        .attr('x', margin.left + cell.head * cellSize)
        .attr('y', margin.top + cell.layer * cellSize)
        .attr('width', cellSize - 1)
        .attr('height', cellSize - 1)
        .attr('rx', 2)
        .attr('fill', fill)
        .on('click', () => onCellClick({ layer: cell.layer, head: cell.head }));

      rect.append('title').text(
        `L${cell.layer} H${cell.head}\nDLA: ${cell.dla.toFixed(2)}\n${cell.fraction_of_layer.toFixed(1)}% of layer\nTop: "${cell.top_token}"${!cell.is_decomposable ? '\n(non-decomposable)' : ''}`,
      );
    });

    // Highlight hot cell
    const top = hotCells[0];
    if (top) {
      g.append('rect')
        .attr('x', margin.left + top.head * cellSize - 1)
        .attr('y', margin.top + top.layer * cellSize - 1)
        .attr('width', cellSize + 1)
        .attr('height', cellSize + 1)
        .attr('rx', 3)
        .attr('fill', 'none')
        .attr('stroke', '#f59e0b')
        .attr('stroke-width', 2);
    }
  }, [result, cells, nLayers, nHeads, hotCells, onCellClick]);

  if (!result) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-zinc-500">
        <p className="text-lg font-medium mb-2">Layer 1: DLA Heatmap</p>
        <p className="text-sm">Where does factual retrieval happen?</p>
        <p className="text-xs mt-4 text-zinc-600">
          Enter a factual prompt and click Run to compute Direct Logit Attribution
          across all layers and heads.
        </p>
      </div>
    );
  }

  return (
    <div>
      <div className="flex items-start justify-between mb-4">
        <div>
          <h2 className="text-lg font-semibold">DLA Heatmap</h2>
          <p className="text-sm text-zinc-400 mt-1">
            Target: "<span className="text-amber-400">{result.target_token}</span>"
            {' | '}
            {nLayers} layers x {nHeads} heads = {nLayers * nHeads} slots
          </p>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-4">
        {/* Heatmap */}
        <div className="lg:col-span-3 overflow-x-auto bg-zinc-900/50 rounded-xl p-4 border border-zinc-800">
          <svg ref={svgRef} className="w-full" style={{ maxHeight: '70vh' }} />
        </div>

        {/* Hot cells panel */}
        <div className="bg-zinc-900/50 rounded-xl p-4 border border-zinc-800">
          <h3 className="text-sm font-medium text-zinc-300 mb-3">Top Copy Heads</h3>
          <div className="space-y-3">
            {hotCells.slice(0, 8).map((cell, i) => (
              <button
                key={`${cell.layer}-${cell.head}`}
                onClick={() => onCellClick({ layer: cell.layer, head: cell.head })}
                className={`w-full text-left p-2.5 rounded-lg border transition-colors ${
                  i === 0
                    ? 'bg-amber-900/20 border-amber-700/40 hover:bg-amber-900/30'
                    : 'bg-zinc-800/50 border-zinc-700/30 hover:bg-zinc-800'
                }`}
              >
                <div className="flex justify-between items-center">
                  <span className={`font-mono text-sm ${i === 0 ? 'text-amber-400' : 'text-zinc-300'}`}>
                    L{cell.layer} H{cell.head}
                  </span>
                  <span className="text-xs text-zinc-500">#{cell.abs_rank}</span>
                </div>
                <div className="mt-1 text-xs text-zinc-500">
                  DLA: {cell.dla.toFixed(2)} | {cell.fraction_of_layer.toFixed(1)}% of layer
                </div>
                <div className="mt-0.5 text-xs text-zinc-600">
                  Top: "{cell.top_token}"
                </div>
              </button>
            ))}
          </div>
          <p className="mt-3 text-xs text-zinc-600">
            Click to drill into content projection
          </p>
        </div>
      </div>

      <div className="mt-3 flex items-center gap-4 text-xs text-zinc-500">
        <span className="flex items-center gap-1.5">
          <span className="w-3 h-3 rounded bg-zinc-900 border border-zinc-700" />
          Near zero
        </span>
        <span className="flex items-center gap-1.5">
          <span className="w-3 h-3 rounded bg-amber-600" />
          High DLA
        </span>
        <span className="flex items-center gap-1.5">
          <span className="w-3 h-3 rounded bg-zinc-800 border border-zinc-600" />
          Non-decomposable
        </span>
      </div>
    </div>
  );
}
