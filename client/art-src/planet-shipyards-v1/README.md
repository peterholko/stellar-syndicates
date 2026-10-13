# Planet-scene shipyards

Scene-only art: the existing menu/inventory icon set is unchanged.

- `shipyard/tier-1.png` … `tier-6.png`: approved ground yards from `output/imagegen/surface-structures-v2/buildings/shipyard`.
- `naval_drydock/tier-1.png` … `tier-6.png`: newly generated Orbital Shipyards.
- `naval_drydock/tier-1-occupied.png` … `tier-6-occupied.png`: corresponding yards with one unfinished ship held inside a berth.
- `capital_slipway/tier-1.png` … `tier-6.png`: newly generated super-capital construction stations.

All masters retain native generated alpha, resolution and aspect ratio. Each has its generation specification in the adjacent `.prompt.json`. New orbital images were created using the built-in imagegen tool, with the ground Shipyard/Boreal painting as material and lighting references; upgrades use their family's first orbital tier as the identity reference. No Authority seals, roads, ground pads or planet paintings are baked into orbital art.

Occupied variants were edited with the built-in imagegen tool from each matching idle tier. The original station/camera/framing is preserved, with a generic exposed keel, hull ribs and partially fitted bow added inside one berth. Original idle PNGs are untouched. The renderer switches only from received, body-local Orbital Shipyard ship jobs; the image stays occupied when work pauses and clears on a received empty queue. It does not visualize precise build progress or multiple simultaneous hulls.

`node client/scripts/build-planet-shipyard-art.mjs` creates 128/256/512-wide WebP derivatives under `client/public/art/derived/planet-shipyards/v1`. The manifest records source SHA-256 and dimensions. Only resizing/encoding is performed; originals are not edited. The normal client build includes this step.

`client/src/planet-structure-art.ts` selects the scene art and fixed orbital positions. The planet renderer keeps the backdrops intact and places the two orbital families in the starfield. They keep normal industrial support/worker requirements and do not grant free development capacity.

Run `npm --prefix client run test:orbital-shipyards` to verify assets, real scene output and build gates. The isolated preview uses `node client/scripts/test-planet-panel.mjs --serve --orbital-yards --port=4330`; it never connects to a game server. Select any world and use **Next yard tier** to review all six levels, and **Toggle ship queue** to switch between idle and occupied reports.
