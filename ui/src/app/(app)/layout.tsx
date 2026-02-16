"use client";

import { useEffect, useState } from "react";
import { AppSidebarNav } from "@/components/app-sidebar-nav";
import { BreadcrumbProvider } from "@/contexts/breadcrumb-context";

interface Node {
  id: string;
  name: string;
  url: string;
  startedAt: string;
}

function getNodeCookie(): string | null {
  const match = document.cookie.match(/nightshift-node-url=([^;]+)/);
  return match ? decodeURIComponent(match[1]) : null;
}

function setNodeCookie(url: string) {
  // biome-ignore lint/suspicious/noDocumentCookie: Cookie Store API lacks broad support; middleware reads this cookie
  document.cookie = `nightshift-node-url=${encodeURIComponent(url)}; path=/; max-age=31536000`;
}

export default function AppLayout({ children }: { children: React.ReactNode }) {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

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
          setNodeCookie(nodes[0].url);
        }

        const health = await fetch("/api/opencode/config");
        if (!health.ok) {
          setError("Node unreachable");
          setLoading(false);
          return;
        }
      } catch {
        setError("Node unreachable");
      } finally {
        setLoading(false);
      }
    }
    init();
  }, []);

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
