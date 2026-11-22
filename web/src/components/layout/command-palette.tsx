'use client';

import { useEffect, useRef, useState } from 'react';
import { useRouter } from 'next/navigation';
import { ArrowRight, FileText, Search, X } from 'lucide-react';
import { useAppStore } from '@/lib/store';
import { navItems } from './sidebar';

export function CommandPalette() {
  const router = useRouter();
  const { commandPaletteOpen, setCommandPaletteOpen, documents, fetchDocuments } = useAppStore();
  const [query, setQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const dialogRef = useRef<HTMLDialogElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const optionsRef = useRef<HTMLDivElement>(null);
  const normalizedQuery = query.trim().toLowerCase();
  const items = [
    ...navItems.filter((item) => item.name.toLowerCase().includes(normalizedQuery)),
    ...(normalizedQuery ? documents.filter((doc) => doc.name.toLowerCase().includes(normalizedQuery)).slice(0, 5)
      .map((doc) => ({ name: doc.name, href: `/documents?document=${encodeURIComponent(doc.id)}`, icon: FileText })) : []),
  ];
  const selected = Math.min(activeIndex, items.length - 1);

  useEffect(() => {
    function toggle(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k' && !event.repeat) {
        event.preventDefault();
        setCommandPaletteOpen(!useAppStore.getState().commandPaletteOpen);
      }
    }
    window.addEventListener('keydown', toggle);
    return () => window.removeEventListener('keydown', toggle);
  }, [setCommandPaletteOpen]);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (commandPaletteOpen && dialog && !dialog.open) {
      dialog.showModal();
      inputRef.current?.focus();
      if (!useAppStore.getState().documentsLoading) void fetchDocuments();
    }
    if (!commandPaletteOpen && dialog?.open) dialog.close();
  }, [commandPaletteOpen, fetchDocuments]);

  function select(href: string) {
    setCommandPaletteOpen(false);
    router.push(href);
  }

  return (
    <dialog
      ref={dialogRef}
      aria-label="Search workspace"
      onClick={(event) => { if (event.target === event.currentTarget) setCommandPaletteOpen(false); }}
      onClose={() => {
        setCommandPaletteOpen(false);
        setQuery('');
        setActiveIndex(0);
      }}
      className="fixed inset-x-0 top-[min(18dvh,7rem)] m-auto mt-0 w-[calc(100%-2rem)] max-w-xl overflow-hidden rounded-2xl border border-white/10 bg-zinc-950 text-zinc-100 shadow-2xl backdrop:bg-black/70 backdrop:backdrop-blur-sm"
    >
      <div className="flex items-center gap-3 border-b border-white/10 px-4 py-3">
        <Search className="size-5 shrink-0 text-zinc-400" />
        <input
          ref={inputRef}
          className="min-w-0 flex-1 bg-transparent text-sm outline-none"
          placeholder="Search pages and documents…"
          role="combobox"
          aria-label="Search pages and documents"
          aria-expanded={commandPaletteOpen}
          aria-controls="workspace-search-results"
          aria-activedescendant={selected >= 0 ? `workspace-option-${selected}` : undefined}
          value={query}
          onChange={(event) => { setQuery(event.target.value); setActiveIndex(0); }}
          onKeyDown={(event) => {
            if (!items.length) return;
            if (event.key === 'Enter') {
              event.preventDefault();
              select(items[selected].href);
            }
            if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
              event.preventDefault();
              const direction = event.key === 'ArrowDown' ? 1 : -1;
              const next = (selected + direction + items.length) % items.length;
              setActiveIndex(next);
              optionsRef.current?.children[next]?.scrollIntoView({ block: 'nearest' });
            }
          }}
        />
        <button type="button" aria-label="Close search" onClick={() => setCommandPaletteOpen(false)} className="rounded-lg p-1 text-zinc-400 hover:bg-white/10"><X className="size-4" /></button>
      </div>
      <div id="workspace-search-results" ref={optionsRef} role="listbox" aria-label="Search results" className="max-h-[55dvh] overflow-y-auto p-2">
        {items.map(({ name, href, icon: Icon }, index) => (
          <button
            key={href}
            id={`workspace-option-${index}`}
            type="button"
            tabIndex={-1}
            role="option"
            aria-selected={index === selected}
            onClick={() => select(href)}
            onMouseEnter={() => setActiveIndex(index)}
            className={`flex w-full items-center gap-3 rounded-xl px-3 py-2 text-left text-sm ${index === selected ? 'bg-indigo-500/20 text-white' : 'text-zinc-300'}`}
          >
            <Icon className="size-4 shrink-0 text-indigo-400" />
            <span className="min-w-0 flex-1 truncate">{name}</span>
            <ArrowRight className="size-4 shrink-0 text-zinc-400" />
          </button>
        ))}
      </div>
      {items.length === 0 && <p role="status" className="px-5 pb-5 text-sm text-zinc-400">No matching pages or documents.</p>}
    </dialog>
  );
}
