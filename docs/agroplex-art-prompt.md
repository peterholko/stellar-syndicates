# Agroplex art, tiers IV–VI

This is an art request for the user's other image model. It re-renders the top
three Agroplex tiers in `client/art-src/planet-surface-structures-v2/`.

## Why

The Agroplex is the right size, but its levels barely change. The original
prompts planned a climb from low greenhouse bays to a vertical farm. Every tier
was generated from the tier-1 image, though, so tiers IV–VI came out as the
same two barrel-vault greenhouses, a little larger each time. At game size,
levels I, II, IV and V look alike. Tiers I–III match their descriptions and are
kept.

## Prompt

Generate tiers 4, 5 and 6 in that order, with the template below. For each tier,
change only the line marked "This tier specifically".

**Attach:**
- Image 1, for every tier:
  `client/art-src/planet-surface-structures-v2/buildings/agroplex/tier-3.png`.
- Image 2, for tiers 5 and 6 only: the new tier just below, so each level grows
  from the one before.

**Save as:**
`client/art-src/planet-surface-structures-v2/buildings/agroplex/redo/tier-4.png`,
`tier-5.png` and `tier-6.png`, each with a `tier-N.prompt.json` beside it. The
originals stay where they are, because the art build checks them against their
recorded hashes.

```text
Use case: stylized-concept.
Asset type: ONE standalone planetary building-layer sprite, square transparent PNG, approximately 1024 x 1024.
Purpose: reusable architecture above a SEPARATE biome ground layer. Produce BUILDING ONLY, not a diorama. Genuine zero-alpha transparency around the building; preserve translucent soft shadow. No baked soil, gravel apron, moss, grass, rocks around the site, concrete base tile, raised platform, roads, path, border or landscape. Small integral ramps/steps/support feet belonging to the structure are allowed.
Style: the approved new Boreal/Temperate structures, grounded near-future frontier engineering. Soft semi-realistic environmental 3D rendering, matte weathered steel, subdued grey/slate/forest-green panels, restrained amber/cyan service lights. Large readable forms, no ink outlines, no cel shading, plastic gloss, extreme contrast, excessive greebles or neon. No text, logos, Authority branding, numerals, UI, people, vehicles or spacecraft.
Camera and lighting: elevated front-left three-quarter view about 55 degrees above the ground plane; more roof than wall, mild perspective; front and right walls visible. Soft upper-left daylight, neutral/cool fill and semi-transparent soft lower-right cast/contact shadow attached to the feet.
Framing/alignment (match the approved Agroplex tiers 1-3 exactly): solid building width about 94% of the canvas (x about 3-97%), centred at x=50%. Its near/front foot, the lowest solid point, at y about 88%. Same ground footprint as tier 3: the low greenhouse bays stay at ground level and all new height grows upward from them. Keep at least 3% transparent margin above the tallest point; if a tower would rise higher, make it slimmer or lower, never shrink the footprint, never crop.
Output exactly one structure, no tier lineup/contact sheet.
Input image 1 is the approved Agroplex tier 3. Keep its architecture, materials, grey-green ribs, glazing, visible crop rows, nutrient tanks and finish; this is the same building family upgraded, not a new design. Input image 2, when attached, is the tier just below this one; grow from it.
Subject: AGROPLEX - TIER N of 6.
Family identity: low glazed crop-production bays with grey-green ribs, visible green crop rows, compact packing/control rooms and squat nutrient tanks. Recognizable industrial greenhouse silhouette. Crops grow only under glass: no planted roofs, roof gardens, trees or stepped garden terraces (those belong to a different building).
This tier specifically: [see the list below].
Tier family progression for context (do NOT draw a lineup): 1: two low attached greenhouse bays; 2: two reinforced glazed bays with a taller rear packing room; 3: three compact growing bays with a raised climate core; 4: the three bays carry a second glazed gallery deck; 5: a three-deck glass growing hall around a central service core; 6: a vertical-farm tower of stacked glazed crop decks rising from the same bays, same compact site.
Output: exactly the requested tier alone, genuine transparent PNG.
```

**"This tier specifically" for each image:**

4. the three glazed growing bays of tier 3 now carry a second glazed gallery
   deck along their full length, reached by an external stair and lift tower,
   with a larger nutrient tank.
5. a three-deck glass growing hall: three stacked glazed galleries with
   grey-green ribs and crop rows visible on every deck, around a central service
   and climate core with ducts; low growing bays and nutrient tanks still at its
   foot.
6. a vertical farm: a tall glazed growing tower of five or six stacked crop
   decks with grey-green ribs and a rooftop climate plant, rising from the same
   low greenhouse bays, with twin nutrient tanks, on the same compact site.

## Delivered Oct 9 2026

The three images arrived as `buildings/agroplex/tier-4-v2.png`, `tier-5-v2.png`
and `tier-6-v2.png`, beside the originals. The user's pipeline normalised them
to the brief's framing: 94% solid width with the front foot at 88%, which
matches tiers I–III.

- `additions-manifest.json` lists them under `replacements`. Each entry's
  `replaces` names the original master.
- `scripts/build-planet-surface-structure-art.mjs` ships each replacement under
  the original's id, so the game asks for the same image paths as before.
- `test-planet-surface-structure-art.mjs` checks the swap and that the
  originals are still on disk.

**One weak step remains: VI is less bulky than V.** V is a two-deck glass hall
that fills the site, with its top at 5% of the canvas. VI is a tower behind
single-height bays, with its top at 9% and less glass overall (45% solid area
against 51%). The cause is the brief: tier 6 said the tower rises "from the same
low greenhouse bays", so the art dropped V's hall. A redo of VI only would keep
V's hall as the base, with the tower rising from its rear. It can't get much
taller than V, because V already reaches 5% and the brief requires 3%
clearance. The gain would be bulk, not height.
