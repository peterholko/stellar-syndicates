# Battle rules v9: hulls are bodies, large wrecks are real, big ships turn slowly

Superseded for new battles by v10 (`docs/battle-rules-v10.md`), which keeps all
of this and adds fleet formations. Battles opened on rules v9
(`crates/sim/src/tactical/v9.rs`) keep it. Three changes,
everything else (v8's weaving, v7's reach, every weapon, roll and range) kept:

1. **Hulls keep clear of each other.** Every hull has a body radius of about
   30% of its drawn length in the battle viewer (an interceptor 13 arena
   units, a cruiser 50, a titan 101; `body_radius`), so hulls may brush at the
   tips but never stack. Hulls steer away from any body within 1.6× contact
   distance, the lighter one giving way more; after moving, any remaining
   overlap is pushed apart the same way (four passes). Defense platforms never
   move. Hull against hull does no damage.
2. **Large hulls leave physical wreckage.** A hull of 8000+ mass (battleship,
   dreadnought, titan, and the large, heavy and bulk freighters) that dies
   leaves its sections on the field: 3, 4 or 5 of them, laid end to end along
   its course (the same counts the viewer breaks it into). They move with the
   dead hull's momentum, fly apart a step later (a titan two), and slow with
   drag (`WRECK_DRAG` 0.8 a step). Hulls steer around them. A torpedo whose
   path this step crosses a section detonates on it (a new torpedo ending,
   `wreck`), so wreckage is cover against torpedo strikes. A hull that still
   runs into a section takes damage by its closing speed: up to half its hull
   at 100 units a step, scaling with the square of the speed, nothing below
   4 (a scrape). Both lose their closing speed. Beams and mass drivers still
   fire through wreckage. Past 8 wrecked hulls, the oldest wreck is removed.
   Hulls never aim into wreckage: a desired point inside a section's keep-out
   zone moves to its edge, and a straight run through a zone aims past its
   edge instead, so hulls go around. The zone grows with the hull's stopping
   distance, so a fast capital starts its turn early. A wreck still breaking
   up shoulders neighbours aside without damage; after that, contact is a ram.
3. **Big ships turn slowly.** A hull heavier than a destroyer can swing its
   course by at most its turn rate a step (`turn_rate`): a cruiser 0.30 rad,
   a battleship 0.20, a dreadnought 0.13, a titan 0.085 (about 37 steps to
   reverse), and the big civilian hulls on the same ladder. Asked to turn
   harder, it keeps the asked-for speed on the capped course, so it brakes
   as it comes about. Interceptors, corvettes and destroyers turn as freely as
   before. v8's weave circles widen to fit (a battleship's radius is about
   125 units, a titan's about 180). The battle viewer turns each nose at the
   same rates, so noses follow their courses exactly.

The cost is small: close pairs are found with a sweep along x (near-linear),
and there are at most 40 wreck sections, so even the battle estimates, which
simulate whole battles many times, stay fast.

## Recorded for the viewer

Each round records every wreck section (`wrecks`: hull, section, position,
radius) and every ram (`rams`: hull, section, contact point, damage). Each
death now names the hull that died (`cid`). The battle viewer draws a large
wreck exactly where the engine has it: the hull stays whole until the engine
breaks it, then each section moves as recorded, with its tumble, char and
fires drawn on top. A ram is the rammed hull's evidenced impact, with a flash
at the contact point. Wrecks of lighter hulls stay a viewer effect.

## Measured effect

Against frozen v8 (`cargo test -p sim --lib balance_probe_v9 -- --ignored
--nocapture`, 32 rollouts each). Slow turning moves some matchups a lot:

- 2 cruisers against a battleship: the attackers win 47% (88% in v8).
- 8 mass-driver interceptors against 2 cruisers win 66% (44%).
- A 5-cruiser mirror: the attackers win 66% (41%); a 10-interceptor mirror
  38% (59%), and it ends sooner (54 → 36 steps).
- Raids on corvettes win 25% (12%).
- A titan with corvettes against three battleships runs 118 → 169 steps;
  a torpedo strike on cruisers and a dreadnought 226 → 171.
- Most other matchups are within noise.

Two tests were re-earned: the Whipple armor test compares the first volley a
mass driver lands, read from the shot evidence (another hull's beam can land
first), and the hull-spacing test allows four separation passes.

## Test fights

The engine fights in `client/public/dev/battles/` are recorded on v9.
"Collisions in a wreck field" (`collision`) is built to show them: a crippled
dreadnought dies early (wreck at round 16), battleships bump its sections
(rounds 18 and 21), interceptors clip them at speed (rounds 21 and 42), and
six torpedoes detonate on the wreckage (rounds 24 and 29). Fights can start
with crippled hulls (`crippled` in `crates/server/src/battle_fixtures.rs`).
Regenerate after any combat change (the server test fails while they are
stale):

```bash
BATTLE_FIXTURES_WRITE=1 cargo test -p server battle_fixtures
```
