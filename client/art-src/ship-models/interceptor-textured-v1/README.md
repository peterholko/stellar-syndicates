# Textured Interceptor

An editable 3D reconstruction of the approved front / top / side reference sheet, built directly with Python and NumPy. No Blender dependency.

## Deliverables

- `interceptor.glb`: primary glTF 2.0 model, with three PNG textures embedded. Import this file into a game engine or 3D editor.
- `interceptor.obj` and `interceptor.mtl`: alternate editable mesh export. Keep the `textures` folder next to them.
- `viewer.html`: portable interactive viewer. Open it directly in a browser; the model, renderer, and textures are embedded, and it needs no network connection. Includes orbit, top, bottom, front, side, wireframe, texture-only display, and PNG export.
- `previews`: actual renders of the exported GLB from seven camera angles, including direct-bottom and underside perspective views.
- `build_model.py` and `mesh_core.py`: editable geometry and UV mapping source.

The model has **7,208 triangles and 30 named mesh parts**, including a faceted fuselage, vaulted canopy, swept wings, wingtip fins, gun housings, and recessed engine nozzles. Weapon and engine socket nodes are included. Coordinates are **Y up, -Z forward**. Dimensions are approximately 11 × 2.39 × 13.45 arbitrary units; the drawing did not specify a real-world scale.

## Textures and interpretation

The original reference artwork supplies the visible color textures, including the silver panels, charcoal structure, brass details, cockpit frames, and cyan lights. The UV atlas preserves the source pixels and extends edge colors into background margins to avoid white seams. Emission and metallic/roughness channels are derived from those colors. All three maps are 1536 × 1024 pixels.

The fuselage belly, wing undersides, root armor, and lower wingtip faces have mapped ventral armor using these same embedded textures. Wing seams and root vents continue underneath. The fuselage belly uses the reference's aft hull panels and service details, excluding the upper canopy. A slightly cooler material tint distinguishes the ventral armor, and a lower studio light makes it visible while orbiting. Nozzle lips, recesses, and metal collars retain their intentional solid-color materials.

This is a modeled reconstruction of an illustration, not a scan. Depth, small inconsistencies between the reference views, the unseen underside, and internal nozzle surfaces are interpreted. The reference contains painted lighting, which remains in the color texture. `Texture view` in the viewer shows that artwork without added studio lighting. The model is not rigged or optimized into a single game-engine draw call.

## Rebuild

Python requires NumPy and Pillow. From this folder:

```sh
python3 prepare_textures.py
python3 build_model.py
npm install
npm run build
npm run validate
```

`prepare_textures.py` uses the repository reference at `../../interceptor-reference-v1/interceptor-reference.png`; the complete delivery also includes an identical copy at `textures/reference-sheet.png`. The delivered textures, GLB, OBJ, and viewer are already generated.

To serve the viewer and optionally capture review images:

```sh
python3 preview_server.py
```

Open `http://127.0.0.1:8767/viewer.html`. The `?capture=1` variant writes the seven review PNGs to `previews` through a loopback-only capture endpoint.

Khronos glTF Validator reports zero errors and zero warnings. Informational notes describe non-power-of-two textures, unused UV coordinates on solid-color materials, unused materials, and empty socket nodes. The GLB was loaded and visually inspected in Three.js from above and below. An inspection of the exported triangles confirmed 100% texture coverage and nondegenerate UVs on the downward-facing fuselage, wings, root armor, wingtip endplates, engine fairings, nozzle jackets, and weapon housings. Both GLB and OBJ/MTL exports include the corrected underside mapping.
