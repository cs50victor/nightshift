"use client";
import {
  PaperAirplaneIcon,
  SparklesIcon,
  UserIcon,
} from "@heroicons/react/24/solid";
import { useParams } from "next/navigation";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AgentSelect } from "@/components/agent-select";
import {
  FileMentionPopover,
  useFileMention,
} from "@/components/file-mention-popover";
import { ModelSelect } from "@/components/model-select";
import { PartRenderer } from "@/components/parts";
import { PermissionDialog } from "@/components/permission-dialog";
import { QuestionDialog } from "@/components/question-dialog";
import { SessionDiffInline } from "@/components/session-diff-inline";
import { SessionStatusBar } from "@/components/session-status-bar";
import { SessionTodos } from "@/components/session-todos";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Loader } from "@/components/ui/loader";
import { Textarea } from "@/components/ui/textarea";
import { useBreadcrumb } from "@/contexts/breadcrumb-context";
import { useAgentStore } from "@/stores/agent-store";
import { type MessageWithParts, useMessageStore } from "@/stores/message-store";
import { useModelStore } from "@/stores/model-store";
import { useSessionStore } from "@/stores/session-store";

interface QueuedMessage {
  id: string;
  text: string;
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
  const [fileResults, setFileResults] = useState<string[]>([]);
  const messageQueueRef = useRef<QueuedMessage[]>([]);
  const isProcessingQueue = useRef(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const chatContainerRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const isNearBottomRef = useRef(true);
  const prevMessagesLengthRef = useRef(0);
  const fileMention = useFileMention();

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
    async (messageText: string, messageId: string) => {
      if (!sessionId) return;
      try {
        await useMessageStore.getState().sendMessage(sessionId, {
          text: messageText,
          model: selectedModel,
          agent: selectedAgent,
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
      const next = messageQueueRef.current.shift()!;
      await sendMessage(next.text, next.id);
    }

    isProcessingQueue.current = false;
    setSending(false);
  }, [sessionId, sendMessage]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!input.trim() || !sessionId) return;

    const messageText = input.trim();
    setInput("");
    setError(null);

    const messageId = useMessageStore
      .getState()
      .addOptimisticMessage(sessionId, messageText);
    messageQueueRef.current.push({ id: messageId, text: messageText });
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

      <div className="border-t border-border p-4 shrink-0 relative">
        <FileMentionPopover
          isOpen={fileMention.isOpen}
          searchQuery={fileMention.searchQuery}
          textareaRef={textareaRef}
          mentionStart={fileMention.mentionStart}
          selectedIndex={fileMention.selectedIndex}
          onSelectedIndexChange={fileMention.setSelectedIndex}
          onFilesChange={setFileResults}
          onClose={fileMention.close}
          onSelect={(filePath) => {
            const newValue = fileMention.handleSelect(filePath, input);
            setInput(newValue);
          }}
        />
        <form onSubmit={handleSubmit} className="w-full">
          <Textarea
            ref={textareaRef}
            value={input}
            onChange={(e) => {
              const value = e.target.value;
              setInput(value);
              if (fileMention.isOpen || value.includes("@")) {
                const cursorPos = e.target.selectionStart ?? value.length;
                fileMention.handleInputChange(value, cursorPos);
              }
            }}
            onInput={(e) => {
              const target = e.target as HTMLTextAreaElement;
              const value = target.value;
              if (value.includes("@")) {
                const cursorPos = target.selectionStart ?? value.length;
                fileMention.handleInputChange(value, cursorPos);
              }
            }}
            onSelect={(e) => {
              const target = e.target as HTMLTextAreaElement;
              if (fileMention.isOpen || input.includes("@")) {
                const cursorPos = target.selectionStart ?? input.length;
                fileMention.handleInputChange(input, cursorPos);
              }
            }}
            onKeyDown={(e) => {
              const handled = fileMention.handleKeyDown(e, fileResults.length);
              if (handled) {
                if (
                  (e.key === "Enter" || e.key === "Tab") &&
                  fileResults.length > 0
                ) {
                  const selectedFile = fileResults[fileMention.selectedIndex];
                  if (selectedFile) {
                    const newValue = fileMention.handleSelect(
                      selectedFile,
                      input,
                    );
                    setInput(newValue);
                  }
                }
                return;
              }
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                if (input.trim()) {
                  handleSubmit(e as unknown as React.FormEvent);
                }
              }
            }}
            placeholder="Type your message... (use @ to mention files)"
            className="w-full resize-none min-h-32 max-h-32 overflow-y-auto"
            rows={5}
          />
          <div className="mt-3 flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex items-center justify-between gap-2 sm:justify-start">
              <AgentSelect sessionId={sessionId} />
            </div>
            <div className="flex items-center justify-between gap-2 sm:justify-end">
              <ModelSelect />
              <Button
                type="submit"
                isDisabled={!input.trim()}
                className="min-w-32"
              >
                <PaperAirplaneIcon className="size-4" />
                {sending ? "Sending..." : "Send"}
              </Button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
}
