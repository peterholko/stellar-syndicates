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
| **M2 — True-world sim (continuous space + acceleration)** | ✅ **Complete** | Galaxy, ships, flip-and-burn physics; clients render the shared moving world. |
| **M3 — Lightspeed information model (the core)** | ✅ **Complete** | Per-player delayed/fogged views from each command center; fairness guarantee enforced & adversarially reviewed; command latency. |
| **M4 — Raiding loop (PvP)** | ✅ **Complete** | Intercept-commit pursuit; resolution in true space; delayed reports on each player's own clock; recall can miss. |
| **M5 — Full multiplayer economy** | ✅ **Complete** | Hub Exchange (instant execution, lagged ticker), market + limit orders with uniform-price batch clearing, raidable trade convoys, buy/sell asymmetry, slow equity valuations. |
| **M6 — Robust sessions, persistence, scale to 12** | ✅ **Complete** | Restart restores the galaxy from the latest snapshot; 12 players in one galaxy with the loop keeping up; corps persist + reconnect resumes. |
| **M7 — Client polish** | ✅ **Complete** | Credits/equity in the HUD, the full delayed-map + market + raid UI tied together, and a run + play guide; the core loop is playable by multiple people. |

**All seven milestones of the build plan are complete** — plus three additive
features layered on the core: the **signals animation** (the order's full round
trip visualized), and the **two-tier information model** (Convention broadcast +
sensor-range detection).

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

### What M2 delivers (verified)

- **Continuous 2D galaxy in the pure core** — a central wormhole hub, seeded
  procedurally-placed star systems (area-uniform), and a ring of home anchors
  assigned to players on join. Galaxy radius scales with player count (§4).
- **Flip-and-burn movement (§7)** — ships have position + velocity and move
  under an acceleration-limited controller that always plans the arrival burn
  (accelerate, flip, decelerate to arrive **at rest**; travel time ≈ 2·√(d/a)).
  Convoy (slow/heavy) vs raider (fast/light) is just two parameters. All speeds
  stay below `c`. Unit-tested for arrival-at-rest, travel time, the speed cap,
  and determinism.
- **Shared advancing world** — the game loop integrates movement each tick; each
  player gets a `View` of ships + anchors (M2: true positions — explicitly
  temporary until the M3 delay layer). On join a player gets a demo convoy +
  raider that patrol, so the world is visibly alive.
- **Pixi map** — renders the hub, systems (with designations), home anchors
  (own highlighted), and ships as velocity-oriented markers, smoothly
  extrapolated between server updates.

**M2 checkpoint proven:** ships move with flip-and-burn; multiple clients see the
same world advancing with identical positions. See
[`scripts/m2_smoke.mjs`](scripts/m2_smoke.mjs).

### What M3 delivers (verified) — the core

- **Per-player lightspeed view filter** (`crates/server/src/view.rs`, a
  first-class component): keeps every ship's recent true-position history and,
  for each player, reconstructs what the light reaching THEIR command center
  shows — every object at its *retarded* position (where it was when the
  arriving light left it).
- **The fairness guarantee, made exact.** A sample `(t, p)` is observable at a
  command center `cc` iff `t + |p − cc|/c ≤ now`. Because ships move slower than
  `c`, `arrival(t)` is strictly increasing, so the filter shows the unique latest
  observable sample and nothing fresher — provably no leak. Verified by unit
  tests *and* a wire-level smoke test that checks every ghost's staleness equals
  its light-distance, plus an **adversarial multi-agent review** that hunted for
  leaks. That review found two presence leaks (anchor-ownership and a global
  player-count revealed instantly); **both are fixed** — anchor ownership is now
  light-gated, and presence/ops state moved to a separate `/status` meta endpoint
  outside the game view.
- **Two fog regimes (§6):** your own ships are delayed-but-coherent (no
  uncertainty); rivals are shown at a stale position with an **uncertainty cone**
  (`age · max_speed` — how far they could have moved since the light left) and an
  age label, fading with staleness.
