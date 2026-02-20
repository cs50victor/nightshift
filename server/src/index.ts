import { addNode, getNode, listNodes, removeNode, refreshNodeTTL, type Node } from "./redis";
import { getProvider } from "./providers";

const provider = getProvider();

async function handleProxy(req: Request, nodeId: string, path: string, search: string): Promise<Response> {
  const node = await getNode(nodeId);
  if (!node) {
    return Response.json({ error: "node not found" }, { status: 404 });
  }

  const target = new URL(path + search, node.url);

  const headers = new Headers(req.headers);
  headers.delete("host");

  provider.injectProxyAuth(target, headers);

  const upstream = await fetch(target.toString(), {
    method: req.method,
    headers,
    body: req.body,
    // @ts-expect-error -- Bun supports duplex streaming for request bodies
    duplex: "half",
  });

  // NOTE(victor): Bun's fetch() auto-decompresses the body, but the original headers
  // still claim content-encoding (e.g. zstd). Strip encoding headers so downstream
  // consumers don't try to decompress an already-decompressed body.
  const respHeaders = new Headers(upstream.headers);
  respHeaders.delete("content-encoding");
  respHeaders.delete("content-length");

  return new Response(upstream.body, {
    status: upstream.status,
    statusText: upstream.statusText,
    headers: respHeaders,
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

    "/machines": {
      GET: async () => {
        const machines = await provider.list();
        return Response.json({ machines });
      },
      POST: async (req) => {
        const body = await req.json().catch(() => ({}));
        const result = await provider.create(body.name);
        return Response.json(result, { status: 201 });
      },
    },

    "/machines/:name": {
      DELETE: async (req) => {
        await provider.destroy(req.params.name);
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
