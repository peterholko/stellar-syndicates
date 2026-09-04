# Codex implementation prompt — mobile UX (single task, 8 phases)

Repo: stellar-syndicates. Base branch: `codex/panel-ux-diet` (@ db40d9f).
Working branch: `codex/mobile`.

Design docs in this repo — READ BOTH BEFORE STARTING:
  docs/mobile-web-analysis.md   — what is broken on mobile today, with file:line
  docs/mobile-shell-split.md    — the core/shell architecture this task implements

## Product decisions (already made — do not re-litigate)

- The CURRENT UX becomes the DESKTOP SHELL, moved verbatim. It must not change.
- MOBILE IS FULL PARITY: every desktop panel gets a mobile form, including the
  research programme boards, operations, syndicate, and the battle/ground
  replay theaters.
- MOBILE IS PORTRAIT-ONLY. Landscape shows a rotate-to-portrait gate. This
  means the battle theater CANNOT fall back to "rotate to view the replay" —
  it needs a genuine portrait arena (Phase 6).

## Non-negotiable rules

1. Work phase by phase, in order. Commit at every numbered step.
2. STOP AND REPORT at the end of each phase. Do not roll phases together.
3. Phases 1-3 are refactors with NO user-visible change. Move code VERBATIM —
   no renames, no reformatting, no "while I'm here" cleanups. Those hide
   regressions in a 7,700-line move.
4. `npm run build` (which runs `tsc --noEmit`) must be clean after every commit.
5. If you find a pre-existing bug, leave it and note it in the phase report.
6. There is NO client test suite. Verify by hand against the running server and
   the `__ss` debug rig on `window`.
7. Server (Rust) changes are out of scope for every phase except where Phase 7
   explicitly allows them.

## Codebase orientation (measured — trust these numbers)

`client/src/main.ts` is 9,985 lines / 333 functions. Transitive DOM analysis:
  - 191 fns / 7,764 lines touch the DOM        -> becomes the desktop shell
  - 106 fns / 1,535 lines are DOM-free logic   -> extract to core/ (Phase 1)
  -  36 fns /   658 lines are DOM-free HTML    -> desktop shell
`client/index.html` is 1,781 lines: a 1,452-line <style> block + 321 lines of
body markup. The other 8,343 lines (protocol, render, battletheater,
systemview, groundtheater, state, icons, stars, net, prng) are already
shared-clean modules.

---

# PHASE 0 — touch primitives + reconnect

Makes the existing client reachable on a phone and able to survive
backgrounding. No layout redesign, no refactor. Ships independently.

0.1 Viewport meta (index.html:5): add `maximum-scale=1, user-scalable=no,
    viewport-fit=cover`. In-game zoom replaces document pinch-zoom.

0.2 touch-action. There is currently ZERO `touch-action` in the codebase, so a
    one-finger drag on the map canvas is claimed by the browser as a page
    gesture and fires pointercancel mid-drag — map panning is broken on touch.
    This is the single biggest break.
      - `touch-action: none` on the Pixi map canvas and on the battle-theater
        canvas (`.bv-theater-holder canvas`).
      - `touch-action: pan-y` + `overscroll-behavior: contain` on every
        `overflow-y: auto` region: #rail .rail__body, #market, #checkin,
        .svm-body, #ship-panel .sp-body, .build-shell .bp-list, #planet-panel,
        #research-panel, #operations-panel, #syndicate-panel, #faction-panel,
        #battle-viewer, #ground-viewer, #standing-list.

0.3 iOS input auto-zoom. Every input/select is 10-12px (index.html lines 261,
    269, 431, 836, 1124, 1167, 1194). iOS Safari zooms the viewport whenever a
    focused field is under 16px and never zooms back — focus the market qty
    field once and the layout is wrong for the rest of the session. Add a
    `@media (pointer: coarse)` block setting all form controls to 16px. Do not
    change desktop sizes.

