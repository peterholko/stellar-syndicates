# Shipbuilding overhaul: art prompts

Nov 7 2026. The shipbuilding overhaul (docs/shipbuilding-overhaul-plan.md)
adds thirteen goods and renames one. Each ships with a **placeholder SVG glyph** in
`client/public/art/goods-placeholder/<slug>.svg` until its master lands. The
masters follow the one resource-icon style (docs/resource-icon-prompts.md):
same prompt frame, same 3/4 view, same upper-left light, 1254 × 1254 with real
alpha. Generate them in another model; nothing here needs a code change to
accept them beyond the two lines in "Wiring" below.

## The prompt frame

Use the resource-icon frame unchanged. Replace `[subject]` with the line from
the table.

```text
Use case: stylized-concept. Asset type: sci-fi strategy game inventory resource icon for Stellar Syndicates. A single centered resource object, polished hard-surface 3D game icon, crisp broad forms readable at 32–64 px, 3/4 view, restrained texture, strong silhouette, studio light from upper left. Object occupies 80% of square canvas. Genuine transparent RGBA background; no scene, ground, border, frame, text, labels, watermark, glow haze, UI or extra props. [subject]
```

- **Save as:** `client/art-src/goods-2026-11-07/<file>.png`, with a
  `prompts.json` beside them in the ore set's format (`mode`, `assets` with
  `key` and `prompt`).
- **File names are the game's slugs**: they become the icon names.
- Keep every subject distinct at 32 px. The four reactors (and the four drives) share one body and
  differ by the colour of their emitter ring and conduit glow, so the ladder
  reads as one family; nothing else in the set is a cylinder.

## Order (Nov 8 2026)

Later images are edits of earlier ones, so generate in this order:

1. The four warp drives (below): masters delivered Nov 8 in `goods-2026-11-07/`.
2. Hull Sections, from [resource-icon-prompts.md](resource-icon-prompts.md):
   the heavy and capital frames are edits of it.
3. The seventeen modules (below).
4. The eighteen heavy and capital parts (below), each an edit of its light
   master.

**Done Nov 8 2026:** all forty masters are in the game: the drives in
`goods-2026-11-07/`, Hull Sections in `goods-2026-10-24/`, the seventeen
modules and eighteen sized parts in `goods-2026-11-08/`, each folder with its
exact prompts and a generation manifest. See "Wiring".

## The reactors: one body, four tints (§reactors-drives, Nov 7 2026)

The four cores became the four reactors when ships' engines split into a
reactor and a warp drive (docs/reactor-drive-split.md). The Nov 7 core masters
are reused as they are: `fission_core.png` → `fission_reactor.png`,
`fold_core.png` → `antimatter_reactor.png`; the Fusion Reactor and Advanced
Fusion Reactor keep their own masters below. Nothing to regenerate here unless
the family is redrawn — if it is, keep the one-body-four-tints rule: amber
(Fission), blue-white (Fusion), gold (Advanced Fusion), violet (Antimatter).

## The warp drives: one body, four tints (wired Nov 8 2026, masters in)

Generate the Zephyr Drive first, then the other three as the same object with
only the tint and one detail changed. The four native 1254 × 1254 transparent
masters replaced the flat-drawn placeholders in `client/art-src/goods-2026-11-07/`.
Each variant was edited directly from Zephyr; exact prompts and measurements are
recorded beside the masters.

| File | Good | Prompt (after the frame) |
|---|---|---|
| `zephyr_drive.png` | Zephyr Drive | A compact warp drive: a dark steel thruster bell seen from the rear quarter, three copper conduit bands around its throat, and a single wide emitter ring at its mouth glowing **amber-orange**. Squat and plain, the colony's first drive. |
| `gale_drive.png` | Gale Drive | The same drive body, its emitter ring glowing **pale blue-white**, a second thin ring just inside the first; the bell a shade longer. |
| `tempest_drive.png` | Tempest Drive | The same drive body, its emitter ring glowing **warm gold** with three short gold vanes set into the bell; the copper bands brighter. |
| `maelstrom_drive.png` | Maelstrom Drive | The same drive body, its emitter ring glowing **deep violet** with a faint spiral of violet light inside the mouth, and a black magnetic collar at the throat. |

## The reactors

| File | Good | `[subject]` |
|---|---|---|
| `fusion_reactor.png` | Fusion Reactor | A planet-side fusion reactor core: a stout toroidal steel vessel with a blue-white plasma ring glimpsed through four small viewports, heavy mounting lugs at the base. Wider than tall; nothing like the slim warp core. |
| `advanced_fusion_reactor.png` | Advanced Fusion Reactor | The same toroidal reactor vessel with a second outer ring of gold field coils and gold plasma in its viewports; a small iridium-white badge plate on the front lug. |

The old `goods-v1/fusion_core` master was drawn as the plant's heart, so the
client already shows it for the Fusion Reactor; the new reactor master replaces
it and frees the name.

## The kit parts

