# Belt view: painted asteroid fields

A belt's world view used to show its map icon, a cluster of three rocks, blown
up as the scene's "globe". Since Oct 24 2026 it shows a painted asteroid field
of the belt's own type.

## The paintings

| Belt | Master |
|---|---|
| Stony | `client/art-src/belt-view/stony-belt-v1.png` |
| Metallic | `client/art-src/belt-view/metallic-belt-v1.png` |
| Icy | `client/art-src/belt-view/icy-belt-v1.png` |
| Shattered | `client/art-src/belt-view/shattered-belt-v1.png` |

- Size 1672 × 1181, like the planet views' orbit paintings: about 240 rows of
  quiet space on top for the orbit layer, the field in the 16:9 ground frame
  below (rows 240–1181) where every building site is measured.
- Made from one shared prompt with a belt-type line per painting (below);
  Peter's pipeline painted them at 1492 × 1054 and scaled them up 12%.

### Prompt

Attach Image 1: `client/art-src/planet-view/surfaces/barren-surface-v1-orbit.png`
(style, light and starfield only). Image 2: the type's chunk sprite (stony:
`client/public/art/celestial_sprites/planets/asteroid_belt_chunk.png`; others:
`client/art-src/belt-chunks-v1/{metallic,icy,shattered}.png`).

```text
Use case: environment-concept.
Asset type: empty asteroid-belt background for the planet view of Stellar Syndicates, a space strategy game.
Input images: Image 1 is an approved planet-view painting from the same game, used ONLY as the reference for rendering style, lighting, starfield, colour grading and finish. Do not copy its planet, terrain or horizon. Image 2 is the game's sprite for this belt's rock type, used ONLY as the reference for the rock material and colour.

Primary request: Paint the inside of an asteroid belt as a wide cinematic field of drifting rocks, seen from just above the belt's plane and looking along it. This is not a planet, a moon or a single asteroid.

Composition invariants: Landscape image of exactly 1672 x 1181 pixels. The belt is a broad band of rocks that sweeps across the whole width on a gentle curve, the arc of an orbit around a star outside the frame, rising slightly from left to right. Depth in three layers: a dense haze of tiny distant rocks and dust along the band's far edge, at about 30-40% of the image height; medium rocks through the middle; and in the lower half several large near rocks of varied size, two or three of them cropped by the bottom and side edges. Among the near and middle rocks keep several broad, gently sloping upper faces, clear and evenly lit, where separate game structures can later be placed; do not bake in any fixed number of locations. The top fifth of the image (about rows 0-240) is open deep space: near-black, with sparse small white stars and at most a few distant specks of rock. Upper-left raking sunlight from a star just outside the frame: a soft warm glare in the top-left corner and no visible sun disc. Rocks are lit on their upper-left faces with dark, crisp shadow sides; the belt's dust catches the light as a soft, low haze along the band.

Style: realistic, detailed, cinematic game-environment painting matching Image 1; readable large forms, restrained fine detail, and good value separation so that building and ship sprites placed on top stay legible.

Output constraints: A single full-frame image of only this asteroid field. No planets, moons, gas giants, comets, nebulae or galaxies; no structures, stations, ships, satellites, mining equipment, beams, lights, ruins or artificial debris; no collage, split view, border, UI, text, labels, numbers, logos or watermark.

Belt type:
<BELT TYPE>
```

| File | Belt type |
|---|---|
| `stony-belt-v1.png` | A stony belt: grey-brown porous rock with pitted surfaces and a few pale chips, the same material as Image 2. The dust haze along the band is a warm grey-brown. |
| `metallic-belt-v1.png` | A metallic belt: dark steel-grey metallic rock with bright silver fracture planes, small metallic glints and a few rust-brown patches, heavier and denser-looking than stony rock, the same material as Image 2. Sunlight catches small metallic glints across the field; the dust haze has a cool steel-blue sheen. |
| `icy-belt-v1.png` | An icy belt: blue-white chunks of frosted pale blue ice and rock with translucent edges, embedded dark grit and fine fractures, the same material as Image 2. Fine ice dust glitters in the sunlight as a pale blue haze along the band; the ice itself does not glow. |
| `shattered-belt-v1.png` | The shattered crust of a broken planet: rust-red and charcoal rock with visible layered strata on sharp broken faces and small metallic veins, the same material as Image 2. Larger, flatter slab-like fragments than an ordinary belt, jagged violent fractures, and streaks of fine red-orange debris along the band; no lava, fire or glow. |

## In the game

- **Surface:** `planetSurface()` returns `<kind>-belt` for a belt
  (`BELT_SURFACES` in `planet-surfaces.ts`), so the view takes the painted path
  (`.deck-planet__scene.is-belt`, labelled "Asteroid field") instead of the
  orbital globe. Gas giants keep the orbital scene.
- **Art:** `build-planet-workbench-art.mjs` derives each master to
  `public/art/derived/planet-workbench/belt-v1/<kind>-belt-{768,1280,1672}.webp`.
- **Sites:** a belt holds at most a Mining Complex per deposit (four) and its
  power plant, so each painting names six sites by hand on the tops of its
  largest rocks, nearest first (`PAINTINGS` in `build-planet-sites.mjs`; the
  generated table checks they stay inside the frame and apart). A belt fills
  its sites in order rather than by catalogue position, and keeps its saved
  layout under its own key (`planet-sites.ts`).
- **Buildings:** no ground plates on a belt; the Power Plant keeps its floating
  look (`planet-structure-art.ts`). Buildings stand a third larger than on a
  planet, since a belt has only a few on a wide field (`planet-panel.css`).
- **Check:** `node client/scripts/build-planet-sites.mjs --review <dir>` draws
  every painting's sites; the `planet-fixture-belt` launch entry (port 4182)
  shows a metallic belt with its Mining Complex and a free site.
