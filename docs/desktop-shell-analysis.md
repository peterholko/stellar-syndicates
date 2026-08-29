# Desktop shell — why patching plateaued, and the shape of the rebuild

Analysis on `codex/desktop-ux-revamp` @ `337546c` (post P0–P3), 2026-08-29.
Method: live playthrough with a fresh corp ("Vantage Audit", still on the dev
roster) at 1920×1080 against the running release server, plus two full code
audits (surface/placement inventory of `shell/desktop/*` + contract audit of
`core/*`, `boot.ts`, and `shell/mobile/*` as the reference shell).

## 0. Verdict

The P0–P3 patches were real improvements — and they have hit their ceiling.
Root causes 1 and 5 from the August audit (map punishes the core gesture;
missing verbs) are substantially fixed. Root causes 2–4 (panel anarchy, actions
far from data, feedback misrouted) are *structural*: they live in the fact that
the desktop shell is still ten legacy panels wearing a shared frame, plus seven
independently-floating satellites, with no layout system, no navigation system,
no type system, and no single home for command feedback. You cannot patch your
way out of "there is no system." Mobile got a designed shell built fresh on the
core seams and it holds together; desktop deserves the same, and the seams —
built precisely so a second shell could exist — make it cheap now.

**Recommendation: build a new desktop shell (`shell/deck/`) from scratch on the
existing core contracts, port the content emitters that are genuinely good,
and delete `shell/desktop/` when parity lands.** The implementation prompt is
in `docs/desktop-rebuild-prompt.md`.

## 1. What P0–P3 actually bought (credit where due)

Verified live:

- **Star-click with a fleet selected now arms a move intent** with an
  excellent confirm bar: distance, ETA split into signal + flight, fuel
  estimate, FUEL SHORT warning. The intent-preview grammar is the best thing
  in the shell.
- **Hover cards exist** (`#map-hover` names what the next click will do) and
  the cursor changes shape.
- **Docked fleets draw as berth pips** and are pickable; engaged fleets pick
  at their battle position; ongoing battles are clickable.
- **Verbs moved onto their data**: Hold (new, end-to-end through the sim),
  Refit, Transit throttle (Full/Stealth), Ship-goods-to-market on the
  Production tab, deposits inline on body rows, the ship panel de-tabbed into
  one scroll.
- **Multi-select command groups** (Ctrl-click, `[`/`]` cycling, batch move).
- **A workspace frame** (P1) gives ten panels one placement, one z-index, one
  Back/Close/title, and killed the old right-dock eviction fights.
- **Check-in no longer auto-opens**; badges surface what needs attention.

This is why the game is *playable* now. It is also why the remaining wrongness
feels so frustrating: the content is good and the verbs exist, but the
structure they live in fights you.

## 2. The structural failures (all verified live, post-P3)

### 2.1 The workspace is a corral, not a system

`workspace.ts` physically reparents the ten legacy panels into one container
and neutralizes their old fixed-position CSS with a 12-declaration
`!important` block. Nothing was redesigned. Each page keeps its own header
idiom, its own tab strip, its own width assumptions, its own fonts:

