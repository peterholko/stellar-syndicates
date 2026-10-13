# Stand-in replacement art — 9 October 2026

Completed from `docs/stand-in-art-prompts.md`: **13 primary PNG assets and one textured Survey Probe model**. Images were generated with the built-in ImageGen tool. Exact prompts, references and assembly records are in [prompts.json](prompts.json), with individual `.prompt.json` files beside each PNG. [manifest.json](manifest.json) lists paths, dimensions, hashes and verification results.

## Survey Probe

- [Transparent top-down sprite](../survey-probe-reference-v1/survey-probe.png) — 1193 × 1193 RGBA, native art with transparent canvas margins cropped to 89% ship height; no upscale.
- [Five-view reference](../survey-probe-reference-v1/survey-probe-reference.png) — 1774 × 887.
- [Textured GLB](../ship-models/survey-probe-textured-v1/survey-probe.glb) — 2,448 triangles, two render sections, one engine socket, zero glTF errors or warnings.
- [Model previews](../ship-models/survey-probe-textured-v1/previews/contact-sheet.png) and [rebuild/validation notes](../ship-models/survey-probe-textured-v1/README.md).

## Planet and surface art

[Marginal-ocean planet sprite](../planets-2026-10-08/marginal-ocean-v1.png): 1254 × 1254 RGBA, approximately 91% solid diameter, genuine transparent padding.

| World | 1672 × 941 painting | 1672 × 1181 orbit version | Clean land sites |
| --- | --- | --- | ---: |
| Marginal ocean | [Painting](../planet-view/surfaces/marginal-ocean-surface-v1.png) | [Orbit](../planet-view/surfaces/marginal-ocean-surface-v1-orbit.png) | 17 |
| Boreal | [Painting](../planet-view/habitable-v1/boreal-world-v2.png) | [Orbit](../planet-view/habitable-v1/boreal-world-v2-orbit.png) | 24 |
| Volcanic Highlands | [Painting](../planet-view/habitable-v1/volcanic-highlands-v2.png) | [Orbit](../planet-view/habitable-v1/volcanic-highlands-v2-orbit.png) | 24 |
| Archipelago | [Painting](../planet-view/habitable-v1/archipelago-world-v2.png) | [Orbit](../planet-view/habitable-v1/archipelago-world-v2-orbit.png) | 17 |
| Hostile ocean | [Painting](../planet-view/surfaces/ocean-surface-v1-v2.png) | [Orbit](../planet-view/surfaces/ocean-surface-v1-v2-orbit.png) | 19 |

Every orbit image adds 240 rows above its painting. A decoded-pixel comparison confirms **zero changed pixels in the lower 941 rows**. The generated sky alone is fitted and seam-matched; the source painting is copied unresampled. The original pre-v2 masters remain in place.

The [placement report](placement-verification.json) and [review overlays](placement-review/) use the game's existing terrain thresholds, footprint sizes and spacing. The two ocean reviews additionally restrict candidates to the central island band; this excludes sunlit water and ice floes that the hue classifier can mistake for land. Counts describe usable artwork, not installed game placements. The normal overlay contains all 24 requested addresses; red rectangles identify rough fallback addresses and are not included in the clean counts.

## Handoff

Art and the Probe model are ready for the separate integration step. That step should update the hull catalog and model scale, select the new surface sources, and regenerate/review placement addresses with the ocean shoreline exclusions. The Mining Barge already has its finished art in `ship-models/mining-barge-textured-v1/`.

`tools/save-asset.mjs` records technical canvas assembly and pixel-preservation checks. `tools/check-placement.mjs` runs the existing placement algorithm on these versioned images and writes only this review folder; it does not update game tables.
