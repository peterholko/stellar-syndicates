# Mobile web (portrait + landscape) — what it would take

Analysis against `codex/panel-ux-diet` @ db40d9f. Client is a single Pixi 8 canvas
(`client/src/render.ts`) under ~16 independently-positioned fixed DOM layers, all
styled in one 1781-line `<style>` block in `client/index.html`, driven by
`client/src/main.ts` (9,985 lines).

Verdict: the input model, the layout model, the asset pipeline, and the session
model each need work. None of it is exotic, but the layout tier is a genuine
refactor rather than a media-query pass — the current model is "panels are
free-floating fixed satellites, any number co-open," which does not compress.

---

## 0. What already helps

Worth naming, because it changes the cost of everything below:

- **Pointer Events, not mouse events.** `main.ts:5380-5425`, `battletheater.ts:339-354`.
  Tap and one-finger drag already produce the right event stream.
- **`--hud-safe-top` is measured, not guessed.** `main.ts:40-49` publishes the HUD's
  real bottom edge via `ResizeObserver`; every fixed panel anchors to it. When the
  HUD wraps to 6 rows on a phone, the panels already move. This is the single most
  valuable thing in the codebase for this project.
- **Body-class panel arbitration exists.** `main.ts:51-76` maintains
  `is-right-dock-open` / `is-focus-overlay-open` / `is-system-view` /
  `is-planet-panel-open`, and `index.html:1432-1457` uses them to hide conflicting
  map chrome. The hook for a mobile layout mode is already there.
- **Several panels are already fluid** — `min(480px, calc(100vw - 28px))`,
  `min(760px, 94vw)`, `min(470px, calc(100vw - 24px))`.
- **Pixi is DPI-aware and self-resizing** — `resizeTo: window`, `autoDensity: true`,
  `resolution: devicePixelRatio` (`render.ts:508-514`).
- **Perf phases 1–3 have landed** (protocol deltas, Pixi pooling, panel signature
  guards). The mobile frame/network budget starts from the post-audit baseline, not
  the pre-audit one.
- **Server-side session resume already works.** `ws.rs:155` derives the player id via
  `player_id_from_name`, so re-joining with the same name resumes the same corp.
  Auto-reconnect is therefore a client-only change.

---

## 1. Hard blockers — touch input

These are not polish. Each one breaks a core interaction outright.

### 1.1 No `touch-action` anywhere → the map cannot be panned
Zero occurrences of `touch-action` in `index.html`. With the default
`touch-action: auto`, the browser claims a one-finger drag on the canvas as a
scroll/pan-zoom gesture and fires `pointercancel` partway through. The pan handler
at `main.ts:5397-5411` stops receiving `pointermove`, and `main.ts:5425` resets
`down = false`. Net effect on a phone: the map jitters and stops.

Fix: `touch-action: none` on the map canvas and the battle-theater canvas;
`touch-action: pan-y` on scrollable panel bodies; `overscroll-behavior: contain` on
every `overflow-y: auto` region so panel scrolls don't chain to the document.

### 1.2 The entire semantic-zoom system is wheel-only
`main.ts:5433-5466` is the only zoom path, and it is `wheel`. There is no touch
equivalent for any of it:

- galaxy zoom (`renderer.zoomAt`)
- galaxy → system handoff (`beginSystemScrubIn` + `adjustSystemScrub` at max zoom)
- system → galaxy exit (`beginSystemScrubOut`, deliberately a zoom-*out* gesture)
- galaxy → battle theater (`atBattleZoomThreshold` + `battlePick`)
- the scrub itself (`SYSTEM_SCRUB_STEP = 0.18` per wheel notch)

`dblclick` (`main.ts:5471-5486`) covers *entering* system/battle but not the scrub,
not exiting, and not free zoom — and `dblclick` on touch is unreliable (iOS fires it
inconsistently and pairs it with double-tap-to-zoom).

Fix: a two-pointer pinch handler that produces the same `zoomAt(cx, cy, factor)` and
`adjustSystemScrub(delta)` calls the wheel handler produces. Pinch is a continuous
scalar, which actually maps *better* onto the scrub than wheel notches do. Plus a
long-press or explicit back affordance for exit, since "zoom out to leave" has no
discoverable touch form.

