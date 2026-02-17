import { addNode, getNode, listNodes, removeNode, refreshNodeTTL, type Node } from "./redis";
import { createSprite, destroySprite, listSprites } from "./sprites";

async function handleProxy(req: Request, nodeId: string, path: string, search: string): Promise<Response> {
  const node = await getNode(nodeId);
  if (!node) {
    return Response.json({ error: "node not found" }, { status: 404 });
  }

  const target = new URL(path + search, node.url);

  const headers = new Headers(req.headers);
  headers.delete("host");

  // NOTE(victor): sprite URLs are private -- inject auth so the UI doesn't need the token
  if (target.hostname.endsWith(".sprites.app") || target.hostname.endsWith(".sprites.dev")) {
    const token = process.env.SPRITES_TOKEN;
    if (token) {
      headers.set("Authorization", `Bearer ${token}`);
    }
  }

  const upstream = await fetch(target.toString(), {
    method: req.method,
    headers,
    body: req.body,
    // @ts-expect-error -- Bun supports duplex streaming for request bodies
    duplex: "half",
  });

  // NOTE(victor): pass body stream through directly -- this preserves SSE (text/event-stream)
  return new Response(upstream.body, {
    status: upstream.status,
    statusText: upstream.statusText,
    headers: upstream.headers,
  });
}

const server = Bun.serve({
  port: Number(process.env.PORT ?? 8080),
  routes: {
    "/health": () => Response.json({ ok: true }),

    "/nodes": {
      GET: async () => {
        const nodes = await listNodes();
        return Response.json({ nodes });
      },
      POST: async (req) => {
        const body = (await req.json()) as Node;
        if (!body.id || !body.url) {
          return Response.json({ error: "missing required fields: id, url" }, { status: 400 });
        }
        await addNode(body);
        return Response.json({ ok: true }, { status: 201 });
      },
    },

    "/nodes/:id": {
      DELETE: async (req) => {
        await removeNode(req.params.id);
        return Response.json({ ok: true });
      },
    },

    "/nodes/:id/heartbeat": {
      PUT: async (req) => {
        const refreshed = await refreshNodeTTL(req.params.id);
        if (!refreshed) {
          return Response.json({ error: "node not found" }, { status: 404 });
        }
        return Response.json({ ok: true });
      },
    },

    "/sprites": {
      GET: async () => {
        const sprites = await listSprites();
        return Response.json({ sprites });
      },
      POST: async (req) => {
        const body = await req.json().catch(() => ({}));
        const result = await createSprite(body.name);
        return Response.json(result, { status: 201 });
      },
    },

    "/sprites/:name": {
      DELETE: async (req) => {
        await destroySprite(req.params.name);
        return Response.json({ ok: true });
      },
    },
  },

  fetch(req) {
    const url = new URL(req.url);
    const match = url.pathname.match(/^\/proxy\/([^/]+)(\/.*)?$/);
    if (match) {
      return handleProxy(req, decodeURIComponent(match[1]), match[2] ?? "/", url.search);
    }
    return Response.json({ error: "not found" }, { status: 404 });
  },

  error(e) {
    console.error("request failed:", e);
    return Response.json({ error: e.message }, { status: 500 });
  },
});

console.log(`server listening on :${server.port}`);

export default server;
