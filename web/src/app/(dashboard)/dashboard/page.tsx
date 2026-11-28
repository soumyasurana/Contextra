'use client';

import { useEffect } from 'react';
import Link from 'next/link';
import { ArrowRight, FileText, FolderArchive, Layers, MessageSquare, RefreshCw } from 'lucide-react';
import { useAppStore } from '@/lib/store';

export default function DashboardPage() {
  const {
    documents, collections, conversationsCount, apiConnected,
    documentsLoading, collectionsLoading, conversationsLoading,
    documentsError, collectionsError, conversationsError, fetchDashboardData,
  } = useAppStore();

  useEffect(() => {
    void fetchDashboardData();
  }, [fetchDashboardData]);

  const loading = documentsLoading || collectionsLoading || conversationsLoading;
  const totalChunks = documents.reduce((count, document) => count + document.chunks_count, 0);
  const metrics = [
    { label: 'Documents', value: documents.length, href: '/documents', icon: FileText, loading: documentsLoading, error: documentsError },
    { label: 'Indexed chunks', value: totalChunks, href: '/documents', icon: Layers, loading: documentsLoading, error: documentsError },
    { label: 'Collections', value: collections.length, href: '/collections', icon: FolderArchive, loading: collectionsLoading, error: collectionsError },
    { label: 'Conversations', value: conversationsCount, href: '/conversations', icon: MessageSquare, loading: conversationsLoading, error: conversationsError },
  ];

  return (
    <div className="space-y-6">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-white">Workspace overview</h1>
          <p className="mt-1 text-sm text-zinc-400">Browse the documents, collections, and conversations stored by your gateway.</p>
        </div>
        <button type="button" onClick={() => fetchDashboardData()} disabled={loading}
          className="glass-card flex items-center gap-2 rounded-xl px-3 py-2 text-sm text-zinc-300 hover:text-white disabled:opacity-50">
          <RefreshCw className={`size-4 ${loading ? 'animate-spin' : ''}`} /> Refresh
        </button>
      </header>

      {(documentsError || collectionsError || conversationsError) && (
        <p role="alert" className="rounded-xl border border-rose-500/20 bg-rose-500/10 p-4 text-sm text-rose-300">
          {apiConnected ? 'Some workspace data could not be loaded.' : 'The gateway could not be reached.'} Select Refresh to retry.
        </p>
      )}

      <div className="grid grid-cols-2 gap-3 sm:gap-4 xl:grid-cols-4">
        {metrics.map(({ label, value, href, icon: Icon, loading, error }) => (
          <Link key={label} href={href} className="glass-card glass-card-hover min-w-0 rounded-2xl p-4 sm:p-5">
            <span className="flex items-center gap-2 text-sm text-zinc-400"><Icon className="size-4 shrink-0 text-indigo-400" />{label}</span>
            <span className="mt-3 block font-mono text-2xl font-bold text-white">{loading ? '…' : error ? '—' : value.toLocaleString()}</span>
          </Link>
        ))}
      </div>

      <div className="grid gap-4 xl:grid-cols-2">
        <section className="glass-panel min-w-0 rounded-2xl p-4 sm:p-6">
          <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
            <h2 className="font-semibold text-white">Collections</h2>
            <Link href="/collections" className="flex items-center gap-1 text-xs text-indigo-300">View all <ArrowRight className="size-3" /></Link>
          </div>
          {collectionsError ? <p className="text-sm text-zinc-400">{collectionsError}</p> : collectionsLoading ? (
            <p role="status" className="text-sm text-zinc-400">Loading collections…</p>
          ) : collections.length === 0 ? (
            <p className="text-sm text-zinc-400">No collections yet.</p>
          ) : (
            <ul className="divide-y divide-white/10">
              {collections.slice(0, 5).map((collection) => (
                <li key={collection.id} className="flex items-start justify-between gap-3 py-3 text-sm">
                  <div className="min-w-0">
                    <p className="wrap-anywhere font-medium text-white">{collection.name}</p>
                    {collection.description && <p className="mt-1 wrap-anywhere text-xs text-zinc-400 line-clamp-1">{collection.description}</p>}
                  </div>
                  <span className="shrink-0 font-mono text-xs text-zinc-400">{collection.documents_count} docs</span>
                </li>
              ))}
            </ul>
          )}
        </section>

        <section className="glass-panel min-w-0 rounded-2xl p-4 sm:p-6">
          <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
            <h2 className="font-semibold text-white">Recent documents</h2>
            <Link href="/documents" className="flex items-center gap-1 text-xs text-indigo-300">View all <ArrowRight className="size-3" /></Link>
          </div>
          {documentsError ? <p className="text-sm text-zinc-400">{documentsError}</p> : documentsLoading ? (
            <p role="status" className="text-sm text-zinc-400">Loading documents…</p>
          ) : documents.length === 0 ? (
            <p className="text-sm text-zinc-400">No documents yet.</p>
          ) : (
            <ul className="divide-y divide-white/10">
              {documents.slice(-5).reverse().map((document) => (
                <li key={document.id}>
                  <Link href={`/documents?document=${encodeURIComponent(document.id)}`} className="flex items-center justify-between gap-3 py-3 text-sm text-white hover:text-indigo-300">
                    <span className="min-w-0 truncate">{document.name}</span>
                    <span className="shrink-0 text-xs text-zinc-400">{document.chunks_count} chunks</span>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
    </div>
  );
}
