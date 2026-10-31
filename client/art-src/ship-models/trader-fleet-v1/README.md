# Independent trader models

Three textured 3D reconstructions of the approved trader reference sheets. Texture revision 2 repairs the mapping faults found in the full audit.

| Ship | Owner | Triangles | Model | Interactive preview |
| --- | --- | ---: | --- | --- |
| Clean Slate | Ines Varga | 6,250 | [GLB](../clean-slate-textured-v1/clean-slate.glb) | [Orbit](../clean-slate-textured-v1/viewer.html) |
| Second Wind | Mags Halloran | 7,778 | [GLB](../second-wind-textured-v1/second-wind.glb) | [Orbit](../second-wind-textured-v1/viewer.html) |
| Small Fortune | Rafi Mensah | 7,764 | [GLB](../small-fortune-textured-v1/small-fortune.glb) | [Orbit](../small-fortune-textured-v1/viewer.html) |

[Read the texture audit and compare before/after](texture-audit-v1/report.html) · [Open the gallery](index.html) · [Download all three GLBs](trader-ships-glb.zip) · [Rendered comparison](trader-fleet-models.png)

Each GLB has two material sections, three embedded 2048px PBR maps, dedicated underside textures, and named attachment nodes. The models use Y up and −Z forward, in relative model units. All are static. TOP governs the silhouette where the generated concept elevations disagree; the elevations supply volume and surface detail. Game scale, LODs, animation and runtime wiring are separate production stages.

The model folders include GLB, OBJ/MTL, texture maps, model specifications, source hashes, seven software renders, live WebGL screenshots, validation reports and a self-contained browser viewer. `manifest.json` records final file hashes. The portable ZIP contains the three GLBs and their metadata.

## Rebuild

From the repository root, with the project's existing Python NumPy/Pillow and Node dependencies:

```sh
python3 client/art-src/ship-models/trader-fleet-v1/build_models.py
python3 client/art-src/ship-models/trader-fleet-v1/inspect_models.py
node client/art-src/ship-models/mining-fleet-v1/validate.mjs ../clean-slate-textured-v1/clean-slate.glb
node client/art-src/ship-models/mining-fleet-v1/validate.mjs ../second-wind-textured-v1/second-wind.glb
node client/art-src/ship-models/mining-fleet-v1/validate.mjs ../small-fortune-textured-v1/small-fortune.glb
node client/art-src/ship-models/trader-fleet-v1/test_assets.mjs
node client/art-src/ship-models/trader-fleet-v1/build_previews.mjs
python3 client/art-src/ship-models/trader-fleet-v1/audit_textures.py --phase after
python3 client/art-src/ship-models/trader-fleet-v1/write_texture_report.py
python3 client/art-src/ship-models/trader-fleet-v1/package_models.py
```

The builder and inspector accept individual ship IDs. They reuse `mining-fleet-v1/reconstruction.py`, `pipeline_geometry.py`, `render_core.py` and the existing metal material strip. The preview bundler reuses the Free Trader viewer source and existing Pirate Fleet esbuild installation. No external art API or Blender is needed to rebuild.

## Inspect interactively

The individual `viewer.html` files embed their GLB and renderer and can be opened directly in a browser. For the linked gallery:

```sh
python3 -m http.server 8789 --bind 127.0.0.1 --directory client/art-src/ship-models
```

Open `http://127.0.0.1:8789/trader-fleet-v1/index.html`. The viewers include orbit controls, orthographic views, wireframe, unlit texture inspection, PNG capture and GLB download.

## Verification

All three exported assets pass Khronos glTF validation with zero errors and zero warnings. The independent buffer inspector checks closed positive-volume components, finite attributes, winding, nondegenerate geometry, UV areas, dedicated underside coverage, feature counts, socket directions and actual open spaces in the cargo cradle and runner engine-boom gaps. The installed Three.js r180 loader verifies materials, embedded texture headers, dimensions and socket transforms. Actual pixels are checked through software renders and live WebGL inspection.

## Texture audit revision 2

The original GLBs, specifications, builders and nine-view inspection sheets are preserved in `texture-audit-v1/before/`. The new UV workflow rejects collapsed mappings, aligns docking and underside projections, subdivides long walls at registration breakpoints, uses physical cylindrical/radial mappings for fittings, folds paint across thin panel edges, and uses sloped canopy geometry. Dedicated material strips replace undersized paint samples. Their saved source is `material-trims-v2.png`; `material-trims-v2.prompt.json` records the exact built-in ImageGen prompt.

`texture-audit-v1/report.html` contains per-ship defects, changes, remaining visual seams and interactive before/after comparisons. The JSON measurements include per-triangle distortion and density, barycentric checks of side docking alignment, shared-edge color-step candidates and GLB hashes. Valid GLB structure is not treated as proof of visual alignment. Some native view-projection seams remain; this is documented rather than described as a seamless production unwrap.

## Second Wind command module follow-up

The first sloped canopy still placed its roof ridge through the painted windscreen. Its replacement uses separately traced roof and sill rings, so the complete windscreen lies on one sloping plane and the upper pane lies on the roof. The console and antenna mounts are seated on that cabin; its short lower skirt continues the adjacent artwork. [Close-up comparison](command-module-repair/report.html) preserves the immediately preceding GLB and four inspection angles.

`inspect_command_module.py` samples the actual exported surfaces under window landmarks. It checks windscreen planarity, roof height consistency, source-pixel registration, and byte-identical non-bridge geometry/normals/UVs. Run `python3 client/art-src/ship-models/trader-fleet-v1/inspect_command_module.py after` to render and measure the cabin, or add `--measure-only` to skip rendering.
