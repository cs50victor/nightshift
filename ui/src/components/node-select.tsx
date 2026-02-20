"use client";

import { TrashIcon } from "@heroicons/react/20/solid";
import { ServerStackIcon } from "@heroicons/react/24/outline";
import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import {
  Menu,
  MenuContent,
  MenuItem,
  MenuSeparator,
  MenuTrigger,
} from "@/components/ui/menu";
import { toast } from "@/components/ui/toast";
import { useDeleteMachine, useNodes } from "@/hooks/use-opencode";
import type { Node } from "@/lib/types";
import { useNodeStore } from "@/stores/node-store";
import { CreateNodeModal } from "./create-node-modal";

function isLocal(url: string) {
  return url.includes("localhost") || url.includes("127.0.0.1");
}

export function NodeSelect() {
  const { data, mutate } = useNodes();
  const activeNodeUrl = useNodeStore((s) => s.activeNodeUrl);
  const setActiveNode = useNodeStore((s) => s.setActiveNode);
  const deleteMachine = useDeleteMachine();
  const [modalOpen, setModalOpen] = useState(false);

  const nodes: Node[] = data?.nodes ?? [];

  const handleSelect = (url: string, id: string) => {
    if (url === activeNodeUrl) return;
    setActiveNode(url, id);
    window.location.reload();
  };

  const handleDelete = async (node: Node) => {
    const machineName = node.machineName ?? node.name;
    try {
      await deleteMachine(machineName);
      toast.success(`Deleted ${node.name}`);
      if (node.url === activeNodeUrl) {
        const remaining = nodes.filter((n) => n.url !== node.url);
        if (remaining.length > 0) {
          setActiveNode(remaining[0].url, remaining[0].id);
        }
      }
      await mutate();
      if (node.url === activeNodeUrl) {
        window.location.reload();
      }
    } catch (e) {
      toast.error((e as Error).message);
    }
  };

  const activeNode = nodes.find((n) => n.url === activeNodeUrl);
  const label = activeNode?.name ?? "Select node";

  return (
    <>
      <Menu>
        <MenuTrigger className="flex items-center gap-1.5 rounded-md border border-border px-2 py-1 text-xs font-mono text-muted-fg hover:text-fg hover:bg-secondary transition-colors">
          <ServerStackIcon className="size-3.5" />
          <span className="max-w-32 truncate">{label}</span>
        </MenuTrigger>
        <MenuContent placement="bottom" className="min-w-56">
          {nodes.map((node) => (
            <MenuItem
              key={node.id}
              id={node.id}
              onAction={() => handleSelect(node.url, node.id)}
              textValue={node.name}
            >
              <div className="flex w-full items-center justify-between gap-2">
                <div className="flex items-center gap-2 min-w-0">
                  <span className="truncate">{node.name}</span>
                  {isLocal(node.url) && (
                    <Badge intent="secondary" className="text-[10px]">
                      local
                    </Badge>
                  )}
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  {!isLocal(node.url) && node.os && (
                    <span className="text-muted-fg text-[10px]">
                      {node.os}/{node.arch}
                    </span>
                  )}
                  {node.url === activeNodeUrl && (
                    <span className="size-1.5 rounded-full bg-success" />
                  )}
                  {!isLocal(node.url) && (
                    <button
                      type="button"
                      className="p-0.5 rounded hover:bg-danger-subtle text-muted-fg hover:text-danger-subtle-fg transition-colors"
                      onPointerDown={(e) => {
                        e.stopPropagation();
                        e.preventDefault();
                      }}
                      onClick={(e) => {
                        e.stopPropagation();
                        e.preventDefault();
                        handleDelete(node);
                      }}
                    >
                      <TrashIcon className="size-3" />
                    </button>
                  )}
                </div>
              </div>
            </MenuItem>
          ))}
          <MenuSeparator />
          <MenuItem id="new-node" onAction={() => setModalOpen(true)}>
            + New Node
          </MenuItem>
        </MenuContent>
      </Menu>
      <CreateNodeModal isOpen={modalOpen} onOpenChange={setModalOpen} />
    </>
  );
}
