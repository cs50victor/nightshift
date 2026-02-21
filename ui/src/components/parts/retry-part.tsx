"use client";

import type { RetryPart } from "@opencode-ai/sdk";
import { Badge } from "@/components/ui/badge";

export function RetryPartView({ part }: { part: RetryPart }) {
  const errorMessage =
    typeof part.error === "object" && part.error !== null
      ? "message" in part.error
        ? String(part.error.message)
        : JSON.stringify(part.error)
      : String(part.error);

  return (
    <div className="flex items-center gap-2 rounded-md bg-warning-subtle/50 px-3 py-1.5 text-xs text-warning-subtle-fg">
      <Badge intent="warning">Retrying (attempt {part.attempt})</Badge>
      <span className="truncate">{errorMessage}</span>
    </div>
  );
}
