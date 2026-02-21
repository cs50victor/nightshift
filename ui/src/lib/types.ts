// NOTE(victor): Must stay in sync with daemon/src/nodes.rs NodeInfo
export interface Node {
  id: string;
  name: string;
  url: string;
  startedAt: string;
  os: string;
  arch: string;
  daemonVersion: string;
  machineName?: string;
}

// NOTE(victor): Must stay in sync with daemon/src/teams.rs API response types

export interface FileStat {
  path: string;
  additions: number;
  deletions: number;
  status: string;
}

export interface DiffSummary {
  filesChanged: number;
  additions: number;
  deletions: number;
  files: FileStat[];
}

export interface MemberSummary {
  name: string;
  agentType: string;
  model: string;
  cwd: string;
  isActive: boolean;
  color: string | null;
  diffSummary: DiffSummary | null;
}

export interface TaskSummary {
  id: string;
  subject: string;
  status: string;
  owner: string | null;
}

export interface ConflictInfo {
  path: string;
  members: string[];
}

export interface TeamSummary {
  name: string;
  description: string;
  createdAt: number;
  archived: boolean;
  members: MemberSummary[];
  tasks: TaskSummary[];
  conflicts: ConflictInfo[];
}

export interface MemberDiffDetail {
  name: string;
  team: string;
  cwd: string;
  baselineCommit: string | null;
  currentCommit: string | null;
  diff: string;
}

export interface ToolCall {
  tool: string;
  title: string | null;
  inputSummary: string;
  status: string;
  timestamp: number;
  durationMs: number | null;
}

export interface ToolStats {
  total: number;
  reads: number;
  writes: number;
  edits: number;
  bash: number;
  other: number;
}

export interface MemberToolHistory {
  name: string;
  team: string;
  backend: string;
  toolCalls: ToolCall[];
  stats: ToolStats;
}
