# Season art (R10–R14): prompts

New art for the season, pirate pressure, the colony charter and captain gear.
Every piece is something the code will show. Until it lands, each feature runs
on stand-ins: a CSS chip, an existing glyph, or the old pirate silhouettes.

Framing numbers are measured from the approved art each piece sits beside
(see the art-brief-framing rule in `living-terraces-art-prompt.md`).

| # | Asset | Used for | Count | Folder | Priority |
|---|---|---|---|---|---|
| 1 | Captain gear icons | R14 Officers page, armory, Signal finds | 8 | `client/art-src/captain-gear-v1/` | Required |
| 2 | Map glyph sheet 5 | Gate site, relics, pirate assaults, derelicts, season chip | 6 on one sheet | `client/art-src/galaxy-map-glyphs-v1/` | Required |
| 3 | The Gate | R10 gate project at the Hub, five build stages | lineup + 5 sprites (+ optional sheet) | `client/art-src/season-gate-v1/` | Required |
| 4 | Pirate warships of the line | R11 heavy pirate hulls on the map | 2 sprites (+ optional sheets) | `client/art-src/npc-ships/` | Required |
| 5 | Story illustrations | New colony (R12), the gate opens (season end), relics awaken (week 4) | 3 | `client/art-src/story-events-v1/` | 1 required, 2 recommended |
| 6 | Optional icons | Season champion medal, colony charter licence | 2 | `client/art-src/ui-icons/raster-masters/` | Optional |

No new art for: R13 goal ladders (text), derelict colonies on the planet stage
(a desaturate/darken filter), the hall of fame (titles plus the medal icon), and
pirate heavy hulls in the battle theater (they use the destroyer and cruiser
models until the optional pirate GLBs exist).

---

## 1 · Captain gear icons (8)

Captains wear up to two pieces of gear. Each piece raises one attribute: Field
grade by 1, Relic grade by 2. The icons sit on the Officers page beside the
captain portraits, in the armory list and on Signal reports, at 64 px (panel)
and 128 px.

Match the module icons (`client/art-src/ui-icons/raster-masters/module-*.png`):
1254 × 1254 RGBA masters in an elevated three-quarter view. The measured
subjects span 72–80% of the canvas width and 56–68% of its height, centred.
Attach `module-mass-driver.png` as Image 1 for style only.

Generate one image per icon. Use the shared prompt, replacing `{NAME}` and
`{SUBJECT}` with that icon's entry below. Save each as
`captain-gear-v1/<slug>.png`.

```text
Use case: stylized-concept. Asset type: one transparent game UI item icon for Stellar Syndicates, a premium dark sci-fi strategy game. It joins an existing icon family; Image 1 is a member of that family, attached ONLY for style, material, camera and finish. Do not copy its subject.
Subject: {NAME}: {SUBJECT}
This is a captain's personal kit, a portable object a starship captain carries or wears on duty. It is NOT a ship module, NOT a weapon, NOT a building and NOT a vehicle. Its size reads as hand-held or desk-sized.
Camera and framing: the same elevated three-quarter view as Image 1, light from the upper left. One complete object, centred on a square 1254 x 1254 canvas. It spans about 72-80% of the canvas width and 56-68% of its height, like the family's measured icons. Bold, simple silhouette that still reads at 64 px; a few large readable forms, restrained small detail.
Style: crisp, professionally rendered hard-surface 3D game icon, no painterly strokes. Charcoal gunmetal, worn steel edges, small panel lines.
GRADE:
- FIELD grade (+1): practical issue kit. Charcoal gunmetal and steel, restrained cyan indicator lights, one small amber accent, mild wear. Rugged and functional.
- RELIC grade (+2): the same kind of object, rare and ancient: aged gold and brass filigree inlaid in dark gunmetal, one violet precursor crystal set at its heart with a soft violet inner light, a few cyan details. Ornate but still a tool, slightly grander than its Field twin. The violet light stays inside the object; no outer halo.
Background: genuinely transparent PNG with real alpha; no floor, plinth, shadow plate, smoke, starfield or frame.
Constraints: exactly one object, no text, letters, numbers, labels, logos, insignia lettering, watermark, border, badge, circle behind it or UI. No hands, people, faces or skulls. No glow halos, sparks or beams.
```

