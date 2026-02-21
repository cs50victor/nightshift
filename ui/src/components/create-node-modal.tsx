"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Loader } from "@/components/ui/loader";
import {
  Sheet,
  SheetBody,
  SheetContent,
  SheetFooter,
  SheetHeader,
} from "@/components/ui/sheet";
import api, { useNodes } from "@/lib/api";
import type { Node } from "@/lib/types";
import { useNodeStore } from "@/stores/node-store";

interface CreateNodeModalProps {
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CreateNodeModal({
  isOpen,
  onOpenChange,
}: CreateNodeModalProps) {
  const [name, setName] = useState("");
  const [status, setStatus] = useState<"idle" | "creating" | "error">("idle");
  const [error, setError] = useState<string | null>(null);
  const { refresh: refreshNodes } = useNodes();
  const setActiveNode = useNodeStore((s) => s.setActiveNode);

  const handleCreate = async () => {
    setStatus("creating");
    setError(null);
    try {
      const { nodeId } = await api.postRaw<{ name: string; nodeId: string }>(
        "/api/machines",
        name.trim() ? { name: name.trim() } : undefined,
      );
      const res = await fetch("/api/nodes");
      const data = await res.json();
      const node = (data.nodes as Node[])?.find((n) => n.id === nodeId);
      if (node) {
        setActiveNode(node.url, node.id);
      }
      refreshNodes();
      onOpenChange(false);
      setStatus("idle");
      window.location.reload();
    } catch (e) {
      setError((e as Error).message);
      setStatus("error");
    }
  };

  const handleClose = () => {
    if (status === "creating") return;
    onOpenChange(false);
    setStatus("idle");
    setError(null);
  };

  return (
    <Sheet isOpen={isOpen} onOpenChange={handleClose}>
      <SheetContent
        isOpen={isOpen}
        onOpenChange={handleClose}
        isDismissable={status !== "creating"}
        closeButton={status !== "creating"}
        side="right"
        aria-label="Create node"
      >
        <SheetHeader
          title="New Remote Node"
          description="Provision a remote node with opencode and nightshift daemon."
        />
        <SheetBody>
          {status === "idle" && (
            <div className="flex flex-col gap-3">
              <label className="flex flex-col gap-1.5">
                <span className="text-sm font-medium">Name</span>
                <input
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="e.g. work-laptop"
                  className="rounded-md border border-border bg-bg px-3 py-1.5 text-sm font-mono placeholder:text-muted-fg focus:outline-none focus:ring-1 focus:ring-primary"
                />
              </label>
            </div>
          )}
          {status === "creating" && (
            <div className="flex flex-col items-center gap-3 py-8">
              <Loader className="size-6" />
              <p className="text-sm text-muted-fg">Provisioning node...</p>
            </div>
          )}
          {status === "error" && (
            <div className="rounded-md border border-danger/30 bg-danger-subtle p-3">
              <p className="text-sm text-danger-subtle-fg">{error}</p>
            </div>
          )}
        </SheetBody>
        <SheetFooter>
          {status === "idle" && (
            <>
              <Button intent="plain" onPress={handleClose}>
                Cancel
              </Button>
              <Button intent="primary" onPress={handleCreate}>
                Create Node
              </Button>
            </>
          )}
          {status === "error" && (
            <>
              <Button intent="plain" onPress={handleClose}>
                Close
              </Button>
              <Button intent="primary" onPress={handleCreate}>
                Retry
              </Button>
            </>
          )}
        </SheetFooter>
      </SheetContent>
    </Sheet>
  );
}
