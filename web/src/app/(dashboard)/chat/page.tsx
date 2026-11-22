'use client';

import { useEffect, useRef, useState, type FormEvent } from 'react';
import Link from 'next/link';
import { Bot, Plus, Send } from 'lucide-react';
import { toast } from 'sonner';
import { api } from '@/lib/api';
import { useAppStore } from '@/lib/store';

export default function ChatPage() {
  const {
    messages, messagesLoading, messagesError, settings, activeConversationId,
    setActiveConversationId, fetchMessages, chatPending, setChatPending,
  } = useAppStore();
  const [input, setInput] = useState('');
  const [pendingMessage, setPendingMessage] = useState<{ conversationId: string; text: string } | null>(null);
  const [sendError, setSendError] = useState('');
  const messagesRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (activeConversationId) void fetchMessages(activeConversationId);
  }, [activeConversationId, fetchMessages]);

  useEffect(() => {
    const container = messagesRef.current;
    if (container) container.scrollTop = container.scrollHeight;
  }, [messages, pendingMessage]);

  async function handleSend(event: FormEvent) {
    event.preventDefault();
    const text = input.trim();
    if (!text || useAppStore.getState().chatPending) return;

    setChatPending(true);
    setSendError('');
    let conversationId = activeConversationId;
    try {
      if (!conversationId) {
        const created = await api.createConversation(text.slice(0, 80), settings.api_key, settings.gateway_url);
        const current = useAppStore.getState();
        if (current.settings !== settings || current.activeConversationId !== activeConversationId) return;
        conversationId = created.id;
        setActiveConversationId(conversationId);
      }

      setPendingMessage({ conversationId, text });
      await api.sendMessage(conversationId, text, settings.api_key, settings.gateway_url);
      setInput('');
      if (useAppStore.getState().settings === settings) await fetchMessages(conversationId);
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Could not send the message.';
      setSendError(message);
      toast.error(message);
    } finally {
      setPendingMessage(null);
      setChatPending(false);
    }
  }

  return (
    <section className="glass-panel flex h-[max(28rem,calc(100dvh-8rem))] min-w-0 flex-col overflow-hidden rounded-2xl border border-white/10">
      <header className="flex flex-wrap items-center justify-between gap-3 border-b border-white/10 bg-zinc-950/60 px-4 py-3 sm:px-6">
        <h1 className="flex items-center gap-2 font-semibold text-white">
          <Bot className="size-5 text-indigo-400" /> Playground Chat
        </h1>
        <div className="flex items-center gap-3 text-xs">
          <Link href="/conversations" className="text-zinc-300 hover:text-white">History</Link>
          <button
            type="button"
            disabled={chatPending || (!activeConversationId && !input)}
            onClick={() => {
              setActiveConversationId('');
              setInput('');
              setSendError('');
            }}
            className="flex items-center gap-1 rounded-lg border border-white/10 px-3 py-2 text-zinc-200 hover:bg-white/5 disabled:opacity-50"
          >
            <Plus className="size-4" /> New chat
          </button>
        </div>
      </header>

      <div ref={messagesRef} role="log" aria-label="Chat messages" className="min-h-0 flex-1 space-y-4 overflow-y-auto p-4 sm:p-6">
        {messagesError && (
          <div role="alert" className="flex flex-wrap items-center gap-3 text-sm text-rose-300">
            {messagesError}
            <button type="button" onClick={() => fetchMessages(activeConversationId)} className="underline">Retry history</button>
          </div>
        )}
        {messagesLoading && messages.length === 0 && <p role="status" className="text-sm text-zinc-400">Loading conversation…</p>}
        {!messagesLoading && !messagesError && messages.length === 0 && !pendingMessage && (
          <div className="mx-auto max-w-sm py-12 text-center">
            <p className="font-semibold text-white">Start a conversation</p>
            <p className="mt-2 text-sm text-zinc-400">Send a message to the model configured on your gateway.</p>
          </div>
        )}
        {messages.map((message) => (
          <article key={message.id} className={`flex ${message.role === 'user' ? 'justify-end' : ''}`}>
            <div className={`max-w-[min(90%,42rem)] rounded-2xl px-4 py-3 text-sm ${message.role === 'user' ? 'bg-indigo-700 text-white' : 'bg-zinc-900 text-zinc-200'}`}>
              <p className="mb-1 text-xs font-medium capitalize opacity-80">{message.role === 'user' ? 'You' : message.role}</p>
              <p className="whitespace-pre-wrap wrap-anywhere leading-relaxed">{message.content}</p>
            </div>
          </article>
        ))}
        {pendingMessage?.conversationId === activeConversationId && (
          <div className="space-y-3">
            <div className="ml-auto w-fit max-w-[90%] whitespace-pre-wrap wrap-anywhere rounded-2xl bg-indigo-700 px-4 py-3 text-sm text-white">{pendingMessage.text}</div>
            <p role="status" className="text-sm text-zinc-400">Waiting for the gateway…</p>
          </div>
        )}
      </div>

      <form onSubmit={handleSend} className="space-y-2 border-t border-white/10 p-4 sm:px-6">
        {sendError && <p role="alert" className="text-sm text-rose-300">{sendError}</p>}
        <div className="flex items-end gap-2">
          <textarea
            aria-label="Message"
            value={input}
            onChange={(event) => setInput(event.target.value)}
            rows={2}
            readOnly={chatPending}
            placeholder="Write a message…"
            className="min-w-0 flex-1 resize-none rounded-xl border border-white/10 bg-zinc-900 px-3 py-2 text-sm"
          />
          <button
            type="submit"
            disabled={!input.trim() || chatPending || Boolean(activeConversationId && (messagesLoading || messagesError))}
            className="flex items-center gap-2 rounded-xl bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white hover:bg-indigo-500 disabled:opacity-50"
          >
            <Send className="size-4" /> Send
          </button>
        </div>
      </form>
    </section>
  );
}
