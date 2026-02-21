"use client";

import type { Part } from "@opencode-ai/sdk";
import { Badge } from "@/components/ui/badge";

type SubtaskPart = Extract<Part, { type: "subtask" }>;

export function SubtaskPartView({ part }: { part: SubtaskPart }) {
  return (
    <div className="flex items-center gap-2 text-xs py-1">
      <Badge intent="info">{part.agent}</Badge>
      <span className="text-muted-fg">{part.description}</span>
    </div>
  );
}
