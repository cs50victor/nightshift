import { readFileSync } from "node:fs";
import { join } from "node:path";
import { SpritesClient } from "@fly/sprites";
import { nodeExists, removeNode } from "../redis";
import { getServerUrl, type VMProvider } from "./provider";

const POLL_INTERVAL_MS = 2000;
const POLL_TIMEOUT_MS = 60000;

const DAEMON_RELEASE_URL =
  "https://github.com/cs50victor/nightshift/releases/latest/download/nightshift-daemon-x86_64-unknown-linux-gnu";

const SETUP_SCRIPT = readFileSync(
  join(import.meta.dir, "../scripts/setup-sprite.sh"),
  "utf-8",
);

function getToken(): string {
  const token = process.env.SPRITES_TOKEN;
  if (!token) throw new Error("SPRITES_TOKEN not set");
  return token;
}

function getClient(): SpritesClient {
  return new SpritesClient(getToken());
}

// NOTE(victor): SDK doesn't expose the sprite URL in its types.
// Fetch it from the REST API directly.
async function getSpriteUrl(name: string): Promise<string> {
  const res = await fetch(`https://api.sprites.dev/v1/sprites/${encodeURIComponent(name)}`, {
    headers: { Authorization: `Bearer ${getToken()}` },
  });
  if (!res.ok) throw new Error(`failed to get sprite URL: ${res.status}`);
  const data = await res.json();
  console.log("[sprite api]", JSON.stringify(data, null, 2));
  if (!data.url) throw new Error("sprite API returned no url");
  return data.url;
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

export const spritesProvider: VMProvider = {
  async create(displayName?: string): Promise<{ name: string; nodeId: string }> {
    const client = getClient();
    const serverUrl = getServerUrl();
    const slug = displayName
      ? displayName.toLowerCase().replace(/[^a-z0-9-]/g, "-").replace(/-+/g, "-").slice(0, 30)
      : crypto.randomUUID().slice(0, 8);
    const name = `nightshift-${slug}`;

    const sprite = await client.createSprite(name);

    try {
      const publicUrl = await getSpriteUrl(name);

      const result = await sprite.execFile("bash", ["-c", SETUP_SCRIPT], {
        env: {
          SPRITE_NAME: name,
          DAEMON_RELEASE_URL,
          NIGHTSHIFT_SERVER_URL: serverUrl,
          NIGHTSHIFT_PUBLIC_URL: publicUrl,
          NIGHTSHIFT_PROXY_PORT: "8080",
        },
      });

      console.log("[sprite setup]", result.stdout);
      if (result.stderr) console.error("[sprite setup stderr]", result.stderr);

      const nodeId = await pollNodeRegistration(name);
      return { name, nodeId };
    } catch (e) {
      await client.deleteSprite(name).catch(() => {});
      throw e;
    }
  },

  async destroy(name: string): Promise<void> {
    const client = getClient();
    await removeNode(`${name}-8080`).catch(() => {});
    await client.deleteSprite(name);
  },

  async list(): Promise<Array<{ name: string; status?: string }>> {
    const client = getClient();
    const sprites = await client.listAllSprites("nightshift-");
    return sprites.map((s) => ({
      name: s.name,
      status: s.status,
    }));
  },

  injectProxyAuth(targetUrl: URL, headers: Headers): void {
    if (targetUrl.hostname.endsWith(".sprites.app") || targetUrl.hostname.endsWith(".sprites.dev")) {
      const token = process.env.SPRITES_TOKEN;
      if (token) {
        headers.set("Authorization", `Bearer ${token}`);
      }
    }
  },
};
