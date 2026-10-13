# Isometric ship art · v1

Six transparent, three-quarter-view variants of the current approved ship masters. These are scene and panel art; the top-down sprites used by the galaxy map and battle theater are separate.

| Isometric master | In-game hull | Reference master |
| --- | --- | --- |
| `interceptor.png` | Raider / Interceptor | `../ships-2026-09-08/interceptor.png` |
| `cruiser.png` | Cruiser | `../ships-2026-09-08/cruiser.png` |
| `titan.png` | Titan | `../ships-2026-09-08/titan.png` |
| `freighter.png` | Convoy / corporate freighter | `../ships-2026-09-08/freighter.png` |
| `colony.png` | Colony ship | `../ships-2026-09-08/colony_ship.png` |
| `construction.png` | Builder / construction tender | `../ships-2026-09-08/construction_ship.png` |

All six were edited from their current top-down reference with the built-in imagegen tool. Shared generation brief: premium rendered hard-surface strategy-game art; a true three-quarter view from about 30° above; nose toward upper right, stern toward lower left; preserve each reference hull's defining silhouette and functional details; pale titanium, gunmetal, restrained brass and cyan lights; one complete centered ship with real RGBA transparency; no ground, starfield, shadow, exhaust plume, other ships or text. The per-hull identity constraints are recorded in `prompts.json`.

Open `review.html` to compare the top-down references with these variants on a dark game-like background. Run `node client/scripts/build-isometric-ship-art.mjs` from the repository root to regenerate the 256, 512 and 1024 pixel WebP derivatives and URL manifest in `client/public/art/derived/ships-isometric/v1/`. The PNGs here are the native masters.
