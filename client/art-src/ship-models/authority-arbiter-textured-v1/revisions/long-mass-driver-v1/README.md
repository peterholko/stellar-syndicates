# Authority Arbiter — ivory and dark imperial variants

Textured 3D enforcement capital ship for Stellar Syndicates, reconstructed from
the approved Authority Arbiter reference. Built with Python/NumPy; no Blender.

## Review and assets

- `viewer.html`: self-contained interactive Three.js preview, opening in dark
  armor. Switch between **Dark armor** and **Ivory armor** at the same camera
  position. **Galaxy backdrop** and **Low light** controls compare readability.
  Orbit, zoom,
  five orthographic views, underside view, spine and mass-driver close-ups,
  turret motion, texture inspection,
  wireframe, PNG capture and GLB download. No external network assets.
- `authority-arbiter-dark.glb`: dark imperial variant; complete portable glTF
  materials, embedded textures and standard vertex colors.
- `authority-arbiter.glb`: preserved ivory version; textures and materials embedded.
- `authority-arbiter.obj` and `.mtl`: static interchange export; keep `textures/`
  alongside it. Use the GLB for PBR materials and movable gun assemblies.
- `previews/three-quarter.png`: rendered from the actual exported GLB.
- `previews/contact-sheet.png`: eight rendered inspection views.
- `previews/spines-detail.png`: the three rebuilt central ridges.
- `previews/mass-driver-detail.png`: the new main weapon and its bow mounting.
- `model-info.json`: dimensions, feature counts, gun assemblies and sockets.
- `dark-model-info.json`: dark palette and the matching asset specification.
- `previews/dark-galaxy.png`: dark variant on a near-black space backdrop.
- `previews/ivory-dark-comparison.png`: same geometry, view and illumination.
- `previews/dark-map-readability.png`: top views at 256 and 128 pixels, plus
  reduced-light top and bottom views.

## Dark armor study

The dark finish uses blue-charcoal armor, lighter nickel bevels and structural
edges, antique-gold crests, warm amber bridge windows and sparse cyan lights.
The broad armor remains a visible midtone; dark recesses supply depth. Gold
trim follows the halo, spine crests and weapon housing to define the outline.

`build_dark_variant.py` applies linear glTF `COLOR_0` material multipliers to
the existing model, plus a small roughness adjustment. The three original PNG
maps remain byte-identical. UVs, positions, normals, indices, all sockets and
the node hierarchy are retained. Both finishes have **20,148 triangles and 26
render sections**. This finish survives a normal GLTFLoader import and does
not require a custom shader. The OBJ export remains the ivory version.

The viewer uses the current game's ship-model lighting parameters and its
galaxy background base color, `#05070d`. Its sparse starfield is a preview
backdrop, not a screenshot of a running game. **Low light** reduces direct,
ambient and environment lighting to 40%; it does not dim the existing lamps.
There is no added bloom, selection outline or artificial glowing armor.

Software renders verify top, rear and underside coverage and show smaller map
sizes. Their silhouette contrast measurements are recorded in
`dark-surface-verification.json`; these describe the inspection setup, not a
guarantee for every possible in-game background or lighting condition.

## Design

The later requests for a more imperial appearance and additional weapons are
incorporated into the model. A stepped command citadel, tall narrow bridge,
ivory buttresses, warm amber windows, gold cornices and armored broadside
galleries distinguish it from the player fleet.

Three unequal parallel spines are rebuilt as sculpted navy cores with four
separate ceramic armor segments each, beveled flanks, narrow gold crests,
root buttresses and small status lights. The right spine is longest; the left
is shortest. The central ridge arrangement preserves the Authority silhouette.

The original bow lance is replaced by one very large mass driver. The new
weapon has a reinforced breech, recoil sleeves, two armored rail casings,
two physical acceleration rails and one open rectangular launch bore.
The muzzle has a dark mechanical interior and a deep closed breech. It is
a kinetic-weapon design; there is no glowing beam-emitter lens at the muzzle.
The single horizontal halo, four struts, split prow, four-engine bank and
closed ventral boarding hangar remain.

| Armament | Mounts | Barrels |
| --- | ---: | ---: |
| Heavy twin turrets | 6 | 12 |
| Secondary twin batteries | 8 | 16 |
| Paired point defense | 8 | 16 |
| Capital mass driver | 1 | 1 |

