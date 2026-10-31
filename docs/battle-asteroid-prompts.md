# Battle asteroids: prompts

Rocks for the asteroid field behind a pirate den's battles (Oct 29 2026:
"some asteroid belt and a distant star"). These sheets give each belt type
six single asteroids in place of the System View's three-rock cluster: four
sheets, 24 rocks.

**Status:** delivered Oct 29 and integrated Oct 30 2026: 220 rocks per den
fight drawn from its belt's six (records in
`client/art-src/battle-asteroids-v1/README.md`).

## How to make them

- **ChatGPT:** one sheet per message. Attach the images listed for that
  sheet, in the order given (Image 1 first), then paste its prompt.
- **Size:** ask for the widest landscape size (1536 × 1024) with a
  transparent background.
- **Saving:** save each sheet as delivered under its file name in
  `client/art-src/battle-asteroids-v1/`. Don't cut or crop it; I'll cut the
  six rocks apart along their transparent gaps, trim and size them, record
  the prompts and wire them into the den's backdrop.
- **Redo a sheet if** two rocks touch, the background isn't transparent, or
  the rocks are seen at an angle rather than from straight above.

## The set

| # | File | Belt | Attach |
|---|---|---|---|
| 1 | `stony-sheet.png` | Stony | `asteroid_belt_chunk.png`, `stony-belt-v1.png` |
| 2 | `metallic-sheet.png` | Metallic | `metallic.png`, `metallic-belt-v1.png` |
| 3 | `icy-sheet.png` | Icy | `icy.png`, `icy-belt-v1.png` |
| 4 | `shattered-sheet.png` | Shattered | `shattered.png`, `shattered-belt-v1.png` |

The attachments are all in the repository:

| File | Path |
|---|---|
| `asteroid_belt_chunk.png` | `client/public/art/celestial_sprites/planets/asteroid_belt_chunk.png` |
| `metallic.png` | `client/art-src/belt-chunks-v1/metallic.png` |
| `icy.png` | `client/art-src/belt-chunks-v1/icy.png` |
| `shattered.png` | `client/art-src/belt-chunks-v1/shattered.png` |
| `stony-belt-v1.png` | `client/art-src/belt-view/stony-belt-v1.png` |
| `metallic-belt-v1.png` | `client/art-src/belt-view/metallic-belt-v1.png` |
| `icy-belt-v1.png` | `client/art-src/belt-view/icy-belt-v1.png` |
| `shattered-belt-v1.png` | `client/art-src/belt-view/shattered-belt-v1.png` |

## Why they look like this

- **Straight top-down, lit from above.** The game turns each rock to a random
  angle. A rock lit from one side would show its bright side pointing every
  which way; even light from the viewer reads the same at any rotation.
- **Six shapes per belt.** Long, round, wedge, two-lobed, slab and compact:
  enough silhouettes that a field of 220 doesn't show repeats.
- **Each fills its cell.** The game sets every rock's size itself, so the
  sheet only needs them evenly large.

---

## 1 · `stony-sheet.png` — Stony belt

Save as `client/art-src/battle-asteroids-v1/stony-sheet.png`.

Attach:

1. `client/public/art/celestial_sprites/planets/asteroid_belt_chunk.png`
2. `client/art-src/belt-view/stony-belt-v1.png`