- **Command latency / the three clocks (§6):** a move order travels to the ship
  at light speed (scheduled in the pure core), and the player learns the result
  later still via their delayed view. The client shows the estimate from its
  stale sighting — you command on old intel, and the real delay differs.
- **Each player sees a genuinely different delayed galaxy.** Distant things are
  stale; nearer things fresher; rivals are dark until their light arrives.

**M3 checkpoint proven:** two players each see their own coherent delayed/fogged
view; staleness equals light-distance on the wire; commands lag; no information
(positions, presence, or counts) leaks between players' horizons. See
[`scripts/m3_smoke.mjs`](scripts/m3_smoke.mjs).

### What M4 delivers (verified) — player-vs-player raiding

- **Intercept-commit (§8):** a player commits a raider to a target; the raider
  pursues autonomously (`movement::intercept_step` solves the lead point) — no
  real-time piloting. The commit is a novel command to a mobile asset, so it
  travels at light speed: the raider begins pursuing only once the order reaches
  it, and it chases the target's *true* position, not the stale ghost the player
  committed on.
- **Randomized resolution in true space:** contact within `CONTACT_RADIUS` rolls
  a **battle** (not an auto-kill) — convoy destroyed, raider destroyed, both
  destroyed, or both survive (driven off). A convoy reaching the hub
  (`HUB_SAFE_RADIUS`) still escapes before contact. **Raiders can now intercept
  rival raiders too** (same commit/contact machinery), with their own even-odds
  battle table. All rolls use the **seeded sim `Rng`** (`crates/sim/src/rng.rs`)
  — one roll per battle, reproducible from seed + commands, no `std` rand.
- **Delayed reports on each player's own clock (§14):** a per-player *event*
  scheduler (`crates/server/src/reports.rs`) holds each battle outcome until its
  light reaches that player's command center, so **attacker and defender learn
  it at different times** — verified on the wire (e.g. attacker 19s stale,
  defender 8s, each equal to its own light-distance).
- **Destruction observed through each player's delayed frame (§6):** a battle
  resolves ONCE in true space with ONE outcome; both players observe that *same*
  fixed result, each delayed by light — never a per-viewer re-roll. A destroyed
  ship does **not** blink out: each player keeps seeing it as a light-delayed
  **ghost flying on old light** until the destruction's light reaches *their*
  command center (`T + |P − CC| / c`), then it vanishes. The view filter
  (`crates/server/src/view.rs`, `mark_destroyed` + the per-viewer gate) enforces
  this, so attacker and defender watch the *same* ship die at *different* times.
  Because a raider is only shown inside the viewer's *sensor coverage*, a
  destroyed raider's detection is latched to its **own retarded frame**
  (`detected_at_retarded_time`): the winner breaking off home can't pull its
  sensor bubble off the kill and make the dead raider blink out before its
  destruction light arrives — without ever revealing a raider the viewer never
  tracked. (Convoys broadcast galaxy-wide, so they were always correct; the
  raider sensor-gated path is the subtle case, covered by four RVR view tests.)
- **Recall can miss the window:** a recall is light-delayed too; if the raider
  has already made contact, you are "commanding into the past" (deterministic
  sim tests cover intercept, successful recall, and recall-too-late).
- **Client:** select your raider, click a rival ghost to raid it, press **R** to
  recall; delayed battle reports surface as a news log phrased per outcome and
  role ("your convoy was destroyed by a rival raider — delayed news, 25s old").

**M4 checkpoint proven:** A raids B's convoy under honest delay; the battle has
ONE seeded outcome both players observe on their own clocks; a destroyed ship
lingers as a ghost per-viewer until its light arrives (attacker and defender
see it vanish at different times); recall can miss. See
[`scripts/m4_smoke.mjs`](scripts/m4_smoke.mjs) and the two-player battle
observer [`scripts/battle_smoke.mjs`](scripts/battle_smoke.mjs) (+ sim battle
tests and `view::tests::destroyed_ship_vanishes_per_viewer_by_light`).

### Signals animation (additive — visualizing the communication delay)

Two traveling signals make the lightspeed delay legible, as **client-side
feedback driven entirely by server-authoritative timing** (the client computes no
delay and never sees true positions):

