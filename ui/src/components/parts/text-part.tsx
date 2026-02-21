"use client";

import type { TextPart } from "@opencode-ai/sdk";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";

export function TextPartView({ part }: { part: TextPart }) {
  if (!part.text?.trim()) return null;
  if (part.ignored) return null;

  return (
    <div
      className={`prose prose-sm dark:prose-invert max-w-none overflow-x-hidden ${part.synthetic ? "text-muted-fg italic" : ""}`}
    >
      <Markdown remarkPlugins={[remarkGfm]}>{part.text}</Markdown>
    </div>
  );
}