The secondary batteries and point defense were added after the five-view sheet
was approved. Point defense includes four dorsal and four ventral mounts.
All 22 gun mounts have independent yaw pivots. The viewer's optional motion is
a mechanical preview; targeting, firing and interdiction behavior are not wired
into the game.

## Asset specification

- **20,148 triangles**, 26 mesh/render sections, two PBR materials.
- Static geometry merged by material; separate halo and 22 gun assemblies.
- 16-bit indices; no transparent billboards or planar stand-ins for hull parts.
- Three embedded 2048 × 2048 maps: base color, metallic/roughness and emission.
- Every physical triangle has nonzero-area UVs, including side walls, bevels,
  engine caps, halo edges and the underside. Ventral armor uses its dedicated
  reference drawing.
- 402 closed, positively oriented geometric components. Components may overlap
  at manufactured joints; this is a game asset, not a single watertight print mesh.
- Y up, −Z forward. Bounds are approximately 13.72 wide × 6.91 high × 27.4397 long.
  Units are relative; gameplay scale is assigned when integrating the asset.
- 73 named sockets: 44 gun muzzles, 22 gun pivots, four engines, one mass driver,
  one command position and one interdiction center.

Weapon sockets are children of their gun assembly in the GLB and move with it.
GLB socket translations are local to their parent. `model-info.json` stores
their rest-pose model-space coordinates. Socket direction vectors are in the
parent's local frame. Rotate the named assembly around its local Y axis for yaw.
No skeletal rig or firing animation is embedded.

The main-weapon socket is now `socket.mass-driver.muzzle`, with role
`capital-mass-driver`. The previous `socket.lance.tip` has been removed.

## Texture provenance

The approved source sheet is
`../../authority-arbiter-reference-v1/authority-arbiter-reference.png`.
The original sprite is copied to `concept.png`. The reference itself is preserved
unchanged as `textures/reference-sheet.png`.

Plan-facing armor is UV mapped to the top and bottom reference drawings.
Exposed walls and new weapons use the existing Authority Nexus ivory panel and
machined brass textures at consistent surface density. These source maps are
copied into `textures/`; navy structure is a material tint of the ivory panels.
Atlas packing, edge padding, material masks and emission extraction are
deterministic build steps. No new reference illustration was generated here.

## Rebuild and validation

From this directory, with Python dependencies from `requirements.txt` installed:

```sh
python3 build_model.py
python3 build_dark_variant.py
python3 inspect_model.py
python3 inspect_dark_variant.py
node build_preview.mjs
node validate.mjs
node validate.mjs authority-arbiter-dark.glb
node test_asset.mjs
node test_asset.mjs --dark
```

The JavaScript scripts use local dependencies or the existing
`../pirate-fleet-v1/node_modules` dependency set. `package.json` pins the versions
needed to install this folder independently.

Validation outputs:

- `gltf-validation.json`: Khronos glTF validation, **0 errors and 0 warnings**.
- `surface-verification.json`: independent exported-buffer checks for UVs,
  normals, winding, positive closed components, feature counts, open halo gaps,
  texture presence, engine alignment, unobstructed cyan exhausts, the open mass
  driver bore, four armor segments on each staggered spine, and exact preservation
  of geometry outside the requested spine/main-weapon redesign.
- `runtime-load-validation.json`: Node Three.js GLTFLoader import, bounds,
  material bindings, exact embedded viewer model, both detail-view bounds and
  all 44 moving muzzle sockets.
  The Node test uses PNG-header bitmap stand-ins; pixel inspection is separate.
- Ten software renders inspect actual exported geometry and embedded images.
  They are asset inspection renders rather than a full WebGL/PBR lighting test.
- `dark-gltf-validation.json`, `dark-runtime-load-validation.json` and
  `dark-surface-verification.json`: validation of the dark export, standard
  vertex-color material loading, exact preservation of geometry and texture
  bytes, and dark-background inspection renders.

The browser viewer was bundled and its model data checked. It was not exercised
in a browser because browser access is disabled by the saved permission.

This folder is an art asset and review viewer. It does not alter gameplay,
fleet definitions, shipyard entries, map rendering or battle balance.

The initial model, before these revisions, is preserved in
`revisions/spinal-lance-v1/` for comparison and recovery.