### 1.3 Viewport meta permits document pinch-zoom
`index.html:5` — `width=device-width, initial-scale=1.0`, no `maximum-scale`, no
`user-scalable`, no `viewport-fit=cover`. The player's pinch zooms the *page*, not
the map. Needs `maximum-scale=1, user-scalable=no, viewport-fit=cover`
(accessibility tradeoff is real but standard for canvas games; the in-game zoom is
the substitute).

### 1.4 iOS auto-zooms on every text field
All inputs and selects are 10–12px (`index.html:261, 269, 431, 836, 1124, 1167, 1194`).
iOS Safari zooms the viewport whenever a focused field is under 16px, and does not
zoom back out. Focus the market qty field once and the layout is wrong for the rest
of the session. Needs `font-size: 16px` on all form controls at mobile widths.

### 1.5 `100vh` is wrong on mobile
15 uses of `100vh` / `calc(100vh - …)`. Under a collapsing URL bar, `100vh` is the
*largest* viewport height, so bottom-anchored chrome sits under the browser UI. Every
bottom-docked element is affected: `#founding-guide` (`bottom:14px`), `#readout`,
`#intent-bar`, `#legend`, `#reports-log`. Needs `dvh`/`svh`, plus a
`visualViewport` resize/scroll listener — `window.resize` alone does not fire
reliably for URL-bar collapse on iOS.

### 1.6 No safe-area insets
No `env(safe-area-inset-*)`. In landscape on a notched iPhone the notch eats the left
edge where `#zoom-controls` lives (`left:14px`, `index.html:862`) and the home
indicator crosses the bottom-docked chrome.

### 1.7 Modifier-key and keyboard-only verbs have no touch path
- **Shift+tap on the galaxy map = destroy instead of raid** (`main.ts:5420`,
  `handleMapClick(..., e.shiftKey)`). A gameplay verb with no touch equivalent.
- ~20 keyboard shortcuts at `main.ts:5498+`: `J` arm jump, `R` recall raid,
  `M/R/S/V/P/O/U/F/G` panel toggles, `Enter` confirm pending intent, `Esc` cancel.
  `#intent-bar` gives Enter/Esc a button equivalent; jump-arm and recall are
  reachable from the ship panel; but the arming/aiming flows
  (`armJumpAiming`, `guardAiming`, `emplace-btn`) all end in "now tap the map,"
  and `Esc` is the only advertised cancel.

Fix: a mode chip on the map showing the armed verb with an explicit Cancel, and a
long-press or a two-option action sheet for raid-vs-destroy.

### 1.8 Hover carries real information
62 `:hover` rules and **202 `title=` attributes** (27 in `index.html`, 175 in
`main.ts`). A meaningful subset is the *only* place a number is explained — e.g.
`hud-credits`'s title (`main.ts:~95`) is where "~" reserved credits are accounted
for; `svm-slots`' title is where dev-slot rules live; `emplace-btn` titles carry the
kit cost. None of it reaches touch.

Also `renderer.cursorWorld` (`main.ts:5380-5386`): a touch drag sets it, and
`pointerleave` may never fire, so a stale hover highlight can persist after the
finger lifts.

Fix: audit the 202 titles into (a) decorative — drop on mobile, (b) informational —
promote into the panel body or a tap-to-reveal popover. Clear `cursorWorld` on
`pointerup` when `pointerType !== "mouse"`.

---

## 2. Layout — the real design work

### 2.1 The panel model does not compress
There is **no exclusivity** between the eight nav overlays. `openMarket`,
`openOperations`, `openSyndicate`, `openResearch`, `openCheckin`, `openFaction`
(`main.ts:2422, 2524, 2592, 2695, 2989, 9865`) each do nothing but add `.is-open` to
their own element. On desktop they overlap at different fixed positions and it's
survivable. On a 390px phone, eight full-width sheets stack on top of each other with
no back navigation.

