# Living Terraces art and the Marginal ground patch

This is an art request for the user's other image model. It has two parts:
1. Six world-view sprites for **Living Terraces**, the Gaia-only garden housing.
2. The missing **Marginal** ground patch that goes under buildings on Marginal
   worlds.

Both follow the existing v2 planet-surface sets in
`client/art-src/planet-surface-structures-v2/`, so they match every other
building in camera, light, scale and finish.

## 1. Living Terraces, tiers I–VI

Generate one image per tier with the template below. For each tier, change only
the line marked "This tier specifically".

**Attach for every tier:**
- Image 1: `client/art-src/planet-surface-structures-v2/buildings/habitat/tier-1.png`
- Image 2: `client/art-src/planet-surface-structures-v2/buildings/agroplex/tier-1.png`

**Save as:**
`client/art-src/planet-surface-structures-v2/buildings/living_terraces/tier-1.png` … `tier-6.png`,
each with a `tier-N.prompt.json` beside it, like the other families.

```text
Use case: stylized-concept.
Asset type: ONE standalone planetary building-layer sprite, square transparent PNG, approximately 1024 x 1024.
Purpose: reusable architecture above a SEPARATE biome ground layer. Produce BUILDING ONLY, not a diorama. Genuine zero-alpha transparency around the building; preserve translucent soft shadow. No baked soil, gravel apron, moss, grass, rocks around the site, concrete base tile, raised platform, roads, path, border or landscape. Small integral ramps/steps/support feet belonging to the structure are allowed. Planted roofs and terrace gardens are PART OF THE BUILDING and are allowed; nothing grows around it.
Style: the approved new Boreal/Temperate structures, grounded near-future frontier engineering. Soft semi-realistic environmental 3D rendering, matte weathered steel, warm cream and pale-stone panels with grey-green trim, restrained amber service lights and warm windows. Large readable forms, no ink outlines, no cel shading, plastic gloss, extreme contrast, excessive greebles or neon. No text, logos, Authority branding, numerals, UI, people, vehicles or spacecraft.
Camera and lighting: elevated front-left three-quarter view about 55 degrees above the ground plane; more roof than wall, mild perspective; front and right walls visible. Soft upper-left daylight, neutral/cool fill and semi-transparent soft lower-right cast/contact shadow attached to the feet.
Framing/alignment: complete compact structure centered in a square. Building width about 65–70% of canvas, with ample transparent margin. Keep the ground footprint centred at approximately x=50%, y=65%; its near/front foot reaches y≈82%, and its far/rear foot y≈45%. This is a reusable stable site anchor, not a floating base. Never crop. Keep a similar physical site footprint through upgrades; add height and terraces, not a sprawling campus.
Output exactly one structure, no tier lineup/contact sheet.
Input image 1 is the house style for frontier HOUSING (panels, windows, roof equipment, scale). Input image 2 is the house style for living crops under glass. Create a NEW building family in that shared style; do not copy either building.
Subject: LIVING TERRACES — TIER N of 6.
Family identity: stepped garden housing for a lush Gaia world. Homes are built as stepped terraces, each step's roof a planted garden of vegetable beds, low shrubs and small fruit trees, with warm-lit windows and small glazed sunrooms on the step faces. Cream and pale-stone walls, grey-green steel trim, slim irrigation piping. A recognizable stepped, green-topped silhouette that reads as "people live here and grow their own food", clearly different from the grey Habitat blocks and the barrel-vaulted Agroplex greenhouses.
This tier specifically: [see the list below].
Tier family progression for context (do NOT draw a lineup): 1: a low two-step terrace of garden homes with planted roofs and one glazed sunroom; 2: three stepped terraces with roof gardens and a small rainwater cistern; 3: four stepped levels wrapped around a small planted courtyard, greenery spilling over the edges; 4: five stepped levels with rooftop orchards and a glazed walkway between wings; 5: a stepped garden pyramid of cascading planted terraces around a central glass atrium; 6: a tall stepped garden ziggurat with lush terraces on every level, small trees on the upper steps and a thin waterfall falling between levels, same compact site.
Output: exactly the requested tier alone, genuine transparent PNG.
```

