# Power Plant art

This is an art request for the user's other image model. It has two parts:

1. Six world-view sprites for the **Power Plant**, tiers I–VI. §fuel-overhaul
   (Oct 12 2026): tiers I–II are **Solar Arrays** and burn nothing; tiers
   III–VI are **Fusion Plants**, reactor halls built around the Authority's
   Fusion Core. III–IV burn Deuterium; V–VI burn Helium-3. The plant keeps
   Solar II's output free, but the fusion art shows the reactor only: no
   panels (Oct 12: Peter preferred it without).
2. Two **orbital** versions for plants on asteroid belts and gas giants, which
   have no ground: one solar (I–II) and one fusion (III–VI).

The combustion plants of the first brief are gone: nothing in the game burns
fuel for power below fusion any more.

Both follow the existing v2 planet-surface sets in
`client/art-src/planet-surface-structures-v2/`, so they match every other
building in camera, light, scale and finish. Until they land, the game draws
the Fuel Refinery's art for the plant.

**Framing:** this brief uses the measured v2 framing, not the original prompts'
numbers (see `docs/living-terraces-art-prompt.md`): the building about 94% of
the canvas wide, its front foot at y ≈ 88%.

## 1. Power Plant, tiers I–VI

Generate one image per tier with the template below. For each tier, change only
the line marked "This tier specifically".

**Attach for every tier:**

- Image 1: `client/art-src/planet-surface-structures-v2/buildings/fuel_refinery/tier-1.png`
- Image 2: `client/art-src/planet-surface-structures-v2/buildings/smelter/tier-1.png`

**Save as:**
`client/art-src/planet-surface-structures-v2/buildings/power_plant/tier-1.png` …
`tier-6.png`, each with a `tier-N.prompt.json` beside it, like the other
families.

```text
Use case: stylized-concept.
Asset type: ONE standalone planetary building-layer sprite, square transparent PNG, approximately 1024 x 1024.
Purpose: reusable architecture above a SEPARATE biome ground layer. Produce BUILDING ONLY, not a diorama. Genuine zero-alpha transparency around the building; preserve translucent soft shadow. No baked soil, gravel apron, grass, rocks around the site, concrete base tile, raised platform, roads, path, border or landscape. Small integral ramps, steps, pipe racks, panel stands and support feet belonging to the structure are allowed.
Style: the approved new Boreal/Temperate structures, grounded near-future frontier engineering. Soft semi-realistic environmental 3D rendering, matte weathered steel, pale-stone and slate panels with dark grey trim, restrained amber service lights and a few warm windows. Large readable forms, no ink outlines, no cel shading, plastic gloss, extreme contrast, excessive greebles or neon. No text, logos, Authority branding, numerals, UI, people, vehicles or spacecraft. No lightning bolts or glowing energy effects; power reads through the machinery.
Camera and lighting: elevated front-left three-quarter view about 55 degrees above the ground plane; more roof than wall, mild perspective; front and right walls visible. Soft upper-left daylight, neutral/cool fill and semi-transparent soft lower-right cast/contact shadow attached to the feet.
Framing/alignment: complete compact structure centered in a square. Building width about 94% of the canvas (x ≈ 4–98%), never cropped. Keep the ground footprint centred at approximately x=50%; its near/front foot reaches y≈88%, and its far/rear foot y≈45%. This is a reusable stable site anchor, not a floating base. Keep a similar physical site footprint through upgrades; add height, panels and reactor mass, not a sprawling campus.
Output exactly one structure, no tier lineup/contact sheet.
Input image 1 is the house style for HEAVY INDUSTRY (tanks, pipework, panels, scale). Input image 2 is the house style for hot process plant (halls, heavy machinery). Create a NEW building family in that shared style; do not copy either building.
Subject: POWER PLANT — TIER N of 6.
Family identity: the planet's power station. Tiers I–II are SOLAR ARRAYS: tilted rows of dark blue-grey photovoltaic panels on steel stands, an inverter house with a low ridged roof, and a fenced transformer yard of insulators and busbars. No stacks, tanks or furnaces: sunlight burns nothing. Tiers III–VI are FUSION PLANTS: a massive domed or toroidal reactor hall ringed by field-coil housings, a heavy shielded core block at its heart, banks of pale radiator fins and a transformer yard. No solar panels and no stacks on the fusion tiers. III–IV store Deuterium in squat insulated tanks; V–VI add frosted cryogenic Helium-3 tanks. A recognizable power-station silhouette, clearly different from the Fuel Refinery's distillation towers and the Smelter's furnace halls.
This tier specifically: [see the list below].
Tier family progression for context (do NOT draw a lineup): 1: a small solar array, three short rows of panels, a small inverter house and a small transformer yard; 2: a larger solar array, five panel rows climbing to a taller inverter hall, a wider transformer yard; 3: a first fusion plant: one compact domed reactor hall ringed with field-coil housings, two squat insulated Deuterium tanks and a bank of radiator fins; 4: a larger fusion plant: a taller domed reactor hall, a control block, four Deuterium tanks and taller radiator banks; 5: a Helium-3 fusion plant: a broad toroidal reactor hall under a ribbed dome, frosted cryogenic Helium-3 tanks and wide radiator banks; 6: a great Helium-3 fusion plant: a larger toroidal reactor hall with a second reactor hall beside it, tall radiator banks and a row of frosted cryogenic tanks, on the same compact site.
Output: exactly the requested tier alone, genuine transparent PNG.
```

