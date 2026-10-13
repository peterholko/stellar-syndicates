# Orbital defense stations

Every system can raise one orbital defense station, level by level, from a
Defense Platform to a Star Fortress. A station is the `orbital_defense`
structure (`StructureKind::OrbitalDefense`), and its tier is its level. The
ladder lives in `crates/sim/src/station.rs`, the battle rules in
`crates/sim/src/tactical/v15.rs` (`docs/battle-rules-v15.md`), and every
client surface reads it through `client/src/core/derive/station.ts`.

## The ladder

| Level | Name | Hardpoints | Hull each | Attack weight each | Reach | Research | Yard in the system | Build time |
|---|---|---|---|---|---|---|---|---|
| 1 | Defense Platform | 1 | 2,400 | 3 | 1.0 | Fire Control | none | 22 s |
| 2 | Orbital Battery | 2 | 2,000 | 2 | 1.0 | Orbital Batteries (Weapons II) | none | 1 h |
| 3 | Defense Station | 3 | 3,000 | 3 | 1.0 | Defense Stations (Combat Management III) | Ordnance Foundry | 6 h |
| 4 | Citadel | 4 | 12,000 | 6.5 | 1.1 | Grand Batteries (Combat Management V) | Orbital Shipyard | 48 h |
| 5 | Star Fortress | 6 | 32,000 | 22 | 1.2 | Star Fortress (Combat Management V) | Capital Slipway | 192 h |

For scale, a corvette's attack weight is 1 and a Titan's 24, and a Titan's
hull is 32,000. A Star Fortress has six times a Titan's hull and more than
five times its firepower, for about two and a half Titans' goods.

Each level is paid for when it is raised. The goods below are for that level
alone. At base prices the whole ladder costs about 279,000 credits:

| Level | Goods | Base value |
|---|---|---|
| 1 | 35 Alloys, 15 Electronics, 15 Armaments | 2,260 |
| 2 | 60 Alloys, 25 Electronics, 25 Armaments, 10 Machinery | 4,430 |
| 3 | 90 Alloys, 40 Electronics, 50 Armaments, 20 Machinery, 20 Composites, 6 Precision Components | 9,292 |
| 4 | 300 Alloys, 150 Electronics, 250 Armaments, 80 Machinery, 80 Composites, 60 Hull Sections, 20 Precision Components | 45,380 |
| 5 | 1,200 Alloys, 600 Electronics, 1,000 Armaments, 300 Machinery, 400 Composites, 300 Hull Sections, 120 Precision Components, 200 Titanium, 100 Rare Elements | 217,640 |

## Raising a station

- The station orbits one body of the system and takes an Infrastructure slot
  there. The first level goes on the body chosen in the builder. An order
  that names no body goes to an uninhabited body with a free slot. Each later
  level is raised on the same body.
- Each level is its own construction project. The station keeps fighting at
  its current level while the next one builds.
- A level needs its research, completed (queued or active research never
  counts), and Fire Control. It also needs its yard standing in the system.
- **One station per system.** A second body can't start one while a station
  or a station job exists elsewhere in the system.
- **One Star Fortress per corporation**, standing or rising: a Citadel
  already being raised elsewhere counts.
- The builder lists the station's five levels with each level's goods, time,
  research and yard. It says what blocks the next level. The sim checks every
  gate again when the order lands (`OneStationPerSystem`,
  `StarFortressLimit`, `NeedsYard`, `NeedsResearch`).
- The legacy Defense Platform structure is no longer built. Old colonies and
  pirate bases keep their platform tiers, which fight under the old platform
  model.

## Hardpoints

A station fights through its hardpoints, and each one is a stationary battery
in battle (`docs/battle-rules-v15.md`).

- **Fits.** A hardpoint has three slots and eight fitting points, a
  destroyer's budget:

  | Module | Points |
  |---|---|
  | Mass Driver | 2 |
  | Torpedo Rack | 3 |
  | Prismatic Lance | 3 |
  | Reflective Plating | 2 |
  | Whipple Armor | 3 |
  | Stasis Web | 2 |

  A Prismatic Lance needs Platform Lances research and no crate. Point
  defense comes built into every hardpoint, so the point-defense module isn't
  fitted, and neither are utilities or warp stabilizers. A hardpoint with no
  fit is the stock beam battery.
- **Refitting** needs an Ordnance Foundry in the system. It takes module
  crates from the system's ledger and puts the removed ones back. It can't
  happen during a battle. The order is `FitStation`: in the Defense section,
  Refit a hardpoint, choose its modules and confirm. The editor shows the
  crates it needs and how many the system holds.
- **Knocked out.** A hardpoint destroyed in battle stays offline until it is
  repaired to a quarter of its hull (`ONLINE_HEALTH`). It doesn't limp back
  into the next fight on one hit point.
- **Repair.** A hardpoint repairs a full hull per siege clock, twice as fast
  with an Ordnance Foundry in the system. There is no repair during a battle
  at the system or under a held blockade.
- **Silenced.** A station with every hardpoint out is silenced. It no longer
  contests a blockade, and the blockade can be held.

## In battle and in blockades

- A station joins every battle within platform cover (1,300 su) of its
  system on its owner's side. That includes a raid on the owner's ships near
  the system (`covering_platform`). At a world, it holds orbit at the
  defenders' anchor.
- Battleships, dreadnoughts and titans hit hardpoints ×1.25: the siege class.
- Station research in battle:
  - **Breaching Ordnance:** +25% against stations and platforms.
  - **Platform Netfire:** the hardpoints' point defense reaches 700 su.
  - **Fortress Doctrine:** your hulls can't be tackled in a battle your
    station fights.
  - **Grand Batteries:** +25% opening damage in battles your stations fight.
