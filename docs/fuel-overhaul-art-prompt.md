# Fuel overhaul art: Uranium Mill, Antimatter Plant, three goods icons

This is an art request for the user's other image model, Oct 12 2026. The fuel
overhaul (`docs/fuel-overhaul.md`) added two buildings and three goods that
draw stand-ins until their art lands:

| Piece | Stand-in now |
|---|---|
| Uranium Mill, tiers I–IV | the Fuel Refinery's art |
| Antimatter Plant, tiers I–IV | the Fuel Refinery's art |
| Uranium icon | ☢ glyph |
| Antimatter icon | ✴ glyph |
| Fusion Core icon | ⚛ glyph |

Deuterium keeps the old Fuel icon and the Warp Core keeps the Drive Assemblies
icon; neither needs new art. The Power Plant has its own brief,
`docs/power-plant-art-prompt.md`.

Both buildings stop at tier IV: nothing researches their V–VI.

**Framing:** the buildings use the measured v2 framing (see
`docs/living-terraces-art-prompt.md`): the building about 94% of the canvas
wide, its front foot at y ≈ 88%.

## 1. Uranium Mill, tiers I–IV

The mill sifts fissile grains out of Ferrite Ore: two Uranium per ore. Every
home builds one on day one, so it should read as modest, practical industry.

**Attach for every tier:**

- Image 1: `client/art-src/planet-surface-structures-v2/buildings/smelter/tier-1.png`
- Image 2: `client/art-src/planet-surface-structures-v2/buildings/mining_complex/tier-1.png`

**Save as:**
`client/art-src/planet-surface-structures-v2/buildings/uranium_mill/tier-1.png` …
`tier-4.png`, each with a `tier-N.prompt.json` beside it.

```text
Use case: stylized-concept.
Asset type: ONE standalone planetary building-layer sprite, square transparent PNG, approximately 1024 x 1024.
Purpose: reusable architecture above a SEPARATE biome ground layer. Produce BUILDING ONLY, not a diorama. Genuine zero-alpha transparency around the building; preserve translucent soft shadow. No baked soil, gravel apron, grass, rocks around the site, concrete base tile, raised platform, roads, path, border or landscape. Small integral ramps, steps, conveyors and support feet belonging to the structure are allowed.
Style: the approved new Boreal/Temperate structures, grounded near-future frontier engineering. Soft semi-realistic environmental 3D rendering, matte weathered steel, pale-stone and slate panels with dark grey trim, restrained amber service lights and a few warm windows. Large readable forms, no ink outlines, no cel shading, plastic gloss, extreme contrast, excessive greebles or neon. No text, logos, Authority branding, numerals, UI, people, vehicles or spacecraft. No radiation symbols, green glow or glowing effects.
Camera and lighting: elevated front-left three-quarter view about 55 degrees above the ground plane; more roof than wall, mild perspective; front and right walls visible. Soft upper-left daylight, neutral/cool fill and semi-transparent soft lower-right cast/contact shadow attached to the feet.
Framing/alignment: complete compact structure centered in a square. Building width about 94% of the canvas (x ≈ 4–98%), never cropped. Keep the ground footprint centred at approximately x=50%; its near/front foot reaches y≈88%, and its far/rear foot y≈45%. Keep a similar physical site footprint through upgrades; add height and process mass, not a sprawling campus.
Output exactly one structure, no tier lineup/contact sheet.
Input image 1 is the house style for ore processing. Input image 2 is the house style for mining machinery. Create a NEW building family in that shared style; do not copy either building.
Subject: URANIUM MILL — TIER N of 4.
Family identity: a mill that crushes Ferrite Ore and separates fissile grains from it. A covered ore hopper fed by an inclined conveyor, a crusher house, a row of tall slender separation columns, and a heavy shielded packing hall with a thick, windowless concrete-and-steel wall where the finished Uranium is sealed into small drums. Clearly different from the Smelter's furnace halls and the Fuel Refinery's distillation towers.
This tier specifically: [see the list below].
Tier family progression for context (do NOT draw a lineup): 1: one hopper, a short conveyor, a small crusher house, two separation columns and a small shielded packing shed; 2: a larger crusher house, four separation columns and a longer packing hall; 3: twin hoppers, a tall crusher block, six columns and a heavier shielded hall with a drum store; 4: a large mill: a tall crusher tower, a double row of columns and a long shielded packing hall with a loading bay, on the same compact site.
Output: exactly the requested tier alone, genuine transparent PNG.
```

**"This tier specifically" for each image:**

1. one ore hopper fed by a short conveyor, a small crusher house, two separation columns and a small shielded packing shed.
2. a larger crusher house, four separation columns and a longer shielded packing hall.
3. twin ore hoppers, a tall crusher block, six separation columns and a heavier shielded hall with a covered drum store.
4. a large mill: a tall crusher tower, a double row of separation columns and a long shielded packing hall with a loading bay.

## 2. Antimatter Plant, tiers I–IV

The last tech's factory: Deuterium and ten times a factory's power go in,
a few grains of Antimatter come out. It should read as rare, precise and
power-hungry, the most advanced building a corporation can own.

**Attach for every tier:**

- Image 1: `client/art-src/planet-surface-structures-v2/buildings/drive_works/tier-1.png`
- Image 2: `client/art-src/planet-surface-structures-v2/buildings/fuel_refinery/tier-1.png`

