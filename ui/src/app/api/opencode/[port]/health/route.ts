import { NextResponse } from "next/server";
import { opencodeGet } from "@/app/api/lib/opencode-client";

export async function GET(_req: Request, { params }: { params: Promise<{ port: string }> }) {
  const { port } = await params;
  const portNum = Number(port);
  if (!portNum || isNaN(portNum)) return NextResponse.json({ error: "Invalid port" }, { status: 400 });
  try {
    const data = await opencodeGet(portNum, "/config");
    return NextResponse.json({ healthy: !!data, port: portNum });
  } catch {
    return NextResponse.json({ healthy: false, port: portNum });
  }
}