- **Order round trip** (violet) — the three clocks of §6 made fully legible:
  when you issue any order, the server sends a
  `CommandSignal { ship_id, depart_time, arrive_time, observe_time }` the moment
  it accepts the order. The client renders the whole round trip:
  1. *Comet out* over `[depart, arrive]` — a violet comet crosses from your
     command center to the commanded ship's **live ghost** (endpoint is the ghost
     the renderer already draws, so it meets it and cannot overshoot).
  2. *Order received* — a brief pulse at the ghost when the comet lands.
  3. *Response light home* over `[arrive, observe]` — a faint violet pulse
     travels back from the ship toward your command center, with a status label
     **"RECEIVED · response light ~Xs"** counting down. This fills what used to be
     a dead, unexplained gap: the ship hasn't visibly reacted yet *because the
     light of its maneuver is still on its way home*.
  4. At `observe`, the return light arrives and the ghost's new course becomes
     visible — so the course change is explained (it coincides with the response
     light landing), not mysterious.

  `arrive − depart` and `observe − arrive` each equal the player's *observed*
  one-way light delay to the ship (its ghost's staleness), so nothing reveals the
  ship's true distance — the round trip is the player's honest estimate from their
  delayed view, and the client only interpolates between the server's three times.
- **Inbound result rings** (gold): when a raid report becomes observable (M4's
  per-player delivery already gates this by light), gold rings depart the
  resolution point and travel home to the command center, **revealing the verdict
  on arrival**. This reuses the existing `RaidReport` (`pos` + `age`) — already
  fair, since the player has that data — so no new protocol was needed for it.

The single source of truth is the server's per-player observed stream, so the old
prototype's bugs ("comet overshoots the ghost", "report leaves before you see the
resolution") are structurally impossible. Smoothing/interpolation between
server-provided endpoints and times is the only client-side computation.

**Protocol addition:** `ServerMsg::CommandSignal { ship_id, depart_time,
arrive_time, observe_time }` (server→client, to the issuing player only) in
`crates/server/src/protocol.rs` + `client/src/protocol.ts` — the three clock-times
of the order's round trip. The inbound raid rings needed no addition (they reuse
`RaidReport`'s `pos` + `age`).

### Two-tier information model (broadcast + sensor range)

A second layer of "what each player is allowed to see" sits on top of the
lightspeed delay — and it is enforced **in the view filter**, so it is part of
the fairness guarantee, not a client effect. One law still governs everything:
all information travels at `c`. Nothing here is instant.

- **Tier 1 — broadcast (the Galactic Convention), galaxy-wide, light-delayed.**
  Convoys broadcast identity + position + route, so every convoy (yours and
  rivals') appears as a light-delayed ghost galaxy-wide. **Raiders do not
  broadcast — they are dark.**
- **Tier 2 — sensor range.** Each of a player's assets (every ship + the command
  center) projects a `sensor_range` detection radius; coverage is their union.
  Within coverage you learn more: a convoy's **cargo** is revealed, and a **dark
  raider becomes visible**. Outside coverage, cargo is withheld and a rival
  raider is **omitted from the view payload entirely** — your only warning of an
  approaching raider is the moment it trips your sensors.

**View-filter change & the no-leak choice** (`crates/server/src/view.rs`):
`view_for` now (1) includes all convoys with route, (2) attaches cargo only when
the convoy is within the viewer's coverage, and (3) includes a raider only when
within coverage — otherwise it is *omitted server-side*, never sent-and-hidden.
Detection is computed in the **command center's delayed composite frame**: an
object is "in coverage" when its **delayed ghost** falls within `sensor_range` of
an asset's **delayed ghost** (or the command center). This uses only light that
has arrived, so it never reveals the true position of a dark ship (you still only
see where it *was*), and it matches exactly what the client draws — a detected
raider always appears inside a drawn coverage circle.

**Protocol additions:** `GalaxyInfo.sensor_range`; `GhostView.route` (convoy
broadcast waypoints) and `GhostView.cargo` (present only in range); a `CargoView`
+ `Commodity`. In the sim: a `sensor_range` config constant and an
`Option<Cargo>` on ships (convoys carry demo cargo; raiders carry none).

**Client visualizations:** soft teal **sensor-coverage** bubbles around your
assets; convoy **routes** (waypoints + path, light-delayed); **cargo labels**
shown when known (gold for an in-range rival's manifest — intel) and `cargo ?`
when out of range; a detected rival raider rendered as a **pulsing red "⚠ RAIDER"
threat contact**.

**Verified** (`scripts/sensor_smoke.mjs` + 6 view-filter unit tests): convoys
broadcast galaxy-wide; cargo is present *iff* the convoy is within coverage; a
dark raider well outside coverage is absent from the payload (no leak), and every
visible rival raider is within coverage; browser-confirmed the coverage bubbles,
routes, cargo reveal, and the threat contact appearing as a raider enters range.

### What M5 delivers so far (sub-step 5a — the hub Exchange)

The economic spine of §9, tied to the raiding loop:

- **The hub Exchange** (`crates/sim/src/market.rs`): one shared market, a standing
  price per commodity that **walks with flow** (buys lift, sells depress) and
  **drifts** on a slow seeded random walk so there's always something to trade.
- **Instant execution, lagged price information.** A market order settles *now*
  at the true standing price (correlation is instant), but the **price ticker is
  light-delayed** from the hub (the server's `PriceHistory` sends each player the
  prices as of the light that has reached their command center). So you commit to
  the *true* price, not the stale number you read — verified: the ticker showed
  ≈10.00 while a buy filled at the drifted-true 10.42.
- **Orders carry intent + destination, spawning raidable convoys.** A **buy**
  settles instantly (credits debited) and spawns a delivery convoy **hub → home**
  (price-certain, delivery-risky). A **sell** commits the goods *first* and spawns
  a convoy **home → hub** that clears at the **price-on-arrival** (the §9 buy/sell
  asymmetry — double uncertainty). Both convoys are ordinary `Convoy`s, so they
  are **raidable in transit** (M4); a raided trade convoy's goods are simply lost.
- **Credits + inventory** on each corporation; a **market panel** client UI
  (prices, staleness, your wallet, Buy/Sell — press **M**) and an economy news log.
- *(Nice lightspeed detail: a buy's delivery convoy spawns at the hub, ~16s of
  light from home, so you don't even see your own inbound convoy until its light
  arrives.)*

**Protocol additions:** `ClientMsg::MarketBuy` / `MarketSell`; `View.market`
(lagged `PriceView`s + `staleness`) and `View.wallet` (`credits` + `inventory`);
`ServerMsg::Trade`. Sim: a `Market`, `Corporation.credits`/`inventory`, a
`TradeMission` on ships, and `TradeEvent`s.

**Verified** (`scripts/economy_smoke.mjs` + 3 sim trade tests): lagged ticker;
buy settles instantly and spawns a delivery convoy; sell commits goods to a
hub-bound convoy; delivery/sale resolve on arrival; browser-confirmed the market
panel, trade news, and convoys crossing raidable space.

**Sub-step 5b — limit orders + batch clearing.** Limit orders rest on a per-
commodity book (resources reserved at placement — credits for a buy, goods for a
sell). Every ~20 s a **periodic uniform-price call auction** clears each book:
all trades settle at one price, so reacting fastest confers no edge (the §9 anti-
sniping mechanism). A matched buy settles and spawns a delivery convoy (refunding
any over-reservation); a matched sell is paid; unmatched orders rest to the next
batch. Client: a limit toggle + price in the market panel and a resting-orders
list. Verified by `scripts/limit_smoke.mjs` + 2 sim tests (a crossing pair clears
at the uniform price; non-crossing orders rest).

**Sub-step 5c — equity valuations.** Each corporation's net worth (credits +
goods at market — held, in transit, and reserved in resting orders — plus
buy-order escrow) is recomputed on a **slow cadence** (≈ every 60 s) to keep it
readable, not noisy (§9), and shown in the market panel ("equity"). Verified the
figure ≈ credits + inventory value.

### What M6 delivers (verified) — robustness, persistence, scale

- **Restart restores the galaxy from the latest snapshot (§14).** Snapshots (full
  `World` JSON) are written off the hot path every ~10 s; on startup the server
  loads the most recent one and resumes from it (else generates a fresh galaxy).
  A reconnecting player resolves to the same corporation (the stable name hash),
  now restored with its credits, inventory, ships, resting orders, and market.
  Verified by `scripts/restart_smoke.sh`: a player buys fuel (credits 10000 →
  8023), the world snapshots, the server is **killed and restarted**, and the
  rejoining corp is restored at 8023. *(Restart transient: the per-player view
  history is rebuilt fresh, so the galaxy re-illuminates over ~one light-crossing
  as light propagates from the restored positions.)*
- **Scale to 12 players in one galaxy.** Galaxy radius scales with player count
  (§4); the single authoritative loop builds 12 distinct per-player delayed views
  and keeps up. Verified by `scripts/scale_smoke.mjs` (run with `MAX_PLAYERS=12`):
  12 distinct players each get a live ~10 Hz delayed view and `/status` reports
  12 online — the loop isn't falling behind.
- **Session robustness.** Corporations persist across disconnects and keep
  running on their standing orders (ships patrol, trade convoys continue);
  reconnecting with the same name resumes the corporation; half-open connections
  are reaped by the M1 keepalive + idle timeout.

M5 thus realises the §9 model: instant execution + lagged prices, market AND
limit orders with uniform-price batch clearing, order-spawned **raidable** trade
convoys, the buy/sell asymmetry, and slow valuations. *(Documented
simplifications: limit-order goods settle at the exchange rather than each
spawning a crossing; the sell-news is shown promptly rather than light-delayed;
home is treated as light-distance from the hub rather than a zero-lag coherence
peak — all consistent, additive-friendly choices noted for later refinement.)*

**Verified in-browser:** issuing an order shows the violet comet traveling from
the command center to the ship's ghost (paced by the server's observed delay); a
resolved raid shows gold rings arriving home and the verdict revealed on arrival.
Each player sees these from their own command center / observed frame (the comet
goes only to the issuing player; both signals are built from that player's
command center + ghosts/report).

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

