"use client";

import type { Part } from "@opencode-ai/sdk";
import { memo } from "react";
import { AgentPartView } from "./agent-part";
import { CompactionPartView } from "./compaction-part";
import { FilePartView } from "./file-part";
import { PatchPartView } from "./patch-part";
import { ReasoningPartView } from "./reasoning-part";
import { RetryPartView } from "./retry-part";
import { SubtaskPartView } from "./subtask-part";
import { TextPartView } from "./text-part";
import { ToolCallPartView } from "./tool-part";

export const PartRenderer = memo(function PartRenderer({
  part,
}: {
  part: Part;
}) {
  switch (part.type) {
    case "text":
      return <TextPartView part={part} />;
    case "reasoning":
      return <ReasoningPartView part={part} />;
    case "tool":
      return <ToolCallPartView part={part} />;
    case "file":
      return <FilePartView part={part} />;
    case "subtask":
      return <SubtaskPartView part={part} />;
    case "patch":
      return <PatchPartView part={part} />;
    case "retry":
      return <RetryPartView part={part} />;
    case "agent":
      return <AgentPartView part={part} />;
    case "compaction":
      return <CompactionPartView part={part} />;
    case "step-start":
    case "snapshot":
      return null;
    default:
      return null;
  }
});
