// §front-door (docs/front-door.md): stellarsyndicates.com. Cloudflare serves
// the client's files itself (`assets` in wrangler.jsonc); this script runs only
// for the account service under /api/, which reaches the VPS through the
// tunnel, and for paths no file matches. The game socket, /status and
// /healthz live on play.stellarsyndicates.com (src/play.ts).
import { toOrigin } from "./origin";

export default {
  async fetch(request, env): Promise<Response> {
    if (new URL(request.url).pathname.startsWith("/api/")) return toOrigin(request, env);
    return new Response("Not found", { status: 404, headers: { "content-type": "text/plain; charset=utf-8" } });
  },
} satisfies ExportedHandler<Env>;
