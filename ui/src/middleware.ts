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

  const targetPath = `/api/opencode-proxy${path}${search}`;
  const internalTarget = new URL(targetPath, req.url);
  const requestHeaders = new Headers(req.headers);
  requestHeaders.set("x-nightshift-node-url", nodeUrl);

  if (isLocal(nodeUrl)) {
    requestHeaders.delete("x-nightshift-node-id");
    requestHeaders.delete("x-nightshift-server-url");
    return NextResponse.rewrite(internalTarget, {
      request: {
        headers: requestHeaders,
      },
    });
  }

  const nodeId = req.cookies.get("nightshift-node-id")?.value;
  if (!nodeId) {
    return NextResponse.json({ error: "No node ID set" }, { status: 503 });
  }

  const serverUrl = process.env.NIGHTSHIFT_SERVER_URL;
  if (!serverUrl) {
    return NextResponse.json(
      { error: "NIGHTSHIFT_SERVER_URL not configured" },
      { status: 500 },
    );
  }

  requestHeaders.set("x-nightshift-node-id", nodeId);
  requestHeaders.set("x-nightshift-server-url", serverUrl);
  return NextResponse.rewrite(internalTarget, {
    request: {
      headers: requestHeaders,
    },
  });
}

export const config = {
  matcher: "/api/opencode/:path*",
};
