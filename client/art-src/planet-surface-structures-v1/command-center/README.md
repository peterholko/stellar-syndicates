# Corporate Command Center — production sprites v3

Concepts generated using the built-in image-generation tool on 2026-09-26 under the imagegen skill. This is raster planet-surface art, not 3D model output. The initial concept stage made no gameplay changes; the production integration below adds this three-level structure family without replacing other structure art.

Edited on 2026-09-27 using the same built-in tool to remove Terran Charter Authority branding. These command centers belong to the player's corporation. The current v2 drafts retain blank graphite identity panels; the headquarters' ornamental gold roof crest was also removed. Functional lighting, communications equipment and the building designs are retained. Original v1 files remain alongside them for reference.

## Three proposed stages

1. [Landing Command](landing-command-unbranded-v2-draft.png): converted landing cabin, short entry ramp, small communications dish and mast, blank corporate identity panel.
2. [Colony Command Center](colony-command-center-unbranded-v2-draft.png): permanent attached administrative wings and a raised operations room.
3. [Syndicate Headquarters](syndicate-headquarters-unbranded-v2-draft.png): terraced civic complex, distinctive taller command tower and blank future corporate nameplate.

The shared family uses cream steel, graphite framing, amber entrance trim, cyan command glazing and compact foundation pads, with no Authority insignia. Materials and rendering follow the warehouse art experiment.

## Production sprites

The approved unbranded v2 designs were cut out with the built-in image-generation tool on 2026-09-27. The shipping masters are `tier-1.png`, `tier-2.png`, and `tier-3.png`: 1254 × 1254 RGBA PNGs with actual transparent pixels, blank corporate panels and no Authority insignia. Only background extraction was requested; the building designs are retained.

`client/scripts/build-command-center-art.mjs` creates the 128/256/512px WebP derivatives under `client/public/art/derived/command-center/v1/`. It trims empty padding, adds a consistent safe margin and preserves generated alpha. The manifest records source hashes and output sizes. No other structure's icon set is replaced. These sprites are used in the planet scene, desktop/mobile construction panels and settlement kit.

Exact background-extraction instructions and output provenance are in [edits-v3.json](edits-v3.json). The built-in tool's model identifier was not exposed.

## Retained concept drafts (not shipping assets)

All three current v2 files are 1254 × 1254 RGB PNGs, three channels, fully opaque, with no alpha channel. They preserve the v1 background, including its painted checkerboard. Although genuinely transparent 1024 × 1024 cutouts were originally requested for v1, those outputs were not transparent. The v2 edit changes branding only. These must **not** be integrated as transparent sprites.

Those draft files are retained as references only. Their replacement cutouts and small-scale browser integration are verified; no local background-removal fallback was used.

## Reproduction

The exact v2 edit prompts, input and output paths, and validation results are preserved in [edits-v2.json](edits-v2.json). The original v1 generation prompts remain unchanged in [prompts.json](prompts.json). Both versions used the built-in image-generation tool; its model identifier was not exposed.
