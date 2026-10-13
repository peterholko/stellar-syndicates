# Asteroid belt mining: art prompts

**Status (Oct 7 2026): all generated and checked.**

- **Ore icons.** The nine icons are derived to 64/128 px by
  `npm run assets:derive`. Olivine, Bornite and Ilmenite are live. The six
  pocket-ore and material icons wait for phase 2.
- **Belt chunks.** Metallic, Icy and Shattered are derived to
  `public/art/derived/belt-chunks/v1/{kind}-{256,512}.png` and live in the
  System View, the belt panel and the Worlds rows. Stony keeps the original
  sprite.
- **Hulls.** The sprites and five-view sheets for the Mining Barge and the
  Mining Belt Station are filed. They still need the Python reconstruction to
  GLB, which happens when those hulls are built (phases 3 and 4).

These are the images the belt plan needs (`docs/asteroid-belt-mining-plan.md`).
Each prompt copies the format of the art the game already uses, so the new
pieces match:

- the ore icons use `client/art-src/ores-2026-09-16/prompts.json`;
- the hulls use `client/art-src/ships-2026-09-08/prompts.json` and
  `client/art-src/construction-ship-reference-v1/prompt.txt`.

How to use them:

- **One image per prompt.** Attach the reference images listed with the
  prompt, in the order given.
- **Transparency.** Icons, chunks and hull sprites need a transparent
  background. Regenerate any that come back on white or black.
- **Saving.** Save each result under the exact path given. I'll wire them into
  the game. The hull sheets also go through the existing Python
  reconstruction, the same as the other ships.

## 1. Ore and material icons (nine images)

These are square, transparent masters like the current ore icons, which are
1254 × 1254. Save them in `client/art-src/ores-2026-09-16/`. The build already
turns every PNG in that folder into 64 and 128 px icons. No references are
needed.

**`olivine_ore.png`**

```text
Use case: stylized-concept. Asset type: sci-fi strategy game inventory resource icon for Stellar Syndicates. A single centered resource object, polished hard-surface 3D game icon, crisp broad forms readable at 32–64 px, 3/4 view, restrained texture, strong silhouette, studio light from upper left. Object occupies 80% of square canvas. Genuine transparent RGBA background; no scene, ground, border, frame, text, labels, watermark, glow haze, UI or extra props. A compact chunk of dark grey-brown basalt studded with clusters of glassy olive-green olivine crystals; several broad translucent green facets catch the light. Raw stone, not a gem or jewellery.
```

**`bornite_ore.png`**

```text
Use case: stylized-concept. Asset type: sci-fi strategy game inventory resource icon for Stellar Syndicates. A single centered resource object, polished hard-surface 3D game icon, crisp broad forms readable at 32–64 px, 3/4 view, restrained texture, strong silhouette, studio light from upper left. Object occupies 80% of square canvas. Genuine transparent RGBA background; no scene, ground, border, frame, text, labels, watermark, glow haze, UI or extra props. A rugged warm bronze-brown copper ore rock whose broken faces show an iridescent peacock tarnish: patches of deep blue, purple and magenta over the bronze. The bronze body dominates; raw ore, not refined metal.
```

**`ilmenite_ore.png`**

```text
Use case: stylized-concept. Asset type: sci-fi strategy game inventory resource icon for Stellar Syndicates. A single centered resource object, polished hard-surface 3D game icon, crisp broad forms readable at 32–64 px, 3/4 view, restrained texture, strong silhouette, studio light from upper left. Object occupies 80% of square canvas. Genuine transparent RGBA background; no scene, ground, border, frame, text, labels, watermark, glow haze, UI or extra props. A heavy slab of jet-black ilmenite ore built from thick tabular plates with a dull sub-metallic sheen and thin rusty-brown weathering along the edges. Dense dark raw rock, not polished metal.
```

**`iridium_ore.png`**

```text
Use case: stylized-concept. Asset type: sci-fi strategy game inventory resource icon for Stellar Syndicates. A single centered resource object, polished hard-surface 3D game icon, crisp broad forms readable at 32–64 px, 3/4 view, restrained texture, strong silhouette, studio light from upper left. Object occupies 80% of square canvas. Genuine transparent RGBA background; no scene, ground, border, frame, text, labels, watermark, glow haze, UI or extra props. A small, very dense raw ore nugget: dark matte host rock split open to reveal bright white-silver platinum-group metal grains with a faint warm sheen. Heavy and precious-looking, not an ingot or coin.
```

