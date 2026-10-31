# System View

The star's rung of the zoom ladder (galaxy → system → world). Its job is to show
what is happening at this star right now: who is docked, what is coming, what is
being built, and what is being fought over. Per-building detail stays on the
world rung (the planet stage); the system workspace keeps the lists.

The System View remains a presentation of system-level facts. The sim has no
intra-system positions, travel or targeting, and nothing here adds any. All
inputs come from the same light-gated, fog-filtered picture the galaxy map uses.

## What is drawn

- **Tilted orbital plane.** Orbits are foreshortened ellipses
  (`ORBIT_TILT` in `systemlayout.ts`); sprites stay upright. Worlds behind the
  star pass behind it.
- **Orbital motion.** Each body turns about its parent as a pure function of its
  public seed and time (Kepler-flavoured periods, `PLANET_PERIOD_S`). A remote
  system is posed at its observed time: its picture runs behind by its light age.
- **Light.** Planet and moon artwork stays upright as it orbits. Only the
  night-side shade turns away from the star; the painted surface is never
  rotated to chase the light. Owner-reported population adds settlement lights on the
  night side (a rival's bodies report none).
- **Rings and stars.** About half of gas giants wear rings. Each star type moves
  in its own way (binary pairs turn, pulsars sweep, magnetars flare, red giants
  swell, black holes draw infall). A star inside a public nebula takes its haze.
- **Charting.** A system's contents are unknown until you survey it or own it.
  The server sends no planets, moons or belts for an unsurveyed system, so the
  System View shows only the star under "NAME · UNCHARTED · SEND A SCOUT OR
  SURVEY PROBE". A Survey Probe charts one system and is spent doing it
  (`docs/survey-probe.md`). The star's spectral band (its size and glow on the galaxy map) is
  the one hint that reaches you from afar.
- **Asteroid belts.** Belts are real bodies rolled per galaxy
  (`crates/sim/src/belts.rs`) and are charted with the rest of the system. It is drawn as dust and chunk sprites tinted by type
  (stony, metallic, icy, shattered), in the gap between planet orbits nearest
  its rolled radius, so it never runs through a world. Unsurveyed systems show
  no belts.
- **Orbital infrastructure.** Orbital Shipyards and Capital Slipways use their
  station art, including the occupied variant. Defense platforms, orbital
  warehouses and sensor arrays are drawn as vector stand-ins (no orbital art
  exists yet). A status chip under an owned world reports construction, idle
  converters, and food or storage trouble.
- **True scale (Oct 20 2026, `docs/real-system-coordinates.md`).** The sim
  owns positions inside a system: every world orbits its star (served as
  `BodyView.orbit`, laid out by the sim with this view's own seeded draw, so
  systems look as they always did), and every ship at a system is at one of
  its worlds. One System View unit is 860 su (`SYSTEM_UNIT_SU`).
- **Docking slots.** A docked fleet is moored in a ring about the world it is
  at (the sim berths warships at a yard, freighters at a warehouse, transports
  at a garrison, otherwise the main world). Each world draws up to 3 hulls at
  fit zoom (6 and 10 as the camera zooms); the rest fold into a "+N docked"
  token.
- **Cordon.** A fleet at rest at a world it isn't berthed at (a visitor, a
  blockader, a raider, a pirate pack at its base) rings that world, facing it.
- **Near zone.** Contacts under way out to the defenses' reach (1,300 su,
  `NEAR_ZONE_SU`) are drawn at their true distance and bearing; past the
  frame's edge they hold it. The zone has no rings or label. Pointing at a
  world that carries your defenses (a station or platforms) shows their reach
  as one dashed ring, to scale.
- **Edge arrows.** Beyond the zone, only your own fleets and hostiles within
  sensor range appear, as arrows at its edge. An arrow points in for a fleet
  arriving within 45 minutes (ETA from served motion, or your flight plan). It
  points out for a fleet that left the zone in the last two minutes; your own
  fleets name their destination and arrival. Everything else beyond 1,300 su
  (parked fleets, allies, fights not tagged to this star) stays on the galaxy
  map.
