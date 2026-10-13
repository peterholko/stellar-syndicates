# Ship model detail experiment: nine ships

A separate working prototype for static-part merging, lower-detail models, and distant sprites. The game and all original source models are unchanged. No Blender was used. The original experiment directory and URL are retained so existing links continue to work.

Open **viewer.html** directly, or run `python3 preview_server.py` and open **http://127.0.0.1:8770/viewer.html**. The portable viewer embeds all nine ships, all textures, nine sprite atlases, and Three.js. Normal inspection makes no external network requests.

Choose **Ship model → Interceptor / Corvette / Cruiser / Tiny Freighter / Small Freighter / Large Freighter / Heavy Freighter / Bulk Freighter / Titan**. The viewer opens on Cruiser by default; `?ship=interceptor` opens on Interceptor and **?ship=tiny-freighter** opens on Tiny Freighter. **?ship=small-freighter** opens on Small Freighter; **?ship=large-freighter** opens on Large Freighter; **?ship=heavy-freighter** opens on Heavy Freighter; **?ship=bulk-freighter** opens on Bulk Freighter; **?ship=titan** opens on Titan; **?ship=corvette** opens on Corvette. Original, Merged, Medium, and Far show the selected representation at the same camera position. Automatic and the four distance buttons demonstrate the transitions. Wireframe reveals the geometry reduction; Formation offers 1, 12, and 80 copies, plus **One of each ship**.

## Size comparison

Choose **Formation → One of each ship**, or open **http://127.0.0.1:8770/viewer.html?formation=all**. Nine labeled ships share one orthographic camera, with their sterns aligned. Top is the initial view; Orbit and Bottom also retain the common scale. Reset frames the entire lineup again. Save PNG includes the names and size labels.

**Size scale → Game display scale** is the default. Each GLB is uniformly scaled by its measured nose-to-tail bounds to match the existing battle theater's display-length proportions:

| Ship | Length relative to Interceptor |
|---|---:|
| Interceptor | 1.00× |
| Corvette | 1.50× |
| Tiny Freighter | 2.07× |
| Small Freighter | 2.54× |
| Cruiser | 4.00× |
| Large Freighter | 4.04× |
| Heavy Freighter | 5.05× |
| Bulk Freighter | 6.27× |
| Titan | 8.00× |

These are presentation sizes, not meters, collision dimensions, or a new physical scale. `comparison-scale.js` records the 2026-09-30 snapshot: the Interceptor/Corvette/Cruiser/Titan ratios come from `client/src/shipart.ts` (`COMBAT_HULL_LENGTH_SCALE`); freighters use `client/src/battletheater.ts` (`MASS` and `spritePx`). No game code is changed.

**Original model scale** removes those display transforms and shows the original GLB dimensions in model units. Those models were authored independently and are not calibrated to a common physical scale. `?formation=all&scale=native` opens that comparison directly.

Detail mode applies to the entire lineup, with automatic LOD selected independently for each ship's projected diameter. The counters and cost table show the combined cost of all nine ships. Wireframe and the Corvette, Cruiser, and Titan turret slider also work in the lineup. **Download lineup sizes (JSON)** exports the selected scale, dimensions, and uniform transforms. Selecting a ship or a 1/12/80 formation returns to the existing perspective preview.

The lineup contains the nine models available in this experiment; the separate Medium Freighter source has no LOD variants in this viewer yet.

## Results

