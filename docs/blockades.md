# Blockades

How a corporation blockades a rival's system, what each stage means, what
each side can do about it, and how freighters run a held blockade. The rules
live in `World::resolve_blockades` and `World::run_blockades`
(`crates/sim/src/world.rs`). Every surface reads them through
`client/src/core/derive/blockade.ts`.

## Ordering a blockade

- A Blockade order needs an interceptor in the fleet and a system another
  corporation owns. It also needs a declared war or a live right of reprisal,
  with no pact protecting the owner (non-aggression, ceasefire, separation
  window). The client refuses the order before sending and says why. The sim
  checks again when the order arrives and every tick after. If the war ends,
  a treaty arrives or the reprisal lapses, the fleet is told why and sails
  home. Pirates and the Authority act outside diplomacy.
- A fleet is on station within 140 su of the system. Once a blockade stands,
  its ships keep it anywhere within 740 su: the run zone and a margin. A
  blockader that runs a freighter down doesn't lift the blockade on its way
  back to station.

## Stages

- **Contested.** The system's defense still stands: its orbital station
  with any hardpoint firing (`docs/orbital-defense.md`), legacy platform
  tiers, the owner's warships in platform cover (1300 su), or stationed, fed
  allies whose doctrine fights. A station whose hardpoints are all knocked out
  is silenced and no longer contests. Only the establishment battle happens. Logistics run,
  nothing is stripped and no clock runs. If the blockaders retreat or are
  beaten off, they go home and the blockade lifts.
- **Held.** The defense is beaten. Deliveries wait outside on a ring 900 su
  out, and outbound shipping and standing orders hold at the origin. The
  Market Hub refuses bookings to and from the system, on its own light-delayed
  knowledge. The besieger's lowest-id fleet strips the stockpile at 1.5 units
  a second, never below 40 of any good; the Authority never strips. Guns on
  station bombard the garrison. If the defense comes back, it is contested
  again.
- **Siege.** Held, and not the owner's home: the capture clock runs. It takes
  the base time × (1 + 0.3 × the effective garrison), divided by 1.25 while a
  battleship-class hull of the besieger is on station. Once it ripens, marines
  landed by a separate, idle troop-transport fleet at the star can take the
  system. A home can be blockaded but never captured.
- The besieger is whoever holds the blockade. If all its ships leave, the
  blockade passes to the lowest-id fleet still on station, and the capture
  clock starts over.

## What each side sees and does

Both sides see the stage on the map badge (CONTESTED or CONTESTING, BLOCKADED
or BLOCKADING, SIEGE n%). The system panels, the inbox and the mobile sheets
use the same wording, saying what the stage means and what to do.

- **The besieger** can blockade (refused before sending, with the reason),
  lift the blockade (the recall, light-delayed), and choose whether its
  blockade also stops Authority freighters. The held brief shows how often
  its blockade catches a runner.
- **The owner** contests the blockade by sending warships or raising the
  system's station a level: the new level's hardpoints come online firing.
  A silenced station's hardpoints don't repair under a held blockade.
  Deliveries wait the blockade out unless they are set to run it.

## Running a held blockade

- A freighter fleet (freighters and no warships) runs a held blockade when it
  moves within 600 su of the system. That covers the owner's own freighters,
  and an ally's that the besieger may engage. The run zone lies inside the 900
  su ring where deliveries wait, so a waiting delivery never rolls. A fleet
  rolls once per crossing: going in and coming back out are separate runs,
  and a freighter parked at the berth doesn't roll.
- **The catch chance** comes from the besieger's ships on station that aren't
  already fighting. Each hull and each web rolls on its own: an interceptor
  30%, a corvette or destroyer 10%, a cruiser 5%, a battleship, dreadnought or
  titan 3%, and each Stasis Web 10%. Three interceptors catch 66% of runners;
  four battleships alone catch 12%, so a capital-only blockade leaks.
- **Warp Stabilizers** follow the tackle rule. With one on every hull of the
  runner, it shrugs off one interceptor: three interceptors then catch 51%,
  and a lone interceptor catches nothing. A freighter's one berth can take a
  stabilizer instead of a utility. That is a military module, so the refit
  needs an Ordnance Foundry.
- **Blockade Runners** research (Hulls, Corsair IV) halves what is left.
- **A caught freighter is raided** where it was caught, by the blockade's
  lowest-id free ship. It is a cargo raid with the usual short cap. A
  freighter that survives the raid sails on and delivers. The blockader
  returns to station either way.
- **The policy** is a standing per-fleet setting on every freighter fleet:
  Waiting held blockades out (the default) or Running held blockades. It is
  light-delayed to the fleet, like any configuration. A waiting delivery sits
  on the ring with its delivery open. Its status reads "Waiting outside the
  blockade of Vidar · running it: caught 66% · 51% with a Warp Stabilizer".
  Switching the policy to running sends it in.
- The pirate and Authority sentinels don't run freighters down.

## The waiting fix

A delivery that reached the standoff ring used to go idle, and its delivery
then resolved as if it had arrived. With storage room, its cargo landed
through the blockade from 900 su out. Without room, it dropped its delivery
and never resumed. It now waits on the ring, marked `Fleet::held_outside`,
with its delivery still open, and sails in when the blockade lifts or is
contested again.

## Events

`BlockadeContested`, `BlockadeEstablished`, `BlockadeLifted` (whether it had
held, and by whom), `BlockadeRun` (caught or not, and the chance it faced),
and `SystemPlundered` (one timeline line a minute, with the haul). Each
reaches each corporation on the light from the system, or from where the
freighter crossed.

## Tests

In `crates/sim/src/world.rs`:

- Stages: `a_contested_blockade_is_only_a_battle_until_the_defense_falls`,
  `a_blockader_beaten_off_goes_home_and_the_blockade_lifts`,
  `an_allied_garrison_keeps_the_blockade_contested`,
  `a_blockade_passes_to_the_besieger_still_on_station`,
  `a_battleship_speeds_the_siege_only_on_station`.
- Orders: `a_blockade_order_is_refused_with_a_reason`,
  `a_blockade_order_lapses_when_a_treaty_arrives_first`.
- Plunder: `the_authority_blockades_without_looting`.
- Running: `a_held_blockade_is_as_tight_as_its_interceptors`,
  `a_runner_rolls_once_and_a_miss_delivers`,
  `a_caught_runner_is_raided_and_the_blockade_holds`,
  `stabilizers_and_research_help_a_runner`,
  `sailing_out_is_a_run_and_sentinels_never_give_chase`,
  `set_run_blockades_sends_a_waiting_delivery_in`.
- Waiting: `a_waiting_delivery_unloads_only_after_the_lift`.

Client: `npm run test:blockade` (stages, badges, briefs, order refusals, run
odds, waiting status). Server timeline:
`a_blockade_run_reaches_both_corporations_by_light`.
