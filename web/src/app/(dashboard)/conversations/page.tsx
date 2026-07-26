'use client';

import React, { useState } from 'react';
import { useRouter } from 'next/navigation';
import { motion } from 'framer-motion';
import {
  History,
  MessageSquare,
  HardDrive,
  Cpu,
  Clock,
  ArrowRight,
  Search,
  BrainCircuit,
  Sparkles,
  Layers,
} from 'lucide-react';
import { useAppStore } from '@/lib/store';
import { ConversationResource } from '@/types';

export default function ConversationsPage() {
  const router = useRouter();
  const { conversations, setActiveConversationId } = useAppStore();
  const [searchQuery, setSearchQuery] = useState('');

  const filtered = conversations.filter(
    (c) =>
      c.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
      c.summary.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const handleOpenConversation = (convId: string) => {
    setActiveConversationId(convId);
    router.push('/chat');
  };

  return (
    <div className="space-y-6">
      {/* Page Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-white flex items-center space-x-2">
            <History className="w-6 h-6 text-teal-400" />
            <span>Conversation Memory History</span>
          </h1>
          <p className="text-sm text-zinc-400 mt-1">
            Timeline of stored conversation threads, dynamic importance summaries, and token memory footprints.
          </p>
        </div>
      </div>

      {/* Search Filter */}
      <div className="glass-panel p-4 rounded-2xl">
        <div className="relative w-full max-w-md">
          <Search className="w-4 h-4 text-zinc-400 absolute left-3 top-3" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search conversation memory..."
            className="w-full pl-9 pr-4 py-2 bg-zinc-900/80 border border-white/10 rounded-xl text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:border-teal-500/50"
          />
        </div>
      </div>

      {/* Timeline UI List */}
      <div className="relative border-l-2 border-white/10 ml-4 pl-6 space-y-6">
        {filtered.map((conv, idx) => (
          <motion.div
            key={conv.id}
            initial={{ opacity: 0, x: -15 }}
            animate={{ opacity: 1, x: 0 }}
            transition={{ duration: 0.2, delay: idx * 0.05 }}
            className="relative group"
          >
            {/* Timeline Dot */}
            <div className="absolute -left-[31px] top-1.5 w-4 h-4 rounded-full bg-zinc-900 border-2 border-teal-500 group-hover:scale-125 group-hover:bg-teal-500 transition-all shadow-md shadow-teal-500/20" />

            <div className="glass-card glass-card-hover rounded-2xl p-6 space-y-4">
              <div className="flex flex-col md:flex-row md:items-center justify-between gap-2 border-b border-white/5 pb-3">
                <div className="flex items-center space-x-3">
                  <div className="w-8 h-8 rounded-xl bg-teal-500/10 border border-teal-500/20 text-teal-400 flex items-center justify-center font-mono font-bold text-xs">
                    {conv.message_count}
                  </div>
                  <div>
                    <h3 className="text-base font-bold text-white group-hover:text-teal-300 transition-colors">
                      {conv.title}
                    </h3>
                    <p className="text-[11px] text-zinc-500 font-mono">ID: {conv.id}</p>
                  </div>
                </div>

                <div className="flex items-center space-x-4 text-xs font-mono text-zinc-400">
                  <div className="flex items-center space-x-1">
                    <Clock className="w-3.5 h-3.5 text-zinc-500" />
                    <span>{conv.last_updated}</span>
                  </div>

                  <button
                    onClick={() => handleOpenConversation(conv.id)}
                    className="px-3 py-1.5 rounded-xl bg-teal-500/10 hover:bg-teal-500/20 text-teal-300 border border-teal-500/30 flex items-center space-x-1.5 transition-all text-xs font-semibold"
                  >
                    <span>Resume Session</span>
                    <ArrowRight className="w-3.5 h-3.5" />
                  </button>
                </div>
              </div>

              {/* Memory Summary Card */}
              <div className="p-4 rounded-xl bg-zinc-950/80 border border-white/5 space-y-2">
                <div className="flex items-center space-x-2 text-xs font-semibold text-teal-400">
                  <BrainCircuit className="w-4 h-4" />
                  <span>Rolling Importance Summary:</span>
                </div>
                <p className="text-xs text-zinc-300 leading-relaxed font-sans">{conv.summary}</p>
              </div>

              {/* Stats Footer */}
              <div className="grid grid-cols-2 sm:grid-cols-4 gap-4 text-xs text-zinc-400 font-mono pt-1">
                <div>
                  <span className="text-[10px] text-zinc-500 uppercase font-sans">Messages:</span>{' '}
                  <span className="text-zinc-200 font-semibold">{conv.message_count} msgs</span>
                </div>
                <div>
                  <span className="text-[10px] text-zinc-500 uppercase font-sans">Memory Size:</span>{' '}
                  <span className="text-teal-400 font-semibold">{conv.memory_size_kb} KB</span>
                </div>
                <div>
                  <span className="text-[10px] text-zinc-500 uppercase font-sans">Token Budget:</span>{' '}
                  <span className="text-indigo-400 font-semibold">{conv.token_count} tokens</span>
                </div>
                <div>
                  <span className="text-[10px] text-zinc-500 uppercase font-sans">Status:</span>{' '}
                  <span className="text-emerald-400 font-semibold">Active Memory</span>
                </div>
              </div>
            </div>
          </motion.div>
        ))}
      </div>
    </div>
  );
}
