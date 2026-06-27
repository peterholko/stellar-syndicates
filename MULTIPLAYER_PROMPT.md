# Multiplayer Build Plan — Claude Code Handoff

Build the game described in `GAME_DESIGN.md`. **Target: the full per-GDD multiplayer game — up to
12 players, robust sessions, the complete multiplayer economy.** Read `GAME_DESIGN.md` in full,
especially §6 (the lightspeed information model — the heart), §9 (the market), §14 (architecture),
and §16 (milestones). Build in a fresh project.

## Read this first — how to approach a build this size

This is a LARGE target — well beyond a single session. Do not attempt to build all of it at once;
that produces a broken mess. Instead:

- **Architect for the FULL target from line one.** The server is built for up to 12 concurrent players,
  per-player information horizons, and the full economy FROM THE START — never single-player-then-
  retrofit. Multiplayer and the per-player view filter are foundational, not features added later.
- **Then build in the NUMBERED MILESTONES below, IN ORDER.** Each milestone ends in a verifiable,
  runnable, COMMITTED checkpoint. Complete and commit each before starting the next.
- **If you run out of time/context, STOP AT A COMPLETED MILESTONE.** Never leave a half-finished one.
  A smaller number of fully-working, committed milestones is far more valuable than all of them broken.
  You will likely NOT finish all milestones in one run — that is expected and fine. Land on solid ground.
- **Leave a STATUS note** (in the README and your final message): which milestones are complete, what is
  stubbed, how to run it, what's next, and anything that looks off.

## Non-negotiables (from GAME_DESIGN.md)

- **The lightspeed information model is the heart (§6).** Each player sees only a delayed, fogged
  reconstruction of the galaxy from THEIR command center; commands cross space at light-speed. With
  multiple players this means N simultaneous, DIFFERENT delayed views — and a hard fairness guarantee:
  **a player must never receive information their light hasn't reached yet, even though the server knows
  the truth and is serving every other player a different slice of it simultaneously.** No information
  leaks between players' horizons. This is the novel, risky core — build it carefully and make it
  deterministically testable ("player X could not have known Y at time T").
- **Keep the simulation core PURE (§14):** no I/O, no async, no DB, no networking inside it. It takes
  world state + commands and returns next state + events. Determinism via seeded RNG. Networking, DB,
  sessions, and rendering live OUTSIDE the core. This is what makes a game this size testable.
- **Pillar 2 — disclosure (§6):** wherever delayed information drives a decision, the UI must SHOW how
  stale it is. Never hide the lag.
- Verify dependency versions at build time (`cargo add`, npm) — current axum, tokio, sqlx, Pixi.js. Do
  not trust versions from memory.

## Tech stack (§14)
- **Server:** Rust. Pure deterministic simulation core + a single authoritative game-loop task owning
  the world in memory + axum WebSockets for I/O + async Postgres (sqlx) for snapshot + event-log
  persistence OFF the hot path. Built for up to 12 concurrent players.
- **Client:** Pixi.js (browser). Renders the per-player delayed/fogged view; holds NO authoritative
  state and runs NO game logic — sends player intents, renders the filtered stream the server pushes.

---

## MILESTONE 1 — Multiplayer architecture scaffold + sessions
Goal: the real architecture skeleton, built for many players, end to end.
- Rust server: a pure `sim` crate/module (no tokio/axum/sqlx deps) + a `server` binary running tokio +
  axum (HTTP + WebSocket).
- **Multiplayer session layer from the start:** multiple concurrent WebSocket connections, each mapped
  to a player identity; join and leave handling; a registry of connected players. Keep auth minimal for
  now (a name or simple token is fine — robust accounts come later) but build the SESSION plumbing
  properly (per-connection player mapping, disconnect handling, per-player outbound stream).
- A tokio game-loop task ticking at a fixed rate (e.g. 30 Hz), owning the world, broadcasting a
  PER-PLAYER message to each connected client (for now, just a tick + their player id).
- Postgres via sqlx: migrations folder, connection, a trivial table. If the DB environment blocks you,
  stub persistence behind a trait and continue (note it).
- Pixi.js client: connects, identifies as a player, renders a blank canvas + its player id + live tick.
- **Checkpoint:** TWO+ clients can connect simultaneously, each gets its own per-player stream and a
  live tick from the authoritative loop; joins/leaves are handled. COMMIT.

## MILESTONE 2 — True-world simulation: continuous space + acceleration (§7)
Goal: the authoritative multi-entity world.
- In the pure `sim` core: 2D continuous space, a central wormhole **hub**, a **home anchor per player**
  (placed around the galaxy), and randomly-placed **star systems** (seeded, deterministic).
- Ships with **position + velocity** and **acceleration-based flip-and-burn** movement (§7): accelerate
  to max speed, flip, decelerate to arrive at rest. **Convoy** (slow, heavy) and **raider** (fast,
  light) types with different accel/max-speed.
- Game-loop advances the true world each tick via the pure core. (Temporarily render TRUE positions to
  verify movement before the delay layer lands in M3.)
- Client renders the map: hub, all home anchors, systems, ships.
- **Checkpoint:** ships move with flip-and-burn in the shared world; multiple connected clients see the
  same world advancing. COMMIT.

## MILESTONE 3 — The lightspeed information model, MULTI-PLAYER (THE CORE — §6)
Goal: the heart of the game, at multiplayer scale. This is the hardest and most important milestone.
- Each player has a **command center** (starts at their home anchor) = their vantage. Per object, per
  player: delay = distance_from_THAT_player's_command_center / c.