| File | Good | `[subject]` |
|---|---|---|
| `nav_computer.png` | Nav Computer | A ruggedised navigation computer: a dark slate-grey flight box with a single inset screen showing a faint cyan star-plot, two latched side handles and a short gold data-port strip along the bottom edge. |
| `cargo_bay.png` | Cargo Bay | A prefabricated cargo bay module: an open-fronted grey steel hold section with ribbed walls, a hinged loading ramp folded up, and one orange strap-down point inside. Reads as a room for cargo, not a crate. |
| `pulse_laser.png` | Pulse Laser | A light ship-mounted pulse laser: a short grey gun housing on a small round turret base with a stubby barrel ending in a red-tinted emitter lens; one heat vane along the top. Small and quick-looking. |
| `beam_battery.png` | Beam Battery | A heavy line beam battery: a long twin-barrel beam mount on an armoured turret base, both barrels ending in red emitter lenses, with a coolant block and three heat vanes behind. Twice the Pulse Laser's length. |
| `armaments.png` | Weapon Mounts | Weapon-mount hardware: a grey steel turret ring with its pivot and two empty gun cradles, a coiled fire-control cable and a short magazine feed block beside it. No missiles: the mount, not the munition. |

`armaments.png` **replaces** the current missiles-on-a-cradle master. The
good's internal id stays `armaments`; it displays as Weapon Mounts.

## Phase 5: the munitions (wired Nov 7 2026, masters in)

| File | Good | `[subject]` |
|---|---|---|
| `torpedoes.png` | Torpedoes | A rack of three dark-grey ship torpedoes with red nose bands and small fin clusters, lying on a steel loading cradle. Longer and slimmer than the old missiles. |
| `slugs.png` | Slugs | A neat pyramid of six dull tungsten-grey mass-driver slugs, each a short dense cylinder with a bevelled tip, on a steel tray. Heavy and plain. |
| `beam_crystals.png` | Beam Crystals | Three faceted focusing crystals for beam weapons, pale rose-clear, held upright in a padded steel case with the lid open; one crystal has a faint internal glow. |

## The modules (§modules-as-goods; wired Nov 8 2026, masters in)

Modules became goods on Nov 8 and sit in the Market's Modules tab. Twelve wore
borrowed UI icons (Stasis Web and Interdiction Field were emoji) and five the
older panel style, so all seventeen were drawn here as one set. Same prompt
frame as above.

- **Weapons** stand on the Pulse Laser master's round turret base, in its grey
  gun-metal, from its camera (give `pulse_laser.png` as the reference image),
  so the guns read as one family; each differs by its barrel or launcher.
- **The three found modules** (Survey Booster, Nebula Spectrometer, Prismatic
  Lance) are built in a pale pearl-white alloy with fine engraved lines, so
  finds read as one family.
- Nothing here is a plain cylinder or a bell with an emitter ring: those are the
  reactors and the drives.
- Save as `client/art-src/goods-2026-11-08/<file>.png` with a `prompts.json`
  beside them, like the Nov 7 set. File names are the game's slugs (the
  Survey Booster's stays `survey_drive`).

| File | Good | `[subject]` |
|---|---|---|
| `mass_driver.png` | Mass Driver | A ship-mounted mass driver on a round turret base: two long parallel copper-lined rails forming an open barrel, a squat capacitor block at the breech and a small slug magazine clipped to its side. No lens, no glow. |
| `torpedo_rack.png` | Torpedo Rack | A torpedo launcher on a round turret base: an armoured grey box of four square launch tubes, two by two, red-banded torpedo noses just visible in two of them, a hinged loading hatch at the back. |
| `point_defense_screen.png` | Point-Defense Screen | A point-defence turret on a round turret base: a low dome bristling with four short twin-barrel rapid-fire guns aimed in different directions, a flat tracking-radar paddle on top. Busy and compact. |
| `lance_array.png` | Lance Array | A heavy beam lance on a wide armoured turret base: three long slim barrels in a triangular bundle, each ending in a blue-white emitter lens, a ribbed coolant manifold behind. Longer and heavier than the Beam Battery. |
| `prismatic_lance.png` | Prismatic Lance | A found beam weapon on a round turret base, its housing in pale pearl-white alloy with fine engraved lines: one long barrel ending in a large clear faceted crystal prism that throws a faint rainbow sheen along the barrel. |
| `reflective_plating.png` | Reflective Plating | Three curved armour plates stacked with small gaps, their faces polished to mirror chrome that catches a hint of red laser light along the edges, heavy bolts at the corners. |
| `whipple_armor.png` | Whipple Armor | Spaced kinetic armour: two dull grey armour plates held a hand's width apart by short steel standoffs, the front plate pocked with small impact craters and one slug lodged in it. |
| `extended_tanks.png` | Extended Tanks | Two long rounded fuel tanks lying side by side in a curved steel saddle, bare brushed steel with amber hazard stripes and a filler cap on each tank. |
| `recon_suite.png` | Recon Suite | A recon sensor pod: a slim streamlined grey pod with a dark glass sensor window at its nose, a small folding radar dish on top and a short whip antenna at the tail. |
| `cargo_pods.png` | Cargo Pods | Three boxy modular cargo pods, orange with grey frames, stacked in a short steel rack with quick-release latches. Closed containers, not an open hold. |
| `escort_datalink.png` | Escort Datalink | A datalink mast: a short armoured mast topped by a flat square phased-array panel, two blade antennas angled out from its sides, three small green link lights at its foot. |
| `fuel_transfer_rig.png` | Fuel Transfer Rig | A refuelling boom: a folded telescoping steel boom ending in a funnel-shaped drogue coupling, a hose reel and a pump block at its base, yellow-and-black hazard bands. |
| `survey_drive.png` | Survey Booster | A found strap-on booster in pale pearl-white alloy with fine engraved lines: a flat wedge-shaped sled carrying two small auxiliary thruster pods side by side, their nozzles lit pale cyan. No bell and no emitter ring. |
| `nebula_spectrometer.png` | Nebula Spectrometer | A found field spectrometer in pale pearl-white alloy with fine engraved lines: a short telescope barrel with a large round lens on a compact instrument box, a strip of rainbow spectrum lines across its side panel. |
| `stasis_web.png` | Stasis Web | A stasis web projector: a concave emitter dish ringed by four short prongs, a taut web of fine cyan filaments strung between the prong tips across the dish face. |
| `warp_stabilizer.png` | Warp Stabilizer | A warp stabilizer: three nested steel gimbal rings set at different angles around a small pale-green core sphere, on a squat mounting base. Reads as a gyroscope. |
| `interdiction_field.png` | Interdiction Field | An interdiction field generator: a heavy dark-steel anchor-shaped projector standing on its base, a ring of six coils glowing deep red around its foot and a stubby emitter spike on top. |

