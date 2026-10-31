# Wormhole Hub

A textured polygon model reconstructed from the approved front artwork and six-view concept sheet. The circular frame, six tower spires, two docking arms, emitter bridges and axial supports are actual three-dimensional geometry. The galaxy is mapped onto a separate, double-sided energy membrane with a recessed center.

## Use the model

- **wormhole-hub.glb** — recommended import for Blender, Three.js, Unity or another glTF-compatible tool. Geometry, eight texture maps, materials and the `PortalClose` animation are embedded in this one file.
- **viewer.html** — self-contained interactive viewer. Open it in a WebGL-capable browser. Drag to orbit, scroll to zoom, and use the orthographic camera buttons. Portal, Flow, Traffic, Rotate, Wireframe and Clay controls help inspect the scene. The viewer works offline, including the freighter traffic.
- **wormhole-hub.obj**, **wormhole-hub.mtl** and **textures/** — editable interchange export with individually named source components. Keep the texture folder beside the OBJ and MTL. OBJ does not retain the GLB animation or all glTF material properties.
- **reference-sheet.png** — the modeling reference supplied for this reconstruction.
- **previews/** — renders of the actual exported geometry from all six orthographic directions, two oblique directions, and with the portal hidden.

## Geometry and scale

33,000 triangles, 10 render sections, 813 source components and nine materials. Static hardware is merged by material for rendering; named source-component ranges remain in GLB extras, and the OBJ preserves individual objects.

The model uses Y up, with its circular ring in the XY plane. Front faces +Z and the travel axis runs along Z. Overall dimensions are approximately 17.10 × 17.34 × 2.47 relative units, including the energy funnel. The hardware itself is 1.73 units deep. Ring outer diameter is 10.42; the unobstructed central passage is approximately 7.21 units wide. Physical size is unspecified.

Named GLB assemblies are `station-hardware`, `containment-field` and `wormhole`. Hiding the latter two reveals a real central opening. The three-second `PortalClose` animation shrinks the membrane within its plane. The membrane curves back 1.60 units from its anchored rim, with 32 radial bands and a subtle blue vertex tint toward the core. Both GLB and OBJ contain the curved geometry; the GLB also retains the depth tint. `portal-profile.json` keeps the geometry builder and freighter routes aligned.

The viewer's flowing effect is a runtime shader in `wormhole-flow.js`; the standalone GLB retains the static galaxy texture and closing animation. The galaxy rotates at a uniform rate with a bounded oscillation of at most 0.10 radians. The rim stays fixed through a separate blend. Radial variation never multiplies elapsed time, so the spiral cannot wind progressively tighter. The animation clock wraps at a common period without a visible jump.

The original front artwork defines the circular proportions. Depth, hidden surfaces and small construction details are interpreted from the concept sheet. This is a reference reconstruction with simplified geometric detail. Some fine front detail and lighting remain painted in the projected reference texture.

## Freighter traffic

Nine staggered traffic slots use the project's approved Tiny, Small and Medium Freighters, with three of each and up to seven visible at once. Their lengths are 0.25, 0.32 and 0.40 units beside the station's 17.10-unit span. A new arrival starts every four seconds. Short cyan engine wakes and small entry ripples mark their passage.

The traffic system is adapted from the existing Authority Nexus viewer for this hub's vertical XY ring and Z-axis passage. Freighters curve in from the front, stay clear of the towers and docking arms, and descend into the funnel before contracting at the energy membrane so they cannot emerge from the rear. Entry ripples follow the curved surface. Turning **Portal** off diverts them to separate holding positions. Reopening resumes arrivals. **Traffic** independently hides and pauses the ships. Traffic continues when the portal's **Flow** animation is paused. Hidden browser tabs pause the render loop; reduced-motion preferences initially disable Flow and Traffic until their controls are selected.

`wormhole-traffic.js` contains the reusable runtime effect; `traffic-models/` contains the unchanged freighter GLBs and their source hashes. Both are packaged with the viewer. The station GLB contains the hub geometry and `PortalClose` animation. Import the traffic module alongside the station when using the freighters in another Three.js scene:

```js
const traffic = createWormholeTraffic({
  'tiny-freighter': tinyScene,
  'small-freighter': smallScene,
  'medium-freighter': mediumScene,
});
scene.add(traffic.group);
traffic.update(deltaSeconds);  // In the render loop.
traffic.setPower(portalOpen ? 1 : 0);
traffic.setEnabled(showTraffic);
// On teardown: traffic.dispose();
```

The traffic uses 23 instanced render sections and at most 52,713 submitted triangles. The viewer's triangle counter describes the station. All three ship files are embedded in the approximately 32.2 MiB offline viewer.

## Materials and source assets

`textures/reference-front.png` is an unchanged copy of the approved front image, sampled through UV coordinates for the armor and portal. The membrane adds a vertex tint without changing the source image. Side faces have local, noncollapsed UVs. Titanium, brass and reactor texture maps are copied from the existing project's `authority-nexus-textured-v1` material library; all required images are included here. The source images are not cropped or overwritten.

## Verification

The Khronos glTF validator reports zero errors and zero warnings. Independent checks on exported buffers verify finite positions, unit normals, consistent face winding, nondegenerate triangles, noncollapsed UVs, and 812 closed solids with positive volume. The membrane is the only intentionally open surface. The passage clearance, six spires and two docking arms are also checked. Results are saved in `gltf-validation.json` and `geometry-validation.json`.

The browser viewer was also inspected with textured and clay materials, the portal hidden, the left profile, wireframe enabled, and the flowing portal enabled. No browser rendering errors or warnings were reported. `previews/viewer.jpg` records the current interactive view. `previews/portal-depth.jpg` shows the curved membrane with flow and freighters enabled, and `previews/portal-profile.jpg` shows its physical depth from the side.

`test_flow.mjs` samples the actual GLSL rotation and brightness expressions at elapsed times from zero through 24 hours. It verifies bounded radial stretching and continuity at the animation wrap; results are in `flow-validation.json`.

`test_traffic.mjs` loads the real ship and hub meshes through Three.js, verifies 45 recurring arrivals, checks 4,155 station-clearance samples and 98,302 ship-separation samples, and exercises portal closure, holding, reopening, pause and disposal. Minimum sampled hardware clearance is 1.0858 units; minimum hull-to-hull gap is 0.2116 units. Another 45,225 samples check against the exported curved membrane, with a minimum clearance of 0.0463 units before each ship is consumed. It also verifies that the offline viewer embeds the current station and all three packaged freighters. Results are in `traffic-validation.json`. `previews/traffic-viewer.jpg` shows the browser preview with freighters enabled.

## Rebuild

Python dependencies are listed in `requirements.txt`. Node dependencies are in `package.json`; inside this checkout the viewer builder and validator can also use the existing shared dependencies.

```sh
python3 build_model.py
python3 inspect_model.py
node build_viewer.mjs
node validate.mjs
npm test
```

All modeling inputs are included. The rebuild uses the local copies once present. The viewer bundles Three.js under its included MIT license, `THREE-LICENSE.txt`.
