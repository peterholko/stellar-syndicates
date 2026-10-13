# Art still owed: Survey Probe, Marginal ocean, four crowded paintings

This is an art request for the user's other image model, Oct 9 2026. It covers
the stand-ins listed before alpha. Every framing number is measured from the
approved art each piece sits beside (see the art-brief-framing note in
`living-terraces-art-prompt.md`). The prompts don't reuse the original
prompts' numbers, which the approved art doesn't match.

**The Mining Barge needs no new art.** Its sprite, five-view sheet and textured
GLB (7,816 triangles, with sockets) are already in
`client/art-src/mining-barge-reference-v1/` and
`client/art-src/ship-models/mining-barge-textured-v1/`. What's left is code: bake
it into the game's ship-model set and remove its `ART_STAND_INS` entry in
`client/scripts/ship-model-catalog.mjs`.

## 1. Survey Probe

The probe currently borrows the Scout's art. It follows the other hulls'
pipeline: sprite, then five-view sheet, then the GLB. Save everything in
`client/art-src/survey-probe-reference-v1/`.

### Step 1: `survey-probe.png`, the hull sprite

Attach `client/art-src/ships-2026-09-08/scout.png` as Image 1. The fleet's
sprites fill about 88–93% of their canvas height (Scout y 4–92%, Corvette
4–94%, Interceptor and Freighter about 92%), so this prompt asks for that,
not the 80–84% in the original brief.

```text
Use case: stylized-concept.
Asset type: one production-quality transparent raster spaceship hull sprite for Stellar Syndicates, a premium space strategy game.
Input image: Image 1 is an existing Stellar Syndicates hull, attached ONLY as the fleet's style, material and finish reference. Do not copy its silhouette.
Primary request: design a NEW hull as a fresh, distinctive, professionally art-directed hard-surface spacecraft. It must read clearly both as a 32–80 pixel galaxy-map sprite and in a closer view.
Composition: strict orthographic TOP-DOWN plan view, camera directly overhead at 90 degrees; the craft's nose points exactly UP (12 o'clock), its thruster points DOWN. One complete centered craft on a square canvas. Its length spans about 88–90% of the canvas height, with about 5% clear space above the nose and below the tail, centered left to right, matching the existing fleet sprites. No perspective tilt, no side view. The outer silhouette should be purposeful and unmistakable.
Visual style: clean high-end rendered hard-surface game art; crisp machined geometry, readable large forms with restrained secondary panel detail. Brushed titanium, cool gunmetal, pale silver panels, a cyan sensor lens and tiny cyan running lights; restrained warm brass hardware. Consistent soft upper-left illumination and good value separation for a dark navy map. Neutral lights, no team-color wash.
Background: genuinely transparent PNG with real RGBA alpha, including gaps between parts. No ground plane, black rectangle, painted checkerboard, smoke, outer halo, scene or starfield. Only a tiny thruster-aperture glow, no exhaust plume. No beams, scans, signals or effects.
Constraints: exactly one craft, no text, numbers, labels, logos, watermark, border, circular badge or UI. No painterly brush marks, crayon edges, excessive micro-greeble noise, organic shapes, toy appearance or recognizable franchise design. Maximize real native detail without artificially upscaling.

Subject and distinguishing silhouette (takes priority):
A SURVEY PROBE, a small CREWLESS robotic drone, NOT a crewed ship: no cockpit, no windows, no crew modules, no guns, no cargo. A compact armored sensor core with ONE large forward sensor dish whose cyan lens faces the nose; FOUR slim antenna booms splayed diagonally like an X from the core; two small folded solar/radiator fins at the sides; and exactly ONE small cylindrical thruster at the tail. About 0.75 width-to-length ratio. Pale silver and gunmetal with a few ochre hazard markings. The silhouette must say robotic probe, clearly simpler than the Scout.
```

### Step 2: `survey-probe-reference.png`, the five-view sheet

Attach the Step 1 result as Image 1, and
`client/art-src/construction-ship-reference-v1/construction-ship-reference.png`
as Image 2. Use the Step 2 prompt in `docs/survey-probe.md` unchanged.

### Step 3: the GLB

Rebuild `survey-probe.glb` from the five-view sheet with the shared pipeline
(`client/art-src/ship-models/mining-fleet-v1/README.md`). Save it as
`client/art-src/ship-models/survey-probe-textured-v1/`, laid out like the Mining
Barge's folder (GLB, previews, `model-info.json`).
- It is the smallest hull in the fleet: about 1.5–3k triangles.
- Give it one socket, at the thruster nozzle.

## 2. Marginal ocean

Marginal ocean worlds borrow the ice sprite and the ice painting. They need a
planet sprite and a ground painting, plus the painting's orbit version. The
building ground patch can reuse the Marginal one (`grounds/marginal.png`), so
it needs no new art.

