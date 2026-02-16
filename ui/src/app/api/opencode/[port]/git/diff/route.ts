import { NextResponse } from "next/server";
import { opencodeGet, withErrorHandler } from "@/app/api/lib/opencode-client";
import { exec } from "node:child_process";
import { promisify } from "node:util";

const execAsync = promisify(exec);

type Ctx = { params: Promise<{ port: string }> };

export const GET = withErrorHandler<Ctx>(async (_req, { params }) => {
  const { port } = await params;
  const portNum = Number(port);
  if (!portNum || isNaN(portNum)) return NextResponse.json({ error: "Invalid port" }, { status: 400 });

  const project = await opencodeGet(portNum, "/project/current");
  if (!project?.worktree) {
    return NextResponse.json({ error: "No project worktree found" }, { status: 404 });
  }

  try {
    const { stdout } = await execAsync("git diff HEAD", {
      cwd: project.worktree,
      maxBuffer: 10 * 1024 * 1024,
    });
    return NextResponse.json({ diff: stdout, worktree: project.worktree });
  } catch (error) {
    const err = error as { stderr?: string; message?: string };
    return NextResponse.json({ error: err.stderr || err.message || "Failed to get git diff" }, { status: 500 });
  }
});
