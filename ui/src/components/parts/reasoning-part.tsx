"use client";

import type { ReasoningPart } from "@opencode-ai/sdk";
import { useState } from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";

function formatDuration(start: number, end?: number): string {
  if (!end) return "...";
  const seconds = (end - start) / 1000;
  if (seconds < 1) return `${Math.round(seconds * 1000)}ms`;
  return `${seconds.toFixed(1)}s`;
}

export function ReasoningPartView({ part }: { part: ReasoningPart }) {
  const [expanded, setExpanded] = useState(false);

  if (!part.text?.trim()) return null;

  return (
    <div className="rounded-md border-l-2 border-muted-fg/30 bg-secondary/50">
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="flex w-full items-center gap-2 px-3 py-1.5 text-xs text-muted-fg hover:text-fg transition-colors"
      >
        <span className="font-medium">Thinking</span>
        <span className="opacity-60">
          {formatDuration(part.time.start, part.time.end)}
        </span>
        <span className="ml-auto opacity-40">
          {expanded ? "\u25B2" : "\u25BC"}
        </span>
      </button>
      {expanded && (
        <div className="px-3 pb-2 prose prose-sm dark:prose-invert max-w-none text-muted-fg">
          <Markdown remarkPlugins={[remarkGfm]}>{part.text}</Markdown>
        </div>
      )}
    </div>
  );
}
