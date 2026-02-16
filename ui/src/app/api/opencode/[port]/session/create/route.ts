import { NextResponse } from "next/server";
import { opencodePost, withErrorHandler } from "@/app/api/lib/opencode-client";

type Ctx = { params: Promise<{ port: string }> };

export const POST = withErrorHandler<Ctx>(async (req, { params }) => {
  const { port } = await params;
  const portNum = Number(port);
  if (!portNum || isNaN(portNum)) {
    return NextResponse.json({ error: "Invalid port" }, { status: 400 });
  }
  const body = await req.json().catch(() => ({}));
  const data = await opencodePost(portNum, "/session", { title: body?.title, parentID: body?.parentID });
  return NextResponse.json(data);
});
