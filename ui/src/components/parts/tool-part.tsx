"use client";

import {
  EyeIcon,
  MagnifyingGlassIcon,
  PencilIcon,
  PencilSquareIcon,
} from "@heroicons/react/24/outline";
import type {
  FilePart,
  ToolPart,
  ToolStateCompleted,
  ToolStateError,
} from "@opencode-ai/sdk";
import { useState } from "react";
import { Badge } from "@/components/ui/badge";

function formatToolCall(part: ToolPart): {
  icon: React.ReactNode;
  label: string;
  details?: string;
} {
  const toolName = part.tool?.toLowerCase() || "";
  const input = ("input" in part.state ? part.state.input : {}) as Record<
    string,
    unknown
  >;

  switch (toolName) {
    case "edit": {
      const filePath = input.filePath || input.file || "";
      const oldStr = String(input.oldString || "");
      const newStr = String(input.newString || "");
      const additions = newStr.split("\n").length;
      const deletions = oldStr.split("\n").length;
      return {
        icon: <PencilIcon className="size-3" />,
        label: `edit ${filePath}`,
        details: `(+${additions}-${deletions})`,
      };
    }
    case "read": {
      const filePath = input.filePath || input.file || "";
      return {
        icon: <EyeIcon className="size-3" />,
        label: `read ${filePath}`,
      };
    }
    case "write": {
      const filePath = input.filePath || input.file || "";
      const content = String(input.content || "");
      const lines = content.split("\n").length;
      return {
        icon: <PencilSquareIcon className="size-3" />,
        label: `write ${filePath}`,
        details: `(${lines} lines)`,
      };
    }
    case "bash": {
      const command = String(input.command || input.cmd || "");
      const shortCmd = command.split("\n")[0]?.slice(0, 50) || "";
      return {
        icon: <span className="font-mono text-[10px]">$</span>,
        label: `bash ${shortCmd}${command.length > 50 ? "..." : ""}`,
        details: input.description ? `# ${input.description}` : undefined,
      };
    }
    case "glob": {
      const pattern = input?.pattern || "";
      const path = input?.path || "";
      return {
        icon: <MagnifyingGlassIcon className="size-3" />,
        label: `glob ${pattern}`,
        details: path ? `in ${path}` : undefined,
      };
    }
    case "grep": {
      const pattern = input.pattern || "";
      const path = input.path || "";
      return {
        icon: <MagnifyingGlassIcon className="size-3" />,
        label: `grep "${pattern}"`,
        details: path ? `in ${path}` : undefined,
      };
    }
    default: {
      const firstArg = Object.entries(input)[0];
      return {
        icon: <span className="text-[10px]">&#9724;</span>,
        label: toolName || "unknown",
        details: firstArg
          ? `${firstArg[0]}: ${String(firstArg[1]).slice(0, 30)}...`
          : undefined,
      };
    }
  }
}

function statusBadge(status: string) {
  switch (status) {
    case "pending":
      return <Badge intent="secondary">Pending</Badge>;
    case "running":
      return <Badge intent="info">Running</Badge>;
    case "completed":
      return <Badge intent="success">Completed</Badge>;
    case "error":
      return <Badge intent="danger">Error</Badge>;
    default:
      return null;
  }
}

function formatDuration(start: number, end: number): string {
  const ms = end - start;
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function AttachmentList({ attachments }: { attachments: FilePart[] }) {
  return (
    <div className="mt-2 space-y-1">
      {attachments.map((file) => (
        <div key={file.id} className="text-xs text-muted-fg">
          {file.filename || "attachment"} ({file.mime})
        </div>
      ))}
    </div>
  );
}

export function ToolCallPartView({ part }: { part: ToolPart }) {
  const [expanded, setExpanded] = useState(false);
  const { icon, label, details } = formatToolCall(part);
  const { state } = part;

  const title = "title" in state ? state.title : undefined;

  return (
    <div className="rounded-md border border-border/50">
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="flex w-full items-center gap-1.5 px-2 py-1 text-xs font-mono min-w-0 hover:bg-secondary/50 transition-colors"
      >
        <span className="opacity-60 shrink-0">{icon}</span>
        <span className="truncate">{title || label}</span>
        {details && (
          <span className="opacity-60 shrink-0 hidden sm:inline">
            {details}
          </span>
        )}
        <span className="ml-auto shrink-0">{statusBadge(state.status)}</span>
      </button>
      {expanded && (
        <div className="border-t border-border/50 px-2 py-2 space-y-2 text-xs">
          {"input" in state && Object.keys(state.input).length > 0 && (
            <div>
              <div className="font-medium text-muted-fg mb-1">Input</div>
              <pre className="overflow-auto max-h-48 rounded bg-secondary/50 p-2 text-[11px]">
                {JSON.stringify(state.input, null, 2)}
              </pre>
            </div>
          )}
          {state.status === "completed" && (
            <>
              <div>
                <div className="font-medium text-muted-fg mb-1">Output</div>
                <pre className="overflow-auto max-h-48 rounded bg-secondary/50 p-2 text-[11px] whitespace-pre-wrap">
                  {(state as ToolStateCompleted).output}
                </pre>
              </div>
              <div className="text-muted-fg">
                {formatDuration(
                  (state as ToolStateCompleted).time.start,
                  (state as ToolStateCompleted).time.end,
                )}
              </div>
              {(state as ToolStateCompleted).attachments?.length ? (
                <AttachmentList
                  attachments={
                    (state as ToolStateCompleted).attachments as FilePart[]
                  }
                />
              ) : null}
            </>
          )}
          {state.status === "error" && (
            <div>
              <div className="font-medium text-danger mb-1">Error</div>
              <pre className="overflow-auto max-h-48 rounded bg-danger-subtle/50 p-2 text-[11px] whitespace-pre-wrap text-danger">
                {(state as ToolStateError).error}
              </pre>
            </div>
          )}
          {state.status === "running" && "time" in state && (
            <div className="text-muted-fg">
              Started {new Date(state.time.start).toLocaleTimeString()}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
