# Authority Nexus — Wormhole Anchor

A low-polygon, reference-based reconstruction of the approved Authority Nexus.
Built with Python/NumPy and exported directly to glTF 2.0; no Blender was used.

## Open the model

- `viewer.html` is a self-contained offline Three.js viewer. Open it in a WebGL-capable browser; no server or internet connection is required. Drag to orbit, scroll to zoom, and use the view buttons to inspect the underside and elevations.
- The wormhole flows automatically: a slow violet swirl, inward-travelling energy ripples and subtle cyan highlights. Toggle **Wormhole motion** to pause or resume it. Reduced-motion preferences start it paused; hidden tabs and a closed portal stop the continuous animation loop.
- **Ship traffic** adds a repeating stream of Interceptors and Tiny, Small and Medium Freighters, all tiny beside the station, with short cyan engine wakes and small arrival ripples. They approach between the stabilizers and descend into the vortex. Toggle the button to hide/show and pause/resume traffic. Reduced-motion preferences start traffic off.
- **Attack disabled** switches to the damaged station and locks containment off. Toggle it again to compare the operational version. `viewer.html?state=disabled` opens directly in the disabled state; **Download GLB** downloads whichever version is selected.
- `authority-nexus.glb` contains geometry, ten embedded texture maps, material definitions and the `WormholeCollapse` animation.
- `authority-nexus.obj` and `authority-nexus.mtl` are editable interchange exports. Keep `textures/` beside them. OBJ does not include the animation.
- `authority-nexus-model-sheet.png` and `previews/` show software renders of the actual exported mesh, including its underside and the portal switched off.

## Geometry and materials

The model contains 11,440 triangles in 28 material sections. Static parts are combined by material inside each assembly; the four stabilizers remain separately addressable. Exact duplicate vertices are consolidated, with 16-bit indices where possible.

The scene uses Y up and -Z forward. The outer diameter is approximately 19.8 relative units, not an asserted in-game size. Overall bounds are recorded in `model-info.json`.

Color and emissive detail for the terraces, port architecture and underside is projected from the five-view reference. All eight pylon shells use a dedicated ivory armor texture, with local UV coordinates covering both broad sides, the edges and caps. The panels include fine seams, fasteners, subtle wear and restrained brass inlay. A matching roughness/metallic map gives the armor and inlays different finishes. Ivory fittings, crowns and deck edges use local planar or cylindrical UVs. Graphite supports, conduit housings and mounts reuse the panel texture with a dark material tint. Brass fittings, pipes and the inner lip reuse the machined-brass maps. These material instances add no texture images or render sections. Painted shadows from the concept remain in the hull color texture; this is an efficient concept reconstruction, not a scan or a physically calibrated material set.

An exhaustive texture audit also found collapsed reference projections on port caps and radiator edges. Those perpendicular faces now have local graphite-panel mapping. Every physical surface in both exports has an embedded color texture and noncollapsed UVs, including concealed caps. The damaged reactor cap and exposed impact ribs reuse the scorched armor texture with a titanium tint. The free containment energy and seven small emergency lamps intentionally use uniform emissive colors. `texture-audit-before.json` records the original findings and geometry digests; `texture-validation.json` records the corrected coverage. All triangle positions and normals remain identical, with unchanged triangle counts, render sections and texture-image counts.

The tower texture adds no geometry. `tower-texture-generation.json` records the exact prompt used with the built-in ImageGen tool in new-image mode. The generated 1254 × 1254 armor texture is saved in `textures/authority-nexus-ivory-panels.png` and is retained as a build input.

The broad gold ring around the underside opening (`hull.ventral-collar`) uses `textures/authority-nexus-brass-ring.png`, a 2172 × 724 machined-brass panel texture, plus a derived metallic/roughness map. It covers the bottom and top faces and both cylindrical walls, with eight circumferential repeats and continuous UVs at the angular seam. The same material textures all 24 gold reactor rings and 32 feed couplings. These smaller cylinders use planar UVs on both end faces and continuous angular UVs on their outer walls. The viewer enables anisotropic filtering on the brass textures for oblique views.

The four tall blue reactor cores, 32 narrow power feeds and four small beacons use `textures/authority-nexus-power-cells.png`, a 724 × 2172 blue energy-cell texture. Local cylindrical UVs wrap the filaments, segmented cells and dark separators around the hardware. Derived metallic/roughness and selective emissive maps preserve surface detail, and the viewer dims their emission with containment power. Gold collars share one material and all cyan hardware in each stabilizer shares another, keeping the model at 28 sections. All triangle positions, normals and the 11,440-triangle count are unchanged. `previews/power-core-detail.png` isolates one actual exported core and its six rings for inspection.

`ring-texture-generation.json` and `power-texture-generation.json` record the final prompts and the built-in ImageGen/new-image mode used to create the brass and blue cell textures. All three generated albedo images are retained as build inputs inside `textures/`.

## Containment behavior