## The sizes: heavy and capital parts (§hull-ladder; wired Nov 8 2026, masters in)

Engines and frames come in three sizes (`fuel::EngineSize`): a heavy part is
25 light ones assembled, a capital part 25 heavy. Eighteen goods: heavy and
capital Fission / Fusion / Advanced Fusion / Antimatter Reactors, Zephyr /
Gale / Tempest / Maelstrom Drives and Hull Sections. Each wears its own
master; a size without one would fall back to its light part's icon
(`commodityIcon`).

Each is an **edit of its light master**, given as the reference image: the
size shows as scale cues, never a badge or a number. Heavy: a low cradle with
lifting lugs. Capital: a gantry with a person-sized catwalk. The light
masters: the reactors' and drives' in `goods-2026-11-07/`, Hull Sections' in
`goods-2026-10-24/` (step 2 of the order). Save as
`client/art-src/goods-2026-11-08/heavy_<slug>.png` and `capital_<slug>.png`.

**Heavy** (`heavy_fission_reactor`, `heavy_fusion_reactor`,
`heavy_advanced_fusion_reactor`, `heavy_antimatter_reactor`,
`heavy_zephyr_drive`, `heavy_gale_drive`, `heavy_tempest_drive`,
`heavy_maelstrom_drive`, `heavy_hull_sections`), the edit prompt:

```text
Edit this icon. Keep the object exactly as it is: body, colours, materials, camera and light. Make it a heavy industrial part: sit it in a low dark-steel transport cradle with heavy bolted feet, and fix two lifting lugs to the top of the object. The object and its cradle together fill 80% of the square canvas, centred. Genuine transparent RGBA background; no scene, ground, border, text, numbers, badge, glow haze or extra props.
```

**Capital** (the same nine slugs with `capital_`), the edit prompt:

```text
Edit this icon. Keep the object exactly as it is: body, colours, materials, camera and light. Make it a capital-ship part: mount it in an open steel gantry frame with a narrow catwalk and handrail running along its side, the handrail sized for a person so the object reads as enormous. The object and its gantry together fill 80% of the square canvas, centred. Genuine transparent RGBA background; no people, scene, ground, border, text, numbers, badge, glow haze or extra props.
```

## Checking a result

- 1254 × 1254 with real transparency; the object fills about 80–90% of the
  square, centred.
- The same 3/4 view, upper-left studio light and finish as the ore icons (lay
  them beside `client/art-src/ores-2026-09-16/preview.webp`).
- No text, frame, glow haze or background.
- The four reactors line up as one object in four colours, the four drives as another; the two plant reactors as
  one object in two.

## Wiring (done Nov 8 2026)

1. Masters sit in a folder listed in `GOODS_SOURCES`
   (`client/scripts/build-art-derivatives.mjs`; `goods-2026-11-08` is last).
   `npm run assets:derive` in `client/` trims and recentres them into
   `client/public/art/ui_icons/resource/goods-v1/<slug>-{64,128}.png`.
2. `client/src/icons.ts`: every slug is in `GOODS_ICONS`; the `module…`
   entries point at `R("goods-v1/<slug>-128", …)` so fit pickers match the
   Market (the Lance Array has its own, `moduleLanceArray`); `commodityIcon`
   tries the sized slug before the light part.
3. `test-ore-economy.mjs` checks `GOODS_ICONS` against the masters (its folder
   list follows `GOODS_SOURCES`); `test-equipment.mjs` that each module wears
   its own goods icon; `test-industry.mjs` the outputs' icon paths.
4. A client-only change: `npx vite build` and reload; no server restart.