| Slug | Name | Attribute · grade | `{SUBJECT}` |
|---|---|---|---|
| `tactical-datacore` | Tactical Datacore | Command · Field | A ruggedized command datacore: a thick armored data cartridge seated half-way into a compact field rack, with a small cyan tactical holo-lens on top showing a few abstract wedge shapes. Side grip handles, a row of small status lights. Field grade. |
| `admirals-datacore` | Admiral's Datacore | Command · Relic | The same command datacore made for a fleet admiral: an elongated cartridge in a gold-filigreed gunmetal cradle, a violet precursor crystal set into the cartridge's face, a slim brass holo-lens crown above it. Relic grade. |
| `astrogation-suite` | Astrogation Suite | Navigation · Field | A navigator's astrogation slate: a heavy tablet in a gunmetal frame with a fold-out cyan star-chart projector arm and two small calibration dials. Field grade. |
| `pathfinders-astrolabe` | Pathfinder's Astrolabe | Navigation · Relic | An ancient pathfinder's astrolabe: three thin interlocking gold-and-brass rings around a floating violet crystal core, mounted on a small dark gunmetal base. Relic grade. |
| `survey-kit` | Survey Kit | Fieldcraft · Field | A field survey kit: a compact armored case opened at an angle, holding a handheld scanner wand with a cyan emitter tip and three small sample vials in clips. Field grade. |
| `precursor-lens` | Precursor Lens | Fieldcraft · Relic | A precursor survey lens: a large faceted violet crystal lens held in a dark weathered stone-and-gold mount with a short grip, faint violet light in the crystal. Relic grade. |
| `quartermasters-ledger` | Quartermaster's Ledger | Logistics · Field | A quartermaster's ledger: a thick armored data ledger, half open, with a cyan manifest screen inside and a small cargo-crate emboss on its cover clasp. Field grade. |
| `chandlers-seal` | Chandler's Seal | Logistics · Relic | A ship chandler's seal: a heavy brass-and-gold seal stamp with an octagonal face, a violet crystal pommel and a short chain. Next to it lies one small sealed charter tab under a wax-like violet seal. Relic grade. |

Keep each Field/Relic pair recognisably the same kind of object. Across the four
attributes the silhouettes must differ: cartridge-and-rack, slate or rings, case
or lens, ledger or stamp.

---

## 2 · Galaxy map glyph sheet 5 (6 glyphs)

The fifth sheet of the existing set (`galaxy-map-glyphs-v1/`): white
silhouettes on black, drawn into the map's coloured markers.

Attach `01-exploration-sites.png` as Image 1 (the style master). Save as
`galaxy-map-glyphs-v1/05-season.png`.

Each glyph must not be confused with an existing one:
- No pylons and C-ring: that's the megaproject.
- No obelisk or blades: the precursor artifact.
- No rocks: the asteroid cluster.
- No shields or crossed swords: escort, patrol and combat objective.
- No wings and star: the regional council.

