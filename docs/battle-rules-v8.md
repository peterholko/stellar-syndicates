# Battle rules v8: big ships keep moving

Superseded for new battles by v9 (`docs/battle-rules-v9.md`), which keeps this
weave and adds hull bodies and physical wreckage. Battles opened on rules v8
(`crates/sim/src/tactical/v8.rs`) keep it. A line hull
(corvette, destroyer, cruiser) that has reached its firing band, and a capital
anchoring the line (battleship, dreadnought, titan), no longer parks. Each
flies a small circle at half its top speed. The to-hit roll reads a target's
current speed, so a parked hull used to be the easiest target on the field:

| Hull | Beam hit chance, parked → weaving | Circle radius |
|---|---|---|
| Cruiser | 91% → 50% | 89 |
| Battleship | 94% → 57% | 80 |
| Dreadnought | 95% → 64% | 74 |
| Titan | 95% → 71% | 66 |

Torpedoes still hit dreadnoughts and titans 95% of the time, so they remain
the answer to capitals.

Each circle is as tight as the hull can turn with 60% of its acceleration,
which also stays within the battle viewer's turn rates, so noses follow the
circle. A line hull's circle sits a little inside its firing band, so the
whole circle stays in range. Capitals sharing an anchor take different
phases, so they spread around it instead of stacking.

Unchanged: civilians, platforms, point-defense screens, escorts, missions,
skirmishers and anything withdrawing move exactly as in v7, and every weapon,
range, roll and v7's hull-sized reach are kept. Saved battles keep the rules
they opened with, and their replays are unchanged.

## Measured effect

Against frozen v7 (`cargo test -p sim --lib balance_probe_v8 -- --ignored
--nocapture`, 32 rollouts each), fights that involve line or capital hulls
run 1.4–2.5× longer, and those hulls survive far better:

- 4 corvettes against 2 destroyers: 68 → 168 steps; 2 cruisers against a
  battleship: 98 → 180; a 5-cruiser mirror: 103 → 170.
- 3 interceptors raiding 5 corvettes win 12% of the time (56% in v7);
  8 mass-driver interceptors against 2 cruisers win 44% (97%); 20
  interceptors against a titan win 84% (100%).
- Interceptor-only fights and raids on convoys are unchanged.

`WEAVE_SPEED` (0.5 of top speed) is the dial. At 0.3, fights still run 20–75%
longer: under the current to-hit formula even slow movement protects a lot.

One test was re-earned: the per-kind battle report uses four raiders against
five corvettes (not three against six), because three raiders now die
without sinking a weaving corvette.

## Torpedoes in the battle record

Every recorded round now lists each torpedo in flight (position, target and a
stable id, at most 32 per side) and how each one that ended did so: a hit and
its damage, a miss, shot down (and by which hull), or ran dry because its
target was gone. The id comes from the torpedo's own launch damage roll, so no
saved state, checksum or random draw changes, and older battles gain the data
when their rounds are rebuilt from their replays. The battle viewer flies each
torpedo along its real path at its real speed (140 units a round). See
`docs/ship-model-integration.md`.

## Test fights

The engine fights in `client/public/dev/battles/` are recorded on v8. Regenerate
them after any combat change (the server test fails while they are stale):

```bash
BATTLE_FIXTURES_WRITE=1 cargo test -p server battle_fixtures
```