Open the client in two or more browser tabs (or machines, pointing at the same
server). Enter a **different corporation name** in each — each becomes a distinct
player commanding from its own home anchor, with its own delayed view.
Reconnecting with the same name resumes that corporation (its ships, credits,
inventory, and resting orders persist). Size the galaxy for the player count with
`MAX_PLAYERS=12 cargo run -p server`.

## Playing the game

You command a chartered corporation from your **home anchor** — and you never see
the galaxy as it *is*, only as the light that has reached your chair (§6). Every
sighting shows where something *was*; every order crosses space at light speed.

- **Read your delayed map.** Your own ships are crisp cyan (a coherent feed, just
  late). Rivals are red **ghosts** at their last-known position, with an
  uncertainty cone (how far they could have moved since the light left) and a
  "Δ Ns" staleness label. Soft **teal bubbles** are your sensor coverage; outside
  them you're blind to raiders. Convoys broadcast galaxy-wide (with their route);
  cargo only shows for convoys inside your sensors. A pulsing red **⚠ RAIDER** is
  your only warning of an attacker that has entered range.
- **Command across the delay.** Click one of your ships to select it, then click
  empty space to **move** it — a violet comet shows your order crossing to the
  ship; then a return pulse + "RECEIVED · response light ~Ns" shows you waiting
  for the light of its maneuver to come home (the ghost only changes course when
  that light lands). The three clocks are always visible.
