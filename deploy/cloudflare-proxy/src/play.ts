// §front-door (docs/front-door.md): play.stellarsyndicates.com, the game
// server's own address. Only the game socket, /status and /healthz reach the
// VPS through the tunnel; its root sends visitors to the front page. www
// redirects to the front page too.
import { toOrigin } from "./origin";

const FRONT = "https://stellarsyndicates.com";
const GAME_HOST = "play.stellarsyndicates.com";
const GAME_PATHS = new Set(["/ws", "/status", "/healthz"]);

export default {
  async fetch(request, env): Promise<Response> {
    const url = new URL(request.url);
    if (url.hostname !== GAME_HOST) return Response.redirect(`${FRONT}${url.pathname}${url.search}`, 308);
    if (GAME_PATHS.has(url.pathname)) return toOrigin(request, env);
    if (url.pathname === "/") return Response.redirect(`${FRONT}/`, 308);
    return new Response("Not found", { status: 404, headers: { "content-type": "text/plain; charset=utf-8" } });
  },
} satisfies ExportedHandler<Env>;
