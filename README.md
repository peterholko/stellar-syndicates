# Stellar Syndicates

An asynchronous, multiplayer (4–12 player), continuous-time 4X space strategy game about
corporate trade and conflict across a wormhole-linked galaxy.

Its defining mechanic is **lightspeed-delayed observation and command**: you never see the
galaxy as it is *now*, only as the light that has reached your command center — and your
orders cross space at the speed of light, arriving late. You are a remote commander reading
reports from the dark, not a god moving pieces on a board.

[`GAME_DESIGN.md`](GAME_DESIGN.md) is the design document, kept in sync with this code.

---

## Quick start

Prerequisites: Rust (stable, built with 1.91), Node 18+ (built with Node 24), and
optionally PostgreSQL 16 for durable persistence.

One command builds the client, builds a release server, resets the galaxy, and waits for
health:

```bash
scripts/start.sh
```

Then open <http://localhost:8080> and enter a corporation name. Pass `--keep-galaxy` to
resume the existing snapshot instead of starting fresh.

**§jump-v1 requires a fresh galaxy.** Snapshots created by the retired hyperspace-lane/relay
build are not compatible with the jump-drive world model. Let `scripts/start.sh` reset the
galaxy once; snapshots produced by this build can then be resumed normally.

**Manually**, if you prefer:

```bash
cd client && npm install && npm run build && cd ..
cargo run --release -p server
```

Use `--release` for anything you intend to play. The debug build is several times slower on
the per-tick serialization path and visibly stutters.

**Client hot reload** during UI work — Vite on :5173, talking to the server on :8080:

```bash
cd client && npm run dev
```

Point the client at a different server with `?server=ws://host:port/ws`.

**Multiple players:** open the client in two or more tabs (or machines pointed at the same
server) and enter a *different* corporation name in each. Each becomes a distinct player
commanding from its own home system with its own delayed view. Reconnecting with the same
name resumes that corporation — ships, credits, warehouse, holdings, and resting orders all
persist. Size the galaxy for the player count:

```bash
MAX_PLAYERS=12 cargo run --release -p server
```

### Environment

| Variable | Default | Effect |
|---|---|---|
| `PORT` | 8080 | HTTP + WebSocket listen port |
| `GALAXY_SEED` | `0xC0FFEE` | deterministic generation seed |
| `MAX_PLAYERS` | 4 | sizes the galaxy (radius scales as `4000 × √players`) |
| `HOME_RING_SU` | 80,000 su nominal outer ring | optional absolute home-ring radius override; useful for replaying archived layouts |
| `SIM_PACING` | 4 | sim seconds per wall second for compressed balance playtests; set to `1` for standard/production pacing |
| `DATABASE_URL` | unset | Postgres DSN; unset means an in-memory no-op stub |
| `SNAPSHOT_EVERY_TICKS` | pacing-adjusted | full-world snapshot cadence; approximately 10 wall seconds (1,200 ticks at 4×, 300 at 1×) |
| `RUST_LOG` | — | e.g. `info` |

Endpoints: `/ws` (the game), `/healthz`, and `/status` (connection and session meta, kept
off the per-player game view so presence cannot leak faster than light).

### Optional: durable persistence

A throwaway, isolated dev cluster that does **not** touch your system Postgres:

```bash
scripts/devdb.sh init
```

Then `export DATABASE_URL="$(scripts/devdb.sh url)"` and start the server; it writes events
and snapshots, and a restart restores the galaxy from the latest snapshot. `scripts/devdb.sh
stop` or `nuke` when you're done.

---

## Playing it

You command a chartered corporation from a home star system you own outright. Every sighting
shows you where something *was*; every order crosses space at *c*.

**Read your delayed map.** Fleets are ghosts — cyan for yours, red for rivals — and each
declares how old its last-arrived light is. Potential uncertainty is `age × speed` and applies
to *your own* fleets too: there is no FTL tether. Sensor coverage is deliberately not drawn as
a certainty ring because speed and fleet signature change its effective reach; outside served
coverage you are blind to Raiders and Scouts, which run dark. Civilian and capital fleets
broadcast galaxy-wide, but their composition and cargo sharpen only inside your sensors.

**Command across the delay.** Select a fleet, click empty space to move it. A violet comet
shows your order crossing to the fleet; the panel and the map then track it through
**in transit → awaiting echo → confirmed**, and the ghost only changes course when the light
of its manoeuvre gets home.

**Fight.** Raid a rival convoy with a raider to steal its cargo, or attack a fleet to destroy
it. Both pursue the target's *true* position, not the stale ghost you clicked. Committing
opens a projected-outcome estimate — Monte Carlo over the real combat engine, run on your own
view data, showing a win rate, loss bands, and the age of every input. Battles take real time,
anchor both sides in place, and can be reinforced or withdrawn from mid-fight (also
light-delayed). Outcomes arrive as delayed reports on each side's own clock, and finished
battles are replayable in the Battle Theater.

