'use client';

import React from 'react';
import { motion } from 'framer-motion';
import {
  FileText,
  Layers,
  FolderArchive,
  MessageSquare,
  Code2,
  Cpu,
  Activity,
  Clock,
  ArrowUpRight,
  TrendingUp,
  Server,
  Database,
  Zap,
  CheckCircle2,
  Sparkles,
} from 'lucide-react';
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  BarChart,
  Bar,
  PieChart,
  Pie,
  Cell,
} from 'recharts';
import {
  MOCK_METRICS,
  MOCK_REQUESTS_OVER_TIME,
  MOCK_PROVIDER_USAGE,
  MOCK_LATENCY_BREAKDOWN,
  MOCK_ACTIVITIES,
  MOCK_SYSTEM_STATUS,
} from '@/lib/mock-data';

export default function DashboardPage() {
  const metricCards = [
    { label: 'Total Documents', value: MOCK_METRICS.total_documents, icon: FileText, change: '+12%', color: 'from-blue-500 to-indigo-600' },
    { label: 'Ingested Chunks', value: MOCK_METRICS.total_chunks.toLocaleString(), icon: Layers, change: '+8.4%', color: 'from-indigo-500 to-purple-600' },
    { label: 'Collections', value: MOCK_METRICS.total_collections, icon: FolderArchive, change: 'Stable', color: 'from-purple-500 to-pink-600' },
    { label: 'Conversations', value: MOCK_METRICS.total_conversations.toLocaleString(), icon: MessageSquare, change: '+24%', color: 'from-emerald-500 to-teal-600' },
    { label: 'Prompt Templates', value: MOCK_METRICS.total_prompts, icon: Code2, change: '+2 new', color: 'from-amber-500 to-orange-600' },
    { label: 'Vector Embeddings', value: MOCK_METRICS.total_embeddings.toLocaleString(), icon: Cpu, change: '+18.4k', color: 'from-cyan-500 to-blue-600' },
    { label: 'Total Requests', value: MOCK_METRICS.total_requests.toLocaleString(), icon: Activity, change: '+32.1%', color: 'from-pink-500 to-rose-600' },
    { label: 'Avg Latency', value: `${MOCK_METRICS.avg_latency_ms} ms`, icon: Clock, change: '-14ms', color: 'from-emerald-400 to-green-600' },
  ];

  return (
    <div className="space-y-8">
      {/* Page Header */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-white flex items-center space-x-3">
            <span>Platform Overview</span>
            <span className="px-2.5 py-0.5 text-xs font-semibold bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 rounded-full flex items-center space-x-1">
              <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
              <span>Rust Engine Active</span>
            </span>
          </h1>
          <p className="text-sm text-zinc-400 mt-1">
            Real-time context engineering telemetry, vector indexing, and pipeline performance metrics.
          </p>
        </div>

        <div className="flex items-center space-x-3">
          <div className="px-3 py-1.5 rounded-xl glass-card text-xs text-zinc-300 flex items-center space-x-2">
            <Zap className="w-3.5 h-3.5 text-amber-400" />
            <span>Qdrant HNSW: <strong className="text-white font-mono">18,450 vectors</strong></span>
          </div>
        </div>
      </div>

      {/* Metrics Grid */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {metricCards.map((card, idx) => {
          const Icon = card.icon;
          return (
            <motion.div
              key={card.label}
              initial={{ opacity: 0, y: 15 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.25, delay: idx * 0.04 }}
              className="glass-card glass-card-hover rounded-2xl p-5 relative overflow-hidden"
            >
              <div className="flex items-center justify-between">
                <span className="text-xs font-medium text-zinc-400">{card.label}</span>
                <div className={`w-8 h-8 rounded-xl bg-gradient-to-tr ${card.color} flex items-center justify-center text-white shadow-md`}>
                  <Icon className="w-4 h-4" />
                </div>
              </div>

              <div className="mt-3 flex items-baseline justify-between">
                <span className="text-2xl font-bold tracking-tight text-white font-mono">{card.value}</span>
                <span className="text-xs font-semibold text-emerald-400 flex items-center space-x-0.5">
                  <TrendingUp className="w-3 h-3" />
                  <span>{card.change}</span>
                </span>
              </div>
            </motion.div>
          );
        })}
      </div>

      {/* Main Charts Section */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Requests & Latency Area Chart (Spans 2 columns) */}
        <div className="lg:col-span-2 glass-panel rounded-2xl p-6 space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <h2 className="text-base font-semibold text-white">Requests & Latency Over Time</h2>
              <p className="text-xs text-zinc-400">24-hour API request volume and average response time (ms)</p>
            </div>
            <div className="flex items-center space-x-4 text-xs">
              <div className="flex items-center space-x-1.5">
                <div className="w-2.5 h-2.5 rounded-full bg-indigo-500" />
                <span className="text-zinc-400">Requests</span>
              </div>
              <div className="flex items-center space-x-1.5">
                <div className="w-2.5 h-2.5 rounded-full bg-emerald-400" />
                <span className="text-zinc-400">Latency (ms)</span>
              </div>
            </div>
          </div>

          <div className="h-72 w-full pt-4">
            <ResponsiveContainer width="100%" height="100%">
              <AreaChart data={MOCK_REQUESTS_OVER_TIME} margin={{ top: 10, right: 10, left: -20, bottom: 0 }}>
                <defs>
                  <linearGradient id="colorRequests" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="#6366f1" stopOpacity={0.4} />
                    <stop offset="95%" stopColor="#6366f1" stopOpacity={0.0} />
                  </linearGradient>
                  <linearGradient id="colorLatency" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor="#10b981" stopOpacity={0.3} />
                    <stop offset="95%" stopColor="#10b981" stopOpacity={0.0} />
                  </linearGradient>
                </defs>
                <CartesianGrid strokeDasharray="3 3" stroke="#27272a" vertical={false} />
                <XAxis dataKey="time" stroke="#71717a" fontSize={11} tickLine={false} />
                <YAxis stroke="#71717a" fontSize={11} tickLine={false} />
                <Tooltip
                  contentStyle={{ backgroundColor: '#12141d', border: '1px solid rgba(255,255,255,0.1)', borderRadius: '12px' }}
                  labelStyle={{ color: '#f4f4f5', fontWeight: 600, fontSize: '12px' }}
                />
                <Area type="monotone" dataKey="requests" stroke="#6366f1" strokeWidth={2} fillOpacity={1} fill="url(#colorRequests)" />
                <Area type="monotone" dataKey="latency" stroke="#10b981" strokeWidth={2} fillOpacity={1} fill="url(#colorLatency)" />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        </div>

        {/* Provider Usage Distribution Pie Chart */}
        <div className="glass-panel rounded-2xl p-6 space-y-4 flex flex-col justify-between">
          <div>
            <h2 className="text-base font-semibold text-white">LLM Provider Usage</h2>
            <p className="text-xs text-zinc-400">Distribution of chat completions across model providers</p>
          </div>

          <div className="h-52 w-full flex items-center justify-center">
            <ResponsiveContainer width="100%" height="100%">
              <PieChart>
                <Pie data={MOCK_PROVIDER_USAGE} cx="50%" cy="50%" innerRadius={55} outerRadius={80} paddingAngle={4} dataKey="value">
                  {MOCK_PROVIDER_USAGE.map((entry, index) => (
                    <Cell key={`cell-${index}`} fill={entry.color} stroke="#090a0f" strokeWidth={2} />
                  ))}
                </Pie>
                <Tooltip
                  contentStyle={{ backgroundColor: '#12141d', border: '1px solid rgba(255,255,255,0.1)', borderRadius: '12px' }}
                  itemStyle={{ color: '#f4f4f5', fontSize: '12px' }}
                />
              </PieChart>
            </ResponsiveContainer>
          </div>

          <div className="grid grid-cols-2 gap-2 pt-2 border-t border-white/5 text-xs">
            {MOCK_PROVIDER_USAGE.map((p) => (
              <div key={p.name} className="flex items-center space-x-2 truncate">
                <span className="w-2.5 h-2.5 rounded-full shrink-0" style={{ backgroundColor: p.color }} />
                <span className="text-zinc-300 truncate">{p.name}</span>
                <span className="text-zinc-500 font-mono text-[11px]">{p.value}%</span>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Second Row: Latency Breakdown & Status + Activity */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Latency Pipeline Breakdown (2 columns) */}
        <div className="lg:col-span-2 glass-panel rounded-2xl p-6 space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <h2 className="text-base font-semibold text-white">Retrieval Pipeline Latency (p95 ms)</h2>
              <p className="text-xs text-zinc-400">Time spent in each sub-system during a hybrid context retrieval request</p>
            </div>
            <span className="text-xs font-mono text-indigo-400 bg-indigo-500/10 px-2.5 py-1 rounded-full border border-indigo-500/20">
              Total p95: 228 ms
            </span>
          </div>

          <div className="h-60 w-full">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={MOCK_LATENCY_BREAKDOWN} layout="vertical" margin={{ top: 5, right: 20, left: 80, bottom: 5 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="#27272a" horizontal={false} />
                <XAxis type="number" stroke="#71717a" fontSize={11} tickLine={false} />
                <YAxis dataKey="stage" type="category" stroke="#a1a1aa" fontSize={11} tickLine={false} width={130} />
                <Tooltip
                  contentStyle={{ backgroundColor: '#12141d', border: '1px solid rgba(255,255,255,0.1)', borderRadius: '12px' }}
                />
                <Bar dataKey="p95" fill="#6366f1" radius={[0, 6, 6, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </div>
        </div>

        {/* System Health & Status */}
        <div className="glass-panel rounded-2xl p-6 space-y-4">
          <div className="flex items-center justify-between border-b border-white/10 pb-3">
            <h2 className="text-base font-semibold text-white flex items-center space-x-2">
              <Server className="w-4 h-4 text-emerald-400" />
              <span>System Status</span>
            </h2>
            <span className="text-xs font-semibold text-emerald-400">100% Healthy</span>
          </div>

          <div className="space-y-3">
            {MOCK_SYSTEM_STATUS.map((item) => (
              <div key={item.name} className="flex items-center justify-between p-2.5 rounded-xl bg-zinc-900/60 border border-white/5 text-xs">
                <div className="flex items-center space-x-2.5">
                  <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
                  <div>
                    <p className="text-zinc-200 font-medium">{item.name}</p>
                    <p className="text-[10px] text-zinc-500 font-mono">{item.service}</p>
                  </div>
                </div>
                <div className="text-right font-mono">
                  <p className="text-emerald-400 font-medium">{item.latency_ms} ms</p>
                  <p className="text-[10px] text-zinc-500">{item.uptime}</p>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Recent Activity Stream */}
      <div className="glass-panel rounded-2xl p-6 space-y-4">
        <h2 className="text-base font-semibold text-white flex items-center space-x-2">
          <Activity className="w-4 h-4 text-indigo-400" />
          <span>Recent Activity Stream</span>
        </h2>

        <div className="divide-y divide-white/5">
          {MOCK_ACTIVITIES.map((act) => (
            <div key={act.id} className="py-3 flex items-center justify-between text-xs hover:bg-white/[0.02] px-2 rounded-xl transition-colors">
              <div className="flex items-center space-x-3">
                <div className={`w-2 h-2 rounded-full ${act.status === 'success' ? 'bg-emerald-400' : act.status === 'error' ? 'bg-rose-500' : 'bg-indigo-400'}`} />
                <div>
                  <p className="text-zinc-200 font-medium">{act.title}</p>
                  <p className="text-zinc-400 text-[11px]">{act.description}</p>
                </div>
              </div>
              <span className="text-zinc-500 font-mono text-[11px]">{act.timestamp}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