**`helium3_ice.png`**

```text
Use case: stylized-concept. Asset type: sci-fi strategy game inventory resource icon for Stellar Syndicates. A single centered resource object, polished hard-surface 3D game icon, crisp broad forms readable at 32–64 px, 3/4 view, restrained texture, strong silhouette, studio light from upper left. Object occupies 80% of square canvas. Genuine transparent RGBA background; no scene, ground, border, frame, text, labels, watermark, glow haze, UI or extra props. A chunk of dense pale blue-white glacial ice with fine internal fractures, frosted rough edges and a faint lilac tint deep inside. A raw natural ice block; no container, no glow.
```

**`void_crystal.png`**

```text
Use case: stylized-concept. Asset type: sci-fi strategy game inventory resource icon for Stellar Syndicates. A single centered resource object, polished hard-surface 3D game icon, crisp broad forms readable at 32–64 px, 3/4 view, restrained texture, strong silhouette, studio light from upper left. Object occupies 80% of square canvas. Genuine transparent RGBA background; no scene, ground, border, frame, text, labels, watermark, glow haze, UI or extra props. A single jagged obsidian-black crystal shard with unnaturally precise internal geometric facets that reflect deep indigo and teal. Alien and valuable but physical: no glow, aura or energy effects.
```

**`iridium.png`** (refined)

```text
Use case: stylized-concept. Asset type: sci-fi strategy game inventory resource icon for Stellar Syndicates. A single centered resource object, polished hard-surface 3D game icon, crisp broad forms readable at 32–64 px, 3/4 view, restrained texture, strong silhouette, studio light from upper left. Object occupies 80% of square canvas. Genuine transparent RGBA background; no scene, ground, border, frame, text, labels, watermark, glow haze, UI or extra props. Two dense polished silver-white metal ingots stacked crosswise, bright platinum-like surfaces, crisp beveled edges and a small stamped hexagonal mark. Manufactured metal, no raw rock.
```

**`helium3.png`** (refined)

```text
Use case: stylized-concept. Asset type: sci-fi strategy game inventory resource icon for Stellar Syndicates. A single centered resource object, polished hard-surface 3D game icon, crisp broad forms readable at 32–64 px, 3/4 view, restrained texture, strong silhouette, studio light from upper left. Object occupies 80% of square canvas. Genuine transparent RGBA background; no scene, ground, border, frame, text, labels, watermark, glow haze, UI or extra props. A compact spherical cryogenic flask held in a brushed-steel cage frame, frosted pale blue-white surface, a small valve and pressure fitting on top. Manufactured fuel container, no glow.
```

**`exotic_matter.png`** (refined)

```text
Use case: stylized-concept. Asset type: sci-fi strategy game inventory resource icon for Stellar Syndicates. A single centered resource object, polished hard-surface 3D game icon, crisp broad forms readable at 32–64 px, 3/4 view, restrained texture, strong silhouette, studio light from upper left. Object occupies 80% of square canvas. Genuine transparent RGBA background; no scene, ground, border, frame, text, labels, watermark, glow haze, UI or extra props. A small containment capsule: a dark gunmetal frame with brass clamps holding a clear faceted crystal cylinder, inside which a dark iridescent indigo-teal crystalline core floats. Manufactured containment; no glow haze or energy beams.
```

## 2. Belt chunk sprites (three images, plus one optional)

The System View scatters these along a belt. They are square and transparent.

- **Save in:** `client/art-src/belt-chunks-v1/`.
- **Attach as Image 1:** `client/public/art/celestial_sprites/planets/asteroid_belt_chunk.png`, the current chunk. It already serves as the Stony type, so regenerate Stony only if the new set doesn't match it.

Every chunk uses the prompt below. Replace the last line with the rock type
from the table that follows.

```text
Use case: stylized-concept. Asset type: small transparent sprite of asteroid-belt rocks for the System View of Stellar Syndicates, a space strategy game.
Image 1 is the existing belt-chunk sprite. Match its camera, scale, lighting, finish and faint cool rim light exactly; change only the rock type.
Composition: a loose cluster of exactly three irregular asteroid rocks (one large, one medium, one small), centered on a square canvas, the cluster spanning about 45% of the canvas width, top-down three-quarter view, soft light from the upper left.
Background: genuinely transparent PNG with real RGBA alpha; no starfield, dust cloud, glow haze, ground, border, text, labels or watermark.
Rock type: <ROCK TYPE>
```