```text
Use case: logo-brand.
Asset type: matched raster icon atlas for the galaxy map of a sci-fi strategy game. Image 1 is SHEET 1 of this set: match its exact style, weight, line thickness and binary palette.
OUTPUT: Exactly 1536 pixels wide by 1024 pixels high, landscape 3:2. Opaque pure BLACK (#000000) canvas.
LAYOUT: Exactly SIX distinct glyphs, in a precise 3-column by 2-row grid with no visible grid. Each cell is exactly 512x512. Cell centers are (256,256), (768,256), (1280,256), (256,768), (768,768), (1280,768). Each complete glyph fits within about 332x332 centered in its cell (65% of cell width), with ample completely black space between glyphs. Do not merge cells or add decorative filler.
STYLE: Each glyph is a solid pure WHITE (#FFFFFF) silhouette with any interior details cut out in pure BLACK. A strict two-color binary image: white ink and black void ONLY, no gray or off-white pixels, no anti-aliased gray edges. Bold geometric hard-surface science-fiction shapes; clean straight cuts, chamfered corners, simple smooth curves. Thick filled shapes, not thin line illustrations. Same apparent weight as Image 1, with major stems, ring bands and solid bars about 48 pixels thick and negative-space channels at least 32 pixels wide. Very sparse interior detail. The symbols must remain recognizable at 16x16.
VIEW: Strictly flat 2D top-down or front-on; zero perspective, isometric view, volume, 3D, shading, gradients, glow, texture, noise or wear.
NO TEXT: No text, letters, numbers, labels, titles, logo words, watermark, frames, grid lines, enclosing icon circles or borders. Rings and arcs are allowed only where they are part of the subject, never as a generic badge. Draw only the six requested subjects in the requested positions.

SHEET 5 of 5 — THE SEASON.
Subjects in reading order:
TOP LEFT: A jump gate UNDER CONSTRUCTION. One thick circular ring broken into exactly THREE equal arcs with three clear black gaps between them. A small blocky construction crane arm reaches in from outside the ring and bridges the gap at the upper right. No pylons beside the ring and no spokes.
TOP MIDDLE: The same jump gate COMPLETE AND OPEN. The same thick ring, now whole, around a solid white core disc with a bold black FOUR-POINTED STAR cut out of its centre. No spokes, no hub, no pylons.
TOP RIGHT: A RELIC: one tall, narrow, vertical faceted crystal shaped like an elongated diamond, with a single black vertical facet line down its middle and one short diagonal facet cut. Around it, ONE broken orbit made of two thick arc segments, with one small square chip riding the orbit on each side. No base, no blades, no rocks.
BOTTOM LEFT: A PIRATE ASSAULT: a scythe-winged raider silhouette diving DOWN (nose pointing to 6 o'clock), with long curved swept-back wing blades, and exactly THREE short horizontal speed bars stacked above its tail. No shield, no crossed blades, no skull.
BOTTOM MIDDLE: A DERELICT COLONY: a half-dome habitat on a short flat base, split by one bold zig-zag black crack running from the dome's top to its base. Beside it, a single antenna mast snapped and bent over at an angle.
BOTTOM RIGHT: The SEASON EMBLEM: a whole, thick jump-gate ring (the same ring as top middle, smaller, with an empty black centre) held between two symmetrical laurel branches, each of exactly FIVE simple pointed leaves, which curve up from the bottom and stop below the ring's top.
```

---

## 3 · The Gate (R10)

**What it is:** in week 6 the Terran Charter Authority opens a gate project at
the Hub. Each syndicate builds its own gate in a gate field just outside the
Hub's no-combat bubble, in ten levels fed by goods hauled across the galaxy.
The first to finish opens a passage to a new galaxy, which ends the season.

**Visual brief:** the Market Hub (the Authority Nexus) is a natural violet
wormhole held open by a frame. The season gate is the opposite: a fully
constructed Authority ring that, when finished, opens on a brilliant white-gold
aperture.
- Ivory ceramic armor, machined brass inlay, graphite structure and cyan running
  lights, so it reads as the same Authority as the Nexus.
- Built visibly in segments, so each stage shows progress.
- No franchise look: no Stargate chevrons or dialling ring, no Mass Effect relay
  arms.

| Stage | Gate levels | What shows |
|---|---|---|
| 1 | 1–2 | Four anchor pylons and a skeletal half-ring of bare truss, one construction tender clamped on |
| 2 | 3–5 | The whole ring as truss; one third clad in ivory armor segments |
| 3 | 6–8 | The ring fully clad; eight field-emitter modules mounted, dark |
| 4 | 9 | Complete; emitters lit cyan; a thin white-gold charging film across the aperture |
| 5 | 10 (winner) | Open: a blazing white-gold aperture, light kept inside the ring |

### Step 1 · Lineup

