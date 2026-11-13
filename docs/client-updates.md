# Client updates

Nov 11 2026. How an open tab picks up a newer client after a deploy. Peter:
"if we deploy to production, how will we ensure the players receive the
latest client side app version?"

## Before

- A fresh page load already got the newest client: the page is revalidated
  on every load and the bundle's files are named by content
  (`client/public/_headers`). The service worker only shows push alerts; it
  caches nothing.
- An open tab ran whatever it loaded, for good. Nothing checked for a newer
  build, so a client-only deploy never reached it.
- After a protocol bump the server closes an old tab with code 4002 ("reload
  the game"), but Chromium never saw it. The tab offered its old
  `stellar.msgpack.vN`; the server's handshake named no protocol, and
  Chromium fails such a handshake with 1006 (tested in the desktop app's
  pane). The tab showed "reconnecting…" and retried every 30 s for ever.

## Now

- **Build stamp** (`client/vite.config.ts`, `buildStamp`): every build hashes
  the names of the files it emits (content-hashed by Vite, so the stamp
  changes exactly when the client does) into each page's
  `<meta name="ss-build">` and into `dist/version.json`
  (`{"build": "…", "protocol": 91}`), served `no-store`. Rebuilding unchanged
  code keeps the stamp.
- **Checks** (`client/src/client-update.ts`): when the tab comes back into
  view (at most once per 30 s), every 5 minutes, and at once when the server
  closes the socket for an old protocol (`NetHandlers.onProtocolChanged`).
  The Vite dev server's pages say `dev` and never check.
- **On a newer build:**
  - Out of view, the tab reloads itself.
  - In view, a pill under the top bars: "New version · Reload", or "Update
    required · Reload" when the protocol differs (that tab can't reach the
    server any more). Leaving the tab with the pill up reloads it.
  - A tab reloads itself once per build (`sessionStorage`), so a cache still
    serving the old page mid-deploy can't loop it; it shows the pill instead.
- **Old tabs' handshake** (`crates/server/src/ws.rs`, `accepted_protocol`):
  the server names the tab's own `stellar.msgpack.vN`, so every browser opens
  the socket and receives the 4002 close.
- **One release script** (`deploy/release.sh`): client build, server build on
  the box, switch and restart, then the client upload straight after, then
  checks that `/healthz` answers and the live `version.json` matches the
  build. Untested against the box: production has been down since the Nov 6
  reinstall (docs/vps-debian-setup.md).

## Tests

- `client/scripts/test-client-update.mjs`: offer in view, reload out of view
  once per build, required on a new protocol, the burst guard, the wiring.
- `crates/server/src/ws/tests.rs`: an old tab gets its own protocol back and
  then the 4002 close; which offers are echoed.
- Preview: `/scripts/update-fixture.html` on the Vite dev server shows both
  pills (`?need=required`).