| File | Rock type line |
| --- | --- |
| `metallic.png` | Dark steel-grey metallic rock with bright silver fracture planes, small metallic glints and a few rust-brown patches; heavier and denser-looking than stony rock. |
| `icy.png` | Blue-white ice-and-rock chunks: frosted pale blue ice with translucent edges, embedded dark grit and fine fractures; no glow. |
| `shattered.png` | Fractured chunks of a broken planet's crust: rust-red and charcoal rock with visible layered strata on one sharp broken face and small metallic veins; jagged, violent fracture lines. |
| `stony.png` (optional) | Grey-brown porous stony rock with pitted surfaces and a few pale chips, the same material as Image 1. |

## 3. Mining Barge (two images)

Make the top-down hull sprite first, then the five-view sheet from it. Save both
in `client/art-src/mining-barge-reference-v1/`.

**Step 1: `mining-barge.png`**, the hull sprite. Attach
`client/art-src/ships-2026-09-08/construction_ship.png` as Image 1.

```text
Use case: stylized-concept.
Asset type: one production-quality transparent raster spaceship hull sprite for Stellar Syndicates, a premium space strategy game.
Input image: Image 1 is an existing Stellar Syndicates hull, attached ONLY as the fleet's style, material and finish reference. Do not copy its silhouette.
Primary request: design a NEW hull as a fresh, distinctive, professionally art-directed hard-surface spaceship. It must read clearly both as a 48–100 pixel galaxy-map sprite and in a closer battle view.
Composition: strict orthographic TOP-DOWN plan view, camera directly overhead at 90 degrees; ship's nose points exactly UP (12 o'clock), aft engines point DOWN. One complete centered ship on a square canvas, approximately 80–84% of canvas height, at least 8% clear padding on every side. No perspective tilt, no side view. The outer silhouette should be purposeful and unmistakable.
Visual style: clean high-end rendered hard-surface game art; crisp machined geometry, readable large armor forms with restrained secondary panel detail. Brushed titanium, cool gunmetal, pale silver armor panels, tiny cyan running lights and compact cyan engine apertures; restrained warm brass hardware. Consistent soft upper-left illumination and good value separation for a dark navy map. Neutral lights, no team-color wash.
Background: genuinely transparent PNG with real RGBA alpha, including gaps between hull parts. No ground plane, black rectangle, painted checkerboard, smoke, outer halo, scene or starfield. Only tiny engine-aperture glow, no long exhaust plumes. Mining tools idle: no beams, sparks, debris or rocks.
Constraints: exactly one hull, no text, numbers, labels, logos, watermark, border, circular badge or UI. No painterly brush marks, crayon edges, excessive micro-greeble noise, organic flesh, toy appearance or recognizable franchise ship. Maximize real native detail without artificially upscaling.

Subject and distinguishing silhouette (takes priority):
A MINING BARGE, a slow, sturdy, unarmed industrial mining ship, NOT a freighter and NOT a fighter. Distinctive FRONT-HEAVY silhouette: a broad forward MINING HEAD with TWO short articulated mining arms, one on each side of the bow, each ending in a heavy cylindrical cutter/laser emitter pointing forward, flanking a central rectangular crusher intake. Behind the head, a squat armored hull with ONE large ribbed ore hopper amidships (a single covered bay, never rows of cargo pods), a small bridge offset to one side, and two modest aft thrusters. About 0.7 width-to-length ratio. Pale silver and gunmetal with industrial ochre hazard striping and scuffed wear on the mining head, small cyan work lights. The silhouette must unmistakably say asteroid miner.
```

**Step 2: `mining-barge-reference.png`**, the five-view sheet. Attach the Step 1
result as Image 1 and
`client/art-src/construction-ship-reference-v1/construction-ship-reference.png`
as Image 2.

