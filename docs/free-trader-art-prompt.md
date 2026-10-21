# Free Trader: art prompts

The traveling merchant ([traveling-merchant.md](traveling-merchant.md)) flies
the Authority Freighter's art as a stand-in. These prompts give it its own
hull: an independent merchant, not an Authority ship.

It follows the other hulls' pipeline: sprite, then five-view sheet, then the
GLB. Save everything in `client/art-src/free-trader-reference-v1/`.

**The look**
- **Market deck:** a wide, striped trading deck amidships, the one shape no
  other hull has. It should read as a stall even at 48 px.
- **Livery:** deep purple and cream stripes with polished brass trim. No
  shipped hull uses purple.
- **It avoids** the Authority's container grid, the corporate pod ladder, the
  Salvage Carrier's patchwork crates, the Courier's burgundy arrowhead and the
  Falcon-style saucer every "free trader" prompt drifts toward.
- **Fast and unarmed:** a streamlined bow and two oversized engines, no guns.
  It's unarmed in the sim too.

Framing is measured from the shipped freighters: their length spans 90–94%
of the canvas height (Authority Freighter y 3–93%, Medium Freighter 2–96%).

## Step 1: `free-trader.png`, the hull sprite

Attach `client/art-src/ships-2026-09-08/construction_ship.png` as Image 1.

```text
Use case: stylized-concept.
Asset type: one production-quality transparent raster spaceship hull sprite for Stellar Syndicates, a premium space strategy game.
Input image: Image 1 is an existing Stellar Syndicates hull, attached ONLY as the fleet's style, material and finish reference. Do not copy its silhouette.
Primary request: design a NEW hull as a fresh, distinctive, professionally art-directed hard-surface spaceship. It must read clearly both as a 48–100 pixel galaxy-map sprite and in a closer view.
Composition: strict orthographic TOP-DOWN plan view, camera directly overhead at 90 degrees; the ship's nose points exactly UP (12 o'clock), its engines point DOWN. One complete centered ship on a square canvas. Its length spans about 90% of the canvas height, with about 5% clear space above the nose and below the engines, centered left to right, matching the existing fleet sprites. No perspective tilt, no side view. The outer silhouette should be purposeful and unmistakable.
Visual style: clean high-end rendered hard-surface game art; crisp machined geometry, readable large forms with restrained secondary panel detail. Brushed titanium, cool gunmetal, pale silver panels, tiny cyan running lights and compact cyan engine apertures; polished warm brass hardware. Consistent soft upper-left illumination and good value separation for a dark navy map. Neutral lights, no team-color wash; the purple is paint on the canopy stripes only, never light or glow.
Background: genuinely transparent PNG with real RGBA alpha, including gaps between hull parts. No ground plane, black rectangle, painted checkerboard, smoke, outer halo, scene or starfield. Only tiny engine-aperture glow, no long exhaust plumes. No docked shuttles, loose cargo, beams or effects.
Constraints: exactly one ship, no text, numbers, labels, logos, watermark, border, circular badge or UI. No painterly brush marks, crayon edges, excessive micro-greeble noise, organic shapes, toy appearance or recognizable franchise ship. Maximize real native detail without artificially upscaling.

Subject and distinguishing silhouette (takes priority):
A FREE TRADER, an independent merchant ship that sells goods in orbit at frontier colonies: privately owned, prosperous and well travelled. NOT an Authority container barge, NOT a corporate pod freighter, NOT a warship, NOT a saucer or disc. Three readable parts: a sleek rounded streamlined bow with a wide wraparound cyan bridge canopy; amidships, the widest part of the ship, a broad rounded-rectangle MARKET DECK roofed by a segmented canopy of exactly EIGHT transverse panels striped in alternating cream and deep royal purple livery, with exactly FOUR small round docking collars on its edges, two to port and two to starboard, for customers' shuttles; behind it a narrow waist and exactly TWO oversized cylindrical engines on short swept-back pylons, set wide apart, the engines of a fast runner. About 0.62 width-to-length ratio. Pale silver and gunmetal with noticeably more polished brass trim than fleet ships, a few slightly mismatched replacement panels and tidy wear, never grime. Unarmed: no guns or turrets. The silhouette must say travelling merchant at a glance.
```

## Step 2: `free-trader-reference.png`, the five-view sheet

Attach the Step 1 result as Image 1 and
`client/art-src/construction-ship-reference-v1/construction-ship-reference.png`
as Image 2.