**Build an economy.** Claim systems by building a colony ship and sending it — claiming is
physical, telegraphed, and raidable. Site structures on individual planets and moons, staff
their production lines with workforce and specialists, feed the population, and chain raw
extraction into processed and advanced goods.

**Trade.** The Global Market settles instantly against your warehouse at the hub, but
its price ticker is light-delayed, so you commit to the true price rather than the one you
read. Hauling is a separate, explicit act on the **Warehouse** tab, where you pick the
carrier for each lot: book Authority freight (a fee, a fixed timetable, someone else's hull,
and it runs both ways) or send one of your own convoys (free and immediate, outbound only,
and yours to escort and lose).

**Automate.** Standing logistics orders and corp-wide fleet doctrine run on the server clock
whether or not you are connected. That is the point: presence buys awareness, not advantage.

### Interface

The map is the master view; panels dock beside it. `Esc` backs out one level.

| Key | Opens |
|---|---|
| `S` | System — the star-system workspace (geology, production, build) |
| `M` | Market — Exchange · Warehouse · Specialists · Modules |
| `O` | Logistics — standing orders |
| `F` | Fleet doctrine |
| `G` | Rankings |
| `R` | Research boards (or **recall**, when one of your raiders is selected) |
| `Y` | Syndicate |
| `C` | Charter — your standing with the Authority |
| `L` | Check-in log — what became observable while you were away |

Mouse wheel zooms toward the cursor, left-drag pans, `+`/`−` and arrows do the same from the
keyboard. Clicking a system enters its detail view; clicking a body opens the planet panel.

---

## What's implemented

The game is playable end to end by several people at once. Everything below is live, wired to
the client, and covered by tests; §20 of the design document lists what is designed but *not*
built, and §21 the open questions.

| Area | State |
|---|---|
| **The lightspeed model** | Every novel report and directed order travels on one straight 2,000 su/s warp-light chord to or from each player's command center. Per-player arrival-ordered views enforce the fairness guarantee; order confirmation can fire only from compliance light already served on the map. No lane, relay, or wire channel remains. |
| **Detection** | Speed-signature visibility: `distance ≤ sensor_capability × signature`, from a shared function used by both the view filter and the sim's own sensing. Size aggregates as √signal; flank speed lights you up; a Full/Stealth transit throttle is the player's lever. |
| **Intel** | Count-class buckets always, exact composition and cargo only inside sensor coverage, scout snapshots of rival fortifications delivered at *c* and then aging. |
| **Movement** | Straight thruster/warp travel with constant per-kind speeds, analytic interception, and formation speed set by the slowest member. Raider/Scout-only fleets also have a 50,000 su jump: 10 s stationary spool, gravity-well lockout, warp-equivalent fuel cost, instantaneous relocation, and a light-delayed served snap. |
| **Standing upkeep** | Every fleet eats Provisions every second, wherever it is, online or off — the ceiling on force. Charged on crew rather than tonnage, paid from the owner's nearest stocked system. A shortfall **immobilizes** a fleet and never destroys it: it keeps its guns and its course, and moves again the moment it is fed. |
| **Fleets** | A fleet is a **roster of individual hulls** — each with its own id, fit, and remaining hull. Build/join, merge, split at owned systems; twelve hull kinds including a research-gated Destroyer→Titan capital ladder. |
| **Officers** | A home-Academy Captain roster with physical assignment/reserve duty, light-delayed remote progression, XP and eight service ranks, weighted command capacity, four bounded specialties, and delayed rescue/injury/capture/death outcomes after fleet loss. |
| **Persistent damage** | Battle damage lives on the individual ship, survives the battle, the snapshot, and any merge or split, and never heals on its own. Nothing is pooled or apportioned; a hull held in reserve takes none. Repaired at an Ordnance Foundry on the standard factor chain — an unsupplied yard mends less, never destroys. |
| **Combat** | An individual-ship tactical engine — positioned combatants, range bands, live torpedoes, five published role scripts, per-battle isolated seeded RNG. Modules with a one-to-one counter matrix and fitting-point budgets. Battle records, a Pixi replay theater, and a Monte Carlo pre-commit estimator that samples the real engine. |
| **Standing defense** | Corvette screens, defense platforms, autonomous pickets, and per-fleet postures — all running with the owner offline. |
| **Economy** | Twelve commodities in three rungs, twenty structures across three derived slot pools, per-body siting, population and a four-rung food ladder, workforce and five specialist professions, storage caps that idle production rather than voiding it. |
| **Shipbuilding** | A yard ladder — Shipyard (light hulls) → Naval Drydock (the line of battle) → Capital Slipway (super-capitals), each needing the one below it on the same system, plus an Ordnance Foundry for refits. A yard's tier is its slipway count, so throughput is a real decision. |
| **Market & the Authority** | Instant warehouse-settled trades, a light-delayed ticker, uniform-price limit batches, scheduled raidable freight, and the charter law: standing, citations that travel at *c*, five escalating bands, and scripted enforcement expeditions. |
| **Territory** | Physical claiming by colony ship, blockade, a siege clock, and capture of non-home systems. Homes can be blockaded but never taken — there is no elimination and no victory condition; rankings stand in. |
| **Ground war** | Settling and conquering are different acts with different hulls: a colony ship takes *unclaimed* ground, held ground takes **marines**. A landing is *fought* — seeded rounds on their own clock, with suppression re-read every tick, so relief that breaks a blockade can turn an invasion already on the ground. A pre-commit estimate sampled from the real engine prices the gamble first, including what happens if the guns leave. |
| **Ground theater** | Every landing leaves a replayable record: both sides' strength round by round, the fraction of the garrison pinned at each moment, and derived beats (guns lifted, lead changed hands). Participants read exact troop counts; an observer with sensor coverage sees the fight's shape without its arithmetic. Rounds arrive on their own light. |
| **Plunder** | A held blockade strips the stockpile it strangles, into the blockader's own hold — so the loot still has to survive the trip home. Bounded by rate, hold room, and a per-commodity reserve the besieger can never strip below: a colony always survives and recovers. |
| **Research** | Six corporation-owned programme boards with twelve schools, 111 programmes, verb gates, and a goods-funded clock distributed across your Academies. Joining or leaving a Syndicate never grants, pauses, or removes research. |
| **Syndicates** | Capped alliances with Founder/Officer/Quartermaster/Member permissions, shared operations, doctrine fits and one flagship; membership propagates at light speed. |
| **Operations** | One light-honest contract engine for bounties, surveys, deliveries, salvage, escorts, enforcement, strategic control, regional competitions and staged syndicate projects. |
| **Diplomacy** | Non-aggression pacts, ceasefires and declared wars with light-travel notices, activation grace, post-syndicate separation and defender-only reprisal. |
| **Neutrals** | Pirate enclaves that escalate if ignored (with an onboarding grace window per corporation) and the Authority's freighters and enforcement squadrons. |
| **Async loop** | Standing logistics orders, corp doctrine, offline accrual, and a reconnect digest of what became observable while you were away. |
| **Ops** | 12 players in one galaxy with the loop keeping up; snapshot persistence with restart recovery; reconnect resumes a corporation. |

