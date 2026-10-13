# Combat balance matrix

A measurement harness for fleet balance: every fleet type against every other
at equal cost, fought in both roles on the live battle engine. It changes no
rule and nothing in the game reads it. Use it before and after any combat
change to see what the change does to every matchup, and to agree on targets
before tuning.

```bash
cargo run --release -p sim --example combat_balance
```

It writes `docs/balance/combat-balance-v<rules>.json` (the numbers) and
`.html` (the report: one self-contained page that opens in any browser and can
be published as is).

| Flag | Default | |
|---|---|---|
| `--seeds N` | 100 | Fights per pairing in each role (200 per pairing in all, about ±7 points at 50%) |
| `--tiers a,b` | `skirmish,fleet,major` | Fight sizes to run |
| `--only key,key` | all | Archetypes to include (keys below) |
| `--threads N` | all cores | |
| `--out DIR` | `docs/balance` | |
| `--baseline FILE` | none | An earlier report's json: the page adds a "Change vs baseline" view |
| `--render FILE` | | Rebuild the page (and the derived summaries) from a saved json without fighting again |

The full run is about 248,000 fights plus the catch-and-hold cells: 20 minutes
on 8 cores under rules v13 (21 under v12, 13 under v11, 9 under v10). Skirmishes and
fleet actions take about a minute; the major-battle tier, with swarms of up to
200 hulls a side, takes most of the time (under v12 more of those hulls fight
outside a formation, and each looks for its nearest enemy every round).

## What it measures

**Cost.** Every hull and module is priced in credits: its build recipe at the
reference market prices (`market::base_price`), the same reference the
capital recipes are pinned to. Yard time and crew upkeep are reported beside
it but not matched on. `sim::combat_balance::{hull_value, module_value,
ship_value, hull_build_hours, upkeep_per_day}`.

**Fleet types** (`combat_balance::ARCHETYPES`):

- Every warship hull with each weapon at full weight: Interceptors, Corvettes,
  Destroyers, Cruisers, Battleships, Dreadnoughts and Titans, each with stock
  beams (no module), mass drivers or torpedo racks. A weapon fit carries as
  many launchers as the hull's slots and fitting points allow, because copies
  stack (a cruiser takes four racks, a titan six drivers).
- Five fleets built around a counter or a mix: a torpedo wolfpack (torpedo
  destroyers and interceptors), screened battleships (driver battleships with
  point-defense corvettes), armored cruisers (beams with Reflective and
  Whipple armor), brawler corvettes (a driver and Whipple armor) and a battle
  group (driver battleships, beam cruisers, PD corvettes and interceptors).

- Rules v11 added three: escorted battleships (driver battleships with
  destroyer escorts carrying webs), stabilized battleships (three drivers and
  a Warp Stabilizer) and webbing interceptors (stock beams and a web).
- Rules v12 added the dreadnought screen: dreadnoughts with point defense and
  four drivers, whose heavy screens cover driver battleships.

Keys are `interceptor_beam` … `titan_torpedo`, `wolfpack`,
`screened_battleships`, `armoured_cruisers`, `brawler_corvettes`,
`escorted_battleships`, `stabilized_battleships`, `web_interceptors`,
`dreadnought_screen`, `battle_group`. Seeds and baselines key on them: never
reuse a key for a different fleet.

**Catch and hold** (`combat_balance::{RUNNERS, HUNTERS, escape}`): each runner
withdraws from the first round while each hunter closes, at equal cost, and
the page shows how much of the runner's value gets away (past the edge of the
fight, alive), is held at the cap, or is lost. This is where tackle shows: the
stand-up matrix never withdraws.

**Fight sizes** (`combat_balance::TIERS`): a skirmish at 55,000 credits a side
(two battleships or one dreadnought; no titan), a fleet action at 115,000 (one
titan) and a major battle at 340,000 (three titans, about 180 interceptors).
Each fleet type is built in whole hulls, dearest hull first with the rounding
carried into the next part, and sits a size out if it cannot come within 15%
of the budget.

**Fights.** Each pairing is fought `--seeds` times with each side attacking,
on the kernel the in-game battle estimate uses (`tactical::simulate_engagement`,
current rules): one fleet a side, open space, no platforms, no research
modifiers, default missions (so Automatic formations), to the death or the
600-round cap.

**The table**, per fight size (`combat_balance::table`):

- *Win share*: the row's share of the points against the column over both
  roles (a win 1, a mutual kill or an undecided fight ½), with a 95% Wilson
  interval. Constant-sum: row against column plus column against row is 100%.
- *Value trade*: the row's share of all the value destroyed between the two.
- *Attacking wins*, *fight length* (median rounds) and *undecided* (share of
  fights still going at the cap).
