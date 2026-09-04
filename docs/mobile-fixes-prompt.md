# Codex fix prompt — mobile UX defects (post-review)

Repo: stellar-syndicates. Work on the existing `codex/mobile` branch.

These nine findings come from a code review of the shipped mobile shell. They
are all SHELL-LEVEL fixes: the core/shell architecture is sound and must not
change. Rules:

- Do NOT modify `src/core/*` except where a finding explicitly says so.
- Do NOT modify the desktop shell (`src/shell/desktop/*`) or `desktop.css`.
- No Rust/server changes.
- `npm run build` clean after every commit. One commit per finding, in order.
- Where a finding calls for judgment, prefer the smallest change that removes
  the annoyance.

---

## F1 — Mobile typography and tap targets (the big one)

`src/styles/mobile.css` carried the desktop's dense 9-10px monospace idiom onto
a phone: 70 of ~98 font-size rules are <=10px, tab labels are 7px
(`.m-tabs button > small`), there is a 6.5px rule, eyebrows are 8px. Many
controls are 28-38px tall, under the 44px touch floor.

Establish a mobile type scale and apply it across mobile.css:
  - body / list text:        12px minimum (13px for primary lines)
  - secondary / dim text:    11px minimum
  - eyebrows / overlines:    10px minimum (they are uppercase + letterspaced;
                             below 10px they smear)
  - tab labels:              10px
  - NOTHING below 10px. Delete the 6.5-9px rules by promoting them.
  - Sheet title stays 15-17px; stat strong values 13-14px.

Tap targets: every interactive element the thumb hits routinely — tab buttons,
sheet header buttons, list-row buttons, qty steppers, segment controls,
checkin/decision actions — gets min-height 44px (visual density can stay via
padding/negative-space; the HIT AREA must be 44px). The sheet grip zone and
drag header already qualify.

Expect the taller rows to change how much fits per screen — that is fine; the
sheets scroll. Do not shrink type elsewhere to compensate.

## F2 — Gameplay events are silent

`src/shell/mobile/index.ts` `onCore()` ignores `ReportArrived`,
`OrderConfirmed`, `BattleConcluded`, `CommandSignal`, `CommandChevron`, and
`EstimateReady` (see `src/core/events.ts` for the union). A raid resolves and
nothing appears anywhere on mobile.

Implement:
  a. A transient TOAST STACK for mobile (reuse/extend `m-map-notice` into a
     small stack, newest on top, max 3, each auto-dismissing; tap = open the
     relevant sheet). Surface:
       - ReportArrived  -> one-line summary, good/bad tone, tap opens Log sheet
       - BattleConcluded-> outcome line, tap opens the battle sheet for that id
       - OrderConfirmed -> brief low-key confirmation (auto-dismiss 2s)
     Derive wording from the same report/outcome fields the desktop
     `shell/desktop/checkin.ts` / reports-log uses — do not invent new copy.
  b. An UNREAD BADGE on the Log tab (`m-tabs button[data-destination="log"]`):
     count of reports + decision-inbox items arrived since the Log sheet was
     last opened. Clear on opening Log. Session-scoped is fine.
  c. EstimateReady: check whether the mobile ship sheet even exposes the
     engagement-estimate request (desktop has it on the ship panel). If the
     request button is missing, add it to the fleet sheet
     (`src/shell/mobile/surfaces.ts` renderShip); surface the arriving
     estimate in that sheet and toast if the sheet is closed.
  d. CommandSignal / CommandChevron are map-drawn by the renderer; verify the
     renderer paths still fire on mobile (they are state-driven) and add
     nothing if so.

## F3 — Tab behavior fights the player

`src/shell/mobile/index.ts` `m-tabs` click handler: tapping the ACTIVE tab
calls `replaceSheet(destination)`, which re-renders with `resetDetent=true` —
resetting the detent and collapsing open <details>. And there is no
toggle-to-dismiss.

Fix:
  - Tapping the active destination tab CLOSES the sheet stack (`closeAll()`)
    — tab acts as a toggle, returning to the map.
  - Tapping a different tab replaces, as now, but `replace()` of a DIFFERENT
    id may keep resetting; `replace()`/`refresh()` of the SAME id must
    preserve the current detent and open-<details> state (the openDetails
    machinery in `src/shell/mobile/sheets.ts` already exists — the bug is that
    `render(resetDetent=true)` is used on same-id replace).

## F4 — The map slides on every sheet motion

`SheetStack.moveDrag()` calls `onLayout()` on every pointermove, which calls
`syncCameraRect()` -> `renderer.setCameraRect()`, which re-centres the world
focus — so the camera shifts continuously while dragging the sheet, and jumps
on every open/close.

