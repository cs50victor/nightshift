import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { NextResponse } from "next/server";

const NODES_PATH = join(homedir(), ".nightshift", "nodes.json");

export async function GET() {
  try {
    if (!existsSync(NODES_PATH)) {
      return NextResponse.json({ nodes: [] });
    }
    const content = readFileSync(NODES_PATH, "utf-8");
    const nodes = JSON.parse(content);
    return NextResponse.json({ nodes });
  } catch {
    return NextResponse.json({ nodes: [] });
  }
}
