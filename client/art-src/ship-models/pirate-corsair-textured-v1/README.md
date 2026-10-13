# Pirate Corsair

Textured low-polygon reconstruction of the included five-view pirate reference.

- **2,928 triangles**, 5 consolidated render sections.
- GLB embeds 2048 × 2048 base-color, metallic/roughness and emissive PNG textures.
- Separate top and underside UV projections preserve the reference's ventral details.
- Recessed engine mouths, solid hulls and weapons are real geometry. The Boarding Raider's cages have open frames.
- Y up, forward along −Z; approximately 20 authoring units long. Physical game sizes are not assigned.
- Named component geometry is retained in the OBJ and deterministic Python source; GLB static parts are combined by material.
- Engines and muzzle/capture sockets are exported where applicable. This is a static model with no animation rig.

## Files

`pirate-corsair.glb` is the self-contained runtime asset. `pirate-corsair.obj` / `.mtl` and `textures/` are editable interchange files. `previews/` contains eight software-rendered views read from the actual GLB. `textures/reference-sheet.png` is the unchanged generated reference; `source/` retains the original game sprite and exact image prompts.

## Rebuild

```sh
python3 -m pip install -r requirements.txt
python3 prepare_textures.py
python3 build_model.py
```

No Blender or external modeling application is required. `designs.py` traces the hull in reference pixel coordinates. `model-spec.json` registers each reference view; `mesh_core.py` triangulates, projects UVs and exports; `geometry.py` supplies closed geometry helpers.

## Validation

Khronos glTF Validator: **0 errors, 0 warnings**. The independent geometry check confirms finite attributes, valid indices, unit normals, consistent winding, no degenerate triangles, noncollapsed bottom UVs and positive-volume closed surfaces for all 39 authored components. Separate solid components can intersect; this is not a single watertight 3D-printing surface.

The supplied previews use an independent software rasterizer. They are not a WebGL/PBR acceptance test. See the fleet folder for the interactive offline viewer.

Reference images were generated with the built-in image generation tool. Material-channel textures are derived from those images; small structural metal parts use matching solid materials. Some perspective and lighting information is baked into the reference artwork.
