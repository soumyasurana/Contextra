'use client';

import { useState, type FormEvent } from 'react';
import { KeyRound, Save } from 'lucide-react';
import { toast } from 'sonner';
import { DEFAULT_GATEWAY_URL } from '@/lib/api';
import { useAppStore } from '@/lib/store';

export default function SettingsPage() {
  const { settings, updateSettings } = useAppStore();
  const [url, setUrl] = useState(settings.gateway_url);
  const [key, setKey] = useState(settings.api_key);
  const [error, setError] = useState('');

  function handleSave(event: FormEvent) {
    event.preventDefault();
    const input = url.trim();
    let gatewayUrl = '';
    if (input) {
      try {
        const parsed = new URL(input);
        if (!['http:', 'https:'].includes(parsed.protocol) || parsed.username || parsed.password || parsed.search || parsed.hash) {
          throw new Error();
        }
        gatewayUrl = parsed.href.replace(/\/+$/, '');
      } catch {
        setError('Enter an HTTP or HTTPS gateway URL without credentials, a query string, or a fragment.');
        return;
      }
    }
    updateSettings({ gateway_url: gatewayUrl, api_key: key.trim() });
    setError('');
    toast.success('Connection settings saved for this session.');
  }

  return (
    <div className="max-w-2xl space-y-5">
      <header>
        <h1 className="text-2xl font-bold text-white">Settings</h1>
        <p className="mt-1 text-sm text-zinc-400">Connect this workspace to a Contextra gateway.</p>
      </header>

      <form onSubmit={handleSave} className="glass-panel space-y-5 rounded-2xl p-4 sm:p-6">
        <h2 className="flex items-center gap-2 border-b border-white/10 pb-3 font-semibold text-white">
          <KeyRound className="size-4 text-indigo-400" /> Gateway connection
        </h2>
        <label className="block text-sm font-medium text-zinc-200">
          Gateway URL
          <input type="text" inputMode="url" value={url} onChange={(event) => setUrl(event.target.value)}
            placeholder={DEFAULT_GATEWAY_URL || 'Use this site’s gateway proxy'} aria-invalid={Boolean(error)}
            className="mt-2 w-full rounded-xl border border-white/10 bg-zinc-900 px-3 py-2 font-mono text-sm" />
        </label>
        <label className="block text-sm font-medium text-zinc-200">
          API key <span className="font-normal text-zinc-400">(optional)</span>
          <input type="password" autoComplete="off" value={key} onChange={(event) => setKey(event.target.value)}
            className="mt-2 w-full rounded-xl border border-white/10 bg-zinc-900 px-3 py-2" />
        </label>
        {error && <p role="alert" className="text-sm text-rose-300">{error}</p>}
        <p className="text-xs text-zinc-400">These settings apply to this session. Model and ingestion settings are configured on the gateway.</p>
        <button type="submit" className="flex items-center gap-2 rounded-xl bg-indigo-600 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-500">
          <Save className="size-4" /> Save connection
        </button>
      </form>
    </div>
  );
}
