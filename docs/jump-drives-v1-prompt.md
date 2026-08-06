
# §jump-v1 — Remove the hyperspace layer, fly by jump drive

Playtest verdict: hyperspace is coming OUT — the lane network, the buoys and repeaters, and every
system that existed only to serve them. **Remove the code; do not park it, flag it, or leave it
dormant.** In its place, dark ships get **jump drives**: a fleet holds still, spools for 10
seconds, then relocates instantly to a chosen point within 50,000 su. The information model
reverts to its plain form: every event radiates in a straight line at warp light (`c` 400 ×
`WARP_FACTOR` 5 = 2,000 su/s) to each command center, and nobody — the owner included — learns a
jump happened until light from it reaches them.

Baseline: commit `18a1a20` on branch `hyperspace-lanes` (the lane-era archive — the point of the
branch is that this commit preserves the removed work). Confirm with `git status`; build on that
HEAD and commit on the current branch.

Read first: `GAME_DESIGN.md` §6 (information model) and §7 (movement), and the legend of
`docs/comms-event-audit-v3.5.md` — its rule still governs everything here: *private truth is
still a leak if a remote change reaches the command center before its physical report.*

## Non-negotiables

- **Removal means removal.** Delete every type, branch, message field, UI surface, and test whose
  only caller was the lane/relay world. Method: delete the core types first (`LaneNetwork`,
  `CommSite`, the three lane `EmplacementKind`s, `Regime::Hyperspace`, `Fleet.route`), then let
  `cargo check` / `tsc` enumerate every dependent site — at each, delete the branch, never stub
  it. Finish with grep sweeps (`lane|hyperspace|buoy|repeater|relay|bubble|tunnel|coupled|
  reentry|beyond_comms|stirs`) across sim/server/client until only deliberate survivors remain.
- **What deliberately survives** (do not over-delete): the frontier-cursor DARK serving path with
  its out-of-order **arrival heap** (jumps become the reason it exists — a hull teleporting
  closer reorders arrivals); `confirm_orders_from_served` — the one confirmation clock
  (§comms-v3.7); the mortal-order UI (× rows + dismiss — jumps become its only producer); the
  analytic order-echo estimate clock; `DriveState` + warp spool/drop; gravity wells +
  `HYPERLIMIT` (the name is now literally apt: it is the jump limit); `DeepSpaceSensor` and all
  detection; `relay_factor` IF it turns out to be the exotic-node effect rather than
  buoy-derived — verify at world.rs:10849 and keep only if node-based.
- The sim core stays pure and deterministic; `audit_determinism.rs` green after every commit
  (update its stripped-field list if it names removed fields).
- **Pre-pivot snapshots are incompatible** (removed `EmplacementKind` variants can't
  deserialize): fresh galaxies only; say so in the README. Keep generated CHARTS identical for a
  given seed: replace the buoy-throw derivation in `home_ring_frac` (config.rs:133) with a
  literal `80_000.0 / (1.0 + HOME_SLOT_RADIAL_JITTER_FRAC)` and move `GALAXY_SCALE` into
  config.rs — same numbers, no lane dependency.
- The fairness guarantee is absolute: a jump makes two light wavefronts from two places; each is
  priced independently; no surface may interpolate, extrapolate, or toast across the gap.
- No new undelayed information channels. Spool progress is never served as truth.
- Constants are playtest tunables: `JUMP_SPOOL_S = 10.0`, `JUMP_RANGE = 50_000.0`.

## Locked design decisions

1. **Only Raider and Scout fleets jump** — exactly the `!broadcasts()` dark kinds (ship.rs:190).
   Convoys, Authority Freighters, capitals, civilians: warp only.
2. **Both endpoints clear of gravity wells**: spool position AND target ≥ `HYPERLIMIT` (900 su)
   from every system (and the Market Hub — see gotchas).
3. **Spool = 10 s stationary**, drives dropped. An engagement joining the fleet **aborts** the
   spool (the Survey abort precedent, world.rs:4224-4228; the battle is the news). A later order
   delivered mid-spool **replaces** the jump — that IS the cancel path (Recall/R unchanged).
