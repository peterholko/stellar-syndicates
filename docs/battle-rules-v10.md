# Battle rules v10: formations

Superseded for new battles by v11 (`docs/battle-rules-v11.md`), which keeps
these formations and adds roles for small ships. Battles opened on rules v10
(`crates/sim/src/tactical/v10.rs`) keep it. A fleet now
holds a formation in battle, set in its mission profile (the fleet panel's
**Formation**: Automatic, Battle line, Escort ring, Wedge or Free). Everything
else (v9's bodies, wreckage and slow big-ship turning, v8's weaving, v7's
reach, every weapon, roll and range) is kept, and a fleet set to Free fights
exactly as under v9.

## The shapes

- **Battle line.** Side by side across the line of fire, heaviest in the
  middle, spaced by hull size. It closes until its shortest-ranged ship can
  fire, so every gun bears. Thin, and its point defense is spread out.
- **Escort ring.** Capitals and civilians in the middle; escorts in a crescent
  across the front at point-defense range (point-defense ships at its centre,
  the rest on its flanks), widening toward a full ring only when it runs out
  of room. It turns its screen toward the enemy's torpedo boats, and holds far
  enough back that its front escorts are in range while its core stays back.
- **Wedge.** The heaviest hull at the point, the rest trailing back on both
  flanks. It closes until its point is in range; the point takes the first
  fire, and the trailing escorts may not reach.
- **Free.** No formation: every hull fights by its role, as in v9.

**Automatic** picks a shape from the fleet and the enemy in front of it, and
re-reads the fight every step, so it adapts as ships die:

- Against torpedo boats, a fleet with point defense forms an escort ring; one
  without stays Free (its line ships and screens do better on their own).
- A fleet with civilians forms an escort ring.
- Otherwise it forms a battle line.

A wedge is always a deliberate choice.

## How a formation moves

Each formation has a frame: a centre, a facing and its own weave. The frame
faces the enemy's main body (its line, capitals, screens and platforms, not
its skirmishers) and wheels no faster than its slowest hull turns, nor faster
than the far end of the shape can keep up. It closes to its holding distance,
then weaves as one body at half its slowest hull's speed, on a circle that
hull can turn. It never outruns its slowest hull, and it waits for its
straggler, since a slow-turning capital chasing a slot that keeps moving at
full speed swings too wide to ever take it. If the enemy has nothing but
skirmishers, the formation faces the nearest of them but holds its anchor
point: they come to it, and chasing them would drag its slowest hulls and
whatever it guards into the open.

Each hull steers to its slot, led by the formation's own motion. Spacing,
wreck avoidance and slow turning still apply. Not in formations: interceptors
(they keep skirmishing, since speed is their armour), anything withdrawing, a
datalinked escort guarding its charge, and warships screening transports
that are present (Escort duty: Screen transports). Formations are saved with
the battle and replay exactly.

## Measured effect

`cargo test -p sim --lib balance_probe_v10 -- --ignored --nocapture` (24–32
rollouts each). Each formation against three opponents, as the defender's
chance of holding:

| Fleet | Against | Free | Line | Ring | Wedge |
|---|---|---|---|---|---|
| Battleship, 4 cruisers | 8 torpedo interceptors, 4 torpedo destroyers | 83% | 67% | 79% | 29% |
| Battleship, 4 cruisers | 4 cruisers, 4 destroyers | 0% | **46%** | 17% | 0% |
| Convoy, 2 cruisers, 3 PD corvettes | 8 torpedo interceptors, 4 torpedo destroyers | 71% | 54% | **100%** | 62% |

No shape wins everywhere: the battle line wins gun fights, the escort ring
(with point defense) stops torpedo strikes, and Free is the flexible default.
Against v9 with Automatic formations, most matchups are within noise. The
honest cost: three point-defense corvettes escorting a convoy against six
torpedo interceptors lose 19% of fights in their ring, where v9's free
screens never lost, because escorts weaving at the convoy's pace are easier to
hit than escorts moving freely. Choose Free for that fleet if it matters.

One test was re-earned: the mid-battle save test runs until the first hits
land, since the defenders first form a battle line.

## Test fights

`client/public/dev/battles/` fights now give each side one fleet, so they hold
formations: in "A battle line against an escort ring" (`formations`) a
battleship, 4 cruisers and 2 destroyers form a battle line, and a convoy's 2
cruisers and 3 point-defense corvettes form a ring around it; in the torpedo
strike the defenders' point defense forms a screen. Regenerate after any
combat change:

```bash
BATTLE_FIXTURES_WRITE=1 cargo test -p server battle_fixtures
```
