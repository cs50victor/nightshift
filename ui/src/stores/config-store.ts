"use client";
import type { Agent, Provider } from "@opencode-ai/sdk";
import { create } from "zustand";
import api from "@/lib/api";

const HIDDEN_AGENTS = new Set([
  "plan",
  "general",
  "explore",
  "compaction",
  "title",
  "summary",
]);

interface ConfigState {
  providers: Provider[];
  defaultModel: string | null;
  agents: Agent[];

  loadAll: () => Promise<void>;
}

export const useConfigStore = create<ConfigState>((set) => ({
  providers: [],
  defaultModel: null,
  agents: [],

  loadAll: async () => {
    const [_config, providerData, agentData] = await Promise.all([
      api.fetch("/config"),
      api.fetch<{ providers: Provider[]; default: string }>(
        "/config/providers",
      ),
      api.fetch<Agent[]>("/agent"),
    ]);
    const agents = Array.isArray(agentData)
      ? agentData.filter((a) => !HIDDEN_AGENTS.has(a.name))
      : [];
    set({
      providers: providerData.providers,
      defaultModel: providerData.default ?? null,
      agents,
    });
  },
}));