Named scene assemblies:

- `structure`: terraces, physical containment collar, port halls and lower hull.
- `stabilizer-1` through `stabilizer-4`: separate pylon shells, coils, feeds and emitter jaws.
- `containment-field`: independent luminous rim and short field tethers.
- `wormhole`: a two-sided curved energy membrane; the sole intentionally open mesh surface.

The operational model's embedded 2.7-second `WormholeCollapse` animation contracts the energy membrane and field to zero. The static collar has a genuine central opening with no hidden floor. The viewer also dims the cyan emission as power falls. Static attack damage is available in the separate variant below. Game destruction and repair logic have not been integrated into the running game.

## Attack-disabled variant

`disabled/authority-nexus-disabled.glb` is a separate repairable damaged-state model, with matching OBJ/MTL exports and eleven embedded texture maps. It has 9,856 triangles in 32 sections and 368 closed solid components. Its origin, orientation and overall bounds match the operational station. Stabilizer 2 has a jagged, capped fracture and a shortened reactor core; two impact recesses expose damaged machinery and torn armor. Ivory armor and brass rings have scorch marks and chipped surfaces. Reactor cells are dark and cracked, with zero emission. Seven small amber markers remain on reserve power.

The wormhole membrane and containment-field meshes are absent from this export, leaving a real opening. It has no reopening animation. Its damaged textures, material masks and modeling inputs are in `disabled/textures/`; `disabled/texture-generation.json` records every final prompt and file path. The built-in ImageGen tool edited the ivory, brass and power-cell albedos and generated the impact texture; PBR masks are derived by the model builder.

`nexus-state.js` drives both viewer variants with the real Three.js animation mixer and wormhole/traffic effects. While damage is selected, attempts to raise containment power cannot reopen the passage; cargo diverts to holding positions. Opening the disabled URL starts with traffic already holding, including when reduced-motion preferences initially hide it. Switching back restores the previous operational power setting. Emergency lights retain their emission. The continuous render loop stops once the disabled traffic settles unless manual rotation is enabled.

The viewer embeds both station models and all four traffic models, making it approximately 63 MB. Only the selected station is visible and rendered. Both exports include the texture-coverage repairs; their geometry and normals remain unchanged. `disabled/previews/` and `disabled/authority-nexus-disabled-model-sheet.png` show software renders of the actual geometry.

## Living vortex effect

`wormhole-effect.js` is the reusable Three.js runtime effect, bundled into the offline viewer. It reuses the portal's reference texture, adds no meshes or draw calls, keeps the outer rim anchored, and renders from both sides. UV warping and procedural filaments run in the fragment shader. The station remains at 11,440 triangles and 28 sections. The fragment shader adds some GPU work inside the visible aperture.

The flowing surface is a runtime shader, not a glTF animation channel. The downloaded GLB retains its original static portal material and its existing `WormholeCollapse` clip. To reproduce the living vortex in the game, attach the included effect after loading the GLB:

```js
import { createWormholeEffect } from './wormhole-effect.js';
const anchor = gltf.scene.getObjectByName('wormhole');
const flow = createWormholeEffect(anchor);
// In the render loop; pause calls while the page is hidden:
flow.update(deltaSeconds);
// Alongside scrubbing the WormholeCollapse animation:
flow.setPower(containmentPower); // 0..1
// Optional controls and cleanup:
flow.setPlaying(false);
flow.dispose(); // Restores the original material; leaves shared textures alive.
```

## Small ship traffic

`wormhole-traffic.js` reuses the game's approved LOD2 Interceptor, Tiny Freighter, Small Freighter and Medium Freighter assets, copied byte-for-byte into `traffic-models/`. Their lengths are 0.17, 0.25, 0.30 and 0.36 units against the station's 19.8-unit diameter: approximately 0.9%–1.8% of its width. This is a visual scale study, not an asserted in-game physical size.

Ten staggered traffic slots contain three Interceptors, three Tiny Freighters, two Small Freighters and two Medium Freighters. Up to seven ships are visible during steady arrivals; restoring containment can briefly show all ten as the holding traffic resumes. A ship arrives every 3.6 seconds, follows a curved approach lasting about 26 seconds, contracts into the event horizon, then returns after an offscreen interval. Ships vanish at entry from every viewing angle instead of reappearing below the membrane. Static ship pieces are merged by material and instanced: the traffic adds 29 render sections and at most 44,034 submitted triangles, including the wakes and entry ripples. The station itself stays at 11,440 triangles and 28 sections. The sidebar counters explicitly refer to the station.

Below 94% containment power, arriving ships divert upward to separate holding positions outside the aperture; new arrivals wait. Staggered approach and holding lanes keep the larger freighters apart as they resume. Once they settle, traffic stops requesting animation frames. Restoring power resumes the stream. Traffic pauses in hidden tabs and remains independent of the **Wormhole motion** toggle. These are decorative viewer behaviors; the game simulation and the downloadable station GLB are unchanged.

