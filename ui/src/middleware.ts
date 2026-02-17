import { type NextRequest, NextResponse } from "next/server";

function isLocal(url: string) {
  return url.includes("localhost") || url.includes("127.0.0.1");
}

export function middleware(req: NextRequest) {
  const raw = req.cookies.get("nightshift-node-url")?.value;
  const nodeUrl = raw ? decodeURIComponent(raw) : undefined;
  if (!nodeUrl) {
    return NextResponse.json({ error: "No node selected" }, { status: 503 });
  }

  const path = req.nextUrl.pathname.replace(/^\/api\/opencode/, "");
  const search = req.nextUrl.search;

  if (isLocal(nodeUrl)) {
    const target = new URL(path + search, nodeUrl);
    return NextResponse.rewrite(target);
  }

  const nodeId = req.cookies.get("nightshift-node-id")?.value;
  if (!nodeId) {
    return NextResponse.json({ error: "No node ID set" }, { status: 503 });
  }

  const serverUrl = process.env.NIGHTSHIFT_SERVER_URL;
  if (!serverUrl) {
    return NextResponse.json({ error: "NIGHTSHIFT_SERVER_URL not configured" }, { status: 500 });
  }

  const target = new URL(`/proxy/${encodeURIComponent(nodeId)}${path}${search}`, serverUrl);
  return NextResponse.rewrite(target);
}

export const config = {
  matcher: "/api/opencode/:path*",
};
