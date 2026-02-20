"use client";
import useSWR from "swr";
import { useNodeStore } from "@/stores/node-store";

const fetcher = async (url: string) => {
  const res = await fetch(url);
  if (!res.ok) {
    const body = await res.json().catch(() => null);
    throw new Error(body?.error || `Request failed: ${res.status}`);
  }
  return res.json();
};

export function useNodes() {
  return useSWR("/api/nodes", fetcher);
}

export function useSessions() {
  return useSWR("/api/opencode/session", fetcher);
}

export function useSession(id: string | null) {
  return useSWR(id ? `/api/opencode/session/${id}` : null, fetcher);
}

export function useConfig() {
  return useSWR("/api/opencode/config", fetcher);
}

export function useProviders() {
  return useSWR("/api/opencode/config/providers", fetcher);
}

const HIDDEN_AGENTS = new Set(["compaction", "title", "summary"]);

export function useAgents() {
  const result = useSWR("/api/opencode/agent", fetcher);
  const filtered = Array.isArray(result.data)
    ? result.data.filter(
        (agent: { name: string }) => !HIDDEN_AGENTS.has(agent.name),
      )
    : result.data;
  return { ...result, data: filtered };
}

export function useHealth() {
  return useSWR("/api/opencode/config", fetcher);
}

export function useCurrentProject() {
  return useSWR("/api/opencode/project/current", fetcher);
}

export function useProjectPath() {
  return useSWR<{ path: string }>(
    "/api/opencode/project/absolute_path",
    fetcher,
  );
}

export function useCreateSession() {
  return async (title?: string) => {
    const { activeNodeUrl } = useNodeStore.getState();
    if (!activeNodeUrl) {
      throw new Error(
        "No node selected. Select a node before creating a session.",
      );
    }

    const res = await fetch("/api/opencode/session", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ title }),
    });

    if (!res.ok) {
      throw new Error(`Failed to create session: ${res.status}`);
    }

    return res.json();
  };
}

export function useDeleteSession() {
  return async (sessionId: string) => {
    const res = await fetch(`/api/opencode/session/${sessionId}`, {
      method: "DELETE",
    });

    if (!res.ok) {
      throw new Error(`Failed to delete session: ${res.status}`);
    }

    return res.json();
  };
}

export function useGitDiff(path?: string | null) {
  return useSWR<{ diff: string; worktree: string }>(
    path ? `/api/git/diff?path=${encodeURIComponent(path)}` : null,
    fetcher,
  );
}

export function useCreateMachine() {
  return async (name?: string) => {
    const res = await fetch("/api/machines", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name }),
    });
    if (!res.ok) {
      const data = await res.json().catch(() => null);
      throw new Error(data?.error || `Machine creation failed: ${res.status}`);
    }
    return res.json() as Promise<{ name: string; nodeId: string }>;
  };
}

export function useDeleteMachine() {
  return async (name: string) => {
    const res = await fetch(`/api/machines/${name}`, { method: "DELETE" });
    if (!res.ok) {
      const data = await res.json().catch(() => null);
      throw new Error(data?.error || `Machine deletion failed: ${res.status}`);
    }
    return res.json();
  };
}
