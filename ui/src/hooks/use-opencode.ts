"use client";
import useSWR from "swr";

const fetcher = async (url: string) => {
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`Request failed: ${res.status}`);
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

export function useAgents() {
  return useSWR("/api/opencode/agent", fetcher);
}

export function useHealth() {
  return useSWR("/api/opencode/config", fetcher);
}

export function useCurrentProject() {
  return useSWR("/api/opencode/project/current", fetcher);
}

export function useHostname() {
  return useSWR("/api/system/hostname", fetcher);
}

export function useCreateSession() {
  return async (title?: string) => {
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