| Ship | Representation | Triangles / ship | Draw calls / ship |
|---|---|---:|---:|
| Interceptor | Original | 7,208 | 39 |
| Interceptor | Merged / LOD 0 | 7,208 | 6 |
| Interceptor | Medium / LOD 1 | 3,168 | 6 |
| Interceptor | Far / LOD 2 | 1,664 | 6 |
| Corvette | Original | 20,700 | 23 |
| Corvette | Merged / LOD 0 | 20,700 | 23 |
| Corvette | Medium / LOD 1 | 11,032 | 23 |
| Corvette | Far / LOD 2 | 7,020 | 23 |
| Cruiser | Original | 14,400 | 104 |
| Cruiser | Merged / LOD 0 | 14,400 | 17 |
| Cruiser | Medium / LOD 1 | 7,556 | 17 |
| Cruiser | Far / LOD 2 | 5,374 | 17 |
| Tiny Freighter | Original | 6,804 | 7 |
| Tiny Freighter | Merged / LOD 0 | 6,804 | 7 |
| Tiny Freighter | Medium / LOD 1 | 4,712 | 7 |
| Tiny Freighter | Far / LOD 2 | 3,624 | 7 |
| Small Freighter | Original | 9,336 | 7 |
| Small Freighter | Merged / LOD 0 | 9,336 | 7 |
| Small Freighter | Medium / LOD 1 | 6,600 | 7 |
| Small Freighter | Far / LOD 2 | 5,506 | 7 |
| Large Freighter | Original | 14,524 | 7 |
| Large Freighter | Merged / LOD 0 | 14,524 | 7 |
| Large Freighter | Medium / LOD 1 | 9,982 | 7 |
| Large Freighter | Far / LOD 2 | 7,984 | 7 |
| Heavy Freighter | Original | 19,564 | 7 |
| Heavy Freighter | Merged / LOD 0 | 19,564 | 7 |
| Heavy Freighter | Medium / LOD 1 | 12,226 | 7 |
| Heavy Freighter | Far / LOD 2 | 9,192 | 7 |
| Bulk Freighter | Original | 36,200 | 7 |
| Bulk Freighter | Merged / LOD 0 | 36,200 | 7 |
| Bulk Freighter | Medium / LOD 1 | 25,902 | 7 |
| Bulk Freighter | Far / LOD 2 | 21,442 | 7 |
| Titan | Original | 19,360 | 23 |
| Titan | Merged / LOD 0 | 19,360 | 23 |
| Titan | Medium / LOD 1 | 11,272 | 23 |
| Titan | Far / LOD 2 | 7,946 | 23 |
| Any ship | Distant sprite | 2 | 1 |

The Interceptor's far level reduces triangles by **76.9%**; the Cruiser's by **62.7%**; the Tiny Freighter's by **46.7%**; the Small Freighter's by **41.0%**; the Large Freighter's by **45.0%**; the Heavy Freighter's by **53.0%**; the Bulk Freighter's by **40.8%**; the Titan's by **59.0%**; the Corvette's by **66.1%**. The Cruiser keeps more protected structure and separate turrets. All five freighter sources already combine their static components into seven material groups, so their full-detail comparison has no further draw-call reduction. Medium and far versions reduce geometry while retaining those seven groups. Corvette and Titan also start consolidated: seven static hull groups and four groups per turret, for 23 draws at every mesh level.

Measured single-pass results for **80 ships**:

| Ship | Original draws | Merged draws | Original triangles | Far triangles |
|---|---:|---:|---:|---:|
| Interceptor | 3,120 | 480 | 576,640 | 133,120 |
| Corvette | 1,840 | 1,840 | 1,656,000 | 561,600 |
| Cruiser | 8,320 | 1,360 | 1,152,000 | 429,920 |
| Tiny Freighter | 560 | 560 | 544,320 | 289,920 |
| Small Freighter | 560 | 560 | 746,880 | 440,480 |
| Large Freighter | 560 | 560 | 1,161,920 | 638,720 |
| Heavy Freighter | 560 | 560 | 1,565,120 | 735,360 |
| Bulk Freighter | 560 | 560 | 2,896,000 | 1,715,360 |
| Titan | 1,840 | 1,840 | 1,548,800 | 635,680 |

Eighty sprites draw 160 triangles in 80 calls. These are actual browser renderer counters with all ships in view, not an FPS benchmark. The experiment does not use instancing, shadows, weapon effects, explosions, or battle UI. The 1/12/80 formations contain the selected hull. The separate **One of each ship** option compares all nine available models.

## Corvette, Cruiser, and Titan turrets

The Cruiser has **seven static hull material groups** and **five material groups per turret**, for 17 draws. Both twin-barrel turrets remain separate at every mesh LOD. Each is parented to a named pivot, `turret.port.pivot` or `turret.starboard.pivot`, around its original mount. A cancelling child translation preserves original vertex coordinates exactly at rest. Rotate the outer pivot about local Y to traverse a turret. The two weapon sockets on each turret are its descendants and move with it.