**Balance is deliberately untuned.** Every constant is a first-pass playtest value, grouped
into `Tunable` blocks per subsystem so one edit re-paces a whole layer. Battle duration in
particular is *emergent* from the step cadence and the to-hit calibration — it should not be
forced to a target by retuning that calibration constant.

---

## Architecture

```
            ┌───────────────────────────────────────────────────────┐
            │  server (Tokio)                                       │
  client ───┤  ┌────────────┐   intents    ┌──────────────────────┐ │
  (Pixi) ◄──┤  │ ws conn    │ ───────────► │ game loop — single   │ │
    WS      │  │ (axum)     │ ◄─────────── │ owner of World +     │ │
            │  └────────────┘  per-player  │ Sessions; 30 Hz tick │ │
            │        ▲          streams    └──────────┬───────────┘ │
            │        │                                │ events,     │
            │        │                        ┌───────▼──────────┐  │
            │        │                        │ persistence task │  │
            │        │                        │ (sqlx → Postgres │  │
            │        │                        │  or no-op stub)  │  │
            │        │                        └──────────────────┘  │
            └────────┼──────────────────────────────────────────────┘
                     │ uses (pure, no I/O)
             ┌───────▼───────┐
             │   sim crate   │  World + step(commands) → events
             └───────────────┘
```

- **`crates/sim` is pure.** No I/O, no async, no networking, no database. `World::step` takes
  commands and returns the next state plus events; determinism comes from a seeded RNG and a
  fixed 30 Hz timestep. The Academy-to-first-colony balance harness runs that same engine and
  emits milestone CSV across seeds and opening strategies:
  `cargo run --release -p sim --example opening_balance -- --seeds 100`.
- **One Tokio task owns the world** and the session registry, so there are no locks and no
  data races on game state — by construction, not by discipline.
- **axum + WebSockets are pure I/O.** Connections take intents and push filtered state; they
  never touch game state.
- **Persistence is off the hot path** and never blocks the tick loop.
- **The per-player view filter is a first-class component**, not a detail: it holds each event
  until its light-travel time to that player's command center has elapsed and filters by what
  that player's assets can observe. It is the code embodiment of the whole information model.

