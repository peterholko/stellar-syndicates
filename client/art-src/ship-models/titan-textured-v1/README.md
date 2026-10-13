# Titan — textured 3D model

Built from the approved **Titan v4 five-view reference**, using Python and NumPy without Blender. The low, layered hull has broad armored shoulders, a recessed command blister, a deep service keel, two stern engine blocks with four vertical exhausts, and four separate twin-barrel turret assemblies.

## Open or download

- **titan.glb**: complete glTF 2.0 source model with embedded textures and separate turret assemblies.
- The **Titan ZIP** also contains an `optimized/` folder with all three articulated LOD GLBs, the sprite atlas, and their metadata.
- **titan.obj / titan.mtl**: alternative export with separately named components. Keep the textures folder beside the files.
- **viewer.html**: portable offline preview with orbit, top, bottom, front, back and side cameras, wireframe, texture-only display, rotation, PNG export and GLB download.
- **previews/**: eight 1600 × 1200 renders of the exported model.
- **build_model.py, mesh_core.py, geometry.py, prepare_textures.py**: editable geometry, UV mapping, export and texture preparation.

The shared experiment opens at **http://127.0.0.1:8770/viewer.html?ship=titan**. It includes three mesh levels, a forty-view sprite atlas, turret traversal, 1/12/80-ship formations, and the eight-ship size comparison. The optimized GLBs in the sibling experiment folder add named rotation pivots for all four turrets.

## Geometry and textures

**19,360 triangles · 22,516 vertices · 23 render sections · 228 authored components.** Seven static hull material groups and four material groups per turret preserve independent movement. All mesh levels use the same seven materials.

Coordinates are **Y up, −Z forward**. Bounds are **10.25 × 3.1717 × 18.65 arbitrary units**. Eighteen named sockets locate the bridge, reactor, four engines, four turret mounts and eight gun muzzles. Source geometry is in model space; the optimized models have turret pivot hierarchies. There is no skeletal rig or animation clip.

The top drawing sets the silhouette and the side drawing guides hull depth. The illustrated views have inconsistent dimensions; local projection adjustments align them to a coherent model. Layered shelves, lower armor, nozzle cavities and turret housings are real geometry. Small panel lines and machinery remain in the original artwork.

Three embedded **2048 × 2048 PNGs** provide base color, metallic/roughness and emission. **textures/reference-sheet.png** contains the original 1774 × 887 sheet. Native artwork pixels are retained, with margin padding to avoid white seams. The metallic/roughness and emission maps are derived from the reference colors; painted lighting remains in the base color.

Dedicated **BOTTOM** artwork covers the hull, keel, shoulder armor, maintenance plates, bridge, stern fairing, both engine housings and their lower service plates. Back-view artwork is registered to the four physical exhaust apertures. Small rails, gun barrels, collars and frame pieces use matching metal materials.

## Detail levels

| Representation | Triangles | Draw calls |
|---|---:|---:|
| Original / full detail | 19,360 | 23 |
| Medium / LOD 1 | 11,272 | 23 |
| Far / LOD 2 | 7,946 | 23 |
| Distant sprite | 2 | 1 |

The far model reduces triangles by **59.0%**. Main hull, deck, belly, shoulder armor, command blister, stern fairing, engine blocks, sidewall frames and turret housings retain their exact geometry. Rounded nozzles, barrels and fittings are simplified. Every mesh level retains the original image bytes, materials, bottom projection and sockets. Full detail is already consolidated, so there is no additional draw-call reduction in LOD 0.

The optimized models use `turret.port.1.pivot`, `turret.port.2.pivot`, `turret.starboard.1.pivot` and `turret.starboard.2.pivot`. Rotate each about local Y. Its two muzzle sockets move with it. The sprite atlas has forty views including the underside, with turrets in their resting pose. Detail switching is implemented in the experiment viewer, not inside the GLBs.

In **One of each ship → Game display scale**, Titan length is **8× Interceptor** and **2× Cruiser**, matching the game's existing display ratios. This is a visual scale, not a physical measurement.

## Verification

Khronos validation reports **zero errors and zero warnings** for the source and all three optimized models. Checks cover non-finite values, collapsed triangles and UVs, complete bottom-view coverage on 29 primary armor components, matching embedded images, three closed main bodies, four exhaust positions, separate turret assemblies and eight muzzle sockets.

The original and merged models render identically in matched upper and lower views. Browser checks cover actual triangle/draw counts in single and 80-ship scenes, automatic LOD transitions, hysteresis and turret movement. The eight-ship comparison also verifies display-length ratios and all existing ship modes. Counts describe the isolated viewer; it does not use instancing or battle effects.

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

The included outputs are ready to open. Texture preparation falls back to the packaged reference if the repository artwork is unavailable.

For local preview, run `python3 preview_server.py` and open **http://127.0.0.1:8776/viewer.html**. Add `?capture=1` to regenerate the eight renders through the loopback-only capture endpoint. Use `--port` to select another port.