Fix:
  - During an active sheet drag, do NOT sync the camera rect; update the CSS
    height only. Sync once in `endDrag()` when the detent settles.
  - On sheet open/close, sync once (as now) — but suppress the focus-preserving
    recentre when the rect change is small (< 15% of viewport height) so
    half<->half-ish changes don't nudge the map. Camera moves should feel like
    a response to a detent change, not to every pixel of gesture.

## F5 — Founding guide and map notice collide

Both `.m-founding` and `.m-map-notice` are pinned at
`calc(var(--mobile-chrome-bottom) + 8px)` (mobile.css:333, :231); the notice
(z29) covers the founding card (z27), and the ~330px founding card covers the
top map band — exactly the map that remains visible when a sheet is open.

Fix:
  - Founding guide on mobile defaults to its MINIMIZED single-line chip form
    (the is-minimized machinery exists in `surfaces.ts` refreshFounding);
    expanded is an explicit tap. Persist per the existing localStorage key.
  - Stack the notice BELOW the founding chip when both are visible (compute
    the founding chip's height like `--mobile-chrome-bottom` is computed, or
    simply anchor `.m-map-notice` under `.m-founding` when it is visible).
    They must never overlap.

## F6 — Two-finger drag does not pan

`MobileMapInteraction.applyPinch()` (`src/shell/mobile/map.ts`) early-returns
when `|log(factor)| < 0.0001`, so a two-finger translation does nothing, and
the pan path only runs for a single pointer. Standard map muscle memory
(pinch-drag zooms AND pans) is broken.

Fix: track the previous pinch midpoint; on each two-pointer move, in galaxy
mode (and not scrubbing), call `renderer.panBy(midpoint.dx, midpoint.dy)`
BEFORE the zoom step, then apply the zoom at the new midpoint. Keep the scrub
and battle/system handoff branches driven by the scale delta exactly as now
(the early return should only skip the ZOOM, never the pan).

## F7 — Keyboard pops over the join screen

`src/shell/mobile/index.ts` mount(): `byId<HTMLInputElement>("m-name").focus()`
throws the iOS keyboard over the join art before the player has read anything.
Remove the autofocus on the mobile shell (leave desktop's as-is). Focus moves
to the field when the player taps it.

## F8 — Dead pinch into battles

`applyPinch()` gates battle entry on
`battle && record?.outcome === null` — if the BattleRecords stream has not yet
delivered the record, the gesture silently does nothing. Desktop enters the
viewer regardless and shows its empty/waiting state.

Fix: enter on `battle` alone (drop the record requirement). The battle sheet
(`src/shell/mobile/battle.ts`) must render a "record still arriving —
light-delay" waiting state when the record for that id is absent, rather than
crashing or rendering blank.

## F9 — Sheets re-render at every View

`onCore()` calls `sheets.refresh()` on every ViewApplied (5 Hz view cadence).
`refresh()` -> `render(false)` rebuilds the full sheet HTML string every
200ms; morph absorbs the DOM churn but research/operations string-build is
wasted work and lists that reorder can jitter.

Fix with the codebase's OWN idiom (see desktop signature guards, e.g.
lastSyndicateSig pattern): each sheet render computes a cheap signature of the
state slice it draws plus `Math.floor(liveSimTime())` as a 1-second heartbeat
(keeps countdowns/ages live). `refresh()` skips `render()` when the signature
is unchanged. Per-sheet-id signature storage lives in the renderer classes
(`surfaces.ts` / `parity.ts` / `battle.ts`), not in SheetStack. The battle
sheet's theater tick must remain per-frame — only the HTML rebuild is gated.

---

## Acceptance (test in DevTools iPhone-14 emulation + one real phone)

- No text below 10px anywhere in the mobile shell; tab labels legible at
  arm's length; all routine controls hit-test at >=44px.
- Complete a raid: a toast appears when the report arrives, the Log tab shows
  a badge, opening Log clears it.
- Tap the active tab -> sheet closes to the map. Expand a sheet to full,
  wait through several Views -> it stays full with details open.
- Drag the sheet through its range: the map does not move until the drag ends.
- Early-game: founding chip is one line; a notice appears below it, never
  overlapping.
- Two-finger drag pans the galaxy; pinch still zooms and still hands off to
  system scrub at max zoom.
- Load the app on a phone: no keyboard until the name field is tapped.
- Pinch into a battle marker immediately after the battle starts: the battle
  view opens with a waiting state if the record has not arrived.
- With the research sheet open and nothing changing, the HTML rebuild is
  skipped (verify via a counter or performance profile).
- Desktop at 1920x1080 is untouched: `git diff` shows no changes under
  src/shell/desktop/ or styles/desktop.css.
