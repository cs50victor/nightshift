"use client";
import useSWR from "swr";
import type {
  MemberDiffDetail,
  MemberToolHistory,
  Node,
  TeamSummary,
} from "@/lib/types";

const api = {
  fetch: async <T = unknown>(path: string): Promise<T> => {
    const url = path.startsWith("/api/") ? path : `/api/opencode${path}`;
    const res = await fetch(url);
    if (!res.ok) throw new Error(`${res.status}: ${await res.text()}`);
    return res.json();
  },
  post: async <T = unknown>(path: string, body?: unknown): Promise<T> => {
    const url = path.startsWith("/api/") ? path : `/api/opencode${path}`;
    const res = await fetch(url, {
      method: "POST",
      headers: body ? { "Content-Type": "application/json" } : {},
      body: body ? JSON.stringify(body) : undefined,
    });
    if (!res.ok) throw new Error(`${res.status}: ${await res.text()}`);
    if (res.status === 204) return undefined as T;
    return res.json();
  },
  delete: async <T = unknown>(path: string): Promise<T> => {
    const url = path.startsWith("/api/") ? path : `/api/opencode${path}`;
    const res = await fetch(url, { method: "DELETE" });
    if (!res.ok) throw new Error(`${res.status}: ${await res.text()}`);
    return res.json();
  },
  postRaw: async <T = unknown>(url: string, body?: unknown): Promise<T> => {
    const res = await fetch(url, {
      method: "POST",
      headers: body ? { "Content-Type": "application/json" } : {},
      body: body ? JSON.stringify(body) : undefined,
    });
    if (!res.ok) throw new Error(`${res.status}: ${await res.text()}`);
    if (res.status === 204) return undefined as T;
    return res.json();
  },
  deleteRaw: async <T = unknown>(url: string): Promise<T> => {
    const res = await fetch(url, { method: "DELETE" });
    if (!res.ok) throw new Error(`${res.status}: ${await res.text()}`);
    return res.json();
  },
};
export default api;

const swrFetcher = <T>(path: string): Promise<T> => api.fetch<T>(path);

async function fetchRaw<T = unknown>(url: string): Promise<T> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${res.status}: ${await res.text()}`);
  return res.json();
}
const swrRawFetcher = <T>(url: string): Promise<T> => fetchRaw<T>(url);

export function useGitDiff(path?: string | null) {
  const key = path ? `/api/git/diff?path=${encodeURIComponent(path)}` : null;
  const { data, error, isLoading, mutate } = useSWR<{
    diff: string;
    worktree: string;
  }>(key, swrRawFetcher);
  return {
    data: data ?? null,
    error: error ?? null,
    isLoading,
    refresh: () => mutate(),
  };
}

export function useProjectPath() {
  const { data, error, isLoading } = useSWR<{ path: string }>(
    "/path",
    swrFetcher,
  );
  return {
    data: data ?? null,
    error: error ?? null,
    isLoading,
  };
}

export function useCurrentProject<T = unknown>() {
  const { data, isLoading } = useSWR<T>("/project/current", swrFetcher, {
    shouldRetryOnError: false,
  });
  return {
    data: data ?? null,
    isLoading,
  };
}

export function useNodes() {
  const { data, error, isLoading, mutate } = useSWR<{ nodes: Node[] }>(
    "/api/nodes",
    swrRawFetcher,
  );
  return {
    nodes: data?.nodes ?? [],
    loading: isLoading,
    error: error ?? null,
    refresh: () => mutate(),
  };
}

export function useTeams() {
  const { data, error, isLoading, mutate } = useSWR<TeamSummary[]>(
    "/teams",
    swrFetcher,
    { refreshInterval: 5000 },
  );
  return {
    teams: data ?? [],
    error: error ?? null,
    isLoading,
    refresh: () => mutate(),
  };
}

export function useMemberDiff(team: string | null, name: string | null) {
  const key = team && name ? `/teams/${team}/members/${name}/diff` : null;
  const { data, error, isLoading } = useSWR<MemberDiffDetail>(key, swrFetcher);
  return {
    data: data ?? null,
    error: error ?? null,
    isLoading,
  };
}

export function useMemberTools(team: string | null, name: string | null) {
  const key = team && name ? `/teams/${team}/members/${name}/tools` : null;
  const { data, error, isLoading } = useSWR<MemberToolHistory>(
    key,
    swrFetcher,
    { refreshInterval: 10000 },
  );
  return {
    data: data ?? null,
    error: error ?? null,
    isLoading,
  };
}
