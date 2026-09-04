# Codex fixes prompt — the Deck, round 1 (single task, one phase)

Repo: stellar-syndicates. Branch: `codex/desktop-deck` (@ f63645b).
Context: docs/desktop-rebuild-prompt.md is the spec these findings are judged
against. Findings come from a live post-D6 playthrough (1920×1080 and
1366×768, corp "Vantage Audit") plus a full code audit, 2026-08-29.

The rebuild is structurally sound — router, Esc chain, hub-move, founding
deep links, teardown, token hygiene, and sender parity all verified good.
This prompt is the punch list. Work top to bottom; commit per numbered fix;
`npm --prefix client run build` clean after every commit; STOP AND REPORT
at the end with a live re-verification of each ✔ criterion.

The root cause behind most of the geometry items: `--deck-workspace-inset`
(published by `workspace.ts:58`) is consumed ONLY by `.deck-zoom`
(`deck.css:442`). The strip and toast lane ignore it, and the bottom band
(strip / founding chip / zoom) has no coordination rule. Fix the band as a
system, not as three patches.

## P0 — interaction breakers

F1. **Command strip overlaps the workspace** (`deck.css:349`).
    `left:50%; translateX(-50%)` centers on the viewport. With the wide
    workspace it overlaps at every viewport below ~1908px (80% of the strip
    at 1440); with the standard workspace below ~1640px. Measured live:
    640×161px over the market at 1920, 137×147px over the fleet panel at
    1366 — the strip floats mid-panel and its right cells cover the
    workspace's own controls.
    Fix: center on the unoccluded map rect —
    `left: calc((100vw - var(--deck-workspace-inset, 0px)) / 2)` — and clamp
    `width` to `min(720px, calc(100vw - var(--deck-workspace-inset) - 24px))`
    so it never crosses the workspace edge.
    ✔ strip never intersects the workspace at any width×state; verify wide
    market @1440 and standard fleet @1366.

F2. **Toast lane covers the workspace header** (`deck.css:381`).
    `right:12px` puts the lane inside the open workspace; toasts are
    `pointer-events:auto` and live 12s, making Back/width/✕ unclickable and
    covering the breadcrumb (confirmed live: OrderConfirmed toast at
    1548×48 over the market header).
    Fix: `right: calc(var(--deck-workspace-inset, 0px) + var(--space-3))`.
    ✔ toast renders left of the workspace edge with a route open; header
    buttons clickable while a toast shows.

F3. **Route accelerators swallow browser/system shortcuts**
    (`index.ts:364-377`, plus the `s` branch at `:374`).
    `routeKeys[key.toLowerCase()]` + `preventDefault()` with no modifier
    check: ⌘R→Research, ⌘C→Faction, ⌘V→Fleets, ⌘L/⌘P/⌘S/⌘Y all hijacked.
    Fix: `if (event.ctrlKey || event.metaKey || event.altKey) return;` at
    the top of the accelerator handler.
    ✔ ⌘R reloads, ⌘C copies; plain M/V/R… still route.

