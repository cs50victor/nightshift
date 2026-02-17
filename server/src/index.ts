import { addNode, listNodes, removeNode, refreshNodeTTL, type Node } from "./redis";
import { createSprite, destroySprite, listSprites } from "./sprites";

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

  fetch() {
    return Response.json({ error: "not found" }, { status: 404 });
  },

  error(e) {
    console.error("request failed:", e);
    return Response.json({ error: e.message }, { status: 500 });
  },
});

console.log(`server listening on :${server.port}`);

export default server;
