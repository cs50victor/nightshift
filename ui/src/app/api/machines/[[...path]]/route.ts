import { type NextRequest, NextResponse } from "next/server";

const SERVER_URL =
  process.env.NIGHTSHIFT_SERVER_URL ?? "https://nightshift-server.fly.dev";

export async function POST(req: NextRequest) {
  try {
    const body = await req.json().catch(() => ({}));
    const res = await fetch(`${SERVER_URL}/machines`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch (e) {
    return NextResponse.json({ error: (e as Error).message }, { status: 500 });
  }
}

export async function DELETE(
  _req: NextRequest,
  { params }: { params: Promise<{ path?: string[] }> },
) {
  const { path } = await params;
  const name = path?.[0];
  if (!name) {
    return NextResponse.json(
      { error: "missing machine name" },
      { status: 400 },
    );
  }
  try {
    const res = await fetch(`${SERVER_URL}/machines/${name}`, {
      method: "DELETE",
    });
    const data = await res.json();
    return NextResponse.json(data, { status: res.status });
  } catch (e) {
    return NextResponse.json({ error: (e as Error).message }, { status: 500 });
  }
}