4. **Range validates at the ship at execution, from current position** — never at issue: an
   instant range verdict computed from remote truth would leak. The client warns from the served
   ghost; the sim decides at the hull.
5. **Fuel prices a jump like warping the distance**: `fuel_cost(d, mass) / JUMP_FUEL_FACTOR`,
   with `JUMP_FUEL_FACTOR = WARP_FACTOR`. (Raw `fuel_cost` of a 50k jump is `0.05·mass` — 143%
   of the `0.035·mass` tank — jumps would be silently impossible.) Debit at fire,
   pay-before-you-move; a dry fleet latches `stalled`, KEEPS the order, fires the tick it's fed.
6. **In-flight orders aimed at a hull that jumps are physically lost** — the signal arrives where
   the ship isn't. Reuse the mortal-order machinery, disclosed on the standard news clock.
7. **Jumps are detection-silent this pass** (signature is speed-derived; Raider/Scout run dark
   anyway). Accepted gap — record in GDD §21 with "jump flash" as the designed follow-up.
8. **Confirmation is served map light only**: the jump order confirms when landing light reaches
   the CC (`confirm_orders_from_served`). With relays gone this is uniform — there is no wire,
   no re-entry, no beyond-comms distinction anymore.

## COMMIT 1 — `§jump-v1: remove the hyperspace layer`

One coherent excision: geometry, wire, and bubbles together. Post-state: straight warp flights,
straight warp light, DARK-path serving for everything, analytic echo estimates, `DeepSpaceSensor`
the only emplacement. Work compile-error-outward from these deletions:

**Sim — travel geometry:**
- `World.lanes` (world.rs:735) + the `lane::generate` call (world.rs:1080-1087) + the
  `rebuild_junctions` call in `fixup_after_load`.
- `Fleet.route` (ship.rs:996) and `lane::Leg` — flight is straight at the order's dest:
  delete route planning at delivery (world.rs:4684-4706); `Fleet::advance` steers by
  `order` dest alone (the regime want at ship.rs:1634 loses its lane tag source).
- `Regime::Hyperspace` (→ two variants), `LANE_MULT`, `LANE_SPOOL_S/DROP_S`, alignment gating,
  `speed_factor`/`hits`/`on_lane`, `stirs_the_lane()` (ship.rs:889), the dark-arrival drive
  doctrine (world.rs:11424).
- **Delete `crates/sim/src/lane.rs` entirely.** Rehome survivors into a new small
  `crates/sim/src/transit.rs`: `Regime { Thrusters, Warp }`, `WARP_FACTOR`,
  `WARP_SPOOL_S`/`WARP_DROP_S` + `spool_seconds`/`drop_seconds`, `HYPERLIMIT`,
  `TransitEnv { wells }` + `factor()`, and a straight-light signal module:
  `signal_speed(c) = c * WARP_FACTOR`, `delay(a, b)`, and `command_meeting_delay` reduced to its
  `pos + vel·delay` fixed-point (8 iterations; the `advance_route`/`advance_lane` extrapolants
  die). `GALAXY_SCALE` → config.rs (build.rs:563, tca.rs:53 follow).

**Sim — comm wire and bubbles:**
- `EmplacementKind`: remove `HyperspaceBuoy`, `HyperspaceRepeater`, `HyperspaceSensor` (the lane
  listener dies with its lane, emplace.rs:32-38) → `DeepSpaceSensor` only. With them:
  `needs_a_lane`, `throw()`, `SiteError::NotOnALane` (site_check keeps `MIN_SPACING` only),
  `LANE_LISTEN_RANGE`, their build recipes/menus, `grant_starter_buoy` (world.rs:11329) and the
  starter-grant call, demolition targeting stays (sensors are still wreckable).
