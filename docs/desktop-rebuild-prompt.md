# Implementation prompt — desktop shell rebuild ("the Deck", single task, 7 phases)

Repo: stellar-syndicates. Base branch: `codex/desktop-ux-revamp` (@ 337546c).
Working branch: `codex/desktop-deck`.

Read BEFORE starting:
  docs/desktop-shell-analysis.md  — why the current desktop shell is being
                                    replaced, what must survive, the target
                                    model, and the measurable acceptance bar.
  This file                       — the pinned design and the phase plan.

## Product decisions (already made — do not re-litigate)

- BUILD A NEW DESKTOP SHELL from scratch at `client/src/shell/deck/` with its
  own `client/src/styles/deck.css`. The existing `client/src/shell/desktop/`
  keeps working as the default until the Phase D6 cutover, then is DELETED.
- This is a RE-STRUCTURE, not a re-theme. Keep the dark terminal aesthetic,
  the existing color tokens, and the light-delay voice in every string.
- FULL PARITY with today's desktop shell: every ClientMsg sender in Appendix B
  must be reachable in the Deck. Mobile stays untouched except the shared-core
  edits explicitly allowed in "Core changes."
- The layout is: full-bleed map stage + one top bar + ONE right workspace
  (three widths, route-declared) + one bottom command strip + one toast lane +
  modal overlays (join, battle theater, ground theater, shortcuts help) + one
  founding chip. NOTHING else floats. No panel ever covers the map without the
  camera being told.
- One router drives everything: nav buttons, map inspection, deep links,
  breadcrumbs, Back, browser back, and Esc all walk the same history.
- Selection is not navigation: left-click commands, right-click inspects, and
  the hub is a move target like any star.
- The renderer, both theaters, the semantic zoom, and all of `core/` are
  UNTOUCHED except the explicit "Core changes" list and the Phase D5 renderer
  additions.

## Non-negotiable rules

1. Work phase by phase, in order. Commit at every numbered step.
   STOP AND REPORT at the end of each phase.
2. `npm --prefix client run build` (runs `tsc --noEmit`) must be clean after
   every commit.
3. RUST IS FROZEN. Every protocol message the Deck needs already exists
   (Appendix B). If you believe you need a server change, stop and report —
   do not make it.
4. Core edits are limited to the "Core changes" list. `core/mapclick.ts` is
   shared with mobile: after ANY edit there, re-verify mobile by loading the
   client at a phone viewport (or `?shell=mobile` once D0 lands) and issuing a
   tap-move, a long-press attack, and a pinch scrub.
5. Port the content emitters in Appendix C with their copy and math intact —
   adapt container markup and class names only. Do NOT rewrite the battle
   replay machine (`bvTick`/`gvTick` and the enter/exit crossfade in
   `shell/desktop/battle.ts`) or anything inside `battletheater.ts` /
   `groundtheater.ts`.
6. There is no client test suite. Verify against the running server
   (`.claude/launch.json`: `stellar-server` on 8080 + `stellar-client` on
   5173; `scripts/start.sh` for a clean world). Port the `window.__ss` debug
   rig in D0 — it is the only manual test path for the theaters.
7. The Deck must be teardown-safe: create ONE `AbortController` in `mount()`,
   pass `{ signal }` to every listener, and fully detach in `teardown()` —
   copy the mobile pattern (`shell/mobile/index.ts:104, 289-318`). Do NOT
   copy the old desktop shell's parked-DocumentFragment / never-removed
   window listeners pattern (`shell/desktop/index.ts:54-63`).
8. Where a ported function produced user-facing wording (readouts, reject
   reasons, inbox copy), keep the wording. Where `resolveMapClick` is
   unchanged, reject strings stay byte-identical.
9. If you find a pre-existing bug, leave it and note it in the phase report.

## Codebase orientation (verified 2026-08-29 — trust these)

The shell contract (`client/src/shell/types.ts:34-41`):

    export interface Shell {
      mount(root: HTMLElement, ctx: CoreContext): Promise<void>;
      onCore(events: CoreEvent[]): void;
      onViewTick(): void;
      framePolicy(): FramePolicy;   // {maxFps, renderGalaxy}
      cameraRect(): Rect;
      teardown(): void;
    }