- **Battles.** A fight at this star (inside the defenses' reach, which is also
  where the server tags a battle to a system) is drawn where it is fought, and
  stays there while its world turns on. Untagged fights farther out stay on
  the galaxy map. The two sides face each other and fire at a rate
  set by the arrived rounds. A concluded fight leaves a wreck field for as long
  as its record is served (dismissing the marker removes it).
- **Sieges.** Participants see a cordon and capture clock around the world
  the besiegers hold over (the main world), bombardment while the garrison is pinned, and landing craft during a
  landing. The headline above the orrery states the siege. The world's planet
  stage shows the same siege, with a "Follow landing" control.
- **Light-age headline.** "NAME · LIVE PICTURE" or "NAME · PICTURE Δ23m OLD".

## Interaction

- Fleets, battle sites, wreck fields and the landing headline are clickable;
  worlds otherwise, then belts. A belt's band is an annulus hit test. Its hover
  names its type and ores, and a click opens the belt's world panel, where its
  Mining Complex is built and staffed. A fleet or battle opened from the orrery keeps the orrery
  underneath its workspace. Double-clicking a battle or wreck opens its theater.
- Wheel, +/− and fit drive a short orrery zoom (up to `SYSTEM_ZOOM_MAX`) that
  follows the world it aims at. Pushing past the limit over a world opens that
  world. Returning from a world eases the camera back out.
- One selection at a time (Peter, Oct 21 2026). A world taking the ring
  (a click, its world route, a panel link) deselects fleets
  (`intent.deselectFleets`), and while a fleet is selected no world keeps a
  ring or pulse (`Renderer.syncSystemSelection`). Ctrl/⌘-grouped fleets
  still ring together.
- A world's ✕ (desktop) climbs one rung, like Esc: back to its system's orrery
  and panel, never out to the galaxy (Peter, Oct 21 2026). A second ✕ closes
  to the galaxy. Mobile's × closes the sheets and leaves the map where it is.
- Dragging pans the orrery at any zoom (mouse, one finger, or a two-finger
  drag on touch). The camera stays inside the near zone's extent, a drag lets
  go of a followed world, and fit recentres a panned orrery before it leaves.
