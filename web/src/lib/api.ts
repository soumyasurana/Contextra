import {
  DocumentResource,
  CollectionResource,
  ConversationResource,
  ChatMessage,
} from '@/types';

export const DEFAULT_GATEWAY_URL = process.env.NEXT_PUBLIC_GATEWAY_URL ?? '';

export function getBaseUrl(gatewayUrl?: string): string {
  return (gatewayUrl?.trim() || DEFAULT_GATEWAY_URL.trim()).replace(/\/+$/, '');
}

async function request<T>(url: string, options: RequestInit, timeoutMs = 5000): Promise<T> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(url, { ...options, signal: controller.signal });
    if (!response.ok) {
      const body = await response.json().catch(() => null);
      throw new Error(body?.error?.message || `Gateway returned HTTP ${response.status}`);
    }
    return await response.json() as T;
  } catch (error) {
    if (controller.signal.aborted) throw new Error('The gateway request timed out.');
    if (error instanceof TypeError) throw new Error('Unable to reach the gateway.');
    throw error;
  } finally {
    clearTimeout(timer);
  }
}

function authHeaders(apiKey?: string, json = false): Record<string, string> {
  const headers: Record<string, string> = {};
  if (json) headers['Content-Type'] = 'application/json';
  if (apiKey) {
    headers['x-api-key'] = apiKey;
  }
  return headers;
}

// ── Response mapping helpers ──────────────────────────────────────────
// The backend returns lean resource shapes; we map them to the richer
// frontend types, filling in defaults for fields that don't exist in
// the API response.

interface BackendDocumentResource {
  id: string;
  collection_id: string;
  content: string;
  metadata: Record<string, unknown>;
}

interface BackendCollectionResource {
  id: string;
  name: string;
  metadata: Record<string, unknown>;
}

interface BackendConversationResource {
  id: string;
  title: string | null;
  metadata: Record<string, unknown>;
}

interface BackendMessageResource {
  id: string;
  conversation_id: string;
  role: string;
  content: string;
  metadata: Record<string, unknown>;
}

interface BackendChatExecutionResponse {
  id: string;
  model: string;
  message: string;
  finish_reason: string | null;
}

interface PageResponse<T> {
  items: T[];
  next_cursor: string | null;
  has_more: boolean;
  total_count: number | null;
}

function mapDocument(doc: BackendDocumentResource): DocumentResource {
  const meta = doc.metadata || {};
  return {
    id: doc.id,
    name: (meta.name as string) || (meta.source_path as string) || doc.id,
    collection_id: doc.collection_id,
    collection_name: (meta.collection_name as string) || '',
    status: 'Ingested',
    chunks_count: (meta.chunks_count as number) || 0,
    file_size: (meta.file_size as string) || '—',
    uploaded_at: (meta.uploaded_at as string) || (meta.created_at as string) || '',
    metadata: meta as Record<string, string | number | boolean | null | undefined>,
    raw_content: doc.content,
  };
}

function mapCollection(col: BackendCollectionResource): CollectionResource {
  const meta = col.metadata || {};
  return {
    id: col.id,
    name: col.name,
    description: (meta.description as string) || '',
    documents_count: (meta.documents_count as number) || 0,
    chunks_count: (meta.chunks_count as number) || 0,
    created_at: (meta.created_at as string) || '',
  };
}

function mapConversation(conv: BackendConversationResource): ConversationResource {
  const meta = conv.metadata || {};
  return {
    id: conv.id,
    title: conv.title || 'Untitled Conversation',
    message_count: (meta.message_count as number) || 0,
    summary: (meta.summary as string) || '',
    memory_size_kb: (meta.memory_size_kb as number) || 0,
    token_count: (meta.token_count as number) || 0,
    last_updated: (meta.last_updated as string) || '',
  };
}

function mapMessage(msg: BackendMessageResource): ChatMessage {
  return {
    id: msg.id,
    conversation_id: msg.conversation_id,
    role: msg.role as 'user' | 'assistant' | 'system',
    content: msg.content,
    timestamp: (msg.metadata?.timestamp as string) || '',
  };
}