F4. **The bottom band has no stacking rule** (strip · founding chip · zoom,
    all `bottom:12px`).
    Confirmed live at 1366×768 with a fleet selected: strip covers the
    founding chip's action edge (49×139), strip buries the zoom cluster
    entirely (150×32 — click-dead, z30 over z10), and founding vs zoom
    collide at a z TIE (both `--z-chrome`; DOM order decides,
    `markup.ts:37,38`). Real collision thresholds: strip×chip below
    ~1464px viewport (not the spec's 1100px guess), founding×zoom whenever
    map width < ~535px.
    Fix as a system: (a) after F1 the strip lives in the map rect — also
    clamp its LEFT edge to `founding-chip right + 12px` when the chip is
    visible, and below the width where both fit, dock the strip ABOVE the
    chip (single column, strip first); (b) give the chip
    `max-height: calc(100dvh - var(--deck-topbar-height) - 24px)` with an
    internal scroll (`overflow-y:auto; overscroll-behavior:contain` on
    `.deck-founding__body`, `deck.css:392`) so it can never reach the top
    bar on short viewports; (c) zoom cluster gets `--z-command` or a
    dedicated slot above chrome so it is never buried, and keeps its
    inset-aware right anchor.
    ✔ probe at 1366×768 and 1280×720 with selection + route open: zero
    intersections among strip/chip/zoom/workspace; zoom buttons clickable.

F5. **Enter fires pending orders under overlays** (`index.ts:354`).
    The Enter branch checks only `state.pendingIntent` — pressing Enter
    while a battle replay or the shortcuts overlay is open confirms a
    hidden order.
    Fix: bail out of Enter-confirm (and the accelerator table) whenever an
    overlay is open (theaters, help, join); Esc keeps its current chain.
    ✔ arm an intent, open the help overlay, press Enter → nothing sent;
    close overlay, Enter confirms.

## P1 — degraded UX

F6. **The strip bypasses the shared DOM kit** (`strip.ts:64`).
    Direct `innerHTML =` with no `renderDeferred`: a status/event arriving
    mid-press destroys the Confirm button between pointerdown and pointerup
    — the exact bug `shell/dom.ts:3-19` exists to prevent, on the shell's
    most important button. Also resets the verb row's scroll and drops
    :hover.
    Fix: render via `renderDeferred("deck-command-strip", …)` + `setHtml`.
    `data-deck-command` is already NODE_KEY-safe only by accident — either
    add it to `NODE_KEY_ATTRS` (`dom.ts:109`) or switch the strip to
    `data-deck-act` for one convention.
    ✔ hold pointerdown on Confirm while a View arrives; release completes
    the click.

F7. **Workspace content breakpoints key off the viewport, not the
    workspace** (`deck.css:332,338,344`).
    All content media queries are viewport-width, but workspace width is
    route/user-controlled. Live consequence: `.deck-builder`
    (`deck.css:101`) has a 620px min grid inside a ~435px standard-width
    box → 185px horizontal overflow, and since width choice is persisted
    per route (`workspace.ts:43`), a user who toggles Build to standard is
    broken permanently. Research at standard = three 137px columns.
    Fix: make the workspace a container (`container-type: inline-size` on
    `.deck-workspace__body`) and convert the three content queries to
    `@container` widths; or key collapse off `data-width` on the root.
    Also order the query block 900 → 1050 → 1250 while touching it
    (currently 1250 → 900 → 1050 — a future-conflict trap).
    ✔ builder/research/officers readable with no horizontal scroll at BOTH
    widths at 1920 and 1366.

F8. **Ctrl+click double-fires on macOS** (`map.ts:139` + `:182-186`).
    Ctrl+LMB emits pointerup AND contextmenu → toggles the fleet into the
    group AND navigates the workspace to inspect. ⌘-click is clean.
    Fix: on macOS treat Ctrl+LMB's contextmenu as consumed when the
    pointerup already handled a group toggle (suppress inspect for the
    paired event), or stop advertising Ctrl and make it ⌘-only in the help
    overlay (`markup.ts:66`).
    ✔ Ctrl-click an own fleet: group toggles, workspace does NOT navigate.

F9. **The strip is oversized and its verb row scrolls invisibly**
    (`deck.css:361,368`; live: 216px tall at 1920, "Transit Stealth"
    clipped with a hidden h-scrollbar at 720px).
    Fix: tighten the strip toward one verb row + one status line (~120px):
    collapse Transit Full/Stealth into one segmented cell, drop per-verb
    subtitle lines into the hover/disabled reason, and when verbs still
    overflow, wrap to a second row instead of overflow-x. Keep the full
    explanations in the fleet route — the strip is for firing, not
    reading.
    ✔ all verbs visible without scrolling at map widths ≥ 700px; strip
    height ≤ ~140px.

F10. **Nav tail clips with zero affordance below ~1100px** (`deck.css:14`,
    `overflow-x:auto; scrollbar-width:none`). Syndicate/Faction/Log
    become unreachable except via accelerators (broken until F3).
    Fix: allow the nav to wrap OR add an overflow "⋯" menu; hiding the
    scrollbar on an unscrollable-looking row is the worst option.
    ✔ all nine destinations reachable by mouse at 1024px.

F11. **Double-click semantic entry arms a spurious move first** (live:
    dblclick home system with a fleet selected → "Move Interceptor to
    Themis." readout flashes/persists into the system view).
    Fix: on dblclick semantic entry, clear any intent armed by the pair's
    first click and clear the strip status line; or defer single-click
    intent arming by the dblclick window on system targets.
    ✔ dblclick into a system leaves no pending intent and no stale
    readout.

F12. **Camera rect ignores the top bar and the bottom band**
    (`workspace.ts:48-54` — `y:0, h:innerHeight` always). The spec's frame
    rule says nothing covers the map without the camera knowing; today the
    40px bar always covers it and the strip/chip band (up to ~216px) does
    too.
    Fix: `y: topbarHeight`, and subtract the bottom band height when strip
    or chip is visible — coalesce like mobile (ignore deltas < 15% of
    viewport height) so the camera doesn't re-tween on every strip
    appearance.
    ✔ selected fleet centered via ⌾ is not hidden under the strip.

## P2 — polish (batch into one commit each where sensible)

F13. Toast lane: pre-join it floats at `top:48px` over the join card while
     the top bar is hidden (`index.ts:633` vs `deck.css:381`) — key the
     offset off the bar's real visibility; and decide toasts-vs-overlays
     layering (z50 over z40 currently paints toasts over replays; either
     suppress non-critical toasts while a theater is open or drop the lane
     under overlays).
F14. Breadcrumb duplication: the command home renders "Command ·
     Command" (route title repeats the crumb); render the crumb only when
     depth > 1, and drop the "Command ›" root prefix on global routes
     (Market/Research/…) where it adds nothing.
F15. Focus treatment: `deck.css` has zero `:focus-visible` styles — add a
     designed ring on buttons/tabs/inputs (UA default currently carries
     it).
F16. Token strays: `#9a7b53` (`deck.css:521`), `#ffd98a` (`:558`), and the
     two drifted rgba values (`deck.css:502` vs `--accent`, `:268` vs
     `--warn`) → tokens/color-mix. Delete the dead `--hud-safe-top` /
     `--detail-safe-top` block + stale comment in `tokens.css:48-52`.
     `rmdir client/src/shell/desktop`.
F17. `toasts.ts:67`: register the dismiss FADE_MS timer in `this.timers`
     so teardown clears it. Add `overscroll-behavior: contain` to
     `.deck-workspace__body` (`deck.css:32`).
F18. Declare the shared-core change that landed silently: `dispatchBuildKey`
     ship gate widened from a 5-kind allowlist to `k in SHIP_YARD &&
     k !== "transport"` (`core/derive/market.ts`) — affects mobile too
     (capital-ship builds now dispatch there). Keep it (it reads as a fix),
     but smoke-test a destroyer build from the MOBILE shell and note it in
     the report.
F19. Verify `framePolicy().renderGalaxy:false` actually skips galaxy work
     under theaters: the code path reads correct (`index.ts:270-277`), but
     `data-render-paused` stays "false" with a theater open — confirm boot
     skips `renderer.update` and, if the attribute only tracks maxFps,
     document that; also consider hiding the strip/chip/zoom chrome while
     a full overlay is open so it doesn't ghost through the backdrop.

## Explicitly NOT bugs (verified good — don't "fix")

- Enter-to-confirm works (`window` keydown is fine); an earlier report of
  it being dead was a test-harness artifact (automation sent "Return").
- Hub-click with a fleet selected = move intent, selection retained.
- Esc chain order matches the spec (the battle-view step between overlay
  and workspace-back is a sensible insertion — keep it).
- Founding deep links: chip → build route with body + structure
  preselected (2 clicks to queue). Keep exactly this.
- RMB inspect, route widths + persistence, one toast impl, 0 native
  dialogs, 0 title=, teardown/AbortController, binder wiring, __ss rig,
  token hygiene (0 raw font px, 0 raw z, 0 unclamped widths).

## Acceptance sweep (report pass/fail for each)

1. Overlap probe (fixed/absolute rects, pairwise) at 1920×1080, 1440×900,
   1366×768, 1280×720 × states {idle, selection, selection+standard route,
   selection+wide route, intent pending, 3 toasts, founding visible}:
   zero unintended intersections.
2. ⌘R/⌘C/⌘V untouched; M/V/R/P/U/Y/C/L/S still route; ? overlay lists
   only what works.
3. Confirm survives a mid-press View tick (F6).
4. Builder + research readable at both workspace widths, no horizontal
   scroll (F7).
5. Enter inert under any overlay (F5).
6. Mobile smoke after core-touching fixes: tap-move, long-press attack,
   pinch scrub, hub tap with fleet selected, destroyer build dispatch.
