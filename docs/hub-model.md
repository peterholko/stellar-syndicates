# Market Hub station: the Authority Nexus

The Market Hub is drawn as the Authority Nexus, the wormhole anchor modelled in
`client/art-src/ship-models/authority-nexus-textured-v1/` (see its README). It
appears on the galaxy map and in the market panel.

## Galaxy map

- **Size.** The hub follows its own size curve (`HUB_SIZE_STOPS`: 44 to 180 px
  at 96×). Past 96× it grows with the camera, as hulls do in the inspection
  range, reaching 720 px at 384×.
- **Fleets beside it.** Fleets near the hub take their System View size, so the
  Nexus dwarfs them (an Interceptor is an eighth of the station's width at 96×
  and a 23rd at 384×). The ease runs over the same 40× to 64× window as near a
  star. It is full within 3 station radii (as displayed) of the centre and gone
  by 6 (`nearHubBlend`, `HUB_NEAR_RADII`, `HUB_CLEAR_RADII` in `render.ts`).
  Picking keeps its 24 px minimum. Below 96× the station is drawn larger than
  its world footprint, so a fleet a station radius out can sit on its rim.
- **Below 12× zoom.** A baked top-down sprite of the station, with a still
  vortex.
- **12× to 22×.** The same ramp live hulls use. The sprite crossfades into the
  live 3D station, which leans to −37° as it blends, seen from the screen's
  bottom edge. From 22× the hub is fully live.
- **The vortex.** It flows with the asset's runtime shader, redrawn at up to
  30 fps (20 fps on the largest tile). Like the System View's pulsars, it
  ignores the reduced-motion setting.
- **Traffic.** From a 300 px station (about 160×), the asset's ten small ships
  fly between the stabilizers and down into the vortex, with their wakes and
  entry ripples. They switch off again below 260 px. They are drawn at twice the
  asset's scale study (Interceptor to Medium Freighter, 1.7–3.6% of the
  station's width) so they read on the map. They sit inside the station's own
  render (`wormhole-traffic.ts`, in the station's frame), with no markers, labels
  or picks, so they never stand for real fleets. Their four hulls are the game's
  LOD2 models with 256 px textures, loaded the first time traffic is wanted.
- **Tile.** The live tile is 512, 1024 or 2048 px, the smallest that covers the
  station on the display (pixel ratio up to 2). It spans 1.25 station radii,
  room for traffic arriving from beyond the rim.
- **Label.** The hub label follows the leaning station's lower rim.
- **Loading.** three.js and the 1.96 MB station GLB load only while the hub is
  that close and on screen. They are released 15 s after it leaves, or at once
  when the map stops rendering.
- **Pose.** The stabilizer towers stand on the diagonals (`HUB_YAW`). In the
  panel hero's low view, a tower on an axis would rise in front of the vortex.

## Market panel

The "Authority market station" card shows a baked hero render of the station
(`HUB_MODEL.hero`).

## Pipeline

1. `npm run assets:hub:bake` (launch.json `hub-bake`, a loopback page on port
   8783 with no proxy) renders the full-resolution GLB. It produces the map
   sprite and the panel hero into `client/art-src/hub-models/`, using the live
   map's lighting, camera and still vortex phase. It then builds.
2. `npm run assets:hub` (also part of `assets:derive` and `npm run build`)
   derives into `client/public/art/derived/hub-model/v1/`:
   - **The web GLB.** Its ten textures are re-encoded as JPEG at ≤1024 px, about
     2 MB from 17 MB. Geometry, materials and the `WormholeCollapse` clip stay
     byte-identical.
   - **The map sprite.** A 1254 px WebP with lossless alpha, 247 kB.
   - **The hero.** 512 × 384 px.
   - **The catalog**, `client/src/hub-model.generated.ts`.

Bake again after changing the GLB, `HUB_SPRITE_FIELD`, `HUB_YAW` or
`HUB_HERO_ELEVATION`; the tests detect a stale bake. `HUB_ART_FILL` in
`render.ts` must equal the sprite's measured visible span, which the tests also
check.

## Not used

- **`WormholeCollapse`** and the traffic's containment-failure holding pattern.
  No game state drives them; they remain in the art folder.

## Code

- `hub-model-policy.ts`: ramp, lean, framing and timing rules.
- `hub-model-stage.ts`: loader and camera, shared by the runtime and the baker.
- `hub-model-runtime.ts`: the dynamically imported WebGL tile.
- `hub-model.ts`: the Pixi bridge that crossfades the live station over the
  sprite.
- `wormhole-effect.ts`: the vortex shader.
- `wormhole-traffic.ts`: the decorative traffic.
- `render.ts`: `drawHubBody`.

Tests: `npm run test:hub-model`. `node client/scripts/test-hub-model.mjs --serve`
(launch.json `hub-fixture`, port 4179) runs the real renderer on the hub, with
an own Interceptor parked beside it for scale. `?zoom=N` sets the zoom (default
192×) and `?ship=0` hides the Interceptor.

The art folder also holds an attack-disabled variant
(`disabled/authority-nexus-disabled.glb`), which the game does not use yet.