**"This tier specifically" for each image:**

1. a small solar array: three short rows of tilted photovoltaic panels on steel stands, a small inverter house and a small transformer yard.
2. a larger solar array: five panel rows stepping up toward a taller inverter hall, and a wider transformer yard.
3. a first fusion plant: one compact domed reactor hall ringed with field-coil housings, two squat insulated Deuterium tanks and a bank of radiator fins. No solar panels, no stacks.
4. a larger fusion plant: a taller domed reactor hall joined to a control block, four insulated Deuterium tanks and taller radiator banks. No solar panels.
5. a Helium-3 fusion plant: a broad toroidal reactor hall under a ribbed dome, frosted cryogenic Helium-3 tanks and wide radiator banks. No solar panels.
6. a great Helium-3 fusion plant: a larger toroidal reactor hall under a ribbed dome with a second reactor hall beside it, tall radiator banks and a row of frosted cryogenic tanks, on the same compact site. No solar panels.

## 2. Orbital Power Plant, two looks

Belts and gas giants have no ground, so their plant floats.

**Attach for both:**

- Image 1: `client/art-src/planet-shipyards-v1/naval_drydock/tier-1.png`
- Image 2: the matching ground tier from part 1 (`power_plant/tier-2.png` for
  solar, `power_plant/tier-6.png` for fusion)

**Save as:**
`client/art-src/planet-surface-structures-v2/buildings/power_plant/orbital-solar.png`
and `orbital-fusion.png`, each with a `.prompt.json` beside it.

```text
Use case: stylized-concept.
Asset type: a single standalone orbital power-station sprite for a space strategy game's planetary view, with genuine transparent alpha. Square canvas, approximately 1024 x 1024.
Input image 1 is a MATERIAL/RENDERING reference only, the approved Orbital Shipyard. Do NOT copy its rails, gantries or berth. Input image 2 is the FAMILY reference: the same plant on the ground. Keep its identity and materials, but rebuild it as a free-floating station: no ground, feet, shadow on terrain, roads or landscape.
Create one FREE-FLOATING SPACE STRUCTURE, compact and readable at small size: [see the list below]. A short docking arm with cyan docking lights serves the supply tenders. Near-future frontier engineering, matte slate graphite steel and pale grey panels, restrained amber work lights, large readable forms, realistic restrained detail, soft semi-realistic environmental 3D rendering, no ink outlines or cel shading. No glow haze or energy beams.
Camera: elevated three-quarter front-left, about 30 degrees, matching the orbital yards. Soft upper-left light with neutral fill. The structure about 90% of the canvas wide, centred, never cropped.
No planet, sky, stars, horizon, text, logos, people or spacecraft.
Output exactly one structure, genuine transparent PNG.
```

**The subject for each image:**

- **Solar:** a slim central truss spine with a compact inverter module and
  relay mast, carrying two long wings of dark blue-grey photovoltaic panels
  to either side.
- **Fusion:** a toroidal fusion reactor ringed by field-coil housings around a
  shielded core block, with frosted cryogenic tanks clustered at its hub and
  wide pale radiator wings.

## When it lands

- The derive step adds `power_plant` to the v2 family manifest. The plant's
  menus use its scene sprites, as the Living Terraces' do.
- The Fuel Refinery stand-in comes out of `icons.ts` and
  `planet-structure-art.ts`.
- Belts and gas giants draw the orbital pair: solar for I–II, fusion for
  III–VI.