- `CommSite`, `in_comm_bubble`, `comm_bubble_crossing`, `time_to_comm_bubble`, `relay_network`/
  `relay_network_known` (world.rs:6187-6211), comm-death machinery (`CommDeath`,
  `scheduled_signal_survives_loss`, `apply_comm_death_to_orders/echoes`, world.rs:187-294) — all
  producer-less; delete. `PendingOrder.signal` hops → one straight leg (or drop the hops list and
  keep depart/arrive times; `ServerMsg::CommandSignal` keeps the comet with a single leg).
- `PendingEcho`: delete every `reentry_*` field and the re-entry branch of
  `resolve_order_echoes` (world.rs:4787-4889 keeps only the analytic estimate path);
  `response_on_reentry` leaves the wire (`PendingOrderView`), as does `beyond_comms`
  (everything is beyond the wire now — the concept is gone). KEEP `PendingOrderLoss` and the
  lost-order wire/UI fields, deliberately unreachable until commit 2 re-adds their producer —
  say so in the commit message. Update the hard-coded "relay destroyed" copy in commit 4.
- Orders like `BuildEmplacement` keep working for the sensor; `every_player_starts_with_one_buoy`
  test and friends die.

**Server — serving:**
- LIVE branch of `serve_track_cached` (own+in_comms replay, view.rs:2381-2429) — everything
  serves on the DARK strict-arrival path (own fleets via `FrontierPath::DirectOwn`).
  `Track.in_comms`/`live_delay`/`bubble_transitions`, `nominal_bubble_crossing`
  (view.rs:2336-2351), `presentation_reentry_contains`/`BUBBLE_HYST` (view.rs:60), comm
  epochs/deaths in `FrontierCache` (view.rs:382-394) — delete.
- **Signal pricing collapses to one division**: with no lanes and no sites,
  `delay = dist / (c·WARP_FACTOR)`. Delete the route-research machinery whole:
  `FrozenRoute`, `RouteStart`, `route_channel`/`route_start` (view.rs:2674-2737),
  `adjusted_fast_delay`, `ROUTE_RESEARCH_EVERY`, `schedule_sample`'s caching layers
  (view.rs:2761-2912 reduces to: price emission, push to the heap). KEEP the heap +
  `pending_by_arrival`/cursor discipline — straight-line pricing from a static CC is
  arrival-ordered for cruising hulls, and commit 2's jumps are what re-introduce reordering.
- `estimate.rs`/`timeline.rs`: swap their `DelayField` inputs for the straight `delay()`;
  `ears` parameters die.
- game_loop: `PreviewRoute`/`RoutePreview` (both directions + handler) die — the client draws
  its own straight preview. `intended_route` (game_loop.rs:166-182) becomes the two-point
  `[sighting, dest]` line (own pending orders still draw). Welcome loses `GalaxyInfo.lanes`.

**Client:**
- Types: `LaneView`, `GalaxyInfo.lanes`, `PathPointView.lane` (paths are plain points now),
  `PreviewRoute`/`RoutePreview`, `beyond_comms`/`response_on_reentry` fields and their copy.
- render.ts: `drawLanes` (:1195), `laneBoundaryWorld`, `laneArcs`/`projectLane`/
  `laneHalfWidthAt`/`laneSpan`, `LANE_DRAW_FRAC`, `commLaneZones` (:417), lane-vs-warp path
  styling (:2090, :2148 — one dashed style remains), buoy siting feedback (:2527-2550).
- main.ts: lane/tunnel copy — `regimeCell` (:1332) and `domainCell` (:1392) lose hyperspace
  rows/tooltips, `notifyTunnelTransitions` (:1033), tunnel notice (:1705), emplacement section
  reduces to the Deep Space Sensor (:4132-4216), "ETA unknown" re-entry copy (:1156).
- state.ts: `ghostInTunnel` (:54), `tunnelBookmarks` (:122), `CommandSignal.hops` doc.
- The intent-bar move flow (`beginPendingIntent` :850, confirm :881) — falls back to a locally
  drawn straight dashed preview; keep the flow itself untouched (commit 4 extends it).

