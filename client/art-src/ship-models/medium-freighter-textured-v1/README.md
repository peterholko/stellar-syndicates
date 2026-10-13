# Medium Freighter — textured 3D model

Reconstructed from the accompanying five-view reference sheet with Python and NumPy. No Blender.

- Full model: `medium-freighter.glb` with three embedded 2048 × 2048 PBR texture maps.
- Editable interchange: `medium-freighter.obj` and `medium-freighter.mtl`, with maps in `textures/`.
- Reference: `textures/reference-sheet.png` — front, back, top, bottom and side.
- Interactive preview: open `viewer.html` in a browser. All renderer code and model bytes are embedded.
- Authored components: 160; triangles: 17,056; render sections: 7.
- Engines: 4; separate turret assemblies: 0.
- Axes: Y up; −Z forward. Working units are independent from physical/game sizes.

Textures preserve native reference pixels with separate dorsal, ventral, side and end projections. White background margins are dilated to prevent pale seams. Occluded rear armor uses a visible reference panel; recessed exhaust faces have local rear-view mapping. Base color, metallic/roughness and cyan emission maps are embedded. This is an editable reconstruction of the concept, rather than a photogrammetry scan.

`build_model.py`, `designs.py`, `geometry.py`, `mesh_core.py` and `model-spec.json` contain the deterministic source. Plan contours use native reference pixel coordinates. Static pieces consolidate by material. Named turret groups and attachment sockets survive; the experiment's LOD exporter adds articulated yaw pivots.

Rebuild with Python, NumPy and Pillow:

```sh
python3 prepare_textures.py
python3 build_model.py
```

Preview rebuild and glTF validation use the pinned dependencies in `package.json`:

```sh
npm install
node build_preview.mjs
node validate.mjs
```

`validation.json` records zero glTF errors/warnings. `surface-verification.json` checks nonzero geometry and textured UV areas, underside UV placement, texture count and sockets. `previews/` contains eight actual model renders, including underside and all five orthographic views.

The reference image was created with the built-in image-generation tool from the project's established ship artwork. Exact generation and revision prompts are in `prompts/`.