```text
Use case: stylized-concept. Asset type: design lineup sheet for a five-stage construction progression of one megastructure in the dark sci-fi strategy game Stellar Syndicates.
Show FIVE images of the SAME jump gate in one row on a plain white 3:1 landscape sheet, left to right, each the next construction stage. All five use the SAME camera: a three-quarter view tilted back about 37 degrees from top-down, seen from the near (bottom) edge, light from the upper left, and ONE consistent scale, so every stage has the same ring diameter. Every stage is fully visible with clear space between them.
SUBJECT: the Terran Charter Authority's new jump gate: one large circular ring built from EIGHT equal armored segments, its radial width about 14% of its radius, with the aperture filling the rest. FOUR slender anchor pylons stand just outside the ring on the diagonals, linked to it by short struts. Ivory ceramic armor plates with machined brass inlay over graphite truss, small cyan running lights. The same family as a premium Authority station: precise, civic, monumental, not military.
STAGES:
1. FOUNDATION: the four anchor pylons complete; only HALF of the ring exists, as bare graphite truss arcs with no armor. ONE small boxy construction tender clamped onto the end of the truss arc, with a short crane arm.
2. FRAME: the whole ring exists as bare graphite truss; the upper third of it is clad in ivory armor segments with brass seams.
3. CLAD: the whole ring is clad in ivory armor; EIGHT field-emitter modules (squat brass-and-cyan cylinders) mounted around its inner edge, all DARK.
4. CHARGING: identical to stage 3, but all eight emitters glow cyan and a thin translucent white-gold film stretches across the aperture.
5. OPEN: the finished gate; the aperture is a blazing white-gold vortex of light with a bright core, its light held tightly inside the ring; cyan emitters at full power. No beams leaving the gate.
STYLE: precise colored hard-surface 3D game concept rendering, clean contours, fine panel lines, mild wear, soft neutral illumination, no harsh shadows. Each silhouette must read clearly from above at small size.
No stars, nebula, planets, ships (except the one construction tender at stage 1), people, scale bars, logos or watermark. No franchise design: no chevrons or dialling ring, no Mass Effect relay arms, no Death Star.
Text exactly once each, small and charcoal, centered under each stage: "FOUNDATION", "FRAME", "CLAD", "CHARGING", "OPEN".
```

### Step 2 · Map sprite, one per stage

Attach the approved lineup crop of that stage as Image 1. Five images, saved as
`season-gate-v1/gate-stage-1.png` … `gate-stage-5.png`. The map draws them
top-down, beside the Nexus. The stations' measured sprites fill 82–91% of the
canvas width (Star Fortress 91%), so the ring's outer diameter, pylons
included, spans 90%.

```text
Use case: stylized-concept. Asset type: one transparent top-down map sprite of a megastructure for Stellar Syndicates, a premium space strategy game.
Image 1 is the authoritative design of this construction stage. Redraw that SAME stage, unchanged, as a strict orthographic TOP-DOWN plan view: camera directly overhead, zero tilt.
{STAGE}
Composition: square canvas, 1254 x 1254. The ring's outer diameter, anchor pylons included, spans about 90% of the canvas width, centred, with equal margins. Keep the same segment, pylon, emitter and tender counts as Image 1.
Readability: it must still read at 48 pixels wide: big clear shapes, strong value separation against a very dark navy map, restrained small detail.
Visual style: clean, high-end rendered hard-surface game art; crisp machined geometry, soft even light from the upper left. Ivory ceramic armor, machined brass inlay, graphite truss, small cyan running lights. Glow only where the stage has it (emitters, aperture), kept tight inside the ring.
Background: genuinely transparent PNG with real RGBA alpha, including the empty aperture of unfinished stages (true holes, no fill). No ground plane, starfield, scene, halo or outer glow.
Constraints: exactly one structure, no text, numbers, labels, logos, watermark, border, grid or UI; no other ships except the stage-1 tender.
```

`{STAGE}` per stage: copy that stage's line from the Step 1 prompt.

### Step 3 · Optional: five-view sheet for a 3D model

Only if the gate should become a GLB like the Nexus (rebaked with the hub
pipeline in `docs/hub-model.md`): run the station reference-sheet prompt from
`client/art-src/defense-stations-v1/prompts.md` §2 on stage 5, with
`{NAME}` = `SEASON GATE` and `{DESIGN}` = the stage-5 line above. Stages 1–4 can
then be produced from the model by hiding segments, emitters and the aperture.

