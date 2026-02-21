"use client";
import {
  ArchiveBoxIcon,
  ArrowPathIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  ExclamationTriangleIcon,
} from "@heroicons/react/24/outline";
import { parsePatchFiles } from "@pierre/diffs";
import { FileDiff } from "@pierre/diffs/react";
import { useEffect, useMemo, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Loader } from "@/components/ui/loader";
import { useBreadcrumb } from "@/contexts/breadcrumb-context";
import { useMemberDiff, useMemberTools, useTeams } from "@/lib/api";
import type {
  ConflictInfo,
  MemberSummary,
  TaskSummary,
  TeamSummary,
  ToolCall,
} from "@/lib/types";

const DIFF_OPTIONS = {
  diffStyle: "unified" as const,
  diffIndicators: "bars" as const,
  theme: "github-dark" as const,
};

function StatusBadge({ status }: { status: string }) {
  const intent =
    status === "completed"
      ? "success"
      : status === "in_progress"
        ? "info"
        : "secondary";
  const label = status === "in_progress" ? "in progress" : status;
  return <Badge intent={intent}>{label}</Badge>;
}

function ToolIcon({ tool }: { tool: string }) {
  const lower = tool.toLowerCase();
  const colors: Record<string, string> = {
    read: "text-blue-400",
    write: "text-green-400",
    edit: "text-yellow-400",
    bash: "text-orange-400",
    glob: "text-purple-400",
    grep: "text-pink-400",
    task: "text-cyan-400",
  };
  return (
    <span
      className={`font-mono text-xs font-medium ${colors[lower] ?? "text-muted-fg"}`}
    >
      {tool}
    </span>
  );
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function formatTimestamp(ts: number): string {
  const d = new Date(ts);
  return d.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function MemberToolsPanel({ team, name }: { team: string; name: string }) {
  const { data, isLoading } = useMemberTools(team, name);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-8">
        <Loader className="size-5" />
      </div>
    );
  }

  if (!data || data.toolCalls.length === 0) {
    return (
      <div className="py-4 text-center text-sm text-muted-fg">
        No tool calls recorded
      </div>
    );
  }

  return (
    <div className="space-y-3">
      <div className="flex flex-wrap gap-2 text-xs font-mono">
        <span className="text-muted-fg">{data.stats.total} total</span>
        {data.stats.reads > 0 && (
          <span className="text-blue-400">{data.stats.reads} reads</span>
        )}
        {data.stats.writes > 0 && (
          <span className="text-green-400">{data.stats.writes} writes</span>
        )}
        {data.stats.edits > 0 && (
          <span className="text-yellow-400">{data.stats.edits} edits</span>
        )}
        {data.stats.bash > 0 && (
          <span className="text-orange-400">{data.stats.bash} bash</span>
        )}
        {data.stats.other > 0 && (
          <span className="text-muted-fg">{data.stats.other} other</span>
        )}
      </div>

      <div className="max-h-80 overflow-auto space-y-1">
        {data.toolCalls.map((call: ToolCall, i: number) => (
          <div
            key={`${call.timestamp}-${i}`}
            className="flex items-center gap-2 rounded px-2 py-1 text-xs hover:bg-secondary/50"
          >
            <span className="shrink-0 text-muted-fg font-mono w-16">
              {formatTimestamp(call.timestamp)}
            </span>
            <ToolIcon tool={call.tool} />
            <span className="truncate text-fg flex-1" title={call.inputSummary}>
              {call.title || call.inputSummary}
            </span>
            {call.status === "error" && <Badge intent="danger">err</Badge>}
            {call.durationMs != null && (
              <span className="shrink-0 text-muted-fg font-mono">
                {formatDuration(call.durationMs)}
              </span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

function MemberDiffPanel({ team, name }: { team: string; name: string }) {
  const { data, isLoading } = useMemberDiff(team, name);

  const files = useMemo(() => {
    if (!data?.diff) return [];
    try {
      const patches = parsePatchFiles(data.diff);
      return patches.flatMap((patch) => patch.files);
    } catch {
      return [];
    }
  }, [data?.diff]);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-8">
        <Loader className="size-5" />
      </div>
    );
  }

  if (!data?.diff || files.length === 0) {
    return (
      <div className="py-4 text-center text-sm text-muted-fg">
        No changes detected
      </div>
    );
  }

  return (
    <div>
      <div className="mb-2 flex items-center gap-2 text-xs font-mono text-muted-fg">
        <span>
          {data.baselineCommit?.slice(0, 7) ?? "?"}..
          {data.currentCommit?.slice(0, 7) ?? "HEAD"}
        </span>
      </div>
      <div className="overflow-auto">
        {files.map((file, index) => (
          <FileDiff
            key={file.name || file.prevName || index}
            fileDiff={file}
            options={DIFF_OPTIONS}
          />
        ))}
      </div>
    </div>
  );
}

function MemberRow({
  member,
  teamName,
}: {
  member: MemberSummary;
  teamName: string;
}) {
  const [expanded, setExpanded] = useState(false);
  const [tab, setTab] = useState<"diff" | "tools">("diff");

  const ds = member.diffSummary;

  return (
    <div className="border-b border-border last:border-b-0">
      <button
        type="button"
        className="flex w-full items-center gap-3 px-3 py-2 text-left text-sm hover:bg-secondary/30"
        onClick={() => setExpanded(!expanded)}
      >
        {expanded ? (
          <ChevronDownIcon className="size-3.5 shrink-0 text-muted-fg" />
        ) : (
          <ChevronRightIcon className="size-3.5 shrink-0 text-muted-fg" />
        )}
        <span
          className="size-2.5 rounded-full shrink-0"
          style={{ backgroundColor: member.color || "#888" }}
        />
        <span className="font-medium">{member.name}</span>
        <Badge intent="secondary" isCircle={false}>
          {member.agentType}
        </Badge>
        {!member.isActive && (
          <span className="text-xs text-muted-fg">(inactive)</span>
        )}
        <span className="ml-auto flex items-center gap-2 text-xs font-mono text-muted-fg">
          {ds && ds.filesChanged > 0 && (
            <>
              <span>
                {ds.filesChanged} {ds.filesChanged === 1 ? "file" : "files"}
              </span>
              <span className="text-success">+{ds.additions}</span>
              <span className="text-danger">-{ds.deletions}</span>
            </>
          )}
          {(!ds || ds.filesChanged === 0) && <span>no changes</span>}
        </span>
      </button>

      {expanded && (
        <div className="border-t border-border bg-bg px-4 py-3">
          <div className="mb-3 flex items-center gap-1 text-xs">
            <button
              type="button"
              className={`rounded px-2.5 py-1 ${tab === "diff" ? "bg-secondary text-fg" : "text-muted-fg hover:text-fg"}`}
              onClick={() => setTab("diff")}
            >
              Diff
            </button>
            <button
              type="button"
              className={`rounded px-2.5 py-1 ${tab === "tools" ? "bg-secondary text-fg" : "text-muted-fg hover:text-fg"}`}
              onClick={() => setTab("tools")}
            >
              Tools
            </button>
            <span
              className="ml-auto text-xs font-mono text-muted-fg truncate max-w-48"
              title={member.cwd}
            >
              {member.cwd}
            </span>
          </div>
          {tab === "diff" ? (
            <MemberDiffPanel team={teamName} name={member.name} />
          ) : (
            <MemberToolsPanel team={teamName} name={member.name} />
          )}
        </div>
      )}
    </div>
  );
}

function ConflictsWarning({ conflicts }: { conflicts: ConflictInfo[] }) {
  if (conflicts.length === 0) return null;
  return (
    <div className="rounded border border-warning/30 bg-warning/5 px-3 py-2">
      <div className="flex items-center gap-2 text-sm font-medium text-warning">
        <ExclamationTriangleIcon className="size-4" />
        {conflicts.length} file{" "}
        {conflicts.length === 1 ? "conflict" : "conflicts"}
      </div>
      <div className="mt-1 space-y-0.5">
        {conflicts.map((c) => (
          <div key={c.path} className="text-xs text-muted-fg">
            <span className="font-mono">{c.path}</span>
            <span className="ml-1">({c.members.join(", ")})</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function TaskRow({ task }: { task: TaskSummary }) {
  return (
    <div className="flex items-center gap-2 px-3 py-1.5 text-sm">
      <StatusBadge status={task.status} />
      <span className="flex-1 truncate">{task.subject}</span>
      {task.owner && (
        <span className="text-xs text-muted-fg font-mono">{task.owner}</span>
      )}
    </div>
  );
}

function TeamCard({ team }: { team: TeamSummary }) {
  const [showTasks, setShowTasks] = useState(false);

  const activeTasks = team.tasks.filter((t) => t.status !== "completed");
  const completedTasks = team.tasks.filter((t) => t.status === "completed");

  return (
    <div className="rounded-lg border border-border overflow-hidden">
      <div className="flex items-center gap-3 border-b border-border px-4 py-3">
        <h2 className="text-base font-semibold">{team.name}</h2>
        {team.archived && (
          <Badge intent="secondary">
            <ArchiveBoxIcon className="size-3" data-slot="icon" />
            archived
          </Badge>
        )}
        {team.description && (
          <span className="text-sm text-muted-fg truncate">
            {team.description}
          </span>
        )}
        <span className="ml-auto text-xs text-muted-fg font-mono">
          {team.members.length}{" "}
          {team.members.length === 1 ? "member" : "members"}
        </span>
      </div>

      <ConflictsWarning conflicts={team.conflicts} />

      <div>
        {team.members.map((m) => (
          <MemberRow key={m.name} member={m} teamName={team.name} />
        ))}
      </div>

      {team.tasks.length > 0 && (
        <div className="border-t border-border">
          <button
            type="button"
            className="flex w-full items-center gap-2 px-4 py-2 text-left text-xs font-medium text-muted-fg hover:text-fg"
            onClick={() => setShowTasks(!showTasks)}
          >
            {showTasks ? (
              <ChevronDownIcon className="size-3" />
            ) : (
              <ChevronRightIcon className="size-3" />
            )}
            Tasks ({activeTasks.length} active, {completedTasks.length} done)
          </button>
          {showTasks && (
            <div className="border-t border-border py-1">
              {activeTasks.map((t) => (
                <TaskRow key={t.id} task={t} />
              ))}
              {completedTasks.map((t) => (
                <TaskRow key={t.id} task={t} />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export default function TeamsPage() {
  const { teams, error, isLoading, refresh } = useTeams();
  const { setPageTitle } = useBreadcrumb();

  useEffect(() => {
    setPageTitle("Teams");
    return () => setPageTitle(null);
  }, [setPageTitle]);

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <Loader className="size-8" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4">
        <div className="text-danger">Error loading teams: {error.message}</div>
        <Button onPress={() => refresh()}>
          <ArrowPathIcon className="size-4" />
          Retry
        </Button>
      </div>
    );
  }

  if (teams.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4">
        <div className="text-muted-fg">No teams detected</div>
        <div className="text-xs text-muted-fg max-w-sm text-center">
          Teams appear when agents are spawned via claude-code-teams-mcp. The
          daemon watches ~/.claude/teams/ for changes.
        </div>
        <Button onPress={() => refresh()}>
          <ArrowPathIcon className="size-4" />
          Refresh
        </Button>
      </div>
    );
  }

  return (
    <div className="-m-4 flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-border px-4 py-3">
        <div className="flex items-center gap-4">
          <h1 className="text-lg font-semibold">Teams</h1>
          <span className="text-xs font-mono text-muted-fg">
            {teams.length} {teams.length === 1 ? "team" : "teams"}
          </span>
        </div>
        <Button intent="secondary" size="sm" onPress={() => refresh()}>
          <ArrowPathIcon className="size-4" />
          Refresh
        </Button>
      </div>

      <div className="flex-1 overflow-auto p-4 space-y-4">
        {teams.map((team) => (
          <TeamCard key={team.name} team={team} />
        ))}
      </div>
    </div>
  );
}
