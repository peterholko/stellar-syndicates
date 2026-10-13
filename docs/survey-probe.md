# Survey Probe

A cheap, crewless, one-way drone that charts a star system.

A system's planets, moons and belts stay unknown until you survey it or own it;
see `docs/system-view.md` § Charting. The Survey Probe makes charting cheap
enough to do widely, without risking Scouts.

## Rules

- **Build:** at a Shipyard I. Alloys 6, Electronics 4 and Fuel 3, in 10 s, so
  it costs well under half a Scout. No research is needed.
- **Hull:** crewless (so no Provisions upkeep), no guns, no armour, no cargo
  and no module slots. Speed 120, just faster than a Scout. Its signature is
  0.3, smaller than any crewed hull.
- **Surveying:** order it like a Scout: select it and click an unsurveyed star,
  or give it a survey operation. It flies there and dwells for 20 seconds, which
  makes it loud, like any survey.
  - When the survey completes, the probe is **spent**. A fleet of nothing but
    that probe is removed, but its report still travels home at light speed and
    charts the system.
  - A fleet that also carries a Scout surveys with the Scout and keeps its
    probes.
- **Risk:** anything that catches it destroys it.

The code is `ShipKind::SurveyProbe` in `crates/sim/src/ship.rs`, its recipe and
yard in `build.rs`, and the gate and spending in `World::resolve_surveys`.

## Art

Until the probe's own art exists, it borrows the Scout's sprite, 3D model,
hardpoints and icons. That comes from the `ART_STAND_INS` entry in
`client/scripts/ship-model-catalog.mjs`. Once real art lands, remove the entry
and add the hull to the catalog lists.

The probe follows the same pipeline as the other hulls: a top-down sprite,
then a five-view sheet, then the Python reconstruction to a GLB. Save both
images in `client/art-src/survey-probe-reference-v1/`. (Oct 9 2026: the
framing is now the fleet's measured 88–90% of canvas height; see
[stand-in-art-prompts.md](stand-in-art-prompts.md).)

**Step 1: `survey-probe.png`**, the hull sprite. Attach
`client/art-src/ships-2026-09-08/scout.png` as Image 1.

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
A SURVEY PROBE, a small CREWLESS robotic drone, NOT a crewed ship: no cockpit, no windows, no crew modules, no guns, no cargo. A compact armored sensor core with ONE large forward sensor dish whose cyan lens faces the nose; FOUR slim antenna booms splayed diagonally like an X from the core; two small folded solar/radiator fins at the sides; and exactly ONE small cylindrical thruster at the tail. About 0.75 width-to-length ratio. Pale silver and gunmetal with a few ochre hazard markings. The silhouette must say robotic probe, clearly smaller and simpler than the Scout.
```

**Step 2: `survey-probe-reference.png`**, the five-view sheet. Attach the Step 1
result as Image 1, and
`client/art-src/construction-ship-reference-v1/construction-ship-reference.png`
as Image 2.

```text
Use case: stylized-concept. Asset type: fully textured five-view 2D orthographic spacecraft reference sheet for accurate 3D reconstruction in the Stellar Syndicates fleet.
Image 1 is the authoritative craft design. Expand it to exactly FIVE physically consistent views of that SAME craft. Image 2 is only the fleet's reference-sheet presentation and material style; do not copy its silhouette.
DESIGN:
SURVEY PROBE: a small crewless robotic drone, NOT a crewed ship: no cockpit, windows, crew modules, guns or cargo. A compact armored sensor core with exactly ONE large forward sensor dish whose cyan lens faces forward; exactly FOUR slim antenna booms splayed diagonally like an X from the core; two small folded solar/radiator fins at the sides; exactly ONE small cylindrical thruster at the tail. Titanium and gunmetal with a few mustard hazard markings, brass joints, modest cyan lights, plated underside with service hatches.
LAYOUT: wide 2:1 landscape white sheet, high native detail. Exactly two large vertical plan drawings on the left 58% of the canvas, TOP in first column and BOTTOM in second column, noses UP. Their silhouettes use identical length, width and station positions. Right 42%: FRONT at the top, BACK in the middle, SIDE at the bottom with nose LEFT. Clean charcoal title at upper left and a small uppercase view label beneath each of five drawings. Generous clear separation. No sixth drawing. The entire craft fits inside the sheet.
PROJECTION: true orthographic, parallel projection, ZERO camera pitch on FRONT/BACK/SIDE. FRONT/BACK are shallow cross-sections, not compressed overhead plans. SIDE shows a flat profile, never a visible broad roof or far flank. TOP/BOTTOM have no perspective. The thruster points backward; top/side show its cylindrical wall and edge-on nozzle rim, NOT an exhaust disc rotated toward the camera. Only BACK shows the end-on circular nozzle. Keep the dish, booms, fins and thruster counts consistent across every view; hidden far-side items naturally overlap in profile. Do not add unexplained extra engines, booms or guns.
BOTTOM: fully detailed dedicated ventral plating with seams, hatches and cooling grilles, same outer shape and openings as TOP. No dorsal details printed on the bottom. True holes remain blank white. Front/back/side must also have coherent plating textures, not empty gray blocks.
STYLE: precise colored hard-surface 2D technical concept illustration, clean contours, fine panel lines, pale silver/titanium plates, charcoal gunmetal structure, muted brass hardware, small cyan lights. Mild wear, soft neutral illumination, no strong highlights or cast shadows. Match fleet reference style. No scenery, checkerboard, grid, dimensions, logos, beams, signals, plumes or watermark.
Text exactly once each: "SURVEY PROBE", "TOP", "BOTTOM", "FRONT", "BACK", "SIDE".
```

The GLB should be the smallest hull in the fleet: about 1.5–3k triangles, with a
socket at the thruster.