Add the right rail (`#rail`), the ship panel, `#planet-panel`, the two `.build-shell`
panels, `#battle-viewer`, `#ground-viewer`, `#sysview-manage`, `#breadcrumb`,
`#zoom-controls`, `#legend`, `#readout`, `#intent-bar`, `#founding-guide`,
`#reports-log` — ~16 independently positioned fixed layers.

**This is the main refactor.** Mobile needs a panel *stack*: `pushPanel(id)` /
`popPanel()`, one visible at a time, hardware/gesture back pops. Every `openX` call
site routes through it. Desktop keeps today's behaviour behind a layout-mode flag.
The existing `LAYOUT_WATCH_IDS` / `syncOverlayLayout` machinery (`main.ts:51-76`) is
the natural place to host it.

### 2.2 The HUD eats the screen
`#hud` (`index.html:91-103`) is `flex-wrap: wrap` with 8 stat items + 8 nav buttons +
a link indicator. Intrinsic width is roughly 1400–1500px. At 390px portrait that
wraps to ~6 rows — `--hud-safe-top` lands around 150–170px, i.e. ~22% of a 740px
viewport consumed before anything is drawn. In landscape (844×390) it's 2–3 rows,
but height is the scarce axis there, so it's proportionally worse.

The measured-safe-top design means nothing *breaks* — it just leaves very little map.

Fix: at mobile widths collapse the HUD to 3 stats (credits, link/tick, contacts) with
the rest behind a tap-to-expand; move the 8 nav buttons to a **bottom tab bar** in
portrait (thumb reach) and keep them top in landscape.

### 2.3 Two panels overflow outright
- `#checkin` — `width: 468px` fixed (`index.html:276`). Clipped and partly
  unreachable at 390px, since `body { overflow: hidden }`.
- `#join .card` — `width: 360px` fixed (`index.html:70`). Fits a 390px phone with
  15px to spare; fails on a 360px device.
- `#battle-panel`, `#hub-panel`, `#syndicate-panel`, `#faction-panel` — `width: 320px`
  fixed. They fit, but as a fixed 320px column against the left edge they read as
  broken rather than designed.

### 2.4 Existing responsive coverage is incidental
Six media queries total, and five of them adjust *content* grids, not layout:
`max-width:1100` (intent bar), `720` (market grid columns), `1279` (build shell +
planet card), `560` (build shell columns), `620` (research boards), `max-height:560`
(planet panel). There is no phone breakpoint and no orientation query anywhere.

### 2.5 Landscape is the cheaper target
Landscape (844×390 / 926×428) is structurally the desktop layout at reduced scale:
map full-bleed, one right dock at `min(360px, 45vw)`, HUD collapsed to one row,
bottom chrome moved to the safe-area inset. Most of §1 still applies, but §2.1's
stack refactor can be softened to "at most one dock + one overlay."

Portrait (390×740) is the one that needs the new layout: map as a fixed top region or
full-bleed background, panels as bottom sheets with a drag handle, nav as a bottom
tab bar.

---

## 3. Camera and rendering

### 3.1 The camera fits to the canvas, not to the visible map
`fitScale()` (`render.ts:723`) is `min(viewW, viewH) * 0.46 / galaxy.radius` and
`recompute()` (`render.ts:989`) centres on `viewW/2, viewH/2`. Same pattern in
`SystemScene.layout` (`systemview.ts:644-653`, `min * 0.42`, centred) and
`zoomByFactor` (`render.ts:743`).

In portrait that's merely wasteful — the galaxy occupies 359px of a 740px column with
~190px dead above and below. With a bottom sheet covering the lower half, it's wrong:
the galaxy centre and the selected system sit *behind* the sheet.

Fix: introduce a **camera viewport rect** (the map area not occluded by chrome) and
fit/centre against it. `syncOverlayLayout` already knows which panels are open, so it
can publish the rect. Touches `fitScale`, `recompute`, `zoomByFactor`,
`systemEndpointCamera`, and `SystemScene.layout`.

### 3.2 Resize handling is incomplete for mobile
`render.ts:563` listens on `window.resize` only. Pixi's `resizeTo: window` handles the
backbuffer, but orientation changes on iOS fire `resize` with stale dimensions, and
URL-bar collapse often doesn't fire it at all. Needs `visualViewport` resize/scroll
listeners, an `orientationchange` handler, and a debounce — plus a re-run of
`syncHudSafeTop` after the settle.

