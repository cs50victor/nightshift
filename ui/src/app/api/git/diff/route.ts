import { exec } from "node:child_process";
import { promisify } from "node:util";
import { cookies } from "next/headers";
import { NextResponse } from "next/server";

const execAsync = promisify(exec);
const MAX_BUFFER = 10 * 1024 * 1024;

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

export async function GET(req: Request) {
  const pathParam = new URL(req.url).searchParams.get("path");
  let cwd: string;

  if (pathParam) {
    cwd = pathParam;
  } else {
    const cookieStore = await cookies();
    const nodeUrl = cookieStore.get("nightshift-node-url")?.value;
    if (!nodeUrl) {
      return NextResponse.json({ error: "No node selected" }, { status: 503 });
    }

    try {
      const res = await fetch(`${nodeUrl}/project/current`);
      if (!res.ok) {
        return NextResponse.json(
          { error: "Failed to fetch project info" },
          { status: 502 },
        );
      }
      const project = await res.json();
      if (!project?.worktree) {
        return NextResponse.json(
          { error: "No project worktree found" },
          { status: 404 },
        );
      }
      cwd = project.worktree;
    } catch {
      return NextResponse.json({ error: "Node unreachable" }, { status: 502 });
    }
  }

  try {
    await execAsync("git rev-parse --git-dir", { cwd });
  } catch {
    return NextResponse.json(
      { error: `Not a git repository: ${cwd}` },
      { status: 422 },
    );
  }

  try {
    const [{ stdout: tracked }, untracked] = await Promise.all([
      execAsync("git diff HEAD", { cwd, maxBuffer: MAX_BUFFER }),
      untrackedDiffs(cwd),
    ]);
    return NextResponse.json({ diff: tracked + untracked, worktree: cwd });
  } catch (error) {
    const err = error as { stderr?: string; message?: string };
    return NextResponse.json(
      { error: err.stderr || err.message || "Failed to get git diff" },
      { status: 500 },
    );
  }
}
