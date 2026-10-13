# Bulk Freighter — textured 3D reference model

Reconstructed from the approved five-view Bulk Freighter reference using Python and NumPy, without Blender. Eight rounded mineral silos alternate with eight rectangular cargo cages along a four-chord lattice spine. Sixteen open attachment trusses connect the cargo to the spine. A small forward command pod and a separate rear reactor with eight recessed engines complete the ship.

## Model and downloads

- **bulk-freighter.glb** — glTF 2.0 model with all textures embedded; static parts consolidated by material.
- **bulk-freighter.obj / bulk-freighter.mtl** — editable alternative with separately named components. Keep the textures directory beside these files.
- **viewer.html** — standalone offline viewer with orbit, top, bottom, front, back and side cameras, wireframe, texture-only display, rotation, PNG export and GLB download.
- **previews/** — eight 1600 × 1200 renders from the exported GLB, including the underside.
- **build_model.py / geometry.py / mesh_core.py / prepare_textures.py** — editable geometry, UV projection and export source.

**36,200 triangles · 45,752 exported vertices · 7 material groups / draw calls · 655 authored components.**

Coordinates are **Y up, −Z forward**. Dimensions are **9.810 × 2.488 × 19.515 arbitrary units**; the illustration does not establish a physical scale. There are twenty-six named attachment sockets: sixteen cargo mounts, eight engines, the bridge and reactor. The model is static, with no animation rig.

The top and bottom drawings establish the silhouette and eight cargo stations. Four transverse mineral silos on each flank alternate with four shorter cages. Curved restraint bands wrap each silo, with gold locks fitted to their curved shoulders. The four spine chords, diagonal braces, cross ties and cargo arms are actual geometry with open space between them. The stern engines occupy one row, all at the same height and exhaust plane.

The other reference views guide depth and surface projection, with illustrated scale differences reconciled per component. Complete cargo-cage side panels are reused where two side faces are obscured in the concept sheet. Matching front artwork supplies hidden silo fore/aft faces. Reactor panel artwork supplies the drive bulkhead so engine discs are not painted behind the physical nozzles. This is a geometric interpretation of concept art, rather than a CAD model with surveyed dimensions.

## Textures and underside

Three 2048 × 2048 embedded PNG atlases provide base color, metallic/roughness and emission. The original 1774 × 887 reference is packaged as **textures/reference-sheet.png**. Native artwork pixels are retained; padding fills non-surface margins to prevent white UV seams and does not add source detail.

Dedicated **BOTTOM** artwork covers silo bellies and sump hatches, cargo cages and inspection panels, spine service housings, the command pod and sensor fairings, reactor and drive support, and all eight engine casings. Trusses, restraint bands, machined rims and other metal fittings use matching solid materials. Small running lights are emissive. Painted lighting remains in the atlas; Texture view displays base colors without additional studio lighting.

## Detail experiment

The ship is included in the shared [low-triangle viewer](../interceptor-lod-experiment-v1/viewer.html). Open **http://127.0.0.1:8770/viewer.html?ship=bulk-freighter** while its local server is running.

| Representation | Triangles | Draw calls |
|---|---:|---:|
| Original / full detail | 36,200 | 7 |
| Medium / LOD 1 | 25,902 | 7 |
| Far / LOD 2 | 21,442 | 7 |
| Distant sprite | 2 | 1 |

The far version reduces geometry by **40.8%**. All cargo modules, the main lattice and cargo attachment trusses, service housings, command pod, reactor and drive support retain their geometry. Rounded restraint bands, engine hardware and secondary fittings are simplified. Every mesh level keeps the original texture bytes and underside mapping. The sprite atlas has forty directions, including views from below. Detail switching and formation rendering belong to the experiment viewer; the base GLB has one detail level.

## Verification

Khronos glTF Validator reports **zero errors and zero warnings**. Surface checks confirm no collapsed geometry, collapsed textured UV triangles or non-finite values, all sixty-eight checked underside components have valid bottom-view UVs, and embedded images match the delivered files. All sixteen cargo bodies are closed and manifold across their material boundaries, with positive enclosed volume. Twenty unobstructed vertical rays check openings beside the cargo attachments and within the spine web. Socket checks confirm sixteen cargo mounts and eight engines in one row on a common exhaust plane.

Eight WebGL views were inspected during refinement. The shared viewer checks automatic switching, hysteresis and expected triangle/draw counts for single ships and 80-ship formations. Full-detail merged renders match the original pixel for pixel in upper and lower comparisons. Formation counters describe this isolated preview, not a game FPS benchmark; instancing and battle effects are outside the experiment.

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

Open **viewer.html** directly, or run `python3 preview_server.py` and visit **http://127.0.0.1:8775/viewer.html**. The optional `?capture=1` parameter regenerates the eight previews through a loopback-only endpoint. Use `--port` for a different port.