- **Raid.** Select a raider, click a **rival ghost** to commit an intercept — it
  pursues the rival's *true* position, not the stale ghost you saw. Press **R** to
  recall (it may arrive too late). When a raid resolves, gold **report rings**
  cross home and reveal the verdict on arrival — and the two players learn it on
  *different* clocks.
- **Trade (press M).** The **Hub Exchange** ticker is light-delayed, so you commit
  to the *true* price, not the stale one you read. **Buy** settles now and a
  delivery convoy crosses home (raidable). **Sell** ships goods to the hub first
  and clears at the price-on-arrival (riskier). Or place **limit orders** (tick
  "limit @", set a price) that rest and clear in a periodic uniform-price batch —
  no sniping edge. Your credits, holdings, equity, and resting orders are in the
  panel; credits + equity are also in the top HUD.

The core loop: **command from home through honest lightspeed delay, trade on the
shared Exchange, raid each other's convoys, and learn the outcomes as delayed
news on your own clock.**

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
cargo test                            # 33 unit tests: determinism, flip-and-burn
                                      # physics, the lightspeed fairness invariant,
                                      # command latency, raid resolution + recall,
                                      # delayed-report delivery, two-tier sensor model

# end-to-end checkpoint smoke tests (server must be running on :8080):
cargo run -p server &                 # in one shell
node scripts/m1_smoke.mjs             # M1: per-player streams, join/leave (+/status)
node scripts/m2_smoke.mjs             # M2: galaxy + flip-and-burn movement
node scripts/m3_smoke.mjs             # M3: per-player lightspeed views, no leaks (~35s)
node scripts/m4_smoke.mjs             # M4: raid → delayed reports on own clocks (~70s)
node scripts/sensor_smoke.mjs         # broadcast + sensor range: cargo gating, dark
                                      # raiders omitted out of coverage (~35s)