```text
Use case: stylized-concept. Asset type: fully textured five-view 2D orthographic spaceship reference sheet for accurate 3D reconstruction in the Stellar Syndicates fleet.
Image 1 is the authoritative ship design. Expand it to exactly FIVE physically consistent views of that SAME ship. Image 2 is only the fleet's reference-sheet presentation and material style; do not copy its ship silhouette.
DESIGN:
FREE TRADER: an independent merchant ship, unarmed. A sleek rounded streamlined bow with a wide wraparound cyan bridge canopy. Amidships, the widest part, a broad rounded-rectangle MARKET DECK roofed by a slightly domed segmented canopy of exactly EIGHT transverse panels in alternating cream and deep royal purple livery, with exactly FOUR small round docking collars on its edges, two to port and two to starboard, facing outward. A narrow waist behind it, and exactly TWO oversized cylindrical engines on short swept-back pylons, side by side at one height. Pale silver and titanium plates, charcoal gunmetal structure, polished brass trim, a few mismatched replacement panels, modest cyan lights, plated underside with cargo hatches beneath the market deck. No guns, turrets, cargo pods or extra engines.
LAYOUT: wide 2:1 landscape white sheet, high native detail. Exactly two large vertical plan drawings on the left 58% of the canvas, TOP in first column and BOTTOM in second column, noses UP. Their silhouettes use identical length, width and station positions. Right 42%: FRONT at the top, BACK in the middle, SIDE at the bottom with nose LEFT. Clean charcoal title at upper left and a small uppercase view label beneath each of five drawings. Generous clear separation. No sixth drawing. The entire ship fits inside the sheet.
PROJECTION: true orthographic, parallel projection, ZERO camera pitch on FRONT/BACK/SIDE. FRONT/BACK are shallow cross-sections, not compressed overhead plans. SIDE shows a flat profile, never a visible broad roof or far flank. TOP/BOTTOM have no perspective. Engines point backward; top/side show cylindrical walls and edge-on nozzle rims, NOT exhaust discs rotated toward the camera. Only BACK shows end-on circular exhausts. Keep the canopy panel, docking collar and engine counts consistent across every view; hidden far-side items naturally overlap in profile. Do not add unexplained extra engines, collars or guns.
BOTTOM: fully detailed dedicated ventral plating with seams, cargo hatches, cooling grilles and keel texture, same outer shape and openings as TOP. No dorsal canopy stripes or bridge glass printed on the bottom. True holes remain blank white. Front/back/side must also have coherent plating textures, not empty gray blocks.
STYLE: precise colored hard-surface 2D technical concept illustration, clean contours, fine panel lines, pale silver/titanium plates, charcoal gunmetal structure, polished brass hardware, cream and deep purple canopy livery, small cyan lights. Mild wear, soft neutral illumination, no strong highlights or cast shadows. Match fleet reference style. No scenery, checkerboard, grid, dimensions, logos, docked shuttles, plumes or watermark.
Text exactly once each: "FREE TRADER", "TOP", "BOTTOM", "FRONT", "BACK", "SIDE".
```

## Step 3: the GLB

Rebuild `free-trader.glb` from the five-view sheet with the shared pipeline
(`client/art-src/ship-models/mining-fleet-v1/README.md`). Save it as
`client/art-src/ship-models/free-trader-textured-v1/`, laid out like the Mining
Barge's folder (GLB, previews, `model-info.json`).
- About 6–8k triangles: a mid-size freighter, between the Mining Barge and the
  Authority Freighter.
- Sockets:
  - `socket.engine.port` and `socket.engine.starboard` at the nozzles,
    direction +Z;
  - `socket.docking.1` to `socket.docking.4` at the collars, directions −X/+X.

Wiring it in is code: the client picks this art for fleets flagged `merchant`
instead of the Authority Freighter's.

## As built (Oct 14 2026)

- **Steps 1–2** (Codex): `free-trader.png` and the five-view sheet in
  `client/art-src/free-trader-reference-v1/`. The sprite fills 93.7% of the
  canvas height at 0.60 width to length. The sheet's BACK view is drawn about
  35% wider than TOP; the model follows TOP.
- **Step 3** (Codex): `client/art-src/ship-models/free-trader-textured-v1/`,
  7,872 triangles, two materials, the six sockets, zero glTF errors.
- **LODs:** `node build_models.mjs free-trader` in the (git-ignored)
  `interceptor-lod-experiment-v1` lab, with the Mining Barge's rules: the hull,
  bridge, all eight canopy stripes, the deck frame and the underside stay exact;
  collars and engines lose tessellation. LOD2 is 4,354 triangles and passes
  `verify_geometry.py free-trader`.
- **In the game:** `free_trader` is an art kind (`ShipArtKind` in
  `client/src/shipart.ts`), not a ship kind. The merchant still flies an
  Authority Freighter hull in the sim; `fleetArtKind` gives fleets flagged
  `merchant` this art. Its map sprite keeps the Authority Freighter's
  footprint (`ART_VARIANTS` in `scripts/ship-model-catalog.mjs`). It shows on
  the galaxy map (sprite, then the live LOD2 model up close), in the System
  View, in a planet's orbit strip, and as the contact and fleet-list icon.
- **Not covered:** battle records carry ship kinds only, so a merchant in a
  battle would still draw as an Authority Freighter.

