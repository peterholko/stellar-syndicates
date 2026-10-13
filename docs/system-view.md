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
- **Light.** Each world's baked highlight is turned toward its star, with a
  night-side shade. Owner-reported population adds settlement lights on the
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
- **Docking slots.** A docked fleet parks beside the world hosting the facility
  it uses: warships at a yard, freighters at a warehouse, transports at a
  garrison, otherwise the main world. A rival's empty structure maps fall back to
  public geography. Each world draws up to 3 hulls at fit zoom (6 and 10 as the
  camera zooms); the rest fold into a "+N docked" token.
- **Near zone.** Just outside the outermost orbit, contacts between the docking
  radius (260 su) and the defenses' reach (1,300 su, `NEAR_ZONE_SU`) are drawn
  to scale at their true bearing. The zone has no rings or label. Pointing at a
  world that carries your defenses (a station or platforms) shows their reach
  as one dashed ring at the zone's edge.
- **Edge arrows.** Beyond the zone, only your own fleets and hostiles within
  sensor range appear, as arrows at its edge. An arrow points in for a fleet
  arriving within 45 minutes (ETA from served motion, or your flight plan). It
  points out for a fleet that left the zone in the last two minutes; your own
  fleets name their destination and arrival. Everything else beyond 1,300 su
  (parked fleets, allies, fights not tagged to this star) stays on the galaxy
  map.
- **Battles.** A fight at this star (inside the defenses' reach, which is also
  where the server tags a battle to a system) is drawn at the defended world; a
  tagged fight beyond it sits at the near zone's edge. Untagged fights farther
  out stay on the galaxy map. The two sides face each other and fire at a rate
  set by the arrived rounds. A concluded fight leaves a wreck field for as long
  as its record is served (dismissing the marker removes it).
- **Sieges.** Participants see a cordon and capture clock around the besieged
  world, bombardment while the garrison is pinned, and landing craft during a
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
- Dragging pans the orrery at any zoom (mouse, one finger, or a two-finger
  drag on touch). The camera stays inside the near zone's extent, a drag lets
  go of a followed world, and fit recentres a panned orrery before it leaves.
- A selected fleet in the System View uses the shared live-3D atlas.

## Galaxy map hints

Docked fleets still have no galaxy marker; instead a "⚓ N" tally sits under an
owned star's label. From 40× to the 96× doorway, a star's orbits and worlds
fade in, sized and posed like the System View they hand off to. The worlds use
the System View's own planet art, lit from the star with the night side away
from it, and those on the far side of their orbit pass behind the star (a grey
dot stands in until the art loads). Moons, rings and city lights appear only in
the System View. Ships near a
star ease to their System View size over the same approach (complete by 64×,
fully inside the 1,300 su near zone, back to full size 5,200 su out), so a
departing hull never dwarfs the system it is leaving and keeps its size across
the doorway. Past 96× the galaxy map keeps zooming unless the pointer is on the
star; there, ships near a star keep the System View's proportion to the growing
preview (never smaller than their System View size), so zooming onto a ship
near a star no longer enlarges it for close inspection. See `nearStarBlend` and
`shipSizePx` in `render.ts`; the sizes come from `systemShipPx`. Fleets beside
the Market Hub take the same size (`docs/hub-model.md`).

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
- `battlebackdrop.ts` — the theater's place plan.

Debug: `__ss.systemDemo()` shows a synthetic picture in the viewed system
(berths, near-zone traffic, an inbound pirate, two departures, a battle, a wreck
and a siege);
`__ss.systemDemo(false)` restores the served picture. Nothing is sent.

Tests: `npm run test:system-view`, `test:battle-backdrop`, `test:star-art`,
`test:map-zoom`, `test:planet-panel`.

Art still owed: orbital variants of the defense platform, orbital warehouse and
sensor array (they currently use vector stand-ins).
