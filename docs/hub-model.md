# Market Hub station: the Wormhole Hub

The Market Hub is drawn as the Wormhole Hub (Oct 22 2026), modelled in
`client/art-src/ship-models/wormhole-hub-textured-v1/` (see its README). It
appears on the galaxy map and in the market panel. It replaced the Authority
Nexus, whose art stays in `authority-nexus-textured-v1/` and
`public/art/derived/hub-model/v1/`, unused.

## Galaxy map

- **Size.** The hub follows its own size curve (`HUB_SIZE_STOPS`: 44 to 180 px
  at 96×). Past 96× it grows with the camera, as hulls do in the inspection
  range, reaching 720 px at 384×.
- **Fleets beside it.** Fleets near the hub take their System View size, so the
  hub dwarfs them (an Interceptor is an eighth of the station's width at 96×
  and a 23rd at 384×). The ease runs over the same 40× to 64× window as near a
  star. It is full within 3 station radii (as displayed) of the centre and gone
  by 6 (`nearHubBlend`, `HUB_NEAR_RADII`, `HUB_CLEAR_RADII` in `render.ts`).
  Picking keeps its 24 px minimum. Below 96× the station is drawn larger than
  its world footprint, so a fleet a station radius out can sit on its rim.
- **Below 12× zoom.** A baked top-down sprite of the station, with a still
  portal.
- **12× to 22×.** The same ramp live hulls use. The sprite crossfades into the
  live 3D station, which leans to −37° as it blends, seen from the screen's
  bottom edge. From 22× the hub is fully live.
- **The portal.** The galaxy turns with the asset's runtime flow
  (`wormhole-flow.js`), patched into the membrane's own material: one rotation
  rate inside a fixed rim, with a bounded ripple, so it never winds tighter. It
  is redrawn at up to 30 fps (20 fps on the largest tile). Like the System
  View's pulsars, it ignores the reduced-motion setting.
- **Traffic.** From a 300 px station (about 160×), the asset's nine freighters
  (three each of Tiny, Small and Medium) curve in over the ring's face and dive
  into the curved membrane, with their wakes and entry ripples. They switch off
  again below 260 px. They are drawn at twice the asset's scale study (3–4.7%
  of the station's width) so they read on the map; the closest pair stays more
  than a unit apart. They sit inside the station's own render
  (`wormhole-traffic.ts`, in the station's authored frame), with no markers,
  labels or picks, so they never stand for real fleets. Their three hulls are
  the game's LOD2 models with 256 px textures, loaded the first time traffic is
  wanted.
- **Tile.** The live tile is 512, 1024 or 2048 px, the smallest that covers the
  station on the display (pixel ratio up to 2). It spans 1.25 station radii,
  room for traffic arriving from beyond the rim.
- **Label.** The hub label follows the leaning station's lower rim.
- **Loading.** three.js and the 3.66 MB station GLB load only while the hub is
  that close and on screen. They are released 15 s after it leaves, or at once
  when the map stops rendering.
- **Pose.** The model is authored standing: ring in XY, front +Z, passage
  along Z. The loader lays it flat, front up (`HUB_LAY_FLAT`), with no yaw
  (`HUB_YAW` 0), so the map's top-down view is the approved front artwork: the
  spires top and bottom and on the diagonals, the docking arms left and right.

## Market panel

Peter (Oct 28 2026): "on the left side can you add a picture of the Wormhole
Hub as if the player is actually visiting". The desktop Market pop-out has a
full-height arrival picture on its left (`.deck-workspace__scene` in
`shell/deck/markup.ts`, shown only on the `market` route): 320 px wide, the
panel 1,240 px (its own 920 px plus the column). Below a 1,280 px window the
column hides and the Market keeps its old width. Mobile is unchanged.

The picture is a tall cut of the login painting's right side (the hub's
wormhole above, a freighter's engines below):
`client/art-src/login/login-market-horizon-v1.png` x 1100–1672, full height,
derived by `buildHubVisitArt` in `build-art-derivatives.mjs` to
`public/art/derived/hub-visit/hub-visit-v1.webp` (bump the file version when
the cut or source changes). The Warehouse tab's old "Authority market
station" card with the baked hero went with it; `HUB_MODEL.hero` still draws
the hub's port in Logistics.

## Pipeline

1. `npm run assets:hub:bake` (launch.json `hub-bake`, a loopback page on port
   8783 with no proxy) renders the full-resolution GLB. It produces the map
   sprite and the panel hero into `client/art-src/hub-models/`, using the live
   map's lighting, camera and still portal phase. It then builds. Without a
   human step: drive the page in headless Chrome over CDP and wait for
   `<html data-ready>`.
2. `npm run assets:hub` (also part of `assets:derive` and `npm run build`)
   derives into `client/public/art/derived/hub-model/v2/`:
   - **The web GLB.** Its eight textures are re-encoded as JPEG at ≤1024 px,
     3.66 MB from 17.2 MB. Geometry, materials and the `PortalClose` clip stay
     byte-identical.
   - **The map sprite.** A 1254 px WebP with lossless alpha, 187 kB.
   - **The hero.** 512 × 384 px.
   - **The catalog**, `client/src/hub-model.generated.ts`.

Bake again after changing the GLB, `HUB_SPRITE_FIELD`, `HUB_LAY_FLAT`,
`HUB_YAW` or `HUB_HERO_ELEVATION`; the tests detect a stale bake. The sprite is
not round: the docking arms span 1212 px across, the spires 1230 px up and
down. `HUB_ART_FILL` in `render.ts` is the widest span, and `HUB_RIM` and
`HUB_RIM_KEEL` in the policy are measured from the model; the tests check all
three.

## Not used

- **`PortalClose`** (the membrane shrinking shut) and the traffic's holding
  pattern for a closed portal. No game state drives them; they remain in the
  art folder.

## Code

- `hub-model-policy.ts`: ramp, lean, framing and timing rules.
- `hub-model-stage.ts`: loader and camera, shared by the runtime and the baker.
- `hub-model-runtime.ts`: the dynamically imported WebGL tile.
- `hub-model.ts`: the Pixi bridge that crossfades the live station over the
  sprite.
- `wormhole-effect.ts`: the portal flow.
- `wormhole-traffic.ts`: the decorative traffic.
- `render.ts`: `drawHubBody`.

Tests: `npm run test:hub-model`. `node client/scripts/test-hub-model.mjs --serve`
(launch.json `hub-fixture`, port 4179) runs the real renderer on the hub, with
an own Interceptor parked beside it for scale. `?zoom=N` sets the zoom (default
192×) and `?ship=0` hides the Interceptor.

The Nexus art folder also holds an attack-disabled variant
(`disabled/authority-nexus-disabled.glb`); the Wormhole Hub has none yet.