```text
Use case: stylized-concept. Asset type: a sprite sheet of six single asteroids for the battle background of Stellar Syndicates, a dark science-fiction strategy game. The game cuts the sheet into six separate sprites and scatters them, turned to any angle and at any size, beneath a space battle.
Image 1 is the game's existing rock sprite for this belt: match its rock material, colours and finish, but draw single rocks, not clusters.
Image 2 is the game's painting of this belt: a material and colour reference only. Do not copy its scene, camera or lighting.
Composition: one landscape 1536 x 1024 canvas holding exactly six separate asteroids in a 3 x 2 grid, each centred in its own 512 x 512 cell and filling about 70% of it, with at least 60 px of clear transparent space between any two rocks. Nothing crosses a cell's edge. No grid lines, cell borders or labels.
Camera: straight top-down, looking directly down on each rock (not a three-quarter or side view), so a rock can be turned to any angle.
Lighting: soft, even light from directly above, toward the viewer. Craters and crevices sit in soft shadow and each rock's outer edge falls slightly darker. No bright side, no highlight on one edge, no rim light and no cast shadow, so a rock reads the same at any rotation.
The six rocks, left to right, top row then bottom row:
1. a long, narrow potato-shaped rock;
2. a large rounded boulder with two shallow craters;
3. an angular wedge-shaped shard with flat broken faces;
4. a lumpy two-lobed rock, two boulders fused together;
5. a flat irregular slab with a chipped edge;
6. a chunky, compact rock with jagged edges.
Rock type: Grey-brown porous stony rock with pitted surfaces, a few pale chips and small glassy olive-green olivine flecks.
Style: realistic game sprite with a crisp silhouette that reads at 30 px, natural rock detail, restrained texture, no painterly blur.
Background: genuinely transparent PNG with real RGBA alpha; no starfield, dust, glow, haze, shadow plate, ground, frame, text, labels or watermark.
```

## 2 · `metallic-sheet.png` — Metallic belt

Save as `client/art-src/battle-asteroids-v1/metallic-sheet.png`.

Attach:

1. `client/art-src/belt-chunks-v1/metallic.png`
2. `client/art-src/belt-view/metallic-belt-v1.png`

```text
Use case: stylized-concept. Asset type: a sprite sheet of six single asteroids for the battle background of Stellar Syndicates, a dark science-fiction strategy game. The game cuts the sheet into six separate sprites and scatters them, turned to any angle and at any size, beneath a space battle.
Image 1 is the game's existing rock sprite for this belt: match its rock material, colours and finish, but draw single rocks, not clusters.
Image 2 is the game's painting of this belt: a material and colour reference only. Do not copy its scene, camera or lighting.
Composition: one landscape 1536 x 1024 canvas holding exactly six separate asteroids in a 3 x 2 grid, each centred in its own 512 x 512 cell and filling about 70% of it, with at least 60 px of clear transparent space between any two rocks. Nothing crosses a cell's edge. No grid lines, cell borders or labels.
Camera: straight top-down, looking directly down on each rock (not a three-quarter or side view), so a rock can be turned to any angle.
Lighting: soft, even light from directly above, toward the viewer. Craters and crevices sit in soft shadow and each rock's outer edge falls slightly darker. No bright side, no highlight on one edge, no rim light and no cast shadow, so a rock reads the same at any rotation.
The six rocks, left to right, top row then bottom row:
1. a long, narrow potato-shaped rock;
2. a large rounded boulder with two shallow craters;
3. an angular wedge-shaped shard with flat broken faces;
4. a lumpy two-lobed rock, two boulders fused together;
5. a flat irregular slab with a chipped edge;
6. a chunky, compact rock with jagged edges.
Rock type: Dark steel-grey metallic rock with bright silver fracture planes, small metallic glints and a few rust-brown patches; heavier and denser-looking than stony rock.
Style: realistic game sprite with a crisp silhouette that reads at 30 px, natural rock detail, restrained texture, no painterly blur.
Background: genuinely transparent PNG with real RGBA alpha; no starfield, dust, glow, haze, shadow plate, ground, frame, text, labels or watermark.
```

## 3 · `icy-sheet.png` — Icy belt

Save as `client/art-src/battle-asteroids-v1/icy-sheet.png`.

Attach:

1. `client/art-src/belt-chunks-v1/icy.png`
2. `client/art-src/belt-view/icy-belt-v1.png`

