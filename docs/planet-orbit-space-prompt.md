# Planet paintings: more orbit space above the planet

Art request for the user's other image model. The planet view's orbit layer
(orbital shipyard, capital slipway, defense station, docked ships, landing craft)
lives in the dark space above the horizon. That band is only the top 12–14 % of
the six habitable paintings and 18–19 % of the six basic ones, so the orbit layer
crowds against the bright atmosphere. Each painting needs more space added above
it. Nothing else in any painting may change.

## What to deliver

One extended image per painting, 12 in all:

- **Size:** 1672 × 1181 PNG, RGB. That is the original 1672 × 941 painting plus
  240 px of new space on top. Space becomes roughly the top 30 % of the image on
  habitable worlds and 35 % on basic ones.
- **Placement:** the original painting fills the bottom 941 rows, exactly as it
  is now: not moved, cropped, rescaled or repainted.
- **File names:** the master's name with `-orbit` before `.png`, saved beside
  the master (for example `habitable-v1/highland-plateau-orbit.png`).

| Painting | Master | Sky today |
|---|---|---|
| Highland Plateau | `client/art-src/planet-view/habitable-v1/highland-plateau.png` | thin black sky, sparse stars, blue limb |
| Savanna World | `client/art-src/planet-view/habitable-v1/savanna-world.png` | thin black sky, sparse stars, blue limb |
| Volcanic Highlands | `client/art-src/planet-view/habitable-v1/volcanic-highlands.png` | thin black sky, sparse stars, blue limb |
| Boreal World | `client/art-src/planet-view/habitable-v1/boreal-world.png` | thin black sky, sparse stars, blue limb |
| Archipelago World | `client/art-src/planet-view/habitable-v1/archipelago-world.png` | thin black sky, sparse stars, blue limb |
| Temperate Basin | `client/art-src/planet-view/habitable-v1/temperate-basin.png` | thin black sky, sparse stars, blue limb |
| Barren | `client/art-src/planet-view/surfaces/barren-surface-v1.png` | black sky, warm sun glare top-left |
| Desert | `client/art-src/planet-view/surfaces/desert-surface-v1.png` | black sky, warm sun glare top-left |
| Ice | `client/art-src/planet-view/surfaces/ice-surface-v1.png` | black sky, warm sun glare top-left |
| Terrestrial | `client/art-src/planet-view/surfaces/terrestrial-surface-v1.png` | black sky, warm sun glare top-left |
| Lava | `client/art-src/planet-view/surfaces/lava-surface-v1.png` | black sky, warm sun glare top-left, orange-red limb glow |
| Ocean | `client/art-src/planet-view/surfaces/ocean-surface-v1.png` | black sky, warm sun glare top-left, blue limb |

## Prompt

One painting at a time; attach the master.

    Extend this planet painting upward only, by adding more outer space above
    the planet. Output a 1672 × 1181 image: the attached painting is the bottom
    941 rows, unchanged, and the top 240 rows are new space.

    Do not change anything in the attached painting. Keep the planet, terrain,
    water, clouds, atmosphere, horizon, stars, colours, lighting, camera angle
    and painterly style exactly as they are. Do not move, crop, rescale, zoom,
    straighten or repaint any part of it. The only change is the added space at
    the top.

    The new space continues the painting's existing sky seamlessly: the same
    near-black deep space, the same small, sparse white stars at the same size
    and density, and the atmosphere's thin glow fading out above the horizon
    exactly as it already does. The seam between the old and new rows must be
    invisible.

    Add no new objects: no moons, planets, suns, stations, ships, satellites,
    asteroids, nebulae, galaxies, auroras, lens flares, text, borders or
    interface.

Add one line for the six basic paintings, which all have a sun glare (barren,
desert, ice, terrestrial, lava, ocean):

    The warm sun glare in the top-left corner continues upward naturally into
    the new space and fades with distance, as light from a sun just outside the
    frame. Do not add a visible sun disc.

And for lava:

    Keep the orange-red glow along the horizon exactly as it is; above it the
    new space is the same near-black starfield.

## Checking a result

- Lay the returned image's bottom 941 rows over the master. Apart from a soft
  blend into the new sky just above the seam, nothing should differ.
- The new space should hold no objects and no change of brightness or colour at
  row 240.

## In the game (done Oct 6 2026)

- **Art:** `client/scripts/build-planet-workbench-art.mjs` derives the extended
  masters into `planet-workbench/habitable-orbit-v1/` and `orbit-v1/`, and fails
  the build if any extension's bottom 941 rows differ from its master. The
  returned files were already pixel-identical, so no compositing was needed.
- **Frame:** the planet view keeps the 16:9 ground frame that every building
  site, haze rule and badge is placed in; the painting reaches 240 rows above it
  (`planet-panel.css`, `.deck-planet__scene:not(.is-orbital)`). Sites are
  unchanged and needed no regeneration, because the ground is unchanged.
- **Founding screen:** keeps the 16:9 originals (`planetSurfaceArt(surface)`);
  the planet view asks for `planetSurfaceArt(surface, true)`.
- **Orbit layer:** on painted worlds the Orbital Shipyard and Capital Slipway sit
  at y −12 %, the defense station at −16 % + its level (`orbitalYardAddress`,
  `painted`); gas giants keep their old positions around the globe. Docked fleets
  (up to three, then "+N in orbit") use the same berth rule as the System View.
- **Siege:** the tint covers the whole extended painting, the banner rides in
  one line above the yards, and landing craft drop from high in the band.

## Result (Oct 5 2026)

All 12 extensions arrived and passed: 1672 × 1181; the bottom 941 rows are
pixel-identical to the masters; no visible join; nothing larger than a star in
the new space; the sun glare continues upward on all six basic worlds. Minor:
the new space has fewer stars than the original sky on most worlds (about half
to nine-tenths as many), and faint 1–2-level rectangular patches show only when
the image is brightened about tenfold.