The offline viewer embeds all four ship models, using `traffic-models/manifest.json` as its asset list. For reuse, pass loaded scenes to `createWormholeTraffic` under the keys `interceptor`, `tiny-freighter`, `small-freighter` and `medium-freighter`. Add its `.group` under the station, call `.update(deltaSeconds)` from the render loop and `.setPower(containmentPower)` alongside the collapse animation. `.setEnabled(false)` hides and pauses traffic. `.dispose()` releases the merged geometry and cloned materials while leaving the source models and their textures alive.

## Validation

- `audit_textures.py` and `texture-validation.json`: checks every exported triangle in both variants for an embedded albedo, finite noncollapsed UVs and valid sampler coordinates. Interior samples confirm color variation on every textured component. All 9,744 operational hardware triangles and 9,716 disabled hardware triangles are textured. The portal has another 704 textured triangles. Uniform energy and emergency-light surfaces are explicitly identified. Per-component triangle/normal digests match the original exports. `previews/texture-coverage-detail.png` shows actual mesh close-ups of the repaired underside, crown, damaged core cap and emitter hardware.
- `gltf-validation.json`: Khronos glTF Validator, zero errors and warnings.
- `geometry-validation.json`: independent checks on exported buffers. All 391 solid components are closed, with positive signed volumes; normals and winding are consistent, attributes finite, UVs valid for their sampler and triangles nondegenerate. All eight tower shells, 256 underside collar triangles, 40 blue cylinders, 24 reactor rings and 32 feed couplings have textures and noncollapsed UVs. Cylindrical seams are checked for stretching, and blue cylinders have sampled emissive detail. The underside collar uses repeating horizontal UVs from 0 to 8. The energy membrane has an intentional circular boundary.
- `inspect_model.py`: renders eight viewing directions plus a portal-off view from the exported GLB.
- `test_asset.mjs`: runs the exported animation through Three.js's real animation mixer, including repeated close/reopen cycles, and verifies that the offline viewer embeds the current model and reference.
- `test_wormhole.mjs`: checks pause/resume, closed-portal suspension, restoration, bounded elapsed time, reduced-motion initialization, geometry/material isolation and shared-texture cleanup with real Three.js objects.
- `wormhole-validation.json` and `previews/wormhole-flow-check.png`: the actual vertex/fragment shaders were compiled, linked and rendered with native offscreen OpenGL, the exported portal triangles, the sRGB atlas and Three.js tone/color chunks. Frames at 0, 2, 4 and 6 seconds change visibly; the long-period wrap differs by at most one 8-bit color value and zero power renders black.
- `test_traffic.mjs` and `traffic-validation.json`: real GLTFLoader geometry/material parsing (with Node image-decoding placeholders), 50 arrivals, pause/restore, containment diversion/holding and 3,695 clearance samples against the exported station solids. The closest sampled ship clearance is about 0.65 units. Another 125,202 pairwise checks use bounding spheres derived from the real hulls, including their portal-entry shrink, and retain at least 0.43 units between hull envelopes while approaching, diverting, holding and resuming. The offline payload test verifies all four embedded ship GLBs against the packaged files.
- `inspect_traffic.py` renders the actual instanced ship geometry, textures and poses at station scale in `previews/traffic-hero.png` and `previews/traffic-top.png`. These inspection images use the static portal material and approximate transparent wake blending; they are not browser screenshots.
- `disabled/gltf-validation.json` and `disabled/geometry-validation.json`: zero glTF errors/warnings, closed solids, consistent normals, finite attributes, shortened fractured tower, unobstructed central opening, absent energy surfaces and zero reactor emission.
- `test_disabled.mjs` and `disabled/runtime-validation.json`: real loaded models and the same state controller used by the viewer. Three comparison cycles verify the power lock, restoration, emergency lights and still initial holding state. Another 5,200 ray samples check diverted traffic against the damaged hull; minimum sampled clearance is about 0.97 units. `test_asset.mjs` verifies both embedded station files and the disabled preview against the saved exports.

The software model renders and native GPU shader frames were inspected. The WebGL interface was built and its payload checked, but was not exercised in a browser because browser access was unavailable for this session.

## Rebuild

Install Python dependencies from `requirements.txt` and Node dependencies with `npm install`. Then run:

```sh
python3 build_model.py
python3 inspect_model.py
python3 build_disabled.py
python3 inspect_disabled.py
python3 audit_textures.py --render
npm run build:viewer
npm run validate
node validate.mjs disabled/authority-nexus-disabled.glb
npm test
python3 inspect_traffic.py
```

In the project checkout, the builder uses the approved references in `../../authority-nexus-reference-v1/` and `../../authority-nexus-anchor-v1/`. The included `reference.png` and `concept.png` are used as fallbacks when those project folders are absent.

Three.js is distributed under its included MIT license (`THREE-LICENSE.txt`). The offline viewer bundles it locally.
