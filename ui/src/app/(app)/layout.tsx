"use client";

import { useEffect, useState } from "react";
import { AppSidebarNav } from "@/components/app-sidebar-nav";
import { BreadcrumbProvider } from "@/contexts/breadcrumb-context";
import type { Node } from "@/lib/types";
import { useModelStore } from "@/stores/model-store";
import { useNodeStore } from "@/stores/node-store";

function getNodeCookie(): string | null {
  const match = document.cookie.match(/nightshift-node-url=([^;]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

export default function AppLayout({ children }: { children: React.ReactNode }) {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const setActiveNode = useNodeStore((s) => s.setActiveNode);

  useEffect(() => {
    async function init() {
      try {
        const existing = getNodeCookie();
        if (!existing) {
          const res = await fetch("/api/nodes");
          const data = await res.json();
          const nodes: Node[] = data.nodes || [];
          if (nodes.length === 0) {
            setError("No nodes available");
            setLoading(false);
            return;
          }
          setActiveNode(nodes[0].url, nodes[0].id);
        } else {
          useNodeStore.getState().activeNodeUrl !== existing &&
            useNodeStore.setState({ activeNodeUrl: existing });
        }

        const health = await fetch("/api/opencode/global/health");
        if (!health.ok) {
          setError("Node unreachable");
          setLoading(false);
          return;
        }

        const providersRes = await fetch("/api/opencode/config/providers");
        if (providersRes.ok) {
          const data = await providersRes.json();
          const firstProvider = data.providers?.[0];
          const firstModelId = Object.keys(firstProvider?.models ?? {})[0];
          if (firstProvider && firstModelId) {
            useModelStore
              .getState()
              .setModelFromDefault(`${firstProvider.id}/${firstModelId}`);
          }
        }
      } catch {
        setError("Node unreachable");
      } finally {
        setLoading(false);
      }
    }
    init();
  }, [setActiveNode]);

  if (loading) {
    return (
      <div className="flex h-dvh items-center justify-center">
        <div className="text-muted-fg">Connecting...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex h-dvh items-center justify-center">
        <div className="text-center">
          <h2 className="text-lg font-medium mb-2">{error}</h2>
          <p className="text-muted-fg">
            Check that a nightshift daemon is running.
          </p>
        </div>
      </div>
    );
  }

  return (
    <BreadcrumbProvider>
      <div className="flex h-dvh flex-col overflow-hidden">
        <AppSidebarNav />
        <div className="flex-1 overflow-auto p-4">{children}</div>
      </div>
    </BreadcrumbProvider>
  );
}