- A blockade stays contested while any hardpoint fires or any legacy tier
  stands (`StarSystem::defenses_standing`). Once the station is silenced and
  no defenders remain, the blockade is held and the siege clock can run.
- After each battle the owner gets a report (`StationEngaged`): how many
  hardpoints were knocked out, how many still fire, and whether the enemy
  was destroyed or driven off.
- **Capture.** A captured system's station comes over one level down, with
  every hardpoint knocked out. A captured Defense Platform is lost. Other
  structures are halved as before.

## Intel and estimates

A scout's report carries the system's station level. The battle estimate
fields a scouted station whole and stock-fitted, because intel doesn't show
its fits or damage.

## Surfaces

- **System View:** the station is drawn in orbit around its body, using its
  level's art, and grows with each level.
- **Defense section (desktop and mobile):** the station's art and level, how
  many hardpoints fire, and each hardpoint's fit, hull and repair. It also
  holds the refit editor and the next level with what it needs, plus a
  "Raise to …" button that opens the builder.
- **Planet stage:** the station hangs in the sky above its world, between
  the yards. It grows with each level and hangs a little lower, so a Star
  Fortress is the largest thing in orbit. Its row in the world's structure
  list reads "Level N of 5".
- **Battle theater:** each side's station is drawn once, under its
  hardpoints, sized by level, with the hardpoints as turrets on a ring over
  the art. A knocked-out hardpoint bursts. A silenced station dims.
- **Art:** Peter's lineup is
  `client/art-src/orbital-defense-lineup-v1/orbital-defense-lineup.png`. The
  per-level masters are in `client/art-src/orbital-defense-v1/`.
  `client/scripts/build-station-art.mjs` (part of `npm run assets:derive`)
  writes `client/public/art/derived/stations/v1/level-N-{128,256,512}.webp`.

## Balance

Measured with `tactical::v15::tests::probe_station_ladder`: 200 seeded fights
per row at the defenders' world, against stations with stock batteries. Each
attacking fleet is worth the station's goods at base prices, counting every
level up to it. Battleships fit four mass drivers, dreadnoughts five and the
Titan six.

| Station (goods, all levels) | Torpedo interceptors | Destroyers | Driver cruisers | Battleships | Dreadnoughts |
|---|---|---|---|---|---|
| Defense Platform (2,260) | 1: 0% | 1: 40% | 1: 100% | 1: 100% | 1: 100% |
| Orbital Battery (6,690) | 4: 0% | 1: 0% | 1: 100% | 1: 100% | 1: 100% |
| Defense Station (15,982) | 8: 0% | 3: 10% | 1: 11% | 1: 100% | 1: 100% |
| Citadel (61,362) | 32: 0% | 11: 2% | 5: 38% | 2: 84% | 1: 44% |
| Star Fortress (279,002) | 146: 0% | 50: 4% | 25: 38% | 11: 65% | 5: 50% |

Each cell is the number of hulls and how often they win. Fleets are whole
hulls, at least one, so below the Citadel a single cruiser or larger is worth
more than the station. Only the Citadel and Star Fortress rows compare equal
goods.

Against the Star Fortress:

| Attacker | Goods | Attacker wins |
|---|---|---|
| A lone Titan | about 110,600 | 0% |
| A Titan and three dreadnoughts | 263,445 | 56% |
| A Titan and five dreadnoughts | 365,335 | 96% |

The intent: at the Citadel and the Star Fortress, siege-class capitals
worth the station's goods win about half the time or better (44–84%; the
battleship fleets do best), cruisers about a third (38%), and destroyers and
interceptors almost never. A station is a huge, still target that fires a few big guns at moving
hulls and can never help anywhere else. In return it carries about twice the
hull and twice the firepower its goods would buy as warships. A lone Titan
never takes a Star Fortress.

## Not built yet

These are from the station design, for a second pass:
- fighter or drone bays;
- the Seal, a system lock that holds jumps;
- a station commandant;
- the Sunlance, the Star Fortress's superweapon.

Four older platform researches now act on stations: Breaching Ordnance,
Platform Lances, Platform Netfire and Fortress Doctrine.

## Tests

- Sim, `station.rs`:
  - `the_ladder_climbs_past_a_titan`
  - `hardpoints_take_weapons_armour_and_webs_only`
  - `only_standing_hardpoints_fight`
- Sim, `world.rs`:
  - `a_station_climbs_one_researched_level_at_a_time`
  - `one_star_fortress_per_corporation`
  - `a_station_contests_a_blockade_until_it_is_silenced`
  - `hardpoints_repair_faster_at_a_foundry`
  - `a_hardpoint_refits_from_the_ledger_at_a_foundry`
  - `a_station_covers_raids_near_its_system`
  - `a_captured_station_comes_over_a_level_down`
- Battle rules: `tactical::v15` tests (`docs/battle-rules-v15.md`), and
  `replay::a_station_fight_replays_exactly_across_checkpoints`.
- Client:
  - `npm run test:system-defense`: summary, hardpoint rows, refit editor and
    the FitStation order, next level.
  - `npm run test:industry`: the ladder's build gates.
  - `npm run test:battle-models`: hardpoints on the ring, still, bursting as
    heavy hulls.
  - `npm run test:planet-panel`: the station in the sky by level, and its
    level line. The fixture's orbital-yards mode
    (`planet-fixture-orbital` in `.claude/launch.json`) puts stations of
    every level over its worlds.
- Test fight: `client/public/dev/battles/citadel`.
