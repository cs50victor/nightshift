"use client";

import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { useSessionStore } from "@/stores/session-store";

interface SessionTodosProps {
  sessionID: string;
}

const STATUS_ICONS: Record<string, string> = {
  pending: "○",
  in_progress: "◐",
  completed: "●",
  cancelled: "✕",
};

const PRIORITY_INTENT: Record<
  string,
  "danger" | "warning" | "info" | "secondary"
> = {
  high: "danger",
  medium: "warning",
  low: "info",
};

export function SessionTodos({ sessionID }: SessionTodosProps) {
  const todos = useSessionStore((s) => s.sessionTodos[sessionID]);
  const [expanded, setExpanded] = useState(false);

  if (!todos || todos.length === 0) return null;

  const completed = todos.filter((t) => t.status === "completed").length;

  return (
    <div className="rounded-lg border border-border bg-secondary/30">
      <button
        type="button"
        className="flex w-full items-center justify-between px-3 py-2 text-sm font-medium text-fg hover:bg-secondary/50"
        onClick={() => setExpanded(!expanded)}
      >
        <span>
          Todos ({completed}/{todos.length})
        </span>
        <span className="text-xs text-muted-fg">{expanded ? "▲" : "▼"}</span>
      </button>
      {expanded && (
        <div className="flex flex-col gap-1 border-t border-border px-3 py-2">
          {todos.map((todo) => (
            <div key={todo.content} className="flex items-center gap-2 text-sm">
              <span
                className={
                  todo.status === "completed"
                    ? "text-success"
                    : todo.status === "in_progress"
                      ? "text-info"
                      : "text-muted-fg"
                }
              >
                {STATUS_ICONS[todo.status] ?? "○"}
              </span>
              <span
                className={
                  todo.status === "completed"
                    ? "text-muted-fg line-through"
                    : "text-fg"
                }
              >
                {todo.content}
              </span>
              {todo.priority && todo.priority !== "none" && (
                <Badge
                  intent={PRIORITY_INTENT[todo.priority] ?? "secondary"}
                  className="ml-auto"
                >
                  {todo.priority}
                </Badge>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
