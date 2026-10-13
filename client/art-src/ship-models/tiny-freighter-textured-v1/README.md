# Tiny Freighter — textured 3D reference model

Built from the approved four-view Tiny Freighter sheet using Python and NumPy, without Blender. The model preserves the two freight sections, narrow forward cabin, gold retaining clamps, graphite framing, cyan lights, and twin rear engines.

## Files

- **tiny-freighter.glb** — primary glTF 2.0 model; all textures embedded, static geometry combined by material.
- **tiny-freighter.obj / tiny-freighter.mtl** — editable alternative with 66 individually named components. Keep the textures folder alongside these files.
- **viewer.html** — standalone offline interactive viewer, including orbit, top, bottom, front, back and side cameras, wireframe, texture-only display, rotation, PNG export, and GLB download.
- **previews/** — eight 1600 × 1200 renders of the exported GLB, including the underside.
- **build_model.py / geometry.py / mesh_core.py** — editable geometry, UV projection, and export source.
- **prepare_textures.py** — reference-to-atlas preparation.
- **validation.json / surface-verification.json** — format and surface verification reports.

## Model details

**6,804 triangles · 8,868 exported vertices · 7 render sections / draw calls.**

The GLB combines 66 authored static parts into seven material groups, deduplicates identical vertex attributes, and uses 16-bit indices. Component index ranges remain in each mesh's metadata; the OBJ retains separate objects. This version has one detail level and no animation rig.

Coordinates: **Y up, -Z forward**. Dimensions: **7.648 × 3.610 × 14.980 arbitrary units**. The illustration does not establish a physical scale. Two engine sockets, two cargo sockets, and a bridge socket are included.

The containers, rail returns, clamps, access hatches, cabin, engine casings, tapered nozzle jackets, and recessed exhaust apertures have physical depth. The model is a geometric interpretation of the illustrated views, whose proportions are not perfectly consistent. Top artwork controls the planform; side and front artwork guide the depth and visible end panels. Rear surfaces and occluded cargo walls are inferred using matching panels from the same reference.

## Textures and underside

Three 2048 × 2048 PNG atlases provide base color, metallic/roughness, and emission. The approved 1774 × 887 artwork is preserved in **textures/reference-sheet.png**. The atlas retains native reference pixels and pads surrounding background to prevent white seams; its larger canvas does not add source detail.

The dedicated **BOTTOM** drawing covers the belly armor, cargo undersides, access hatches, cabin underside, sensor fairings, central tail, and both engine casings. Structural beams, rail end returns, and machined nozzle hardware use intentional solid metal materials. Small running-light inserts use a cyan emissive material. Painted lighting remains in the source; the viewer's Texture view button shows the base colors without additional studio lighting.

## Validation

Khronos glTF Validator: **zero errors, zero warnings**. The exported geometry has no zero-area triangles, collapsed textured UV triangles, or non-finite values. Embedded images match the delivered texture files byte for byte. The primary downward-facing surfaces all have valid UVs in the dedicated bottom-view region. The model was loaded in Three.js and reviewed from above, below, and oblique angles; the interactive viewer reports seven draw calls.

## Rebuild

From this folder:

```sh
python3 -m pip install -r requirements.txt
python3 prepare_textures.py
python3 build_model.py
python3 verify_surfaces.py
npm install
npm run build
npm run validate
```

All generated files are included, so rebuilding is optional. Texture preparation uses the packaged reference sheet when the repository reference is unavailable.

Open **viewer.html** directly for the offline preview, or serve it locally:

```sh
python3 preview_server.py
```

Then open **http://127.0.0.1:8771/viewer.html**. The optional `?capture=1` query saves the eight renders through a loopback-only capture endpoint. Use `--port` to choose another port.
