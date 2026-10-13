# Structure construction

Status: built Oct 8 2026, uncommitted. Protocol v44. It follows the clickable
mockup Peter approved.

## The rule the UI teaches

Each world builds one structure at a time. Orders wait in line in the order
they arrived. Hulls, modules and Academy courses use their own facilities and
never take that slot. Founding Camp projects do share it. Goods are paid when
an order reaches the world, so the build list counts goods already promised to
orders in flight as held.

## Desktop world view

- **Construction bar**, pinned under the world's header so it never scrolls
  away. It shows:
  - the job holding the slot, with a progress bar and time left (or "Queued ·
    starts next" while its start report travels);
  - orders sent from this client and not yet reported. An order with no report
    after two light legs plus a margin says it may have been refused, with
    Dismiss;
  - "N queued · next X". This expands to the whole line with start and ready
    estimates, plus Cancel where allowed (see below);
  - hulls in this world's yard;
  - **Build** for structures and **Build ship** on a shipbuilding world.
- **Structure rows:**
  - A building being upgraded reads "upgrading to II · 1m".
  - A new building that is still under construction or ordered is listed
    beneath the built ones.
- **On the planet:**
  - The active site has a progress ring and the time left.
  - Waiting sites show their place in line.
  - An upgrade rides on its building ("↑ II · 40s").
  - An order in flight holds its site as a faint "order sent" ghost.
- **Building (§build-sheet, Oct 9 2026):** choosing what to build or upgrade
  opens as a sheet across the planet view, from Peter's approved mockup
  (https://claude.ai/artifact/QghcWtTiTF3qboLTp9GQFM). The world column, with
  its construction bar, stays in view beside it.
  - Three ways in: **Build** (construction bar or the worlds strip), a free
    site on the planet (the sheet opens on that site's slot type, new buildings
    only), or **Upgrade to N** on a selected building (the sheet opens on its
    next level).
  - Tabs are the slot types, each showing its used/total meter. A tab lists
    *Upgrade* (what stands), *New · N free sites*, and a folded *Can't build
    here (N)* with each reason ("No free Industrial site", "Needs Shipyard II",
    "Gaia worlds only", "Needs … research").
  - Cards show the building's scene art on this world's ground, the step
    ("New", "IV → V"), any order state ("Order sent", "V queued #2",
    "II building · 40s"), and goods icons with numbers plus the build time.
    Short goods are red.
  - The detail column shows a larger picture, the level ladder I–VI, a short
    description and the sim's own figures (`structureFacts` in
    `core/derive/worldbuild.ts`: output ×, crew, housing, storage, Command
    Center bonuses, yard unlocks). It also lists costs against what the system
    has, when the order would start and finish, and Queue. Levels V and VI are
    marked as research; only catalogued levels (Command Center, stations) can
    say whether yours is done, because the client doesn't receive per-building
    research unlocks.
  - **Choose on planet** folds the sheet into a bar ("Place Smelter I on a lit
    site · Use this one · Back"). The sites the pick can take light up, the
    rest dim, and a ghost shows on the suggested site. Esc steps back from
    placing to the sheet, then out.
  - Free sites always show for the owner as quiet + marks that name their
    kind on hover.
  - Ships keep the central workbench. Done or Esc closes the sheet.
- **The Queue button:**
  - It names the exact job ("Queue Smelter I", "Queue level V").
  - Once sent, it reads "Order sent · confirming" and cannot send again.
  - After the report, the card shows its place in line, and the button offers
    the next level as its own order.
  - Missing goods are listed with **Buy at Market**.
- **Completion:** the toast opens the world with the new building selected. If
  the building needs a crew, the toast says so and the workforce + is ringed.
- **Top bar chip:** "N building · M idle" across every developed world. It
  opens a menu with each world's job and time left, or "Build slot idle", plus
  Open or Build.

## Cancel

`CancelBuild { system_id, job_id }` travels at light speed like
`DevelopSystem`. On arrival, a structure order that is still waiting is
removed, and its recipe at its tier (times the Unstable Geology premium, as
`apply_build` charged) returns to the system stockpile. A `BuildCancelled`
event shows on the timeline.

It is refused, through `IndustryRejected`, when:

- the job has started;
- it is not a structure;
- a later order for the same structure on that world still builds on it;
- the job is gone;
- the system has changed hands.

The client offers Cancel only on the cancellable rows, behind a Confirm.
`BuildState` carries the job `id`.

## Also fixed

Both ship builders (desktop `empire.ts`, mobile `parity.ts`) now list the
Survey Probe and the Mining Barge. Before this, the charter's probe and barge
steps could not be built from the UI.

## Code

- **Queue model:** `client/src/core/derive/worldbuild.ts`.
- **Bar and build sheet:** `empire.ts` (`constructionModel`, `constructionBarHtml`,
  `worldSheetHtml`, `sheetCard`, `sheetDetailHtml`, `PendingBuild`).
- **Column and scene:** `planet.ts` (the sheet renders in the scene; `upgradeHtml`).
- **Styles:** the bottom of `planet-panel.css`; the chip in `deck.css`.
- **Sim:** `crates/sim/src/world/construction.rs` (`cancel_build`).
- **Tests:** `client/scripts/test-planet-panel.mjs`; the sim construction tests.

Mobile keeps its own build sheet for now.