```text
Use case: stylized-concept. Asset type: a sprite sheet of six single asteroids for the battle background of Stellar Syndicates, a dark science-fiction strategy game. The game cuts the sheet into six separate sprites and scatters them, turned to any angle and at any size, beneath a space battle.
Image 1 is the game's existing rock sprite for this belt: match its rock material, colours and finish, but draw single rocks, not clusters.
Image 2 is the game's painting of this belt: a material and colour reference only. Do not copy its scene, camera or lighting.
Composition: one landscape 1536 x 1024 canvas holding exactly six separate asteroids in a 3 x 2 grid, each centred in its own 512 x 512 cell and filling about 70% of it, with at least 60 px of clear transparent space between any two rocks. Nothing crosses a cell's edge. No grid lines, cell borders or labels.
Camera: straight top-down, looking directly down on each rock (not a three-quarter or side view), so a rock can be turned to any angle.
Lighting: soft, even light from directly above, toward the viewer. Craters and crevices sit in soft shadow and each rock's outer edge falls slightly darker. No bright side, no highlight on one edge, no rim light and no cast shadow, so a rock reads the same at any rotation.
The six rocks, left to right, top row then bottom row:
1. a long, narrow potato-shaped rock;
2. a large rounded boulder with two shallow craters;
3. an angular wedge-shaped shard with flat broken faces;
4. a lumpy two-lobed rock, two boulders fused together;
5. a flat irregular slab with a chipped edge;
6. a chunky, compact rock with jagged edges.
Rock type: Blue-white ice-and-rock: frosted pale blue ice with translucent edges, embedded dark grit and fine fractures; no glow.
Style: realistic game sprite with a crisp silhouette that reads at 30 px, natural rock detail, restrained texture, no painterly blur.
Background: genuinely transparent PNG with real RGBA alpha; no starfield, dust, glow, haze, shadow plate, ground, frame, text, labels or watermark.
```

## 4 · `shattered-sheet.png` — Shattered belt

Save as `client/art-src/battle-asteroids-v1/shattered-sheet.png`.

Attach:

1. `client/art-src/belt-chunks-v1/shattered.png`
2. `client/art-src/belt-view/shattered-belt-v1.png`

```text
Use case: stylized-concept. Asset type: a sprite sheet of six single asteroids for the battle background of Stellar Syndicates, a dark science-fiction strategy game. The game cuts the sheet into six separate sprites and scatters them, turned to any angle and at any size, beneath a space battle.
Image 1 is the game's existing rock sprite for this belt: match its rock material, colours and finish, but draw single rocks, not clusters.
Image 2 is the game's painting of this belt: a material and colour reference only. Do not copy its scene, camera or lighting.
Composition: one landscape 1536 x 1024 canvas holding exactly six separate asteroids in a 3 x 2 grid, each centred in its own 512 x 512 cell and filling about 70% of it, with at least 60 px of clear transparent space between any two rocks. Nothing crosses a cell's edge. No grid lines, cell borders or labels.
Camera: straight top-down, looking directly down on each rock (not a three-quarter or side view), so a rock can be turned to any angle.
Lighting: soft, even light from directly above, toward the viewer. Craters and crevices sit in soft shadow and each rock's outer edge falls slightly darker. No bright side, no highlight on one edge, no rim light and no cast shadow, so a rock reads the same at any rotation.
The six rocks, left to right, top row then bottom row:
1. a long, narrow potato-shaped rock;
2. a large rounded boulder with two shallow craters;
3. an angular wedge-shaped shard with flat broken faces;
4. a lumpy two-lobed rock, two boulders fused together;
5. a flat irregular slab with a chipped edge;
6. a chunky, compact rock with jagged edges.
Rock type: Fractured pieces of a broken planet's crust: rust-red and charcoal rock with visible layered strata on sharp broken faces and small metallic veins; jagged, violent fracture lines.
Style: realistic game sprite with a crisp silhouette that reads at 30 px, natural rock detail, restrained texture, no painterly blur.
Background: genuinely transparent PNG with real RGBA alpha; no starfield, dust, glow, haze, shadow plate, ground, frame, text, labels or watermark.
```
