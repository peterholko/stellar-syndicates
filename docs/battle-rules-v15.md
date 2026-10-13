# Battle rules v15: stations

Superseded for new battles by v16 (`docs/battle-rules-v16.md`), which keeps
every v15 rule and computes trig and powers in software, the same in every
build. Battles opened on rules v15 keep it.

New battles opened on rules v15 (`crates/sim/src/tactical/v15.rs`). It adds a
system's orbital defense station to the fight (see `docs/orbital-defense.md`)
and the siege class. Everything else is v14. A fight with no station, no
platform and no capital is fought exactly as under v14, to the bit.

## In battle

- **Hardpoints.** A station fights through its hardpoints, one at level 1
  and six at level 5. Each joins the holding side's battle as a stationary
  battery. It has its own hull, its level's attack weight and reach, and fires
  what it is fitted with, one shot per gun as in v12. A hardpoint with no fit
  is the stock beam battery. Armour fits protect it the way they protect a
  ship. Every hardpoint carries the heavy point-defense screen (400 su). Only
  hardpoints that stand when the battle opens fight: a knocked-out one sits
  it out until repaired.
- **Placement.** Hardpoints open at the holding side's anchor, 24 su apart
  across the line of approach. At a world (v14), they hold orbit at the
  anchor with any legacy platform tiers, 20 su apart.
- **The siege class.** Battleships, dreadnoughts and titans hit hardpoints
  and legacy platforms ×1.25 (`SIEGE_BONUS`). Smaller hulls hit them at the
  usual rate.
- **Station research in battle:**
  - **Breaching Ordnance:** a further +25% against hardpoints and platforms,
    for any hull.
  - **Platform Netfire:** the station's point-defense screen reaches 700 su
    (`NETFIRE_RADIUS`), covering the fleet beside it.
  - **Fortress Doctrine:** in a battle that fields your station, your hulls
    can't be held by tacklers.
  - **Grand Batteries:** +25% opening damage, now only in a battle where your
    side has station or platform support.
- **After the battle.** Each hardpoint's hull is written back to the station.
  One destroyed in the battle is knocked out. The battle record lists which
  hardpoints each side fielded (`Engagement.station_mounts`), so a hardpoint
  missing at the end was destroyed.
- **The record and the theater.** A hardpoint is recorded like any hull, with
  its station's level (`KfShip.station`). The battle theater draws each side's
  station once, at the centre of its hardpoints and sized by level, with the
  hardpoints as turrets on a ring over it. A knocked-out hardpoint bursts like
  a heavy hull. A silenced station dims.
- **The estimate.** A station in the viewer's scouted intel enters the
  projection whole and stock-fitted: the intel says its level, not its fits
  or its damage.

## Why these rules

A station has to be worth its goods without being a wall. Before the siege
class, fleets of cruisers or battleships worth a Star Fortress's goods beat
it every time: its hull was a pile of hit points in front of guns that never
miss a still target. Hardpoints with big guns fixed that. Then the siege
class gave battleships and larger a role against fixed defenses. At ×2 it
made battleship fleets dominant against every level. At ×1.25 with stronger
upper levels, the ladder lands where the design wants it. Against a Citadel
or a Star Fortress, siege-class capitals worth its goods win 44–84%, cruisers
38%, and destroyers and interceptors almost never. A lone Titan never takes a
Star Fortress. The numbers are in
`docs/orbital-defense.md` (probe `tactical::v15::tests::probe_station_ladder`).

## Tests

- `a_fight_without_stations_or_capitals_is_exactly_v14`
- `hardpoints_fight_as_fitted`
- `capitals_besiege_stations` (siege class, and Breaching Ordnance on top)
- `fortress_doctrine_slips_tackle`
- `netfire_widens_the_screen`
- `the_estimate_fields_stations`
- `replay` tests: a recorded fight with a station replays to the same
  checksum. The mount enters the checksum only when present, so older records
  keep theirs.

## Test fight

`client/public/dev/battles/citadel` (`__ss.theaterDemo("citadel")`, or "A
battle line besieges a Citadel" on the battle fixture page). Two
mass-driver battleships, three driver cruisers and two point-defense corvettes
attack a level-4 Citadel at its world. The Citadel's four hardpoints carry
three different fits and one stock battery, screened by two torpedo
destroyers and two corvettes. The battle line breaks the station in about 135
rounds and comes out battered. Regenerate after any combat change:

```bash
BATTLE_FIXTURES_WRITE=1 cargo test -p server battle_fixtures
```
