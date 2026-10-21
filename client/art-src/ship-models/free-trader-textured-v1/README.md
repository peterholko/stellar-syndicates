# Free Trader — textured 3D model

Reconstructed from the approved Free Trader sprite and five-view sheet in
`../../free-trader-reference-v1/`, following the Mining Fleet Python/NumPy
pipeline. No Blender, new concept art or image upscaling was used.

## Asset

- `free-trader.glb`: self-contained glTF 2.0, **7,872 triangles**, two materials
  and two render sections. Static parts are merged by material, with exact
  vertex deduplication and 16-bit indices.
- `free-trader.obj` / `free-trader.mtl`: textured mesh interchange copy.
- `textures/`: 2048 × 2048 base-colour, metallic/roughness and emissive atlases,
  plus the unchanged five-view reference sheet.
- `concept.png`: unchanged approved transparent top-view sprite.
- `previews/`: seven rendered inspection views and a contact sheet.
- `viewer.html`: standalone interactive Three.js preview with the model and
  renderer embedded; orbit, orthographic views, wireframe, texture view,
  screenshot and GLB download controls.

The model uses **Y up, −Z forward**. Dimensions are approximately
11.8905 × 3.43 × 19.8637 relative model units. Gameplay mass/scale, LODs,
sprite baking and game catalog wiring are not part of this model delivery.

## Design and textures

The broad market canopy has eight separate curved panels with alternating
cream and purple paint. The hull has four side-facing docking collars, one
raised glazed bridge, a narrow aft waist and two oversized engines. It is
unarmed. Closed ventral cargo hatches, plating and service fittings provide
dedicated underside detail.

The approved native 1254 px sprite supplies the dorsal texture. Native pixels
from the five-view sheet supply ventral, side, glazing and docking/nozzle
textures. Existing Mining Fleet material-strip crops cover hidden fittings
and structural walls. All atlas packing is at native resolution, with edge
padding for filtering; the source images remain unchanged. Purple is paint,
with zero emissive contribution; cyan glass and apertures have restrained
emission. The top view governs footprint and station positions where the
concept elevations disagree, while side/bottom views guide volume and
underside construction.

## Attachment nodes

Socket directions are stored in node `extras.direction` using exported model
axes. These nodes are children of the hull root and follow its transforms.

| Node | Location | Direction |
| --- | --- | --- |
| `socket.docking.1` | Port forward collar | −X |
| `socket.docking.2` | Port aft collar | −X |
| `socket.docking.3` | Starboard forward collar | +X |
| `socket.docking.4` | Starboard aft collar | +X |
| `socket.engine.port` | Port nozzle | +Z |
| `socket.engine.starboard` | Starboard nozzle | +Z |

There are no weapon sockets.

## Rebuild and inspect

From the repository root, using the existing NumPy/Pillow, client Three.js,
and Mining Fleet glTF-validator dependencies:

```sh
python3 client/art-src/ship-models/free-trader-textured-v1/build_model.py
python3 client/art-src/ship-models/free-trader-textured-v1/inspect_model.py
node client/art-src/ship-models/mining-fleet-v1/validate.mjs ../free-trader-textured-v1/free-trader.glb
node client/art-src/ship-models/free-trader-textured-v1/test_asset.mjs
node client/art-src/ship-models/free-trader-textured-v1/build_preview.mjs
python3 -m http.server 8786 --bind 127.0.0.1 --directory client/art-src/ship-models/free-trader-textured-v1
```

Open `http://127.0.0.1:8786/viewer.html`. The preview builder reuses the existing
Pirate Fleet esbuild dependency when esbuild is not available at client level.

Validation results are recorded in `gltf-validation.json`,
`surface-verification.json` and `three-validation.json`:

- glTF validation: **zero errors and zero warnings**.
- 94 closed components with finite attributes, valid indices, unit normals,
  positive volume and consistent winding; no degenerate physical triangles.
- Every triangle has textured, noncollapsed UVs, including 1,622 downward-facing
  triangles; no untextured material slots.
- Required counts: eight canopy panels, four docking collars, two engines,
  one bridge, zero guns, six sockets.
- The game's installed Three.js r180 loads both material sections and all
  embedded textures; all six sockets follow translation, rotation and scale.
- Real WebGL preview checked in the in-app browser, including underside and
  side views. No browser warnings or errors were reported during inspection.

The OBJ export carries mesh/material data; named attachment nodes live in GLB.
Software previews inspect actual embedded texture pixels, but are not a GPU
performance benchmark.
