import type { LayerTab } from '../types';

const TABS: { id: LayerTab; label: string; layer: string; available: boolean }[] = [
  { id: 'dla', label: 'DLA Heatmap', layer: '1', available: true },
  { id: 'content', label: 'Content Projection', layer: '2', available: true },
  { id: 'cost', label: 'Infrastructure Cost', layer: '3', available: true },
  { id: 'kspace', label: 'K-Space Crowding', layer: '4', available: false },
  { id: 'injection', label: 'Injection Test', layer: '5', available: true },
];

interface Props {
  activeTab: LayerTab;
  onTabChange: (tab: LayerTab) => void;
}

export function TabBar({ activeTab, onTabChange }: Props) {
  return (
    <div className="mt-4 flex gap-1 border-b border-zinc-800">
      {TABS.map(tab => (
        <button
          key={tab.id}
          onClick={() => tab.available && onTabChange(tab.id)}
          disabled={!tab.available}
          className={`
            px-4 py-2.5 text-sm font-medium rounded-t-lg transition-colors relative
            ${activeTab === tab.id
              ? 'text-amber-400 bg-zinc-900'
              : tab.available
                ? 'text-zinc-400 hover:text-zinc-200 hover:bg-zinc-900/50'
                : 'text-zinc-600 cursor-not-allowed'
            }
          `}
        >
          <span className="text-[10px] text-zinc-600 mr-1.5">L{tab.layer}</span>
          {tab.label}
          {!tab.available && (
            <span className="ml-1.5 text-[10px] text-zinc-700">Phase 2</span>
          )}
          {activeTab === tab.id && (
            <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-amber-500" />
          )}
        </button>
      ))}
    </div>
  );
}
