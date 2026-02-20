"use client";
import { create } from "zustand";
import { persist } from "zustand/middleware";

interface NodeState {
  activeNodeUrl: string | null;
  activeNodeId: string | null;
  setActiveNode: (url: string, id: string) => void;
}

export const useNodeStore = create<NodeState>()(
  persist(
    (set) => ({
      activeNodeUrl: null,
      activeNodeId: null,
      setActiveNode: (url, id) => {
        set({ activeNodeUrl: url, activeNodeId: id });
        // biome-ignore lint/suspicious/noDocumentCookie: Cookie Store API lacks broad support; middleware reads these cookies
        document.cookie = `nightshift-node-url=${encodeURIComponent(url)}; path=/; max-age=31536000`;
        // biome-ignore lint/suspicious/noDocumentCookie: Cookie Store API lacks broad support; middleware reads these cookies
        document.cookie = `nightshift-node-id=${encodeURIComponent(id)}; path=/; max-age=31536000`;
      },
    }),
    {
      name: "nightshift-active-node",
    },
  ),
);
