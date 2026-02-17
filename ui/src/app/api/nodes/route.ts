import { NextResponse } from "next/server";

const SERVER_URL =
  process.env.NIGHTSHIFT_SERVER_URL ?? "https://nightshift-server.fly.dev";

export async function GET() {
  try {
    const res = await fetch(`${SERVER_URL}/nodes`, { cache: "no-store" });
    if (!res.ok) return NextResponse.json({ nodes: [] });
    const data = await res.json();
    return NextResponse.json(data);
  } catch {
    return NextResponse.json({ nodes: [] });
  }
}
