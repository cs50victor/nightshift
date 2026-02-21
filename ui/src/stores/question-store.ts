"use client";
import { create } from "zustand";
import api from "@/lib/api";

// NOTE(victor): SDK does not export Question types; defining locally based on API spec.
interface QuestionOption {
  label: string;
  description: string;
}

interface QuestionInfo {
  question: string;
  header: string;
  options: QuestionOption[];
  multiple?: boolean;
  custom?: boolean;
}

export interface QuestionRequest {
  id: string;
  sessionID: string;
  questions: QuestionInfo[];
  tool?: { messageID: string; callID: string };
}

interface QuestionState {
  pending: Record<string, QuestionRequest>;

  reply: (requestID: string, answers: string[][]) => Promise<void>;
  reject: (requestID: string) => Promise<void>;
  onQuestionAsked: (request: QuestionRequest) => void;
  onQuestionReplied: (requestID: string) => void;
  onQuestionRejected: (requestID: string) => void;
}

export const useQuestionStore = create<QuestionState>((set) => ({
  pending: {},

  reply: async (requestID, answers) => {
    await api.post(`/question/${requestID}/reply`, { answers });
    set((state) => {
      const { [requestID]: _, ...rest } = state.pending;
      return { pending: rest };
    });
  },

  reject: async (requestID) => {
    await api.post(`/question/${requestID}/reject`);
    set((state) => {
      const { [requestID]: _, ...rest } = state.pending;
      return { pending: rest };
    });
  },

  onQuestionAsked: (request) =>
    set((state) => ({
      pending: { ...state.pending, [request.id]: request },
    })),

  onQuestionReplied: (requestID) =>
    set((state) => {
      const { [requestID]: _, ...rest } = state.pending;
      return { pending: rest };
    }),

  onQuestionRejected: (requestID) =>
    set((state) => {
      const { [requestID]: _, ...rest } = state.pending;
      return { pending: rest };
    }),
}));
