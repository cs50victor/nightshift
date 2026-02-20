import { readFileSync } from "node:fs";
import { join } from "node:path";
import { nodeExists, removeNode } from "../redis";
import type { VMProvider } from "./provider";

const DO_API = "https://api.digitalocean.com/v2";

const SETUP_SCRIPT = readFileSync(
  join(import.meta.dir, "../scripts/setup-digitalocean.sh"),
  "utf-8",
);

export class DOApiError extends Error {
  constructor(
    public code: string,
    message: string,
    public status: number,
  ) {
    super(message);
    this.name = "DOApiError";
  }
}

function getToken(): string {
  const token = process.env.DIGITALOCEAN_TOKEN;
  if (!token) throw new Error("DIGITALOCEAN_TOKEN not set");
  return token;
}

async function doRequest<T>(
  token: string,
  method: string,
  path: string,
  body?: unknown,
): Promise<T> {
  const res = await fetch(`${DO_API}${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
    },
    body: body ? JSON.stringify(body) : undefined,
  });

  if (res.status === 204) return undefined as T;

  const data = await res.json();

  if (!res.ok) {
    throw new DOApiError(
      data.id ?? "unknown",
      data.message ?? `DO API error: ${res.status}`,
      res.status,
    );
  }

  return data as T;
}

function getServerUrl(): string {
  if (process.env.NODE_ENV === "production") {
    const url = process.env.SERVER_URL;
    if (!url) throw new Error("SERVER_URL not set");
    return url;
  }
  return "https://nightshift-server.fly.dev";
}

interface Droplet {
  id: number;
  name: string;
  status: string;
  networks?: {
    v4?: Array<{ ip_address: string; type: string }>;
  };
}

async function pollDropletActive(
  token: string,
  dropletId: number,
): Promise<{ ip: string }> {
  const interval = 3000;
  const timeout = 600000;
  const start = Date.now();

  while (Date.now() - start < timeout) {
    await new Promise((r) => setTimeout(r, interval));
    const data = await doRequest<{ droplet: Droplet }>(token, "GET", `/droplets/${dropletId}`);
    const droplet = data.droplet;

    if (droplet.status === "active") {
      const pub = droplet.networks?.v4?.find((n) => n.type === "public");
      if (pub) return { ip: pub.ip_address };
    }

    if (droplet.status === "off" || droplet.status === "archive") {
      throw new Error(`droplet ${dropletId} entered terminal state "${droplet.status}"`);
    }
  }

  throw new Error(`droplet ${dropletId} did not become active within ${timeout / 1000}s`);
}

async function pollNodeRegistration(nodeId: string): Promise<void> {
  const interval = 5000;
  const timeout = 600000;
  const start = Date.now();

  while (Date.now() - start < timeout) {
    await new Promise((r) => setTimeout(r, interval));
    const exists = await nodeExists(nodeId);
    if (exists) return;
  }

  throw new Error(`node ${nodeId} did not register within ${timeout / 1000}s`);
}

async function createDropletWithRetry(
  token: string,
  body: Record<string, unknown>,
  maxRetries: number,
): Promise<{ droplet: Droplet }> {
  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      return await doRequest<{ droplet: Droplet }>(token, "POST", "/droplets", body);
    } catch (e) {
      // NOTE(victor): new DO accounts get "finalizing" errors -- retry with backoff
      const isFinalizing =
        e instanceof DOApiError && e.message.toLowerCase().includes("finalizing");
      if (isFinalizing && attempt < maxRetries) {
        await new Promise((r) => setTimeout(r, 5000));
        continue;
      }
      throw e;
    }
  }
  throw new Error("unreachable");
}

export const digitalOceanProvider: VMProvider = {
  async create(displayName?: string): Promise<{ name: string; nodeId: string }> {
    const token = getToken();
    const serverUrl = getServerUrl();
    const region = process.env.DIGITALOCEAN_REGION ?? "nyc1";
    const size = process.env.DIGITALOCEAN_SIZE ?? "s-1vcpu-2gb";
    const image = process.env.DIGITALOCEAN_IMAGE ?? "ubuntu-24-04-x64";

    const slug = displayName
      ? displayName.toLowerCase().replace(/[^a-z0-9-]/g, "-").replace(/-+/g, "-").slice(0, 30)
      : crypto.randomUUID().slice(0, 8);
    const random6 = crypto.randomUUID().slice(0, 6);
    const name = `nightshift-${slug}-${random6}`.replace(/[^A-Za-z0-9.-]/g, "");

    const userData = SETUP_SCRIPT.replace("${NIGHTSHIFT_SERVER_URL}", serverUrl);

    if (new TextEncoder().encode(userData).length >= 64 * 1024) {
      throw new Error("user_data exceeds 64 KiB limit");
    }

    const data = await createDropletWithRetry(token, {
      name,
      region,
      size,
      image,
      user_data: userData,
      tags: ["nightshift"],
    }, 10);

    const dropletId = data.droplet.id;

    try {
      await pollDropletActive(token, dropletId);
      const nodeId = `${name}-8080`;
      await pollNodeRegistration(nodeId);
      return { name, nodeId };
    } catch (e) {
      await doRequest(token, "DELETE", `/droplets/${dropletId}`).catch(() => {});
      throw e;
    }
  },

  async destroy(name: string): Promise<void> {
    const token = getToken();
    await removeNode(`${name}-8080`).catch(() => {});

    const data = await doRequest<{ droplets: Droplet[] }>(
      token,
      "GET",
      "/droplets?tag_name=nightshift&per_page=200",
    );

    const droplet = data.droplets.find((d) => d.name === name);
    if (!droplet) return;

    try {
      await doRequest(token, "DELETE", `/droplets/${droplet.id}`);
    } catch (e) {
      if (e instanceof DOApiError && e.status === 404) return;
      throw e;
    }
  },

  async list(): Promise<Array<{ name: string; status?: string }>> {
    const token = getToken();
    const data = await doRequest<{ droplets: Droplet[] }>(
      token,
      "GET",
      "/droplets?tag_name=nightshift&per_page=200",
    );
    return data.droplets.map((d) => ({
      name: d.name,
      status: d.status,
    }));
  },

  injectProxyAuth(): void {},
};
