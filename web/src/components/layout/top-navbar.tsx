'use client';

import { useEffect } from 'react';
import { Search, Server } from 'lucide-react';
import { useAppStore } from '@/lib/store';

export function TopNavbar() {
  const setCommandPaletteOpen = useAppStore((state) => state.setCommandPaletteOpen);
  const apiConnected = useAppStore((state) => state.apiConnected);
  const checkApiHealth = useAppStore((state) => state.checkApiHealth);

  useEffect(() => {
    void checkApiHealth();
    const interval = setInterval(checkApiHealth, 15000);
    return () => clearInterval(interval);
  }, [checkApiHealth]);

  return (
    <header className="glass-panel sticky top-0 z-30 flex min-h-16 items-center justify-between gap-3 border-b border-white/10 bg-[#090a0f]/90 px-3 backdrop-blur-xl sm:px-6">
      <button
        type="button"
        aria-haspopup="dialog"
        onClick={() => setCommandPaletteOpen(true)}
        className="flex min-w-0 flex-1 items-center gap-2 rounded-xl border border-white/10 bg-zinc-900 px-3 py-2 text-sm text-zinc-300 hover:border-indigo-500/50 sm:max-w-72"
      >
        <Search className="size-4 shrink-0 text-indigo-400" />
        <span className="truncate">Search workspace</span>
        <kbd className="ml-auto hidden shrink-0 rounded border border-zinc-700 px-1.5 text-xs text-zinc-400 sm:block">Ctrl / ⌘ K</kbd>
      </button>

      <span className={`flex shrink-0 items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-medium ${apiConnected ? 'border-emerald-500/20 text-emerald-400' : 'border-amber-500/20 text-amber-400'}`}>
        <Server className="size-3.5" />
        {apiConnected ? 'API online' : 'API offline'}
      </span>
    </header>
  );
}
