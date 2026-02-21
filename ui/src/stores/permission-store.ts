"use client";
import { create } from "zustand";
import api from "@/lib/api";

export interface PermissionRequest {
  id: string;
  sessionID: string;
  permission: string;
  patterns: string[];
  metadata: Record<string, unknown>;
  always: string[];
  tool?: { messageID: string; callID: string };
}

interface PermissionState {
  pending: Record<string, PermissionRequest>;

  reply: (
    permissionID: string,
    reply: "once" | "always" | "reject",
  ) => Promise<void>;
  onPermissionAsked: (request: PermissionRequest) => void;
  onPermissionReplied: (requestID: string) => void;
}

export const usePermissionStore = create<PermissionState>((set) => ({
  pending: {},

  reply: async (permissionID, reply) => {
    await api.post(`/permission/${permissionID}/reply`, { reply });
    set((state) => {
      const { [permissionID]: _, ...rest } = state.pending;
      return { pending: rest };
    });
  },

  onPermissionAsked: (request) =>
    set((state) => ({
      pending: { ...state.pending, [request.id]: request },
    })),

  onPermissionReplied: (requestID) =>
    set((state) => {
      const { [requestID]: _, ...rest } = state.pending;
      return { pending: rest };
    }),
}));
