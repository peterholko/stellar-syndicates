# Ground and orbital shipbuilding

## Hull allocation

| Facility | Tier I | Tier II | Tier III |
| --- | --- | --- | --- |
| Ground Shipyard | Tiny/Small Freighters, Scouts, Colony Ships, construction craft | Medium Freighters, Interceptors, Corvettes | More concurrent slips |
| Orbital Shipyard | Large Freighters, Destroyers | Heavy Freighters, Cruisers | Bulk Freighters, Battleships |
| Capital Slipway (orbital) | Dreadnoughts | Titans | More concurrent slips |

Each hull still needs its own existing research. Authority freighters remain unbuildable and troop transports still use a Garrison. Facility tier is also simultaneous ship capacity. Existing recipes, prices, construction durations, workforce rules, ownership and information delay are unchanged.

## Research and prerequisites

- Autoforges (Materials II) now unlocks Orbital Shipyards alongside its existing factories. The normal industrial activity gate and Materials I predecessor still apply; no warship construction or battles are required.
- Modular Berths remains an alternate initial unlock, preserving military progression and older researched saves.
- Orbital Shipyard requires Shipyard II somewhere in the same system. Capital Slipway retains its Dreadnought research gate and requires Orbital Shipyard III.
- Foundry IV / Orbital Yards unlocks Orbital Shipyard V–VI as well as its existing ground Shipyard V–VI and Orbital Warehouse rewards. It does not bypass the initial yard unlock.
- Bots already derive yard projects from the authoritative hull mapping and value research unlock effects, so they use this civilian path without a separate hull-gate table.

## Save and report compatibility

The displayed **Naval Drydock** is renamed **Orbital Shipyard**, but `naval_drydock` / `NavalDrydock` identifiers remain unchanged in saves and commands. Existing buildings continue operating; no automatic free structures or galaxy migration is needed.

New ship jobs store `construction_yard` at receipt. Paid large-freighter jobs in older saves, which lack the field, continue using their original ground Shipyard. Staffing, slip occupancy and bot workforce planning use that recorded yard. New large-freighter orders require the orbital yard. Owner-only build reports include the original yard from the same delayed snapshot; the client never looks at live job state.

The catalogue sends allowed alternate research unlocks explicitly. Higher-tier research is not treated as an alternate initial unlock.

## Art and placement

Six native transparent art tiers each for Ground Shipyard, Orbital Shipyard and Capital Slipway, plus six occupied Orbital Shipyard variants (72 lightweight WebP derivatives in total). Only planet-scene sprites change; menus keep the existing icon set.

When an arrived, owner-only build report contains a ship assigned to this world's Orbital Shipyard, its sprite switches to an occupied version showing one partially assembled hull. All six tiers retain their own station design. The hull is a generic activity indicator, not a depiction of the exact class, number of ships, or completion percentage. Queued and paused jobs retain it, including after an estimated completion time passes. It returns to empty only when the received queue no longer contains a matching job. Unconfirmed orders, other worlds, ground-yard jobs, modules, structure upgrades and rival queues cannot trigger it. Old reports without yard identity treat only Destroyer/Cruiser/Battleship jobs as orbital; legacy large freight remains ground-built.

The two space facilities have fixed positions above all twelve painted horizons, including while under construction. They have no terrain pads, surface shadows or ground haze. Ground Shipyard stays on its normal site; building an orbital yard does not move existing ground buildings. Gas giant scenes use the same orbital station sprites. All selection/workforce controls still work from received reports, and rival infrastructure remains hidden.

The full surface-art integration now pairs Ground Shipyard with the world's
shared biome ground layer. Orbital yards retain their normal industrial capacity
cost but use separate cosmetic starfield addresses rather than reserving terrain
positions. See [planet-surface-structures.md](planet-surface-structures.md).

Artwork and prompts: `client/art-src/planet-shipyards-v1/`. Neither the shared scene backdrop nor live galaxies are changed.
