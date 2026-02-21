"use client";

import type { CompactionPart } from "@opencode-ai/sdk";

export function CompactionPartView({ part }: { part: CompactionPart }) {
  return (
    <div className="flex items-center gap-3 py-1.5 text-[11px] text-muted-fg">
      <div className="flex-1 border-t border-border/30" />
      <span>
        {part.auto ? "Context compacted" : "Context compacted (manual)"}
      </span>
      <div className="flex-1 border-t border-border/30" />
    </div>
  );
}
