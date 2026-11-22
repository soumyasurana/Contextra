import { create } from 'zustand';
import {
  DocumentResource,
  CollectionResource,
  ChatMessage,
  ConversationResource,
  PromptTemplate,
  EvalBenchmark,
  SystemSettings,
} from '@/types';
import {
  MOCK_PROMPTS,
  MOCK_EVAL_BENCHMARKS,
} from './mock-data';
import { api } from './api';

interface AppState {
  // System Settings
  settings: SystemSettings;
  updateSettings: (partial: Partial<SystemSettings>) => void;

  // Documents
  documents: DocumentResource[];
  documentsLoading: boolean;
  documentsError: string | null;
  fetchDocuments: () => Promise<void>;

  collections: CollectionResource[];
  collectionsLoading: boolean;
  collectionsError: string | null;
  fetchCollections: () => Promise<void>;

  conversations: ConversationResource[];
  conversationsCount: number;
  conversationsLoading: boolean;
  conversationsError: string | null;
  fetchConversations: () => Promise<void>;

  activeConversationId: string;
  messages: ChatMessage[];
  messagesLoading: boolean;
  messagesError: string | null;
  setActiveConversationId: (id: string) => void;
  fetchMessages: (id: string) => Promise<void>;
  chatPending: boolean;
  setChatPending: (pending: boolean) => void;

  // Prompts (no backend endpoint — keep mock for now)
  prompts: PromptTemplate[];
  activePromptId: string;
  setActivePromptId: (id: string) => void;
  updatePromptTemplate: (id: string, text: string) => void;
  addPromptTemplate: (prompt: PromptTemplate) => void;

  // Evaluations (no backend endpoint — keep mock for now)
  evals: EvalBenchmark[];

  // Global UI
  commandPaletteOpen: boolean;
  setCommandPaletteOpen: (open: boolean) => void;
  apiConnected: boolean;

  // Aggregated dashboard fetch
  fetchDashboardData: () => Promise<void>;
  checkApiHealth: () => Promise<void>;
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : 'The gateway request failed.';
}

const requests = { documents: 0, collections: 0, conversations: 0, messages: 0, health: 0 };

export const useAppStore = create<AppState>((set, get) => ({
  // Settings
  settings: { gateway_url: '', api_key: '' },
  updateSettings: (partial) => {
    set((state) => ({
      settings: { ...state.settings, ...partial },
      apiConnected: false,
      documents: [], documentsError: null,
      collections: [], collectionsError: null,
      conversations: [], conversationsCount: 0, conversationsError: null,
      activeConversationId: '', messages: [], messagesLoading: false, messagesError: null,
    }));
    void get().checkApiHealth();
  },

  documents: [],
  documentsLoading: false,
  documentsError: null,
  fetchDocuments: async () => {
    const { settings } = get();
    const requestId = ++requests.documents;
    set({ documentsLoading: true, documentsError: null });
    try {
      const items = await api.getDocuments(settings.api_key, settings.gateway_url);
      if (requests.documents === requestId && get().settings === settings) {
        set({ documents: items });
      }
    } catch (error) {
      if (requests.documents === requestId && get().settings === settings) {
        set({ documentsError: errorMessage(error) });
      }
    } finally {
      if (requests.documents === requestId) set({ documentsLoading: false });
    }
  },

  collections: [],
  collectionsLoading: false,
  collectionsError: null,
  fetchCollections: async () => {
    const { settings } = get();
    const requestId = ++requests.collections;
    set({ collectionsLoading: true, collectionsError: null });
    try {
      const items = await api.getCollections(settings.api_key, settings.gateway_url);
      if (requests.collections === requestId && get().settings === settings) {
        set({ collections: items });
      }
    } catch (error) {
      if (requests.collections === requestId && get().settings === settings) {
        set({ collectionsError: errorMessage(error) });
      }
    } finally {
      if (requests.collections === requestId) set({ collectionsLoading: false });
    }
  },

  conversations: [],
  conversationsCount: 0,
  conversationsLoading: false,
  conversationsError: null,
  fetchConversations: async () => {
    const { settings } = get();
    const requestId = ++requests.conversations;
    set({ conversationsLoading: true, conversationsError: null });
    try {
      const items = await api.getConversations(settings.api_key, settings.gateway_url);
      if (requests.conversations === requestId && get().settings === settings) {
        set({ conversations: items, conversationsCount: items.length });
      }
    } catch (error) {
      if (requests.conversations === requestId && get().settings === settings) {
        set({ conversationsError: errorMessage(error) });
      }
    } finally {
      if (requests.conversations === requestId) set({ conversationsLoading: false });
    }
  },

  activeConversationId: '',
  messages: [],
  messagesLoading: false,
  messagesError: null,
  setActiveConversationId: (id) => {
    if (get().activeConversationId === id) return;
    requests.messages++;
    set({ activeConversationId: id, messages: [], messagesLoading: false, messagesError: null });
  },
  fetchMessages: async (conversationId) => {
    const { settings, activeConversationId } = get();
    if (activeConversationId !== conversationId) return;
    const requestId = ++requests.messages;
    const isCurrent = () =>
      requests.messages === requestId && get().settings === settings && get().activeConversationId === conversationId;
    set({ messagesLoading: true, messagesError: null });
    try {
      const messages = await api.getMessages(conversationId, settings.api_key, settings.gateway_url);
      if (isCurrent()) set({ messages });
    } catch (error) {
      if (isCurrent()) set({ messagesError: errorMessage(error) });
    } finally {
      if (isCurrent()) set({ messagesLoading: false });
    }
  },
  chatPending: false,
  setChatPending: (chatPending) => set({ chatPending }),

  // Prompts — no backend endpoint, keep mock data
  prompts: MOCK_PROMPTS,
  activePromptId: MOCK_PROMPTS[0].id,
  setActivePromptId: (id) => set({ activePromptId: id }),
  updatePromptTemplate: (id, text) =>
    set((state) => ({
      prompts: state.prompts.map((p) =>
        p.id === id ? { ...p, template_text: text, updated_at: 'Just now' } : p
      ),
    })),
  addPromptTemplate: (prompt) =>
    set((state) => ({
      prompts: [prompt, ...state.prompts],
      activePromptId: prompt.id,
    })),

  // Evals — no backend endpoint, keep mock data
  evals: MOCK_EVAL_BENCHMARKS,
  // UI
  commandPaletteOpen: false,
  setCommandPaletteOpen: (open) => set({ commandPaletteOpen: open }),
  apiConnected: false,

  fetchDashboardData: async () => {
    const { fetchDocuments, fetchCollections, fetchConversations, checkApiHealth } = get();
    await Promise.all([fetchDocuments(), fetchCollections(), fetchConversations(), checkApiHealth()]);
  },

  checkApiHealth: async () => {
    const { settings } = get();
    const requestId = ++requests.health;
    const healthy = await api.checkHealth(settings.gateway_url);
    if (requests.health === requestId && get().settings === settings) set({ apiConnected: healthy });
  },
}));
