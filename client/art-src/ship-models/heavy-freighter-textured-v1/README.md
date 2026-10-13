# Heavy Freighter — textured 3D reference model

Reconstructed from the approved five-view Heavy Freighter reference using Python and NumPy, without Blender. A continuous armored pressure hull carries four cargo hatches, a central service spine, a projecting command prow, two folded flank cranes, and eight recessed exhausts arranged in two tiers of four.

## Model and downloads

- **heavy-freighter.glb** — primary glTF 2.0 model with all textures embedded; static parts consolidated by material.
- **heavy-freighter.obj / heavy-freighter.mtl** — editable alternative with separately named components. Keep the textures directory beside these files.
- **viewer.html** — standalone offline viewer with orbit, top, bottom, front, back and side cameras, wireframe, texture-only display, rotation, PNG export and GLB download.
- **previews/** — eight 1600 × 1200 renders from the exported GLB, including the underside.
- **build_model.py / geometry.py / mesh_core.py / prepare_textures.py** — editable geometry, projection and export source.

**19,564 triangles · 21,864 exported vertices · 7 material groups / draw calls · 178 authored components.**

Coordinates are **Y up, −Z forward**. Dimensions are **9.630 × 2.800 × 14.8625 arbitrary units**; the illustration does not establish a physical scale. There are sixteen named attachment sockets: four cargo mounts, two cranes, eight engines, the bridge and reactor. Crane components are static and merged by material; the model has no animation rig.

The top drawing controls the planform. Other views guide thickness and surface projection, with their differing illustrated scales reconciled per component. Front artwork supplies the main hull's forward and aft cargo walls, avoiding painted engines behind the modeled exhausts. Rear artwork supplies the reactor-service fairing and ion apertures. Hidden surfaces are completed with matching metal materials. This is a geometric interpretation of a concept sheet, rather than a CAD model with surveyed dimensions.

## Textures and underside

Three 2048 × 2048 embedded PNG atlases provide base color, metallic/roughness and emission. The original 1774 × 887 reference is packaged as **textures/reference-sheet.png**. Native artwork pixels are retained; padding fills non-surface margins to prevent white UV seams and does not add source detail.

The dedicated **BOTTOM** artwork textures the pressure hull, cargo service plates, command prow and sensor fairings, central spine and inspection rail, reactor fairing, engine support, crane carriers and hinge blocks, and all eight engine casings. Exposed braces, hydraulic rods, collars and machined nozzle parts intentionally use solid metal materials. Cyan running lights use an emissive material. Source-painted lighting remains in the atlas; Texture view displays base colors without additional studio lighting.

## Detail experiment

This ship is included in the shared [low-triangle viewer](../interceptor-lod-experiment-v1/viewer.html). Open **http://127.0.0.1:8770/viewer.html?ship=heavy-freighter** while its local server is running.

| Representation | Triangles | Draw calls |
|---|---:|---:|
| Original / full detail | 19,564 | 7 |
| Medium / LOD 1 | 12,226 | 7 |
| Far / LOD 2 | 9,192 | 7 |
| Distant sprite | 2 | 1 |

The far version reduces geometry by **53.0%**. The pressure hull, four cargo hatches and locks, armor panels, central spine, braces, bridge and sensor fairings, folded crane arms and mounting blocks, reactor fairing and engine support retain their exact geometry. Round engine hardware and secondary hydraulic fittings are simplified. Every mesh level keeps the original texture bytes and underside mapping. The sprite atlas contains forty directions, including views from below. Detail switching and formation rendering belong to the experiment viewer; the base GLB has a single detail level.

## Verification

Khronos glTF Validator reports **zero errors and zero warnings**. Surface checks confirm no collapsed geometry, collapsed textured UV triangles or non-finite values, all thirty-one required underside components have valid bottom-view UVs, and embedded images match the delivered files. The main pressure hull is closed and manifold across its material boundaries. Socket checks confirm four cargo positions, two cranes, and eight engines in four columns and two tiers on a common exhaust plane.

Eight actual WebGL views were inspected during refinement. The shared viewer verifies automatic switching, hysteresis, and expected triangle/draw counts for 1 and 80 ships. Full-detail merged renders match the original pixel for pixel in the upper and lower comparisons. All six ships remain selectable, and the packaged sprite transitions correctly at distant camera settings.

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

Open **viewer.html** directly, or run `python3 preview_server.py` and visit **http://127.0.0.1:8774/viewer.html**. The optional `?capture=1` parameter regenerates the eight previews through a loopback-only endpoint. Use `--port` for a different port.
