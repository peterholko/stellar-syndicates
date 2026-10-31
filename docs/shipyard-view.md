# Shipyard view

Status: built Oct 21 2026, uncommitted. Client only (no protocol change).

Peter generated three empty yard interiors and asked that a ship queued in
the shipyard appear, under construction, in a new panel over them.

## What the player sees

- **Desktop:** a **Shipyard** pop-out, the size of the Market and the
  trader's table. It opens when ships are queued in the ship builder (the
  builder closes). It also opens from any hull line: "Hulls in the yard" in a
  world's Construction card, or a hull under its world in the system's
  Construction list. Tabs list every hull in the system's yards (building
  first, with their percentages; waiting ones say "waiting for a slip"). Under
  the bay: the hull's name, its yard and world, a bar and the time left, and
  **Build ships** (back to the ship builder). It keeps its full height under
  an open founding guide, which floats over it, as over the trader's table.
- **Mobile:** a **Shipyard** sheet with the same bay, tabs and line. It opens
  after **Queue hulls** and from the hull lines in the system's Construction
  tab and a world's queue. The ship builder sheet starts with an "In the yard"
  row that leads to it.
- **The bay:** the yard's interior plate with the hull's live 3D model (the
  approved LOD2 GLB) in its clear central volume. The hull is built from the
  stern forward. The built part is solid, the rest is a cyan blueprint
  (wireframe over a faint fill), and an amber weld line marks the cut. A
  hull waiting for a slip is all blueprint. On the planetary deck, a soft
  contact shadow sits under the keel. Before its report arrives, a just-ordered
  hull shows as **Sending**.

## Hull stats

Peter, Oct 27 2026: "On the ship building screen, shouldn't it display the
stats of the ship below the 3d model?" Built the same day, client only:

- One row of figures under the 3D hull: **Speed · Hull · Attack · Defense**,
  plus **Cargo** for a hull that carries any (freighters, the Mining Barge, the
  Mining Belt Station), or **Loot** for the Combat Freighter's loot-only hold.
  Base hull, before research and modules. Plain numbers
  (a speed in su/s clipped on a phone).
- Where: the Shipyard pop-out under the bay and its line, above **Build
  ships**; the desktop ship builder right under its 3D preview (the stats used
  to sit below the costs and build time); the mobile `yard` sheet and the mobile
  ship builder, which showed none.
- Code: `hullFigures` in `core/derive/orders.ts` (from `SHIP_STATS`, which now
  covers the Mining Barge, Mining Belt Station and Troop Transport, and
  `cargoUnitsPerHull`); `hullStatsHtml` in `shell/deck/shipyard.ts` (also used by
  the builder in `empire.ts`) and in `shell/mobile/parity.ts`; `.deck-hull-stats`
  is one row of equal cells, `.m-hull-stats` its mobile twin. The bay gives up
  70 px of height for the row (`100dvh - 430px`).
- The fleet details keep their own block ("Impulse … su/s").

## Pieces

- **Art:** `client/art-src/shipyard-interiors-v1/{shipyard-ii,orbital-shipyard,capital-slipway}-empty-bay.png`
  (1672 × 941, made for compositing) → `public/art/derived/shipyard-interiors/v1/{slug}-{768,1280,1672}.webp`
  (`scripts/build-shipyard-interior-art.mjs`, in `assets:derive`). Shipyard →
  Shipyard II plate; Orbital Shipyard (`naval_drydock`) and Capital Slipway
  their own. `shipyard-ii-construction.png` (with a ship baked in) is unused.
- **Scene data:** `client/src/shipyard-scene.ts`. For each yard: the plate,
  where the hull's centre sits (`cx`, `cy`), camera elevation, hull heading,
  `fill` (the yard's longest hull's share of the plate's width), the clear
  volume (`maxW`, `maxH`), deck or vacuum, and warm light. Hulls scale by a
  presentation length, compressed (^0.6) so a probe still reads.
- **Viewer:** `<ship-construction-view kind yard progress>`
  (`ship-construction-view.ts`), a shadow-DOM element. Its canvas is centred
  on the plate's (`cx`, `cy`), and the hull's image stands in until WebGL is
  ready. The runtime (`ship-construction-runtime.ts`, lazy-loaded) renders on
  demand with clipping planes (`localClippingEnabled`). Nothing animates; it
  redraws when the progress changes (the panels redraw every second).
- **Data:** `core/derive/shipyard.ts`, `yardJobs` / `pickYardJob`. Progress
  comes from the served earned-work segment (`buildProgress`). The hull shown
  is the one picked, else the newest of the kind just ordered, else the first
  building.
- **Panels:** `shell/deck/shipyard.ts` (route `shipyard`, a pop-out; opened
  from the System View, it keeps the orrery beneath) and the mobile `yard`
  sheet in `shell/mobile/parity.ts`.

## Checking it

`npm run test:shipyard-view`. To see it: the Vite dev server's
`/scripts/shipyard-view-fixture.html` (`?only=shipyard`, `?all=1`, `?p=0.6`,
`?p=wait`, `?cols=3`). `window.__yards` holds the live poses, and
`__refresh()` re-frames after a tweak. The browser pane, usually hidden,
renders WebGL only when a screenshot forces a frame. Headless Chrome over CDP
(SwiftShader) captures it reliably, but it does not show the contact shadow,
though a real GPU does.
