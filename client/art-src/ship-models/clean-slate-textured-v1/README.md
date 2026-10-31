# Clean Slate — Ines Varga

Ivory and teal precision freighter: six raised sealed cargo lids, two side docking collars, a low glazed bridge, integrated rectangular drives, and a ventral freight elevator.

[Open the interactive model](viewer.html) · [GLB](clean-slate.glb) · [Rendered views](previews/contact-sheet.png) · [Approved reference](textures/reference-sheet.png)

## Asset

- **6,250 triangles**, 104 closed source components, two merged material sections.
- Self-contained GLB with three embedded 2048 × 2048 maps: base color, metallic/roughness, and restrained cyan emission.
- OBJ/MTL interchange copy and external texture PNGs. OBJ preserves named mesh parts; attachment nodes and complete PBR materials are in GLB.
- Y up, −Z forward. Dimensions: 11.9137 × 2.91 × 19.7368 relative units.
- 4 attachment nodes. Directions live in each node's `extras.direction`, in model axes.
- Static model. The crane, doors and engines have no animation or gameplay effects.

## Reconstruction

The approved TOP view defines the footprint and component stations. The other views guide volume and underside plating; proportions are reconciled where the concept views disagree. Major features are explicit geometry; smaller panel lines and service detail come from native reference pixels. The source sheet is retained unchanged. Supporting paint/service trims were generated with the built-in image_gen tool; the saved sheet and exact prompt are in the shared fleet folder. Hidden metal fittings reuse the existing fleet material strip.

Texture revision 2 repairs undersized material crops, collapsed UV substitutions, thin panel edges, cap/ring scaling, canopy geometry, and side/underside registration. Hull faces are split at piecewise registration landmarks so interpolation cannot displace features inside long walls. [Detailed texture audit and before/after views](../trader-fleet-v1/texture-audit-v1/report.html) documents both fixes and remaining concept-projection seams. The original models are preserved in that audit folder.


The model follows the project's Python/NumPy reconstruction pipeline. The shared implementation and inspection tools are in [trader-fleet-v1](../trader-fleet-v1/README.md). No Blender installation is required. Runtime scale, LOD selection, sprite baking and catalog integration are subsequent production steps.

## Verification

`gltf-validation.json` records zero errors and zero warnings. `surface-verification.json` independently checks the exported buffers for finite data, valid winding, closed positive-volume components, nondegenerate triangles, noncollapsed UVs, underside coverage, required features and attachment directions. `three-validation.json` checks loading and socket transforms with the game's Three.js r180. Software renders in `previews/` use the GLB's actual meshes and embedded textures; `webgl-preview.png` records the live browser view. UV distortion/density and shared-edge color measurements are recorded separately in the texture audit; file validity alone does not establish visual alignment.

Rebuild from the repository root:

```sh
python3 client/art-src/ship-models/clean-slate-textured-v1/build_model.py
python3 client/art-src/ship-models/trader-fleet-v1/inspect_models.py clean-slate
node client/art-src/ship-models/mining-fleet-v1/validate.mjs ../clean-slate-textured-v1/clean-slate.glb
node client/art-src/ship-models/trader-fleet-v1/test_assets.mjs
node client/art-src/ship-models/trader-fleet-v1/build_previews.mjs
```
