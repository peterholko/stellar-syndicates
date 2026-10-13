# Textured corporate freighter

A 3D reconstruction of the approved freighter artwork, built directly with Python and NumPy. No Blender dependency. The top-down reference supplies the original surface colors and panel detail; the isometric reference guides depth and volume.

## Deliverables

- `freighter.glb`: primary glTF 2.0 model with all three textures embedded. Import directly into a game engine or 3D editor.
- `freighter.obj` and `freighter.mtl`: alternative editable mesh export. Keep the `textures` directory alongside them.
- `viewer.html`: self-contained offline 3D viewer. Open in a browser to orbit, zoom, inspect top/bottom/front/side views, show wireframe, view the original texture colors without studio lighting, or export a PNG. The renderer and model are embedded; no network connection is required.
- `previews`: seven actual renders of the exported GLB, including direct-bottom and underside perspective views.
- `build_model.py` and `mesh_core.py`: editable geometry, UV projection, and export source.
- `validation.json`: Khronos glTF Validator report.

## Model

The model contains **10,952 triangles and 130 named mesh parts**. It includes exactly six independent freight pods in three pairs, chamfered armor, retention bands and brass latches, six open docking frames with crossed braces, a continuous central keel, a raised bridge, a compact navigation mast, and four recessed stern thrusters. Six cargo attachment sockets and four engine sockets are provided. There are no weapons.

Coordinates are **Y up, -Z forward**. Dimensions are approximately **11.10 × 3.30 × 16.60 arbitrary units**; the source artwork does not establish a real-world scale. Detailed parts remain separately editable, and the mesh has not been consolidated for minimum game-engine draw calls. The model is not rigged.

## Textures and interpretation

Three **2048 × 2048** PNG maps are included: base color, metallic/roughness, and emission. Original opaque reference pixels are preserved at native resolution in the color atlas, with edge colors padded into unused transparent margins. Cargo sides use explicit box UVs and matching panel regions from the same artwork. Cyan running lights also have small modeled inserts. Material and emission channels are derived from the original colors.

The cargo bottoms and retention bands, central keel, bridge belly, and aft machinery deck have UV-mapped ventral armor using the same embedded texture maps. Panel seams, brass trim, and service details continue underneath. The cabin belly maps to plain armor rather than repeating the canopy. A slightly cooler material tint distinguishes the underside, and a lower studio light keeps it visible while orbiting. Structural braces, clamps, and nozzle hardware retain their intentional solid metal materials.

The reference has painted lighting, which remains in the base color. The viewer's `Texture view` control removes extra studio lighting for a direct color comparison. Depth, hidden surfaces, container side layouts, and internal nozzle geometry are interpretations of the illustration, not scanned geometry.

Original references are preserved in `textures/reference-top.png` and `textures/reference-isometric.png`.

## Rebuild

From this directory, with Python and Node installed:

```sh
python3 -m pip install -r requirements.txt
python3 prepare_textures.py
python3 build_model.py
npm install
npm run build
npm run validate
```

Texture preparation uses the repository's `../../ships-2026-09-08/freighter.png`, falling back to the packaged reference copy when used outside the repository. Generated assets are included, so rebuilding is optional.

For a local preview server:

```sh
python3 preview_server.py
```

Open `http://127.0.0.1:8768/viewer.html`. The `?capture=1` variant writes seven review renders to `previews` through a loopback-only capture endpoint.

## Verification

The final GLB passes Khronos glTF Validator with **zero errors and zero warnings**. Informational notes concern unused UV coordinates on solid-color materials, an unused material, and empty attachment/socket nodes. The exported file was loaded and visually inspected in Three.js from above and below. An inspection of the exported triangles confirmed **100% texture coverage and nondegenerate UVs on the downward-facing surfaces of all six cargo pods and retention bands, the central keel, cabin, and main machinery deck**. Both GLB and OBJ/MTL exports reference the corrected underside textures.