**Save as:**
`client/art-src/planet-surface-structures-v2/buildings/antimatter_plant/tier-1.png` …
`tier-4.png`, each with a `tier-N.prompt.json` beside it.

```text
Use case: stylized-concept.
Asset type: ONE standalone planetary building-layer sprite, square transparent PNG, approximately 1024 x 1024.
Purpose: reusable architecture above a SEPARATE biome ground layer. Produce BUILDING ONLY, not a diorama. Genuine zero-alpha transparency around the building; preserve translucent soft shadow. No baked soil, gravel apron, grass, rocks around the site, concrete base tile, raised platform, roads, path, border or landscape. Small integral ramps, steps, pipe racks, conduits and support feet belonging to the structure are allowed.
Style: the approved new Boreal/Temperate structures, grounded near-future frontier engineering. Soft semi-realistic environmental 3D rendering, matte weathered steel, pale-stone and slate panels with dark grey trim, restrained amber service lights and a few warm windows. Large readable forms, no ink outlines, no cel shading, plastic gloss, extreme contrast, excessive greebles or neon. No text, logos, Authority branding, numerals, UI, people, vehicles or spacecraft. No glow haze, beams, sparks or energy effects.
Camera and lighting: elevated front-left three-quarter view about 55 degrees above the ground plane; more roof than wall, mild perspective; front and right walls visible. Soft upper-left daylight, neutral/cool fill and semi-transparent soft lower-right cast/contact shadow attached to the feet.
Framing/alignment: complete compact structure centered in a square. Building width about 94% of the canvas (x ≈ 4–98%), never cropped. Keep the ground footprint centred at approximately x=50%; its near/front foot reaches y≈88%, and its far/rear foot y≈45%. Keep a similar physical site footprint through upgrades; add height and machine mass, not a sprawling campus.
Output exactly one structure, no tier lineup/contact sheet.
Input image 1 is the house style for advanced precision manufacturing. Input image 2 is the house style for fuel processing (tanks, pipework). Create a NEW building family in that shared style; do not copy either building.
Subject: ANTIMATTER PLANT — TIER N of 4.
Family identity: a particle factory. A long, low accelerator ring housing, half-buried and armoured, runs around a central collider hall; heavy power conduits arrive from a transformer yard; insulated Deuterium feed tanks stand on one side; a small, heavily shielded containment vault with thick doors stores the finished magnetic bottles. Pale ceramic and dark steel, very clean, restrained cyan-white service lights. The precision reads through the machinery. Clearly different from the Drive Works' assembly halls and the Fuel Refinery's distillation towers.
This tier specifically: [see the list below].
Tier family progression for context (do NOT draw a lineup): 1: a small accelerator ring, one collider hall, two feed tanks and a small containment vault; 2: a larger ring, a taller collider hall and a wider transformer yard; 3: a double ring around a domed collider hall, four feed tanks and a larger vault; 4: a great antimatter plant: a large double ring, a tall domed collider hall, a full transformer yard and a fortified vault, on the same compact site.
Output: exactly the requested tier alone, genuine transparent PNG.
```

**"This tier specifically" for each image:**

1. a small half-buried accelerator ring around one collider hall, two Deuterium feed tanks and a small containment vault.
2. a larger ring, a taller collider hall and a wider transformer yard.
3. a double accelerator ring around a domed collider hall, four feed tanks and a larger containment vault.
4. a great antimatter plant: a large double ring, a tall domed collider hall, a full transformer yard and a fortified containment vault.

## 3. Goods icons: Uranium, Antimatter, Fusion Core

These follow the ore icon set in `client/art-src/ores-2026-09-16/` (its
`prompts.json` holds the shared preamble used below).

**Save as:** `client/art-src/fuel-goods-2026-10-12/uranium.png`,
`antimatter.png` and `fusion_core.png`, with a `prompts.json` beside them in
the ores set's format.

```text
Use case: stylized-concept. Asset type: sci-fi strategy game inventory resource icon for Stellar Syndicates. A single centered resource object, polished hard-surface 3D game icon, crisp broad forms readable at 32–64 px, 3/4 view, restrained texture, strong silhouette, studio light from upper left. Object occupies 80% of square canvas. Genuine transparent RGBA background; no scene, ground, border, frame, text, labels, watermark, glow haze, UI or extra props. [subject]
```

**The subject for each icon:**

- **Uranium:** a short, sturdy sealed steel drum, tipped slightly, beside a small
  stack of dull olive-grey ceramic fuel pellets. Matte, heavy, industrial; no
  radiation symbol, no green glow.
- **Antimatter:** a compact magnetic containment bottle: a clear cylindrical
  vessel held between two heavy ribbed coil end-caps, with a tiny suspended
  violet-white point at its centre. Precise, expensive, laboratory-clean; the
  point is small and crisp, not a glow.
- **Fusion Core:** a dense cylindrical reactor core module: a polished
  graphite-and-steel cylinder banded by three field-coil rings, a ceramic
  heat-shield collar and an Authority-grey service plate. Reads as a precision
  component, heavier and more industrial than the Warp Core.

## When it lands

- The derive step adds `uranium_mill` and `antimatter_plant` to the v2 family
  manifest at four tiers each; their menus use their scene sprites.
- The Fuel Refinery stand-ins come out of `icons.ts` and
  `planet-structure-art.ts`.
- The icons derive to `/art/ui_icons/resource/fuel-goods-2026-10-12/` at 64
  and 128 px; `commodityIcon` lists them beside the ores, and the three glyph
  placeholders in `icons.ts` go.
