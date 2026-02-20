import { describe, expect, test, mock, afterAll } from "bun:test";
import type { Node } from "../src/redis";

const nodes = new Map<string, Node>();

mock.module("../src/redis", () => ({
  addNode: async (node: Node) => { nodes.set(node.id, node); },
  removeNode: async (id: string) => { nodes.delete(id); },
  listNodes: async () => Array.from(nodes.values()),
  getNode: async (id: string) => nodes.get(id) ?? null,
  nodeExists: async (id: string) => nodes.has(id),
  refreshNodeTTL: async (id: string) => nodes.has(id),
}));

mock.module("../src/providers", () => ({
  getProvider: () => ({
    create: async () => ({ name: "nightshift-abc12345", nodeId: "nightshift-abc12345-8080" }),
    destroy: async (_name: string) => {},
    list: async () => [{ name: "nightshift-abc12345", status: "running" }],
    injectProxyAuth: () => {},
  }),
}));

process.env.PORT = "0";
const { default: server } = await import("../src/index");
const base = `http://localhost:${server.port}`;

afterAll(() => server.stop(true));

const testNode: Node = {
  id: "test-host-19277",
  name: "test-host",
  url: "http://localhost:19277",
  startedAt: "2026-02-16T00:00:00Z",
  os: "linux",
  arch: "x86_64",
  daemonVersion: "0.0.2",
};

describe("node lifecycle", () => {
  test("should register, list, heartbeat, and deregister a node", async () => {
    const postRes = await fetch(`${base}/nodes`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(testNode),
    });
    expect(postRes.status).toBe(201);

    const listRes = await fetch(`${base}/nodes`);
    const { nodes: listed } = await listRes.json();
    expect(listed).toHaveLength(1);
    expect(listed[0].id).toBe(testNode.id);
    expect(listed[0].os).toBe("linux");

    const heartbeatRes = await fetch(`${base}/nodes/${testNode.id}/heartbeat`, { method: "PUT" });
    expect(heartbeatRes.status).toBe(200);

    const deleteRes = await fetch(`${base}/nodes/${testNode.id}`, { method: "DELETE" });
    expect(deleteRes.status).toBe(200);

    const afterDelete = await fetch(`${base}/nodes`);
    const { nodes: remaining } = await afterDelete.json();
    expect(remaining).toHaveLength(0);
  });

  test("should reject node missing required fields", async () => {
    const res = await fetch(`${base}/nodes`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "incomplete" }),
    });
    expect(res.status).toBe(400);
  });

  test("should return 404 for heartbeat on nonexistent node", async () => {
    const res = await fetch(`${base}/nodes/nonexistent/heartbeat`, { method: "PUT" });
    expect(res.status).toBe(404);
  });
});

describe("machine routes", () => {
  test("should create and list machines", async () => {
    const createRes = await fetch(`${base}/machines`, { method: "POST" });
    expect(createRes.status).toBe(201);
    const created = await createRes.json();
    expect(created.name).toStartWith("nightshift-");

    const listRes = await fetch(`${base}/machines`);
    const { machines } = await listRes.json();
    expect(machines.length).toBeGreaterThan(0);
  });

  test("should destroy a machine", async () => {
    const res = await fetch(`${base}/machines/nightshift-abc12345`, { method: "DELETE" });
    expect(res.status).toBe(200);
  });
});

describe("routing", () => {
  test("should return 404 for unknown paths", async () => {
    const res = await fetch(`${base}/nonexistent`);
    expect(res.status).toBe(404);
  });
});
