'use client';

import React, { Suspense, useEffect, useState } from 'react';
import { useSearchParams, useRouter } from 'next/navigation';
import { motion, AnimatePresence } from 'framer-motion';
import {
  FileText,
  UploadCloud,
  Search,
  Eye,
  X,
  CheckCircle2,
  FileCode,
  RefreshCw,
} from 'lucide-react';
import { useAppStore } from '@/lib/store';
import { DocumentResource } from '@/types';
import { api } from '@/lib/api';
import { toast } from 'sonner';

export default function DocumentsPage() {
  return <Suspense fallback={<p role="status" className="text-sm text-zinc-400">Loading documents…</p>}><DocumentRegistry /></Suspense>;
}

function DocumentRegistry() {
  const {
    documents,
    documentsLoading,
    collections,
    settings,
    fetchDocuments,
    fetchCollections,
    documentsError,
    collectionsError,
  } = useAppStore();

  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCollection, setSelectedCollection] = useState<string>('all');
  const router = useRouter();
  const searchParams = useSearchParams();
  const selectedDocId = searchParams.get('document');
  const selectedDoc = documents.find((document) => document.id === selectedDocId);
  const setSelectedDoc = (document: DocumentResource | null) => {
    router.replace(document ? `/documents?document=${encodeURIComponent(document.id)}` : '/documents', { scroll: false });
  };
  const [uploadModalOpen, setUploadModalOpen] = useState(false);

  // Upload state
  const [name, setName] = useState('');
  const [content, setContent] = useState('');
  const [targetCollection, setTargetCollection] = useState('');
  const collectionNames = new Map(collections.map((collection) => [collection.id, collection.name]));
  const [isSubmitting, setIsSubmitting] = useState(false);

  const closeUploadModal = () => {
    setUploadModalOpen(false);
    setName('');
    setContent('');
    setTargetCollection('');
  };

  useEffect(() => {
    fetchDocuments();
    fetchCollections();
  }, [fetchDocuments, fetchCollections]);

  const filteredDocs = documents.filter((doc) => {
    const matchesSearch = doc.name.toLowerCase().includes(searchQuery.toLowerCase());
    const matchesCollection = selectedCollection === 'all' || doc.collection_id === selectedCollection;
    return matchesSearch && matchesCollection;
  });

  const handleIngest = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !content.trim() || isSubmitting) return;

    if (new TextEncoder().encode(content).length > 250_000) {
      toast.error('Document text must be 250 KB or smaller.');
      return;
    }
    setIsSubmitting(true);
    try {
      const document = await api.createDocument(
        { name: name.trim(), content, collection_id: targetCollection || undefined },
        settings.api_key,
        settings.gateway_url,
      );
      setName('');
      setContent('');
      setTargetCollection('');
      setUploadModalOpen(false);
      toast.success(`Ingested ${document.name}`);
      await fetchDocuments();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : 'Failed to ingest document.');
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="space-y-6">
      {/* Header & Primary CTA */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-white flex items-center space-x-2">
            <FileText className="w-6 h-6 text-indigo-400" />
            <span>Documents</span>
          </h1>
          <p className="text-sm text-zinc-400 mt-1">
            Ingest text or Markdown and browse documents by collection.
          </p>
        </div>

        <div className="flex items-center space-x-3">
          <button
            onClick={() => { void fetchDocuments(); void fetchCollections(); }}
            disabled={documentsLoading}
            className="px-3.5 py-2.5 rounded-xl glass-card text-xs text-zinc-300 hover:text-white flex items-center space-x-2 transition-colors"
          >
            <RefreshCw className={`w-4 h-4 ${documentsLoading ? 'animate-spin' : ''}`} />
            <span>Sync</span>
          </button>

          <button
            onClick={() => setUploadModalOpen(true)}
            className="px-4 py-2.5 rounded-xl bg-gradient-to-r from-indigo-600 to-purple-600 hover:from-indigo-500 hover:to-purple-500 text-white font-medium text-sm flex items-center justify-center space-x-2 shadow-lg shadow-indigo-500/25 transition-all hover:scale-[1.02]"
          >
            <UploadCloud className="w-4 h-4" />
            <span>Ingest Document</span>
          </button>
        </div>
      </div>

      {/* Filter Bar */}
      <div className="glass-panel p-4 rounded-2xl flex flex-col md:flex-row items-center justify-between gap-4">
        {/* Search Bar */}
        <div className="relative w-full md:w-80">
          <Search className="w-4 h-4 text-zinc-400 absolute left-3 top-3" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            aria-label="Search documents by name"
            placeholder="Search documents by name..."
            className="w-full pl-9 pr-4 py-2 bg-zinc-900/80 border border-white/10 rounded-xl text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:border-indigo-500/50"
          />
        </div>

        {/* Filters */}
        <div className="flex items-center space-x-3 w-full md:w-auto">
          {/* Collection Filter */}
          <select
            aria-label="Filter by collection"
            value={selectedCollection}
            onChange={(e) => setSelectedCollection(e.target.value)}
            className="px-3 py-2 bg-zinc-900/80 border border-white/10 rounded-xl text-xs text-zinc-300 focus:outline-none focus:border-indigo-500/50 cursor-pointer"
          >
            <option value="all">All Collections</option>
            {collections.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        </div>
      </div>

      {/* Document Data Table */}
      <div className="glass-panel rounded-2xl overflow-hidden border border-white/10 shadow-2xl">
        {documentsError ? (
          <p role="alert" className="px-6 py-8 text-sm text-rose-300">{documentsError} Select Sync to retry.</p>
        ) : documentsLoading ? (
          <div className="p-12 text-center text-zinc-400 text-sm flex items-center justify-center space-x-2">
            <RefreshCw className="w-4 h-4 animate-spin text-indigo-400" />
            <span>Loading documents from backend...</span>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm text-zinc-300">
              <thead className="bg-zinc-950/80 text-xs uppercase font-semibold text-zinc-400 border-b border-white/10">
                <tr>
                  <th className="px-6 py-4">Document Name</th>
                  <th className="px-6 py-4">Collection</th>
                  <th className="px-6 py-4">Status</th>
                  <th className="px-6 py-4">Chunks</th>
                  <th className="px-6 py-4">File Size</th>
                  <th className="px-6 py-4 text-right">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-white/5">
                {filteredDocs.map((doc) => (
                  <tr key={doc.id} className="hover:bg-white/[0.02] transition-colors group">
                    <td className="px-6 py-4 font-medium text-white">
                      <div className="flex items-center space-x-3">
                        <div className="w-8 h-8 rounded-lg bg-indigo-500/10 border border-indigo-500/20 flex items-center justify-center text-indigo-400 shrink-0">
                          <FileCode className="w-4 h-4" />
                        </div>
                        <span className="truncate max-w-xs">{doc.name}</span>
                      </div>
                    </td>
                    <td className="px-6 py-4 font-mono text-xs text-zinc-400">
                      {collectionNames.get(doc.collection_id) || doc.collection_name || "Unassigned"}
                    </td>
                    <td className="px-6 py-4">
                      <span className="inline-flex items-center space-x-1.5 px-2.5 py-1 rounded-full text-xs font-semibold bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                        <CheckCircle2 className="w-3 h-3" />
                        <span>Ingested</span>
                      </span>
                    </td>
                    <td className="px-6 py-4 font-mono text-zinc-200">{doc.chunks_count}</td>
                    <td className="px-6 py-4 font-mono text-xs text-zinc-400">{doc.file_size}</td>
                    <td className="px-6 py-4 text-right space-x-2">
                      <button
                        onClick={() => setSelectedDoc(doc)}
                        className="p-1.5 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-300 hover:text-white transition-colors"
                        title="Inspect Metadata & Content"
                      >
                        <Eye className="w-4 h-4" />
                      </button>
                    </td>
                  </tr>
                ))}
                {filteredDocs.length === 0 && (
                  <tr>
                    <td colSpan={6} className="px-6 py-12 text-center text-zinc-500 text-sm">
                      {searchQuery || selectedCollection !== "all" ? "No documents match the current filters." : "No documents yet. Ingest a document to get started."}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Ingest Modal */}
      <AnimatePresence>
        {uploadModalOpen && (
          <div
            className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-md"
            onClick={(e) => { if (e.target === e.currentTarget && !isSubmitting) closeUploadModal(); }}
            onKeyDown={(e) => { if (e.key === 'Escape' && !isSubmitting) closeUploadModal(); }}
          >
            <motion.div
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.95 }}
              className="w-full max-w-lg glass-panel rounded-2xl p-6 border border-white/10 space-y-6 shadow-2xl relative"
            >
              <div className="flex items-center justify-between border-b border-white/10 pb-4">
                <h3 className="text-lg font-bold text-white flex items-center space-x-2">
                  <UploadCloud className="w-5 h-5 text-indigo-400" />
                  <span>Ingest Document via Gateway</span>
                </h3>
                <button
                  type="button"
                  aria-label="Close modal"
                  disabled={isSubmitting}
                  onClick={closeUploadModal}
                  className="p-1 rounded-lg text-zinc-400 hover:text-white disabled:opacity-50"
                >
                  <X className="w-5 h-5" />
                </button>
              </div>

              <form onSubmit={handleIngest} className="space-y-4">
                <label className="block text-xs font-semibold text-zinc-300">
                  Text or Markdown file
                  <input
                    type="file"
                    accept=".txt,.md,.markdown,text/plain,text/markdown"
                    disabled={isSubmitting}
                    className="mt-1 block w-full text-zinc-400"
                    onChange={async (event) => {
                      const file = event.target.files?.[0];
                      if (!file) return;
                      if (file.size > 250_000) {
                        toast.error('Select a text file smaller than 250 KB.');
                        return;
                      }
                      try {
                        setContent(new TextDecoder('utf-8', { fatal: true }).decode(await file.arrayBuffer()));
                        setName(file.name);
                      } catch {
                        toast.error('Unable to read the file as UTF-8 text.');
                      }
                    }}
                  />
                </label>
                <label className="block text-xs font-semibold text-zinc-300">
                  Document name
                  <input required maxLength={255} value={name} onChange={(event) => setName(event.target.value)}
                    className="mt-1 w-full rounded-xl border border-white/10 bg-zinc-900 px-3 py-2" />
                </label>
                <label className="block text-xs font-semibold text-zinc-300">
                  Document content
                  <textarea required rows={5} value={content} onChange={(event) => setContent(event.target.value)}
                    placeholder="Paste text or choose a file above"
                    className="mt-1 w-full rounded-xl border border-white/10 bg-zinc-900 px-3 py-2 font-mono" />
                </label>
                <label className="block text-xs font-semibold text-zinc-300">
                  Collection
                  <select value={targetCollection} onChange={(event) => setTargetCollection(event.target.value)}
                    className="mt-1 w-full rounded-xl border border-white/10 bg-zinc-900 px-3 py-2">
                    <option value="">Unassigned</option>
                    {collections.map((collection) => <option key={collection.id} value={collection.id}>{collection.name}</option>)}
                  </select>
                  {collectionsError && <span className="mt-1 block text-rose-300">Could not load collections.</span>}
                </label>

                <div className="flex items-center justify-end space-x-3 pt-4 border-t border-white/10">
                  <button
                    type="button"
                    disabled={isSubmitting}
                    onClick={closeUploadModal}
                    className="px-4 py-2 bg-zinc-800 text-zinc-300 hover:text-white rounded-xl text-xs font-medium disabled:opacity-50"
                  >
                    Cancel
                  </button>
                  <button
                    type="submit"
                    disabled={isSubmitting || !name.trim() || !content.trim()}
                    className="disabled:opacity-50 px-4 py-2 bg-gradient-to-r from-indigo-600 to-purple-600 hover:from-indigo-500 hover:to-purple-500 text-white rounded-xl text-xs font-semibold shadow-md flex items-center space-x-2"
                  >
                    {isSubmitting && <RefreshCw className="w-3.5 h-3.5 animate-spin" />}
                    <span>Ingest Document</span>
                  </button>
                </div>
              </form>
            </motion.div>
          </div>
        )}
      </AnimatePresence>

      {/* Metadata Drawer */}
      <AnimatePresence>
        {selectedDoc && (
          <div
            className="fixed inset-0 z-50 flex justify-end bg-black/60 backdrop-blur-sm"
            onClick={(e) => { if (e.target === e.currentTarget) setSelectedDoc(null); }}
            onKeyDown={(e) => { if (e.key === 'Escape') setSelectedDoc(null); }}
          >
            <motion.div
              initial={{ x: '100%' }}
              animate={{ x: 0 }}
              exit={{ x: '100%' }}
              transition={{ type: 'spring', damping: 25, stiffness: 200 }}
              className="w-full max-w-md h-full bg-[#0c0e17] border-l border-white/10 p-6 flex flex-col justify-between overflow-y-auto"
            >
              <div className="space-y-6">
                <div className="flex items-center justify-between border-b border-white/10 pb-4">
                  <div className="flex items-center space-x-2 min-w-0">
                    <FileCode className="w-5 h-5 text-indigo-400 shrink-0" />
                    <h3 className="text-base font-bold text-white truncate">{selectedDoc.name}</h3>
                  </div>
                  <button
                    type="button"
                    aria-label="Close drawer"
                    onClick={() => setSelectedDoc(null)}
                    className="p-1 rounded-lg text-zinc-400 hover:text-white shrink-0"
                  >
                    <X className="w-5 h-5" />
                  </button>
                </div>

                <div className="space-y-3 text-xs">
                  <div className="p-3 rounded-xl bg-zinc-900 border border-white/5 space-y-2">
                    <div className="flex justify-between gap-2 text-zinc-400">
                      <span>Document ID</span>
                      <span className="font-mono text-zinc-200 truncate max-w-[200px]">{selectedDoc.id}</span>
                    </div>
                    <div className="flex justify-between gap-2 text-zinc-400">
                      <span>Collection</span>
                      <span className="text-indigo-400 font-mono truncate max-w-[200px]">
                        {collectionNames.get(selectedDoc.collection_id) || selectedDoc.collection_id || 'Unassigned'}
                      </span>
                    </div>
                    <div className="flex justify-between gap-2 text-zinc-400">
                      <span>Chunks</span>
                      <span className="font-mono text-zinc-200">{selectedDoc.chunks_count}</span>
                    </div>
                    <div className="flex justify-between gap-2 text-zinc-400">
                      <span>Size</span>
                      <span className="font-mono text-zinc-200">{selectedDoc.file_size}</span>
                    </div>
                  </div>

                  <div>
                    <h4 className="font-semibold text-zinc-300 mb-2">Metadata Map</h4>
                    <pre className="p-3 rounded-xl bg-zinc-950 border border-white/10 text-emerald-400 font-mono text-[11px] overflow-x-auto">
                      {JSON.stringify(selectedDoc.metadata, null, 2)}
                    </pre>
                  </div>

                  {selectedDoc.raw_content && (
                    <div>
                      <h4 className="font-semibold text-zinc-300 mb-2">Content Preview</h4>
                      <div className="p-3 rounded-xl bg-zinc-950 border border-white/10 text-zinc-300 font-mono text-[11px] max-h-48 overflow-y-auto whitespace-pre-wrap">
                        {selectedDoc.raw_content}
                      </div>
                    </div>
                  )}
                </div>
              </div>

              <div className="pt-4 border-t border-white/10">
                <button
                  onClick={() => setSelectedDoc(null)}
                  className="w-full py-2 bg-zinc-800 hover:bg-zinc-700 text-white rounded-xl text-xs font-medium"
                >
                  Close
                </button>
              </div>
            </motion.div>
          </div>
        )}
      </AnimatePresence>
    </div>
  );
}
