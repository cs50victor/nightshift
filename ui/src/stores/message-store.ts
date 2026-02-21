"use client";
import type { Message, Part } from "@opencode-ai/sdk";
import { create } from "zustand";
import api from "@/lib/api";

export interface MessageWithParts {
  info: Message;
  parts: Part[];
  isOptimistic?: boolean;
}

interface MessageState {
  messagesBySession: Record<string, MessageWithParts[]>;
  orphanPartsByMessage: Record<string, Part[]>;
  orphanDeltasByPart: Record<string, Array<{ field: string; delta: string }>>;

  loadMessages: (sessionID: string) => Promise<void>;
  sendMessage: (
    sessionID: string,
    params: {
      text: string;
      model: { providerID: string; modelID: string };
      agent?: string;
    },
  ) => Promise<void>;

  addOptimisticMessage: (sessionID: string, text: string) => string;
  removeOptimisticMessage: (sessionID: string, messageID: string) => void;

  onMessageUpdated: (message: Message) => void;
  onMessageRemoved: (sessionID: string, messageID: string) => void;
  onPartUpdated: (part: Part, delta?: string) => void;
  onPartDelta: (
    messageID: string,
    partID: string,
    field: string,
    delta: string,
  ) => void;
  onPartRemoved: (sessionID: string, messageID: string, partID: string) => void;
}

let optimisticCounter = 0;

