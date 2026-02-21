"use client";

import type { PatchPart } from "@opencode-ai/sdk";
import Link from "next/link";
import { Badge } from "@/components/ui/badge";

export function PatchPartView({ part }: { part: PatchPart }) {
  const count = part.files.length;

  return (
    <div className="rounded-md border border-border/50 p-2 text-xs space-y-1">
      <div className="flex items-center gap-2">
        <Badge intent="primary">
          {count} file{count !== 1 ? "s" : ""} changed
        </Badge>
        <Link href="/diff" className="text-primary hover:underline ml-auto">
          View diff
        </Link>
      </div>
      <div className="text-muted-fg space-y-0.5">
        {part.files.map((file) => (
          <div key={file} className="truncate font-mono">
            {file}
          </div>
        ))}
      </div>
    </div>
  );
}
