# Structure construction

Status: built Oct 8 2026, uncommitted. Protocol v44. It follows the clickable
mockup Peter approved.

## The rule the UI teaches

Each world builds one structure at a time. Orders wait in line in the order
they arrived. Hulls, modules and Academy courses use their own facilities and
never take that slot. Founding Camp projects do share it. Goods are paid when
an order reaches the world, so the build list counts goods already promised to
orders in flight as held.

Since Oct 22 2026, upgrades from tier III cost more and take longer (the tier
curve, docs/retention-p0.md).

## One order, one bar (Oct 15 2026)

Peter: "the fact there are two progression bars feels wrong". One build
showed its progress in four places: the construction bar, a ring on the
planet, a second bar (or "upgrading to II · 40s") in the structure list, and
the system view's own queue with bars. After a clickable mockup
(https://claude.ai/artifact/N6KuZvpK6oymghDR7byy3u) he said "implement it".

- An order's bar and time exist only in its world's **Construction card**.
  Exception (Peter, Oct 21 2026): on the planet, a small bar (no time) sits
  over the active order's **Building** tag.
- Everywhere else an order is one word (`jobTag`, `SENDING` in
  `core/derive/worldbuild.ts`): **Building** (holds the slot), **Starting**
  (first in line, start not yet reported), **#N** (its place in line),
  **Sending** (still on its way).
- A world's slot in one line (`slotLine`), for the top-bar menu and the system
  view: "Mining Complex II · 38s · +1 queued", or "Build slot idle".
- **Live bars (Oct 27 2026).** Peter: the bar looked "laggy/jittery".
  - Cause: a bar only moved when its panel redrew. The deck's world panel
    redraws on a whole-second heartbeat plus discrete changes (the
    light-delayed market ticker and the like). Measured on localhost: about 4
    redraws a second, 100–400 ms apart, moving a 15 s build's bar 1–3% each
    time. The server's progress line only changes when its rate does.
  - Fix: each construction bar carries its job's progress line
    (`data-live-progress="at base slope"`, from `progressLine`). Every frame,
    both shells' `onViewTick` sets its fill from the render clock
    (`tickLiveProgress`, shell/liveprogress.ts).
  - Measured after: the same build's bar updates every frame (≤0.14% a step,
    never backwards). Panels keep their own redraw cadence.
  - Bars covered: the Construction card, the planet's Building pill, the
    deck shipyard pop-out, and mobile's construction head and yard.
  - Not covered: the 3D hull filling in the shipyard view still follows its
    panel's redraws.

## Desktop world view

- **Construction card**, pinned under the world's header so it never scrolls
  away. One list, in order:
  - the order holding the slot: art, title, the one progress bar, the time
    left (its tooltip says how old the report is) and Cancel. "Starting…"
    while its start report travels;
  - waiting orders: "#2", title, "starts in 38s", Cancel. Always listed; the
    old "N queued · next X" fold is gone;
  - orders sent from this client and not yet reported: "Sending · 9s"
    (tooltip: confirms in). An order with no report 15 s after two light legs
    is presumed refused: it says so, with Dismiss, and it no longer holds its
    goods or blocks ordering that building again (Oct 15 2026: the refused
    Solar Array that found the fake semiconductors held the trader's
    semiconductors for 10 minutes). The order that finds the fakes is dropped
    at once;
  - hulls in this world's yard, **Build** and
    **Build ship** on a shipbuilding world.
  - Clicking a building on the planet marks its order in the card.
- **Structure rows:** a building with an order reads "Tier 1 · Operating ·
  ↑ II" plus its tag. A new building that is under construction or ordered is
  listed beneath the built ones with its tag. No bars, no times.