- The rail page has a 6-tab strip (System / Fleets / Logistics / Doctrine /
  Officers / Rankings). Market has its own 4 tabs. Research has its own
  header. Check-in has its own ✕ (shown *in addition to* the workspace ✕ —
  it's missing from the duplicate-✕ hide list).
- "Fleets" appears at two levels meaning different things. "Build" appears in
  the system panel *and* the world panel meaning different things — during
  founding step 1 a new player faces both and has to guess.
- Only 6 of the 10 pages map to a nav button, so opening the other 4 clears
  every nav highlight; the rail separately re-adds highlights from its own
  module. Two systems write the same state.
- **Seven live surfaces never entered the workspace at all**: `#sysview-manage`,
  `#planet-panel`, the two `.build-shell` builders, `#battle-viewer`,
  `#ground-viewer`, plus all the map chrome. The August "panel anarchy" count
  barely moved: **17 elements still independently fixed at runtime across 13
  placement idioms** (27 declared in CSS across 16 idioms).

### 2.2 Selection and navigation share one channel — the deck's original sin

The single worst remaining trap, confirmed live: **with a fleet selected,
clicking the Wormhole Hub — the most-visited destination in the game — throws
away your fleet and opens the hub lore panel.** The hover card even announces
it: "Open Wormhole Hub." Move-to-hub, the single most common order in a
trading game, cannot be issued the way every other move is issued.

Same conflation, other direction: clicking your own docked fleet's marker
loses to the system hit-test (`SYSTEM_BIAS` pulls the click), which — since
you still have a fleet selected — arms a zero-distance move intent to the
system it's already in. The fleet is reachable only via click-cycling, which
nothing teaches.

And because the workspace is single-slot, *looking at anything destroys what
you were looking at*: fleet detail replaces the fleets roster; the hub panel
replaces fleet detail. You cannot see a fleet and its destination at once —
the exact failure the August audit called out on the old right dock, inherited
intact by the workspace.

### 2.3 The screen budget is inverted

Measured live at 1920×1080: the workspace is **520px wide**. Into that column
go the 109-programme research boards (6 boards — three visible, the rest
scroll), the 4-tab market with a floating trade ticket overlapping its own
price board, officer rosters, rankings. Meanwhile ~75% of the screen is
starfield that, at galaxy zoom, displays effectively nothing (§2.5).

There is no wide mode, no resize, no route-declared width. The one surface
that *does* get width — the 760px builder — takes it as a *second
independently-positioned left panel* that covers the map with no camera
compensation (the camera-inset machinery only watches the right dock; the
planet panel + builder can occlude 1,254px of map and the camera does nothing).

The stylesheet tells the same story: 49 hardcoded pixel widths, 11 distinct
`min()` max-terms (330/360/468/470/480/520/560/760/920/940/1140), four
surfaces still at a raw unclamped `width: 320px`, and an intent bar positioned
by `left: 402px; right: 412px` — magic numbers encoding the widths of two
panels that no longer exist at those sizes.

### 2.4 Depth without orientation

The founding path to "Build Shipyard I", counted live: **7 clicks across 4
stacked surfaces** (founding card → Open home system → Open System View →
world click → system Build tab → "Open the build panel" button → Shipyard →
Queue). The system Build tab's entire content is *a button that opens the real
build panel somewhere else*. Two tabs named Build. Exit is Esc×3 — which works
now (P0), but drops you at a galaxy fit-zoom with no anchor: no home marker
saliency, no label, no restored context.

Esc itself is not a stack — it's a hard-coded 9-branch priority chain, and it
mostly matches intuition, but the ground viewer isn't in it at all (✕ only),
and native `window.confirm`/`prompt` dialogs (5 of them: hostile-action
confirm, flagship naming, fit naming…) sit outside everything.

### 2.5 The map is a void at rest, a shortcut museum in help

At galaxy fit-zoom a fresh player sees a near-black field: no "you are here,"
own assets sub-pixel, no labels until deep zoom. Orientation lives in a
9-row legend of keyboard chords (Ctrl/⌘-click, `[`/`]`, M/V/R/P/U/Y/C/L, J,
Shift-click…) parked permanently bottom-right — a symptom: when interactions
aren't discoverable in place, you ship a manual.

### 2.6 Feedback still has four homes (plus tooltips, plus dialogs)

A single order's story is spread across: the top-center toast column
(`#reports-log`, 3 independent prepend/cap/fade implementations), the
bottom-left persistent readout (last-write-wins), the intent bar, and inline
panel copy — plus **199 `title=` tooltips**, many load-bearing (the only place
Buy/Sell pool settlement, dev-slot rules, or kit costs are explained), plus 5
native browser dialogs. The build-queue toast literally tells you your
feedback is elsewhere: "A soft-reject (no slot / short on goods) shows in the
Log." And "Confirm ⏎" still doesn't respond to Enter (an August bug, still
live) — the advertised keyboard path to the most common confirmation is dead.

### 2.7 The craft layer

- **Type**: 247 font-size declarations, all raw px, zero tokens; 80% in a
  9–12px band with fractional half-steps (9.5/10.5/11.5). At 1920×1080 the
  game is read at 10px.
- **Z-index**: 12 distinct top-level layer values for ~17 surfaces, with a
  live conflict: `openBattleViewer` (the normal "View battle" path) opens the
  z11 viewer *under* the z20 workspace, which covers ~144px of it.
- **Colors**: 156 raw hex + 135 rgba literals against 18 tokens.
- 490 id-rooted selectors in one 1,500-line stylesheet.

## 3. Why this can't be patched into goodness

Each failure in §2 is a *system-level property*: the layout has no owner, the
navigation has three competing grammars, selection has no channel of its own,
feedback has no single home, type has no scale. Fixing any one of them means
touching every panel — and the panels are 10,764 lines of legacy markup
emitters entangled with the placement they assume. P1 already demonstrated the
limit of the containment strategy: reparent + `!important` + special-cases
(`#checkin` inline-display handled in two places). The next increment of the
patching path *is* a rewrite of every panel's chrome — while keeping the old
markup's constraints.

The rebuild path is cheaper than it looks, because the August→mobile work
already paid for the hard part:

- **The core seams exist and are proven.** `applyServerMessage` + CoreEvent
  union, the intent machine (which owns all order sending), `resolveMapClick`
  (order legality + light-delay gating, shared with mobile), 7 derive modules,
  the renderer's `cameraRect`/scrub/hit-test contract, `framePolicy`,
  signature-gated refresh, the press-guard + morphing DOM kit. A new shell is
  presentation only.
