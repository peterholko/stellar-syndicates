# Veil art

This is an art request for the user's other image model, Nov 2 2026. It covers
the two pieces the Veil (`docs/veil.md`) still borrows:

1. Three building sprites for the **Veil Projector**, levels I–III. Until they
   land the projector wears the Sensor Array's icon in every list, structure row
   and detail panel, so a world with both shows two identical Infrastructure rows.
2. Two **research emblems**, Ionospheric Veil (Countermeasures III) and Deep Veil
   (Countermeasures IV). Until they land both show the Weapons field emblem, like
   the other 119 programmes. Their prompts are also in
   `client/art-src/research-emblems-v1/prompts.json` and the tables in
   `research-art-prompts.md`; they ride along with that batch.

**No new art for:**

- the Veil itself: the aurora curtain over the world on the planet stage and the
  breathing shell around it in the System View are drawn in code
  (`planet-panel.css` §veil, `systemtrafficlayer.ts` `COL_VEIL`);
- a planet-stage building: the projector is off-site like the Vault and the
  Assembly Hall and never draws on the surface. The sprite serves the icon, the
  structure row and the detail panel only (the Power Plant route: `icons.ts`
  `sceneOnly` loads the surface sprite as the icon).

## 1. Veil Projector, levels I–III

The sprites follow the v2 planet-surface set in
`client/art-src/planet-surface-structures-v2/`, so they match every other
building in camera, light, scale and finish.

**Framing:** the measured v2 framing, not the original v2 prompts' numbers
(see `living-terraces-art-prompt.md`): the building about 94% of the canvas
wide (x ≈ 4–98%), its front foot at y ≈ 88%, rear foot y ≈ 45%. The approved
Power Plant tiers measure x 1–99%, foot y 84–93%; the Sensor Array family alone
sits narrower (x ≈ 11–95%) for antenna margin. The projector is a ring of masts
around a hall, not one thin dish, so it takes the Power Plant's fuller width.
Mast tips may reach y ≈ 4%; nothing cropped.

**Attach for every level:**

- Image 1: `client/art-src/planet-surface-structures-v2/buildings/sensor_array/tier-3.png`
  (house style for infrastructure masts and equipment houses)
- Image 2: `client/art-src/planet-surface-structures-v2/buildings/power_plant/tier-3.png`
  (the Fusion Plant the projector runs off; its field-coil housings, radiator
  fins and insulated tanks are the material family)

**Save as:**
`client/art-src/planet-surface-structures-v2/buildings/veil_projector/tier-1.png`
… `tier-3.png`, each with a `tier-N.prompt.json` beside it, like the other
families.

```text
Use case: stylized-concept.
Asset type: ONE standalone planetary building-layer sprite, square transparent PNG, approximately 1024 x 1024.
Purpose: reusable architecture above a SEPARATE biome ground layer. Produce BUILDING ONLY, not a diorama. Genuine zero-alpha transparency around the building; preserve translucent soft shadow. No baked soil, gravel apron, grass, rocks around the site, concrete base tile, raised platform, roads, path, border or landscape. Small integral ramps, steps, cable trays, insulator stands and support feet belonging to the structure are allowed.
Style: the approved new Boreal/Temperate structures, grounded near-future frontier engineering. Soft semi-realistic environmental 3D rendering, matte weathered steel, dark graphite lattice, pale-stone and slate panels with dark grey trim, restrained amber service lights and tiny cyan standby indicators. Large readable forms, no ink outlines, no cel shading, plastic gloss, extreme contrast, excessive greebles or neon. No text, logos, Authority branding, numerals, UI, people, vehicles or spacecraft. No beam, no aurora, no glowing sky, no lightning and no energy effects: the structure is DORMANT; the game draws its effect.
Camera and lighting: elevated front-left three-quarter view about 55 degrees above the ground plane; more roof than wall, mild perspective; front and right walls visible. Soft upper-left daylight, neutral/cool fill and semi-transparent soft lower-right cast/contact shadow attached to the feet.
Framing/alignment: complete compact structure centered in a square. Building width about 94% of the canvas (x ≈ 4–98%), never cropped; mast tips may rise to y ≈ 4%. Keep the ground footprint centred at approximately x=50%; its near/front foot reaches y≈88%, and its far/rear foot y≈45%. This is a reusable stable site anchor, not a floating base. Keep a similar physical site footprint through upgrades; add mast height, mast count and capacitor mass, not a sprawling campus.
Output exactly one structure, no tier lineup/contact sheet.
Input image 1 is the house style for INFRASTRUCTURE (equipment houses, masts, instrument heads, scale). Input image 2 is the house style for the fusion power station this projector runs off (field-coil housings, radiator fins, insulated tanks). Create a NEW building family in that shared style; do not copy the dish or the reactor dome.
Subject: VEIL PROJECTOR — LEVEL N of 3.
Family identity: the colony's ionospheric Veil projector, an array of tall slender emitter masts standing in a ring around a squat shielded capacitor hall. Each mast is a dark graphite lattice tower with insulator stacks and a flat disc emitter head, like an ionospheric heater antenna: no dish, no turret, no barrel. Heavy braided power conduits run from the hall to the foot of every mast, and one thick conduit trunk leaves the site at the right edge toward the off-image Fusion Plant. The hall carries a short bank of pale radiator fins. A recognizable antenna-field silhouette, clearly different from the Sensor Array's parabolic dish and the Defense Platform's turret.
This level specifically: [see the list below].
Level family progression for context (do NOT draw a lineup): 1: four short emitter masts at the corners of a small capacitor hall, one conduit trunk; 2: six taller masts in a ring around a larger domed capacitor hall with a radiator bank, thicker conduits; 3: eight masts in the ring and one taller central mast rising over the hall, the hall grown a second capacitor block and a small transformer yard, two conduit trunks, on the same compact site.
Output: exactly the requested level alone, genuine transparent PNG.
```

