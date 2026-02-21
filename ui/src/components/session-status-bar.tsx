"use client";

import { Button } from "@/components/ui/button";
import { Loader } from "@/components/ui/loader";
import { useSessionStore } from "@/stores/session-store";

interface SessionStatusBarProps {
  sessionID: string;
}

function formatSessionError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object") {
    const record = error as Record<string, unknown>;
    if (typeof record.message === "string") return record.message;
    if (record.error && typeof record.error === "object") {
      const nested = record.error as Record<string, unknown>;
      if (typeof nested.message === "string") return nested.message;
    }
    try {
      return JSON.stringify(error);
    } catch {
      return String(error);
    }
  }
  return "An unknown session error occurred.";
}

export function SessionStatusBar({ sessionID }: SessionStatusBarProps) {
  const status = useSessionStore((s) => s.getSessionStatus(sessionID));
  const sessionError = useSessionStore((s) => s.sessionErrors[sessionID]);
  const abortSession = useSessionStore((s) => s.abortSession);

  if (sessionError) {
    return (
      <div className="px-3 py-2 text-sm text-danger-subtle-fg bg-danger-subtle border-y border-danger-subtle-fg/20">
        {formatSessionError(sessionError)}
      </div>
    );
  }

  if (status.type === "idle") return null;

  if (status.type === "busy") {
    return (
      <div className="flex items-center gap-2 px-3 py-2 text-sm text-muted-fg">
        <Loader variant="spin" className="size-3.5" />
        <span>Working...</span>
        <Button
          intent="outline"
          size="xs"
          onPress={() => abortSession(sessionID)}
        >
          Stop
        </Button>
      </div>
    );
  }

  if (status.type === "retry") {
    return (
      <div className="flex items-center gap-2 px-3 py-2 text-sm text-warning-subtle-fg">
        <Loader variant="spin" className="size-3.5" />
        <span>Retrying (attempt {status.attempt})...</span>
      </div>
    );
  }

  return null;
}