### 3.3 Uncapped device pixel ratio
`resolution: window.devicePixelRatio || 1` in both `render.ts:513` and
`battletheater.ts:300`. On a dPR-3 phone a 390×740 viewport becomes a 1170×2220
backbuffer — 2.6 Mpx at 60 Hz on mobile silicon. Cap at 2 (or 1.5 on low-end).

### 3.4 The battle theater assumes a wide desktop canvas
`CANVAS_W = 1068`, `CANVAS_H = 672` fixed (`battletheater.ts:43-44`), CSS-scaled by
`max-width: 100%`. At 390px portrait the arena renders at 390×245 with 9px health bars
and 9px fit labels. The surrounding `.bv-arena` grid is
`1fr 92px 1fr` (`index.html:1381`) — a two-sides-plus-gutter layout that has no
sensible form below ~300px. `#ground-viewer` (940px) has the same problem.

This needs a decision, not a tweak: a portrait arena variant (vertical stacking,
larger type, fewer simultaneous readouts), or an explicit "rotate to view the
replay" gate. The theater's own `dblclick`-to-reset-camera also needs a touch
equivalent.

### 3.5 The Phase-2 "don't pause the galaxy" decision reverses on mobile
The perf audit explicitly *skipped* pausing the galaxy render under the battle viewer
because the viewer is a centred modal with the map visible around it. On mobile it
becomes a full-screen sheet — so pausing (and pausing under any full-screen sheet)
becomes both correct and valuable.

---

## 4. Asset payload — the cellular killer

`client/public/art` is **56 MB**.

| Bucket | Size | Note |
|---|---|---|
| `captains/` | **32 MB** | 85 PNGs, ~400 KB each, rendered at **64–82 px** |
| `lore_illustrations/` | 8.4 MB | 5 files, ~1.3–1.5 MB each |
| `celestial_sprites/` | 5.5 MB | |
| `ship_sprites/` | 4.2 MB | |
| `ui_icons/` | 1.5 MB | |
| `wormhole_hub.png` | 1.9 MB | |
| `stellar_syndicates_logo.png` | 1.1 MB | |

Worst specifics:

- **The join screen background is a 1.4 MB PNG** —
  `/art/lore_illustrations/corporate_command_center.png` (`index.html:61`). It is the
  first thing a mobile player downloads, before any gameplay.
- **Captain portraits are ~400 KB PNGs drawn at 64–82 px**
  (`main.ts:1932` at 82px, `main.ts:2013` at 64px). That's a ~40× overdraw. Opening
  the officer roster pulls a multi-megabyte burst.
- **`loading="lazy"` is explicitly disabled** — see the comment at `main.ts:481`.
- No `srcset`, no WebP/AVIF, no thumbnail derivatives anywhere.

