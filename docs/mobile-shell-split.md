# Splitting the client into a shared core + desktop shell + mobile shell

Decisions taken: **mobile is full parity** (every desktop panel gets a mobile form,
including the research boards and the battle/ground theaters), and **mobile is
portrait-only** with a rotate-to-portrait gate in landscape.

The current UX becomes the **desktop shell**, moved essentially verbatim. The work is
not rewriting it — it's carving out the seams it currently sits on top of, so a second
shell can sit on the same ones.

## 1. What the codebase is made of

Measured on `codex/panel-ux-diet` @ db40d9f.

| | lines | disposition |
|---|---:|---|
| `protocol.ts`, `render.ts`, `battletheater.ts`, `systemview.ts`, `groundtheater.ts`, `state.ts`, `icons.ts`, `stars.ts`, `net.ts`, `prng.ts` | **8,343** | already shared — move under `core/` |
| `main.ts` — DOM-tainted (transitive) | **7,764** (191 fns) | becomes the desktop shell |
| `main.ts` — DOM-free logic | **1,535** (106 fns) | **extract to `core/` — the enabling work** |
| `main.ts` — DOM-free HTML emitters | 658 (36 fns) | desktop shell |
| `index.html` `<style>` | 1,452 | split: tokens / desktop / mobile |
| `index.html` body markup | 321 | desktop shell markup |

**main.ts is ~78% desktop shell.** Fine — a mobile UX shouldn't reuse desktop panel
markup. The problem is the other 22%: ~1,500 lines of shared domain logic only
reachable by being inside `main.ts`, plus four seams that don't exist yet.

## 2. Target structure

```
client/src/
  core/
    net.ts protocol.ts state.ts        (moved; net gains reconnect)
    session.ts   NEW — Net wiring + applyServerMessage()
    events.ts    NEW — CoreEvent union
    intent.ts    NEW — order-issuing state machine
    mapclick.ts  NEW — resolveMapClick() / resolveSystemClick()
    derive/      NEW — the 106 extracted pure-logic fns
      fleet.ts orders.ts market.ts research.ts captains.ts geo.ts format.ts
    scene/
      render.ts systemview.ts battletheater.ts groundtheater.ts
      stars.ts prng.ts icons.ts
  shell/
    types.ts     NEW — the Shell interface
    dom.ts       NEW — setHtml/morph*/nodeKey + press-guard (shared)
    desktop/     today's main.ts DOM code + markup + CSS
    mobile/      new
  boot.ts        NEW — picks a shell, dynamically imports it
  styles/ tokens.css desktop.css mobile.css
```

One `index.html`, one entry (`boot.ts`), **shell chosen at runtime and dynamically
imported** so Vite code-splits it — a phone never downloads the desktop shell. Chosen
over separate `.html` entries because the desktop shell must survive a window resize
to phone width without a reload, and it keeps one deploy and one URL.

## 3. The four seams

### 3.1 `core/session.ts` — extract the message handler
The server-message switch lives **inside `join()`** (`main.ts:9316-9628`, 339 lines)
and interleaves state mutation, DOM side-effects (toasts, auto-opening panels,
chevrons), and the rAF refresh. Split into:
```ts
export function applyServerMessage(msg: ServerMsg, st: ViewState): CoreEvent[]
```
`CoreEvent` covers `ReportArrived`, `OrderConfirmed`, `CommandChevron`,
`BattleConcluded`, `EstimateReady`, `JoinRejected`, `LinkChanged`. Each shell decides
presentation. Without this there is no second shell.

### 3.2 `core/mapclick.ts` — extract map click resolution
`handleMapClick` is **380 lines** (`main.ts:4882`), the largest function in the file,
and is almost entirely *decision* logic: given the armed mode (jump aiming / guard
aiming / emplacement siting / raider-selected / plain selection), what does a click at
(sx,sy) with modifiers mean? Only each branch's tail is DOM (`readout()`).
```ts
export function resolveMapClick(sx, sy, mods, ctx): MapClickResult
// { kind:"intent", intent } | { kind:"reject", reason } | { kind:"select", target } | { kind:"none" }
```
Payoff: **mobile gets jump aiming, raid/blockade targeting, escort charging and
emplacement siting for free** — the hard part of parity. Same for `handleSystemClick`.