0.4 Dynamic viewport. Replace the 15 `100vh` / `calc(100vh - ...)` uses with
    `100dvh`. Add a `visualViewport` resize+scroll listener next to the
    existing `syncHudSafeTop` (main.ts:40-49) that re-runs it — window
    `resize` alone does not fire for iOS URL-bar collapse, so bottom-anchored
    chrome (#founding-guide, #readout, #intent-bar, #legend) ends up under the
    browser UI.

0.5 Safe areas. Add `env(safe-area-inset-*)` to the fixed chrome insets:
    #zoom-controls (left:14px, index.html:862), #legend and #reports-log
    (right:14px), #founding-guide / #readout / #intent-bar (bottom:14px),
    #hud (top).

0.6 Two panels overflow a 390px viewport and are unreachable because
    `body { overflow: hidden }` clips them: `#checkin` is `width: 468px`
    (index.html:276) and `#join .card` is `width: 360px` (index.html:70).
    Make both `min(<current>, calc(100vw - 28px))`.

0.7 Cap device pixel ratio at 2 in BOTH Pixi inits — render.ts:513 and
    battletheater.ts:300 use uncapped `window.devicePixelRatio`. On a dPR-3
    phone a 390x740 viewport becomes a 1170x2220 backbuffer.

0.8 Stale hover on touch. main.ts:5380 sets `renderer.cursorWorld` on
    pointermove; on touch, pointerleave may never fire, leaving a stuck hover
    highlight. Clear it on pointerup/pointercancel when
    `e.pointerType !== "mouse"`.

0.9 Auto-reconnect (client-only). `client/src/net.ts` has NO reconnect logic:
    `connect()` is called once (main.ts:9630) and onClose (main.ts:9618) just
    sets link=offline. The server tears the socket down at READ_TIMEOUT=60s
    (crates/server/src/ws.rs:37, PING_INTERVAL=20s), so locking the phone for
    a minute ends the session with no recovery but a reload.
    Identity is derived from the corp name via `player_id_from_name`
    (crates/server/src/ws.rs:155), so re-sending Join with the SAME NAME
    resumes the SAME CORP. No server change needed.
    Implement: exponential backoff (0.5s -> 30s, jittered), remember the
    joined name, auto re-send Join on reopen, cancel backoff and reconnect
    eagerly on `visibilitychange` -> visible. Add a "reconnecting..." link
    state in the HUD, distinct from "disconnected", so a drop reads as
    transient.

PHASE 0 ACCEPTANCE
  - Desktop at 1920x1080 visually and behaviourally identical to before.
  - DevTools iPhone 14 emulation: map pans with one finger; tapping the market
    qty field does not zoom the page; bottom chrome is not under the URL bar;
    #checkin and the join card fit the viewport.
  - Kill the server for 20s with a client connected -> the client reconnects
    and resumes the same corp without a reload.
STOP AND REPORT.

---

# PHASE 1 — extract shared domain logic

Move the 106 DOM-free functions out of main.ts into:
  client/src/core/derive/{fleet,orders,market,research,captains,geo,format}.ts

Representative members (find the full set by checking which functions never
transitively reach `$("`, `document.`, `innerHTML`, `classList`, `.style.`,
`addEventListener`, `querySelector`):
  fleet:    jumpCapable, guardCapable, fleetCargoCapacity, estimatedFuelForLeg,
            fleetCommandLoad, coLocatedOwnFleet, systemFleetsAt,
            dockedFreighterStock, dockLoadStock, constructionStock
  orders:   intentSummary, intentTargetLabel, orderObject, orderEtaRange,
            syncOrderLifecycles, armedSelection, latestPendingOrder
  market:   recordPriceHistory, settleMarketReservation,
            recordRecentMarketOrder, moduleRecipeValue, poolUsage,
            bodyPoolUsage, ownedHaulDestinations, kitAffordable
  research: mergeResearch, researchQueueIds, nodeBonusDesc, projectedBand
  captains: captainXpFloor, captainTitle, traitLine, affinityLine
  geo:      gravityWellAt, systemUnderCursor, nearestKnownDock,
            foundingHomeSystemId, emplacementLabel
  format:   agoLabel, arrivalLocal, trend, fmtEta, countClassLabel helpers

Pure moves plus import rewiring. No signature changes, no behaviour change.

PHASE 1 ACCEPTANCE: build clean; desktop behaves identically.
STOP AND REPORT.

---

# PHASE 2 — the four seams

Highest design risk in the whole task. Commit each sub-phase separately.
The interfaces below are PINNED — later phases build against them, so do not
redesign them.

2a. core/session.ts — extract the message handler.
    The entire server-message switch currently lives INSIDE `join()`
    (main.ts:9316-9628, 339 lines) and interleaves three things: mutating
    `state`, firing DOM side-effects (toasts, auto-opening panels, chevrons),
    and scheduling the rAF refresh. Split:

      // core/session.ts — pure. Mutates state, returns what happened. No DOM.
      export function applyServerMessage(msg: ServerMsg, st: ViewState): CoreEvent[]

      // core/events.ts
      export type CoreEvent =
        | { kind: "LinkChanged"; status: LinkStatus }
        | { kind: "Welcomed"; playerId: PlayerId; name: string }
        | { kind: "JoinRejected"; message: string }
        | { kind: "ViewApplied" }
        | { kind: "ReportArrived"; ... }
        | { kind: "OrderConfirmed"; ... }
        | { kind: "CommandSignal"; ... }
        | { kind: "CommandChevron"; ... }
        | { kind: "BattleConcluded"; ... }
        | { kind: "EstimateReady"; ... }
        | { kind: "TradeSettled"; ... }
        | { kind: "ServerError"; message: string };

    Derive the exact variant payloads from the existing switch cases
    (Welcome, GalaxyUpdate, View, BattleRecords, GroundRecords, Sections,
    CommandSignal, CommandChevron, OrderConfirmed, Report, EngagementEstimate,
    Timeline, Trade, Error). One variant per presentational side-effect that is
    currently inlined. The desktop shell consumes the events and does exactly
    what the inline code did.

2b. core/mapclick.ts — extract map click resolution.
    `handleMapClick` (main.ts:4882) is 380 lines, the largest function in the
    file, and is almost entirely DECISION logic: given the armed mode (jump
    aiming / guard aiming / emplacement siting / raider-selected / plain
    selection), what does a click at (sx,sy) with modifiers mean? Only each
    branch's tail is DOM (writing `readout()`).

      export interface MapClickCtx {
        state: ViewState; renderer: Renderer;
        jumpAiming: EntityId | null; guardAiming: EntityId | null;
        emplaceArmed: EmplacementKind | null;
      }
      export type MapClickResult =
        | { kind: "intent"; intent: PendingIntent }
        | { kind: "reject"; reason: string }          // HTML string, current wording
        | { kind: "select"; target: SelectTarget }
        | { kind: "none" };

      export function resolveMapClick(sx: number, sy: number,
                                      mods: { shift: boolean; long: boolean },
                                      ctx: MapClickCtx): MapClickResult
      export function resolveSystemClick(sx: number, sy: number,
                                         ctx: MapClickCtx): MapClickResult

    CRITICAL: this code carries ORDER LEGALITY and LIGHT-DELAY GATING — jump
    range measured against the served sighting, gravity-well spool rules for
    both origin and destination, own-and-served-fleet checks for escort
    targets, raider-required checks for blockade. Preserve every guard AND ITS
    ORDER exactly. Keep reject wording byte-identical so the desktop readout
    does not change.
    Note `mods.long`: the mobile shell will map long-press onto the same
    modifier that Shift currently supplies (destroy vs raid, main.ts:5420).
    Desktop passes `long: false`.

    PAYOFF: this is what buys full parity cheaply — mobile inherits jump
    aiming, raid/blockade targeting, escort charging and emplacement siting
    with no duplicated logic.

2c. core/intent.ts — the pending-order state machine.
    Extract beginPendingIntent, confirmPendingIntent (109 lines),
    clearPendingIntent, armJumpAiming, armGuardAiming, clearGuardAiming,
    clearJumpAiming. Shell-agnostic by nature; DOM-tainted today only because
    each writes its own readout. Emit an `IntentChanged` CoreEvent. #intent-bar
    (desktop) subscribes; the mobile action sheet will subscribe in Phase 4.

2d. render.ts — camera viewport rect.
    fitScale() (render.ts:723) and recompute() (render.ts:989) fit and centre
    on the whole canvas; so do zoomByFactor (render.ts:743),
    systemEndpointCamera, and SystemScene.layout (systemview.ts:644). Both
    shells need them to fit the UNOCCLUDED map area instead — desktop for the
    right dock, mobile for the bottom sheet.
    Add a settable `cameraRect: {x,y,w,h}` defaulting to the full canvas and
    route all of the above through it. Desktop publishes the rect from the
    existing syncOverlayLayout machinery (main.ts:51-76), accounting for an
    open right dock. When the rect is the full canvas, behaviour must be
    bit-identical to today.

PHASE 2 ACCEPTANCE: build clean; every order type still issues correctly
(move, jump, raid, blockade, escort, emplacement) with identical readout text;
identical reject messages for out-of-range jump, in-well spool, in-well
destination, non-own escort target.
STOP AND REPORT. This phase warrants its own review pass before continuing.

---

# PHASE 3 — desktop shell extraction

3.1 Move the 7,764 DOM lines into `client/src/shell/desktop/`, split by panel
    family: rail.ts, market.ts, ship.ts, research.ts, operations.ts,
    syndicate.ts, faction.ts, checkin.ts, sysview.ts, battle.ts, founding.ts,
    mapchrome.ts.

3.2 Promote to `client/src/shell/dom.ts` — SHARED, both shells need these:
    setHtml, morphChildren, morphElement, nodeKey (the DOM-diffing mini
    framework) and the PRESS GUARD at main.ts:125-133. Read the press-guard
    comment before moving it; the bug it fixes (a 10 Hz re-render destroying
    the pressed node mid-click) is WORSE on touch, so mobile depends on it.

3.3 Move the 321 lines of body markup from index.html into
    `shell/desktop/markup.ts`, injected on mount.

3.4 Split the 1,452-line <style> block:
      styles/tokens.css   — the :root custom properties AND the Phase 0 touch
                            primitives. These are SHARED by both shells.
      styles/desktop.css  — everything else.

3.5 Add `client/src/shell/types.ts`:

      export interface Rect { x: number; y: number; w: number; h: number }
      export interface CoreContext {
        state: ViewState;
        net: Net;
        renderer: Renderer;
        intent: IntentMachine;
        send(msg: ClientMsg): void;
      }
      export interface Shell {
        mount(root: HTMLElement, ctx: CoreContext): Promise<void>;
        onCore(events: CoreEvent[]): void;   // presentational reaction
        onViewTick(): void;                  // coalesced rAF refresh
        cameraRect(): Rect;                  // what the map may use
        teardown(): void;                    // for the shell swap
      }

3.6 Add `client/src/boot.ts` as the single entry point. It owns the Net
    connection, the rAF loop and the Pixi Renderer — NOT the shells. It picks
    a shell via matchMedia and DYNAMICALLY IMPORTS it, so Vite code-splits and
    a phone never downloads the desktop shell. Only the desktop shell exists
    at this phase; the mobile branch may throw.
    Shell swap on breakpoint crossing must NOT tear down the Pixi app or the
    WebSocket — that is what makes desktop window-resize and the Phase 6
    rotate gate cheap.

PHASE 3 ACCEPTANCE: desktop at 1920x1080 is PIXEL-IDENTICAL and behaviourally
identical. Every panel opens, every order type issues, the battle viewer
replays, the market transacts. Verify the built bundle actually code-splits
the shell.
STOP AND REPORT. This is the regression window — 7,764 lines moving across
~13 files with no test suite.

---

# PHASE 4 — mobile shell, core loop

First phase with a user-visible deliverable. Build
`client/src/shell/mobile/` implementing the Shell interface, plus
`styles/mobile.css`. Portrait layout only (the rotate gate lands in Phase 6).

4.1 Chrome.
    - Top: a 3-stat status bar (credits, link/tick, contacts) with
      tap-to-expand for the other 5. This replaces the desktop HUD, whose 8
      stat items + 8 nav buttons total ~1,450px of intrinsic width and would
      wrap to ~6 rows at 390px — roughly 22% of a 740px viewport gone before
      anything is drawn.
    - Bottom: an 8-destination tab bar in thumb reach (the current .hud-nav
      destinations: Market, Fleets, Research, Officers, Operations, Syndicate,
      Faction, Log).
    - Map full-bleed behind both.

4.2 Panel stack — the core structural difference from desktop.
    Today openMarket / openOperations / openSyndicate / openResearch /
    openCheckin / openFaction (main.ts:2422, 2524, 2592, 2695, 2989, 9865)
    each do nothing but add `.is-open` to their own element — there is NO
    exclusivity. On a 390px phone that means eight full-width sheets stacked
    with no back navigation. Implement instead:

      pushSheet(id: SheetId, props?: unknown): void
      popSheet(): void
      replaceSheet(id: SheetId, props?: unknown): void

    One sheet visible at a time. Wire the back gesture and hardware back via
    `popstate` to popSheet. Sheets are BOTTOM SHEETS with a drag handle at two
    detents (half / full) so the map stays partly visible while an order is
    being composed — the map is the primary input surface for aiming.
    Publish `cameraRect()` from the current detent so the map re-centres into
    the unoccluded band (Phase 2d).

4.3 Map interaction.
    - Two-pointer PINCH driving the existing `renderer.zoomAt(cx, cy, factor)`
      and `adjustSystemScrub(delta)`. Note the entire semantic-zoom system
      (galaxy zoom, galaxy->system scrub, system->galaxy exit, the battle
      theater doorway) is currently wheel-only at main.ts:5433-5466 and has NO
      touch path. Pinch is a continuous scalar and maps onto the scrub better
      than wheel notches; SYSTEM_SCRUB_STEP=0.18 is the wheel quantum, do not
      reuse it for pinch.
    - LONG-PRESS = the Shift modifier (destroy vs raid). Pass `long: true` to
      resolveMapClick.
    - An armed-mode chip pinned over the map showing the active verb plus an
      explicit Cancel button — replaces `Esc`, which has no touch equivalent.
    - A confirm/cancel action sheet subscribing to IntentChanged, replacing
      #intent-bar and the Enter/Esc keys.
    - All map clicks route through core/mapclick.ts, so behaviour matches
      desktop by construction.

4.4 Core-loop surfaces (mobile forms): galaxy map + system view, fleet list,
    ship/fleet order panel, market (exchange + warehouse + specialists +
    modules), check-in / decision inbox, founding guide.

PHASE 4 ACCEPTANCE: on a real phone (not just emulation), join a game, pan and
pinch the map, enter and exit a system, select a fleet, issue a move order, a
jump order and a raid, buy and sell on the market, and answer a decision-inbox
item. Desktop unaffected.
STOP AND REPORT.

---

# PHASE 5 — mobile parity surfaces

Mobile forms for the remaining panel families: research programme boards,
operations, syndicate, faction, rankings, sysview management, planet panel,
build panels, hub panel.

Expect this to be the longest phase and to surface real information-design
questions — a 6-board research grid does not become a portrait sheet by
reflowing. Where a dense desktop board needs a different mobile information
architecture, propose it in the phase report rather than guessing.

PHASE 5 ACCEPTANCE: every desktop panel family has a working mobile form and
every action available on desktop is reachable on mobile.
STOP AND REPORT.

---

# PHASE 6 — portrait theaters, rotate gate, tooltip audit

6.1 Portrait battle theater. Full parity + portrait-only means this CANNOT
    fall back to "rotate to view the replay."
    The Pixi arena half is cheap: ARENA_R=1000 / VIEW_R=1500
    (battletheater.ts:40-47) describe a RADIALLY SYMMETRIC, centred arena with
    SCALE = CANVAS_H / (2 * VIEW_R). Make CANVAS_W/CANVAS_H configurable
    rather than module constants, and key SCALE off min(CANVAS_W, CANVAS_H) so
    the arena does not overflow the short axis. The camera math (ax/ay, zoom,
    pan) is already centre-relative and needs no change.
    The DOM chrome is the real work: `.bv-arena` is
    `grid-template-columns: 1fr 92px 1fr` (index.html:1381) — two facing sides
    with a central salvo gutter, which has no sensible form below ~300px.
    Portrait: rotate the metaphor 90 degrees. Sides become top/bottom, the
    salvo gutter becomes a horizontal band, `.bv-arrow` rotates,
    `.bv-side.right` drops its `direction: rtl` mirroring. Increase type size
    (bars and fit labels are 9px). Reflow `.bv-transport` and `.bv-scrub`.
    Add a touch equivalent for the theater's dblclick camera reset
    (battletheater.ts:347) — dblclick is unreliable on iOS.

6.2 Portrait ground theater (#ground-viewer, .gt-stage, .gt-scrub) — same
    treatment.

6.3 Rotate gate. Driven by `matchMedia("(orientation: portrait)")`. It must
    NOT tear down the Pixi app — re-init is expensive and loses camera state
    and loaded textures. It is an overlay over a still-running shell.
    Because landscape is gated, `visualViewport` + `orientationchange`
    handling is load-bearing: render.ts:563 currently listens on
    `window.resize` only, which fires with stale dimensions on iOS rotation.

6.4 Tooltip audit. There are 202 `title=` attributes (27 in index.html, 175 in
    main.ts) and 62 `:hover` rules. None of it reaches touch, and a meaningful
    subset is the ONLY place a number is explained — reserved-credit
    accounting on hud-credits, dev-slot rules on svm-slots, kit costs on
    emplace-btn. Classify each as decorative (drop on mobile) or informational
    (promote into the sheet body or a tap-to-reveal popover). List the
    informational ones in the phase report.

PHASE 6 ACCEPTANCE: a battle replay is legible and scrubbable on a 390px
portrait phone; rotating to landscape shows the gate and rotating back
restores the running session with camera state intact.
STOP AND REPORT.

---

# PHASE 7 — payload, perf, PWA

7.1 Asset derivatives. client/public/art is 56 MB. 32 MB of that is 85 captain
    PNGs at ~400 KB each, rendered at 64-82px (main.ts:1932 at 82px,
    main.ts:2013 at 64px) — a ~40x overdraw. `loading="lazy"` is explicitly
    disabled (see the comment at main.ts:481). The join screen background is a
    1.4 MB PNG (/art/lore_illustrations/corporate_command_center.png,
    index.html:61) — the first thing a mobile player downloads.
    Add a build-time derivative pipeline: 96px + 192px WebP/AVIF captain
    thumbs, 2-size WebP for lore illustrations, `srcset` on both, re-enable
    lazy loading for the officer roster. Target: low single-digit MB.

7.2 Mobile perf mode: 30 Hz render loop; pause the galaxy render entirely when
    a full-screen sheet covers it. NOTE: the July 2026 perf audit explicitly
    SKIPPED "pause the galaxy under the battle viewer" because the desktop
    viewer is a centred modal with the map visible around it. On mobile it IS
    full-screen, so that reasoning no longer applies and the optimisation
    becomes correct.

7.3 Consider a per-connection View cadence so mobile clients receive ~5 Hz
    instead of 10 Hz. The quiet-state View is ~40 KB of JSON at 10 Hz
    (~400 KB/s of parse and allocation on a phone). THIS IS THE ONLY PHASE
    THAT MAY TOUCH THE RUST SERVER — propose the change in the phase report
    and get sign-off before implementing it.

7.4 PWA: manifest.json with `display: standalone`, app icons, theme-color,
    apple-mobile-web-app-* meta. Standalone display removes the URL bar, which
    independently resolves most of the dynamic-viewport pain from Phase 0.

PHASE 7 ACCEPTANCE: first-load transfer on a cold cache is under 5 MB;
sustained 30 fps on a mid-tier Android during normal play; installable to home
screen.
STOP AND REPORT.
