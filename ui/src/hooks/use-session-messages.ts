"use client";
import type {
  Message,
  Part,
  TextPart,
  ToolPart,
  ToolState,
} from "@opencode-ai/sdk";
import useSWR, { mutate } from "swr";

export type { Message, Part, TextPart, ToolPart, ToolState };

export interface MessageWithParts {
  info: Message;
  parts: Part[];
  isQueued?: boolean;
}

const fetcher = async (url: string): Promise<MessageWithParts[]> => {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error("Failed to fetch messages");
  }
  const data = await response.json();
  return data || [];
};

export function useSessionMessages(sessionId: string | undefined) {
  const key = sessionId ? `/api/opencode/session/${sessionId}/message` : null;

  const {
    data,
    error,
    isLoading,
    mutate: boundMutate,
  } = useSWR<MessageWithParts[]>(key, fetcher, {
    refreshInterval: 3000,
    keepPreviousData: true,
    revalidateOnFocus: false,
  });

  return {
    messages: data || [],
    error,
    isLoading,
    mutate: boundMutate,
  };
}

export function getMessagesKey(sessionId: string) {
  return `/api/opencode/session/${sessionId}/message`;
}

export function mutateSessionMessages(sessionId: string) {
  mutate(getMessagesKey(sessionId));
}

export function addOptimisticMessage(
  sessionId: string,
  message: MessageWithParts,
): () => void {
  const key = getMessagesKey(sessionId);
  let previousMessages: MessageWithParts[] = [];

  mutate(
    key,
    (current: MessageWithParts[] | undefined) => {
      previousMessages = current || [];
      return [...previousMessages, message];
    },
    { revalidate: false },
  );

  return () => {
    mutate(key, previousMessages, { revalidate: false });
  };
}

export function updateOptimisticMessage(
  sessionId: string,
  messageId: string,
  updates: Partial<MessageWithParts>,
) {
  const key = getMessagesKey(sessionId);

  mutate(
    key,
    (current: MessageWithParts[] | undefined) => {
      if (!current) return current;
      return current.map((m) =>
        m.info.id === messageId ? { ...m, ...updates } : m,
      );
    },
    { revalidate: false },
  );
}

export function removeOptimisticMessage(sessionId: string, messageId: string) {
  const key = getMessagesKey(sessionId);

  mutate(
    key,
    (current: MessageWithParts[] | undefined) => {
      if (!current) return current;
      return current.filter((m) => m.info.id !== messageId);
    },
    { revalidate: false },
  );
}