### 3.3 `core/intent.ts` — pending-order state machine
`beginPendingIntent` / `confirmPendingIntent` (109 lines) / `clearPendingIntent` /
`armJumpAiming` / `armGuardAiming` / `clearGuardAiming`. Shell-agnostic by nature,
DOM-tainted only because each writes its own readout. Extract, emit `IntentChanged`;
`#intent-bar` (desktop) and a bottom action sheet (mobile) both subscribe.

### 3.4 `render.ts` — camera viewport rect
`fitScale()` (`render.ts:723`) and `recompute()` (`render.ts:989`) fit/centre on the
whole canvas. Both shells need the **unoccluded** map area: desktop for the right
dock, mobile for the bottom sheet. Add settable `cameraRect` defaulting to full
canvas; route `fitScale`, `recompute`, `zoomByFactor`, `systemEndpointCamera`,
`SystemScene.layout` through it. Shells publish it from the existing
`syncOverlayLayout` machinery (`main.ts:51-76`).

## 4. The Shell interface

```ts
export interface Shell {
  mount(root: HTMLElement, ctx: CoreContext): Promise<void>;
  onCore(events: CoreEvent[]): void;
  onViewTick(): void;
  cameraRect(): Rect;
  teardown(): void;
}
```
`boot.ts` owns the Net connection and the rAF loop; shells only present. It watches
`matchMedia`; crossing the breakpoint tears down one shell and mounts the other
**without touching the Pixi app or the WebSocket** — the renderer is core, not shell.

## 5. Desktop shell — what changes

Close to nothing, which is the point.
- Move 7,764 DOM lines into `shell/desktop/` split by panel family (`rail.ts`,
  `market.ts`, `ship.ts`, `research.ts`, `operations.ts`, `syndicate.ts`,
  `faction.ts`, `checkin.ts`, `sysview.ts`, `battle.ts`, `founding.ts`,
  `mapchrome.ts`). Mechanical, but ~13 files of untangling — where regressions hide.
- Rewire ~1,500 lines of call sites to `core/derive/*`.
- Body markup (321 lines) → `desktop/markup.ts`; `<style>` → `tokens.css` +
  `desktop.css`.
- `setHtml`/`morphChildren`/`morphElement`/`nodeKey` and the press-guard
  (`main.ts:125-133`) are **shared shell infrastructure** → `shell/dom.ts`. Mobile
  needs both; the press-guard bug is worse on touch.

Touch primitives (`touch-action`, `dvh`, safe-area, 16px form controls) go in
`tokens.css`, not mobile-only — touchscreen laptops hit the same `touch-action` bug.

## 6. Mobile shell

**Chrome.** Top: 3-stat status bar (credits, link/tick, contacts), tap-to-expand for
the other 5 — replaces the 8-item + 8-button HUD that would wrap to ~6 rows and eat
~22% of a 740px viewport. Bottom: 8-destination tab bar in thumb reach. Map full-bleed
behind both.

**Panel stack, not satellites.** Today `openMarket`/`openOperations`/`openSyndicate`/
`openResearch`/`openCheckin`/`openFaction` (`main.ts:2422, 2524, 2592, 2695, 2989,
9865`) each just add `.is-open` with **no exclusivity** — eight full-width sheets would
stack with no back navigation. Mobile gets `pushSheet`/`popSheet`, one at a time,
back-gesture and `popstate` wired to pop. Bottom sheets with a drag handle at two
detents (half/full) so the map stays partly visible while composing an order — the map
is the primary input surface for aiming.

**Map interaction.** Two-pointer pinch driving `renderer.zoomAt` and
`adjustSystemScrub` (pinch is continuous and maps onto the semantic scrub better than
wheel notches). Long-press = the Shift modifier (destroy vs raid, `main.ts:5420`). An
armed-mode chip over the map with explicit Cancel, replacing `Esc`. All routed through
`core/mapclick.ts`, so behaviour matches desktop by construction.

**Tooltips.** 202 total (27 `index.html`, 175 `main.ts`). Audit decorative (drop) vs
informational (promote into sheet body or tap-to-reveal). A real subset is the only
place a number is explained — reserved credits on `hud-credits`, dev-slot rules on
`svm-slots`, kit costs on `emplace-btn`.

## 7. Portrait battle theater

Parity + portrait-only means this can't be punted to "rotate to view."

