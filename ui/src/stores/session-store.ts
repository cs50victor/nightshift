"use client";
import type { FileDiff, Session, SessionStatus, Todo } from "@opencode-ai/sdk";
import { create } from "zustand";
import api from "@/lib/api";
import { useNodeStore } from "@/stores/node-store";

interface SessionState {
  sessions: Record<string, Session>;
  sessionStatus: Record<string, SessionStatus>;
  sessionDiffs: Record<string, FileDiff[]>;
  sessionTodos: Record<string, Todo[]>;
  sessionErrors: Record<string, unknown>;

  loadSessions: () => Promise<void>;
  createSession: () => Promise<Session>;
  deleteSession: (id: string) => Promise<void>;
  abortSession: (id: string) => Promise<void>;

  getSession: (id: string) => Session | undefined;
  getSessionStatus: (id: string) => SessionStatus;
  onSessionCreated: (session: Session) => void;
  onSessionUpdated: (session: Session) => void;
  onSessionDeleted: (session: Session) => void;
  onSessionStatus: (sessionID: string, status: SessionStatus) => void;
  onSessionIdle: (sessionID: string) => void;
  onSessionDiff: (sessionID: string, diffs: FileDiff[]) => void;
  onSessionError: (sessionID: string, error: unknown) => void;
  onTodoUpdated: (sessionID: string, todos: Todo[]) => void;
}

const IDLE_STATUS: SessionStatus = { type: "idle" };

export const useSessionStore = create<SessionState>((set, get) => ({
  sessions: {},
  sessionStatus: {},
  sessionDiffs: {},
  sessionTodos: {},
  sessionErrors: {},

  loadSessions: async () => {
    const data = await api.fetch<Session[]>("/session");
    const sessions: Record<string, Session> = {};
    for (const s of data) {
      sessions[s.id] = s;
    }
    set({ sessions });
  },

  createSession: async () => {
    const { activeNodeUrl } = useNodeStore.getState();
    if (!activeNodeUrl) {
      throw new Error(
        "No node selected. Select a node before creating a session.",
      );
    }
    const session = await api.post<Session>("/session");
    set((state) => ({
      sessions: { ...state.sessions, [session.id]: session },
    }));
    return session;
  },

  deleteSession: async (id) => {
    await api.delete(`/session/${id}`);
    set((state) => {
      const { [id]: _, ...rest } = state.sessions;
      return { sessions: rest };
    });
  },

  abortSession: async (id) => {
    await api.post(`/session/${id}/abort`);
  },

  getSession: (id) => get().sessions[id],

  getSessionStatus: (id) => get().sessionStatus[id] ?? IDLE_STATUS,

  onSessionCreated: (session) =>
    set((state) => ({
      sessions: { ...state.sessions, [session.id]: session },
    })),

  onSessionUpdated: (session) =>
    set((state) => ({
      sessions: { ...state.sessions, [session.id]: session },
    })),

  onSessionDeleted: (session) =>
    set((state) => {
      const { [session.id]: _, ...rest } = state.sessions;
      return { sessions: rest };
    }),

  onSessionStatus: (sessionID, status) =>
    set((state) => {
      const nextErrors = { ...state.sessionErrors };
      if (status.type === "busy" || status.type === "retry") {
        delete nextErrors[sessionID];
      }
      return {
        sessionStatus: { ...state.sessionStatus, [sessionID]: status },
        sessionErrors: nextErrors,
      };
    }),

  onSessionIdle: (sessionID) =>
    set((state) => ({
      sessionStatus: {
        ...state.sessionStatus,
        [sessionID]: { type: "idle" },
      },
    })),

  onSessionDiff: (sessionID, diffs) =>
    set((state) => ({
      sessionDiffs: { ...state.sessionDiffs, [sessionID]: diffs },
    })),

  onSessionError: (sessionID, error) =>
    set((state) => ({
      sessionErrors: { ...state.sessionErrors, [sessionID]: error },
    })),

  onTodoUpdated: (sessionID, todos) =>
    set((state) => ({
      sessionTodos: { ...state.sessionTodos, [sessionID]: todos },
    })),
}));
