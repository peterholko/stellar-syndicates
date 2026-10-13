# Pirate fleet — Stellar Syndicates

Three five-view reference sheets and three textured low-polygon 3D models, created without Blender. The references preserve the original pirate silhouettes, worn metal, patchwork armor and amber engines.

Open **viewer.html** in a browser to orbit each model, inspect the five orthographic directions, show its wireframe, compare its reference sheet and download the GLB. All models, PNGs and the Three.js runtime are embedded, so it needs no server or network connection.

| Model | Triangles | Render sections | GLB |
|---|---:|---:|---|
| Privateer Raider | 3,476 | 5 | [privateer-raider.glb](../privateer-raider-textured-v1/privateer-raider.glb) |
| Pirate Corsair | 2,928 | 5 | [pirate-corsair.glb](../pirate-corsair-textured-v1/pirate-corsair.glb) |
| Pirate Boarding Raider | 4,092 | 6 | [pirate-boarding-raider.glb](../pirate-boarding-raider-textured-v1/pirate-boarding-raider.glb) |

Each model package includes GLB, OBJ/MTL, material textures, the unchanged reference, editable Python build files, eight rendered views and validation reports. `pirate-fleet-models.png` compares their upper surfaces, engine views and undersides.

The five reference directions are **front, back, top, bottom and side**. The image prompts and subsequent view-correction prompts are preserved verbatim in `prompts.json`. References were generated with the built-in image generation tool and conditioned on the original game sprites. Models were authored from those references with Python/NumPy. No Blender was used.

## Validation

All three final GLBs pass Khronos glTF Validator with **0 errors and 0 warnings**. Independent buffer-level checks verify topology, indices, normals and UVs, including the dedicated underside projections. All 161 authored components are individually closed solids with positive volume. Eight views of each exported model were rendered in software; the contact sheet shows three of them.

The interactive viewer was bundled and its embedded files were checked. Browser interaction could not be tested because browser access remains disabled by the saved permission. The software previews are separate model renders, not screenshots of the viewer.

## Scope and scale

These are standalone pirate assets. They have not been connected to the galaxy-map or battle model catalog. Their authoring length is approximately 20 units each; physical game sizes still need assigning during integration. Static geometry is combined by material, and the OBJ retains named construction pieces. No animated rig is included.

## Rebuilding

To rebuild one ship, run `prepare_textures.py` then `build_model.py` from its folder after installing its `requirements.txt`. To rebuild this gallery, install the pinned tools with `npm ci --ignore-scripts`, then run `node build_viewer.mjs` from the fleet pipeline folder. The shared scaffold is optional and uses the bundled exporter templates. `python3 render_models.py` repeats the independent mesh checks and model renders; `node validate.mjs` repeats glTF validation.

Source-component counts, UV registration and hashes are in the JSON manifests. The reference images are unchanged; texture padding and derived material channels live separately in each model's `textures/` directory.