export const useMessageStore = create<MessageState>((set) => ({
  messagesBySession: {},
  orphanPartsByMessage: {},
  orphanDeltasByPart: {},

  loadMessages: async (sessionID) => {
    const data = await api.fetch<MessageWithParts[]>(
      `/session/${sessionID}/message`,
    );
    set((state) => ({
      messagesBySession: {
        ...state.messagesBySession,
        [sessionID]: data ?? [],
      },
    }));
  },

  sendMessage: async (sessionID, { text, model, agent }) => {
    await api.post(`/session/${sessionID}/prompt_async`, {
      parts: [{ type: "text", text }],
      model,
      agent,
    });
  },

  addOptimisticMessage: (sessionID, text) => {
    const tempID = `optimistic-${++optimisticCounter}`;
    const msg: MessageWithParts = {
      info: {
        id: tempID,
        sessionID,
        role: "user",
        time: { created: Date.now() },
        agent: "",
        model: { providerID: "", modelID: "" },
      },
      parts: [
        {
          id: `${tempID}-part`,
          sessionID,
          messageID: tempID,
          type: "text",
          text,
        },
      ],
      isOptimistic: true,
    };
    set((state) => ({
      messagesBySession: {
        ...state.messagesBySession,
        [sessionID]: [...(state.messagesBySession[sessionID] ?? []), msg],
      },
    }));
    return tempID;
  },

  removeOptimisticMessage: (sessionID, messageID) =>
    set((state) => {
      const msgs = state.messagesBySession[sessionID];
      if (!msgs) return state;
      return {
        messagesBySession: {
          ...state.messagesBySession,
          [sessionID]: msgs.filter((m) => m.info.id !== messageID),
        },
      };
    }),

  onMessageUpdated: (message) =>
    set((state) => {
      const sid = message.sessionID;
      const msgs = [...(state.messagesBySession[sid] ?? [])];
      const orphanKey = message.id;
      const orphanParts = state.orphanPartsByMessage[orphanKey] ?? [];
      const idx = msgs.findIndex((m) => m.info.id === message.id);
      if (idx >= 0) {
        const existingParts = msgs[idx].parts;
        msgs[idx] = {
          ...msgs[idx],
          info: message,
          parts: existingParts.length > 0 ? existingParts : orphanParts,
        };
      } else {
        msgs.push({ info: message, parts: orphanParts });
      }

      const { [orphanKey]: _ignored, ...restOrphans } =
        state.orphanPartsByMessage;

      return {
        messagesBySession: { ...state.messagesBySession, [sid]: msgs },
        orphanPartsByMessage: restOrphans,
      };
    }),

  onMessageRemoved: (sessionID, messageID) =>
    set((state) => {
      const msgs = state.messagesBySession[sessionID];
      if (!msgs) return state;
      return {
        messagesBySession: {
          ...state.messagesBySession,
          [sessionID]: msgs.filter((m) => m.info.id !== messageID),
        },
      };
    }),

  // NOTE(victor): delta comes via EventMessagePartUpdated.properties.delta, not a separate event type.
  // When delta is present, we append it to the part field (usually "text") instead of replacing the whole part.
  onPartUpdated: (part, delta) =>
    set((state) => {
      const sid = part.sessionID;
      const msgs = [...(state.messagesBySession[sid] ?? [])];
      const msgIdx = msgs.findIndex((m) => m.info.id === part.messageID);
      const deltaKey = `${part.messageID}:${part.id}`;
      const pendingDeltas = state.orphanDeltasByPart[deltaKey] ?? [];
      let nextPart = part;
      if (pendingDeltas.length > 0) {
        const merged = { ...part } as Record<string, unknown>;
        for (const pending of pendingDeltas) {
          const existing = merged[pending.field];
          if (typeof existing === "string") {
            merged[pending.field] = existing + pending.delta;
          }
        }
        nextPart = merged as Part;
      }
      if (msgIdx < 0) {
        const orphanKey = part.messageID;
        const existingOrphans = [
          ...(state.orphanPartsByMessage[orphanKey] ?? []),
        ];
        const orphanIdx = existingOrphans.findIndex(
          (p) => p.id === nextPart.id,
        );

        if (delta) {
          if (orphanIdx >= 0) {
            const existing = existingOrphans[orphanIdx];
            const field = "text" in existing ? "text" : null;
            if (field) {
              existingOrphans[orphanIdx] = {
                ...existing,
                [field]: (existing as Record<string, unknown>)[field] + delta,
              } as Part;
            } else {
              existingOrphans[orphanIdx] = nextPart;
            }
          } else {
            existingOrphans.push(nextPart);
          }
        } else if (orphanIdx >= 0) {
          existingOrphans[orphanIdx] = nextPart;
        } else {
          existingOrphans.push(nextPart);
        }

        const { [deltaKey]: _ignored, ...remainingDeltas } =
          state.orphanDeltasByPart;

        return {
          orphanPartsByMessage: {
            ...state.orphanPartsByMessage,
            [orphanKey]: existingOrphans,
          },
          orphanDeltasByPart: remainingDeltas,
        };
      }

      const msg = { ...msgs[msgIdx], parts: [...msgs[msgIdx].parts] };
      const partIdx = msg.parts.findIndex((p) => p.id === part.id);

      if (delta) {
        if (partIdx >= 0) {
          const existing = msg.parts[partIdx];
          const field = "text" in existing ? "text" : null;
          if (field) {
            msg.parts[partIdx] = {
              ...existing,
              [field]: (existing as Record<string, unknown>)[field] + delta,
            } as Part;
          } else {
            msg.parts[partIdx] = nextPart;
          }
        } else {
          msg.parts.push(nextPart);
        }
      } else if (partIdx >= 0) {
        msg.parts[partIdx] = nextPart;
      } else {
        msg.parts.push(nextPart);
      }

      msgs[msgIdx] = msg;
      const { [deltaKey]: _ignored, ...remainingDeltas } =
        state.orphanDeltasByPart;
      return {
        messagesBySession: { ...state.messagesBySession, [sid]: msgs },
        orphanDeltasByPart: remainingDeltas,
      };
    }),

  onPartDelta: (messageID, partID, field, delta) =>
    set((state) => {
      for (const [sid, sessionMessages] of Object.entries(
        state.messagesBySession,
      )) {
        const msgIdx = sessionMessages.findIndex(
          (m) => m.info.id === messageID,
        );
        if (msgIdx < 0) continue;

        const msgs = [...sessionMessages];
        const msg = { ...msgs[msgIdx], parts: [...msgs[msgIdx].parts] };
        const partIdx = msg.parts.findIndex((p) => p.id === partID);
        if (partIdx < 0) break;

        const existingPart = msg.parts[partIdx] as Record<string, unknown>;
        const existingValue = existingPart[field];
        if (typeof existingValue === "string") {
          msg.parts[partIdx] = {
            ...msg.parts[partIdx],
            [field]: existingValue + delta,
          } as Part;
          msgs[msgIdx] = msg;
          return {
            messagesBySession: {
              ...state.messagesBySession,
              [sid]: msgs,
            },
          };
        }

        return state;
      }

      const orphanParts = state.orphanPartsByMessage[messageID];
      if (orphanParts) {
        const orphanIdx = orphanParts.findIndex((p) => p.id === partID);
        if (orphanIdx >= 0) {
          const nextOrphans = [...orphanParts];
          const existingPart = nextOrphans[orphanIdx] as Record<
            string,
            unknown
          >;
          const existingValue = existingPart[field];
          if (typeof existingValue === "string") {
            nextOrphans[orphanIdx] = {
              ...nextOrphans[orphanIdx],
              [field]: existingValue + delta,
            } as Part;
            return {
              orphanPartsByMessage: {
                ...state.orphanPartsByMessage,
                [messageID]: nextOrphans,
              },
            };
          }
        }
      }

      const deltaKey = `${messageID}:${partID}`;
      return {
        orphanDeltasByPart: {
          ...state.orphanDeltasByPart,
          [deltaKey]: [
            ...(state.orphanDeltasByPart[deltaKey] ?? []),
            { field, delta },
          ],
        },
      };
    }),

  onPartRemoved: (sessionID, messageID, partID) =>
    set((state) => {
      const msgs = [...(state.messagesBySession[sessionID] ?? [])];
      const msgIdx = msgs.findIndex((m) => m.info.id === messageID);
      if (msgIdx < 0) {
        const orphanKey = messageID;
        const orphans = state.orphanPartsByMessage[orphanKey];
        if (!orphans) return state;
        return {
          orphanPartsByMessage: {
            ...state.orphanPartsByMessage,
            [orphanKey]: orphans.filter((p) => p.id !== partID),
          },
        };
      }

      const msg = {
        ...msgs[msgIdx],
        parts: msgs[msgIdx].parts.filter((p) => p.id !== partID),
      };
      msgs[msgIdx] = msg;
      return {
        messagesBySession: {
          ...state.messagesBySession,
          [sessionID]: msgs,
        },
      };
    }),
}));
