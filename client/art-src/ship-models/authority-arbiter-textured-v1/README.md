# Authority Arbiter — top-view art pass

Dark armor is the lead finish. This is a review asset; no live game catalog,
map integration, fleet definition or combat rules have been changed.
Built with Python/NumPy and Three.js, without Blender.

## Review

- `viewer.html`: self-contained ivory/dark viewer. `?variant=dark&view=top`
  opens the revised overhead view. Orbit, Spines, Mass driver, orthographic
  views, underside, low light, galaxy backdrop, wireframe and turret motion
  are available. Download GLB saves the selected finish.
- `previews/top-design-review.png`: approved sprite, previous model and revised
  dark model, each at the same overall display length, with 128-pixel samples.
- `previews/dark-galaxy.png`: current three-quarter view.
- `previews/dark-underside.png`, `dark-rear.png`, `dark-low-light.png` and
  `dark-map-readability.png`: coverage and readability inspection.
- `sprites/authority-arbiter-dark-top-{1024,512,256,128}.png`: transparent
  candidate top sprites. These use software inspection lighting; a WebGL/PBR
  bake in the game's normal pipeline remains part of later integration.
- `review-sprite-manifest.json`: source hashes and sprite framing.

## Changes in this pass

The mass driver is shortened into the two tapered prow blades. Its muzzle
projects only 1.57% of the ship's length beyond the armor tips, with a real
2.37-unit-deep launch bore. The reinforced breech, two acceleration rails and
recoil sleeves remain; the long external ladder housing is removed.

The three staggered spines keep their sculpted geometry. All upward armor
faces now carry broad gold bands, with narrow staggered joints and dark root
braces. The halo is a wider continuous gold band, including its inner wall.
The stepped command tower and the additional armament remain.

Reference-sheet ship pictures no longer cover the reconstructed armor. Clean
material swatches map to physical faces at controlled density: ceramic armor,
antique gold, navy machinery and gunmetal service panels. Separate UV islands
fit the cooling grilles on the engines, heavy turrets and broadside galleries.
The muzzle uses a plain gunmetal region rather than a striped wall texture.
The underside has dedicated service panels and closed boarding-hangar doors.

## Exports

| Finish | File | Size |
| --- | --- | ---: |
| Dark, lead version | `authority-arbiter-dark.glb` | 1,910,776 bytes |
| Ivory comparison | `authority-arbiter.glb` | 1,541,312 bytes |

Both versions contain **19,388 triangles**, **25 render sections**, two PBR
materials and 414 closed geometric components. This is within the brief's
12–20k budget. The 22 gun mounts stay separate; static parts are consolidated.
The material change adds no triangles or sections.

The three embedded 1024-square maps comprise a 350,489-byte JPEG albedo,
PNG metallic/roughness data and PNG emission. No external textures or
compression extensions are required to load either GLB. The dark version
uses standard linear glTF `COLOR_0` multipliers over the same new texture maps.
The OBJ/MTL export is the ivory model and needs the adjacent `textures/` folder.

Axes: Y up, nose along −Z. Bounds: 13.84 wide × 6.91 high × 23.7272 long,
in relative model units. Gameplay scale is assigned during integration.

## Armament and sockets

- Six heavy twin turrets, eight secondary twin turrets and eight paired
  point-defense mounts: 22 moving mounts and 44 barrel tips.
- One fixed capital mass driver, plus four main engines.
- All 45 physical weapon muzzles now use `socket.weapon.*`: `heavy`,
  `secondary`, `point-defense` and `mass-driver.muzzle` subnames.
- Four exhausts use `socket.engine.*`. Heavy pivot sockets still use
  `socket.turret.*`; the game prefers the weapon muzzles when they exist.
- `model-info.json` and `dark-model-info.json` record all 73 sockets, counts,
  assembly roles and rest-pose model-space coordinates. GLB muzzle nodes are
  children of their moving assembly, with parent-local translations.

The actual exported `hardpointsOf` function from the game's
`client/scripts/build-ship-hardpoints.mjs` discovers **45 guns and four engines**
for both finishes. The catalog and generated hardpoint table are not changed.

## Material source and prompt

The clean atlas was generated with the **built-in `image_gen.imagegen` tool**.

- Saved source: `materials/material-atlas-artpass-v2.png`.
- Exact generation prompt: `materials/art-pass-v2-prompt.json`.
- Packing/mapping code: `reconstruction.py`.
- Native source resolution: 1254 × 1254. Production maps are 1024 × 1024.
- Lamp and exhaust islands come from the approved reference sheet. The
  approved sprite and sheet remain unchanged in `../../authority-arbiter-reference-v1/`.

This pipeline extracts material regions, packs and resamples the atlas, and
constructs deterministic PBR parameter masks. The dark finish is applied on
model vertices; its texture images match the ivory version byte for byte.

## Rebuild and validate

Python requirements are in `requirements.txt`; JS versions are pinned in
`package.json`. Existing dependencies in `../pirate-fleet-v1/node_modules`
are also supported.

```sh
python3 build_model.py
python3 build_dark_variant.py
node build_preview.mjs
python3 inspect_model.py
python3 inspect_dark_variant.py
python3 build_review_art.py
node validate.mjs
node validate.mjs authority-arbiter-dark.glb
node test_asset.mjs
node test_asset.mjs --dark
```

Both glTF validators report **zero errors and zero warnings**. The surface
checks verify finite attributes, normals, winding, nondegenerate triangles,
noncollapsed UVs, closed positive components, the open halo and muzzle bore,
feature counts, gold spine surfaces and the shortened weapon's silhouette.
The Node Three.js loader verifies materials, both embedded viewer assets,
all 22 moving gun mounts and 44 following muzzle sockets, and runs the real
battle hardpoint extractor. Pixel data is separately decoded by Python;
Node bitmap stand-ins are used for loading the scene graph.

The interactive viewer uses the current game's lighting parameters and
`#05070d` galaxy background base. Its starfield is a preview backdrop. Software
renders check 128/256-pixel top views and lighting reduced to 40%, without bloom
or an artificial outline. These are inspection renders, not a browser or
in-game WebGL test. Browser access remains disabled by the saved permission.

## Preserved revisions

- `revisions/spinal-lance-v1/`: initial model with the original lance.
- `revisions/long-mass-driver-v1/`: complete pre-art-pass ivory/dark GLBs,
  source scripts and previews, including the superseded extended weapon.
