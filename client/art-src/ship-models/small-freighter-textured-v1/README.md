# Small Freighter — textured 3D reference model

Reconstructed from the approved five-view Small Freighter reference using Python and NumPy, without Blender. Four solid cargo sections flank an exposed central spine, with physical X-braced mounts, retaining straps, gold locks, a forward cabin, and two recessed engine nozzles.

## Model and downloads

- **small-freighter.glb** — primary glTF 2.0 model with all textures embedded; static parts consolidated by material.
- **small-freighter.obj / small-freighter.mtl** — editable alternative with separately named components. Keep the textures directory beside these files.
- **viewer.html** — standalone offline viewer with orbit, top, bottom, front, back and side cameras, wireframe, texture-only display, rotation, PNG export and GLB download.
- **previews/** — eight 1600 × 1200 renders from the exported GLB, including the underside.
- **build_model.py / geometry.py / mesh_core.py / prepare_textures.py** — editable geometry, projection and export source.

**9,336 triangles · 12,436 exported vertices · 7 material groups / draw calls · 113 authored components.**

Coordinates are **Y up, −Z forward**. Dimensions are **8.303 × 3.300 × 15.060 arbitrary units**; the illustration does not establish a physical scale. There are seven named attachment sockets: four cargo mounts, two engines and the bridge. The model has no animation rig.

The top drawing controls the planform. The other views guide thickness and surface projection, with their differing illustrated scales reconciled per component. Front artwork supplies the cargo end plates obscured by the rear engines. Rear artwork supplies the drive fairing and ion apertures. This is a geometric interpretation of a concept sheet, rather than a CAD model with surveyed dimensions.

## Textures and underside

Three 2048 × 2048 embedded PNG atlases provide base color, metallic/roughness and emission. The original 1774 × 887 reference is packaged as **textures/reference-sheet.png**. Native artwork pixels are retained; padding fills non-surface margins to prevent white UV seams and does not add source detail.

The dedicated **BOTTOM** artwork textures the cargo belly plates and access hatches, cabin and sensor fairings, spine, central drive fairing and both engine casings. Exposed structural beams, strap returns and machined nozzle parts intentionally use solid metal materials. Small cyan running lights use an emissive material. Source-painted lighting remains in the atlas; Texture view displays base colors without additional studio lighting.

## Detail experiment

This ship is included in the shared [low-triangle viewer](../interceptor-lod-experiment-v1/viewer.html). Open **http://127.0.0.1:8770/viewer.html?ship=small-freighter** while its local server is running.

| Representation | Triangles | Draw calls |
|---|---:|---:|
| Original / full detail | 9,336 | 7 |
| Medium / LOD 1 | 6,600 | 7 |
| Far / LOD 2 | 5,506 | 7 |
| Distant sprite | 2 | 1 |

The far version reduces geometry by **41.0%**. Cargo containers and hatches, the main spine, braced connections, cabin and central drive fairing remain exact; secondary fittings and round engine hardware are simplified. Every mesh level keeps the original texture bytes and underside mapping. The sprite atlas contains forty directions, including views from below. Detail switching and formation rendering belong to the experiment viewer; the base GLB has a single detail level.

## Verification

Khronos glTF Validator reports **zero errors and zero warnings**. Surface checks confirm no collapsed geometry, collapsed textured UV triangles or non-finite values, all fifteen required underside components have valid bottom-view UVs, and embedded images match the delivered files. Eight actual WebGL renders were reviewed. The shared viewer verifies automatic switching, hysteresis, and expected triangle/draw counts for 1 and 80 ships. Full-detail merged renders match the original pixel for pixel in the upper and lower comparisons.

## Rebuild

```sh
python3 -m pip install -r requirements.txt
python3 prepare_textures.py
python3 build_model.py
python3 verify_surfaces.py
npm install
npm run build
npm run validate
```

The included outputs are ready to use; rebuilding is optional. Texture preparation falls back to the packaged reference if the repository artwork is unavailable.

Open **viewer.html** directly, or run `python3 preview_server.py` and visit **http://127.0.0.1:8772/viewer.html**. The optional `?capture=1` parameter regenerates the eight previews through a loopback-only endpoint. Use `--port` for a different port.
