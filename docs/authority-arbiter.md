# Authority Arbiter (art brief)

The Terran Charter Authority's enforcement capital ship. It comes for
corporations that defy the Authority by refusing its taxes and fees or by
declaring independence.

Nothing in the game uses this hull yet. Today, the Authority sends six
Corvettes every 7 minutes against a Proscribed corporation's system
(`tca::TCA_ENFORCEMENT_SHIPS`, `World::enforcement_ai`). The Arbiter is meant
to lead those expeditions.

## Current art pass

The current review asset is in
`client/art-src/ship-models/authority-arbiter-textured-v1/`. Dark armor is the
lead enforcement finish; ivory remains available for comparison. The later
user requests replace the original lance with a capital mass driver and add
eight secondary twin mounts and eight paired point-defense mounts.

The top-view art pass recesses the mass driver between the prow blades,
restores broad gold spine crowns and a bold gold halo, and replaces projected
ship pictures with clean mapped material panels. Both finishes have 19,388
triangles and 25 render sections. The dark GLB is 1.91 MB, the ivory GLB
1.54 MB. All 45 weapon tips use the battle-compatible `socket.weapon.*`
namespace; the actual hardpoint extractor finds them and the four engines.

The asset README includes validation and texture-generation provenance.
`previews/top-design-review.png` compares the approved sprite, previous model
and revised dark model. Transparent dark top sprites are staged under
`sprites/` for art review; game integration and the final WebGL sprite bake
have not been performed.

## Original design intent and reference prompts

- **The law arriving.** It should feel imposing, orderly and ceremonial, never
  piratical or spiky.
- **Authority materials:** the Freighter's and the Nexus's ivory ceramic and
  silver armour, dark navy-graphite structure, restrained antique gold and
  warm amber windows.
- **The emblem as architecture.** Seen from above, the Authority seal
  (a thin ring crossed by three slender spires of staggered lengths) becomes
  the hull itself: a thin circular halo ring around the midship, crossed by
  three parallel dorsal spines.
- **An enforcer's kit:**
  - one spinal "writ" lance at the bow;
  - six twin turrets on the flanks;
  - the halo, an interdiction projector that stops defiant fleets escaping;
  - a ventral hangar for boarding and seizure craft.

## The art pipeline

Same as every hull:

1. A top-down sprite.
2. A five-view sheet.
3. A Python reconstruction to a GLB.

Save both images in `client/art-src/authority-arbiter-reference-v1/`. Expect
about 12–20k triangles in the GLB, in the Dreadnought's range, with sockets at
the six turrets, the lance tip and the four engines.

**Step 1: `authority-arbiter.png`**, the hull sprite. Attach as references:

1. Image 1: `client/art-src/ships-2026-09-08/authority_freighter.png`.
2. Image 2: `client/art-src/authority-v2/terran-charter-authority-seal.png`.
3. Image 3: `client/art-src/ships-2026-09-08/dreadnought.png`.

```text
Use case: stylized-concept.
Asset type: one production-quality transparent raster spaceship hull sprite for Stellar Syndicates, a premium space strategy game.
Input images (references only, never copy a silhouette): Image 1 is the Terran Charter Authority's existing freighter — use it ONLY for the Authority's materials, colors and finish. Image 2 is the Authority's emblem, a thin orbital ring crossed by three slender parallel spires of staggered heights — use it ONLY as the motif the new hull's architecture expresses. Image 3 is an existing capital warship of the fleet — use it ONLY for capital-ship scale, detail density and rendering finish.
Primary request: design a NEW hull: the AUTHORITY ARBITER, the Terran Charter Authority's enforcement capital ship, sent against corporations that refuse its taxes or declare independence. A fresh, distinctive, professionally art-directed hard-surface warship. It must read clearly both as a 48–120 pixel galaxy-map sprite and in a closer battle view.
Composition: strict orthographic TOP-DOWN plan view, camera directly overhead at 90 degrees; ship's nose points exactly UP (12 o'clock), aft engines point DOWN. One complete centered ship on a square canvas, approximately 80–84% of canvas height, at least 8% clear padding on every side. No perspective tilt, no side view. The outer silhouette should be purposeful and unmistakable.
Visual style: clean high-end rendered hard-surface game art; crisp machined geometry, readable large armor forms with restrained secondary panel detail. Authority materials: pale ivory ceramic and silver armor plates, dark navy-graphite structure, restrained antique-gold trim and fittings, warm amber bridge windows and a few sparse cyan running lights. Consistent soft upper-left illumination and good value separation for a dark navy map. Neutral lights, no team-color wash.
Background: genuinely transparent PNG with real RGBA alpha, including gaps between hull parts. No ground plane, black rectangle, painted checkerboard, smoke, outer halo glow, scene or starfield. Only tiny engine-aperture glow, no long exhaust plumes. All weapons idle: no beams, shots, impact flashes, battle debris or escorts.
Constraints: exactly one hull, no text, numbers, labels, logos, insignia decals, watermark, border, circular badge or UI. No painterly brush marks, crayon edges, excessive micro-greeble noise, organic flesh, toy appearance, pirate spikes or recognizable franchise ship. Maximize real native detail without artificially upscaling.

Subject and distinguishing silhouette (takes priority):
THE AUTHORITY ARBITER, an imposing, symmetrical, cathedral-like capital warship: austere, orderly and ceremonial — the law arriving, never a pirate. A long armored ivory hull about 0.42 width-to-length ratio. Its signature is the Authority emblem expressed as architecture when seen from above: a THIN CIRCULAR HALO RING, clearly wider than the hull, encircles the midship on slim struts (the interdiction projector), and THREE slender parallel dorsal spine-ridges of staggered lengths run fore-and-aft straight through the ring — the right spine longest, the middle slightly shorter, the left shortest. At the bow, ONE long spinal lance projects forward between two armored prow plates. Exactly SIX heavy twin-barrel turrets, three per side, in two neat rows along the flanks. A tall narrow command tower with warm amber windows stands just aft of the ring. Exactly FOUR large cylindrical engines side by side in one row at the stern under white armored cowls. Restrained antique-gold trim along the ring and spines. The silhouette must unmistakably say Authority capital ship, distinct from every corporate and pirate hull.
```