### Step 1: planet sprite, `client/art-src/planets-2026-10-08/marginal-ocean-v1.png`

- **Image 1:** `client/art-src/planets-2026-09-08/ocean.png`.
- **Image 2:** `client/art-src/planets-2026-10-08/marginal-soft-edges-v1.png`.

Both sprites are 1254 × 1254 with the planet spanning about 94% of the canvas.

```text
Use case: precise-object-edit.
Asset type: one transparent planet sprite for Stellar Syndicates, a premium space strategy game.
Image 1 is the EDIT TARGET: the existing ocean-world sprite. Keep its square canvas, circular silhouette, size, centring, upper-left lighting and soft cinematic illustration style.
Image 2 is ONLY a palette and mood reference for a marginal world (cold, dry, thin-aired). Do not copy its continents.
Primary edit: turn Image 1 into a MARGINAL OCEAN world, almost Earth-like but cold and thin-aired. A cold slate-teal and grey-blue ocean covers most of the face, with a few small low islands of dun and cool khaki rock and dusty sage-lichen patches. Broad pale sea-ice shelves and salt-white shoals reach out from two generous white polar caps. One or two faint thin cloud wisps. The water is darker and less saturated than a living ocean: no tropical turquoise, no reefs, no green forests.
Framing: the SOLID PLANETARY DISK, excluding the faint atmosphere, occupies 90–92% of the canvas width and height, aim 91%: at 1254 × 1254 the disk spans about x=56 to 1198 and y=56 to 1198, an even ~4.5% clear margin on all four sides. A true circle, no clipping.
Atmosphere: a very thin, pale dusty-blue rim tight to the disk edge, fainter than a living ocean's, with clean anti-aliasing and fully transparent space beyond. No halo or bright emission.
Lighting: warm-white upper-left light, about 80% of the face lit; the lower-right night side stays coloured and legible. Broad smooth tones; no added noise, speckle or fine coastline clutter.
Output: exactly ONE high-resolution square RGBA PNG with a genuine transparent background: a single complete centred planet. No text, stars, other objects, shadows, rings, frames or baked checkerboard.
```

### Step 2: ground painting, `client/art-src/planet-view/surfaces/marginal-ocean-surface-v1.png`

- **Image 1:** `client/art-src/planet-view/surfaces/ocean-surface-v1.png`, 1672 × 941.
- **Image 2:** the Step 1 sprite.

The hostile ocean painting has room for only 4 buildings, so this one asks for
islands big enough for a full colony from the start.

```text
Use case: precise-object-edit.
Asset type: empty planet-surface background for the space strategy game Stellar Syndicates.

INPUT ROLES:
Image 1 is the EDIT TARGET and approved composition template: the existing ocean surface painting, 1672 × 941.
Image 2 is ONLY a biome and colour reference for the marginal ocean world. Never borrow its full-globe framing, circular silhouette or transparent background.

PRIMARY EDIT:
Transform Image 1 into the marginal-ocean biome below while keeping Image 1's camera, crop, planetary curvature, horizon placement, lighting direction and cinematic visual format exactly. Produce a single opaque full-frame 1672 × 941 landscape image.

COMPOSITION INVARIANTS:
Keep the same high oblique three-quarter aerial camera looking down over a broad convex planetary cap, same elevation and lens. The planet extends beyond the entire bottom and both side edges. Preserve the strong curved planetary arc and horizon placement. Keep the quiet near-black sparse starfield above it and the same warm sun glare at the extreme top-left corner. Do not zoom out to a small globe or lower the viewpoint to ground level.

WORLD-SPECIFIC TERRAIN:
A marginal ocean world: almost Earth-like, but cold and thin-aired. A cold slate-teal and grey-blue sea with long low swells, scattered pale sea-ice floes and salt-white shoals, and faint frost on the island shores.
Building room (important): in the band between about 55% and 80% of the image height, across the central two-thirds of the width, place ONE or TWO broad, low, flat islands of dun and cool khaki ground with dusty sage-lichen patches, gravel and a few pale salt flats. Together they must be large enough for a settlement of about 17 buildings. Their tops are smooth, level and open: no cliffs, jagged rock spires, pits, ponds or forests on them. Keep a few smaller rocky islets elsewhere for variety, and open sea around the big islands.
The atmosphere is thin: a very faint pale dusty-blue rim follows the curved horizon, clearly thinner and paler than Image 1's. Only a few thin cloud streaks near the horizon, none over the islands.
Palette: slate-teal, grey-blue, dun, cool khaki, dusty sage and salt white. Muted, cold and quiet, but not dead.

EMPTY BACKGROUND:
Pristine natural terrain. No structures, foundations, pavement, roads, paths, building plots, vehicles, ships, lights, ruins or tracks. No trees, forests or grass meadows. No fixed number of locations baked into the terrain.

OUTPUT:
Exactly one seamless full-frame landscape at 1672 × 941, matching Image 1's framing. Opaque, with the black sky intact. No collage, border, UI, text, numbers, logos or watermark. No extra planets in the sky.
```

