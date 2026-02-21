"use client";
import { SparklesIcon, UserIcon } from "@heroicons/react/24/solid";
import { useParams } from "next/navigation";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { type Attachment, ChatInput } from "@/components/chat-input";
import { PartRenderer } from "@/components/parts";
import { PermissionDialog } from "@/components/permission-dialog";
import { QuestionDialog } from "@/components/question-dialog";
import { SessionDiffInline } from "@/components/session-diff-inline";
import { SessionStatusBar } from "@/components/session-status-bar";
import { SessionTodos } from "@/components/session-todos";
import { Badge } from "@/components/ui/badge";
import { Loader } from "@/components/ui/loader";
import { useBreadcrumb } from "@/contexts/breadcrumb-context";
import { useAgentStore } from "@/stores/agent-store";
import { type MessageWithParts, useMessageStore } from "@/stores/message-store";
import { useModelStore } from "@/stores/model-store";
import { useSessionStore } from "@/stores/session-store";

interface QueuedMessage {
  id: string;
  text: string;
  attachments?: Attachment[];
}

const VISIBLE_PART_TYPES = new Set([
  "text",
  "reasoning",
  "tool",
  "file",
  "subtask",
  "patch",
  "retry",
  "agent",
  "compaction",
]);

function hasVisibleParts(msg: MessageWithParts): boolean {
  return msg.parts.some((p) => VISIBLE_PART_TYPES.has(p.type));
}

const EMPTY_MESSAGES: MessageWithParts[] = [];