Fix (largest single win available, and it's build-time only):
a derivative pipeline emitting 96px + 192px WebP/AVIF captain thumbs (32 MB → ~1 MB),
2-size WebP for lore, `srcset` on both, and re-enabling lazy loading for the roster.
The JS bundle itself is 732 KB (mostly Pixi) — acceptable, though ~2s parse on a
mid-tier Android.

---

## 5. Runtime budget on mobile

- **The View stream is ~40 KB of JSON at 10 Hz in the quiet state** (post-Phase-3
  measurement, from the perf audit). That's ~400 KB/s of parse + allocation on a
  phone, plus whatever panel re-render survives the signature guards. Tuned for a
  desktop budget.
- Levers, cheapest first: cap dPR (§3.3); drop the render loop to 30 Hz when a mobile
  layout mode is active; pause the galaxy render under a full-screen sheet (§3.5);
  coalesce View-driven panel refreshes harder than the existing single-rAF
  `scheduleViewRefresh` (`main.ts:8993`); and — server-side, more invasive — a
  per-connection View cadence so mobile clients get 5 Hz.
- Thermal/battery: a 60 Hz always-animating starfield + rival "breath" pulses
  (`systemsAnimating`, `render.ts`) means the GPU never idles. A mobile build should
  idle-skip when nothing is animating and the player isn't interacting.

---

## 6. Session survival — currently a hard blocker

`net.ts` has **no reconnect logic at all**. `connect()` is called once
(`main.ts:9630`); `onClose` (`main.ts:9618`) sets `state.link = "offline"` and
re-enables the join button. That's it.

Server-side, `ws.rs:37` sets `READ_TIMEOUT = 60s` with `PING_INTERVAL = 20s`. A
backgrounded mobile tab stops answering pings, so the connection is torn down after
60 seconds. On mobile, locking the screen or switching apps for a minute ends the
session and the only recovery is reload-and-rejoin.

The good news: identity is `player_id_from_name` (`ws.rs:155`), so re-joining with the
same name resumes the same corp. **Auto-reconnect is a client-only change**:
exponential backoff, remember the joined name, re-send `Join` on reopen, and a
`visibilitychange` handler that reconnects eagerly on foreground. Add a visible
"reconnecting…" state so a dropped link reads as transient.

---

## 7. Delivery

No `manifest.json`, no service worker, no `apple-mobile-web-app-capable`, no
`theme-color`, no app icons. A minimal PWA manifest + `display: standalone` removes
the URL bar entirely, which independently resolves most of §1.5's `100vh` pain and
gives an install path. Cheap relative to its value.

`resolveServerUrl` (`net.ts:17-26`) already picks `wss` under `https`, so secure
contexts are handled — but mobile testing needs a real HTTPS origin, since an
`https` page cannot open a `ws://` socket.

---

## 8. Suggested sequencing

**Phase M0 — unblock touch (small, high value, low risk).**
`touch-action` + `overscroll-behavior`; viewport meta; 16px form controls;
`dvh`/`svh` + `visualViewport` listener; `env(safe-area-inset-*)`; cap dPR;
fix `#checkin` / `#join .card` fixed widths; clear stale `cursorWorld` on touch.
Outcome: the game is *reachable* on a phone. Not yet good.

**Phase M1 — client auto-reconnect.** Independent of everything else, and without it
no amount of layout work makes mobile playable. Server needs no change.

**Phase M2 — pinch/gesture layer.** Two-pointer pinch driving the existing
`zoomAt` / `adjustSystemScrub` calls; long-press for the shift-modifier verb; explicit
Cancel affordance for armed aiming modes; touch equivalents for the theater's
`dblclick` reset.

**Phase M3 — asset derivatives.** Build-time thumbnails + WebP/AVIF + `srcset` +
lazy roster loading. Pure build work, no UI risk, and the single biggest
first-load win (56 MB → low single-digit MB).

**Phase M4 — the layout refactor.** Panel stack with exclusivity and back-pop; a
mobile layout mode flag; HUD collapse; bottom tab bar in portrait; landscape as
"desktop, one dock." This is the large one.

**Phase M5 — camera viewport rect.** Fit/centre against the unoccluded map area
rather than the canvas. Depends on M4 publishing the rect.

**Phase M6 — battle/ground theater portrait treatment.** Needs a design decision
first (portrait arena vs. rotate gate).

**Phase M7 — PWA manifest + mobile perf mode** (30 Hz, pause under sheet, reduced
View cadence).

M0–M3 are mechanical and independently shippable. M4 is where the actual product
design work lives, and it should not start before someone decides what portrait
Stellar Syndicates *is* — a full client, or a companion/check-in view of a
desktop-primary game. That decision changes the size of M4 by an order of magnitude.

---

## 9. Not covered / needs verification

- No device testing was performed; every claim above is static analysis of the
  source. The HUD row-count and `--hud-safe-top` figures in §2.2 are estimates from
  intrinsic content width, not measurements.
- No responsive test harness exists in the repo. A device matrix
  (iPhone SE / 15 / Pixel, portrait + landscape) should be established before M4.
- Accessibility beyond touch targets (screen readers, reduced motion, contrast at
  9–10px type) was not examined. Note that font sizes skew very small: 73 rules at
  10px, 46 at 11px, 36 at 9px, 2 at 8px.
