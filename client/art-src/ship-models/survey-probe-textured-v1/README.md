# Survey Probe — textured reconstruction

[GLB](survey-probe.glb) · [OBJ](survey-probe.obj) · [seven-view previews](previews/) · [contact sheet](previews/contact-sheet.png)

Built from the [five-view reference](../../survey-probe-reference-v1/survey-probe-reference.png) and [concept sprite](../../survey-probe-reference-v1/survey-probe.png), using the shared [Mining Fleet reconstruction pipeline](../mining-fleet-v1/README.md). Blender is not used.

| Property | Value |
| --- | --- |
| Triangles | 2,448 |
| Render sections / materials | 2 / 2 |
| Closed source components | 50 |
| PBR textures | Embedded base color, metallic/roughness and emissive, 2048 × 2048 |
| Axes | Y up, −Z forward |
| Bounds, XYZ | 4.3106 × 1.92 × 5.7827 relative model units |
| Attachment point | `socket.engine.main`, exhaust direction +Z |

The craft has one forward-facing concave sensor dish, four diagonal antenna booms, two folded radiator fins and one cylindrical engine. It has no cockpit, crew modules or weapons. Static parts are merged by material. The sprite fills 89% of its square canvas height; transparent margins were cropped without resampling the art.

Top and bottom armor use their dedicated reference drawings. Hull walls use the side elevation. The dish interior projects the front drawing onto an actual bowl; the single recessed exhaust uses the rear drawing. Cylinder walls and small fittings use continuous UVs and matching native/industrial material samples. Every physical triangle has nonzero-area UVs and PBR texture coverage, including 505 downward-facing triangles.

This is a coherent concept reconstruction. The plan views govern length and width; end/profile views govern the dish and engine. Antenna height and radiator folds are standardized where the generated elevations disagree. Relative model units are not meters; integration should use the intended small Survey Probe gameplay size.

## Rebuild and verify

From the repository root:

```sh
python3 client/art-src/ship-models/survey-probe-textured-v1/build_model.py
python3 client/art-src/ship-models/survey-probe-textured-v1/inspect_model.py
node client/art-src/ship-models/mining-fleet-v1/validate.mjs ../survey-probe-textured-v1/survey-probe.glb
node client/art-src/ship-models/survey-probe-textured-v1/test_asset.mjs
```

Requires Python 3, NumPy and Pillow, plus the repository's existing Node dependencies. The GLB is self-contained; OBJ needs its MTL and `textures/` directory. Keep `mining-fleet-v1/` and the reference folder for rebuilding.

Khronos glTF validation passes with **zero errors and zero warnings**. Independent exported-buffer checks verify closed positive-volume components, normals, winding, UV coverage, feature counts and the triangle budget. Three.js r180 loads the materials and validates the engine attachment transform. Preview images render actual exported geometry and embedded textures in software; these checks are not a browser/GPU performance benchmark.
