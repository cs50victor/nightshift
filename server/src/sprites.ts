import { readFileSync } from "node:fs";
import { join } from "node:path";
import { SpritesClient } from "@fly/sprites";
import { nodeExists, removeNode } from "./redis";

const POLL_INTERVAL_MS = 2000;
const POLL_TIMEOUT_MS = 30000;

const DAEMON_RELEASE_URL =
  "https://github.com/cs50victor/nightshift/releases/latest/download/nightshift-daemon-x86_64-unknown-linux-gnu.tar.gz";

const SETUP_SCRIPT = readFileSync(
  join(import.meta.dir, "scripts/setup-sprite.sh"),
  "utf-8",
);

function getClient(): SpritesClient {
  const token = process.env.SPRITES_TOKEN;
  if (!token) throw new Error("SPRITES_TOKEN not set");
  return new SpritesClient(token);
}

function getServerUrl(): string {
  const url = process.env.SERVER_URL;
  if (!url) throw new Error("SERVER_URL not set");
  return url;
}

export async function createSprite(displayName?: string): Promise<{ name: string; nodeId: string }> {
  const client = getClient();
  const serverUrl = getServerUrl();
  const slug = displayName
    ? displayName.toLowerCase().replace(/[^a-z0-9-]/g, "-").replace(/-+/g, "-").slice(0, 30)
    : crypto.randomUUID().slice(0, 8);
  const name = `nightshift-${slug}`;

  const sprite = await client.createSprite(name);

  try {
    const publicUrl = `https://${name}.sprites.dev:8080`;

    await sprite.execFile("bash", ["-c", SETUP_SCRIPT], {
      env: {
        DAEMON_RELEASE_URL,
        NIGHTSHIFT_SERVER_URL: serverUrl,
        NIGHTSHIFT_PUBLIC_URL: publicUrl,
        NIGHTSHIFT_PROXY_PORT: "8080",
      },
    });

    const nodeId = await pollNodeRegistration(name);
    return { name, nodeId };
  } catch (e) {
    await client.deleteSprite(name).catch(() => {});
    throw e;
  }
}

async function pollNodeRegistration(spriteName: string): Promise<string> {
  const start = Date.now();
  while (Date.now() - start < POLL_TIMEOUT_MS) {
    await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
    const exists = await nodeExists(`${spriteName}-8080`);
    if (exists) return `${spriteName}-8080`;
  }
  throw new Error(`sprite ${spriteName} did not register within ${POLL_TIMEOUT_MS / 1000}s`);
}

export async function destroySprite(name: string): Promise<void> {
  const client = getClient();
  await removeNode(`${name}-8080`).catch(() => {});
  await client.deleteSprite(name);
}

export async function listSprites(): Promise<Array<{ name: string; status: string | undefined }>> {
  const client = getClient();
  const sprites = await client.listAllSprites("nightshift-");
  return sprites.map((s) => ({
    name: s.name,
    status: s.status,
  }));
}
