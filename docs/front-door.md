# Front door

Oct 26 2026, Peter: Cloudflare serves the front page, sign-in and register, and
the game server gets one subdomain carrying the game socket, `/status` and
`/healthz`.

## Addresses

| Address | Serves | From |
|---|---|---|
| `stellarsyndicates.com` | The front page, sign-in, register, the game client and its art, the admin page | Cloudflare: static files attached to the `stellar-syndicates-gateway` Worker |
| `stellarsyndicates.com/api/*` | Accounts, notifications, admin data | Worker → tunnel → VPS `127.0.0.1:18080` |
| `play.stellarsyndicates.com` | `/ws`, `/status`, `/healthz` only. `/` redirects to the front page; anything else is 404 | `stellar-syndicates-play` Worker → same tunnel → VPS |
| `www.stellarsyndicates.com`, `stellar-syndicates.surviveperilous.com` | 308 to the front page, keeping path and query | `stellar-syndicates-play` Worker |

The page stays on `stellarsyndicates.com`. After sign-in it opens the game
socket on `play.stellarsyndicates.com`; players never see play. in the address
bar.

## How sign-in reaches the game host

- **Server settings** (`bin/run-app.sh` on the VPS):
  - `APP_ORIGIN=https://stellarsyndicates.com` is the page.
  - `GAME_ORIGIN=https://play.stellarsyndicates.com` is the game host.
  - `COOKIE_DOMAIN=stellarsyndicates.com` shares the cookies with it.
- **Cookies:** with `COOKIE_DOMAIN` set, the sign-in and device cookies carry
  `Domain=stellarsyndicates.com`. They are named `__Secure-ss_session` and
  `__Secure-ss_device`, because the `__Host-` prefix forbids a Domain.
  SameSite=Strict still holds: play. and the page are the same site.
- **Game URL:** sign-in, register, guest and session replies carry
  `game_url: "wss://play.stellarsyndicates.com/ws"`. The client (`net.ts`,
  `useServer`) connects there.
  - It only accepts the page's scheme, this site's host or a subdomain of it,
    and `/ws`.
  - Anything else keeps the page's own host.
- **Origin check:** unchanged. The page's origin is the one allowed, and the
  game socket's Origin is the page's.
- **Defaults:** without `GAME_ORIGIN` and `COOKIE_DOMAIN`, as on :8080 and the
  Vite dev server, everything stays on one host with host-only cookies.
- **Startup checks** (`AuthConfig::with_game_host`) refuse:
  - a cookie domain that doesn't hold the page's host;
  - a game origin outside it;
  - a scheme downgrade;
  - a game host on another domain without a cookie domain.

## Files

- `deploy/cloudflare-proxy/wrangler.jsonc` defines the front-door Worker.
  - `assets.directory` is `../../client/dist`.
  - `run_worker_first` is `["/api/*"]`.
  - `src/index.ts` forwards `/api/*` and answers 404 to anything else that no
    file matches.
- `deploy/cloudflare-proxy/wrangler.play.jsonc` and `src/play.ts` define the
  game-host Worker. It has no files.
- `src/origin.ts` is the shared tunnel forward. It passes the request
  untouched, so cookies and the WebSocket upgrade survive.
- `client/public/_headers` caches `/assets/*` (content-hashed) for a year and
  marks them immutable. Other files keep Cloudflare's default:
  `max-age=0, must-revalidate` with an ETag.

Static file requests are free and unlimited, and don't run the Worker. On the
Workers Free plan a version may hold 20,000 files of up to 25 MiB each. The
client is about 3,100 files.

## Deploying

- **Client change:**
  1. In `client/`, run `npm run build`.
  2. In `deploy/cloudflare-proxy`, run `npm run deploy`. Only changed files
     upload.
- **Server change:** as before. Upload a new VPS release, build it, switch
  `current`, then run `bin/stop-app.sh` and `bin/start-app.sh`.
  - When the protocol version changes, deploy the server first and the client
    straight after. The client's version check forces a reload.
- **Game-host Worker:** `npm run deploy:play`. This is rarely needed.
- **Health check:** `https://play.stellarsyndicates.com/healthz`.

## Rollback

1. Run `npx wrangler rollback` in `deploy/cloudflare-proxy`. This restores the
   pass-through gateway.
2. Re-add the www and old-address routes to `wrangler.jsonc` and remove them
   from the play Worker.
3. Delete the two settings from `bin/run-app.sh`; the backup is
   `run-app.sh.bak-20261026`.
4. Restart the server. Everyone signs in again.

## Later: more game worlds

Each world can get its own host (`w2.stellarsyndicates.com`) with the same three
paths. Accounts stay on the front door, and the session reply would carry a
list of worlds instead of one `game_url`.
