import { NextResponse } from "next/server";
import { opencodeGet, withErrorHandler } from "@/app/api/lib/opencode-client";

type Ctx = { params: Promise<{ port: string; id: string }> };

export const GET = withErrorHandler<Ctx>(async (_req, { params }) => {
  const { port, id } = await params;
  const portNum = Number(port);
  if (!portNum || isNaN(portNum)) return NextResponse.json({ error: "Invalid port" }, { status: 400 });
  if (!id) return NextResponse.json({ error: "Session ID required" }, { status: 400 });
  const data = await opencodeGet(portNum, `/session/${id}/message/list`);
  return NextResponse.json(data);
});