```text
Use case: stylized-concept. Asset type: fully textured five-view 2D orthographic spaceship reference sheet for accurate 3D reconstruction in the Stellar Syndicates fleet.
Image 1 is the authoritative ship design. Expand it to exactly FIVE physically consistent views of that SAME ship. Image 2 is only the fleet's reference-sheet presentation and material style; do not copy its ship silhouette.
DESIGN:
MINING BARGE: a slow, sturdy, unarmed industrial mining ship. A broad forward MINING HEAD with exactly TWO short articulated mining arms, one on each side of the bow, each ending in one heavy cylindrical cutter/laser emitter pointing forward, flanking a central rectangular crusher intake. Behind it a squat armored hull with exactly ONE large ribbed ore hopper amidships, a small bridge offset to port on the dorsal side, and exactly TWO cylindrical rear engines side by side at one height. Titanium housing with mustard hazard markings on the mining head, brass joints, modest cyan lights, plated underside with maintenance hatches. No guns, no cargo pods, no extra arms.
LAYOUT: wide 2:1 landscape white sheet, high native detail. Exactly two large vertical plan drawings on the left 58% of the canvas, TOP in first column and BOTTOM in second column, noses UP. Their silhouettes use identical length, width and station positions. Right 42%: FRONT at the top, BACK in the middle, SIDE at the bottom with nose LEFT. Clean charcoal title at upper left and a small uppercase view label beneath each of five drawings. Generous clear separation. No sixth drawing. All entire ships fit inside the sheet.
PROJECTION: true orthographic, parallel projection, ZERO camera pitch on FRONT/BACK/SIDE. FRONT/BACK are shallow cross-sections, not compressed overhead plans. SIDE shows a flat profile, never a visible broad roof or far flank. TOP/BOTTOM have no perspective. Engines point backward; top/side show cylindrical walls and edge-on nozzle rims, NOT exhaust discs rotated toward the camera. Only BACK shows end-on circular exhausts. Keep drive counts and mining-arm counts consistent across every view; hidden far-side items naturally overlap in profile. Do not add unexplained extra engines, arms or guns.
BOTTOM: fully detailed dedicated ventral armor with seams, hatches, cooling grilles and keel texture, same outer shape and openings as TOP. No dorsal bridge pictures printed on the bottom. True holes remain blank white. Front/back/side must also have coherent armor textures, not empty gray blocks.
STYLE: precise colored hard-surface 2D technical concept illustration, clean contours, fine panel lines, pale silver/titanium plates, charcoal gunmetal structure, muted brass hardware, small cyan lights. Mild wear, soft neutral illumination, no strong highlights or cast shadows. Match fleet reference style. No scenery, checkerboard, grid, dimensions, logos, mining beams, plumes or watermark.
Text exactly once each: "MINING BARGE", "TOP", "BOTTOM", "FRONT", "BACK", "SIDE".
```

## 4. Mining Belt Station (two images)

The same two steps. The station has no nose or engines, so its front is the
hauler loading gantry, and that end points up. Save both in
`client/art-src/mining-belt-station-reference-v1/`.

**Step 1: `mining-belt-station.png`**, the sprite. Attach
`client/art-src/ships-2026-09-08/construction_ship.png` as Image 1.

```text
Use case: stylized-concept.
Asset type: one production-quality transparent raster sprite of a stationary space station for Stellar Syndicates, a premium space strategy game.
Input image: Image 1 is an existing Stellar Syndicates hull, attached ONLY as the fleet's style, material and finish reference. Do not copy its silhouette.
Primary request: design a NEW station as a fresh, distinctive, professionally art-directed hard-surface structure. It must read clearly both as a 48–100 pixel map sprite and in a closer view.
Composition: strict orthographic TOP-DOWN plan view, camera directly overhead at 90 degrees; the station's FRONT, its hauler loading gantry, points exactly UP (12 o'clock). One complete centered station on a square canvas, approximately 80–84% of canvas height, at least 8% clear padding on every side. No perspective tilt, no side view. The outer silhouette should be purposeful and unmistakable.
Visual style: clean high-end rendered hard-surface game art; crisp machined geometry, readable large forms with restrained secondary panel detail. Brushed titanium, cool gunmetal, pale silver plates, tiny cyan running lights; restrained warm brass hardware. Consistent soft upper-left illumination and good value separation for a dark navy map. Neutral lights, no team-color wash.
Background: genuinely transparent PNG with real RGBA alpha, including gaps between station parts. No ground plane, black rectangle, painted checkerboard, smoke, outer halo, scene, asteroids or starfield. No docked ships, beams or debris.
Constraints: exactly one station, no text, numbers, labels, logos, watermark, border, circular badge or UI. No painterly brush marks, crayon edges, excessive micro-greeble noise, organic shapes, toy appearance or recognizable franchise design. Maximize real native detail without artificially upscaling.

Subject and distinguishing silhouette (takes priority):
A MINING BELT STATION, a stationary industrial ore depot anchored in an asteroid belt: a station, NOT a ship. Distinctive ROUNDED, RADIAL silhouette: a central octagonal core hub ringed by exactly SIX cylindrical ore silos; TWO docking arms reaching out to port and starboard, each ending in an open barge berth cradle; at the top (front) a long hauler loading gantry with a covered conveyor; exactly FOUR short anchoring booms angled outward toward the corners; small station-keeping thruster pods only, with no main engines and no engine block. About 0.95 width-to-length ratio. Pale silver and gunmetal with ochre hazard markings on the gantry and berths, floodlight fittings and cyan running lights. The silhouette must unmistakably say mining station / ore depot, not a warship or freighter.
```

