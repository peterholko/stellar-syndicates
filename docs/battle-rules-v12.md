# Battle rules v12: every gun fires

Superseded for new battles by v13 (`docs/battle-rules-v13.md`), which keeps
this gunnery and adds capital tracking and fortune. Battles opened on rules
v12 (`crates/sim/src/tactical/v12.rs`) keep it. It fixes
how guns are counted, how wide a battle line spreads, and two armour and
capital numbers that the balance matrix showed were off. Everything else
(v11's tackle, webs, light batteries and light guns, v10's formations, v9's
bodies, wreckage and slow turning, v8's weaving, v7's reach, every other
weapon, roll and range) is kept.

## In battle

- **One shot per weapon.** A hull fires one shot for every weapon it carries:
  each Mass Driver, Torpedo Rack or Prismatic Lance picks its own target,
  rolls to hit and for damage, and meets armour on its own. Before, a stack
  fired as one big shot (four drivers made one ×5.2 shot that either hit or
  missed). Expected damage against a big target is unchanged, with far less
  swing. Against small hulls it is a big change: a battleship with four
  drivers rolls four times a volley instead of once, so it kills small hulls
  about four times as fast instead of spending a whole broadside on one
  corvette. A stock or point-defense beam is still one gun, so a hull with a
  single weapon fires exactly as before.
- **A battle line holds no wider than its guns reach.** A line takes station
  where its shortest-ranged hulls can just fire. Its ends used to stretch far
  past their own reach: thirty corvettes facing a titan had about six in
  range. Now a hull takes a place in the line only where the enemy's centre
  is still within its weapon range from its slot (its slot sits no further
  from the centre than 0.6 of its reach, so a line of like hulls is at most
  1.2 reaches wide). Hulls that don't fit (the lightest go first) fight on
  their own: they close to their weapon band like any hull outside a
  formation. A heavy hull with a short reach may not fit beside another one at
  all. Two titans or two dreadnoughts with drivers each fight on their own.
- **Dreadnoughts hit harder:** attack weight 15, up from 12. It was the
  weakest capital per credit.
- **Armour blunts more:** Reflective Plating takes half of every beam hit
  (was 35%), light batteries included; Whipple Armor takes 60% of every
  driver hit (was 45%). Both still scale with the hull's protection affinity
  (cruisers ×1.2).

Capitals still get no secondary batteries: escorts answer swarms.

## Measured effect

The balance matrix (`docs/combat-balance.md`) against the v11 baseline, at
equal cost, 100 fights per pairing in each role (report:
`docs/balance/combat-balance-v12.html`). Design targets met: skirmish 11 of
15 (v11: 8), fleet action 11 of 16 (10), major battle 9 of 16 (10).

- **Counters work.** 26 of the 27 counter checks pass (v11: 22). Whipple
  brawler corvettes now beat driver corvettes (84–96%, except 51% in a major
  battle), and armored cruisers beat driver cruisers at every size (97–100%).
- **Interceptors are no longer the best buy.** Stock-beam interceptors fall
  from 71–75% against the field to 38–43%, and from 58–69% against line ships
  to 19–26%, below the 30–40% target. Interceptor drivers fall to 13–15%.
  Torpedo interceptors stay strong (72–76%).
- **Tackle now delays rather than holds** unless the tacklers can kill. Beam
  and web interceptors keep driver battleships and titans in the fight for
  130–370 rounds (42–65 against anything else), but the runners' separate
  driver shots clear them and every runner gets away (v11: 0–1% of the
  battleships and titans escaped beam interceptors). Torpedo interceptors
  still hold and sink everything but escorted battleships (0–2% escape).
- **The battle line fix.** Thirty driver corvettes facing a titan: about 26
  in range, against 6 under v11's line.
- **Still missed.** The strongest type scores 76–81% (torpedo cruisers in a
  skirmish, torpedo interceptors in a fleet action, the new dreadnought
  screen in a major battle). The mix holds five types in skirmishes and fleet
  actions but three in major battles. 86–87% of pairings are decided 90/10 or
  worse (v11: 75–81%), because more dice per volley means less luck. The
  capital ladder is out of band: battleships still lose to cruisers (torpedo
  cruisers beat every battleship fit, as under v11), dreadnoughts beat
  battleships 95–98% in skirmishes and fleet actions (torpedo dreadnoughts)
  but lose in major battles (15%: driver battleships beat both their driver
  and torpedo fits), and titans beat dreadnoughts 91–97%.

Measured one change at a time (fleet action, 40 fights per pairing in each
role, targets met out of 16): v11 10; one shot per weapon 9; plus the line
cap 10; plus dreadnought attack 15, 10; plus the new armour blunts, 11. One
shot per weapon moves the most: interceptor beams 75% → 41% against the field
and titan drivers 64% → 85%, and the equilibrium mix narrows to three types
until the line cap restores five.

## Tests re-earned

- `combat_balance::interceptors_hold_what_cruisers_cannot_catch_and_torpedoes_finish_it`:
  the hold-and-sink case uses torpedo interceptors instead of the wolfpack.
  Half of the wolfpack's budget goes on destroyers, and its interceptors no
  longer outlast a battleship's four separate driver shots.
- `build::capital_efficiency_peaks_at_destroyer_or_cruiser_and_declines_up`:
  attack 15 lifts the dreadnought's combat weight per Armaments (0.178) just
  past the battleship's (0.167). Both stay under the destroyer and cruiser
  peak, and the titan stays the least efficient. The test no longer requires
  battleship above dreadnought.
- v11's tests open on the frozen `open_v11`. v1's matrix test reads the
  frozen v1 blunts (`rules_v1::REFLECT_BLUNT`, `WHIPPLE_BLUNT`). The
  active-version check compares the live catalogue with v12's attack weights
  and blunts.

## Test fights

`client/public/dev/battles/lockdown` now sends sixteen torpedo interceptors
with the eight webbing ones (it was ten): with one shot per driver, the
battleships kill interceptors faster, and ten no longer sink the held line.
Regenerate after any combat change:

```bash
BATTLE_FIXTURES_WRITE=1 cargo test -p server battle_fixtures
```
