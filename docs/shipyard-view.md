# Shipyard view

Status: built Oct 21 2026, uncommitted. Client only (no protocol change).

Peter generated three empty yard interiors and asked that a ship queued in
the shipyard appear, under construction, in a new panel over them.

## What the player sees

- **Desktop:** a **Shipyard** pop-out, the size of the Market and the
  trader's table. It opens when ships are queued in the System View's ship
  builder (the builder closes). Queued from a planet's own builder, the
  planet stays and the hull takes shape in its ground Shipyard's bay (Peter,
  Nov 9 2026: "Click Queue 1 Ship should not zoom out of the planetary
  view"). Opened from a planet (its yard tag, a repair), it floats over that
  planet like the trader's stall; ✕ and Esc return to it. It also opens from any hull line: "Hulls in the yard" in a
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

## Repairs (Nov 5 2026)

Peter: "It should be really cool to have the ship being repaired in the ground
based shipyard fly down to the ground in the planetary view and add a button to
view the shipyard close up ... could we have that for repairing?"

- **Who is mending:** `repairYardBody` (the world with a staffed Shipyard or
  Ordnance Foundry, sim `build::REPAIR_YARDS`) and `yardRepairs` (your hulls
  docked in that system with damage) in `core/derive/shipyard.ts`.
- **Planet view:** a hull in for repairs leaves the sky and stands on its
  yard's pad (`.deck-planet__yard-ship`, its model poster with a "64%" tag),
  on the yard's world whatever its berth. It flies down once
  (`planet-yard-landing`, 2.4 s); `landedAt` resumes a flight a redraw
  interrupts with a negative animation delay instead of restarting it.
  Reduced motion skips it. A click opens the close-up.
- **Yard detail:** the Shipyard's (or Foundry's) detail lists each hull
  mending ("Repairing · 64% hull") with **View shipyard**.
- **Close-up:** repairs follow the builds as tabs; route query `repair`
  (mobile sheet prop `repair`) opens on one. The hull stands whole in the
  Shipyard bay (`progress` 1) and the bar is the hull it has back.
