# Cruiser · textured five-view reconstruction

An editable 3D reconstruction of `../../cruiser-reference-v1/cruiser-reference.png`, built directly with Python and NumPy. No Blender dependency. The reference's top, bottom, side, front and back drawings provide separate UV projections.

## Deliverables

- `cruiser.glb`: primary glTF 2.0 model with all three texture maps embedded.
- `cruiser.obj` and `cruiser.mtl`: alternate editable mesh export. Keep the `textures` directory alongside these files.
- `viewer.html`: self-contained offline Three.js viewer with the model and renderer embedded. Includes orbit/zoom, Top, Bottom, Front, Back and Side views, rotation, wireframe, texture-only display, PNG export and GLB download. No network connection is required when opened locally.
- `previews`: eight actual 1600 × 1200 renders of the exported GLB, including the belly and four-engine stern.
- `build_model.py`, `mesh_core.py` and `prepare_textures.py`: editable geometry, UV projection, texture preparation and export source.
- `validation.json`: Khronos glTF Validator report.
- `surface-verification.json`: inspection of the exported underside triangles and equipment counts.

## Model

**14,400 triangles · 74 named mesh parts · 27,584 exported vertices.**

The model contains a continuous armored hull, layered pointed prow, raised command bridge, central service spine, beveled shoulder armor, two genuinely recessed bays with small modeled docked shuttles, two twin-barrel turrets with recessed muzzle bores, ventral radiator enclosures, and four stepped cylindrical engines. Each engine has an open nozzle throat and a recessed cyan aperture mapped from the rear reference. Turret, weapon, bay and engine attachment sockets are included.

Coordinates are **Y up, -Z forward**. Dimensions are approximately **11.08 × 3.07 × 18.58 arbitrary units**; the illustration does not specify a real-world scale. Components remain separately editable. The model is not rigged, and draw calls have not been consolidated for deployment in a game.

## Textures

Three **2048 × 2048** PNG maps contain base color, metallic/roughness and emission. The original **1774 × 887** reference is preserved in `textures/reference-sheet.png`. Its native artwork pixels are retained in the atlas; unused white background margins are padded with nearby surface colors. The larger atlas canvas provides packing space, not invented source-image detail.

The **underside uses the dedicated BOTTOM drawing**, including its distinct armor, dark keel, access hatches, radiators, brass fittings and cyan service lights. It does not reuse the bridge or dorsal bay imagery. GLB and OBJ/MTL both include the ventral texture mapping. Small nozzle walls, metal collars, gun barrels and bay walls have intentional solid metal materials.

This is a modeled interpretation of an illustrated reference. Small proportion differences between the five drawings are reconciled into one coherent hull; the top drawing controls the planform, and side/front/rear drawings guide depth and equipment profiles. Painted lighting remains in the source artwork. The viewer's `Texture view` control displays the base colors without additional studio lighting.

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

Texture preparation falls back to the packaged `textures/reference-sheet.png` when used outside the repository. All generated files are already included, so rebuilding is optional.

To serve the preview locally:

```sh
python3 preview_server.py
```

Open `http://127.0.0.1:8769/viewer.html`. Add `?capture=1` to save eight review renders through the loopback-only capture endpoint.

## Verification

The exported GLB passes Khronos glTF Validator with **zero errors and zero warnings**. Informational notes concern unused materials, UVs on solid-color materials and empty attachment nodes. The file was loaded and visually inspected in Three.js from above, below, front, back and side. The exported triangles were checked for **100% textured coverage, valid UV area and correct bottom-atlas placement** on the downward-facing main hull, shoulder armor, ventral radiators and all four nacelles. Equipment checks confirm exactly two main turrets, four main gun barrels and four stern engines.
