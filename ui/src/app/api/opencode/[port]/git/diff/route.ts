import { NextResponse } from "next/server";
import { opencodeGet, withErrorHandler } from "@/app/api/lib/opencode-client";
import { exec } from "node:child_process";
import { promisify } from "node:util";

const execAsync = promisify(exec);
const MAX_BUFFER = 10 * 1024 * 1024;

type Ctx = { params: Promise<{ port: string }> };

async function untrackedDiffs(cwd: string): Promise<string> {
  const { stdout } = await execAsync(
    "git ls-files --others --exclude-standard -z",
    { cwd, maxBuffer: MAX_BUFFER },
  );
  if (!stdout) return "";

  const files = stdout.split("\0").filter(Boolean);
  const diffs = await Promise.all(
    files.map(async (file) => {
      try {
        const { stdout: patch } = await execAsync(
          `git diff --no-index -- /dev/null ${JSON.stringify(file)}`,
          { cwd, maxBuffer: MAX_BUFFER },
        );
        return patch;
      } catch (e) {
        // NOTE(victor): git diff --no-index exits 1 when files differ, which is the normal case
        const err = e as { stdout?: string };
        return err.stdout ?? "";
      }
    }),
  );
  return diffs.join("");
}

export const GET = withErrorHandler<Ctx>(async (_req, { params }) => {
  const { port } = await params;
  const portNum = Number(port);
  if (!portNum || isNaN(portNum)) return NextResponse.json({ error: "Invalid port" }, { status: 400 });

  const project = await opencodeGet(portNum, "/project/current");
  if (!project?.worktree) {
    return NextResponse.json({ error: "No project worktree found" }, { status: 404 });
  }

  try {
    const [{ stdout: tracked }, untracked] = await Promise.all([
      execAsync("git diff HEAD", { cwd: project.worktree, maxBuffer: MAX_BUFFER }),
      untrackedDiffs(project.worktree),
    ]);
    return NextResponse.json({ diff: tracked + untracked, worktree: project.worktree });
  } catch (error) {
    const err = error as { stderr?: string; message?: string };
    return NextResponse.json({ error: err.stderr || err.message || "Failed to get git diff" }, { status: 500 });
  }
});