export default function SessionPage() {
  const params = useParams();
  const sessionId = params.id as string;

  const messages =
    useMessageStore((s) => s.messagesBySession[sessionId]) ?? EMPTY_MESSAGES;
  const session = useSessionStore((s) => s.getSession(sessionId));
  const selectedModel = useModelStore((s) => s.selectedModel);
  const selectedAgent = useAgentStore((s) => s.getSelectedAgent(sessionId));
  const { setPageTitle } = useBreadcrumb();

  const visibleMessages = useMemo(
    () => messages.filter(hasVisibleParts),
    [messages],
  );

  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const [hasScrolledInitially, setHasScrolledInitially] = useState(false);
  const messageQueueRef = useRef<QueuedMessage[]>([]);
  const isProcessingQueue = useRef(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const chatContainerRef = useRef<HTMLDivElement>(null);
  const isNearBottomRef = useRef(true);
  const prevMessagesLengthRef = useRef(0);

  useEffect(() => {
    setLoading(true);
    setError(null);
    useMessageStore
      .getState()
      .loadMessages(sessionId)
      .catch((err: unknown) =>
        setError(err instanceof Error ? err.message : String(err)),
      )
      .finally(() => setLoading(false));
  }, [sessionId]);

  useEffect(() => {
    if (session?.title) {
      const formatted = session.title.replace(
        /\d{4}-\d{2}-\d{2}T[\d:.]+Z$/,
        (iso: string) => new Date(iso).toLocaleString(),
      );
      setPageTitle(formatted);
    }
    return () => setPageTitle(null);
  }, [session?.title, setPageTitle]);

  const scrollToBottom = useCallback(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, []);

  const checkIfNearBottom = useCallback(() => {
    const container = chatContainerRef.current;
    if (!container) return true;
    const threshold = 100;
    const isNear =
      container.scrollHeight - container.scrollTop - container.clientHeight <
      threshold;
    isNearBottomRef.current = isNear;
    return isNear;
  }, []);

  useEffect(() => {
    const container = chatContainerRef.current;
    if (!container) return;
    const handleScroll = () => checkIfNearBottom();
    container.addEventListener("scroll", handleScroll, { passive: true });
    return () => container.removeEventListener("scroll", handleScroll);
  }, [checkIfNearBottom]);

  useEffect(() => {
    if (
      messages.length > prevMessagesLengthRef.current &&
      isNearBottomRef.current
    ) {
      setTimeout(scrollToBottom, 50);
    }
    prevMessagesLengthRef.current = messages.length;
  }, [messages.length, scrollToBottom]);

  useEffect(() => {
    if (!hasScrolledInitially && !loading && messages.length > 0) {
      setTimeout(() => {
        scrollToBottom();
        setHasScrolledInitially(true);
        isNearBottomRef.current = true;
      }, 100);
    }
  }, [hasScrolledInitially, loading, messages.length, scrollToBottom]);

  // biome-ignore lint/correctness/useExhaustiveDependencies: intentionally resets scroll state when session changes
  useEffect(() => {
    setHasScrolledInitially(false);
    isNearBottomRef.current = true;
  }, [sessionId]);

  const sendMessage = useCallback(
    async (
      messageText: string,
      messageId: string,
      attachments?: Attachment[],
    ) => {
      if (!sessionId) return;
      try {
        await useMessageStore.getState().sendMessage(sessionId, {
          text: messageText,
          model: selectedModel,
          agent: selectedAgent,
          attachments,
        });
        useMessageStore
          .getState()
          .removeOptimisticMessage(sessionId, messageId);
        isNearBottomRef.current = true;
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to send message");
        useMessageStore
          .getState()
          .removeOptimisticMessage(sessionId, messageId);
      }
    },
    [sessionId, selectedModel, selectedAgent],
  );

  const processQueue = useCallback(async () => {
    if (isProcessingQueue.current || !sessionId) return;
    isProcessingQueue.current = true;
    setSending(true);

    while (messageQueueRef.current.length > 0) {
      const next = messageQueueRef.current.shift();
      if (!next) {
        continue;
      }
      await sendMessage(next.text, next.id, next.attachments);
    }

    isProcessingQueue.current = false;
    setSending(false);
  }, [sessionId, sendMessage]);

  const handleSubmit = (e: React.FormEvent, attachments?: Attachment[]) => {
    e.preventDefault();
    const messageText = input.trim();
    if ((!messageText && !attachments?.length) || !sessionId) return;

    setInput("");
    setError(null);

    const attachmentCount = attachments?.length ?? 0;
    const optimisticText = messageText || `[${attachmentCount} attachment(s)]`;
    const messageId = useMessageStore
      .getState()
      .addOptimisticMessage(sessionId, optimisticText);
    messageQueueRef.current.push({
      id: messageId,
      text: messageText,
      attachments,
    });
    processQueue();

    isNearBottomRef.current = true;
    scrollToBottom();
  };

  return (
    <div className="flex h-full flex-col -my-4 mx-auto w-full max-w-5xl">
      <div
        className="flex-1 overflow-auto overflow-x-hidden"
        ref={chatContainerRef}
      >
        {loading && (
          <div className="flex items-center justify-center py-8">
            <Loader className="size-6" />
          </div>
        )}

        {error && (
          <div className="rounded-md bg-danger-subtle p-4 m-4 text-danger-subtle-fg">
            Error: {error}
          </div>
        )}

        {!loading && !error && messages.length === 0 && (
          <div className="flex h-full items-center justify-center">
            <div className="text-center text-muted-fg">No messages yet</div>
          </div>
        )}

        <div className="divide-y divide-dashed divide-border overflow-x-hidden">
          {visibleMessages.map((message) => (
            <div key={message.info.id} className="py-3 px-6">
              <div className="flex gap-2">
                {message.info.role === "assistant" ? (
                  <SparklesIcon className="size-4 shrink-0 mt-1 text-muted-fg" />
                ) : (
                  <UserIcon className="size-4 shrink-0 mt-1 text-muted-fg" />
                )}
                <div className="flex-1 min-w-0">
                  {message.isOptimistic && (
                    <Badge intent="warning" className="mb-1">
                      Queued
                    </Badge>
                  )}
                  <div className="space-y-1">
                    {message.parts.map((part) => (
                      <PartRenderer key={part.id} part={part} />
                    ))}
                  </div>
                </div>
              </div>
            </div>
          ))}
          <div ref={messagesEndRef} />
        </div>

        <SessionStatusBar sessionID={sessionId} />
      </div>

      <SessionTodos sessionID={sessionId} />
      <SessionDiffInline sessionID={sessionId} />
      <PermissionDialog sessionID={sessionId} />
      <QuestionDialog sessionID={sessionId} />

      <ChatInput
        sessionId={sessionId}
        input={input}
        onInputChange={setInput}
        onSubmit={handleSubmit}
        sending={sending}
      />
    </div>
  );
}