- **On the planet:** each site with an order carries its tag (Building in
  green, #2, Sending dashed). No ring, no time. An order in flight holds its
  site as a faint ghost.
- **Building (§build-sheet, Oct 9 2026):** choosing what to build or upgrade
  opens as a sheet across the planet view, from Peter's approved mockup
  (https://claude.ai/artifact/QghcWtTiTF3qboLTp9GQFM). The world column, with
  its construction bar, stays in view beside it.
  - Three ways in: **Build** (construction bar or the worlds strip), a free
    site on the planet (the sheet opens on that site's slot type, new buildings
    only), or **Upgrade to N** on a selected building (the sheet opens on its
    next level). The building shows only that button; its costs and time are
    on the sheet (Peter, Oct 28 2026).
  - Tabs are the slot types, each showing its used/total meter. A tab lists
    *Upgrade* (what stands), *New · N free sites*, and a folded *Can't build
    here (N)* with each reason ("No free Industrial site", "Needs Shipyard II",
    "Gaia worlds only", "Needs … research").
  - Cards show the building's scene art on this world's ground, the step
    ("New", "IV → V"), its newest order's tag ("Sending", "V · #2",
    "II · Building"), and goods icons with numbers plus the build time.
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
  - Sending it closes the sheet (Peter, Oct 20 2026), on desktop and mobile;
    Build opens it again for the next order. The order shows at once on the
    Construction card and on its building ("Sending"). Hulls keep their
    workbench open.
  - Once sent, the order holds its spot as "V · Sending" and cannot send again.
  - After the report, the order keeps the spot: its tag ("II · #2", "I ·
    Building") and its **Cancel**. The next level is a quiet "Also queue level
    III" link below, never a primary button where the last click landed
    (Oct 15 2026: Peter queued level II by accident).
  - Missing goods are listed with **Buy at Market**.
- **Completion:** the toast opens the world with the new building selected. If
  the building needs a crew, the toast says so and the workforce + is ringed.
- **Top bar chip:** "N building · M idle" across every developed world. It
  opens a menu with each world's `slotLine`, plus Open or Build.

## System view

The system's Construction section (Overview, Production and the Build tab)
lists its developed worlds, one `slotLine` each, with that world's hulls and
modules beneath ("Medium Freighter · 5m", "waiting for a slip", "paused ·
needs workforce"), and **Open** (or **Build** on an idle slot, which opens the
world's build sheet). It draws no bars.

## Cancel

`CancelBuild { system_id, job_id }` travels at light speed like
`DevelopSystem`. On arrival, the structure order is removed, waiting or
building (since Oct 15 2026; a started job's progress is lost and the next
order takes the slot), and its recipe at its tier (times the Unstable Geology
premium, as `apply_build` charged) returns to the system stockpile. A
`BuildCancelled` event shows on the timeline.

It is refused, through `IndustryRejected`, when:

- it is not a structure (Founding Camp projects included);
- a later order for the same structure on that world still builds on it;
- the job is gone;
- the system has changed hands.

The client offers Cancel only on the cancellable orders, behind a Confirm: in
the Construction card, in the build sheet's action spot, and on mobile in the
build detail and the world's Construction list. `BuildState` carries the job
`id`.

## Also fixed

Both ship builders (desktop `empire.ts`, mobile `parity.ts`) now list the
Survey Probe and the Mining Barge. Before this, the charter's probe and barge
steps could not be built from the UI.

## Code

- **Queue model and words:** `client/src/core/derive/worldbuild.ts`
  (`structureQueue`, `jobTitle`, `jobTag`, `SENDING`, `slotLine`, `yardStatus`).
- **Progress lines and live bars:** `client/src/core/derive/construction.ts`
  (`progressLine`, `progressAt`, `buildProgress`, `liveProgressAttr`,
  `readLiveProgress`); `client/src/shell/liveprogress.ts` (`tickLiveProgress`).
- **Card, system section and build sheet:** `empire.ts` (`constructionModel`,
  `constructionBarHtml`, `queueHtml`, `worldSheetHtml`, `sheetCard`,
  `sheetDetailHtml`, `PendingBuild`).
- **Column and scene:** `planet.ts` (tags, `pickedOn`, `upgradeHtml`; the sheet
  renders in the scene).
- **Mobile:** `parity.ts` (`planetInfrastructure`: the world's list with the one
  bar; `systemConstruction`: one line per world; `structureDetail`).
- **Styles:** `planet-panel.css` (card, tags, sheet); `deck.css` (system lines,
  the chip); `mobile.css` (`.m-order--head`).
- **Sim:** `crates/sim/src/world/construction.rs` (`cancel_build`).
- **Tests:** `client/scripts/test-planet-panel.mjs` (`--serve --construction`
  seeds a timed build, a waiting upgrade and a hull), `test-progression.mjs`
  (progress estimates, system lines), `test-industry.mjs` (mobile); the sim
  construction tests.

## Research unlocks (Oct 28 2026)

Peter: "Anything not available to the player until a research unlocks it
should not be rendered in the UX." The Research screen is the one place that
shows what is still locked.

- **Structure levels:** the server sends each structure's ceiling,
  `ResearchView.structure_tiers` (protocol v72, the sim's
  `max_buildable_tier` on the corporation's research; 0 = not unlocked).
  `structureCeiling` / `structureUnlocked` in `core/derive/market.ts`. A
  building with no level open is not catalogued; a level above the ceiling is
  `capped`: the build sheets leave the building out, the level ladder draws
  only the open rungs, and the building's Upgrade button disappears. This
  covers tiers V–VI, station levels and research-gated buildings.
- **Defense card:** the station's next level appears once researched.
- **Equipment forges:** modules whose programme isn't done are not listed
  (`moduleResearched`).
- **Logistics:** no Harvesting tab before Ramscoop Skimming; no Orbital
  Assembly project before Fusion Reactors; no Cruiser reservation before its
  hull; routes stop adding stops at three (a return trip) until Autonomous
  Freight.
- **Refining estimate:** shown only once a Smelter stands or Enrichment is done.
- **Corvette (Nov 11 2026):** Peter: "Shouldn't the corvette and combat
  freighter not show up in the shipyard until they have been unlocked by
  research?" The Corvette now needs Escort Datalink (Hulls I, whose card
  always named it): `ship::requires_hull_unlock` and the client's
  `HULL_PROGRAMME`, which a test now checks against the sim. The Combat
  Freighter has no programme yet and still shows; its unlock is Peter's pick.
- Already hidden before this: unresearched hulls, structures whose first
  level needs research, the lance battery, Mass Streams, signature mimicry
  and crown designations.

## Vault (Oct 28 2026)

The Vault is dug under a world's Command Center: no slot and no ground site,
so it is listed in the world's column but never drawn on the painting. It
needs a Command Center on that world ("Needs a Command Center") and stays
unseen until the charter completes. Its art is a stand-in (the Garrison's)
until its own lands. Rules: docs/blockades.md, "The Vault".
