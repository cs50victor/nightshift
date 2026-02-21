"use client";

import type { FilePart } from "@opencode-ai/sdk";
import { Badge } from "@/components/ui/badge";

export function FilePartView({ part }: { part: FilePart }) {
  const isImage = part.mime.startsWith("image/");
  const filename = part.filename || "unnamed";

  return (
    <div className="rounded-md border border-border/50 p-2 space-y-2">
      <div className="flex items-center gap-2 text-xs">
        <span className="font-medium truncate">{filename}</span>
        <Badge intent="secondary">{part.mime}</Badge>
      </div>
      {isImage && (
        // biome-ignore lint/performance/noImgElement: external URL from SDK, next/image requires known domains
        <img
          src={part.url}
          alt={filename}
          className="max-w-full max-h-64 rounded"
        />
      )}
      {part.source?.text?.value && (
        <pre className="overflow-auto max-h-48 rounded bg-secondary/50 p-2 text-[11px]">
          {part.source.text.value}
        </pre>
      )}
    </div>
  );
}
