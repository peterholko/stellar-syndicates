# Admin dashboard

`/admin.html` (or `/admin`) is the operator's view of the server: who registered
and from where, who is online, sign-in history, play time, every command each
player sent, and each corporation's holdings. It is read-only.

## Access

- Set `ADMIN_LOGINS` to a comma-separated list of account logins, for example
  `ADMIN_LOGINS=peter`. Sign in to the game in the same browser, then open
  `/admin.html`.
- Without `ADMIN_LOGINS` the dashboard is off (`404`). The audit trail is
  recorded either way, so turning the dashboard on later shows the history.
- Guests, disabled accounts and logins not on the list get `403`.

## What is recorded

All of it is in the accounts database, beside `accounts`. A galaxy reset never
touches it; rows go only when their account is deleted.

| Table | One row per | Holds |
|---|---|---|
| `account_logins` | register, sign-in, new guest, guest registering | time, IP, user agent, language, device id |
| `play_sessions` | game socket, from Join to close | start, end, last heartbeat, IP, user agent, device id, command count, active time |
| `player_actions` | command a player sent | time, kind (`MoveShip`, `BuildShip`, …), the message body (bodies over 4 KB are omitted) |

- **Device id:** a random id in a long-lived HttpOnly cookie (`ss_device`,
  `__Host-ss_device` over HTTPS), issued at sign-in or on the next visit. It
  identifies a browser, authorizes nothing, and is not a fingerprint.
- **IP:** the socket's peer address. The public site reaches the server through
  the Cloudflare tunnel, so every visitor arrives from loopback; for loopback
  peers only, the visitor's address is read from `CF-Connecting-IP`. A request
  that reaches the port directly is recorded as itself. Rate limiting still uses
  the peer address.
- **Not commands:** Join, keepalives, battle estimates and ledger reads.
- **Active time:** five-minute windows with at least one command. Time online is
  the socket's whole lifetime, including an idle background tab.

Holdings (systems, settled worlds, population, fleets, ships, credits,
valuation) are read from the game loop when the dashboard asks; they never
reach a player's view.

## Tutorial funnel

New players are most likely to leave during the founding charter, so the
dashboard follows each one through it and shows where they stop.

- **Steps**, in the guide's own words: sign up, enter the game, found a colony;
  Landfall: bring the mine online, send your first shipment, defeat the Rogue
  Privateer; Industry: build a Shipyard, start research, chart a nearby system;
  Reach: build a Mining Barge, mine the belt, bring the ore home (charter v3,
  [founding-charter.md](founding-charter.md)).
- **Source:** the game loop's founding programme (`FoundingProgram`: stage
  stamps and report arrivals). The shipment, privateer and haul steps also read
  their own report stamps, so they count as soon as each report arrives. A
  later stage stamp proves the steps it requires
  (`crates/server/src/tutorial.rs`).
- **Recording:** a poller asks the game loop every 15 seconds, and each
  dashboard load asks too. The first time an account reaches a step is written
  to `tutorial_progress` (migration 0005), keyed by galaxy, with the game time
  and the wall time counted back from it at the server's pacing. `galaxies`
  holds each galaxy's start. A step reached before a restart reads later by
  however long the server was down.
- **Funnel:** everyone who arrived during the current galaxy (signed up,
  played or progressed since it began), operators left out. For each step it
  shows how many reached it, the share kept from the step before, the median
  time from the step before, and who is on it now: still playing, or gone
  quiet (no activity for 24 hours). Click a count to list those players. The
  step where most players went quiet is highlighted.
- **Per player:** the players table shows steps done and the step the player
  is on, with how long they have waited; the drawer shows the whole timeline.
- Steps of one chapter can be done out of order. A player's step is the first
  one not yet done, as the guide's next action is.

## Shared browsers and networks

An account's detail lists other accounts seen on the same device id or IP;
loopback addresses are ignored. The overview flags accounts that share a
browser with another.

## Writes and failure

One background task writes everything; sockets hand it events over a bounded
queue and drop them when it is full, so the database never slows a socket or a
tick. Commands are written in batches every 5 seconds, open sessions get a
heartbeat every minute, and a clean shutdown closes them. After a hard stop the
next start closes open sessions at their last heartbeat.

## Code

- `crates/server/migrations/0004_admin_activity.sql`
- `crates/server/src/activity.rs`: recorder, client IP and device id
- `crates/server/src/admin.rs`: `/api/admin/overview`, `/api/admin/players/{id}`
- `crates/server/src/tutorial.rs` + `migrations/0005_tutorial_progress.sql`:
  charter steps, the recorder and the funnel
- `crates/server/src/auth.rs`: sign-in records and the device cookie
- `client/admin.html`, `client/src/admin/`, `client/src/styles/admin.css`
- Tests: `admin::tests` (Postgres, run with `TEST_ACCOUNTS_DATABASE_URL` and
  `--ignored`), `activity::tests`, `tutorial::tests`,
  `client/scripts/test-admin.mjs` (also checks the client's step order against
  `tutorial.rs`).
  `node client/scripts/test-admin.mjs --serve` (launch entry `admin-fixture`)
  shows the page with sample data and no server.

## Privacy

This stores personal data (IPs, device ids, browser details, a log of play).
A public deployment should say so in its privacy notice. Nothing is deleted
automatically.
