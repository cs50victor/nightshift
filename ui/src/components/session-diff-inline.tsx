"use client";

import Link from "next/link";
import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { useSessionStore } from "@/stores/session-store";

interface SessionDiffInlineProps {
  sessionID: string;
}

function inferStatus(diff: { before: string; after: string }): string {
  if (!diff.before) return "added";
  if (!diff.after) return "deleted";
  return "modified";
}

const STATUS_INTENT: Record<string, "success" | "danger" | "warning"> = {
  added: "success",
  deleted: "danger",
  modified: "warning",
};

export function SessionDiffInline({ sessionID }: SessionDiffInlineProps) {
  const diffs = useSessionStore((s) => s.sessionDiffs[sessionID]);
  const [expanded, setExpanded] = useState(false);

  if (!diffs || diffs.length === 0) return null;

  const totalAdditions = diffs.reduce((sum, d) => sum + d.additions, 0);
  const totalDeletions = diffs.reduce((sum, d) => sum + d.deletions, 0);

  return (
    <div className="rounded-lg border border-border bg-secondary/30">
      <button
        type="button"
        className="flex w-full items-center justify-between px-3 py-2 text-sm text-fg hover:bg-secondary/50"
        onClick={() => setExpanded(!expanded)}
      >
        <span className="font-medium">
          {diffs.length} file{diffs.length !== 1 ? "s" : ""} changed
          <span className="ml-2 text-success">+{totalAdditions}</span>
          <span className="ml-1 text-danger">-{totalDeletions}</span>
        </span>
        <span className="text-xs text-muted-fg">{expanded ? "▲" : "▼"}</span>
      </button>
      {expanded && (
        <div className="flex flex-col gap-1 border-t border-border px-3 py-2">
          {diffs.map((diff) => (
            <div
              key={diff.file}
              className="flex items-center gap-2 text-sm text-fg"
            >
              <Badge
                intent={STATUS_INTENT[inferStatus(diff)] ?? "warning"}
                className="min-w-[4rem] justify-center"
              >
                {inferStatus(diff)}
              </Badge>
              <span className="truncate font-mono text-xs">{diff.file}</span>
              <span className="ml-auto shrink-0 text-xs text-muted-fg">
                <span className="text-success">+{diff.additions}</span>{" "}
                <span className="text-danger">-{diff.deletions}</span>
              </span>
            </div>
          ))}
          <Link
            href="/diff"
            className="mt-1 text-xs text-primary hover:underline"
          >
            View full diff
          </Link>
        </div>
      )}
    </div>
  );
}
