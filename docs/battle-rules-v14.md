# Battle rules v14: home ground

Superseded for new battles by v15 (`docs/battle-rules-v15.md`), which keeps
this home ground and adds orbital defense stations and the siege class.
Battles opened on rules v14 keep it.

New battles opened on rules v14 (`crates/sim/src/tactical/v14.rs`). A battle
inside a system is fought at its world: the battle's position is within
platform cover (1300 su) of a system. Everything else is v13, and a battle in
open space is fought exactly as under v13, to the bit.

## In battle

- **The world.** It stands in the arena 900 su behind the side that holds
  the system, and its radius is 260. That is the defenders at their own
  system, and also the defenders at an unclaimed system or a third party's.
  It is the attackers when a fleet is attacked inside the attacker's own
  system. The world is terrain. Hulls steer around it and never enter it, and
  a torpedo whose path would cross it bursts on it.
- **Platforms hold orbit at the anchor.** The holding side's Defense Platform
  tiers are grouped 20 su apart on the world's side toward the enemy. That is
  where they stand in open space.
- **Relief arrives from where it came.** A fleet joining a battle enters at
  the arena's edge on the side it actually approached from, fanned across
  that line. A relief force that comes up behind a besieger hits it from
  behind. The direction rides the battle record (`RosterDelta.from`), so a
  replay places it the same way.
- **The estimate knows the world.** When the target is within platform cover
  of a system, the battle estimate projects the fight at that world. The
  world stands behind the side that the viewer's arrived report says holds
  it.

## Why these rules

The probe `tactical::v14::tests::probe_home_ground` fights each matchup 200
times in open space and 200 times at the defender's world:

| Fight | Attacker wins, open space | At the world |
|---|---|---|
| 8 torpedo interceptors + 3 driver cruisers vs 3 cruisers, 2 PD corvettes, 3 tiers | 50% | 48% |
| 2 driver battleships vs 4 cruisers, 4 destroyers, 2 tiers | 40% | 42% |
| 3 driver battleships vs the same | 98% | 97% |

| Retreat | Hulls home, open space | At the world |
|---|---|---|
| 4 cruisers leaving at half strength vs 6 cruisers + 6 web interceptors | 91% | 91% |
| 3 driver battleships leaving at 75% vs 10 web interceptors + 4 cruisers | 100% | 100% |

Fighting at home changes where things happen, not who wins. Two parts of the
first version were measured and dropped:

- **Platforms on the world's face.** With the world 520 behind the defenders
  and the platforms spread over a 1.2-radian arc of its face, attackers won
  61% of the first fight (50% in open space) and about ten points more of the
  second. The spread platforms stood out of mutual support, farther from the
  fight. Beam
  ships backing away from shorter-ranged guns also met the world. Grouping
  the platforms at the anchor and setting the world farther back brought both
  fights back in line.
- **A longer run out of the well.** Leaving a battle at a world meant reaching
  1.4 times the usual exit distance, so a beaten fleet would run longer under
  fire. It did the opposite: 98% of the retreating cruisers' hulls got home,
  against 91% in open space. Fighting hulls keep inside the 1000 su arena
  ring, so a runner held by tacklers at the farther edge sat beyond the
  pursuers' guns. Widening the ring with the exit still let more get away
  (94%) and shifted the ordinary fights (the second fight went from 40% to
  46%). Leaving a battle takes the same run at a world as in open space
  (`exit_radius`).

## Tests

- `a_fight_in_open_space_is_exactly_v13`
- `at_a_world_platforms_hold_orbit_and_nothing_enters_it`
- `torpedoes_burst_on_the_world`
- `leaving_a_battle_at_a_world_takes_the_same_run_as_in_open_space`
- `relief_enters_from_where_it_came_and_replays_the_same`
- `the_estimate_at_a_world_places_the_world_and_is_deterministic`
- `world::a_battle_takes_platform_losses_from_the_worlds_that_hold_them`: a
  battle removes the tiers it destroys from the bodies that hold them. A tier
  finished while the battle runs survives the next write-back.

## Test fight

`client/public/dev/battles/home` (`__ss.theaterDemo("home")`). Three cruisers
and eight torpedo interceptors attack two destroyers, two point-defense
corvettes and three platform tiers at the defenders' world. At round 40 a
relief of three cruisers and two destroyers enters behind the attackers.
Regenerate after any combat change:

```bash
BATTLE_FIXTURES_WRITE=1 cargo test -p server battle_fixtures
```
