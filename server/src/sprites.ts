import { SpritesClient } from "@fly/sprites";
import { nodeExists, removeNode } from "./redis";

const POLL_INTERVAL_MS = 2000;
const POLL_TIMEOUT_MS = 30000;

const DAEMON_RELEASE_URL =
  "https://github.com/cs50victor/nightshift/releases/latest/download/nightshift-daemon-x86_64-unknown-linux-gnu.tar.gz";

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
    await sprite.exec("bun install -g opencode-ai@1.2.1");

    await sprite.exec(
      `mkdir -p ~/.nightshift && curl -fsSL ${DAEMON_RELEASE_URL} | tar -xz -C ~/.nightshift && chmod +x ~/.nightshift/nightshift-daemon`,
    );

    const publicUrl = `https://${name}.sprites.dev`;
    const config = JSON.stringify({
      version: 1,
      serverUrl,
      publicUrl: `${publicUrl}:8080`,
      proxyPort: 8080,
    });
    await sprite.exec(`cat > ~/.nightshift/config.json << 'EOFCFG'\n${config}\nEOFCFG`);

    const serviceBody = JSON.stringify({
      cmd: "/home/sprite/.nightshift/nightshift-daemon",
      args: ["daemon"],
    });
    await sprite.exec(
      `curl -s -X PUT http://localhost/v1/services/nightshift -H 'Content-Type: application/json' -d '${serviceBody}'`,
    );

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