---

## 4 · Pirate warships of the line (2)

R11 sends pirate destroyers from week 4 and cruisers from week 6. On the map
every pirate fleet borrows one of three small raider silhouettes
(`render.ts` `fleetMarker`). These two heavy hulls let a pirate line fleet read
as one, and fill the same gap for the depot and stronghold garrisons today.

Match the pirate sprites (`client/art-src/npc-ships/pirate-corsair-chroma.png`
as Image 1, style only):
- 1254 × 1254, top-down, nose up.
- Pure chroma-green background, keyed later.
- The measured hulls fill 92–94% of the canvas height and 95–97% of its width
  (wingtips).

Save as `npc-ships/pirate-reaver-chroma.png` and
`npc-ships/pirate-warbarge-chroma.png`.

```text
Use case: stylized-concept.
Asset type: one production-quality spaceship hull sprite for Stellar Syndicates, a premium space strategy game, on a chroma-key background.
Input image: Image 1 is an existing pirate hull, attached ONLY as the pirate fleet's style, material and finish reference. Do not copy its silhouette.
Primary request: design a NEW pirate warship of the line, clearly heavier and more dangerous than Image 1, that reads as a capital-class threat both as a 40-90 pixel galaxy-map sprite and in a closer view.
Composition: strict orthographic TOP-DOWN plan view, camera directly overhead; the nose points exactly UP and the engines point DOWN. One complete centred craft on a square 1254 x 1254 canvas. Its length spans about 92% of the canvas height with even margins above the nose and below the engines.
Visual style: the pirate hull culture of Image 1: blackened gunmetal with layered salvaged plates, rust streaks, rivets, mismatched repair patches in burnt orange and worn mustard, dull bronze piping, amber-lit RECESSED engine mouths with no exhaust plumes. Hard-surface game art, crisp readable large forms, restrained secondary detail, soft upper-left light, strong value separation for a dark navy map.
Background: one flat pure chroma green (#00FF00) filling the whole canvas, with no shadows, gradients or green anywhere on the ship.
Constraints: exactly one craft; no text, numbers, labels, logos, skulls, insignia, watermark, border or UI; no weapons fire, beams or smoke; no recognizable franchise design.

Subject and distinguishing silhouette (takes priority):
{SUBJECT}
```

| Slug | Class | `{SUBJECT}` |
|---|---|---|
| `pirate-reaver` | Destroyer | A PIRATE REAVER, destroyer class: a long, narrow armored blade of a hull, about 0.45 width-to-length ratio, built around ONE heavy spinal mass driver whose barrel runs out past the nose. Two short scythe-curved armored fins at mid-hull; one armored sponson on each flank, each with a rack of FOUR torpedo tubes. THREE recessed amber engines across the stern, the middle one set farther back. Patched plating and one long burnt-orange stripe down the port side. |
| `pirate-warbarge` | Cruiser | A PIRATE WARBARGE, cruiser class: a captured heavy freighter turned warship, about 0.7 width-to-length ratio. A broad boxy hull wrapped in welded slab armor; TWO heavy twin-barrel turrets on raised barbettes at the bow, a row of THREE smaller broadside turrets down each side, and its old cargo spine refitted as a long missile rack of open launch cells. FOUR recessed amber engines in a row across the stern. Mismatched orange and mustard patches, a scavenged armored bridge set off-centre. |

**Optional 3D:** for battle-theater models, run the five-view reference prompt
in `client/art-src/ship-models/pirate-fleet-v1/prompts.json` on each approved
sprite, with titles `PIRATE REAVER` and `PIRATE WARBARGE`, then build the GLBs
with that folder's pipeline.

---

## 5 · Story illustrations (3)

Shown in the event pop-ups, in the format of the founding prologue and the
charter events: one 1672 × 941 opaque master per scene. Attach
`client/art-src/founding-intro-v1/first-landing.png` as Image 1 (style only).
Save each under its slug in `story-events-v1/` and record the prompt in its
`prompts.json`, then run `npm run assets:derive`.

