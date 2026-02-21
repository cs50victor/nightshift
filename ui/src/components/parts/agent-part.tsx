"use client";

import type { AgentPart } from "@opencode-ai/sdk";
import { Badge } from "@/components/ui/badge";

export function AgentPartView({ part }: { part: AgentPart }) {
  return (
    <div className="py-0.5">
      <Badge intent="info">{part.name}</Badge>
    </div>
  );
}