- A selected fleet in the System View uses the shared live-3D atlas.
- Tracking inset (Peter, Oct 27 2026: "keep the ships the current size but
  when you click the ship, show a small 'tracking zoomed in view'"):
  - Selecting a fleet opens a camera in the map area's top-left corner
    (the toast lane owns the top right), 450×279 at most (Peter, Oct 27:
    "50% larger"; up to half the map's width). It reads
    "◉ TRACKING · <fleet>" in the fleet's tone, over a faint starfield that
    drifts with the fleet.
  - It draws a System View scene of its own (`tracking` mode), built on
    demand for the system it shows from the orrery's data, never its camera.
  - Its camera is a fixed scale (`TRACKING_SCALE` 330 px a unit), locked on
    the fleet.
  - Panning or zooming the orrery never reframes it (Peter, Oct 27 2026:
    "the tracking window shouldn't move its frame when the player pans or
    zooms").
  - Its scene lays out in the inset's size divided by a magnification (the
    hull about a third of the inset wide, 2.25–6×). The texture's
    resolution gives the pixels back, so hulls, worlds and the live 3D hull
    magnify sharply together.
  - Its camera follows the fleet every frame, so its hulls ride the camera's
    moves and keep easing; only a cut (a new fleet, or the inset shown
    again) snaps them.
  - It shows the world the fleet is at.
  - A Mining Barge cutting ore (`industry.kind == "mine"`, status `mining`)
    works among rocks of its own belt's art, with a pulsing cutting beam from
    its hull to the nearest and ore motes riding back (Peter, Oct 28 2026:
    "shouldn't the mining barge tracking frame show it mining ore"). The
    orrery scatters only 22 rocks around a whole belt, so none fell inside
    the close frame. `TrackingInset.drawWork`, `Renderer.trackingWork`.
  - Names, chips, rings, orbit lines and edge arrows never appear in it.
  - Clicks, hover and the wheel on it never reach the orrery beneath; a
    click keeps the selection.
  - It closes when the fleet is deselected, leaves the picture, or becomes
    an edge arrow, and on leaving the System View.
  - Desktop only; it needs a map area of at least 420×300.
  - A selected hull's minimum size now holds after its art's calibration,
    so an Interceptor also gets its 3D model.
- Raid camera (Peter, Oct 27 2026: the tracking window should "appear
  somewhere appropriate in the center when Pirate raid first happens before
  the Pirate Raid event art appears … disappear when the Pirate raid event
  art is shown"; "It should be tracking the incoming Pirate raid ship"):
  - It starts with the raid focus (boot.ts `noticeRaid`: the privateer's
    first sighting this session) and ends when the "Pirate raid" story
    opens (`StoryEventUI`'s `onShow`). It also ends after 10 s without the
    raider's ghost, after 5 minutes, or with the session.
  - The window is centred over the map, just above its middle, with the
    raid framed beneath it. It wins over a selected fleet's inset, on the
    galaxy map and in the System View (a battle theater hides it).
  - It draws the raided home with the raider where it really is, however
    far out (`trackedHull`; the orrery shows only an edge arrow there):
    first over the starfield, then over the colony as it arrives. The
    privateer loots from the star itself, so it ends silhouetted on the star,
    keeping the heading it came in on.
  - Pirate hulls are culture art: no live 3D hull joins the atlas.
  - Clicks, hover, the wheel and double-clicks on it never reach the galaxy
    map beneath either.
- Raid frame (Peter, Oct 27 2026: how to handle the first sighting of pirates
  on the galaxy map; the old follow is gone):
  - One camera move from wherever the player is (galaxy, System View or a
    planet): `frameRaid` flies the galaxy map (800 ms, even zoom, the raid's
    midpoint sliding straight to its place) onto the home's star and the
    raider together. Leaving a System View, the way out lands on the frame
    itself (the exit transition is retargeted), so there is no stop at the
    old galaxy camera.
  - The frame centres the pair in the map below the raid window (the whole
    map on mobile, above the raider's sheet), their separation about half
    that space, never past 80×: short of the 96× doorway, and past 64×,
    where hulls near a star have shrunk to its scale (at 40× a raider a few
    thousand su out still covered the star). A Command Center first sees
    the raider near its 6,000 su sensor edge; coming in from the side, that
    meets the 80× cap on a 1440×900 deck.
  - The shells open the raider's panel (or sheet) first, then frame, so the
    frame fits the map that is left. Then the map holds: the window follows
    the raider. A drag or zoom is the player's, and a route opening
    mid-flight never cuts it short (`ensureWorldVisible` waits for a camera
    move under way).
  - While the raid camera runs (both shells), a dashed amber line runs from
    the raider as drawn to the home star's disc, under the hulls
    (`drawRaidCourse`).
  - The news (Peter: "Raider detected on course for <System>"): on the
    deck, a band along the raid window's foot reads "RAIDER DETECTED · ON
    COURSE FOR FREYA" in the raider's tone, shrunk to fit a narrow window,
    until the raider crosses the hyperlimit to loot (`raidNote`; never on a
    selected fleet's window). Mobile has no window: a 20 s notice says
    "Raider detected · on course for Freya".
  - Later home raids keep their "Pirates inbound" toast.

## Galaxy map hints

Docked fleets have no galaxy marker at any zoom (except under blockade);
instead a "⚓ N" tally sits under an owned star's label. From 40× to the 96×
doorway, a star's orbits and worlds fade in, to the map's true scale and tilting
into the System View's plane over the dive. The worlds use
the System View's own planet art, lit from the star with the night side away
from it, and those on the far side of their orbit pass behind the star (a grey
dot stands in until the art loads). Moons, rings and city lights appear only in
the System View. Ships near a
star ease to their System View size over the same approach (complete by 64×,
fully inside the 1,300 su near zone, back to full size 5,200 su out), so a
departing hull never dwarfs the system it is leaving and keeps its size across
the doorway. Past 96× the galaxy map keeps zooming only over deep space or a
visible hull; there, ships near a star keep the System View's proportion to the
growing preview (never smaller than their System View size). See `nearStarBlend`
and `shipSizePx` in `render.ts`; the sizes come from `systemShipPx`. Fleets
beside the Market Hub take the same size (`docs/hub-model.md`).

Positions are real (Peter, Oct 20 2026: a travelling ship was out of
alignment across the handoff; "Let's do real coordinates"). Every hull is
drawn at its true place at every zoom, the same place the System View draws
it; over the 40→96× dive only the System View's tilt comes in, about each star
within the near zone, easing back to the flat map by 5,200 su. A ship resting
at a world moves over the dive into the ring slot the System View gives it
(`ringSlot`, shared), to the preview's scale. Picks, route lines, guard lines
and the move preview use the same placement (`nearStarPlacement`,
`restingRingOffset` in `render.ts`). The Oct 20 band warp is gone.

## The handoff (Oct 16 2026)

- Over the same 40×→96× dive the star's lens flare resolves into the System
  View's own disc: the disc opens from a quarter of the star's size, the flare
  fades, and at 96× the galaxy draws the System View's star exactly (art,
  visible size, anchor, and its turn, `starSpinAngle`). The preview's orbits
  pass beneath the star, as in the System View.
- At the doorway the whole orbit preview is the way in, not only the star: the
  outermost orbit and the worlds on it, or 0.6 System View units around an
  uncharted star (`approachSystemAt`). A visible hull keeps the wheel or pinch
  for inspection until the camera reaches 384×. The + key or button enters the
  star under the map's centre, else the selected star while it is on screen.
- Past 96× the preview keeps growing with the camera, at true scale. A handoff
  from any depth starts the schematic at the preview's true scale
  (`Transition.growFrom`), and mid-handoff the preview and every ship near the
  star ride the schematic's scale (`handoffGrowth`), so the two pictures
  coincide while they crossfade (measured to 0.1 px in the map-markers
  fixture, `?charted=1&ships=1`).
- The crossfade is one curve of the semantic position, the same both ways: the
  System View fades in over an opaque galaxy, then the galaxy fades out beneath
  it (`SYSTEM_FADE_SPLIT`). Identical pictures (star, orbits, worlds) therefore
  never dim or double.
- Entering always starts the orrery at rest. A retreat starts from wherever its
  star is on screen (`handoffCamFrom`) while a zoomed orrery eases back to rest;
  the wheel and the − button first return the orrery to rest, then leave.

Rig: the `markers-fixture` launch entry (port 4181) with
`?charted=1` charts Freya. In the embedded pane drive it by hand: `__pump(n)`
renders frames, `__crop(x, y, w, h)` mirrors a magnified region over the canvas
for a screenshot (take the screenshot twice; the first can be stale).

## Battle theater

A fight inside platform cover plays over its world: the world fills one corner,
lit from its star, which glares from the opposite edge (`battleBackdropPlan`).
Fights beyond the defenses' reach keep the distant system scenery; deep-space
fights keep the plain starfield. Backdrops never show names, deposits, infrastructure or lights.

## Code

- `systemlayout.ts` — projection, orbital motion, near-zone mapping, docking-slot rules.
- `systemtraffic.ts` — `deriveSystemPicture`: the fog-safe picture of a star.
- `systemtrafficlayer.ts` — draws fleets, battles, wrecks and sieges; picks.
- `systembodyfx.ts`, `systemstarfx.ts` — world dressing and star motion.
- `systemview.ts` — the scene, camera, labels and chips.
- `trackinginset.ts` — the tracking inset: its corner or centred place, camera, starfield and texture (`render.ts` builds its scene on demand, runs the raid camera, and draws it after the 3D hulls).
- `battlebackdrop.ts` — the theater's place plan.

Debug: `__ss.systemDemo()` shows a synthetic picture in the viewed system
(berths, near-zone traffic, an inbound pirate, two departures, a battle, a wreck
and a siege);
`__ss.systemDemo(false)` restores the served picture. Nothing is sent.

Tests: `npm run test:system-view`, `test:battle-backdrop`, `test:star-art`,
`test:map-zoom`, `test:planet-panel`.

Art still owed: orbital variants of the defense platform, orbital warehouse and
sensor array (they currently use vector stand-ins).