**Step 2: `authority-arbiter-reference.png`**, the five-view sheet. Attach:

1. Image 1: the Step 1 result.
2. Image 2:
   `client/art-src/authority-freighter-reference-v1/authority-freighter-reference.png`.

```text
Use case: stylized-concept. Asset type: fully textured five-view 2D orthographic spaceship reference sheet for accurate 3D reconstruction in the Stellar Syndicates fleet.
Image 1 is the authoritative ship design. Expand it to exactly FIVE physically consistent views of that SAME ship. Image 2 is only the Authority's reference-sheet presentation and material style; do not copy its ship silhouette.
DESIGN:
AUTHORITY ARBITER: the Terran Charter Authority's enforcement capital ship — imposing, symmetrical, cathedral-like, austere and ceremonial. A long armored ivory ceramic and silver hull over dark navy-graphite structure. Exactly ONE thin circular halo ring, wider than the hull, encircles the midship on exactly FOUR slim struts, lying flat in the ship's horizontal plane; exactly THREE slender parallel dorsal spine-ridges of staggered lengths run fore-and-aft straight through the ring (right longest, middle slightly shorter, left shortest). At the bow exactly ONE long spinal lance between two armored prow plates. Exactly SIX heavy twin-barrel turrets, three per side, in two neat rows along the flanks. One tall narrow command tower with warm amber windows just aft of the ring. Exactly FOUR large cylindrical engines side by side at one height at the stern, under white armored cowls. A long ventral hangar bay with closed armored doors for boarding and seizure craft. Restrained antique-gold trim on the ring, spines and prow, brass fittings, sparse cyan running lights. No textual logos or insignia decals.
LAYOUT: wide 2:1 landscape white sheet, high native detail. Exactly two large vertical plan drawings on the left 58% of the canvas, TOP in first column and BOTTOM in second column, noses UP. Their silhouettes use identical length, width and station positions. Right 42%: FRONT at the top, BACK in the middle, SIDE at the bottom with nose LEFT. Clean charcoal title at upper left and a small uppercase view label beneath each of five drawings. Generous clear separation. No sixth drawing. All entire ships fit inside the sheet.
PROJECTION: true orthographic, parallel projection, ZERO camera pitch on FRONT/BACK/SIDE. FRONT/BACK are shallow cross-sections, not compressed overhead plans; the halo ring appears there as a thin horizontal bar wider than the hull. SIDE shows a flat profile, never a visible broad roof or far flank. TOP/BOTTOM have no perspective. Engines point backward; top/side show cylindrical walls and edge-on nozzle rims, NOT exhaust discs rotated toward the camera. Only BACK shows end-on circular exhausts. Keep the ring, struts, spines, turret, lance and engine counts consistent across every view; hidden far-side items naturally overlap in profile. Do not add unexplained extra engines, turrets or rings.
BOTTOM: fully detailed dedicated ventral armor with seams, hatches, cooling grilles, keel texture and the closed hangar doors, same outer shape and openings as TOP; the halo ring and its struts are visible from below too. No dorsal tower, spines or turret pictures printed on the bottom. True holes remain blank white. Front/back/side must also have coherent armor textures, not empty gray blocks.
STYLE: precise colored hard-surface 2D technical concept illustration, clean contours, fine panel lines, pale ivory and silver plates, charcoal-navy structure, muted antique gold and brass hardware, warm amber windows, small cyan lights. Mild wear, soft neutral illumination, no strong highlights or cast shadows. Match the Authority reference style. No scenery, checkerboard, grid, dimensions, logos, weapons fire, plumes or watermark.
Text exactly once each: "AUTHORITY ARBITER", "TOP", "BOTTOM", "FRONT", "BACK", "SIDE".
```
