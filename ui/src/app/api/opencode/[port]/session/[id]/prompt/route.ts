import { NextResponse } from "next/server";
import { opencodePost, withErrorHandler } from "@/app/api/lib/opencode-client";

type Ctx = { params: Promise<{ port: string; id: string }> };

export const POST = withErrorHandler<Ctx>(async (req, { params }) => {
  const { port, id } = await params;
  const portNum = Number(port);
  if (!portNum || isNaN(portNum)) return NextResponse.json({ error: "Invalid port" }, { status: 400 });
  if (!id) return NextResponse.json({ error: "Session ID required" }, { status: 400 });
  const body = await req.json();
  if (!body?.text) return NextResponse.json({ error: "Message text required" }, { status: 400 });
  const data = await opencodePost(portNum, `/session/${id}/prompt`, {
    parts: [{ type: "text", text: body.text }],
    model: body.model,
    agent: body.agent,
  });
  return NextResponse.json(data);
});
