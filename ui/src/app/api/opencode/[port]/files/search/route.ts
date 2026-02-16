import { NextResponse } from "next/server";
import { opencodeGet, withErrorHandler } from "@/app/api/lib/opencode-client";

type Ctx = { params: Promise<{ port: string }> };

export const GET = withErrorHandler<Ctx>(async (req, { params }) => {
  const { port } = await params;
  const portNum = Number(port);
  const q = new URL(req.url).searchParams.get("q");
  if (!q) return NextResponse.json([]);
  const data = await opencodeGet(portNum, `/file/search?query=${encodeURIComponent(q)}`);
  return NextResponse.json(data);
});
