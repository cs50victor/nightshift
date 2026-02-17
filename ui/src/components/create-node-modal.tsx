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
import { useCreateSprite, useNodes } from "@/hooks/use-opencode";
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
  const [status, setStatus] = useState<"idle" | "creating" | "error">("idle");
  const [error, setError] = useState<string | null>(null);
  const createSprite = useCreateSprite();
  const { mutate: mutateNodes } = useNodes();
  const setActiveNode = useNodeStore((s) => s.setActiveNode);

  const handleCreate = async () => {
    setStatus("creating");
    setError(null);
    try {
      const { nodeId } = await createSprite();
      const res = await fetch("/api/nodes");
      const data = await res.json();
      const node = (data.nodes as Node[])?.find((n) => n.id === nodeId);
      if (node) {
        setActiveNode(node.url);
      }
      await mutateNodes();
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
          description="Provision a remote sprite with opencode and nightshift daemon."
        />
        <SheetBody>
          {status === "idle" && (
            <p className="text-sm text-muted-fg">
              This will create a new Fly Sprite, install opencode and the
              nightshift daemon, and register it as a node. This can take up to
              30 seconds.
            </p>
          )}
          {status === "creating" && (
            <div className="flex flex-col items-center gap-3 py-8">
              <Loader className="size-6" />
              <p className="text-sm text-muted-fg">Provisioning sprite...</p>
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
