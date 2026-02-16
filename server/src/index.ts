const server = Bun.serve({
  port: 8080,
  fetch() {
    return new Response("ok");
  },
});

console.log(`server listening on :${server.port}`);