**Tests:** delete lane.rs suites (~48 + route_flight) with the file, emplace lane tests, buoy
grant, lane order-timing (world.rs:14472/14486), view relay/bubble/re-entry/coupled fixtures
(view.rs:3123 `NO_LANES`, :3212, :3691, :3785, :3855, :3923, :3440), game_loop lane comet tests
(:2554-2725 — rewrite the survivors as single-leg straight-signal versions: comet geometry,
meeting solve, confirm-from-served). Update `drive_wire` (ship.rs:2091) for the two-variant
`Regime`. Keep green: `audit_determinism.rs`, `no_leak_before_light_arrives` (refit fixture
sans bubbles), detection/market/tactical/ground suites.

**Verify:** fresh server — join; build a Deep Space Sensor; move/trade/raid round-trip; orders
confirm from served light; pacing note: hub→rim news is now ~200 s (was ~20 s on trunks) and
home→hub order delay ~37 s — that IS the plain model, flag it for playtest tuning, not code.
`cargo test -p sim -p server` green; `npm run build` green; grep sweeps clean.

## COMMIT 2 — `§jump-v1: spool and skip` (sim core)

Constants in transit.rs: `JUMP_SPOOL_S = 10.0`, `JUMP_RANGE = 50_000.0`,
`JUMP_FUEL_FACTOR = WARP_FACTOR`.

