# Asteroid belt mining art — delivery

All 16 requested images are saved at the exact paths in the supplied prompt document. The 14 icons/chunks/hull sprites have real RGBA transparency; the two five-view sheets have white backgrounds. The existing Stony belt chunk remains the matching Stony asset.

The hull sheets have also been reconstructed into textured GLB and OBJ models with the existing Python/NumPy workflow, without Blender. Assets are ready for the game wiring described in the request.

## Images

| Asset | File | Native size | Format |
| --- | --- | --- | --- |
| Olivine ore | [olivine_ore.png](../ores-2026-09-16/olivine_ore.png) | 1254 × 1254 | RGBA PNG |
| Bornite ore | [bornite_ore.png](../ores-2026-09-16/bornite_ore.png) | 1254 × 1254 | RGBA PNG |
| Ilmenite ore | [ilmenite_ore.png](../ores-2026-09-16/ilmenite_ore.png) | 1254 × 1254 | RGBA PNG |
| Iridium ore | [iridium_ore.png](../ores-2026-09-16/iridium_ore.png) | 1254 × 1254 | RGBA PNG |
| Helium-3 ice | [helium3_ice.png](../ores-2026-09-16/helium3_ice.png) | 1254 × 1254 | RGBA PNG |
| Void crystal | [void_crystal.png](../ores-2026-09-16/void_crystal.png) | 1254 × 1254 | RGBA PNG |
| Refined iridium | [iridium.png](../ores-2026-09-16/iridium.png) | 1254 × 1254 | RGBA PNG |
| Refined helium-3 | [helium3.png](../ores-2026-09-16/helium3.png) | 1254 × 1254 | RGBA PNG |
| Exotic matter | [exotic_matter.png](../ores-2026-09-16/exotic_matter.png) | 1254 × 1254 | RGBA PNG |
| Metallic belt chunk | [metallic.png](../belt-chunks-v1/metallic.png) | 1254 × 1254 | RGBA PNG |
| Icy belt chunk | [icy.png](../belt-chunks-v1/icy.png) | 1254 × 1254 | RGBA PNG |
| Shattered belt chunk | [shattered.png](../belt-chunks-v1/shattered.png) | 1254 × 1254 | RGBA PNG |
| Mining Barge five-view sheet | [mining-barge-reference.png](../mining-barge-reference-v1/mining-barge-reference.png) | 1774 × 887 | RGB PNG |
| Mining Belt Station sprite | [mining-belt-station.png](../mining-belt-station-reference-v1/mining-belt-station.png) | 1254 × 1254 | RGBA PNG |
| Mining Belt Station five-view sheet | [mining-belt-station-reference.png](../mining-belt-station-reference-v1/mining-belt-station-reference.png) | 1774 × 887 | RGB PNG |
| Mining Barge sprite | [mining-barge.png](../mining-barge-reference-v1/mining-barge.png) | 1254 × 1254 | RGBA PNG |

The hull sprites include transparent edge clearance. Generated sheet views are used as concept references; the reconstruction standardizes proportions into coherent geometry. Master images are retained at their native generated resolution.

## 3D models

| Model | Triangles | Draw sections | Files |
| --- | ---: | ---: | --- |
| Mining Barge | 7,816 | 2 | [GLB](../ship-models/mining-barge-textured-v1/mining-barge.glb) · [Preview and model notes](../ship-models/mining-barge-textured-v1/README.md) |
| Mining Belt Station | 14,536 | 2 | [GLB](../ship-models/mining-belt-station-textured-v1/mining-belt-station.glb) · [Preview and model notes](../ship-models/mining-belt-station-textured-v1/README.md) |

Both assets have embedded PBR maps, dedicated underside textures and named effect/docking attachment points. Y up, −Z forward; model units are relative and gameplay scale remains an integration choice. The station has six silos, two real open docking cradles, one loading gantry and four anchoring booms. The barge has two cutter tips, one hopper, a port bridge and two engines.

Khronos glTF validation: zero errors and zero warnings for both models. The independent exported-buffer audit verifies texture coverage, noncollapsed UVs, closed source components, normals, feature counts, socket directions and berth openings. [Rebuild and validation instructions](../ship-models/mining-fleet-v1/README.md).

## Prompts and provenance

- [Original prompts, required reference order and destinations](generation-plan.json)
- [Design/projection corrections](generation-corrections.json)
- [Final framing correction and supplemental material texture prompt](additional-iterations.json)
- [Selected generated source files, hashes, dimensions and alpha measurements](generation-results.json)
- [Final image verification](asset-verification.json)

All raster artwork was generated with the built-in `image_gen` tool. One image was generated per requested prompt; correction passes replaced drafts with count or projection errors. A supplementary [flat material strip](../ship-models/mining-fleet-v1/material-strip.png), based on both hull sheets, provides detailed armor/metal texture for reconstructed walls and narrow edges. Its [prompt](../ship-models/mining-fleet-v1/material-strip-prompt.txt) is retained. The 3D texture-packing step produces 2048 × 2048 base-color, metallic/roughness and emissive atlases.