Corvette and Titan each have seven static hull groups and four material groups per turret. Their four mounts are `turret.port.1.pivot`, `turret.port.2.pivot`, `turret.starboard.1.pivot` and `turret.starboard.2.pivot`, with two muzzle sockets under each geometry frame. All three optimized mesh levels retain these pivots.

The **Turret traverse** slider demonstrates these pivots in the merged and lower-detail models. Port and starboard turn in opposite directions. Original and sprite views show the resting pose. This is transform-based articulation, with no skeletal rig, animation clips, recoil, or gun elevation. Pedestals are included in the rotating assemblies.

## Files

- `models/interceptor-{original,lod0,lod1,lod2}.glb`: original comparison copy and the three optimized Interceptor models.
- `models/corvette-{original,lod0,lod1,lod2}.glb`: source copy and three Corvette levels with four articulated turrets.
- `corvette-model-info.json`, `corvette-sprite-manifest.json`, `corvette-geometry-verification.json`, `corvette-render-verification.json`, `previews/corvette/`: Corvette metadata and validation evidence.
- `models/cruiser-{original,lod0,lod1,lod2}.glb`: original comparison copy and the three optimized Cruiser models.
- `models/tiny-freighter-{original,lod0,lod1,lod2}.glb`: original comparison copy and the three optimized Tiny Freighter models.
- `models/small-freighter-{original,lod0,lod1,lod2}.glb`: original comparison copy and the three optimized Small Freighter models.
- `models/large-freighter-{original,lod0,lod1,lod2}.glb`: original comparison copy and the three optimized Large Freighter models.
- `models/heavy-freighter-{original,lod0,lod1,lod2}.glb`: original comparison copy and the three optimized Heavy Freighter models.
- `models/bulk-freighter-{original,lod0,lod1,lod2}.glb`: original comparison copy and the three optimized Bulk Freighter models.
- `models/titan-{original,lod0,lod1,lod2}.glb`: source copy and three Titan levels with four articulated turrets.
- `sprites/{interceptor,cruiser,tiny-freighter,small-freighter,large-freighter,heavy-freighter,bulk-freighter,titan,corvette}-views.png`: transparent 2048 × 1280 atlases; forty 256 × 256 views each.
- `sprite-manifest.json`, `cruiser-sprite-manifest.json`, `tiny-freighter-sprite-manifest.json`, `small-freighter-sprite-manifest.json`, `large-freighter-sprite-manifest.json`, `heavy-freighter-sprite-manifest.json`, `bulk-freighter-sprite-manifest.json`, `titan-sprite-manifest.json`: tile angles, model center, and quad size.
- `model-info.json`, `cruiser-model-info.json`, `tiny-freighter-model-info.json`, `small-freighter-model-info.json`, `large-freighter-model-info.json`, `heavy-freighter-model-info.json`, `bulk-freighter-model-info.json`, `titan-model-info.json`: counts, hashes, materials, LOD policy, and turret pivot definitions.
- `previews/comparison.png`, `previews/cruiser/comparison.png`, `previews/tiny-freighter/comparison.png`, `previews/small-freighter/comparison.png`, `previews/large-freighter/comparison.png`, `previews/heavy-freighter/comparison.png`, `previews/bulk-freighter/comparison.png`, `previews/titan/comparison.png`: matched upper and lower camera comparisons.
- `geometry-verification.json`, `cruiser-geometry-verification.json`, `tiny-freighter-geometry-verification.json`, `small-freighter-geometry-verification.json`, `large-freighter-geometry-verification.json`, `heavy-freighter-geometry-verification.json`, `bulk-freighter-geometry-verification.json`, `titan-geometry-verification.json`: independent geometry, texture, material, bounds, protected-surface, and socket checks.
- `validation.json`: Khronos validation reports for all twenty-seven optimized GLBs.
- `render-verification.json`, `cruiser-render-verification.json`, `tiny-freighter-render-verification.json`, `small-freighter-render-verification.json`, `large-freighter-render-verification.json`, `heavy-freighter-render-verification.json`, `bulk-freighter-render-verification.json`, `titan-render-verification.json`: actual browser counters, LOD switching, and turret articulation checks.
- `build_models.mjs`, `build_preview.mjs`, `viewer.js`, `viewer-shell.html`: editable build and preview source.