- A shell module exports `createShell(): Shell`; that is all boot imports.
  `boot.ts` owns the single `Net`, the rAF loop, and the Pixi `renderer`; it
  funnels everything through `present()` → `activeShell.onCore(events)`.
  The SHELL initiates the first connection in `mount` (mobile
  `index.ts:330-331`: `if (net.connected) net.join(name); else net.connect()`).
  Boot calls `renderer.setCameraRect(shell.cameraRect())` after mount and
  applies `framePolicy()` every frame. Desktop-kind shells get 10 Hz Views.
- There is currently NO way to force a shell (`?server=` is the only query
  param read anywhere). D0 adds `?shell=`.
- `core/session.ts` `applyServerMessage` reduces the wire into `state` and
  returns `CoreEvent[]` (`core/events.ts:12-41`). Note `IntentChanged`
  carries `{intent, jumpAiming, guardAiming, readout?, renderIntentBar?,
  refreshShip?}` — `readout` is HTML and "omitted means leave the readout
  alone." A confirm emits TWO IntentChanged events (outcome readout, then bar
  teardown).
- `core/intent.ts` is the ONLY sender of map-verb orders (move/jump/raid/
  attack/guard/blockade/demolish/survey) via `confirmPendingIntent`. The
  shell arms (`armJumpAiming`/`armGuardAiming`), begins
  (`beginPendingIntent`), confirms, clears — never sends those itself.
- `core/mapclick.ts` `resolveMapClick(sx, sy, {shift, long}, ctx)` resolves
  precedence: jump aiming → guard aiming → blockade (raider + rival system) →
  survey (scout + unsurveyed) → candidate collection with click-cycling
  (`SYSTEM_BIAS = 5`, `CLICK_CYCLE_PX = 10`) → hub/anchor/battle fallbacks →
  empty-space move. `ctx.emplaceArmed` is vestigial (both shells pass null);
  emplacement siting is a panel button using `renderer.siteError`.
- `core/derive/*` is the render vocabulary (fleet, orders, market, research,
  captains, geo, format). TRAP: `bindFleetNet` / `bindMarketDerive` /
  `bindResearchNet` are module globals currently bound only by the OLD
  desktop runtime; `sendCrew`, `dispatchBuildKey`, `sendResearchQueue`
  silently no-op unbound. The Deck MUST bind all three in `mount()` (they
  encode real dispatch logic worth reusing).
- Renderer contract highlights: `renderer.canvas` is the input surface;
  `setCameraRect(rect|null)` tweens/refits (already handles preservation);
  `consumeSystemScrubEndpoint()` must be polled every tick (the ONLY
  renderer→shell channel); `isSystemScrubbing()` suppresses input;
  hit-testers (`fleetHitRadius`, `fleetScreenPosition`, `battlePick`,
  `systemPick`, `hubHitRadius`, `siteError`…) back mapclick; shell-written
  fields: `stateVersion` (bump after selection changes — session bumps for
  Views), `cursorWorld`, `selectedJumpDepartureKey`, `selectedBattleMarkerId`;
  `centerOnWorld(pos)` for ⌾ buttons; `zoomAt`/`panBy`/`resetView`;
  `atMaxZoom()`/`atBattleZoomThreshold()` gate the semantic ladder; use
  `liveSimTime()` from `state.ts`, never raw `state.simTime`, for clocks.
