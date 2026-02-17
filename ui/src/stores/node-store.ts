"use client";
import { create } from "zustand";
import { persist } from "zustand/middleware";

interface NodeState {
  activeNodeUrl: string | null;
  setActiveNode: (url: string) => void;
}

export const useNodeStore = create<NodeState>()(
  persist(
    (set) => ({
      activeNodeUrl: null,
      setActiveNode: (url) => {
        set({ activeNodeUrl: url });
        // biome-ignore lint/suspicious/noDocumentCookie: Cookie Store API lacks broad support; middleware reads this cookie
        document.cookie = `nightshift-node-url=${encodeURIComponent(url)}; path=/; max-age=31536000`;
      },
    }),
    {
      name: "nightshift-active-node",
    },
  ),
);