async function list<T>(path: string, apiKey?: string, gatewayUrl?: string) {
  const items: T[] = [];
  const base = getBaseUrl(gatewayUrl);
  const cursors = new Set<string>();
  let cursor: string | null = null;
  do {
    const params = new URLSearchParams({ limit: '100' });
    if (cursor) params.set('cursor', cursor);
    const page = await request<PageResponse<T>>(`${base}${path}?${params}`, {
      headers: authHeaders(apiKey),
    });
    items.push(...page.items);
    cursor = page.has_more ? page.next_cursor : null;
    if (cursor && cursors.has(cursor)) throw new Error('The gateway repeated a pagination cursor.');
    if (cursor) cursors.add(cursor);
  } while (cursor);
  return items;
}

function post<T>(path: string, body: object, apiKey?: string, gatewayUrl?: string, timeoutMs = 15000) {
  return request<T>(`${getBaseUrl(gatewayUrl)}${path}`, {
    method: 'POST',
    headers: authHeaders(apiKey, true),
    body: JSON.stringify(body),
  }, timeoutMs);
}

export const api = {
  async getDocuments(apiKey?: string, gatewayUrl?: string) {
    return (await list<BackendDocumentResource>('/api/v1/documents', apiKey, gatewayUrl)).map(mapDocument);
  },

  async createDocument(document: { name: string; content: string; collection_id?: string }, apiKey?: string, gatewayUrl?: string) {
    return mapDocument(await post<BackendDocumentResource>('/api/v1/documents', document, apiKey, gatewayUrl, 60000));
  },

  async getCollections(apiKey?: string, gatewayUrl?: string) {
    return (await list<BackendCollectionResource>('/api/v1/collections', apiKey, gatewayUrl)).map(mapCollection);
  },

  async createCollection(name: string, metadata: Record<string, unknown>, apiKey?: string, gatewayUrl?: string) {
    return mapCollection(await post<BackendCollectionResource>('/api/v1/collections', { name, metadata }, apiKey, gatewayUrl));
  },

  async updateCollection(id: string, name: string, metadata: Record<string, unknown>, apiKey?: string, gatewayUrl?: string) {
    const updated = await request<BackendCollectionResource>(
      `${getBaseUrl(gatewayUrl)}/api/v1/collections/${encodeURIComponent(id)}`,
      { method: 'PATCH', headers: authHeaders(apiKey, true), body: JSON.stringify({ name, metadata }) },
    );
    return mapCollection(updated);
  },

  async getConversations(apiKey?: string, gatewayUrl?: string) {
    return (await list<BackendConversationResource>('/api/v1/conversations', apiKey, gatewayUrl)).map(mapConversation);
  },

  async createConversation(title: string, apiKey?: string, gatewayUrl?: string) {
    return mapConversation(await post<BackendConversationResource>('/api/v1/conversations', { title }, apiKey, gatewayUrl));
  },

  async getMessages(conversationId: string, apiKey?: string, gatewayUrl?: string) {
    const path = `/api/v1/conversations/${encodeURIComponent(conversationId)}/messages`;
    return (await list<BackendMessageResource>(path, apiKey, gatewayUrl)).map(mapMessage);
  },

  async sendMessage(conversationId: string, message: string, apiKey?: string, gatewayUrl?: string): Promise<void> {
    const path = `/api/v1/conversations/${encodeURIComponent(conversationId)}/messages`;
    await post<BackendChatExecutionResponse>(path, { message }, apiKey, gatewayUrl, 60000);
  },

  async checkHealth(gatewayUrl?: string): Promise<boolean> {
    try {
      const response = await fetch(`${getBaseUrl(gatewayUrl)}/health`, {
        cache: 'no-store',
        signal: AbortSignal.timeout(5000),
      });
      return response.ok;
    } catch {
      return false;
    }
  },
};