**"This tier specifically" for each image:**
1. a low two-step terrace of garden homes with planted roofs and one glazed sunroom.
2. three stepped terraces with roof gardens and a small rainwater cistern.
3. four stepped levels wrapped around a small planted courtyard, greenery spilling over the edges.
4. five stepped levels with rooftop orchards and a glazed walkway between wings.
5. a stepped garden pyramid of cascading planted terraces around a central glass atrium.
6. a tall stepped garden ziggurat with lush terraces on every level, small trees on the upper steps and a thin waterfall falling between levels, on the same compact site.

## 2. Marginal ground patch

Buildings on Marginal worlds currently stand without a ground patch, because
every other world type has one and this one was missed.

**Attach:**
- Image 1: `client/art-src/planet-view/surfaces/marginal-surface-v1.png`
- Image 2: `client/art-src/planet-surface-structures-v2/grounds/desert.png`

**Save as:** `client/art-src/planet-surface-structures-v2/grounds/marginal.png`
(with `marginal.prompt.json`).

```text
Use case: stylized-concept.
Asset type: ONE standalone biome GROUND layer, square transparent PNG, approximately 1024 x 1024, for use underneath a separate colony building sprite.
Primary request: a modest natural ground transition patch with an EMPTY flat center for a building, no building in this image. True fully transparent alpha outside the patch, irregular softly feathered edge. The patch is a thin surface texture, not a floating terrain island: no thickness, cutaway walls, curb, raised slab or platform.
Input image 1: biome terrain/color/lighting reference ONLY. Do not copy the whole landscape, horizon, sky, water, distant hills or rock outcrops.
Layout: a high-oblique ground footprint, view about 55 degrees downward. Center at canvas x=50%, y=65%; softly irregular diamond-like patch occupying roughly x=9–91%, y=35–91%, with transparent area above. Quiet compacted natural earth/grit in the middle where the future building will sit, richer low biome texture near the fringe. Entire patch and feathering well inset, nothing touches any canvas edge. Matte diffuse soft upper-left light, no hard directional building shadow because no building is present.
No structures, foundations shaped as buildings, roads, paths leaving the patch, markings, text, people, vehicles, trees, tall vegetation, opaque sky/background or checkerboard. No smooth hard oval sticker rim.
Input image 2: footprint alignment and soft alpha edge REFERENCE only. Match its overall empty flat patch layout and camera, but replace all its surface material with this requested biome.
Biome: marginal. Cold, dry, thin-aired ground: compacted dun and cool khaki soil, patches of low dusty sage-green lichen, a few pale salt-crusted spots and small grey stones, faint frost in tiny hollows. No grass meadow, no sand dunes.
If the landscape reference contains any man-made pads or roads, IGNORE them completely; output only natural empty ground.
```

## Delivered Oct 9 2026

Both parts are in the game. The masters are listed in
`client/art-src/planet-surface-structures-v2/additions-manifest.json`, and the
Agroplex stand-in is gone.

**For the next v2 addition, use the measured framing, not the prompt's.** The
numbers above come from the original v2 prompts, but the approved v2 art doesn't
match them:

| | Asked for | What the approved v2 set measures |
|---|---|---|
| Building width | 65–70% of canvas | about 94% (x 4–98%) |
| Building front foot | y ≈ 82% | y ≈ 87–94% |
| Ground patch | x 9–91%, y 35–91% | x 2–98%, y 25–95% |

The new pieces followed the prompt, so they came out smaller than their
neighbours: the terraces at x 16–87% and the Marginal ground at x 10–90%. The
art build now enlarges both to match (`FRAMING` in
`client/scripts/build-planet-surface-structure-art.mjs`). The user's image
pipeline normalises each sprite to the numbers in its prompt, so a brief with
the measured numbers needs no such fix.
