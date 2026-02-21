import { type NextRequest, NextResponse } from "next/server";

function buildTarget(req: NextRequest, path: string): URL {
  const nodeUrl = req.headers.get("x-nightshift-node-url");
  if (!nodeUrl) throw new Error("Missing nodeUrl");

  const search = req.nextUrl.search;
  const nodeId = req.headers.get("x-nightshift-node-id");
  const serverUrl = req.headers.get("x-nightshift-server-url");

  const suffix = path ? `/${path}` : "";

  if (nodeId && serverUrl) {
    return new URL(
      `/proxy/${encodeURIComponent(nodeId)}${suffix}${search}`,
      serverUrl,
    );
  }

  return new URL(`${suffix}${search}`, nodeUrl);
}

async function proxy(req: NextRequest, path: string) {
  let target: URL;
  try {
    target = buildTarget(req, path);
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : "Invalid target" },
      { status: 503 },
    );
  }

  const headers = new Headers(req.headers);
  headers.delete("host");
  headers.delete("content-length");
  headers.delete("connection");
  headers.delete("x-nightshift-node-url");
  headers.delete("x-nightshift-node-id");
  headers.delete("x-nightshift-server-url");

  const hasBody = req.method !== "GET" && req.method !== "HEAD";
  const body = hasBody ? await req.arrayBuffer() : undefined;

  const upstream = await fetch(target, {
    method: req.method,
    headers,
    body,
    cache: "no-store",
    redirect: "manual",
  });

  const upstreamHeaders = new Headers(upstream.headers);
  upstreamHeaders.delete("content-length");

  return new Response(upstream.body, {
    status: upstream.status,
    statusText: upstream.statusText,
    headers: upstreamHeaders,
  });
}

type RouteContext = {
  params: Promise<{ path?: string[] }>;
};

export async function GET(req: NextRequest, context: RouteContext) {
  const params = await context.params;
  return proxy(req, params.path?.join("/") ?? "");
}

export async function POST(req: NextRequest, context: RouteContext) {
  const params = await context.params;
  return proxy(req, params.path?.join("/") ?? "");
}

export async function PATCH(req: NextRequest, context: RouteContext) {
  const params = await context.params;
  return proxy(req, params.path?.join("/") ?? "");
}

export async function PUT(req: NextRequest, context: RouteContext) {
  const params = await context.params;
  return proxy(req, params.path?.join("/") ?? "");
}

export async function DELETE(req: NextRequest, context: RouteContext) {
  const params = await context.params;
  return proxy(req, params.path?.join("/") ?? "");
}