- **Mobile is an existence proof**: full parity, built fresh on these seams,
  ~4,000 lines, and it's the *pleasant* shell today.
- **The good content is portable.** The decision-inbox derivation
  (`computeInbox`), the builder detail panes, the fleet-detail emitters, the
  battle replay state machine (`bvTick`/`gvTick` — hard and correct, do not
  rewrite), the market board, the icon/stat/badge helpers — all identified,
  all liftable with container markup swapped.

## 4. What must survive the rebuild (the game's soul)

1. **Light-delay identity in every string** — delay badges, signal-vs-flight
   ETA splits, "response light arrived," fog-honest staleness ("~15s STALE").
2. **The intent-preview grammar** — dashed preview + quantified confirm.
3. **Builder detail panes** — recipe with have/need, slot accounting, "enables."
4. **The decision inbox** — content and derivation, wholesale.
5. **The founding guide** — 12 server-authored steps with deep links.
6. **The orrery system view, semantic zoom** (wheel scrub galaxy↔system↔battle),
   **and both theaters** (replay machine + Pixi arenas untouched).
7. **Keyboard accelerators** — as accelerators over discoverable UI, not as
   the only path.
8. The dark terminal aesthetic. This is not a re-theme; it's a re-structure.

## 5. The target model — one stage, one workspace, one strip, one router

Full specification in the rebuild prompt; the shape:

- **Map stage** always full-bleed. Camera fits the unoccluded rect (existing
  `setCameraRect` mechanism, now fed honestly by the one dock).
- **One top bar**: identity + stats left, the 8 route buttons right. 
- **One right workspace** with three widths — closed / standard (~460px) /
  wide (min(65vw, 1240px)) — where each route *declares* its width: market,
  research, officers, builders open wide; fleet and system detail open
  standard. One header: Back · breadcrumb · title · width toggle · ✕.
- **One router** behind everything: HUD buttons, map inspection, deep links,
  breadcrumbs, Back, and Esc all drive the same history. Every current
  surface becomes a route — including the world panel and the builders, which
  stop floating over the map.
- **Selection is not navigation.** A persistent fleet-selection model with an
  RTS-style **command strip** (bottom-center): selection chips, legal verbs,
  armed-mode indicator with Cancel, intent summary with Confirm/Cancel that
  actually honors Enter. Left-click commands; right-click inspects; own units
  win hit-tests; **the hub is a move target like any star** (its market is one
  right-click or one hover-chip away). Nothing implicitly clears selection.
- **One toast lane** (top-right, deep-linking like mobile's notices), the Log
  as the durable mirror; refusals appear where the command was issued.
  `#readout` dies; its content moves into the strip and the toasts. Native
  dialogs die.
- **A token layer**: type scale with a 11px floor and 12.5px body, spacing
  scale, six z layers, the existing color tokens. No raw px sizes in deck CSS.
- **Map saliency pass**: own assets get minimum sizes + labels at all zooms,
  home badge, selected-target reticle, inbox "Focus" pings. The legend dies;
  a `?` overlay teaches shortcuts on demand.

## 6. Measurable acceptance (the "is it actually better" bar)

- Founding step 1 ("Build Shipyard I") from the founding card: **≤3 clicks**,
  zero ambiguous tab names (was: 7 clicks, two "Build" tabs).
- Move-to-hub with a fleet selected: **1 click + confirm**, selection retained
  (was: impossible without losing the fleet).
- Fleet + its destination system visible **simultaneously** (strip + workspace).
- Enter confirms a pending intent; Esc order is: intent → armed mode → scrub →
  overlay → workspace back → deselect. Every overlay is in the chain.
- One toast implementation; zero native `confirm`/`prompt`; zero load-bearing
  `title=` (each promoted or dropped deliberately).
- Zero unclamped fixed widths; zero raw px font sizes in deck CSS; ≤6 z values.
- Research readable at its declared width without horizontal scroll at 1440px.
- Old shell deleted; `npm --prefix client run build` clean; mobile untouched
  (same core, re-smoked).

## 7. Source inventories backing this analysis

- Surface/placement/commit inventory (17 live fixed surfaces, 13 idioms, Esc
  chain, z conflicts, font census, tooltip census, keep/discard lists): agent
  report, 2026-08-29, summarized throughout §2 and mirrored in the rebuild
  prompt's appendices.
- Core contract reference (Shell interface, boot, CoreEvent union, intent/
  mapclick/derive APIs, renderer contract, mobile patterns, sender parity
  tables): agent report, 2026-08-29, embedded in the rebuild prompt §2 and
  appendix A/B.
