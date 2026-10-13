# Nine new fleet models

All nine requested ships have five-view reference sheets, textured GLB and OBJ/MTL models, standalone previews, three mesh detail levels and forty-direction sprite atlases. The low-triangle viewer now contains all eighteen ship types.

Open `index.html` for the gallery, or `../interceptor-lod-experiment-v1/viewer.html?formation=all` for the complete size comparison. The gallery provides per-ship reference, GLB and editable-package links.

| Ship | Full / LOD 0 | LOD 1 | LOD 2 | Draw calls |
|---|---:|---:|---:|---:|
| Destroyer | 14,532 | 7,800 | 4,780 | 15 |
| Battleship | 33,772 | 17,104 | 11,270 | 35 |
| Dreadnought | 51,176 | 26,264 | 16,036 | 75 |
| Scout | 8,896 | 4,908 | 3,330 | 7 |
| Colony Ship | 27,896 | 12,182 | 6,186 | 7 |
| Troop Transport | 11,072 | 7,832 | 6,378 | 7 |
| Construction Ship | 10,340 | 6,370 | 4,688 | 7 |
| Medium Freighter | 17,056 | 11,142 | 8,234 | 7 |
| Authority Freighter | 20,144 | 14,012 | 11,368 | 7 |

Every distant sprite is two triangles and one draw call. Static surfaces are combined by material. Destroyer, Battleship and Dreadnought retain 2, 7 and 17 independent turret mounts; turret materials still require separate draws. These counters describe this uninstanced prototype, not a battle performance measurement.

The models use the reference's pale titanium, dark structures, brass details and cyan apertures. All undersides have dedicated texture projections. There are three embedded 2048-pixel PBR maps per model. Five-view concept sheets were generated using the built-in image-generation tool; the 3D reconstruction uses deterministic Python/NumPy geometry, without Blender.

## Reference images and exact prompts

- [Destroyer reference](../destroyer-textured-v1/textures/reference-sheet.png) · [GLB](../destroyer-textured-v1/destroyer.glb) · [prompts](../destroyer-textured-v1/prompts/prompt.txt)
- [Battleship reference](../battleship-textured-v1/textures/reference-sheet.png) · [GLB](../battleship-textured-v1/battleship.glb) · [prompts](../battleship-textured-v1/prompts/prompt.txt)
- [Dreadnought reference](../dreadnought-textured-v1/textures/reference-sheet.png) · [GLB](../dreadnought-textured-v1/dreadnought.glb) · [prompts](../dreadnought-textured-v1/prompts/prompt.txt)
- [Scout reference](../scout-textured-v1/textures/reference-sheet.png) · [GLB](../scout-textured-v1/scout.glb) · [prompts](../scout-textured-v1/prompts/prompt.txt)
- [Colony Ship reference](../colony-ship-textured-v1/textures/reference-sheet.png) · [GLB](../colony-ship-textured-v1/colony-ship.glb) · [prompts](../colony-ship-textured-v1/prompts/prompt.txt)
- [Troop Transport reference](../troop-transport-textured-v1/textures/reference-sheet.png) · [GLB](../troop-transport-textured-v1/troop-transport.glb) · [prompts](../troop-transport-textured-v1/prompts/prompt.txt)
- [Construction Ship reference](../construction-ship-textured-v1/textures/reference-sheet.png) · [GLB](../construction-ship-textured-v1/construction-ship.glb) · [prompts](../construction-ship-textured-v1/prompts/prompt.txt)
- [Medium Freighter reference](../medium-freighter-textured-v1/textures/reference-sheet.png) · [GLB](../medium-freighter-textured-v1/medium-freighter.glb) · [prompts](../medium-freighter-textured-v1/prompts/prompt.txt)
- [Authority Freighter reference](../authority-freighter-textured-v1/textures/reference-sheet.png) · [GLB](../authority-freighter-textured-v1/authority-freighter.glb) · [prompts](../authority-freighter-textured-v1/prompts/prompt.txt)

## Checks

- Nine source GLBs and 54 optimized GLBs pass glTF validation with zero errors/warnings.
- All new mesh levels retain original texture bytes and primary protected geometry; LOD 0 retains every original triangle, normal, UV and material.
- Surface checks cover bottom projections and collapsed geometry/UVs. Browser captures include all six sides and both quarters.
- Forty nonempty sprite tiles per ship; 720 across the eighteen-ship viewer.
- 119 lineup checks pass, including both scales, single-ship and formation regression checks, independent LODs and turret movement.
- 55 previous model/sprite assets are verified byte-identical.

`progress.json`, `prior-assets-verification.json`, source/package reports and the experiment manifest record the delivered state. The game code is outside this asset experiment.

## Reproduction

Each ship folder is independently rebuildable; see its README. `scaffold.py` and `build_all.py` reproduce the nine source packages, while `build_lods.py` produces their optimized variants. `integrate_viewer.py` records the initial one-time lab migration and is not an idempotent rebuild step. Use the experiment's documented commands for subsequent builds.