**Step 2: `mining-belt-station-reference.png`**, the five-view sheet. Attach the
Step 1 result as Image 1 and
`client/art-src/construction-ship-reference-v1/construction-ship-reference.png`
as Image 2.

```text
Use case: stylized-concept. Asset type: fully textured five-view 2D orthographic space-station reference sheet for accurate 3D reconstruction in the Stellar Syndicates fleet.
Image 1 is the authoritative station design. Expand it to exactly FIVE physically consistent views of that SAME station. Image 2 is only the fleet's reference-sheet presentation and material style; do not copy its silhouette.
DESIGN:
MINING BELT STATION: a stationary industrial ore depot, NOT a ship: no main engines. A central octagonal core hub ringed by exactly SIX cylindrical ore silos standing parallel to the core; TWO docking arms reaching to port and starboard, each ending in an open barge berth cradle; at the FRONT a long hauler loading gantry with a covered conveyor; exactly FOUR short anchoring booms angled outward from the lower hull; small station-keeping thruster pods at the four corners. Titanium plating, mustard hazard markings on the gantry and berths, brass joints, floodlight fittings, modest cyan lights, plated underside with maintenance hatches. No guns, no cargo pods, no engine block.
LAYOUT: wide 2:1 landscape white sheet, high native detail. Exactly two large vertical plan drawings on the left 58% of the canvas, TOP in first column and BOTTOM in second column, FRONT (the loading gantry) UP. Their silhouettes use identical length, width and station positions. Right 42%: FRONT at the top, BACK in the middle, SIDE at the bottom with the loading gantry LEFT. Clean charcoal title at upper left and a small uppercase view label beneath each of five drawings. Generous clear separation. No sixth drawing. The entire station fits inside the sheet.
PROJECTION: true orthographic, parallel projection, ZERO camera pitch on FRONT/BACK/SIDE. FRONT/BACK are shallow cross-sections, not compressed overhead plans. SIDE shows a flat profile, never a visible broad roof or far flank. TOP/BOTTOM have no perspective. There are no main engines: thruster pods are small and shown edge-on in TOP and SIDE; BACK shows the rear of the core and silos, never large exhaust discs. Keep silo, arm, boom and thruster counts consistent across every view; hidden far-side items naturally overlap in profile. Do not add unexplained extra silos, arms or engines.
BOTTOM: fully detailed dedicated ventral plating with seams, hatches, cooling grilles and anchoring-boom roots, same outer shape and openings as TOP. No dorsal details printed on the bottom. True holes remain blank white. Front/back/side must also have coherent plating textures, not empty gray blocks.
STYLE: precise colored hard-surface 2D technical concept illustration, clean contours, fine panel lines, pale silver/titanium plates, charcoal gunmetal structure, muted brass hardware, small cyan lights. Mild wear, soft neutral illumination, no strong highlights or cast shadows. Match fleet reference style. No scenery, asteroids, checkerboard, grid, dimensions, logos, beams, plumes or watermark.
Text exactly once each: "MINING BELT STATION", "TOP", "BOTTOM", "FRONT", "BACK", "SIDE".
```

## Not needed yet

- **Belt stage painting.** The belt's own planet-view painting, one per type.
  Until then, the stage is built from the chunk sprites. I'll write these prompts
  when the belt panel gets real art.
- **An orbital Mining Complex.** For a Mining Complex built on a belt. The
  existing `mining_station.png` stands in.
- **A belt icon for the Worlds rows.** The existing `role-mining` icon stands
  in.
- **Pocket glints.** Drawn in code, so no art is needed.
