# Battle rules v7: hull-sized weapon reach

Superseded for new battles by v8 (`docs/battle-rules-v8.md`), which keeps this
reach and adds weaving line and capital hulls. Under v7
(`crates/sim/src/tactical/v7.rs`), each hull's beam and mass-driver reach,
and the stand-off distance it steers for, scale with its class, so light
hulls must close to fight while capitals engage from further out. Saved
battles keep the rules they opened with, and their replays are unchanged.

| Hull | Reach | Beam range | Driver full-strength range |
|---|---|---|---|
| Interceptor | 0.6× | 390 | 210 |
| Corvette | 0.8× | 520 | 280 |
| Destroyer | 0.9× | 585 | 315 |
| Cruiser | 1.0× | 650 | 350 |
| Battleship | 1.1× | 715 | 385 |
| Dreadnought | 1.2× | 780 | 420 |
| Titan | 1.3× | 845 | 455 |

Unchanged: torpedo launch range (800), point defense, defense platforms,
movement speeds, hit chances, damage and every v6 behaviour (missions,
datalinks, discovery weapons, maneuvers). Gun-armed interceptors already
flew close passes (about 160–240 units from their target); now their guns
need it.

## Measured effect

Against frozen v6 (`cargo test -p sim --lib balance_probe -- --ignored
--nocapture`, 32 rollouts each), most matchups are unchanged. The shifts go
the intended way:

- Capitals hold up better against swarms (20 interceptors lose 7, not 6, to a
  titan), and a titan with corvettes beats three battleships losing 2 ships
  instead of 3.
- The interceptor mirror is fairer (59% vs 66% for the attacker) and longer
  (54 rounds vs 43), because nobody trades shots on the approach.
- Interceptors open fire a few rounds later, so raids take slightly longer
  (one interceptor kills a lone convoy in 29 rounds instead of 25).

Three tests were re-earned for v7: the per-kind losses report uses 6
corvettes (not 5) so both sides still bleed; the tutorial privateer's opening
window is 8 seconds (not 6); and the pirate-patrol counters are compared over
200 seeds instead of 20 (point defense's edge against torpedo patrols is
slim under both v6 and v7).

## Test fights

`client/public/dev/battles/*.json` are real fights on the current rules,
recorded the way live battles are, as the attacker sees them. The battle
viewer loads them with `__ss.theaterDemo("mixed" | "swarm" | "torpedo" |
"titan")` and the battle preview page lists them. Regenerate after any
combat change (the server test fails while they are stale):

```bash
BATTLE_FIXTURES_WRITE=1 cargo test -p server battle_fixtures
```
