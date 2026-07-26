import { create } from 'zustand';
import {
  DocumentResource,
  CollectionResource,
  ChatMessage,
  ConversationResource,
  RetrievedChunk,
  PromptTemplate,
  EvalBenchmark,
  SystemSettings,
} from '@/types';
import {
  INITIAL_SETTINGS,
  MOCK_DOCUMENTS,
  MOCK_COLLECTIONS,
  MOCK_CONVERSATIONS,
  MOCK_CHAT_MESSAGES,
  MOCK_RETRIEVED_CHUNKS,
  MOCK_PROMPTS,
  MOCK_EVAL_BENCHMARKS,
} from './mock-data';

interface AppState {
  // System Settings
  settings: SystemSettings;
  updateSettings: (partial: Partial<SystemSettings>) => void;

  // Documents
  documents: DocumentResource[];
  addDocument: (doc: DocumentResource) => void;
  deleteDocument: (id: string) => void;

  // Collections
  collections: CollectionResource[];
  addCollection: (col: CollectionResource) => void;
  deleteCollection: (id: string) => void;
  updateCollection: (id: string, name: string, description: string) => void;

  // Chat
  activeConversationId: string;
  conversations: ConversationResource[];
  messages: ChatMessage[];
  retrievedChunks: RetrievedChunk[];
  selectedChunk: RetrievedChunk | null;
  isStreaming: boolean;
  setActiveConversationId: (id: string) => void;
  setSelectedChunk: (chunk: RetrievedChunk | null) => void;
  addChatMessage: (msg: ChatMessage) => void;
  setStreaming: (streaming: boolean) => void;

  // Prompts
  prompts: PromptTemplate[];
  activePromptId: string;
  setActivePromptId: (id: string) => void;
  updatePromptTemplate: (id: string, text: string) => void;
  addPromptTemplate: (prompt: PromptTemplate) => void;

  // Evaluations
  evals: EvalBenchmark[];
  addEvalRun: (evalRun: EvalBenchmark) => void;

  // Global UI
  commandPaletteOpen: boolean;
  setCommandPaletteOpen: (open: boolean) => void;
  apiConnected: boolean;
  setApiConnected: (connected: boolean) => void;
}

export const useAppStore = create<AppState>((set) => ({
  // Settings
  settings: INITIAL_SETTINGS,
  updateSettings: (partial) =>
    set((state) => ({
      settings: { ...state.settings, ...partial },
    })),

  // Documents
  documents: MOCK_DOCUMENTS,
  addDocument: (doc) =>
    set((state) => ({
      documents: [doc, ...state.documents],
    })),
  deleteDocument: (id) =>
    set((state) => ({
      documents: state.documents.filter((d) => d.id !== id),
    })),

  // Collections
  collections: MOCK_COLLECTIONS,
  addCollection: (col) =>
    set((state) => ({
      collections: [col, ...state.collections],
    })),
  deleteCollection: (id) =>
    set((state) => ({
      collections: state.collections.filter((c) => c.id !== id),
    })),
  updateCollection: (id, name, description) =>
    set((state) => ({
      collections: state.collections.map((c) =>
        c.id === id ? { ...c, name, description } : c
      ),
    })),

  // Chat
  activeConversationId: 'conv_8f3a1d90',
  conversations: MOCK_CONVERSATIONS,
  messages: MOCK_CHAT_MESSAGES,
  retrievedChunks: MOCK_RETRIEVED_CHUNKS,
  selectedChunk: MOCK_RETRIEVED_CHUNKS[0] || null,
  isStreaming: false,
  setActiveConversationId: (id) =>
    set({
      activeConversationId: id,
    }),
  setSelectedChunk: (chunk) => set({ selectedChunk: chunk }),
  addChatMessage: (msg) =>
    set((state) => ({
      messages: [...state.messages, msg],
    })),
  setStreaming: (streaming) => set({ isStreaming: streaming }),

  // Prompts
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

  // Evals
  evals: MOCK_EVAL_BENCHMARKS,
  addEvalRun: (evalRun) =>
    set((state) => ({
      evals: [evalRun, ...state.evals],
    })),

  // UI
  commandPaletteOpen: false,
  setCommandPaletteOpen: (open) => set({ commandPaletteOpen: open }),
  apiConnected: true,
  setApiConnected: (connected) => set({ apiConnected: connected }),
}));
