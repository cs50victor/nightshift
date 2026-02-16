import { NextResponse } from "next/server";
import { opencodeGet, withErrorHandler } from "@/app/api/lib/opencode-client";

type Ctx = { params: Promise<{ port: string }> };

export const GET = withErrorHandler<Ctx>(async (_req, { params }) => {
  const { port } = await params;
  const portNum = Number(port);
  if (!portNum || isNaN(portNum)) return NextResponse.json({ error: "Invalid port" }, { status: 400 });
  const data = await opencodeGet(portNum, "/config");
  return NextResponse.json(data);
});