- **Per-player view filter (a first-class server component, §14):** maintain each moving object's recent
  true-position history; for EACH player, produce a delayed/fogged reconstruction — every object shown
  as of (now − its delay from THAT player). Each player receives ONLY their own filtered view; never
  true positions; never another player's view. Enforce the fairness guarantee: no player gets info
  their light hasn't reached.
- Client renders **delayed ghosts** with staleness VISIBLE (§6, Pillar 2): "seen Xs ago", fading/aging
  with delay, uncertainty growing with staleness. Each player's client shows a DIFFERENT delayed
  picture.
- **Command latency:** a player's order reaches a ship after its light-travel time; the player learns
  the result later still (the three clocks: send / arrive / observe).
- Make it deterministically testable: a way to assert "player X's view at time T contains only what
  light permits."
- **Checkpoint:** two players in the same world each see their OWN coherent delayed/fogged view of each
  other and of all ships; distant things are stale; commands lag; no information leaks between players.
  COMMIT. (This is the milestone that proves the game's central bet — two remote commanders clashing
  through the dark, each blind to the other's truth.)

## MILESTONE 4 — The raiding loop, player-vs-player (§8)
Goal: the core conflict, between real players.
- A player commits a **raider** to intercept another player's **convoy** (intercept-commit, §8): the
  raider pursues autonomously; no real-time steering. Projected intercept shown from the committing
  player's delayed view (can be wrong).
- Resolution in true space (intercept on contact, or convoy reaches safety); each player learns the
  outcome only when its light reaches THEIR command center (so attacker and defender may learn it at
  different times). A delayed **report** delivers the result.
- **Recall** order mid-chase travels at light-speed and may arrive too late ("commanding into the past").
- **Checkpoint:** player A can raid player B's convoy under honest delay; both learn the outcome as
  delayed news on their own clocks; recall can miss the window. COMMIT.

## MILESTONE 5 — The full multiplayer economy (§9)
Goal: the complete market — this is a major milestone; build it in sub-steps and commit between them.
- **The hub Exchange** shared by all players. **Instant execution** (settlement) with **lagged price
  information** (§9): players away from the hub see stale prices; execution is immediate.
- **Market orders** (instant at standing price) AND **limit orders** that rest and clear in a **periodic
  batch / uniform-price call auction** (the anti-sniping mechanism, §9).
- **Orders carry intent + destination** (§9): buys spawn delivery convoys (hub → player); sells spawn
  convoys (player → hub, sell on arrival). These convoys are raidable — tying the economy to the raiding
  loop.
- The **buy/sell asymmetry** (§9): sellers clear at price-on-arrival, not a locked launch price.
- **Equity / corporate valuations** (§9) updating on a slow cadence (avoid share-price noise).
- A real resource/credit economy so players have goals and pressure on each other.
- **Checkpoint(s):** players trade at the shared hub (market + limit orders, batch clearing), goods
  cross raidable space, valuations update; the economy is the full §9 model. COMMIT after each sub-step.

## MILESTONE 6 — Robust sessions, persistence, scale to 12 players
Goal: make it a robust multiplayer game at full player count.
- Harden the session layer: robust join/leave/reconnect; what happens to a disconnected player's
  corporation (keeps running on standing orders); up to **12 concurrent players** in a galaxy (galaxy
  size scales with player count, §4/§16).
- **Persistence (§14):** snapshot + event-log off the hot path; restart reloads the game. Event-sourced
  so the game state survives and is auditable.
- Per-player view filters performing acceptably at 12 players (snapshotting / hot in-memory world as
  needed, §14).
- **Checkpoint:** up to 12 players can join a persistent galaxy, each with their own delayed view, trade
  and raid each other, survive a server restart. COMMIT.

## MILESTONE 7 — Client polish to a testable multiplayer game
Goal: legible, runnable, judgeable.
- Clear client UI: the player's delayed map, their credits/goods/holdings, the market (place market &
  limit orders), commit/recall raiders, staleness/lag indicators everywhere delayed info drives a
  decision.
- README: how to run server, client(s), Postgres; how multiple players join; what they can do.
- **Checkpoint:** multiple people can join one galaxy and play the core loop — command from their home
  anchor through honest lightspeed delay, trade on the shared Exchange, raid each other's convoys, learn
  outcomes as delayed news. COMMIT.

---

## Working rules (critical for a build this large)
- COMMIT after every completed milestone (and after major sub-steps within M5) with a clear message.
  Partial overnight progress must always sit at a clean, known, committed checkpoint.
- Architect for the full target (12 players, full economy) from the start so nothing is retrofitted —
  but BUILD in milestone order and do not skip ahead.
- If something blocks (commonly Postgres environment), STUB it behind a clean interface, note it, and
  continue — EXCEPT the core (M2 movement, M3 lightspeed model, M4 raiding) which must work.
- Keep the `sim` core pure and deterministic — mandated by §14, and the only way a game this size stays
  testable.
- Do not cut corners on the M3 fairness guarantee (no information leaks between players) — it is the
  integrity of the entire design.
- You will likely not reach M7 in one run. That is expected. Get as far as you can with each milestone
  FULLY working and committed, and leave a clear STATUS note on where things stand and what's next.

Prioritize a SMALLER number of FULLY WORKING, COMMITTED milestones over all milestones half-done. Land
on solid ground.