### Step 3: orbit version

Run the orbit prompt below on the Step 2 painting, and save the result as
`marginal-ocean-surface-v1-orbit.png`.

## 3. Four paintings with too few open building sites

A developed world holds 12–17 buildings, but these paintings have fewer clean
sites on open land, so the rest fall onto rough ground:

| Painting | Master | Clean sites now |
|---|---|---|
| Boreal World | `client/art-src/planet-view/habitable-v1/boreal-world.png` | 8 |
| Volcanic Highlands | `client/art-src/planet-view/habitable-v1/volcanic-highlands.png` | 10 |
| Archipelago World | `client/art-src/planet-view/habitable-v1/archipelago-world.png` | 12 |
| Ocean (hostile) | `client/art-src/planet-view/surfaces/ocean-surface-v1.png` | 4 |

For each painting:
1. Attach the 1672 × 941 master and run the edit prompt below, filling in its
   line.
2. Save the result beside the master with `-v2` before `.png`, for example
   `habitable-v1/boreal-world-v2.png`. Keep the originals.
3. Run the orbit prompt on the edited painting and save it as `…-v2-orbit.png`.

```text
Use case: precise-object-edit.
Edit this planet-surface painting (Image 1, 1672 × 941). Keep the composition, camera, horizon, planetary curvature, lighting, colours, starfield and painterly style exactly, and output the same 1672 × 941 framing.
Widen the open ground in the middle distance. In the band between about 55% and 80% of the image height, across the central two-thirds of the width, [THIS PAINTING'S LINE]. The result needs enough level, open ground for a settlement of about 17 buildings. Keep the large lakes, rivers, cliffs, coasts and forests elsewhere, so the world still reads as itself.
No buildings, roads, paths, fields, plots, ruins, vehicles, text or interface.
```

**Each painting's line:**
- **Boreal World:** "turn most of the dense conifer forest, granite outcrops and small ponds into one continuous open meadow of low grass and heath, with only a few scattered trees and boulders"
- **Volcanic Highlands:** "smooth the plateau into continuous open grassland: remove the scattered dark rock outcrops, small ponds and minor ridges there, keeping the big cliffs, waterfalls, lakes, steam vents and the volcano"
- **Archipelago World:** "on the main island, turn the forest patches and small hills of the central lowland into one continuous open grassy plain, keeping its beaches, lagoons, coastal hills and the surrounding islands"
- **Ocean (hostile):** "replace the small jagged islets with one or two broad, low, flat islands of bare grey-brown rock and gravel, smooth and level on top (no spires, pits or cliffs). Keep open deep-blue sea around them and a few small rocky islets elsewhere. This is a hostile world: no grass, trees or greenery"

## The orbit prompt

Every new or edited painting also needs its orbit version: 1672 × 1181, which
is the painting plus 240 rows of space on top. Attach the painting. This is the
prompt from `docs/planet-orbit-space-prompt.md`:

```text
Extend this planet painting upward only, by adding more outer space above the planet. Output a 1672 × 1181 image: the attached painting is the bottom 941 rows, unchanged, and the top 240 rows are new space.

Do not change anything in the attached painting. Keep the planet, terrain, water, clouds, atmosphere, horizon, stars, colours, lighting, camera angle and painterly style exactly as they are. Do not move, crop, rescale, zoom, straighten or repaint any part of it. The only change is the added space at the top.

The new space continues the painting's existing sky seamlessly: the same near-black deep space, the same small, sparse white stars at the same size and density, and the atmosphere's thin glow fading out above the horizon exactly as it already does. The seam between the old and new rows must be invisible.

Add no new objects: no moons, planets, suns, stations, ships, satellites, asteroids, nebulae, galaxies, auroras, lens flares, text, borders or interface.
```

## When the art is in

These are code steps, not art:
- **Survey Probe:** add it to the ship-model catalog, remove its stand-in entry,
  and bake its sprites and LODs.
- **Marginal ocean:**
  - point `PLANET_ART_SOURCES.marginal_ocean` at the new sprite;
  - add the painting to `build-planet-workbench-art.mjs` and
    `build-planet-sites.mjs`;
  - drop the ice fallback in `planetSurface`;
  - map its ground layer to `marginal`.
- **Four paintings:** point the painting sources at the `-v2` files, regenerate
  the building sites (`assets:sites -- --review`), and check each now reaches
  about 17 clean sites.