- `shell/dom.ts` is shared infrastructure: call `installPressGuard()` once in
  mount; wrap every per-View re-render in `renderDeferred(rootId, fn)`; write
  HTML only through `setHtml` (it morphs — preserves focus/caret/scroll/
  hover). NODE_KEY_ATTRS gains `"data-deck-act"` (Core change #3) or list
  reordering will recreate nodes mid-interaction.
- Copy mobile's proven patterns rather than the old desktop's:
  - `shell/mobile/signature.ts` `sheetFingerprint` — the served-slice
    signature gate that lets 10 Hz Views drive HTML without churn. Generalize
    it to `shell/signature.ts` in D0 and have both shells import it.
  - Notice "destinations" (tap a toast → navigate) — `mobile/notices.ts`.
  - One delegated click listener per region dispatching on `data-deck-act`
    (mobile uses `data-mobile-act`; theaters' handlers use `battle-*`/
    `ground-*` prefixes).
  - `history.pushState` integration so browser Back pops the workspace
    (mobile `sheets.ts:101-141, 276`).
- Theater mount contracts: `theaterAttach(mount, rec, pirateId, {width,
  height, maxFps?})` is safe to call every render (re-appends its holder);
  the shell owns ALL playback state and pushes `theaterSetTime(round, frac,
  live)`. Same shape for `groundTheaterAttach`. Fall back to mobile's
  `truthMap()` SVG when `theaterAvailable()` is false.
- The old shell is the CONTENT donor, not the structure donor: Appendix C
  lists exactly what to port. Its `markup.ts`, `workspace.ts`, placement CSS,
  `RIGHT_DOCK_IDS`/`MutationObserver`/`ResizeObserver` layout apparatus, and
  the 17 `__init_*` bootstrap calls are all superseded — do not port them.

---

## The Deck — frame specification

Six z layers, tokenized in `tokens.css` (Core change #4):
map canvas (below all DOM) · chrome 10 (top bar, founding chip, zoom cluster)
· workspace 20 · command strip 30 · overlays 40 (join, theaters, help) ·
toasts 50 · hover card 60.

Type scale, tokenized: `--fs-fine: 11px` (floor — nothing smaller),
`--fs-body: 12.5px`, `--fs-em: 14px`, `--fs-title: 16px`, `--fs-page: 20px`.
Spacing scale 4/8/12/16/24. `deck.css` uses ONLY tokens for font-size,
z-index, and spacing — zero raw px font sizes, zero unclamped widths.

**Top bar** (~40px, full width): left — corp name, credits (with reserved-
credit accounting promoted from the old `title=`), equity, tick/pacing, link
state. Right — the 8 nav buttons as ROUTER LINKS with badges (port
`updateNavBadges`, `shell/desktop/runtime.ts:91-108`): Command · Fleets ·
Market · Research · Officers · Operations · Syndicate · Faction — plus Log.
(Command replaces the old implicit "no page"; Log keeps its badge.)

**Workspace** (right dock, the only data surface):
- Three widths: closed · `standard` = `min(460px, 100vw - 28px)` · `wide` =
  `min(65vw, 1240px)`. Each route DECLARES its width (table below); a header
  toggle lets the user override, persisted per-route in localStorage.
- One header everywhere: ← Back · breadcrumb trail · title · width toggle ·
  ✕. No route renders its own ✕ or its own back.
- One scrollable body. Tabs inside a route are allowed (market's four, the
  system route's four) but exactly one level deep — never tabs-inside-tabs.
- The workspace publishes the camera rect: on every open/close/width change/
  resize it calls `renderer.setCameraRect({x:0, y:0, w: workspaceLeftEdge,
  h: innerHeight})` directly (it OWNS layout — no MutationObserver, no
  ResizeObserver, no measured CSS vars).
- Re-render gates: signature per route (shared `sheetFingerprint`) +
  `renderDeferred` + `setHtml`. Coalesce onto the `onViewTick` rAF as today.

**Command strip** (bottom-center, `min(720px, 92vw)`, z30) — the ONE home for
command state. Visible when a fleet/group is selected OR an intent is pending
OR an armed mode is active; hidden otherwise.
- Selection mode: chip(s) for the selected fleet/group (name, composition
  glyphs, delay badge; ⌾ to center; ✕ to deselect), then LEGAL verb buttons
  derived from existing helpers: Move (hint: click map), Dock (when
  `nearestKnownDock`), Jump (arms via `armJumpAiming`, shows range), Guard
  (arms), Hold (when a course exists), Transit Full/Stealth, Recall (when
  raiding). Illegal verbs render disabled with the reason inline — no
  `title=`-only explanations.
- Armed mode: "JUMP AIMING — click a destination · Esc cancels" chip with an
  explicit Cancel button (same for guard).
- Intent mode: the ported intent summary (`intentSummary`,
  `core/derive/orders.ts:90`) + Confirm (Enter) + Cancel (Esc). ENTER MUST
  WORK: one document-level keydown handler owned by the Deck that ignores
  events only when `target` is editable — verify with the browser, not by
  reading the code (this is a live August bug in the old shell).
- Status line: the last command outcome ("Order away to Interceptor —
  response ~42s"), replacing the old `#readout`. Last-write-wins, fades
  after 15s.
- IntentChanged handling: `readout` field → status line; `renderIntentBar` →
  re-render strip; `refreshShip` → refresh fleet route if open.

**Toast lane** (top-right, under the bar, z50): ONE implementation, cap 5,
12s fade (estimates 15s), each toast optionally carries a route destination
(click navigates — mobile's notices model). Sources: OrderConfirmed,
ReportArrived, BattleConcluded, ServerError, trade news, EstimateReady
summary. Every toast's content also lands in the Log. Refusals additionally
surface inline at the point of action where one exists (e.g. the build
queue row). The "soft-reject shows in the Log" footnote pattern dies.

**Founding chip** (bottom-left, z10): the ported founding guide
(`shell/desktop/founding.ts` content + prospect cards) restyled to tokens,
minimizable (persisted), action buttons drive the ROUTER (deep links). The
old `--founding-guide-clearance` ResizeObserver dance dies — the strip is
centered and the chip is left; at `≤1100px` the strip docks above the chip
via a media query.

**Zoom cluster** (bottom-right, z10): + / fit / − / `?`. The `?` opens the
shortcuts overlay (z40) listing every accelerator — the permanent legend
dies.

**Overlays** (z40, centered, with a dimmed backdrop): join gate (Deck-owned
markup, same behavior incl. SessionReplaced re-show), battle viewer, ground
viewer, shortcuts help. All four are in the Esc chain. While a viewer is
open, `framePolicy()` returns `renderGalaxy: false` (the backdrop makes the
old "map visible around the modal" reasoning obsolete). The battle viewer
ALWAYS opens above the workspace (this is a live z-order bug in the old
shell — `openBattleViewer` under the z20 workspace).

**Native dialogs die**: the 5 `window.confirm`/`prompt` uses (hostile-action
×3, flagship name, fit name) become inline confirm rows / inline text inputs
in their sections.

## Navigation specification

One router: `go(route)`, `back()`, `close()`; one history array mirrored
into `history.pushState` (browser Back = workspace Back; copy mobile's
marker/session pattern). Breadcrumb renders the logical path, e.g.
`Command › Themis › Themis I › Build`.

| route | width | content source (port from) |
|---|---|---|
| `command` (default) | standard | Decision-inbox digest (top 4) + founding progress + objectives + "next decision at" (`checkin.ts` `computeInbox`, `nextDecisionLabel`) |
| `system/:id` | standard | Tabs Overview · Worlds · Production · Build — `sysview.ts` emitters; Build tab hosts the REAL builder (below), not a button to elsewhere |
| `system/:id/world/:bid` | standard | World summary + economy + population as sections (old `#planet-panel` content), breadcrumb-nested |
| `system/:id/build` (+ `?body=`) | wide | Structure + ship builders as master-list → detail (old `.build-shell`: `buildRowHtml`/`buildDetailHtml`/`shipDetailHtml`/pools/queue) — INSIDE the workspace |
| `fleet/:id` | standard | `ship.ts` own stack (`ordersZone`, composition, split/merge, fuel, captain, refit, transit, hold, emplace, logistics, dock-load) or `rivalBody` for rivals |
| `fleets` | standard | Roster + group tools (`rail.ts` fleet tab; ⌾ + ✚ per row) |
| `logistics` | standard | Standing orders (`rail.ts`) |
| `doctrine` | standard | Doctrine form (`rail.ts`) |
| `market` (tabs ×4) | wide | `market.ts` board/ticket/order tables/warehouse/specialists/modules + hub berth roster and hub blurb (old `#hub-panel` merges here) + freight composer |
| `research` | wide | `research.ts` programme boards + academy |
| `officers` | wide | Captain roster (`rail.ts` officers tab) |
| `operations` | standard | `operations.ts` |
| `syndicate` | standard | `syndicate.ts` |
| `faction` | standard | `faction.ts` |
| `rankings` | wide | `rail.ts` rankings |
| `log` | standard | Full decision inbox + light-delayed log (`checkin.ts`) — inbox actions act IN PLACE (no close-and-open-elsewhere) |
| `battle/:id` | standard | Battle/capture report (old `#battle-panel`); "View replay" opens the theater overlay |

Map → router: inspecting a system/hub/rival opens its route. System-view body
clicks open `system/:id/world/:bid`. Deep links (founding buttons, toast
destinations, inbox Focus) are router calls, optionally with a camera
`centerOnWorld`/`pulseSystemBody` side-effect.

**Esc chain (pinned, in order)**: pending intent → jump/guard aiming →
system scrub (`cancelSystemScrub`) → open overlay (help, viewers — theaters
close via their own state machine) → workspace `back()` (one step; closes
when history empties) → deselect fleet → no-op. Keyboard accelerators kept:
M/V/R/P/U/Y/C/L routes, S system-of-selection, `[`/`]` cycle+center fleets,
J jump-arm, G guard-arm, Enter confirm. All listed in the `?` overlay.

## Selection & map-click grammar (pinned)

Persistent selection: `state.selectedShipId` + `selectedShipIds` (existing;
session already prunes/repairs them across Views). Selection changes bump
`renderer.stateVersion`. NOTHING clears selection implicitly — only ✕,
Esc-at-that-level, selecting another fleet, or the fleet ceasing to exist.

| gesture | with own fleet selected | without selection |
|---|---|---|
| LMB own fleet / berth pip | select it (replaces; Ctrl toggles into group) | select it |
| LMB system / star | MOVE intent (existing P0 behavior) | inspect → `system/:id` |
| LMB hub | **MOVE intent (Core change #1)** | inspect → `market` |
| LMB empty space | MOVE intent to point (existing) | nothing |
| LMB rival fleet | raid (sole raider) / attack (Shift or long-press) — existing rules | inspect ghost (readout/select) |
| LMB rival emplacement | demolish intent when `armedSelection` (existing) | select/readout |
| RMB anywhere | **INSPECT (Core change #1c)** — routes to the target's page; never commands, never clears selection; `contextmenu` suppressed on canvas | same |
| double-LMB system | semantic enter (existing) | same |
| Ctrl+LMB own fleet | group toggle (existing P3) | — |
| wheel | zoom / semantic scrub (existing, untouched) | same |

Hit-test priority (Core change #1b): own fleet markers and berth pips WIN
over the `SYSTEM_BIAS` system pull within tolerance — selecting your own
docked fleet must never require click-cycling. Click-cycling stays for
everything else. The hover card is ported (`updateMapHover` copy) and always
names the resolved action ("Move Interceptor to Idun", "Inspect Wormhole
Hub — right-click").

## Core changes (the complete list — nothing else)

1. `core/mapclick.ts`:
   a. Hub branch: own fleet selected + hub click → `{kind:"intent",
      verb:"move", dest: hub pos}` instead of `{kind:"select", type:"hub"}`.
   b. Candidate ordering: own-fleet candidates (incl. berth pips) beat the
      system bias when both are within tolerance.
   c. `mods` gains `inspect: boolean` (RMB). When true, skip every intent
      branch and resolve to the select/inspect target only. Mobile passes
      `inspect: false` (no behavior change); desktop-old is untouched (it
      never passes it — default false).
   After ANY of these: re-verify mobile per Rule 4, and re-verify reject
   wording is unchanged for untouched branches.
2. `boot.ts`: read `?shell=deck|desktop|mobile` once at boot; param wins over
   `matchMedia` for the desktop-kind choice; breakpoint crossing still swaps
   desktop-kind ↔ mobile. Until D6 the desktop-kind default stays
   `shell/desktop`; D6 flips it to `shell/deck`.
3. `shell/dom.ts`: append `"data-deck-act"` to `NODE_KEY_ATTRS`.
4. `styles/tokens.css`: add the type/spacing/z tokens (additive only).
5. D0 only: move `shell/mobile/signature.ts` → `shell/signature.ts`;
   mobile's import path updated (mechanical).
6. D5 only, `render.ts`, all additive and default-off, shell-opt-in:
   own-asset label floor (labels for own fleets/systems at all zooms),
   home-system badge, `pingWorld(pos)` transient focus ring, selected-target
   reticle. Re-verify mobile after.

---

# PHASE D0 — scaffold, frame, and the switch

0.1 Branch `codex/desktop-deck`. Create `client/src/shell/deck/` skeleton:
    `index.ts` (`createShell`, mount/teardown with one AbortController),
    `markup.ts` (the frame regions only), `router.ts` (route table, history,
    breadcrumb, pushState integration), `workspace.ts` (widths, header,
    camera-rect publishing), `toasts.ts`, `strip.ts` (empty shell), and
    `styles/deck.css` (tokens-only). Core change #4 (tokens) and #5
    (shared signature module) land here.
0.2 Core change #2: `?shell=` override in boot; dynamic import of the deck.
    Old shell remains the default desktop-kind.
0.3 Deck join card (same join/SessionReplaced behavior as old, own markup).
    Top bar with live stats + router-wired nav + ported `updateNavBadges`.
0.4 Workspace frame: routes render placeholder bodies; width declaration +
    toggle + persistence; breadcrumb + Back + ✕; camera rect published on
    every change (verify the map re-centers with the existing tween).
0.5 Toast lane (one implementation, destinations supported). Zoom cluster +
    `?` overlay stub. Port the `__ss` rig install (state, renderer, net,
    theater helpers `theaterDemo`/`theaterDemoLive` reachable — steal the
    two demo-record builders from `shell/desktop/battle.ts:1084,1171`).
0.6 `framePolicy()` honest from day one (renderGalaxy false only under
    covering overlays — none yet).

D0 ACCEPTANCE: `?shell=deck` joins a live server, map pans/zooms/scrubs,
all routes navigate with Back/browser-Back/breadcrumbs, camera insets per
width, toasts render, `?shell=desktop` (and no param) is byte-identical to
before, build clean. STOP AND REPORT.

# PHASE D1 — selection and the command grammar

1.1 Core change #1 (mapclick: hub move, own-fleet priority, `inspect` mod) +
    mobile re-verification per Rule 4.
1.2 Deck map interaction: pointer handlers on `renderer.canvas` (5px
    click-vs-drag gate as today), LMB via `resolveMapClick`, RMB via
    `inspect: true` → router, dblclick semantic enter, wheel zoom/scrub port,
    `consumeSystemScrubEndpoint` polling in `onViewTick`, cursorWorld +
    hover card (ported copy, extended with the RMB hint).
1.3 Command strip: selection mode (chips, legal verbs with inline disabled
    reasons), armed mode (chip + Cancel), intent mode (ported
    `intentSummary` + Confirm/Cancel). Document-level keydown: Enter
    confirms, Esc walks the pinned chain. VERIFY ENTER IN THE BROWSER.
1.4 Multi-select: Ctrl-click toggle, `[`/`]` cycle+center, batch-move
    banner (port P3 semantics).
1.5 Status line replaces `#readout` routing: IntentChanged.readout →
    strip status; OrderConfirmed / ServerError → toast + status.

D1 ACCEPTANCE: with a fleet selected — move to star, move to EMPTY SPACE,
MOVE TO HUB (selection retained), jump-arm + click, guard-arm + click, raid
a rival, Shift-attack, hold, transit toggle; Enter confirms and Esc chain
matches the pinned order; RMB on hub opens market while a fleet stays
selected; own docked fleet selectable by direct click; mobile smoke passes.
STOP AND REPORT.

# PHASE D2 — empire routes (the founding path)

2.1 `system/:id` route: Overview/Worlds/Production tabs from `sysview.ts`
    emitters (`updateSystemTab`, body rows with inline deposits,
    `productionReadout`, ship-goods button, `assignmentLines`,
    `buildQueueRows`). Owned vs rival vs unsurveyed states.
2.2 `system/:id/build` wide route: master list → detail from
    `buildRowHtml`/`shipRowHtml`/`buildDetailHtml`/`shipDetailHtml` + pool
    bars + queue + fitting bar/module forge/fit picker. Bind
    `bindMarketDerive` in mount; dispatch via `dispatchBuildKey`. Inline
    soft-reject display at the queue row (plus toast) — no Log-only
    refusals.
2.3 `system/:id/world/:bid`: world summary/economy/population sections
    (old planet-panel content) with Build buttons deep-linking to the build
    route with `?body=`.
2.4 System VIEW (orrery) wiring: body click → world route;
    `setSystemDynamic`/`pulseSystemBody`; breadcrumb shows
    `Galaxy › <system>`; owned-system management chrome (old
    `#sysview-manage`) becomes just… the system route (it already is).
2.5 Founding chip + `command` route home (digest, founding progress,
    prospect cards); every founding action button routes + focuses.

D2 ACCEPTANCE: complete founding steps 1–3 live; "Build Shipyard I" from
the chip is ≤3 clicks with ONE surface named Build; builders never occlude
the map without camera compensation; Esc backs out level by level to an
anchored galaxy (home still selected/centered). STOP AND REPORT.

# PHASE D3 — economy routes

3.1 `market` wide route: price board + sparklines + staleness, trade ticket
    (integrated, not floating), open/incoming/recent order tables,
    warehouse, specialists, modules, hub berths + hub blurb, freight
    composer. Reservation lifecycle on `TradeSettled` →
    `settleMarketReservation` + `recordRecentMarketOrder` (mobile
    `surfaces.ts:126-129` is the reference); `reserveMarketOrder` on
    submit; spendable-credits display with the reserved accounting visible.
3.2 `logistics` route (standing orders builder) + `doctrine` route.
3.3 Trade news toasts with market destinations.

D3 ACCEPTANCE: buy, sell, limit order place + cancel, module buy/sell,
specialist hire, freight booking, standing order set/clear — all live
round-trips with visible feedback in strip/toast/log. Wide market is
readable at 1440px with no horizontal scroll. STOP AND REPORT.

# PHASE D4 — fleet, research, officers

4.1 `fleet/:id`: port the full own-fleet stack (ordersZone with the 3-phase
    order lifecycle, composition + split/merge, fuel + AAA rescue, captain
    card + manage, refit table, transit, hold, emplace-with-`siteError`,
    logistics/dock-load with select-preservation) and `rivalBody` fog view.
    Estimates: `EstimateReady` → panel section when the fleet route is
    open, toast summary otherwise.
4.2 `fleets` roster + group management; `officers` wide roster (lazy
    portraits — keep the derivative pipeline's srcset); `research` wide
    boards + queue via `sendResearchQueue` (bind in mount) + academy.
4.3 Jump departures + emplacement selection paths (map → readout/route)
    ported.

D4 ACCEPTANCE: Appendix B sender sweep — every listed message demonstrably
reachable in the Deck (tick them off in the phase report); refit + transit +
hold verified live; research queue reorder round-trips. STOP AND REPORT.

# PHASE D5 — awareness: log, saliency, help

5.1 `log` route: `computeInbox` ported wholesale; inbox actions act in
    place (Auto-supply etc. send without closing); Focus = router +
    `centerOnWorld` + `pingWorld`. Badge model from `decisionKeys`.
5.2 Core change #6 renderer saliency (own labels, home badge, ping,
    reticle) wired to Deck opt-in; mobile re-verified.
5.3 Tooltip audit: all 199 `title=` in ported content classified — promote
    load-bearing ones into visible copy/inline hints, drop decorative.
    List the promotions in the phase report.
5.4 `?` shortcuts overlay complete; legend markup never ported.

D5 ACCEPTANCE: a fresh corp's first 10 minutes need zero tooltips and zero
legend; every analysis §6 measurable that applies so far passes.
STOP AND REPORT.

# PHASE D6 — closeout, theaters, cutover, deletion

6.1 Remaining routes: `operations`, `syndicate`, `faction`, `rankings`,
    `battle/:id` (+ capture reports), with their emitters ported.
6.2 Theater overlays: battle viewer + ground viewer as z40 modals with
    backdrop; the replay machine (`bvTick`/`gvTick`, aftermath scheduling,
    force strip, per-fleet withdraw, live-frontier chase) ported UNCHANGED
    in logic; semantic-zoom entry (`enterBattleViewer` path) and
    "View replay" both open ABOVE the workspace; Esc closes; ground viewer
    joins the Esc chain; `framePolicy().renderGalaxy = false` while open;
    theater canvas re-attach guard kept (`theaterAttach` every render).
6.3 Replace the 5 native dialogs with inline confirms/inputs.
6.4 CUTOVER: boot default desktop-kind → `shell/deck`. `?shell=desktop`
    removed. DELETE `client/src/shell/desktop/` and
    `client/src/styles/desktop.css` entirely (the Deck must import nothing
    from them — Appendix C content was ported, not re-exported). Remove
    dead body-class arbitration and any leftover ids from `index.html`.
6.5 Final sweep against docs/desktop-shell-analysis.md §6 — every bullet
    verified live, listed in the report with pass/fail. Update README's
    play guide screenshots/keys section if it references old chrome.

D6 ACCEPTANCE: full session on the Deck only — join, found through step 5+,
trade, research, fleet ops incl. a raid + replay viewing, syndicate/faction
visits, log/inbox actions — zero references to the old shell in the bundle
(`grep -r "shell/desktop" client/src` empty), build clean, mobile
untouched-and-verified. STOP AND REPORT.

---

## Appendix A — old surface → Deck disposition

| old surface | disposition |
|---|---|
| `#hud` | top bar (slimmed; stats + nav) |
| `#desktop-workspace` + 10 pages | THE workspace + routes (Appendix in nav table) |
| `#rail` 6 tabs | routes: system/fleets/logistics/doctrine/officers/rankings |
| `#ship-panel` | `fleet/:id` route |
| `#hub-panel` | merged into `market` |
| `#battle-panel` | `battle/:id` route |
| `#sysview-manage` | `system/:id` route (owned state) |
| `#planet-panel` | `system/:id/world/:bid` route |
| `#build-panel` / `#build-ship-panel` | `system/:id/build` wide route |
| `#checkin` | `command` digest + `log` route |
| `#battle-viewer` / `#ground-viewer` | z40 overlays (logic ported unchanged) |
| `#reports-log` | toast lane (one impl) |
| `#readout` | command-strip status line |
| `#intent-bar` | command-strip intent mode |
| `#map-hover` | hover card (kept, copy extended) |
| `#breadcrumb` | workspace header breadcrumb |
| `#founding-guide` | founding chip (content ported) |
| `#legend` | deleted → `?` overlay |
| `#zoom-controls` | bottom-right cluster |
| `#join` | Deck join overlay |
| native confirm/prompt ×5 | inline confirms/inputs |

## Appendix B — sender parity checklist (all must be reachable in the Deck)

Via `core/intent` (map grammar): MoveShip (single + batch), JumpShip,
CommitRaid+EstimateEngagement, AttackFleet+EstimateEngagement, GuardFleet,
BlockadeSystem, DemolishEmplacement, SurveySystem.

Via bound derive senders: SetAssignment (`sendCrew`), BuildShip /
DevelopSystem (`dispatchBuildKey`), SetResearchQueue (`sendResearchQueue`).

Direct `ctx.send` (old desktop file:line for reference): SplitFleet,
MergeFleets, SetFleetPosture, SetEngageFreight, RefitShips, HoldFleet,
HubLoad, SystemLoad, HaulToSystem, HaulToMarketHub, DismissLostOrder
(`ship.ts:68-238`), PlaceLimitOrder (`market.ts:265`), SetFleetTransit,
RecallRaid, RequestFuelRescue, Withdraw, BuildEmplacement, HubUnload,
SystemUnload, MarketBuy, MarketSell, CancelLimitOrder, BuyModule,
SellModule, SaveFit, DeleteFit, HireSpecialist, RelocateMigrants,
SetMigrationPolicy, ShipProduction, NameFlagship, AssignCaptain,
ReserveCaptain, RecruitCaptain, TrainCaptain, SetStandingOrder,
ClearStandingOrder, SetFleetDoctrine, all syndicate/diplomacy/operation
verbs, PayReinstatement.

## Appendix C — port-verbatim content (file:line @ 337546c)

- `checkin.ts:41-409` — `INBOX_W`, `computeInbox`, `inboxCardHtml`, action
  dispatch. The crown jewel; lift wholesale.
- `sysview.ts` — `bodyProfileReadout:426`, `depositRow:676`,
  `productionReadout:704`, `converterBanner:755`, `assignmentLines:774`,
  `buildQueueRows:848`, `fittingBar:962`, `moduleForge:981`,
  `fitPicker:1000`, `colonyOpportunityBlock:1019`, `buildRowHtml:1218`,
  `buildDetailHtml:1229`, `shipRowHtml:1340`, `shipDetailHtml:1358`.
- `ship.ts` — `ordersZone:439`, `compositionSection:594`,
  `splitControls:617`, `headingCell:655`, `regimeCell:667`,
  `ownActivity:728`, `fuelSection:784`, `captainSection:808`,
  `officerCard:857`, `refitSection:1049`, `transitSection:1090`,
  `holdSection:1153`, `rivalBody:1233`, `emplaceSection:1474`,
  `dockLoadOptions:1507`, `syncDockLoadControls:1536`,
  `logisticsSection:1559`.
- `battle.ts` — the whole replay machine: `gvTick:544`, `openBattleViewer
  :625`, crossfade + cancel-safety `:686-729`, aftermath `:735`,
  `bvTick:748`, theater re-attach `:1075-1079`, demo records `:1084,1171`.
- `market.ts` — `renderMarketBoard:317`, `renderComposer:413`, order/
  inventory tables `:480-580`, `addTradeNews` copy `:704`.
- `rail.ts` — `fleetRosterRow:371`, `updateSystemTab:513`, `groundLine:948`,
  `landingOddsLine:984`, `berthLine:1015`, `updateRankingsPanel:280`.
- `mapchrome.ts` — UI kit `badge:303`…`statusIcon:403` + icon maps;
  `updateMapHover:234-291` copy.
- `founding.ts` — guide copy + `foundingProspectCards:40-66`.
- `runtime.ts:91-108` — `updateNavBadges` derivations.
- `checkin`/`format` reject vocabulary: `core/derive/format.ts:161
  rejectText` (already core — just use it).

## Appendix D — hygiene targets (from the analysis, enforced at D6)

≤6 z values, all tokenized · zero raw px font sizes in deck.css, 11px floor
· zero unclamped widths · one toast implementation · zero native dialogs ·
zero load-bearing `title=` · founding step 1 ≤3 clicks · hub-move 1 click +
confirm with selection retained · Enter confirms · every overlay in the Esc
chain · `grep -r "shell/desktop" client/src` empty after D6.
