import { NextResponse } from "next/server";

export function getOpencodeUrl(port: number): string {
  return `http://localhost:${port}`;
}

async function doFetch(url: string, init?: RequestInit) {
  try {
    const res = await fetch(url, init);
    if (!res.ok) throw new Error(`opencode returned ${res.status}`);
    return res.json();
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Unknown error";
    throw new Error(`opencode unreachable at ${url}: ${msg}`);
  }
}

export async function opencodeGet(port: number, path: string) {
  return doFetch(`${getOpencodeUrl(port)}${path}`);
}

export async function opencodePost(port: number, path: string, body?: unknown) {
  return doFetch(`${getOpencodeUrl(port)}${path}`, {
    method: "POST",
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });
}

export async function opencodeDelete(port: number, path: string) {
  return doFetch(`${getOpencodeUrl(port)}${path}`, { method: "DELETE" });
}

export function withErrorHandler<T>(
  handler: (req: Request, ctx: T) => Promise<NextResponse>,
) {
  return async (req: Request, ctx: T) => {
    try {
      return await handler(req, ctx);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Internal error";
      return NextResponse.json({ error: message }, { status: 502 });
    }
  };
}
