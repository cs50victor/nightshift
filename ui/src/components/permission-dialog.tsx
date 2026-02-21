"use client";

import { useMemo } from "react";
import { Button } from "@/components/ui/button";
import { usePermissionStore } from "@/stores/permission-store";

interface PermissionDialogProps {
  sessionID: string;
}

export function PermissionDialog({ sessionID }: PermissionDialogProps) {
  const pending = usePermissionStore((s) => s.pending);
  const reply = usePermissionStore((s) => s.reply);
  const permissions = useMemo(
    () => Object.values(pending).filter((p) => p.sessionID === sessionID),
    [pending, sessionID],
  );

  if (permissions.length === 0) return null;

  return (
    <div className="sticky bottom-0 z-10 flex flex-col gap-2 border-t border-border bg-bg p-3">
      {permissions.map((perm) => (
        <div
          key={perm.id}
          className="flex items-center justify-between gap-3 rounded-lg border border-warning-subtle-fg/30 bg-warning-subtle/50 px-4 py-3"
        >
          <div className="flex flex-col gap-1 text-sm">
            <span className="font-medium text-fg">{perm.permission}</span>
            {perm.patterns.length > 0 && (
              <span className="text-muted-fg">
                {perm.patterns.join(", ")}
              </span>
            )}
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <Button
              intent="primary"
              size="sm"
              onPress={() => reply(perm.id, "once")}
            >
              Allow Once
            </Button>
            <Button
              intent="secondary"
              size="sm"
              onPress={() => reply(perm.id, "always")}
            >
              Always Allow
            </Button>
            <Button
              intent="danger"
              size="sm"
              onPress={() => reply(perm.id, "reject")}
            >
              Reject
            </Button>
          </div>
        </div>
      ))}
    </div>
  );
}
