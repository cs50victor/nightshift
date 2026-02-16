"use client";
import { useEffect, useMemo } from "react";
import { FileDiff } from "@pierre/diffs/react";
import { parsePatchFiles } from "@pierre/diffs";
import { Loader } from "@/components/ui/loader";
import { Button } from "@/components/ui/button";
import { useGitDiff } from "@/hooks/use-opencode";
import { useBreadcrumb } from "@/contexts/breadcrumb-context";
import { ArrowPathIcon } from "@heroicons/react/24/outline";

export default function DiffPage() {
  const { data, error, isLoading, mutate } = useGitDiff();
  const { setPageTitle } = useBreadcrumb();

  useEffect(() => {
    setPageTitle("Git Diff");
    return () => setPageTitle(null);
  }, [setPageTitle]);

  const files = useMemo(() => {
    if (!data?.diff) return [];
    try {
      const patches = parsePatchFiles(data.diff);
      return patches.flatMap((patch) => patch.files);
    } catch {
      return [];
    }
  }, [data?.diff]);

  const summary = useMemo(() => {
    let additions = 0;
    let deletions = 0;
    let newFiles = 0;
    let modified = 0;
    let deleted = 0;
    for (const file of files) {
      if (file.type === "new") newFiles++;
      else if (file.type === "deleted") deleted++;
      else modified++;
      for (const hunk of file.hunks) {
        additions += hunk.additionLines;
        deletions += hunk.deletionLines;
      }
    }
    return { additions, deletions, newFiles, modified, deleted };
  }, [files]);

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <Loader className="size-8" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4">
        <div className="text-danger">Error loading diff: {error.message}</div>
        <Button onPress={() => mutate()}>
          <ArrowPathIcon className="size-4" />
          Retry
        </Button>
      </div>
    );
  }

  if (!data?.diff || files.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4">
        <div className="text-muted-fg">No changes detected</div>
        <Button onPress={() => mutate()}>
          <ArrowPathIcon className="size-4" />
          Refresh
        </Button>
      </div>
    );
  }

  return (
    <div className="-m-4 flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-border px-4 py-3">
        <div className="flex items-center gap-4">
          <h1 className="text-lg font-semibold">Git Diff</h1>
          <div className="flex items-center gap-3 text-xs font-mono text-muted-fg">
            <span>{files.length} {files.length === 1 ? "file" : "files"}</span>
            {summary.newFiles > 0 && <span className="text-success">+{summary.newFiles} new</span>}
            {summary.modified > 0 && <span className="text-warning">{summary.modified} modified</span>}
            {summary.deleted > 0 && <span className="text-danger">{summary.deleted} deleted</span>}
            <span className="text-success">+{summary.additions}</span>
            <span className="text-danger">-{summary.deletions}</span>
          </div>
        </div>
        <Button intent="secondary" size="sm" onPress={() => mutate()}>
          <ArrowPathIcon className="size-4" />
          Refresh
        </Button>
      </div>

      <div className="flex-1 overflow-auto">
        {files.map((file, index) => (
          <FileDiff
            key={file.name || file.prevName || index}
            fileDiff={file}
            options={{
              diffStyle: "unified",
              diffIndicators: "bars",
            }}
          />
        ))}
      </div>
    </div>
  );
}
