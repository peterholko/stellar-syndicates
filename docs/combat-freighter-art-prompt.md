# Combat Freighter: art prompts

The Combat Freighter ([blockades.md](blockades.md), "What a raid keeps") is
the raiding freighter that carries a raid's loot home. These prompts gave it
its own hull. **Built Oct 27 2026:** the GLB is
`client/art-src/ship-models/combat-freighter-textured-v1/`, wired in per
[ship-model-integration.md](ship-model-integration.md); it replaced the Tiny
Freighter stand-in.

It follows the other hulls' pipeline: sprite, then five-view sheet, then the
GLB. Save everything in `client/art-src/combat-freighter-reference-v1/`.

**The look**
- **A freighter's hold in a warship's armour:** one compact armoured cargo
  citadel, not the freighters' white pod blocks. It should read as "cargo,
  but armoured" at 48 px.
- **Prize clamps:** four folded grapple arms along the hold, the one shape no
  other hull has.
- **Fast:** a wedge bow and three engines. It outruns every freighter and the
  Corvette; an Interceptor still catches it.
- **Unarmed:** no guns or turrets. It's unarmed in the sim too.
- **Livery:** dark gunmetal armour (the freighters are white) with one broad
  amber stripe down the hold doors. It avoids the free trader's purple and
  the Courier's burgundy.

Size: between the Tiny Freighter (hull 1,500) and the Small Freighter (2,500).
Framing is measured from the shipped freighters: their length spans 90–94% of
the canvas height.

## Step 1: `combat-freighter.png`, the hull sprite

Attach `client/art-src/ships-2026-09-08/interceptor.png` as Image 1.

```text
Use case: stylized-concept.
Asset type: one production-quality transparent raster spaceship hull sprite for Stellar Syndicates, a premium space strategy game.
Input image: Image 1 is an existing Stellar Syndicates hull, attached ONLY as the fleet's style, material and finish reference. Do not copy its silhouette, wings or canopy.
Primary request: design a NEW hull as a fresh, distinctive, professionally art-directed hard-surface spaceship. It must read clearly both as a 48–100 pixel galaxy-map sprite and in a closer view.
Composition: strict orthographic TOP-DOWN plan view, camera directly overhead at 90 degrees; the ship's nose points exactly UP (12 o'clock), its engines point DOWN. One complete centered ship on a square canvas. Its length spans about 90% of the canvas height, with about 5% clear space above the nose and below the engines, centered left to right, matching the existing fleet sprites. No perspective tilt, no side view. The outer silhouette should be purposeful and unmistakable.
Visual style: clean high-end rendered hard-surface game art; crisp machined geometry, readable large forms with restrained secondary panel detail. Dark gunmetal armour plates over brushed titanium structure, tiny cyan running lights and compact cyan engine apertures, a little polished brass hardware. Consistent soft upper-left illumination and good value separation for a dark navy map. Neutral lights, no team-color wash; the amber is paint on the hold doors only, never light or glow.
Background: genuinely transparent PNG with real RGBA alpha, including gaps between hull parts. No ground plane, black rectangle, painted checkerboard, smoke, outer halo, scene or starfield. Only tiny engine-aperture glow, no long exhaust plumes. No cargo containers outside the hull, cables, beams or effects.
Constraints: exactly one ship, no text, numbers, labels, logos, watermark, border, circular badge or UI. No painterly brush marks, crayon edges, excessive micro-greeble noise, organic shapes, toy appearance or recognizable franchise ship.

Subject and distinguishing silhouette (takes priority):
A COMBAT FREIGHTER, a corporate raiding freighter that follows warships into a blockade and carries the plunder home: an armoured cargo hauler, fast for its size. NOT a warship, NOT a fighter, NOT a white pod freighter, NOT a pirate ship. Four readable parts: a sharp sloped WEDGE BOW of thick armour with a narrow armoured bridge slit, no large glass canopy; amidships, the widest part of the ship, ONE compact rectangular ARMOURED CARGO CITADEL with angled armour skirts, its roof sealed by exactly TWO long clamshell hold doors that meet on the centreline, painted with one broad amber stripe running nose to tail across both doors; along the citadel's flanks, exactly FOUR folded GRAPPLE ARMS, two to port and two to starboard, lying flat against the hull with their three-fingered claw tips pointing forward; at the stern, exactly THREE cylindrical engines side by side, the centre one larger, close together on a short armoured engine block. About 0.55 width-to-length ratio. Unarmed: no guns, turrets or missile racks. The silhouette must say armoured cargo hauler built to take loot at a glance.
```