The wire protocol is versioned (currently **16**) and announced at join. Static tables ship
once in the welcome message, slow-moving per-player sections are signature-gated and re-sent
only on change, and battle records stream incrementally — the map stays smooth at 10 Hz
updates.

### Layout

```
crates/sim/          pure deterministic simulation core (no I/O)
crates/server/       tokio + axum: game loop, per-player view filter, sessions,
  migrations/        ws, reports, timeline, persistence · sqlx migrations
client/src/          Pixi + TypeScript: map renderer, panels, battle theater
client/public/art/   the custom art set (sprites, UI icons, lore scenes)
scripts/             start.sh, devdb.sh, and the wire-level smoke tests
docs/                design handoffs for individual subsystems
```

---

## Tests

```bash
cargo test
```

**560 tests, all passing:** 478 sim unit tests, 3 determinism integration tests (same seed
byte-for-byte, mid-flight snapshot round-trip, pre-feature snapshot load), and 79 server
tests. The client is type- and build-checked with `npm --prefix client run build`
(`tsc --noEmit && vite build`).

The heaviest coverage sits where the design is load-bearing: fog leak checks in both
directions for every owner-only field, the lightspeed fairness guarantee, determinism from
seed, serde round-trips for every persisted structure, and the economy's price/recipe
invariants (every processed good clears its input basket; no converter basket net-adds units).

### Wire-level smoke tests

These drive a *running* server over the real WebSocket. Start one on `:8080` first.

| Script | Checks |
|---|---|
| `m1_smoke.mjs` | per-player streams, join/leave, `/status` |
| `m3_smoke.mjs` | per-player lightspeed views, no cross-player leaks (~35 s) |
| `m4_smoke.mjs` | raid → delayed reports on each side's own clock (~70 s) |
| `sensor_smoke.mjs` | broadcast vs dark, cargo gating outside coverage (~35 s) |
| `own_fog_check.mjs` | your own distant ship carries honest uncertainty, not zero |
| `battle_smoke.mjs`, `battle_rvr_smoke.mjs` | destruction obeys the lightspeed law per viewer |
| `patrol_defense_smoke.mjs` | autonomous defense runs with the owner disconnected |
| `limit_smoke.mjs` | limit orders rest, reserve, and clear at one uniform price |
| `scale_smoke.mjs 12` | 12 players, loop keeps up (run the server with `MAX_PLAYERS=12`) |
| `restart_smoke.sh` | kill and restart restores the galaxy (needs the dev DB) |

Plus `hold_client.mjs`, `hold_defender.mjs`, `aggro.mjs`, and `raid_convoy.mjs`, which park or
drive a second player for manual and visual testing.

**Three scripts have drifted from the code and will fail as written:**

- `claims_smoke.mjs` sends `ClaimSystem`, a command that no longer exists — claiming is
  physical now (send a colony ship). The production and geology-gradient assertions in it are
  still valid ideas worth re-testing through the new path.
- `economy_smoke.mjs` expects a market buy to spawn a delivery convoy. Buys deposit into the
  Market Warehouse now and move nothing; its ticker-staleness and instant-settlement
  assertions still hold.
- `m2_smoke.mjs` describes flip-and-burn movement, which was removed for constant per-kind
  speeds. It still verifies that the galaxy generates and fleets move.

---

## Known gaps and rough edges

- **Persistence stub.** Without `DATABASE_URL` the event log and snapshots are dropped
  (logged, not stored) so the game runs with no database. The Postgres path is real: a restart
  reloads the latest snapshot, bounding loss to the snapshot interval. Command replay between
  snapshots — full event sourcing — is not implemented. After a restart the per-player view
  history rebuilds fresh, so the galaxy re-illuminates over roughly one light-crossing.
- **Delayed reports are fire-and-forget.** A raid outcome is marked delivered when handed to
  the outbound queue. Reports are rare and the queue is almost never full, but reliable
  delivery (re-deliver until acknowledged) is not built.
- **Destroyed ships' ghosts linger** — frozen and ageing — in a viewer's picture until their
  last light passes the history horizon. This is correct (you still see old light) and the
  delayed report tells the truth, but a tidier "last seen, now gone" treatment is still owed.
- **Ten research programmes are live placeholders**: on the boards, researchable, described,
  and currently inert, pending the content passes that give them something to unlock (the
  utility modules, the Lance Array, a convoy refit variant, a few view features).
- **Stale comments in code.** `ship.rs` still opens by describing flip-and-burn acceleration,
  and `explore.rs` quotes an older base-price table. Both are comments only; the tables and
  the movement code are correct.
- **Six corvettes per enforcement expedition** was tuned before the capital ladder existed and
  is now a much softer obstacle than intended. The mechanic is intact; the number wants a pass.
