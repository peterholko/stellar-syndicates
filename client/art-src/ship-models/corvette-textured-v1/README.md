# Corvette — textured 3D model

Built from the approved five-view Corvette reference using Python and NumPy, without Blender. The model has broad forward shoulders, a narrow reinforced aft spine, twin cylindrical engines with recessed exhausts, a low command bridge, and four separate twin-barrel turrets.

## Open or download

- **corvette.glb**: glTF 2.0 source model with three embedded texture maps and four separate turret assemblies.
- **corvette.obj / corvette.mtl**: editable interchange export with named components; keep the textures folder beside them.
- **viewer.html**: portable offline preview with orbit, top, bottom, front, back and side cameras, wireframe, texture display, PNG export and GLB download.
- **previews/**: eight 1600 × 1200 renders of the exported model.
- **build_model.py, mesh_core.py, geometry.py, prepare_textures.py**: editable geometry, UV mapping, export and texture preparation.
- The **Corvette ZIP** also contains `optimized/` with all three articulated LOD GLBs, the sprite atlas and their metadata.

The shared low-triangle viewer opens at **http://127.0.0.1:8770/viewer.html?ship=corvette**. It includes automatic detail switching, turret traversal, 1/12/80-ship formations, and the nine-ship size comparison. Choose **Formation → One of each ship** to compare sizes.

## Geometry and textures

**20,700 triangles · 20,752 vertices · 23 render sections · 137 authored components.** Static geometry is consolidated into seven material groups; each turret retains four material groups.

Coordinates are **Y up, −Z forward**. Bounds are **9.92 × 3.0579 × 14.24 arbitrary model units**. Sixteen sockets locate the bridge, reactor, two engines, four turret mounts and eight gun muzzles. Source mesh coordinates are in model space; the optimized GLBs add turret pivot hierarchies. There is no skeletal rig or animation clip.

The top drawing guides the silhouette; the side and end drawings guide depth and surface details. The illustrated reference views have inconsistent dimensions, so regional projection adjustments register them to the solid geometry. Shoulder armor, lower service plates, turret mounts, engine cowls, nozzles and recessed apertures are modeled. Small seams, vents and surface machinery use the reference artwork.

Three embedded **2048 × 2048 PNGs** provide base color, metallic/roughness and emission. `textures/reference-sheet.png` retains the original 1774 × 887 reference. Margin padding avoids white seams. Metallic/roughness and emission are derived from the artwork colors; the source's painted lighting remains in the base color.

Dedicated **BOTTOM** artwork covers the pressure hull, shoulders, keel, maintenance panels, aft spine, engine housings and cowls. Rear shoulder walls use unoccluded plating from the back view, while each physical exhaust has its own local back-view projection. Small barrels, collars, structural braces and rails use matching metal materials.

## Detail levels

| Representation | Triangles | Draw calls |
|---|---:|---:|
| Original / LOD 0 | 20,700 | 23 |
| Medium / LOD 1 | 11,032 | 23 |
| Far / LOD 2 | 7,020 | 23 |
| Distant sprite | 2 | 1 |

The far model reduces triangles by **66.1%**. Hull, shoulder armor, aft spine, command bridge, lower plates, engine housings/cowls and turret housings retain their exact geometry. Round fittings, barrels and nozzle hardware use fewer triangles. All mesh levels retain the original texture bytes, materials, underside projection and sockets. LOD 0 starts consolidated, so its draw count matches the source.

The optimized models have `turret.port.1.pivot`, `turret.port.2.pivot`, `turret.starboard.1.pivot` and `turret.starboard.2.pivot`. Rotate about local Y to traverse each mount; its two muzzle sockets move with it. The forty-view sprite atlas includes the underside and uses the resting turret pose. Detail switching lives in the viewer, rather than inside the GLBs.

In **One of each ship → Game display scale**, Corvette length is **1.5× Interceptor**, following `COMBAT_HULL_LENGTH_SCALE` in the game. These are display proportions, not physical dimensions.

## Verification

The source and all three optimized GLBs pass Khronos validation with **zero errors and zero warnings**. Geometry checks verify closed pressure bodies, finite vertices, noncollapsed triangles/UVs, complete bottom mapping on primary armor components, identical embedded texture bytes, two aligned circular exhausts, four turret assemblies and eight weapon sockets.

Browser checks compare original and merged upper/lower renders, count geometry and draw calls in single/80-ship scenes, exercise automatic LOD transitions and hysteresis, and verify turret articulation. The nine-ship lineup checks both scale options, names, actual rendered costs and all existing model modes. These checks describe the isolated viewer; they are not an FPS benchmark and do not include instancing or battle effects.

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

The included outputs are ready to open. Texture preparation falls back to the packaged reference when repository artwork is unavailable.

Run `python3 preview_server.py` for the source preview at **http://127.0.0.1:8777/viewer.html**. Add `?capture=1` to regenerate the eight renders through the local capture endpoint, or use `--port` to select a different port.
