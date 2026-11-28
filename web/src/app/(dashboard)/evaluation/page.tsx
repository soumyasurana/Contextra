'use client';

import React from 'react';
import {
  BarChart3,
  CheckCircle2,
  Clock,
  Zap,
  Target,
  Award,
} from 'lucide-react';
import {
  BarChart as ReBarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from 'recharts';
import { useAppStore } from '@/lib/store';

export default function EvaluationPage() {
  const evals = useAppStore((state) => state.evals);

  const activeRun = evals[0];

  const comparisonData = [
    { strategy: 'Dense Vector Only', recall: 82, precision: 76, mrr: 79, latency: 45 },
    { strategy: 'BM25 Keyword Only', recall: 74, precision: 84, mrr: 78, latency: 22 },
    { strategy: 'Hybrid RRF Merge', recall: 91, precision: 87, mrr: 88, latency: 68 },
    { strategy: 'Hybrid + Cohere Rerank', recall: 96, precision: 92, mrr: 94, latency: 134 },
  ];

  return (
    <div className="space-y-8">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-white flex items-center space-x-2">
            <BarChart3 className="w-6 h-6 text-pink-400" />
            <span>Evaluation & Quality Benchmarks</span>
          </h1>
          <p className="text-sm text-zinc-400 mt-1">
            Sample benchmark results for retrieval recall, precision, mean reciprocal rank, and latency.
          </p>
        </div>

      </div>

      {/* Top Quality Metric Cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-4">
        <div className="glass-card p-5 rounded-2xl border border-white/10 space-y-2">
          <span className="text-xs text-zinc-400 font-medium flex items-center space-x-1">
            <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
            <span>Pass Rate</span>
          </span>
          <p className="text-2xl font-bold font-mono text-emerald-400">{activeRun.pass_rate}%</p>
          <p className="text-[11px] text-zinc-500">+1.2% vs baseline</p>
        </div>

        <div className="glass-card p-5 rounded-2xl border border-white/10 space-y-2">
          <span className="text-xs text-zinc-400 font-medium flex items-center space-x-1">
            <Target className="w-3.5 h-3.5 text-indigo-400" />
            <span>Recall@5</span>
          </span>
          <p className="text-2xl font-bold font-mono text-indigo-300">{(activeRun.recall_at_k * 100).toFixed(1)}%</p>
          <p className="text-[11px] text-zinc-500">Top 5 candidates</p>
        </div>

        <div className="glass-card p-5 rounded-2xl border border-white/10 space-y-2">
          <span className="text-xs text-zinc-400 font-medium flex items-center space-x-1">
            <Award className="w-3.5 h-3.5 text-purple-400" />
            <span>Precision@5</span>
          </span>
          <p className="text-2xl font-bold font-mono text-purple-300">{(activeRun.precision_at_k * 100).toFixed(1)}%</p>
          <p className="text-[11px] text-zinc-500">Relevant density</p>
        </div>

        <div className="glass-card p-5 rounded-2xl border border-white/10 space-y-2">
          <span className="text-xs text-zinc-400 font-medium flex items-center space-x-1">
            <Zap className="w-3.5 h-3.5 text-amber-400" />
            <span>MRR Score</span>
          </span>
          <p className="text-2xl font-bold font-mono text-amber-300">{activeRun.mrr.toFixed(3)}</p>
          <p className="text-[11px] text-zinc-500">Mean Reciprocal Rank</p>
        </div>

        <div className="glass-card p-5 rounded-2xl border border-white/10 space-y-2">
          <span className="text-xs text-zinc-400 font-medium flex items-center space-x-1">
            <Clock className="w-3.5 h-3.5 text-rose-400" />
            <span>p95 Latency</span>
          </span>
          <p className="text-2xl font-bold font-mono text-rose-400">{activeRun.avg_latency_ms} ms</p>
          <p className="text-[11px] text-zinc-500">Per evaluation query</p>
        </div>
      </div>

      {/* Comparison Chart */}
      <div className="glass-panel rounded-2xl p-6 space-y-4">
        <div>
          <h2 className="text-base font-semibold text-white">Retrieval Strategy Performance Comparison</h2>
          <p className="text-xs text-zinc-400">Benchmarking Recall@5, Precision@5, and MRR (%) across pipeline configurations</p>
        </div>

        <div className="h-72 w-full pt-4">
          <ResponsiveContainer width="100%" height="100%">
            <ReBarChart data={comparisonData} margin={{ top: 10, right: 30, left: 0, bottom: 0 }}>
              <CartesianGrid strokeDasharray="3 3" stroke="#27272a" vertical={false} />
              <XAxis dataKey="strategy" stroke="#71717a" fontSize={11} tickLine={false} />
              <YAxis stroke="#71717a" fontSize={11} tickLine={false} domain={[0, 100]} />
              <Tooltip
                contentStyle={{ backgroundColor: '#12141d', border: '1px solid rgba(255,255,255,0.1)', borderRadius: '12px' }}
              />
              <Legend wrapperStyle={{ fontSize: '12px', paddingTop: '10px' }} />
              <Bar dataKey="recall" name="Recall@5 (%)" fill="#6366f1" radius={[4, 4, 0, 0]} />
              <Bar dataKey="precision" name="Precision@5 (%)" fill="#a855f7" radius={[4, 4, 0, 0]} />
              <Bar dataKey="mrr" name="MRR (%)" fill="#ec4899" radius={[4, 4, 0, 0]} />
            </ReBarChart>
          </ResponsiveContainer>
        </div>
      </div>

      {/* Evaluation Runs History Table */}
      <div className="glass-panel rounded-2xl overflow-hidden border border-white/10 shadow-2xl">
        <div className="p-4 border-b border-white/10 font-bold text-sm text-white">
          Historical Benchmark Runs
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-left text-sm text-zinc-300">
          <thead className="bg-zinc-950/80 text-xs uppercase font-semibold text-zinc-400 border-b border-white/10">
            <tr>
              <th className="px-6 py-4">Run Name</th>
              <th className="px-6 py-4">Dataset</th>
              <th className="px-6 py-4">Samples</th>
              <th className="px-6 py-4">Pass Rate</th>
              <th className="px-6 py-4">Recall@5</th>
              <th className="px-6 py-4">MRR</th>
              <th className="px-6 py-4 text-right">Run Time</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-white/5 font-mono text-xs">
            {evals.map((run) => (
              <tr key={run.id} className="hover:bg-white/[0.02] transition-colors">
                <td className="px-6 py-4 font-sans font-medium text-white">{run.name}</td>
                <td className="px-6 py-4 text-zinc-400 font-sans">{run.dataset_name}</td>
                <td className="px-6 py-4 text-zinc-300">{run.sample_count}</td>
                <td className="px-6 py-4 text-emerald-400 font-bold">{run.pass_rate}%</td>
                <td className="px-6 py-4 text-indigo-300">{(run.recall_at_k * 100).toFixed(1)}%</td>
                <td className="px-6 py-4 text-amber-300">{run.mrr.toFixed(3)}</td>
                <td className="px-6 py-4 text-right text-zinc-400 font-sans">{run.run_at}</td>
              </tr>
            ))}
          </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