Shared prompt; replace `{REQUEST}`, `{SCENE}`, `{SUBJECT}` and `{MOOD}`:

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Composition/framing: one opaque wide landscape 16:9 illustration, exactly 1672 x 941 pixels. Strong readable focal point, essential subjects within the central 75% of the frame, comfortable edges for responsive display. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: {REQUEST}
Scene/backdrop: {SCENE}
Subject: {SUBJECT}
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: {MOOD}
Constraints: absolutely no text, letters, numbers, logos, labels, title, watermark, borders, captions or fake interface. No readable markings, brand marks, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```

### `new-colony` (R12, required)

Shown when a corporation lands a new colony under a fresh charter.

- **Request:** a corporation's colony ship sets down on a newly chartered world, the start of a second settlement far from home.
- **Scene:** a broad rocky plateau on an untouched world at first light, under an unfamiliar sky with a ringed gas giant low on the horizon and one bright distant star (home). Thin mist in the valleys below, sparse hardy vegetation between the rocks.
- **Subject:** the white colony ship, the same design as in Image 1, a few metres above the ground on final descent. Its landing struts are extended and its downwash raises a ring of dust. Two small survey drones with cyan lights fly ahead of it over the landing zone, and four ground beacons with amber lights mark the zone. One planted pole stands at the plateau's edge with a small plain pennant (no emblem, no text). No people yet.
- **Mood:** cool dawn light with warm amber beacons; quiet, hopeful, the scale of a new beginning.

### `gate-opens` (season end, recommended)

Shown to everyone when a syndicate finishes its gate and the season ends.

- **Request:** the finished Authority jump gate opens for the first time, near the galaxy's central market hub, ending the season.
- **Scene:** deep space near the Authority Nexus: a violet wormhole held in a slim ivory ring sits small in the far background. In the middle distance, several other jump gates stand unfinished at different stages (bare truss arcs, half-clad rings) in a loose field.
- **Subject:** in the foreground, the finished gate, a large ring of eight ivory-and-brass armored segments with cyan field emitters (as in the Gate lineup, stage 5). Its aperture erupts into a blazing white-gold vortex with a bright core. A broad crowd of starships from many corporations hangs back in a respectful arc, small silhouettes catching the gold light: freighters, escorts and a few heavy warships. The gate's light washes over their hulls.
- **Mood:** awe and triumph; the white-gold aperture is the only strong light, against cool blue darkness.

### `relics-awaken` (week 4, recommended)

Shown when the exotic nodes wake as relics.

- **Request:** the galaxy's exotic nodes awaken as relics, and pirates move to claim them.
- **Scene:** a remote star system with a dim red sun. A shattered moonlet drifts in the foreground, its broken interior exposed.
- **Subject:** inside the moonlet's broken interior, a tall elongated violet crystal relic has just woken, its facets glowing with a cold violet light that lights the jagged rock walls. In the middle distance, three pirate warships in the blackened, rust-patched, orange-striped pirate style, with amber engines, close in from the dark. Their silhouettes are rim-lit violet by the relic.
- **Mood:** ominous and wondrous; violet relic light against the red sun and black space; a contest about to begin.

---

## 6 · Optional icons (2)

Both in the module-icon family (§1 framing: 1254 × 1254 RGBA, elevated
three-quarter view, 72–80% width, `module-mass-driver.png` as Image 1).

- `season-champion-medal`: for the hall of fame entries on the corporation
  card. A heavy gold medal disc on a short dark ribbon. Its face is relief-cast
  with a ring-shaped jump gate and a small four-pointed star at the ring's
  centre, ringed by a thin laurel border. Aged gold with a dark gunmetal rim, one
  small cyan inlay. No text.
- `colony-charter`: for buying colony licences. A rolled charter document of
  thick ivory polymer, half unrolled, bound with a brass clasp and sealed with a
  round violet wax-like seal pressed with an abstract star shape. The document
  is blank, with only faint ruled lines: no text.

Use the §1 prompt with `{NAME}` and `{SUBJECT}` from these lines. Ignore the
GRADE block; use aged gold for the medal and ivory, brass and violet for the
charter.