## Preservation and simplification

Static pieces sharing the same material are merged; Corvette, Cruiser, and Titan turret assemblies are partitioned separately. Exact duplicate vertices are welded, indices are reordered for cache locality, and 16-bit indices replace 32-bit indices where possible. Full-detail models retain every original oriented triangle, position, normal, UV, material definition, and embedded image byte. The checked renders match over 99.998% of Interceptor pixels, 99.98% of Cruiser pixels, and 100% of Tiny, Small, Large, Heavy, Bulk Freighter, Titan, and Corvette pixels. Tiny raster differences can occur after draw ordering and scene hierarchy change, while the underlying geometry is identical.

Medium and far levels use meshoptimizer's attribute-aware simplifier separately on each physical component. Open borders and material boundaries remain protected; each component has a minimum budget so a small engine or gun housing is not entirely removed. Simplification selects existing vertices without relocating them.

The Interceptor retains its exact fuselage, canopy, swept wings, root armor, wingtip endplates, and wingtip lights. Its fixed guns are part of the static hull. The Cruiser retains its exact main hull and dedicated belly, spine, prow and shoulder armor, recessed bays, docked shuttles, command bridge, ventral radiators, and turret housings/pedestals. The Tiny Freighter retains its keel, both cargo containers and end panels, loading panels, ventral hatches, forward cabin, sensor fairings, and central drive-service fairing. Its authored component ranges are recovered from the already-merged source before simplifying engine hardware and secondary fittings. The original bottom textures remain in every mesh level. The Small Freighter retains its exact central spine, four cargo containers and their panels/hatches, forward cabin and sensor fairings, drive fairing, X-braced cargo connections, and engine yoke. Its component ranges are recovered before simplifying fittings and engine hardware. The Large Freighter retains both keels and bows, six cargo modules and their hatches, front trusses, rear crossbeam, command cabin, reactor fairing and engine mounts. Its open center channel and dedicated underside are preserved; secondary fittings and round nozzle hardware are simplified. The Heavy Freighter retains its continuous armored pressure hull, four cargo hatches and locks, spine and braces, command prow and sensor fairings, folded crane arms and mounting blocks, reactor fairing, and eight-engine support. Round nozzle hardware and secondary hydraulic fittings are simplified; its dedicated bottom mapping is preserved. The Bulk Freighter retains all sixteen cargo modules, its open lattice spine and sixteen attachment trusses, dorsal and ventral service housings, command pod, reactor and eight-engine support. Rounded restraints and secondary engine fittings are simplified; its cargo undersides and negative space remain intact. The Corvette retains its closed pressure hull, broad shoulders, narrow aft spine, bridge, dedicated belly and maintenance plates, twin engine housings and cowls, engine-root plates and all four turret housings. Round nozzle hardware, gun barrels and secondary fittings are simplified. The Titan retains its layered pressure hull, deck and shoulder armor, keel and lower maintenance panels, command blister, stern fairing, both hollow engine blocks, sidewall frames and all four turret housings. Rounded nozzle hardware, gun barrels and fittings are simplified. The Corvette's, Cruiser's, Titan's and all five freighters' overall bounds are unchanged; the Interceptor's lower engine bound shifts by 0.01 units at reduced detail, less than 0.1% of ship length.

All original attachment sockets survive. All nine experiment source GLB hashes are checked independently. Each ship shares its own materials and texture objects across its LOD variants, but no material is shared across different ship types.

## Automatic detail and sprites

The viewer uses projected bounding-sphere diameter in **CSS pixels**:

```text
diameterPx = 2 × radius × viewportHeight / (2 × tan(verticalFOV / 2) × cameraSpaceDepth)

> 230 px      full detail
100–230 px    medium
38–100 px     far
< 38 px       sprite
```

A 12% hysteresis band prevents threshold flicker. These are experimental starting values, not final game settings. Levels are selected independently per ship. The GLBs themselves do not switch; engine code must implement the policy.

