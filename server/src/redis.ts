import { RedisClient } from "bun";

const NODE_TTL_SECS = 180;

export interface Node {
  id: string;
  name: string;
  url: string;
  startedAt: string;
  os: string;
  arch: string;
  daemonVersion: string;
  machineName?: string;
}

const redis = new RedisClient(process.env.REDIS_URL);

export async function addNode(node: Node): Promise<void> {
  const json = JSON.stringify(node);
  await redis.send("SET", [`node:${node.id}`, json, "EX", NODE_TTL_SECS.toString()]);
  await redis.sadd("nodes", node.id);
}

export async function removeNode(id: string): Promise<void> {
  await redis.srem("nodes", id);
  await redis.del(`node:${id}`);
}

export async function listNodes(): Promise<Node[]> {
  const ids = await redis.smembers("nodes");
  if (ids.length === 0) return [];

  const nodes: Node[] = [];
  const staleIds: string[] = [];

  for (const id of ids) {
    const raw = await redis.get(`node:${id}`);
    if (raw === null) {
      staleIds.push(id);
      continue;
    }
    try {
      nodes.push(JSON.parse(raw));
    } catch {
      staleIds.push(id);
    }
  }

  for (const id of staleIds) {
    await redis.srem("nodes", id);
  }

  return nodes;
}

export async function getNode(id: string): Promise<Node | null> {
  const raw = await redis.get(`node:${id}`);
  if (raw === null) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export async function nodeExists(id: string): Promise<boolean> {
  return redis.exists(`node:${id}`);
}

export async function refreshNodeTTL(id: string): Promise<boolean> {
  const exists = await redis.exists(`node:${id}`);
  if (!exists) return false;
  await redis.expire(`node:${id}`, NODE_TTL_SECS);
  return true;
}
