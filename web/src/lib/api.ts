import {
  DocumentResource,
  CollectionResource,
  ConversationResource,
  ChatMessage,
} from '@/types';

const GATEWAY_URL = process.env.NEXT_PUBLIC_GATEWAY_URL || 'http://localhost:3000';

async function fetchWithTimeout(url: string, options: RequestInit = {}, timeoutMs = 3000) {
  const controller = new AbortController();
  const id = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const res = await fetch(url, { ...options, signal: controller.signal });
    clearTimeout(id);
    return res;
  } catch (err) {
    clearTimeout(id);
    throw err;
  }
}

export const api = {
  async getDocuments(): Promise<DocumentResource[]> {
    try {
      const res = await fetchWithTimeout(`${GATEWAY_URL}/api/v1/documents`);
      if (!res.ok) throw new Error('API Error');
      const data = await res.json();
      return data.items || data;
    } catch {
      return []; // Fall back to store/mock
    }
  },

  async createDocument(sourcePath: string): Promise<DocumentResource | null> {
    try {
      const res = await fetchWithTimeout(`${GATEWAY_URL}/api/v1/documents`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ source_path: sourcePath }),
      });
      if (!res.ok) throw new Error('Failed to create document');
      return await res.json();
    } catch {
      return null;
    }
  },

  async getCollections(): Promise<CollectionResource[]> {
    try {
      const res = await fetchWithTimeout(`${GATEWAY_URL}/api/v1/collections`);
      if (!res.ok) throw new Error('API Error');
      const data = await res.json();
      return data.items || data;
    } catch {
      return [];
    }
  },

  async createCollection(name: string, metadata: Record<string, any> = {}): Promise<CollectionResource | null> {
    try {
      const res = await fetchWithTimeout(`${GATEWAY_URL}/api/v1/collections`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name, metadata }),
      });
      if (!res.ok) throw new Error('Failed to create collection');
      return await res.json();
    } catch {
      return null;
    }
  },

  async getConversations(): Promise<ConversationResource[]> {
    try {
      const res = await fetchWithTimeout(`${GATEWAY_URL}/api/v1/conversations`);
      if (!res.ok) throw new Error('API Error');
      const data = await res.json();
      return data.items || data;
    } catch {
      return [];
    }
  },

  async createConversation(title?: string): Promise<ConversationResource | null> {
    try {
      const res = await fetchWithTimeout(`${GATEWAY_URL}/api/v1/conversations`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ title }),
      });
      if (!res.ok) throw new Error('Failed to create conversation');
      return await res.json();
    } catch {
      return null;
    }
  },

  async sendMessage(conversationId: string, message: string): Promise<ChatMessage | null> {
    try {
      const res = await fetchWithTimeout(`${GATEWAY_URL}/api/v1/conversations/${conversationId}/messages`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ message }),
      });
      if (!res.ok) throw new Error('Failed to send message');
      const data = await res.json();
      return {
        id: data.id || `msg_${Date.now()}`,
        conversation_id: conversationId,
        role: 'assistant',
        content: data.message || data.content,
        timestamp: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
      };
    } catch {
      return null;
    }
  },

  async checkHealth(): Promise<boolean> {
    try {
      const res = await fetchWithTimeout(`${GATEWAY_URL}/api/v1/collections`, {}, 1500);
      return res.ok;
    } catch {
      return false;
    }
  },
};
