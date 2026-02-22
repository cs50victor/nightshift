"use client";
import type {
  Event,
  FileDiff,
  Message,
  Part,
  Session,
  SessionStatus,
  Todo,
} from "@opencode-ai/sdk";
import { create } from "zustand";
import { useMessageStore } from "./message-store";
import type { PermissionRequest } from "./permission-store";
import { usePermissionStore } from "./permission-store";
import type { QuestionRequest } from "./question-store";
import { useQuestionStore } from "./question-store";
import { useSessionStore } from "./session-store";

function hasString(obj: Record<string, unknown>, key: string): boolean {
  return typeof obj[key] === "string";
}
function hasObject(obj: Record<string, unknown>, key: string): boolean {
  return obj[key] != null && typeof obj[key] === "object";
}
function hasArray(obj: Record<string, unknown>, key: string): boolean {
  return Array.isArray(obj[key]);
}

function dispatchEvent(event: Event) {
  const props = event.properties as Record<string, unknown>;

  switch (event.type) {
    case "message.updated":
      if (!hasObject(props, "info")) break;
      useMessageStore.getState().onMessageUpdated(props.info as Message);
      break;
    case "message.removed":
      if (!hasString(props, "sessionID") || !hasString(props, "messageID"))
        break;
      useMessageStore
        .getState()
        .onMessageRemoved(props.sessionID as string, props.messageID as string);
      break;
    case "message.part.updated":
      if (!hasObject(props, "part")) break;
      useMessageStore
        .getState()
        .onPartUpdated(
          props.part as Part,
          typeof props.delta === "string" ? props.delta : undefined,
        );
      break;
    case "message.part.removed":
      if (
        !hasString(props, "sessionID") ||
        !hasString(props, "messageID") ||
        !hasString(props, "partID")
      )
        break;
      useMessageStore
        .getState()
        .onPartRemoved(
          props.sessionID as string,
          props.messageID as string,
          props.partID as string,
        );
      break;

    case "session.created":
      if (!hasObject(props, "info")) break;
      useSessionStore.getState().onSessionCreated(props.info as Session);
      break;
    case "session.updated":
      if (!hasObject(props, "info")) break;
      useSessionStore.getState().onSessionUpdated(props.info as Session);
      break;
    case "session.deleted":
      if (!hasObject(props, "info")) break;
      useSessionStore.getState().onSessionDeleted(props.info as Session);
      break;
    case "session.status":
      if (!hasString(props, "sessionID") || !hasObject(props, "status")) break;
      useSessionStore
        .getState()
        .onSessionStatus(
          props.sessionID as string,
          props.status as SessionStatus,
        );
      break;
    case "session.idle":
      if (!hasString(props, "sessionID")) break;
      useSessionStore.getState().onSessionIdle(props.sessionID as string);
      break;
    case "session.diff":
      if (!hasString(props, "sessionID") || !hasArray(props, "diff")) break;
      useSessionStore
        .getState()
        .onSessionDiff(props.sessionID as string, props.diff as FileDiff[]);
      break;
    case "session.error":
      if (!hasString(props, "sessionID")) break;
      void useMessageStore
        .getState()
        .handleSessionError(props.sessionID as string, props.error)
        .then((handled) => {
          if (!handled) {
            useSessionStore
              .getState()
              .onSessionError(props.sessionID as string, props.error);
          }
        })
        .catch(() => {
          useSessionStore
            .getState()
            .onSessionError(props.sessionID as string, props.error);
        });
      break;
    case "todo.updated":
      if (!hasString(props, "sessionID") || !hasArray(props, "todos")) break;
      useSessionStore
        .getState()
        .onTodoUpdated(props.sessionID as string, props.todos as Todo[]);
      break;

    default:
      break;
  }

  // NOTE(victor): permission and question events are not in the SDK Event union but the server sends them.
  const raw = event as { type: string; properties: Record<string, unknown> };
  if (raw.type === "permission.asked") {
    if (hasString(raw.properties, "id")) {
      usePermissionStore
        .getState()
        .onPermissionAsked(raw.properties as unknown as PermissionRequest);
    }
  } else if (raw.type === "permission.replied") {
    if (hasString(raw.properties, "requestID")) {
      usePermissionStore
        .getState()
        .onPermissionReplied(raw.properties.requestID as string);
    }
  } else if (raw.type === "question.asked") {
    if (hasObject(raw.properties, "questions")) {
      useQuestionStore
        .getState()
        .onQuestionAsked(raw.properties as unknown as QuestionRequest);
    }
  } else if (raw.type === "question.replied") {
    if (hasString(raw.properties, "requestID")) {
      useQuestionStore
        .getState()
        .onQuestionReplied(raw.properties.requestID as string);
    }
  } else if (raw.type === "question.rejected") {
    if (hasString(raw.properties, "requestID")) {
      useQuestionStore
        .getState()
        .onQuestionRejected(raw.properties.requestID as string);
    }
  } else if (raw.type === "message.part.delta") {
    if (
      hasString(raw.properties, "messageID") &&
      hasString(raw.properties, "partID") &&
      hasString(raw.properties, "field") &&
      hasString(raw.properties, "delta")
    ) {
      useMessageStore
        .getState()
        .onPartDelta(
          raw.properties.messageID as string,
          raw.properties.partID as string,
          raw.properties.field as string,
          raw.properties.delta as string,
        );
    }
  }
}

interface EventState {
  connect: () => void;
  disconnect: () => void;
}

export const useEventStore = create<EventState>(() => {
  let abortController: AbortController | null = null;
  let stopped = true;
  const reconnectDelayMs = 250;

  async function startSSE() {
    abortController?.abort();
    const ac = new AbortController();
    abortController = ac;

    try {
      const res = await fetch("/api/opencode/event", {
        signal: ac.signal,
      });
      if (!res.ok || !res.body) throw new Error(`SSE failed: ${res.status}`);

      const reader = res.body.pipeThrough(new TextDecoderStream()).getReader();
      let buffer = "";

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        buffer += value;

        const parts = buffer.split("\n\n");
        buffer = parts.pop() || "";

        for (const part of parts) {
          const dataLine = part.split("\n").find((l) => l.startsWith("data:"));
          if (!dataLine) continue;
          try {
            const event = JSON.parse(dataLine.slice(5).trim());
            dispatchEvent(event);
          } catch {
            /* ignore parse errors */
          }
        }
      }
    } catch {
      if (ac.signal.aborted) return;
    }

    if (!stopped) {
      setTimeout(() => {
        if (!stopped) startSSE();
      }, reconnectDelayMs);
    }
  }

  return {
    connect: () => {
      if (abortController && !abortController.signal.aborted) return;
      stopped = false;
      startSSE();
    },
    disconnect: () => {
      stopped = true;
      abortController?.abort();
      abortController = null;
    },
  };
});