**The Pixi arena is nearly free.** `ARENA_R = 1000` / `VIEW_R = 1500`
(`battletheater.ts:40-47`) describe a **radially symmetric**, centred arena with
`SCALE = CANVAS_H / (2 * VIEW_R)`. Portrait needs: make `CANVAS_W`/`CANVAS_H`
configurable rather than module constants, and key `SCALE` off
`min(CANVAS_W, CANVAS_H)` so the arena doesn't overflow the short axis. Camera math
(`ax`/`ay`, zoom, pan) is already centre-relative — no change.

**The DOM chrome is the work.** `.bv-arena` is `grid-template-columns: 1fr 92px 1fr`
(`index.html:1381`) — two facing sides with a central salvo gutter, no sensible form
below ~300px. Portrait: rotate the metaphor 90°, sides become top/bottom, gutter
becomes a horizontal band, `.bv-arrow` rotates, `.bv-side.right` drops `direction:
rtl`. Larger type (bars and fit labels are 9px), `.bv-transport`/`.bv-scrub` reflowed.
`#ground-viewer` likewise. Add a touch equivalent for the theater's `dblclick` camera
reset (`battletheater.ts:347`).

## 8. Rotate gate

- Must **not tear down the Pixi app** — re-init is expensive and loses camera state and
  textures. An overlay over a still-running shell, driven by
  `matchMedia("(orientation: portrait)")`.
- `visualViewport` + `orientationchange` handling becomes load-bearing:
  `render.ts:563` listens on `window.resize` only, which fires with stale dimensions
  on iOS rotation.

Gating landscape means players who rotate for a bigger map get a blocking overlay. If
that tests badly, landscape-as-a-second-layout is additive on this structure, not a
rework — the shell interface and camera rect already support it.

## 9. Sequencing

- **S0 — touch primitives + reconnect.** `touch-action`, `dvh`/`svh` +
  `visualViewport`, `env(safe-area-inset-*)`, 16px form controls, viewport meta, dPR
  cap, the two overflowing fixed widths (`#checkin` 468px, `#join .card` 360px). Plus
  client auto-reconnect (`net.ts` has none; server tears down at `READ_TIMEOUT = 60s`;
  `ws.rs:155` already resumes by name, so client-only). Ships against today's single
  shell. No architecture work.
- **S1 — extract `core/derive/*`.** 106 fns, ~1,535 lines. Pure moves + import
  rewiring.
- **S2 — the four seams** (§3). Where the design risk is: `handleMapClick` (380 lines)
  and the 339-line message switch are dense and load-bearing for correctness
  (light-delay gating, order legality). Worth its own `/code-review`.
- **S3 — desktop shell extraction.** 7,764 DOM lines into `shell/desktop/*`, CSS
  split, `Shell` interface, `boot.ts`. Desktop must be pixel-identical.
- **S4 — mobile shell, core loop.** Chrome, panel stack, sheets, pinch/gesture, map +
  fleets + ship orders + market + check-in + founding. First playable phone build.
- **S5 — mobile parity surfaces.** Research, operations, syndicate, faction,
  rankings, sysview management.
- **S6 — portrait theaters** (§7) + tooltip audit.
- **S7 — PWA manifest, mobile perf mode** (30 Hz; pause the galaxy render under a
  full-screen sheet — the perf audit skipped that because the desktop viewer is a
  centred modal, but on mobile it *is* full-screen), reduced View cadence, and the
  asset derivative pipeline (56 MB of art, 32 MB of it captain PNGs at ~400 KB drawn
  at 64–82 px — see `mobile-web-analysis.md` §4).

S0 is independently shippable today. S1–S3 are refactors with a hard "desktop is
unchanged" acceptance test and **no user-visible deliverable** — three phases of no
visible progress before S4 shows anything on a phone.

## 10. Risks

- **S3 is the regression window.** 7,764 lines moving across ~13 files with no client
  test suite. Move verbatim, no cleanups in the same commits, lean on the `__ss` rig.
- **`handleMapClick` extraction (S2)** touches order legality and light-delay gating.
  Errors produce illegal orders the sim rejects — visible but confusing.
- **Full parity on a phone is a lot of surface.** 13 panel families; research boards
  and operations are dense. Expect S5 to be longest and to surface real
  information-design questions (a 6-board research grid doesn't become a portrait
  sheet by reflowing).
- **No client test harness.** Before S3, a device matrix and a smoke script that
  joins, issues each order type, and opens every panel would pay for itself.