- **Guide:** step 13 is a step of its own, "Watch the repair" (Interceptor ·
  64% hull) with **Watch** (`watch-repair`; Peter, Nov 11 2026: "a clear
  step with a Watch button ... brings the player to the planetary view").
  On desktop it opens the planet view of the yard's world, where the hull
  stands on the pad; on a phone, whose planet sheet doesn't draw it, the
  yard sheet. Later steps keep "Repairing the Interceptor · 64%" under way.
  It was **View shipyard** (the close-up) until then.

## On the ground Shipyard (Nov 9 2026)

Peter: "when the Interceptor (or any ship is being built in the Shipyard) can
you render a similar ship silhouette/skeleton in the actual Shipyard on the
ground like you kinda did for the Repairing system?"

- **Where:** the desktop planet view (`shell/deck/planet.ts`
  `yardBuildsHtml`). Each hull under construction (active, not waiting for a
  slip) at this world's ground Shipyard sits in the yard's open bay. Orbital
  Shipyard and Capital Slipway hulls don't show; their yards are in the sky.
  A hull in for repairs keeps the pad.
- **In the bay** (Peter, Nov 9 2026: "have the ship in the 'garage part' of
  the shipyard, i.e. rotate the interceptor image 90 degrees to the left"):
  - `SHIPYARD_BAY` gives the bay cradle's place in each tier's sprite (I–III
    about 0.38 × 0.62, IV–VI lower), as fractions of the square. The
    building's footprint is that square, centred on its spot.
  - The hull is turned by `--yard-hull-turn`, −50°: its nose follows the
    bay's rails into the hangar. Peter asked for −90° first, then chose −50°
    when shown both.
  - Further hulls shift along the bay (`--yard-ship-index`).
- **Look** (`styles/planet-panel.css`):
  - The skeleton is cyan blueprint ribs on a dark frame, masked by the
    hull's poster (`.deck-planet__yard-frame`).
  - The built hull (`img.is-hull`) fills it in from stern to nose: the
    posters face right, engines left.
  - An amber weld line marks the edge.
  - The progress tag sits level below the hull, which is drawn at half the
    building footprint.
- **Live:** the hull carries its progress line (`data-live-build`,
  `liveBuildAttr`). The frame tick (`tickLiveProgress`) sets `--build` and
  the tag between redraws.
- **Click:** opens the Shipyard close-up on its kind (`planet-yard-build` →
  route `shipyard`, `query.kind`).
- **Preview:** `planet-fixture-orbital` has a "Toggle ship build" button. The
  fixture bundles its TypeScript at start, so restart it after a TS change.
- **Not done:** mobile, which shows no repairs on the ground either.

## Launch to orbit (Nov 9 2026)

Peter: "an animation to the interceptor (or any ship) completed in the
shipyard that shows it launching into orbit and taking up its regular orbit
spot ... some quasi 'launch animation' instead of just tweening it over
there".

- **When:** a hull leaves the ground Shipyard's slip at its due time (the
  build's served `complete_time`, 5 s leeway) while its world is on screen.
  A hull cancelled before its time doesn't launch. Neither does a world
  drawn for the first time, nor one not drawn in the last 6 s (`noteLaunches`
  in `shell/deck/planet.ts`).
- **Where to:**
  - Its new fleet in this world's sky (or the docked fleet it joined) gets
    the launch, from the bay's cradle to its sky slot.
  - If its fleet folds into "+N in orbit", it flies there and fades.
  - If it berths at another world (the berth rule: Capital Slipway, Orbital
    Shipyard, then Shipyard for warships; warehouses for freighters), it
    climbs up and away and fades. If its fleet shows in this sky mid-climb,
    it heads for the slot instead.
- **The flight** (6 s, `styles/planet-panel.css`). Since Nov 9 2026 a
  takeoff, not a rocket (Peter: "launch more like an airplane which has the
  capabilities to fly up to space vs a rocket launching straight up ... have
  the interceptor travel horizontally out of the shipyard and then up to
  orbit"):
  - 0–0.5 s: the engines spool up in the bay (glow, a shudder).
  - to 1.3 s: it taxis out about a hull length, turning level from the bay's
    50°.
  - to 2.6 s: the takeoff roll along the ground, gathering speed, the plume
    opening up and dust kicked up behind it.
  - then it pitches up 16° and lifts off, climbs (26°) in a curve that
    steepens toward the slot, growing to its orbit size, and levels out there.
  - The runway always points right, the way the hulls' art faces, and a hull
    is never mirrored (Peter, Nov 9 2026: "Can we avoid that flip of the
    image somehow?"; rotated to head left instead, the art's three-quarter
    view reads as upside down). The roll is half the way across to the slot,
    7–18% of the stage, short of its right edge (`launchCourse` in
    `planet.ts`, `--launch-run` in cqw).
  - A slot behind where it lifts off (to its left, or overhead: a warship's
    slot at 38% from a yard right of about 28%) can't be flown to nose first:
    it climbs away over the top and its parked hull fades in at the slot as
    it goes (`is-arriving`). The course is fixed at its first draw, like the
    start offset.
  - The dust is `planet-dust` (the launch's old `planet-smoke` keyframes had
    the siege impact smoke's name and overrode it).
- **Redraws:** each launch's start offset (`skip`) is fixed when its element
  first draws, and orbit ships carry stable ids. `setHtml` morphs nodes in
  place, so rewriting the delay would make a running animation jump.
- **No reduced-motion fallback** (Peter, Nov 9 2026: "this is game"). It had
  switched this launch off in a browser asking for reduced motion. It was
  removed from the whole client: the planet stage's animations, the coach
  ring, the crate tiles, and the battle theater and 3D battle effects (full
  shard counts, glow, spin, flicker, shake and tilt easing).
- **Preview:** `planet-fixture-orbital`. Put a job with `complete_time` at
  or before the fixture's clock (0) on the berth world's slip, then replace
  it with a docked raider.

