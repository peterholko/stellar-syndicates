const ORIGIN_HOST = "127.0.0.1";
const ORIGIN_PORT = "18080";

export default {
  async fetch(request, env): Promise<Response> {
    const originUrl = new URL(request.url);
    originUrl.protocol = "http:";
    originUrl.hostname = ORIGIN_HOST;
    originUrl.port = ORIGIN_PORT;

    try {
      // Passing the request through untouched preserves streaming bodies and the
      // WebSocket Upgrade handshake; only its private-network destination changes.
      return await env.ORIGIN.fetch(new Request(originUrl, request));
    } catch (error) {
      console.error(JSON.stringify({
        event: "origin_fetch_failed",
        method: request.method,
        path: originUrl.pathname,
        error: error instanceof Error ? error.message : String(error),
      }));
      return new Response("Stellar Syndicates is temporarily unavailable.", {
        status: 502,
        headers: { "content-type": "text/plain; charset=utf-8" },
      });
    }
  },
} satisfies ExportedHandler<Env>;