Each sprite atlas is rendered from the full-detail model at eight azimuths and elevations −90°, −45°, 0°, 45°, and 90°, including the underside. The viewer chooses the nearest direction and draws a camera-facing quad. Lighting is baked, angles are discrete, and depth is flat, so sprites are intended for tiny ships. Tone mapping is disabled on the sprite material because it was applied during baking. Corvette, Cruiser, and Titan sprites use resting turrets. Integrating rotating or banking battle ships would require selecting sprite direction in the ship's local frame and handling roll.

Geometry is shared between copies. Standalone GLBs each embed their textures for portability; the comparison viewer also embeds original models and all nine atlases, so its download size is deliberately larger than a production scene. Actual target-device performance, texture memory, and battle effects still require profiling.

## Validation and rebuild

`lineup-verification.json` records 71 passing browser checks for the comparison: exact rendered costs at both scales and every representation, measured world-space length ratios, all eight names in view, independent LOD choices and textures, wireframe, Corvette, Cruiser, and Titan turrets, top/bottom/orbit views, and regression checks for every single model plus the 12/80 formations. Open `?formation=all&capture-lineup=1` to rerun this check and save `previews/lineup-game.png` and `previews/lineup-native.png` through the local preview server. Adding Corvette preserves the 32 preexisting experiment GLBs and eight preexisting sprite atlases byte for byte; nine earlier source GLBs are also hash checked (49 prior assets total).

All twenty-seven optimized GLBs pass Khronos glTF Validator with **zero errors and zero warnings**. Informational notes concern retained socket nodes, unused UVs on solid-color materials, and the original Interceptor's non-power-of-two textures. Geometry and render checks also cover automatic levels, hysteresis, independent Corvette, Cruiser, and Titan pivots, and weapon sockets following their turrets. The Corvette, Titan, and all five freighter checks additionally confirm no collapsed geometry or textured UV triangles, exact original vertex attributes, dedicated bottom-view UV placement, and expected counts for every representation in an 80-ship formation.

Requires Node.js, Python, NumPy, and Pillow. Builders read the original sibling folders when available and fall back to the included original GLBs for standalone rebuilding.

```sh
npm install
npm run models
npm run build
npm run validate
python3 preview_server.py
```

To regenerate one ship only, pass `interceptor`, `corvette`, `cruiser`, `tiny-freighter`, `small-freighter`, `large-freighter`, `heavy-freighter`, `bulk-freighter`, or `titan` to `node build_models.mjs`. To rebake and capture, open the following URLs one at a time:

```text
http://127.0.0.1:8770/viewer.html?ship=interceptor&bake=1&capture=1
http://127.0.0.1:8770/viewer.html?ship=corvette&bake=1&capture=1
http://127.0.0.1:8770/viewer.html?ship=cruiser&bake=1&capture=1
http://127.0.0.1:8770/viewer.html?ship=tiny-freighter&bake=1&capture=1
http://127.0.0.1:8770/viewer.html?ship=small-freighter&bake=1&capture=1
http://127.0.0.1:8770/viewer.html?ship=large-freighter&bake=1&capture=1
http://127.0.0.1:8770/viewer.html?ship=heavy-freighter&bake=1&capture=1
http://127.0.0.1:8770/viewer.html?ship=bulk-freighter&bake=1&capture=1
http://127.0.0.1:8770/viewer.html?ship=titan&bake=1&capture=1
```

Then run `npm run build` again to embed the atlases, followed by `python3 verify_geometry.py interceptor`, `python3 verify_geometry.py corvette`, `python3 verify_geometry.py cruiser`, `python3 verify_geometry.py tiny-freighter`, `python3 verify_geometry.py small-freighter`, `python3 verify_geometry.py large-freighter`, `python3 verify_geometry.py heavy-freighter`, `python3 verify_geometry.py bulk-freighter`, and `python3 verify_geometry.py titan`. The capture endpoint is loopback-only and accepts only named experiment outputs. `preview_server.py --port 8780` can use another local port if necessary.

References: [meshoptimizer simplification](https://github.com/zeux/meshoptimizer/blob/master/js/README.md#simplifier), [Three.js level of detail](https://threejs.org/docs/pages/LOD.html). Runtime and simplifier license notices are included.