node scripts/economy_smoke.mjs        # M5: lagged ticker, instant buy + delivery
                                      # convoy, sell asymmetry (~25s)
node scripts/limit_smoke.mjs          # M5: limit orders + uniform-price batch (~25s)
node scripts/scale_smoke.mjs 12       # M6: 12 players, loop keeps up (run server with MAX_PLAYERS=12)
bash  scripts/restart_smoke.sh        # M6: kill + restart restores the galaxy (needs the dev DB)
```

The server also exposes `GET /status` (JSON: connection/session meta — kept off
the per-player game view so presence can't leak faster than light) and
`GET /healthz`.

---

## Layout

```
crates/sim/        pure deterministic simulation core (no I/O)
crates/server/     tokio + axum server: game loop, sessions, ws, persistence
  migrations/      sqlx Postgres migrations
client/            Pixi.js + Vite + TypeScript client
scripts/           devdb.sh (local Postgres), m1_smoke.mjs (checkpoint test)
```

## What's next (post-alpha, from the design)

The seven-milestone build is done. Beyond it, GAME_DESIGN sketches: **warp-lane
construction** (player-built public speed-up corridors via the mass-reduction
model, §10), the **conquest / home-assault endgame** and victory condition (§11),
and **depth** — research/tech, coherence as a contestable system, exploration,
the settlement-key economy, the movable forward command center (§6.1) — and only
then **balance** (via the bot simulator + human playtest).

## Notes / known stubs

- **Persistence stub:** without `DATABASE_URL` the event log/snapshots are
  dropped (logged, not stored). The Postgres path is real and verified, and a
  restart **restores the galaxy from the latest snapshot** (M6). The stub exists
  so the game runs without a database. *(Restart transient: the per-player view
  history rebuilds fresh, so the galaxy re-illuminates over ~one light-crossing.
  Command-replay between snapshots — full event-sourcing — is a refinement; the
  snapshot reload alone bounds restart loss to the ~10 s snapshot interval.)*
- **Delayed reports** (raid outcomes) are marked delivered when handed to the
  outbound queue. Reports are rare and the queue is almost never full, but M6
  should make delivery reliable (re-deliver until acknowledged).
- A **destroyed ship's ghost** lingers (frozen, ageing) in a viewer's delayed
  picture until its last light passes the history horizon — this is correct (you
  still see old light), and the delayed *report* tells you the truth; a tidier
  "last-seen, now gone" treatment is polish for later.
- **Balance is deliberately untuned** (per the design): ship speeds, galaxy size,
  `c`, and raid radii are first-pass values chosen for legible delays, not
  balance.
