# Stellar Syndicates

An asynchronous, multiplayer (4–12 player) continuous-time 4X space strategy game
about corporate trade and conflict across a wormhole-linked galaxy. Its defining
mechanic is **lightspeed-delayed observation and command**: you never see the
galaxy as it is *now*, only as the light that has reached your command center —
and your orders cross space at the speed of light, arriving late.

See [`GAME_DESIGN.md`](GAME_DESIGN.md) for the full design and
[`MULTIPLAYER_PROMPT.md`](MULTIPLAYER_PROMPT.md) for the milestone build plan.

---

## Status

| Milestone | State | Notes |
|-----------|-------|-------|
| **M1 — Multiplayer architecture scaffold + sessions** | ✅ **Complete** | Full architecture skeleton, end-to-end, built for many players. |
| M2 — True-world sim (continuous space + acceleration) | ⬜ Not started | |
| M3 — Lightspeed information model (the core) | ⬜ Not started | |
| M4 — Raiding loop (PvP) | ⬜ Not started | |
| M5 — Full multiplayer economy | ⬜ Not started | |
| M6 — Robust sessions, persistence, scale to 12 | ⬜ Not started | |
| M7 — Client polish | ⬜ Not started | |

### What M1 delivers (verified)

- **Pure deterministic `sim` core** (`crates/sim`) — no I/O, no async, no DB. Takes
  a `World` + `Command`s, returns the next state + `Event`s. Seeded RNG, fixed
  timestep, fully unit-tested for determinism.
- **Authoritative server** (`crates/server`) — a single Tokio game-loop task owns
  the `World` and the session registry (lock-free by construction), ticking at
  **30 Hz**.
- **Multiplayer session layer from the start** — many concurrent WebSocket
  connections, each mapped to a player identity (a stable hash of the corp name,
  so reconnecting resumes the same corporation), join/leave handling, a
  per-player outbound stream. A player may hold multiple connections; a
  corporation only goes "offline" when its last connection drops.
- **Per-player broadcast** — every connection receives its *own* message stream
  from the authoritative loop (M1: a live tick + identity; from M3 this becomes
  each player's delayed/fogged view).
- **Postgres persistence off the hot path** (`sqlx`) — append-only event log +
  periodic full-world snapshots, written from a dedicated task that the game loop
  never awaits. Migrations in `crates/server/migrations`. **Falls back to an
  in-memory stub if `DATABASE_URL` is unset or unreachable**, so the server runs
  with zero database setup.
- **Pixi.js client** (`client/`) — connects, identifies as a player, and renders
  a galaxy canvas (starfield + the live authoritative tick) with a HUD showing
  corp, id, tick, sim-time, players-online, and link status. Holds no
  authoritative state and runs no game logic.

**M1 checkpoint proven:** two+ clients connect simultaneously, each gets its own
per-player stream and a live tick from the authoritative loop; joins/leaves are
handled (online count rises and falls correctly). See
[`scripts/m1_smoke.mjs`](scripts/m1_smoke.mjs).

---

## Architecture (§14 of the design)

```
            ┌──────────────────────────────────────────────────────┐
            │  server (Tokio)                                        │
  client ───┤  ┌────────────┐   intents    ┌──────────────────────┐ │
  (Pixi) ◄──┤  │ ws conn    │ ───────────► │ game loop (single     │ │
   WS       │  │ (axum)     │ ◄─────────── │ owner of World +      │ │
            │  └────────────┘  per-player   │ Sessions; 30 Hz tick) │ │
            │       ▲          stream       └──────────┬───────────┘ │
            │       │                                  │ events,      │
            │       │                                  │ snapshots    │
            │       │                          ┌───────▼───────────┐  │
            │       │                          │ persistence task  │  │
            │       │                          │ (sqlx → Postgres, │  │
            │       │                          │  or no-op stub)   │  │
            │       │                          └───────────────────┘  │
            └───────┼──────────────────────────────────────────────┘
                    │ uses (pure, no I/O)
            ┌───────▼───────┐
            │  sim crate    │  World + step(commands) -> events
            │  (deterministic)
            └───────────────┘
```

The pure core is the determinism guarantee and (later) the bot-balance oracle.
Everything that touches the outside world lives outside it.

---

## Running it

### Prerequisites
- Rust (stable; built with 1.91)
- Node 18+ (for the client; built with Node 24)
- *(optional)* PostgreSQL 16 for durable persistence

### 1. Build & run the server

```bash
# from the repo root
cargo run -p server
```

The server listens on `:8080` (HTTP + WebSocket at `/ws`). With no `DATABASE_URL`
it uses the in-memory persistence stub and prints a warning — that's fine for
playing/testing.

Environment knobs: `PORT` (default 8080), `GALAXY_SEED`, `MAX_PLAYERS` (default 4,
sizes the galaxy), `DATABASE_URL`, `SNAPSHOT_EVERY_TICKS` (default 600 = 20 s),
`RUST_LOG` (e.g. `info`).

### 2. Run the client

**Development (hot reload):**
```bash
cd client
npm install
npm run dev          # serves on http://localhost:5173, connects to ws://localhost:8080/ws
```

**Production (one command):** build the client once and the server serves it:
```bash
cd client && npm install && npm run build && cd ..
cargo run -p server                # open http://localhost:8080
```

### 3. Multiple players

Open the client in two or more browser tabs (or machines). Enter a **different
corporation name** in each — each becomes a distinct player with its own stream.
Reconnecting with the same name resumes that corporation.

### Optional: durable persistence with Postgres

A throwaway, isolated dev cluster (does **not** touch your system Postgres):

```bash
scripts/devdb.sh init                 # creates ./.devdb on port 5433 (trust auth)
export DATABASE_URL="$(scripts/devdb.sh url)"
cargo run -p server                   # now writes events + snapshots to Postgres
# ...
scripts/devdb.sh stop                 # or `nuke` to delete it entirely
```

---

## Tests

```bash
cargo test                            # pure-core determinism + session unit tests

# end-to-end M1 checkpoint (server must be running on :8080):
cargo run -p server &                 # in one shell
node scripts/m1_smoke.mjs             # in another — asserts the M1 checkpoint
```

---

## Layout

```
crates/sim/        pure deterministic simulation core (no I/O)
crates/server/     tokio + axum server: game loop, sessions, ws, persistence
  migrations/      sqlx Postgres migrations
client/            Pixi.js + Vite + TypeScript client
scripts/           devdb.sh (local Postgres), m1_smoke.mjs (checkpoint test)
```

## Notes / known stubs

- **Persistence stub:** without `DATABASE_URL` the event log/snapshots are
  dropped (logged, not stored). The Postgres path is real and verified; the stub
  exists purely so the game runs without a database.
- The client galaxy view is intentionally near-blank in M1 (starfield + live
  tick). The real delayed/fogged map arrives in M2/M3.
- Outbound per-connection queues are currently unbounded; M6 will bound them and
  add reconnect/backpressure handling.