## Step 2: `combat-freighter-reference.png`, the five-view sheet

Attach the Step 1 result as Image 1 and
`client/art-src/construction-ship-reference-v1/construction-ship-reference.png`
as Image 2.

```text
Use case: stylized-concept. Asset type: fully textured five-view 2D orthographic spaceship reference sheet for accurate 3D reconstruction in the Stellar Syndicates fleet.
Image 1 is the authoritative ship design. Expand it to exactly FIVE physically consistent views of that SAME ship. Image 2 is only the fleet's reference-sheet presentation and material style; do not copy its ship silhouette.
DESIGN:
COMBAT FREIGHTER: an armoured corporate raiding freighter, unarmed. A sharp sloped wedge bow of thick armour with a narrow armoured bridge slit. Amidships, the widest part, one compact rectangular armoured cargo citadel with angled armour skirts on both flanks, its roof sealed by exactly TWO long clamshell hold doors meeting on the centreline, one broad amber stripe painted nose to tail across both doors. Exactly FOUR folded grapple arms lie flat along the citadel's flanks, two to port and two to starboard, three-fingered claw tips pointing forward. At the stern, exactly THREE cylindrical engines side by side at one height, the centre one larger, on a short armoured engine block. Dark gunmetal armour plates, brushed titanium structure, a little polished brass hardware, modest cyan lights, a plated underside with a ventral loading hatch beneath the citadel. No guns, turrets, missile racks, containers or extra engines.
LAYOUT: wide 2:1 landscape white sheet, high native detail. Exactly two large vertical plan drawings on the left 58% of the canvas, TOP in first column and BOTTOM in second column, noses UP. Their silhouettes use identical length, width and station positions. Right 42%: FRONT at the top, BACK in the middle, SIDE at the bottom with nose LEFT. Clean charcoal title at upper left and a small uppercase view label beneath each of five drawings. Generous clear separation. No sixth drawing. The entire ship fits inside the sheet.
PROJECTION: true orthographic, parallel projection, ZERO camera pitch on FRONT/BACK/SIDE. FRONT/BACK are shallow cross-sections, not compressed overhead plans. SIDE shows a flat profile, never a visible broad roof or far flank. TOP/BOTTOM have no perspective. Engines point backward; top/side show cylindrical walls and edge-on nozzle rims, NOT exhaust discs rotated toward the camera. Only BACK shows three end-on circular exhausts. Keep the hold door, grapple arm and engine counts consistent across every view; hidden far-side items naturally overlap in profile. Do not add unexplained extra engines, arms or guns.
BOTTOM: fully detailed dedicated ventral plating with seams, the ventral loading hatch, cooling grilles and keel texture, same outer shape as TOP. No dorsal amber stripe or bridge slit printed on the bottom. True holes remain blank white. Front/back/side must also have coherent plating textures, not empty gray blocks.
STYLE: precise colored hard-surface 2D technical concept illustration, clean contours, fine panel lines, dark gunmetal armour, brushed titanium structure, polished brass hardware, one amber door stripe, small cyan lights. Mild wear, soft neutral illumination, no strong highlights or cast shadows. Match fleet reference style. No scenery, checkerboard, grid, dimensions, logos, plumes or watermark.
Text exactly once each: "COMBAT FREIGHTER", "TOP", "BOTTOM", "FRONT", "BACK", "SIDE".
```

## Step 3: the GLB

Rebuild `combat-freighter.glb` from the five-view sheet with the shared
pipeline (`client/art-src/ship-models/mining-fleet-v1/README.md`). Save it as
`client/art-src/ship-models/combat-freighter-textured-v1/`, laid out like the
Mining Barge's folder (GLB, previews, `model-info.json`).
- About 5–6k triangles: a small hull, a little heavier than the Tiny
  Freighter.
- Sockets: `socket.engine.port`, `socket.engine.centre` and
  `socket.engine.starboard` at the nozzles, direction +Z. No weapon sockets.

Wiring it in is code: add `["combat_freighter", "combat-freighter", "Combat
Freighter"]` to `SHIP_MODELS`, drop its `ART_STAND_INS` entry, then follow
[ship-model-integration.md](ship-model-integration.md) (LODs, bake,
hardpoints, tests).