- *Field score*: the average win share against every other type.
- *Equilibrium mix*: the matchup table solved as a two-player game (each side
  picks a fleet type without seeing the other's), exactly, by simplex. A type
  outside the mix is beaten by some blend of the others whatever the opponent
  builds. A healthy table has several types in the mix.
- *Hull games*: hull class against hull class, each side choosing its best
  weapon fit: the value of the game over their fits.

## Targets

The agreed design goals (Oct 3 2026), checked at every fight size
(`combat_balance::targets`, constants beside it) and shown on the page and in
the terminal as met or missed. They are computed from saved numbers, so
`--render` adds them to an older report.

| Target | Goal |
|---|---|
| Counters (`COUNTERS`): screened battleships over the wolfpack, armored cruisers over beam and driver cruisers, brawler corvettes over driver corvettes, beam destroyers and escorted battleships over beam interceptors, beam interceptors over torpedo interceptors, the wolfpack and torpedo interceptors over driver battleships | ≥ 70% of the wins and ≥ 60% of the value traded |
| Strongest fleet type | ≤ 70% against the field |
| Equilibrium mix | ≥ 5 types at 1% or more |
| Pairings decided 90/10 or worse | ≤ 40% |
| Fights still going at the 600-round cap | ≤ 10% |
| Capital ladder: titans over dreadnoughts over battleships over cruisers (hull games) | 55–70% each |

## Reading the report

The page opens with findings for the chosen fight size (strongest type, the
equilibrium mix, how many pairings are decided 90/10 or worse, types not worth
building, role advantage in mirror matches, fight length and stalemates) and
the targets, then answers the two design questions side by side: weapons on
the same hull, and hull classes against each other. Below are the standings,
every matchup (hover or tap a square for the fights behind it) and the cost
ledger.

## Limits

- Open-space fleet battles only: no platforms, sieges, raids, retreats or
  research. Build time and upkeep are not matched on, and they are large
  levers: interceptors build in seconds and eat their cost in food in about
  five days, capitals take days to build and eat theirs in about one.
- One fleet a side, default missions. A formation or target priority chosen
  for the matchup can do better than Automatic.
- Whole hulls: a fleet type can sit up to 15% off the budget (the standings
  show each fleet's actual cost).

## First run: rules v10 (Oct 3 2026)

`docs/balance/combat-balance-v10.json` is the baseline for the next rules
version (`--baseline`). What it shows, at every fight size:

- Stock-beam interceptors beat every other fleet type (field score 100%), and
  the equilibrium mix is that one type. Stock-beam corvettes are second
  (91–93%). 76–81% of pairings are decided 90/10 or worse.
- The best weapon flips with hull size. On interceptors, corvettes and
  destroyers, stock beams beat stacked drivers and torpedoes (76–100%): a
  module adds 40–60% to a light hull's price, so beams field more hulls. On
  cruisers and up, beams lose to both (0–4%): modules add 4–12% to a big
  hull's price and stack four to six times.
- Most counters work where they are meant to: a torpedo wolfpack beats driver
  battleships (100%), point-defense screens beat the wolfpack (84–100%), and
  armored cruisers beat beam and driver cruisers (83–100% from the fleet
  action up) and lose to torpedo cruisers (0%). Screens and armor cost
  firepower, though: screened battleships lose to plain driver battleships
  (2–36%), and Whipple armor does not answer drivers, since brawler corvettes
  (one driver and Whipple) lose to corvettes with two drivers (0–26%).
- Bigger is not better per credit: interceptors and corvettes beat every
  heavier class; among capitals, titans beat battleships and dreadnoughts.

## Rules v11 (Oct 3 2026)

`docs/balance/combat-balance-v11.json`, run with `--baseline` against v10
(the page's "Change vs baseline" view). Interceptor tackle, webs, destroyer
light batteries and light guns on small hulls (`docs/battle-rules-v11.md`):
the best field score falls from 100% to 75–79%, the equilibrium mix grows from
one type to 3–7, interceptors lose to beam-armed, armored and escorted lines
but still beat ships that can't track them, and 75–81% of pairings are still
decided 90/10 or worse.

## Rules v12 (Oct 3 2026)

`docs/balance/combat-balance-v12.json`, run with `--baseline` against v11.
One shot per weapon, battle lines no wider than their reach, dreadnought
attack 15, Reflective 50% and Whipple 60% (`docs/battle-rules-v12.md`).

- Design targets met: skirmish 11 of 15 (v11: 8), fleet action 11 of 16
  (10), major battle 9 of 16 (10). Counters: 26 of 27 checks pass.
- Interceptor beams fall from 71–75% against the field to 38–43%, and to
  19–26% against line ships (target 30–40%). Tackle by beam or web
  interceptors now delays a fleeing battleship or titan (130–370 rounds)
  instead of holding it; torpedo interceptors still hold and sink.
- Missed: the strongest type scores 76–81% (torpedo cruisers, torpedo
  interceptors, the dreadnought screen), 86–87% of pairings go 90/10 or
  worse, and the capital ladder is out of band (battleships lose to torpedo
  cruisers, as under v11; dreadnoughts beat battleships 95–98% in the two
  smaller sizes but 15% in a major battle; titans beat dreadnoughts 91–97%).
  See `docs/battle-rules-v12.md` for the one-change-at-a-time measurements.

## Rules v13 (Oct 4 2026)

`docs/balance/combat-balance-v13.json`, run with `--baseline` against v12.
Capital guns track small hulls poorly (×0.75) and tacklers worst of all
(×0.15); each side's fortune scales its damage by 60–140% for the battle
(`docs/battle-rules-v13.md`, which also has the sweeps behind both numbers).
Fortune means every fight's result is now partly luck, which is the point:
read win shares as odds.

- Targets met: skirmish 10 of 15, fleet action 11 of 16, major battle 10 of
  16 (v12: 11, 11, 9).
- Tackle holds and sinks unescorted capitals (0–8% get away from beam or web
  interceptors; v12 100%); escorted battleships and beam cruisers get away.
- Interceptor beams 30–37% against line ships (target 30–40%; v12 19–26%).
- Pairings decided 90/10 or worse: 60–63% (v12 86–87%); among types scoring at
  least 45% against the field, 44–49%. Pairings within 35–65%: 11–14%.
- Mix 6, 5 and 5 types; strongest type 75–78%; titans over dreadnoughts in
  band in a fleet action (69%). Missed: brawler corvettes over driver
  corvettes falls to 58–60%, battleships still lose to torpedo cruisers
  (23–38%), dreadnoughts over battleships is 75–79% in the two smaller sizes
  and 34% in a major battle.
