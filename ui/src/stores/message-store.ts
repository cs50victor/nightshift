"use client";
import type { Message, Part } from "@opencode-ai/sdk";
import { create } from "zustand";
import api from "@/lib/api";
import { useConfigStore } from "@/stores/config-store";

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
      attachments?: { dataUrl: string; filename: string; mime: string }[];
    },
  ) => Promise<void>;
  handleSessionError: (sessionID: string, error: unknown) => Promise<boolean>;

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

type Family = "claude" | "openai" | "opencode";

interface QueuedSend {
  parts: Record<string, string>[];
  agent?: string;
  candidates: Array<{ providerID: string; modelID: string }>;
  index: number;
  createdAt: number;
}

const FALLBACK_MAX_AGE_MS = 5 * 60 * 1000;
const pendingFallbackBySession: Record<string, QueuedSend> = {};

function normalizeFamily(providerID: string): Family | null {
  const normalized = providerID.toLowerCase();
  if (normalized === "anthropic" || normalized === "claude") return "claude";
  if (normalized === "openai") return "openai";
  if (normalized === "opencode") return "opencode";
  return null;
}

function familyOrder(primaryProviderID: string): Family[] {
  const primary = normalizeFamily(primaryProviderID);
  if (primary === "claude") return ["claude", "openai", "opencode"];
  if (primary === "openai") return ["openai", "claude", "opencode"];
  if (primary === "opencode") return ["opencode", "openai", "claude"];
  return ["claude", "openai", "opencode"];
}

function pickFamilyModel(
  family: Family,
  currentModelID: string,
): { providerID: string; modelID: string } | null {
  const providers = useConfigStore.getState().providers;
  const matchedProviders = providers.filter(
    (provider) => normalizeFamily(provider.id) === family,
  );
  if (matchedProviders.length === 0) return null;

  for (const provider of matchedProviders) {
    if (provider.models[currentModelID]) {
      return { providerID: provider.id, modelID: currentModelID };
    }
  }

  const provider = matchedProviders[0];
  const modelID = Object.keys(provider.models ?? {})[0];
  if (!modelID) return null;
  return { providerID: provider.id, modelID };
}

function buildFallbackCandidates(model: {
  providerID: string;
  modelID: string;
}): Array<{ providerID: string; modelID: string }> {
  const order = familyOrder(model.providerID);
  const picked = order
    .map((family) => pickFamilyModel(family, model.modelID))
    .filter(
      (
        candidate,
      ): candidate is {
        providerID: string;
        modelID: string;
      } => Boolean(candidate),
    );
  const deduped = picked.filter(
    (candidate, index, all) =>
      all.findIndex(
        (entry) =>
          entry.providerID === candidate.providerID &&
          entry.modelID === candidate.modelID,
      ) === index,
  );
  if (deduped.length > 3) return deduped.slice(0, 3);
  return deduped;
}

function isRetryableError(error: unknown): boolean {
  if (!error) return false;
  if (typeof error === "string") {
    const normalized = error.toLowerCase();
    return (
      normalized.includes("429") ||
      normalized.includes("rate") ||
      normalized.includes("overload") ||
      normalized.includes("temporar") ||
      normalized.includes("503") ||
      normalized.includes("502")
    );
  }
  if (error instanceof Error) {
    return isRetryableError(error.message);
  }
  if (typeof error === "object") {
    const record = error as Record<string, unknown>;
    if (record.name === "APIError") {
      const data =
        record.data && typeof record.data === "object"
          ? (record.data as Record<string, unknown>)
          : null;
      if (data && typeof data.isRetryable === "boolean") {
        return data.isRetryable;
      }
      if (data && typeof data.statusCode === "number") {
        return data.statusCode === 429 || data.statusCode >= 500;
      }
    }
    if (typeof record.message === "string") {
      return isRetryableError(record.message);
    }
  }
  return false;
}

async function postPrompt(
  sessionID: string,
  payload: {
    parts: Record<string, string>[];
    model: { providerID: string; modelID: string };
    agent?: string;
  },
) {
  await api.post(`/session/${sessionID}/prompt_async`, payload);
}

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

  sendMessage: async (sessionID, { text, model, agent, attachments }) => {
    const parts: Record<string, string>[] = [];
    if (text) parts.push({ type: "text", text });
    if (attachments) {
      for (const att of attachments) {
        parts.push({
          type: "file",
          filename: att.filename,
          mime: att.mime,
          url: att.dataUrl,
        });
      }
    }
    const candidates = buildFallbackCandidates(model);
    if (candidates.length === 0) {
      await postPrompt(sessionID, { parts, model, agent });
      return;
    }

    pendingFallbackBySession[sessionID] = {
      parts,
      agent,
      candidates,
      index: 0,
      createdAt: Date.now(),
    };
    await postPrompt(sessionID, {
      parts,
      model: candidates[0],
      agent,
    });
  },

  handleSessionError: async (sessionID, error) => {
    const queued = pendingFallbackBySession[sessionID];
    if (!queued) return false;
    if (!isRetryableError(error)) {
      delete pendingFallbackBySession[sessionID];
      return false;
    }
    const age = Date.now() - queued.createdAt;
    if (age > FALLBACK_MAX_AGE_MS) {
      delete pendingFallbackBySession[sessionID];
      return false;
    }

    const nextIndex = queued.index + 1;
    const nextModel = queued.candidates[nextIndex];
    if (!nextModel) {
      delete pendingFallbackBySession[sessionID];
      return false;
    }

    queued.index = nextIndex;
    queued.createdAt = Date.now();
    pendingFallbackBySession[sessionID] = queued;

    await postPrompt(sessionID, {
      parts: queued.parts,
      model: nextModel,
      agent: queued.agent,
    });
    return true;
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
