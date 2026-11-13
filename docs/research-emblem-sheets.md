# Research emblems: ChatGPT sheet prompts

Peter (Nov 3 2026): "more unique graphics for the research panel". The 119
programme emblems still showing their field's badge, as sheets ChatGPT draws six
at a time, in the arrangement of the approved July field sheet. Generated from
`client/art-src/research-emblems-v1/prompts.json` by
`node client/scripts/build-research-sheet-prompts.mjs`; the motifs and colours are
the ones in `research-art-prompts.md`, only the packaging differs.

## How to run it

1. One ChatGPT message per sheet: paste the block, attach the files its **Attach**
   line names (Image 1 is always the field sheet; convert a `.webp` to PNG if the
   upload refuses it). Ask for the image at the size the block's Layout line gives.
2. Save the result as `client/art-src/research-emblems-v1/sheets/<sheet>.png` (the
   heading's name, for example `propulsion-01.png`). Keep the file as downloaded;
   the build slices and re-frames it.
3. `cd client && npm run assets:research`. It finds each badge by the clear gaps,
   checks the count against `sheets/sheets.json`, and derives the 64–512 px levels.
   A sheet with a backdrop, the wrong number of badges or a touching pair stops
   the build with the sheet's name: ask ChatGPT for the same sheet again, quoting
   the Background or Constraints line.
4. Order: the six `-01` sheets first (36 badges, the Tier I–II programmes a
   new player meets), then the rest. Lay the first finished sheet beside the field
   sheet before going on: if the badge drifts (shape, rim, face colour), fix the
   Badge line here and in `prompts.json` before the run.
5. A single emblem that needs redoing: `research-art-prompts.md` has its 1254 px
   prompt. A delivered `<key>.png` master wins over the sheet's cell.

## Checking a sheet

- Every badge the same hexagon as the field sheet: point up, navy-black face,
  thin dark rim, faint upper-left bevel; the same size as its neighbours.
- Real alpha between and around the badges; no shadow, glow or backdrop.
- The motifs in the listed order, each different from the others at a glance and
  readable at 48 px; no text or numerals anywhere.

## Sheets (21)

| Sheet | Field | Badges | Programmes |
|---|---|---|---|
| `propulsion-01` | Propulsion | 6 | Drive Tuning I, Bunkerage I, Freight Frames I, Extended Tanks I, Efficient Burns II, Heavy Lifters II |
| `propulsion-02` | Propulsion | 6 | Military Drives II, Fusion Freight III, Express Charters III, Torch Regime III, Drop Berths IV, Bulk Charters IV |
| `propulsion-03` | Propulsion | 6 | Fleet Tenders IV, Long Patrol IV, Autonomous Freight V, Mass Streams V, Ramscoop Skimming V, Underway Refit V |
| `materials-01` | Materials | 6 | Deep Bores I, Enrichment I, Bulk Storage I, Ore Recovery I, Beneficiation II, Prefab Construction II |
| `materials-02` | Materials | 6 | Autoforges II, Belt Stations II, Tier-IV Extraction III, Strip Rigs III, Tier-IV Processors III, Slag Reclamation III |
| `materials-03` | Materials | 6 | Trace Refining IV, Prospecting Charters IV, Orbital Yards IV, Arcology Frames IV, Fusion Reactors IV, Mantle Taps V |
| `materials-04` | Materials | 4 | Crown Vein V, Flash Forges V, Crown Project V, Antimatter Synthesis VI |
| `computation-01` | Computation | 6 | Sensor Gain I, Signal Libraries I, Survey Protocols I, Recon Suite I, Predictive Plots II, Deep-Space Arrays II |
| `computation-02` | Computation | 6 | Gravimetric Survey II, Picket Networks III, Long Baselines III, ECM Emitters III, Traffic Analysis III, Battle Archives IV |
| `computation-03` | Computation | 6 | Standing Watch IV, Wake Analysis IV, Panopticon Catalog V, Survey Corps V, Signature Mimicry V, Counter-Intelligence V |
| `weapons-01` | Weapons | 6 | Fire Control I, Magnetic Accelerators I, Warhead Yields I, Orbital Batteries II, Hardened Magazines II, Munitions Lines II |
| `weapons-02` | Weapons | 6 | Fire Discipline II, Lance Array III, Breaching Ordnance III, Flak Doctrine III, Ablative Refits III, Defense Stations III |
| `weapons-03` | Weapons | 6 | Platform Lances IV, Penetrator Slugs IV, Platform Netfire IV, Spall Liners IV, First Strike V, Overpressure Warheads V |
| `weapons-04` | Weapons | 3 | Grand Batteries V, Star Fortress V, Citadel Compartments V |
| `hulls-01` | Hulls | 6 | Escort Datalink I, Cargo Pods I, Drydock Efficiency I, Reinforced Frames I, Slipway Standards I, Rapid Rearm II |
| `hulls-02` | Hulls | 6 | Modular Berths II, Compartmentalization II, Hardened Anchorage III, Wolfpack Doctrine III, Prize Holds III, Destroyer Hull IV |
| `hulls-03` | Hulls | 6 | Fleet Escorts IV, Slip Anchors IV, Blockade Runners IV, Cruiser Hull V, Fortress Doctrine V, Salvage Rigs V |
| `hulls-04` | Hulls | 4 | Boarding Parties V, Battleship VI, Dreadnought VII, Titan VIII |
| `life-01` | Life | 6 | Hydroponics I, Med Bays I, Dense Housing I, Civic Rations II, Cryo Berths II, Gene-Tailored Crops II |
| `life-02` | Life | 6 | Orbital Habitats III, Boom Charters III, Expert Training III, Deep Careers III, Xenoacclimation IV, Closed-Loop Ecology IV |
| `life-03` | Life | 6 | Academy IV IV, Broad Curricula IV, Arcologies V, Generation Charters V, Twin Campuses V, Founders' Institutes V |

### `propulsion-01` · Propulsion · 6 badges

Keys in reading order: `prop_drive_tuning`, `prop_bunkerage`, `prop_freight_frames`, `prop_expedition_iii_extended_tanks`, `prop_efficient_burns`, `prop_heavy_lifters`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`; Image 2 `client/public/art/derived/ships/models-v1/previews/small_freighter-768.webp`; Image 3 `client/public/art/derived/ships/models-v1/previews/convoy-768.webp`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Propulsion. Colour, all six badges: silver-white metal with an icy electric-blue engine glow and trail. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: A single ship engine nozzle seen from the side and slightly behind, a bright blue exhaust cone streaming from its bell, a slim spanner laid diagonally across the nozzle.
2. Top centre: A fat cylindrical fuel bunker tank lying on a heavy cradle, banded with steel straps, a glowing blue level window along its side and a coiled fuel hose beside it.
3. Top right: The Small Freighter from Image 2, its engines glowing blue.
4. Bottom left: A small dart-shaped warship seen from above at an angle, two slim cylindrical external fuel tanks clamped along its flanks, its engine glowing blue.
5. Bottom centre: A single droplet of blue fuel with a small, clean blue flame burning inside it, a short curved gauge arc beneath it with its needle at the low end.
6. Bottom right: The Medium Freighter from Image 3, its cargo frame loaded, its engines glowing blue.
Image 2 is the game's own render of the Small Freighter: use it ONLY for that design's silhouette and details, drawn once, in an elevated three-quarter view, as motif 3, in Image 1's emblem rendering. Ignore its own lighting and background.
Image 3 is the game's own render of the Medium Freighter: use it ONLY for that design's silhouette and details, drawn once, in an elevated three-quarter view, as motif 6, in Image 1's emblem rendering. Ignore its own lighting and background.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `propulsion-02` · Propulsion · 6 badges

Keys in reading order: `prop_military_drives`, `prop_line_fusion_freight`, `prop_line_express_charters`, `prop_expedition_iii_torch_regime`, `prop_line_drop_berths`, `prop_line_bulk_charters`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`; Image 2 `client/public/art/derived/ships/models-v1/previews/large_freighter-768.webp`; Image 3 `client/public/art/derived/ships/models-v1/previews/heavy_freighter-768.webp`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Propulsion. Colour, all six badges: silver-white metal with an icy electric-blue engine glow and trail. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: A sleek arrowhead warship diving steeply toward the lower left, two long blue afterburner streaks behind its twin engines.
2. Top centre: A block of cargo containers pushed by a compact fusion drive: a glowing cyan plasma ring at its stern and one long thin blue plume.
3. Top right: The Large Freighter from Image 2 racing forward, three blue speed streaks trailing behind it.
4. Bottom left: A compact warship seen from behind and to the side, its engine blasting an enormous flaring blue-white torch flame several times longer than the ship.
5. Bottom centre: A rounded colony pod with a row of small lit windows being lowered out of an open docking-berth frame, a short blue retro-thruster plume beneath it.
6. Bottom right: The Heavy Freighter from Image 3, its thick stack of cargo containers dominating the silhouette, its engines glowing blue.
Image 2 is the game's own render of the Large Freighter: use it ONLY for that design's silhouette and details, drawn once, in an elevated three-quarter view, as motif 3, in Image 1's emblem rendering. Ignore its own lighting and background.
Image 3 is the game's own render of the Heavy Freighter: use it ONLY for that design's silhouette and details, drawn once, in an elevated three-quarter view, as motif 6, in Image 1's emblem rendering. Ignore its own lighting and background.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `propulsion-03` · Propulsion · 6 badges

Keys in reading order: `prop_expedition_iv_fleet_tenders`, `prop_expedition_iv_long_patrol`, `prop_line_autonomous_freight`, `prop_line_mass_streams`, `prop_expedition_v_ramscoop`, `prop_expedition_v_underway_refit`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`; Image 2 `client/public/art/derived/ships/models-v1/previews/bulk_freighter-768.webp`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Propulsion. Colour, all six badges: silver-white metal with an icy electric-blue engine glow and trail. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: A stubby tanker ship trailing a long flexible refuelling hose to a small warship following behind it, a blue fuel glow along the hose.
2. Top centre: A lone small warship at the end of a long sweeping arc of faint blue trail that curves around a small dark planet.
3. Top right: The Bulk Freighter from Image 2 following a dotted blue route line that loops around it through three small waypoint dots.
4. Bottom left: Two small bright stars, upper left and lower right, joined by a broad glowing blue stream of light, three tiny freighter silhouettes riding along the stream.
5. Bottom centre: A ship with a wide funnel-shaped ram scoop at its bow skimming low over the banded cloud tops of a gas giant's curve, a blue wake streaming behind it.
6. Bottom right: A small tender ship gripping a damaged warship with two jointed robotic repair arms, bright blue welding sparks where the arms meet the hull.
Image 2 is the game's own render of the Bulk Freighter: use it ONLY for that design's silhouette and details, drawn once, in an elevated three-quarter view, as motif 3, in Image 1's emblem rendering. Ignore its own lighting and background.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `materials-01` · Materials · 6 badges

Keys in reading order: `mat_deep_bores`, `mat_enrichment`, `mat_bulk_storage`, `mat_ore_recovery`, `mat_beneficiation`, `mat_prefab_construction`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Materials. Colour, all six badges: charcoal gunmetal with a molten orange-amber glow. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: A heavy spiral drill bit boring down into layered rock strata, bright orange sparks and glowing chips at its tip.
2. Top centre: A conical ore hopper funnelling raw grey rock into the open mouth of a glowing orange smelting furnace below it.
3. Top right: A tall stack of heavy banded cargo crates with orange hazard stripes, a freight hook hanging above them.
4. Bottom left: A single cast metal ingot ringed by a circular recovery arrow made of flowing molten orange metal.
5. Bottom centre: A tilted vibrating sieve tray full of rough grey rock, a few glowing orange ore nuggets dropping through its mesh.
6. Bottom right: A crane hook lowering one flat prefab wall panel into place beside two panels already standing, orange weld light at the seam.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `materials-02` · Materials · 6 badges

Keys in reading order: `mat_autoforges`, `mat_belt_stations`, `mat_deepcrust_iii_tier4_extraction`, `mat_deepcrust_iii_strip_rigs`, `mat_foundry_iii_tier4_processors`, `mat_foundry_iii_slag_reclamation`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`; Image 2 `client/public/art/derived/ships/models-v1/previews/mining_belt_station-768.webp`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Materials. Colour, all six badges: charcoal gunmetal with a molten orange-amber glow. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: A robotic forging arm with a heavy hammer head striking a glowing orange bar on an anvil, sparks flying.
2. Top centre: The Mining Belt Station from Image 2 anchored to the side of a large asteroid, its docking arms out, orange work lights on.
3. Top right: A towering lattice mining derrick over a deep stepped open pit, orange floodlights shining down into the pit.
4. Bottom left: A tracked crawler rig with a wide spinning cutting drum stripping a band across the cratered surface of a moon, orange dust spraying behind it.
5. Bottom centre: A massive industrial smelter block with three tall chimney stacks and a row of glowing orange furnace windows along its base.
6. Bottom right: A tipping ladle pouring glowing orange slag onto a short conveyor, where it cools into pale whitish crystal chunks.
Image 2 is the game's own render of the Mining Belt Station: use it ONLY for that design's silhouette and details, drawn once, in an elevated three-quarter view, as motif 2, in Image 1's emblem rendering. Ignore its own lighting and background.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `materials-03` · Materials · 6 badges

Keys in reading order: `mat_deepcrust_iv_trace_refining`, `mat_deepcrust_iv_prospecting_charters`, `mat_foundry_iv_orbital_yards`, `mat_foundry_iv_arcology_frames`, `mat_foundry_iv_fusion_reactors`, `mat_deepcrust_v_mantle_taps`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Materials. Colour, all six badges: charcoal gunmetal with a molten orange-amber glow. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: A chunk of grey ore split open with a few small brilliant crystals glittering inside, fine tongs lifting one glowing orange-gold crystal out.
2. Top centre: A scanner beam fanning down from above onto a rough asteroid, revealing a glowing orange ore pocket hidden inside it.
3. Top right: A tall spindly orbital yard tower rising from a planet's curved horizon into space, a ship docked at its top, orange weld lights along it.
4. Bottom left: A tall stepped arcology tower of exposed steel girders, its finished lower floors lit with warm orange windows, its upper frame still open.
5. Bottom centre: A squat armoured reactor dome with a round viewport showing a brilliant orange-white plasma core, heavy magnetic coils ringing its base.
6. Bottom right: A cut-away slice of a planet's crust with a long drill pipe reaching down through dark rock layers into a glowing molten orange mantle.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `materials-04` · Materials · 4 badges

Keys in reading order: `mat_deepcrust_v_crown_vein`, `mat_foundry_v_flash_forges`, `mat_foundry_v_crown_project`, `mat_foundry_vi_antimatter_synthesis`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`.

```text
Use case: stylized-concept. Asset type: a sheet of four transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw FOUR new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Materials. Colour, all four badges: charcoal gunmetal with a molten orange-amber glow. Everything else stays in the family's navy, charcoal and silver.
Layout: square 1024 x 1024 canvas, two columns and two rows. Four identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: A thick glowing orange-gold ore vein running diagonally through dark rock, a small three-pointed crown floating above it.
2. Top right: Two construction cranes side by side, each lowering a glowing orange girder at the same moment, sparks between them.
3. Bottom left: A small rocky planet with two newly lit orange building pads on its surface and a small three-pointed crown floating above it.
4. Bottom right: A slim magnetic containment bottle held between two orange-lit coil rings, a tiny brilliant white spark of antimatter floating at its centre.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The four must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly four badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `computation-01` · Computation · 6 badges

Keys in reading order: `comp_sensor_gain`, `comp_signal_libraries`, `comp_survey_protocols`, `comp_shadow_iv_recon_suite`, `comp_predictive_plots`, `comp_deep_space_arrays`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Computation. Colour, all six badges: pale silver-white line work with one small white star glint; no other colour. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: A round radar scope screen with a bright sweeping beam wedge and two small contact blips.
2. Top centre: A neat stack of four flat data cartridges, one bright waveform line running across their front edges.
3. Top right: A small survey probe with spread antenna arms hovering above a curved planet surface marked with a fine survey grid.
4. Bottom left: A compact sensor pod module with mounting clamps, a folding antenna mast and a small dish, ready to bolt onto a hull.
5. Bottom centre: A solid contact marker trailing a dashed projected path that curves ahead to a hollow ghost marker.
6. Bottom right: Three large radio dishes standing in a row, all aimed at the same small white star above them.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `computation-02` · Computation · 6 badges

Keys in reading order: `comp_gravimetric_survey`, `comp_watch_iii_picket_networks`, `comp_watch_iii_long_baselines`, `comp_shadow_iii_ecm_emitters`, `comp_shadow_iii_traffic_analysis`, `comp_watch_iv_battle_archives`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Computation. Colour, all six badges: pale silver-white line work with one small white star glint; no other colour. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: A small star sinking into a fine square grid of space bent down into a well, ripples spreading outward across the grid.
2. Top centre: Three small sensor buoys at the corners of a triangle, linked by thin glowing lines into a net.
3. Top right: Two small dishes far apart at the two ends of a long straight baseline beam, both aimed at one distant white star.
4. Bottom left: A ship silhouette dissolving at its rear into jagged horizontal bands of static noise.
5. Bottom centre: A round scanning lens held over a freighter silhouette; inside the lens the freighter's cargo crates show through like an x-ray.
6. Bottom right: A data tablet showing two facing rows of tiny ship silhouettes, several of them struck through with short lines.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `computation-03` · Computation · 6 badges

Keys in reading order: `comp_watch_iv_standing_watch`, `comp_shadow_iv_wake_analysis`, `comp_watch_v_panopticon`, `comp_watch_v_survey_corps`, `comp_shadow_v_signature_mimicry`, `comp_shadow_v_counter_intel`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Computation. Colour, all six badges: pale silver-white line work with one small white star glint; no other colour. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: A small hourglass beside a contact track of dotted marks that fade out one by one.
2. Top centre: A fleet chevron trailing a long glowing wake behind it, a small fuel gauge dial beside the wake.
3. Top right: A great ring-shaped sensor station like a watchful eye, many small lenses set around its rim, a white star glint at its centre.
4. Bottom left: A small scout ship circling a star on a thin orbit line, sweeping it with a pale survey cone.
5. Bottom centre: An angular warship whose cast shadow below it has the boxy shape of a freighter.
6. Bottom right: A round scanning lens cracked across, its beam stopped by a small shield held in front of it.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `weapons-01` · Weapons · 6 badges

Keys in reading order: `weap_fire_control`, `weap_magnetic_accelerators`, `weap_warhead_yields`, `weap_orbital_batteries`, `weap_hardened_magazines`, `weap_munitions_lines`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`; Image 2 `client/public/art/derived/stations/v2/level-2-512.webp`; Image 3 `client/public/art/derived/stations/v2/level-3-512.webp`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Weapons. Colour, all six badges: silver metal with a signal-red accent and a small orange-yellow flash. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: The Defense Platform from Image 2 firing one thin red beam, a targeting reticle locked over it.
2. Top centre: A long mass-driver barrel ringed by magnetic coil bands, a slug leaving its muzzle with a red flash.
3. Top right: A torpedo warhead nose cone cut open lengthwise to show a glowing red explosive core inside.
4. Bottom left: The Orbital Battery from Image 3, two turrets on its deck, small red targeting lights.
5. Bottom centre: An armoured ammunition magazine box with its lid open, rows of red-tipped shells packed inside.
6. Bottom right: A conveyor line carrying a row of red-banded missiles under a heavy stamping press.
Image 2 is the game's own art of the Defense Platform station: use it ONLY for that design's silhouette and details, drawn once, in an elevated three-quarter view, as motif 1, in Image 1's emblem rendering. Ignore its own lighting and background.
Image 3 is the game's own art of the Orbital Battery station: use it ONLY for that design's silhouette and details, drawn once, in an elevated three-quarter view, as motif 4, in Image 1's emblem rendering. Ignore its own lighting and background.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `weapons-02` · Weapons · 6 badges

Keys in reading order: `weap_fire_discipline`, `weap_strike_iii_lance_array`, `weap_strike_iii_breaching_ordnance`, `weap_cm_iii_flak_doctrine`, `weap_cm_iii_ablative_refits`, `weap_cm_iii_defense_stations`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`; Image 2 `client/public/art/derived/stations/v2/level-4-512.webp`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Weapons. Colour, all six badges: silver metal with a signal-red accent and a small orange-yellow flash. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: Three gun barrels side by side firing at the same instant, three identical red muzzle flashes in a row.
2. Top centre: A long heavy lance cannon with a faceted crystal emitter at its muzzle, firing a thick red beam.
3. Top right: A shell punching through a thick armour wall plate, the plate cracking outward, red fire on the far side.
4. Bottom left: A point-defense turret spraying a wide fan of red tracer flak at two small incoming missiles.
5. Bottom centre: An angled mirrored armour plate deflecting a red beam away.
6. Bottom right: The Defense Station from Image 2, three turrets, small red lights.
Image 2 is the game's own art of the Defense Station station: use it ONLY for that design's silhouette and details, drawn once, in an elevated three-quarter view, as motif 6, in Image 1's emblem rendering. Ignore its own lighting and background.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `weapons-03` · Weapons · 6 badges

Keys in reading order: `weap_strike_iv_platform_lances`, `weap_strike_iv_penetrator_slugs`, `weap_cm_iv_platform_netfire`, `weap_cm_iv_spall_liners`, `weap_strike_v_first_strike`, `weap_strike_v_overpressure`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Weapons. Colour, all six badges: silver metal with a signal-red accent and a small orange-yellow flash. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: A station turret on a short armoured pylon, a prismatic crystal lance emitter on top firing a red beam.
2. Top centre: A long dense dart-shaped slug with a needle-sharp red-hot tip, flying point first.
3. Top right: A defense station projecting a wide dome of red tracer net over two small ships sheltering beside it.
4. Bottom left: A cut-away of spaced armour layers with a slug caught in the middle layer, red sparks where it stopped.
5. Bottom centre: A red arrowhead strike splitting a round shield clean in two.
6. Bottom right: A torpedo detonating against one hull, a red shockwave ring spreading from it into a second hull beside it.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `weapons-04` · Weapons · 3 badges

Keys in reading order: `weap_cm_v_grand_batteries`, `weap_cm_v_star_fortress`, `weap_cm_v_citadel_compartments`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`; Image 2 `client/public/art/derived/stations/v2/level-5-512.webp`; Image 3 `client/public/art/derived/stations/v2/level-6-512.webp`.

```text
Use case: stylized-concept. Asset type: a sheet of three transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw THREE new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Weapons. Colour, all three badges: silver metal with a signal-red accent and a small orange-yellow flash. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, one row of three. Three identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Left: The Citadel from Image 2 bristling with its heavy gun turrets, red lights along its walls.
2. Centre: The Star Fortress from Image 3, its heavy battery arms radiating from its core, red lights at each gun.
3. Right: A warship hull silhouette with an armoured vault-like citadel core glowing faintly red at its heart.
Image 2 is the game's own art of the Citadel station: use it ONLY for that design's silhouette and details, drawn once, in an elevated three-quarter view, as motif 1, in Image 1's emblem rendering. Ignore its own lighting and background.
Image 3 is the game's own art of the Star Fortress station: use it ONLY for that design's silhouette and details, drawn once, in an elevated three-quarter view, as motif 2, in Image 1's emblem rendering. Ignore its own lighting and background.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The three must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly three badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `hulls-01` · Hulls · 6 badges

Keys in reading order: `hull_line_iii_escort_datalink`, `hull_cargo_pods`, `hull_drydock_efficiency`, `hull_reinforced_frames`, `hull_slipway_standards`, `hull_rapid_rearm`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Hulls. Colour, all six badges: cool steel grey and silver; the only colour is a faint cyan engine glow where a ship has engines. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: A small corvette flying beside a larger freighter, a thin straight datalink beam joining them.
2. Top centre: A freighter spine with two plump cargo pods clamped onto it, one on each side.
3. Top right: A gantry crane lowering a turret onto a half-built warship bow, a small stopwatch dial at the side.
4. Bottom left: A curved section of ship hull ribs with heavy diagonal reinforcing braces bolted across them.
5. Bottom centre: Four identical hull sections lined up one behind another on a straight slipway rail.
6. Bottom right: A jointed repair arm welding a dented hull plate, a bright spray of sparks at the weld.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `hulls-02` · Hulls · 6 badges

Keys in reading order: `hull_modular_berths`, `hull_compartmentalization`, `hull_line_iii_hardened_anchorage`, `hull_corsair_iii_wolfpack`, `hull_corsair_iii_prize_holds`, `hull_line_iv_destroyer`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`; Image 2 `client/public/art/derived/ships/models-v1/previews/destroyer-768.webp`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Hulls. Colour, all six badges: cool steel grey and silver; the only colour is a faint cyan engine glow where a ship has engines. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: A warship hull with one open module bay, a crane arm sliding a square module block into it.
2. Top centre: A ship hull cut in cross-section, divided into sealed compartments by thick bulkheads with round hatches.
3. Top right: A heavy ship's anchor fused with a thick armour plate, a small planet behind it.
4. Bottom left: Three small raider ships in a tight V formation diving to attack, angled like wolf fangs.
5. Bottom centre: A raider ship with its cargo hold open, stuffed with captured crates.
6. Bottom right: The Destroyer from Image 2.
Image 2 is the game's own render of the Destroyer: use it ONLY for that design's silhouette and details, drawn once, in an elevated three-quarter view, as motif 6, in Image 1's emblem rendering. Ignore its own lighting and background.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `hulls-03` · Hulls · 6 badges

Keys in reading order: `hull_line_iv_fleet_escorts`, `hull_corsair_iv_slip_anchors`, `hull_corsair_iv_blockade_runners`, `hull_line_v_cruiser`, `hull_line_v_fortress_doctrine`, `hull_corsair_v_salvage_rigs`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`; Image 2 `client/public/art/derived/ships/models-v1/previews/cruiser-768.webp`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Hulls. Colour, all six badges: cool steel grey and silver; the only colour is a faint cyan engine glow where a ship has engines. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: A small corvette in front of a freighter, a curved shield arc projected between them.
2. Top centre: A small raider ship slipping backward out of a broken chain, the links snapping apart.
3. Top right: A small freighter darting through a gap in a line of three blockading warships.
4. Bottom left: The Cruiser from Image 2.
5. Bottom centre: Three small warships sheltering under the broad armoured wing of a defense station.
6. Bottom right: A salvage claw pulling a square module crate out of a drifting broken wreck.
Image 2 is the game's own render of the Cruiser: use it ONLY for that design's silhouette and details, drawn once, in an elevated three-quarter view, as motif 4, in Image 1's emblem rendering. Ignore its own lighting and background.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `hulls-04` · Hulls · 4 badges

Keys in reading order: `hull_corsair_v_boarding_parties`, `hull_line_vi_battleship`, `hull_line_vii_dreadnought`, `hull_line_viii_titan`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`; Image 2 `client/public/art/derived/ships/models-v1/previews/battleship-768.webp`; Image 3 `client/public/art/derived/ships/models-v1/previews/dreadnought-768.webp`; Image 4 `client/public/art/derived/ships/models-v1/previews/titan-768.webp`.

```text
Use case: stylized-concept. Asset type: a sheet of four transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw FOUR new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Hulls. Colour, all four badges: cool steel grey and silver; the only colour is a faint cyan engine glow where a ship has engines. Everything else stays in the family's navy, charcoal and silver.
Layout: square 1024 x 1024 canvas, two columns and two rows. Four identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: A boarding tube clamped onto a freighter's side hatch.
2. Top right: The Battleship from Image 2.
3. Bottom left: The Dreadnought from Image 3.
4. Bottom right: The Titan from Image 4, filling the badge: the largest ship in the family.
Image 2 is the game's own render of the Battleship: use it ONLY for that design's silhouette and details, drawn once, in an elevated three-quarter view, as motif 2, in Image 1's emblem rendering. Ignore its own lighting and background.
Image 3 is the game's own render of the Dreadnought: use it ONLY for that design's silhouette and details, drawn once, in an elevated three-quarter view, as motif 3, in Image 1's emblem rendering. Ignore its own lighting and background.
Image 4 is the game's own render of the Titan: use it ONLY for that design's silhouette and details, drawn once, in an elevated three-quarter view, as motif 4, in Image 1's emblem rendering. Ignore its own lighting and background.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The four must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly four badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `life-01` · Life · 6 badges

Keys in reading order: `life_hydroponics`, `life_med_bays`, `life_dense_housing`, `life_civic_rations`, `life_cryo_berths`, `life_gene_crops`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Life. Colour, all six badges: fresh leaf green with warm amber lights and silver glass. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: Three rows of young green seedlings growing in hydroponic trays under a bar of warm amber grow lights.
2. Top centre: A medical capsule bed under a curved diagnostic arch, a soft green scan light passing over it.
3. Top right: A compact block of stacked habitat modules with many small warm-lit windows.
4. Bottom left: A ration tray with neat portioned compartments of food, one fresh green leaf on top.
5. Bottom centre: A frosted cryo-sleep pod standing upright, its window glowing soft green, the outline of a sleeper inside.
6. Bottom right: A green plant sprouting straight up out of a twisting DNA double helix.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `life-02` · Life · 6 badges

Keys in reading order: `life_growth_iii_orbital_habitats`, `life_growth_iii_boom_charters`, `life_talent_iii_expert_training`, `life_talent_iii_deep_careers`, `life_growth_iv_xenoacclimation`, `life_growth_iv_closed_loop`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Life. Colour, all six badges: fresh leaf green with warm amber lights and silver glass. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: A ring-shaped orbital habitat with green gardens along its inner surface, floating above a banded gas giant.
2. Top centre: A line of tiny settler silhouettes walking down a liner's lit ramp, warm amber light behind them.
3. Top right: A star-and-chevron specialist rank badge framed by a green laurel.
4. Bottom left: An open book with a small hourglass standing on its pages, a soft green glow rising from them.
5. Bottom centre: A hardy green alien sprout pushing up through cracked barren grey rock.
6. Bottom right: A sealed glass sphere holding a tiny green ecosystem, a circular arrow wrapped around it.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```

### `life-03` · Life · 6 badges

Keys in reading order: `life_talent_iv_academy_iv`, `life_talent_iv_broad_curricula`, `life_growth_v_arcologies`, `life_growth_v_generation_charters`, `life_talent_v_twin_campuses`, `life_talent_v_founders_institutes`.

**Attach:** Image 1 `client/art-src/research-topics-icons/research-topics-icons.png`.

```text
Use case: stylized-concept. Asset type: a sheet of six transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw SIX new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.
Field: Life. Colour, all six badges: fresh leaf green with warm amber lights and silver glass. Everything else stays in the family's navy, charcoal and silver.
Layout: landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement. Six identical hexagon badges, each about 450 px tall, centred in its own 512 x 512 cell, with clear empty space between badges and around the sheet. Badges never touch or overlap.
Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.
Motifs, reading left to right, top row then bottom row:
1. Top left: A grand academy hall with a central dome and columns, warm amber windows.
2. Top centre: Two open books side by side under one lamp, both glowing green.
3. Top right: A vast domed arcology city with terraced green gardens stepping up its sides and warm amber lights.
4. Bottom left: A long generation ship with a green-lit garden ring around its middle and rows of tiny warm windows.
5. Bottom centre: Two matching campus towers joined by a sky bridge, warm amber windows.
6. Bottom right: A founder's statue on a tall pedestal in front of an institute's arched gateway.
Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The six must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.
Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.
Constraints: exactly six badges. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.
```
