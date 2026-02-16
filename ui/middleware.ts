import { type NextRequest, NextResponse } from "next/server";

export function middleware(req: NextRequest) {
  const nodeUrl = req.cookies.get("nightshift-node-url")?.value;
  if (!nodeUrl) {
    return NextResponse.json({ error: "No node selected" }, { status: 503 });
  }

  const path = req.nextUrl.pathname.replace(/^\/api\/opencode/, "");
  const target = new URL(path + req.nextUrl.search, nodeUrl);
  return NextResponse.rewrite(target);
}

export const config = {
  matcher: "/api/opencode/:path*",
};