**"This level specifically" for each image:**

1. four short emitter masts at the corners of a small flat-roofed capacitor
   hall, braided conduits to each foot, one conduit trunk leaving to the right.
2. six taller emitter masts in a ring around a larger domed capacitor hall with
   a bank of pale radiator fins; thicker conduits; the same single trunk.
3. eight emitter masts in the ring and one taller central mast rising over the
   hall; the hall has grown a second capacitor block and a small fenced
   transformer yard; two conduit trunks leave to the right. Same compact site.

### Landing it (code, after the art)

- Add the three entries to
  `client/art-src/planet-surface-structures-v2/additions-manifest.json` under
  `buildings` (family `veil_projector`, tiers 1–3, the Power Plant entries'
  shape) and run `npm run assets:surface-structures`.
- `icons.ts`: `structureVeilProjector: S("veil_projector", "Veil Projector")`,
  `veil_projector` in `sceneOnly`, and the structure map's
  `veil_projector: "structureVeilProjector"` in place of the Sensor Array stand-in.
- Nothing on the planet stage: `planet.ts` and `planet-sites.ts` keep the
  projector off-site.

## 2. Research emblems: Ionospheric Veil, Deep Veil

Two more rows for the Weapons table in `research-art-prompts.md` (now 23 Weapons,
121 in all), batch 2. The shared prompt, badge, framing and checks are that
doc's. Save as `client/art-src/research-emblems-v1/<key>.png`, 1254 × 1254,
real alpha. Image 1 is the field sheet; no Image 2.

**One palette departure, on purpose:** the Weapons palette is silver, signal red
and an orange-yellow flash. These two emblems draw the aurora curtain in the
game's Veil colour, a pale green-cyan (#7DF9C6), and keep the red for the thing
breaking up in it, so the pair reads both as Weapons and as the Veil the player
has seen over a world. Nothing else in the Weapons set changes.

| File | Name | Tier | Image 2 | `{motif}` |
|---|---|---|---|---|
| `weap_cm_iii_ionospheric_veil.png` | Ionospheric Veil | III |  | A curved planet horizon across the lower face with one slender ground emitter mast on it; a shimmering aurora curtain hangs across the sky above; one small falling shell breaks apart in the curtain with a red flash. |
| `weap_cm_iv_deep_veil.png` | Deep Veil | IV |  | The same aurora curtain thickened into three layered bands over a curved planet horizon; one heavy blunt troop transport pushes down through it nose first, its hull edges glowing where the curtain bites, a red flash at its flank. |

`{palette}` for both: `silver metal with a signal-red accent and a small
orange-yellow flash; the aurora curtain alone is a pale green-cyan glow
(#7DF9C6)`.

## Landed (Nov 2 2026)

- All five masters were generated with the built-in image tool at 1254 × 1254
  with genuine alpha. Their sibling `.prompt.json` files retain the reference
  hashes, original prompts, corrective edits and validation measurements.
- The three projector tiers are in `buildings/veil_projector/` and
  `additions-manifest.json`. The surface builder derives 128/256/512 WebP
  images. `structureVeilProjector` uses them in every icon, row and detail
  panel, with its art lookup capped at III. The projector remains off-site.
- Both emblems are in `research-emblems-v1/`; the research builder derives
  64/128/256/512 WebP images and selects them by programme id. Green-cyan is
  reserved for the curtain; red marks the breaking shell or damaged transport.
- The existing Veil unlock-table entries remain. Deep Veil's unlock picture
  now depicts and captions tier III, rather than the generic tier-I building
  preview used for ordinary structure unlocks.
