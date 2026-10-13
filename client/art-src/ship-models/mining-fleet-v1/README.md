# Mining fleet reconstruction pipeline

This batch reconstructs the Mining Barge and Mining Belt Station from the five-view sheets requested in `docs/asteroid-belt-art-prompts.md`. It follows the existing Construction Ship / Authority Nexus Python workflow: explicit closed geometry, native reference projections, PBR masks, merged static parts, embedded GLB textures, and OBJ/MTL export. Blender is not used.

| Asset | GLB | Triangles | Render sections | Source components |
| --- | --- | ---: | ---: | ---: |
| Mining Barge | [mining-barge.glb](../mining-barge-textured-v1/mining-barge.glb) | 7,816 | 2 | 112 |
| Mining Belt Station | [mining-belt-station.glb](../mining-belt-station-textured-v1/mining-belt-station.glb) | 14,536 | 2 | 182 |

Both models use Y up and −Z forward. Coordinates are relative model units, not meters or a shared gameplay scale. Apply the intended barge/station size during integration. The model origin corresponds to the reference-sheet hub/mid-hull origin, not the bounding-box center. There are no animations or gameplay effects in these files.

## Rebuild and inspect

Run from the repository root with Python 3, NumPy and Pillow available:

```sh
python3 client/art-src/ship-models/mining-barge-textured-v1/build_model.py
python3 client/art-src/ship-models/mining-belt-station-textured-v1/build_model.py
python3 client/art-src/ship-models/mining-fleet-v1/inspect_models.py
node client/art-src/ship-models/mining-fleet-v1/validate.mjs ../mining-barge-textured-v1/mining-barge.glb
node client/art-src/ship-models/mining-fleet-v1/validate.mjs ../mining-belt-station-textured-v1/mining-belt-station.glb
```

The validator uses the existing `gltf-validator` installation in the fleet tools. `pipeline_geometry.py` and `render_core.py` reuse the Authority Nexus triangulation and independent GLB inspection renderer; the renderer now includes trilinear mip filtering for these previews.

Each model folder contains the GLB, OBJ/MTL, model specification, metadata, texture PNGs, concept sprite, seven rendered views, a contact sheet, and validation reports. The GLB is self-contained; OBJ requires its MTL and textures directory. Rebuilding requires this shared folder and the two reference folders.

## Texture coverage

Each physical triangle has base-color and metallic/roughness textures, and a nonzero-area UV mapping. Downward armor surfaces use the dedicated BOTTOM drawing. Cylinder walls have angular UVs; caps and narrow edges use planar UVs. All static pieces are consolidated into two materials/render sections, one of which identifies the dedicated ventral surfaces.

The primary atlas preserves the 1774 × 887 five-view source pixels. `material-strip.png` is a supplementary 2172 × 724 ImageGen texture based on both reference sheets. Its silver, graphite and ochre swatches cover walls and fittings that would otherwise enlarge tiny elevation crops. The strip is reduced to 2048 pixels wide during atlas packing; the full generated original is retained. The final three PBR maps are 2048 × 2048.

The models are coherent concept reconstructions, with standardized orthographic proportions where the generated views disagree. The barge is approximately 0.70 width/length; the station is approximately 0.97. Silhouette and component counts follow the supplied design descriptions.

## Attachment points

Named empty glTF nodes hold model-local translations. Their `extras.direction` is a normalized direction in the same Y-up coordinate frame; transform it by the model's world rotation for effects or approach paths. These nodes carry no animation or automatic docking behavior.

| Model | Nodes | Purpose |
| --- | --- | --- |
| Barge | `socket.mining.port`, `socket.mining.starboard` | Two cutter tips, directions −Z |
| Barge | `socket.engine.port`, `socket.engine.starboard` | Aft exhaust attachment points, directions +Z |
| Station | `socket.docking.barge.port`, `socket.docking.barge.starboard` | Centers of open cradle windows; directions −X / +X |
| Station | `socket.docking.hauler` | End of forward loading gantry; direction −Z |
| Station | `socket.anchor.1` through `.4` | Four anchoring-boom ends; directions −Y |

The cradle windows are attachment clearances, not whole-ship bounding boxes. The future docking system should position the barge outside the hub and align its loading contact with the berth.

## Verification

Both final GLBs pass Khronos validation with zero errors and zero warnings. The independent inspector checks finite attributes, unit normals, triangle winding, nonzero geometry and UV areas, closed positive-volume source components, texture coverage, feature counts, socket directions, triangle budgets, and real openings at both berth centers. It reads the exported GLB buffers rather than the builder's in-memory geometry.

Previews render the exported meshes and embedded textures in software. They inspect surfaces and silhouette; they are not a browser/WebGL compatibility or performance test. Runtime integration is left to the game's art wiring as requested.
