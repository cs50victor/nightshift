"use client";

import { useEffect, useState } from "react";
import { AppSidebarNav } from "@/components/app-sidebar-nav";
import { BreadcrumbProvider } from "@/contexts/breadcrumb-context";
import { useInstanceStore } from "@/stores/instance-store";

export default function AppLayout({ children }: { children: React.ReactNode }) {
  const instance = useInstanceStore((s) => s.instance);
  const setInstance = useInstanceStore((s) => s.setInstance);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (instance) {
      setLoading(false);
      return;
    }

    fetch("/api/instances")
      .then((res) => res.json())
      .then((data) => {
        if (data.instances?.length > 0) {
          const inst = data.instances[0];
          setInstance({ id: inst.id, name: inst.name, port: inst.port });
        }
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [instance, setInstance]);

  if (loading) {
    return (
      <div className="flex h-dvh items-center justify-center">
        <div className="text-muted-fg">Connecting...</div>
      </div>
    );
  }

  if (!instance) {
    return (
      <div className="flex h-dvh items-center justify-center">
        <div className="text-center">
          <h2 className="text-lg font-medium mb-2">
            No opencode instance running
          </h2>
          <p className="text-muted-fg">
            Start an opencode instance to get started.
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