Types:
- ship.rs: `ShipKind::has_jump_drive()` beside `broadcasts()` (:190) =
  `matches!(self, Raider | Scout)` + a test pinning equivalence to `!broadcasts()` across all 13
  kinds. `Fleet::can_jump()` = every roster kind has the drive.
  `FleetOrder::Jump { dest: Vec2, #[serde(default)] spool_started: Option<f64> }` — the
  `Construct { started }` / `Survey { dwell_since }` pattern (ship.rs:722, :756). **Do not touch
  `DriveState`** — the spool clock is world-managed. `Fleet.last_jump: Option<f64>`
  `#[serde(default)]` (feeds commit 3's discontinuity detection). In `Fleet::advance`: Jump
  behaves like Idle — extend the `under_way` check (ship.rs:1609) and zero `vel` in the order
  branch (:1716) (cruise drives wind down in ~1 s, inside the spool).
- event.rs: `OrderKind::Jump` + `label()`; `EventPayload::FleetJumped { owner, fleet, from, to }`
  (persistence/audit/test hook — **no player surface**; the map is the disclosure);
  `EventPayload::JumpFailed { owner, fleet, pos, reason }` with
  `JumpFailReason { NotAJumpFleet, OriginInGravityWell, TargetInGravityWell, OutOfRange }`;
  `OrderRejectReason` += `NotAJumpFleet`, `TargetInGravityWell`.
- command.rs: `Command::JumpShip { player_id, ship_id, dest }` beside `MoveShip` (:31).
- fuel.rs: `ShortfallKind::Jump`.

Lifecycle:
- **Issue** (`World::apply`, model on the MoveShip arm world.rs:5077-5121): owner/supply gates;
  `!can_jump()` → `OrderRejected { NotAJumpFleet }` (own composition is CC-known —
  CONTROL-clean); dest inside a well → `OrderRejected { TargetInGravityWell }` (public chart);
  **no range check** (decision 4); fuel **warning** (`ShortfallKind::Jump`, warn-not-block,
  world.rs:5108 stance); then `schedule_for_owner(..., FleetOrder::Jump { dest, spool_started:
  None }, OrderKind::Jump, ...)`.
- **Scheduling**: `schedule_for_owner` (world.rs:10820) — zero changes; the meeting solve,
  comet, and analytic echo are order-agnostic. Add a Jump arm to `pending_order_subject`
  (world.rs:364) so the queue row and comet carry `dest`. `has_fixed_intent_path`
  (game_loop.rs:184-192) already excludes Jump — no intent line is served for it.
- **Delivery**: `deliver_due_orders` — zero changes; installs the order, emits
  OrderApplied/OrderDelivered, pushes the ordinary echo.
- **NEW `World::resolve_jumps(&mut events)`**, called in `step` between `resolve_surveys`
  (world.rs:1953) and `resolve_order_echoes` (:1960) — after movement and the combat passes,
  for the reason the survey comment there states; copy `resolve_surveys`' shape
  (world.rs:4211-4256). Per fleet holding `FleetOrder::Jump`:
  - **Engaged → abort to Idle** (no extra event — the battle is the news).
  - **`spool_started: None` → validate, then start**: `can_jump()` (a rendezvous merge may have
    changed composition), origin outside all wells, dest outside all wells,
    `pos.distance(dest) <= JUMP_RANGE` from CURRENT pos. Fail → `order = Idle` +
    `JumpFailed { reason }`. Pass → `spool_started = Some(now)`. (A docked fleet sits inside
    its system's well → `OriginInGravityWell` rejects it automatically.)
  - **`Some(t0)` with `now - t0 >= JUMP_SPOOL_S` → fire**: revalidate (mid-spool merge); fuel
    pay-before-you-move (`fuel_cost(dist, mass) / JUMP_FUEL_FACTOR`; dry → `stalled` latch +
    one `FuelShortfall { Jump }`, KEEP the order, retry each tick); then teleport:
    `from = pos; pos = dest; vel = ZERO; drive_state = Thrusters; regime = Thrusters;
    order = Idle; last_jump = Some(now)`; emit `FleetJumped`.
  - **Loss sweep** (decision 6): every `pending_orders` row with `ship_id == fleet &&
    loss.is_none() && apply_time > now && issued_at <= now` → `loss` with
    `news_at = now + transit::delay(from, owner_cc)` — the mortal-order machinery gets its new
    (sole) producer. The `issued_at <= now` bound protects orders issued after the jump: the
    meeting solve reads live `ship.pos`, so they aim at the landing and must survive.
- `integrate_movement`: exclude Jump from the cruise `fuel_tick` (world.rs:2191 gains
  `| FleetOrder::Jump { .. }`) so a spooling fleet can't trip `stalled`. LyFlown research credit
  never sees the teleport because `resolve_jumps` sets `pos` outside `integrate_movement` —
  closed by construction; add the regression test, not code (a 50k credit would be 1,250 ly
  against a 200 ly programme gate).
- timeline.rs: ingest `JumpFailed` as a positioned, delayed owner entry (the RaidResolved shape,
  timeline.rs:82-99); copy for the new reject reasons (:287). Deliberately **no timeline entry
  for `FleetJumped`** — undelayed would leak; delayed would duplicate the map.
- config.rs:26: scope-note on `C_SPEED_RATIO` — the invariant covers **cruise speeds**; chained
  jumps (~5,000 su/s average) outrun warp light (2,000 su/s) **by design**; honesty is
  per-wavefront, and the arrival heap (kept in commit 1) is what serves reordered light.
- Tests: spool fires on the first tick `>= 10.0`; kind gate at issue AND fire (mid-spool merge);
  all four `JumpFailReason`s; engagement abort; discrete fuel + dry-stall-then-fed fire; no
  LyFlown credit; in-flight loss (fleet ~30 light-seconds out, move issued in the spool's last
  seconds: `loss` set with `news_at = fire + delay(from, cc)`; a post-jump-issued order
  survives); pursuer of a jumped target replots without panic (`chase_aim` + `CHASE_REPLOT`,
  world.rs:2204-2239); a later order delivered mid-spool cancels the jump;
  `audit_determinism.rs` green — extend its `drive()` schedule with a periodic `JumpShip` so
  jump state sits inside the byte-for-byte and round-trip nets.

## COMMIT 3 — `§jump-v1: serve the skip honestly` (view discontinuity)

The failure mode: samples are one tick apart in **emission**, and serving interpolates when the
**arrival** gap ≤ `SMOOTH_BRACKET_MAX = 0.5 s` (view.rs:3087-3093). A mostly-tangential 50k jump
changes observer distance by under 1,000 su ⇒ sub-0.5 s arrival gap ⇒ the ghost would lerp
across 50,000 su. The flag is required, not cosmetic.

- `Sample` (view.rs:66): add `jump: bool` — "never interpolate from the previous sample to me".
  Server-memory only, stays `Copy`. `Sample::interpolate` (:92) sets it false on synthetics.
- `PositionHistory::record` (view.rs:505): per fleet,
  `jumped = ship.last_jump >= previous_sample.time` (`record` runs post-step at jump time + DT;
  the previous sample carries `time == jump_time`, so `>=` is exact — comment it). Set the flag
  on the current sample.
- The bracket (view.rs:3087-3093): add `&& !newer.sample.jump`. Serving holds the pre-jump
  sample until landing light arrives, then snaps — the discrete seam is the honest form.
- Tests: extend `no_leak_before_light_arrives` (view.rs:4120-4156) — flag the at-Y sample and
  strengthen to a sweep: for every `now` on a fine grid the served position is EXACTLY X or
  EXACTLY Y, never strictly between; add a second command center placed so the X/Y arrival gap
  is sub-0.5 s (the case that fails without the flag). **End-to-end**: real `World` +
  `PositionHistory` — a raider ~40k from its cc spools and jumps; the owner's view serves the
  spool position until `landing + delay(dest, cc)`, and the order confirms only on that served
  evidence (`confirm_orders_from_served`, the game_loop.rs:2022 shape).

## COMMIT 4 — `§jump-v1: aim and confirm` (client + protocol)

Server surface:
- protocol.rs: `ClientMsg::JumpShip { ship_id, dest }` beside MoveShip (:40); `GalaxyInfo` +=
  `jump_range`, `jump_spool_s`, `hyperlimit` (public constants — the `raider_speed` precedent,
  game_loop.rs:692). game_loop.rs:765: route it to `Command::JumpShip`.

Client:
- protocol.ts mirrors; `OrderVerb` += `"jump"` (state.ts:80); module state
  `jumpAiming: string | null` in main.ts.
- **Panel entry**: `jumpSection` beside `postureSection` (main.ts:1206) — only when
  `own && composition.every(kind ∈ {raider, scout})`; button "Jump drive · J" arms aiming;
  readout: "pick a point within 50,000 su — both ends must be clear of gravity wells". `J` key
  (main.ts:4334-4405, unbound) arms for the selected own fleet. Convoys/capitals never see it.
- **Aiming**: in the map-click handler BEFORE the empty-space move branch (main.ts:4071-4075):
  validate against the SERVED ghost (`dist(ghost.pos, dest) <= jump_range`; `dest` ≥
  `hyperlimit` from every system) — invalid: warning readout ("outside the ring" / "inside a
  gravity well"), no intent; valid: `beginPendingIntent({ shipId, verb: "jump", dest })`, clear
  `jumpAiming`.
- **Confirm — the SAME two-step as moves**: `intentSummary` jump case ("Jump <name> → (x·y) —
  ~10 s spool, then instant relocation · signal ~Ns"); `previewReadout` notes the ring is drawn
  from a light-delayed sighting; `confirmPendingIntent` (:881-964) sends
  `{ type: "JumpShip", ship_id, dest }` and does **not** write `state.orders` (a jump is not a
  flight — clear any stale entry); `jumpOrderReadout` in the `moveOrderReadout` style
  (:861-879): "Order away — reaches it in ~Ns, spools ~10 s, then relocates; you'll see the
  departure and the arrival when their light reaches you." Enter/Esc already wired
  (:4339/:4370; Escape clears `jumpAiming` first).
- **Orders zone** (main.ts:1137-1193): `orderObject` gains `"Jump → (x·y)"`. Phases: ◈ signal
  outbound; "◔ spooling ~Ns (est)" while `now < arrives_at + jump_spool_s`; then "◔ presumed
  jumped · awaiting light". All client-derived from `arrives_at + jump_spool_s` — **no JobView
  spool overlay** (that channel is current-truth; leave the anti-decision comment). Lost rows:
  replace the "relay destroyed" copy (:1142-1145) with "LOST — the fleet jumped away before the
  signal arrived" (jumps are now the only producer).
- **render.ts**: while aiming or a jump intent is pending — dashed range ring (`jump_range`)
  around the SERVED ghost labeled "est."; well feedback inside the ring (faint hyperlimit
  circles or a red target tint); chosen dest: target ring + dashed origin→target link,
  illustration-only. **Own-ghost snap**: drop the `!own` condition at render.ts:2704-2707 and
  fire `spawnReacquisition` (:2682) on own snaps past the threshold so a landing reads as an
  event, not a smear.
- Verify: `cargo test -p server`; `npm run build`; manual — select raider → J → aim (ring +
  well feedback) → Confirm·Enter → ◈ → ◔ spooling → ghost snaps on landing light →
  confirmation toast; a convoy shows no jump button; a second client sees the rival ghost
  vanish/appear on ITS own light only.

## COMMIT 5 — `§jump-v1: write it down`

- GAME_DESIGN.md — §7: past-tense retirement paragraph in the flip-and-burn house style
  (:461-466) for the whole hyperspace layer (lanes, three regimes, buoys/repeaters, coupled
  signals): built, playtested, removed — and why; §7: the jump-drive paragraph (kinds, 10 s
  stationary spool, 50k range, wells, fuel-as-warp-distance, no epistemic special case,
  in-flight orders lost), constants marked Tunable; §3 "master principle" (:146-173): rewrite —
  information travels as straight warp light from every event to each command center, no wire;
  §6: purge bubble/re-entry/beyond-comms language; §4.1 knob table: home ring is a literal
  80,000 su (charts unchanged for a given seed); §20 (:1669-1682): replace the stale lane entry
  (lanes were built as generated geography, then removed; the player-built mass-reduction
  design remains unbuilt); §21: the jump-flash detection gap.
- README: "What's implemented" Movement + comms rows; fresh-galaxy requirement (old snapshots
  incompatible); env vars unchanged.
- docs/comms-event-audit-v3.5.md: mark superseded → add a `§jump-v1` revision block: removed
  rows (bubble edge, re-entry confirmation, relay loss, CommandSignal hops), new rows —
  `FleetJumped` (PHYSICS — the map is the sole surface), `JumpFailed` (EVENT — positioned,
  timeline-delayed), `OrderKind::Jump` confirmation (served light only), and the serving rule
  "a flagged discontinuity sample is never interpolated across".
- docs/playtest-galaxies.md: presets regenerate (snapshot break); charts per seed unchanged;
  pacing note — news at 2,000 su/s everywhere (hub→rim ~200 s), order delay home→hub ~37 s.

## Verified gotchas — do not rediscover these

- Raw `fuel_cost` makes a max-range jump cost 143% of a tank — divide by `JUMP_FUEL_FACTOR`.
- The loss sweep must bound `issued_at <= jump_time` or it kills valid post-jump orders.
- Revalidate composition at FIRE, not just spool start (mid-spool rendezvous merge).
- Wells for jump validation are built from `systems` (world.rs:2152-2156) — confirm the Market
  Hub contributes one, else raiders jump straight into the sovereign zone past the hyperlimit.
- The client's reason→copy maps (main.ts:7842 region) need the new reject/shortfall variants or
  toasts fall back to raw slugs.
- The blockade re-target sweep (world.rs:4632-4657) rewrites `MoveTo` on mission fleets only —
  it cannot touch a `Jump`; verified, keep it that way.
- `history horizon` sizing (view.rs:489-501) already spans the galaxy at warp light — jumps
  don't extend it (light still travels; only the hull skips).

## Working rules

- Five commits, exactly the § messages above, each independently green:
  `cargo test -p sim -p server` + `npm run build`. Commit 2's message notes the known,
  client-unreachable serving gap that commit 3 closes.
- Keep the sim core pure; every new persisted field `#[serde(default)]`;
  `audit_determinism.rs` green throughout.
- Removal discipline: no `#[allow(dead_code)]`, no commented-out blocks, no "just in case"
  re-exports. The archive is commit `18a1a20`; the tree carries only the live game.
- If context runs short, stop at a completed commit and leave a STATUS note (README or final
  message): what landed, what's stubbed, what's next, anything that looks off.
