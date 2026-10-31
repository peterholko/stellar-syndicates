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
- A fleet is on station within 140 su of the besieged world: §real-coordinates
  (Oct 20 2026), blockaders hold over the system's main world and ride it.
  Once a blockade stands, its ships keep it anywhere within 1,040 su of the
  star: the run zone and a margin. A blockader that runs a freighter down
  doesn't lift the blockade on its way back to station. Establishment fights
  open where the blockaders hold.

## Raids

Peter (Oct 28 2026): the Raid order, a blockade that comes home
(`FleetOrder::Blockade { raid: true }`, `World::raid_turnarounds`).

- An interceptor and a hold in the fleet: loot only rides in holds, so the
  sim refuses a raid without one (`needs_hold`).
- **No war needed** (Peter, Oct 28 2026: "why does a raid require a war? That
  seems problematic"). A raid is theft, not conquest, so it works like a convoy
  raid: legal against any corporation not covered by founder protection,
  Leave, a pact or a syndicate. When it takes station, its victim gets the
  12 h right of reprisal against the raider (`record_neutral_aggression`), so
  it can raid or blockade back. Without a war the order preview offers Raid
  alone. Blockades (cutting logistics, sieges, capture) still need a war.
- A raid never runs the capture clock, even while it holds a colony.
- It fights for and holds the system like a blockade, and loots at the same
  0.5 units a second. It turns for home when its holds are full, when the
  blockade is contested with loot aboard, or when nothing is left in reach,
  then unloads at home (`TradeMission::DeliverToSystem`, as a haul does).
- Both shells offer Raid beside Blockade in the order preview when every fleet
  in it has a hold. The fleet reads "Raiding Vidar · held · 46/80" and its
  recall is "Lift raid".
- §player-styles (Oct 31 2026, [player-styles.md](player-styles.md)): a raid
  or blockade takes at most a quarter of the victim's storage a day (not once
  its owner has been away three days), its victim is warned when its sensors
  see it coming, and "Raid ↻" repeats a raid after each trip home.

## The Vault

Peter (Oct 28 2026), on Travian's cranny: it only "makes sense logically" as a
place raiders can't reach. A Vault (`StructureKind::Vault`) is a bunker dug
under a world's Command Center: one per world, no slot, no crew, no power,
open from the charter's step 22, "Secure your stores" (§charter-crates, Oct 31
2026; before, once the charter completed).

- Each tier holds 200 units, filled with the system's most valuable goods
  first, the order raiders take them in. The settlers also keep 40 Provisions
  outside it (`raid_reserve`, `RATIONS_UNITS`). This replaced the flat floor of
  40 per good.
- Blockade looting, pirate home raids and the vendetta raid never take it.
  Siege capture still hands over the whole stockpile.
- Stores shows the Vault's room in its header and each good's safe units.
- Pirate home raids now hold orbit for 3 minutes (was 90 s) so they can still
  load their 60 units at the slower rate.

## Stages

- **Contested.** The system's defense still stands: its orbital station
  with any hardpoint firing (`docs/orbital-defense.md`), legacy platform
  tiers, the owner's warships in platform cover (1300 su), or stationed, fed
  allies whose doctrine fights. A station whose hardpoints are all knocked out
  is silenced and no longer contests. Only the establishment battle happens. Logistics run,
  nothing is stripped and no clock runs. If the blockaders retreat or are
  beaten off, they go home and the blockade lifts.
- **Held.** The defense is beaten. Deliveries wait outside on a ring 1,200 su
  out (outside the gravity well), and outbound shipping and standing orders hold at the origin. The
  Market Hub refuses bookings to and from the system, on its own light-delayed
  knowledge. The besieger strips the stockpile at 0.5 units a second
  (§vault, Oct 28 2026: raiders need time to load, so a defender has minutes
  to answer), most valuable good first, never what the Vault holds or the
  settlers' last 40 Provisions (see "The Vault"); the Authority never strips.
  Goods only ride in holds (Peter, Oct 27 2026: "force the player to bring
  along a freighter to loot"): the loot goes into the freighters in the
  besieger's fleets on station, which fill in id order; warships carry none.
  Pirate fleets are rigged for plunder and carry up to 60 a fleet. (Before:
  60 a fleet, warships included, and only the lowest-id fleet loaded.) Prize
  Holds research strips 50% faster and widens Combat Freighter holds by 50%.
  Guns on station bombard the garrison. If the defense comes back, it is contested
  again.
- **Siege.** Held, and not the owner's home: the capture clock runs. It takes
  the base time (`SimConfig::siege_capture_secs`, 6 h since Oct 22 2026; see
  docs/retention-p0.md) × (1 + 0.3 × the effective garrison), divided by 1.25 while a
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
- **A pirate raid on a home** (§raid-feedback, Peter, Oct 27 2026: "very
  little feedback on what impact the raider had"):
  - The badge reads PIRATE RAID and the brief says what it costs: up to 10%
    of each good, 60 units in all (mirrors `pirate::HOME_RAID_STOCK_FRAC` and
    `HOME_RAID_MAX_LOOT`). Deliveries wait until the pirates leave.
  - Toasts in both shells: "Raid inbound · Nergal · 2 raiders · ETA ~4m"
    (the warning, also an Inbox row with Defend), "Raid · Nergal" as they
    start (attacking, or raiding once they hold it), and "Raided · Nergal ·
    Took 46 Ferrite Ore · 14 Provisions" (or "Nothing taken") as they leave.
  - Since the Log overhaul (protocol v71, docs/log.md) the toasts come from
    the journal line's kind and phase (`raid:{system}` group), each once, and
    the whole raid is one History row whose report prices the take. A pirate
    raid no longer posts a green "LIFTED" line.

## Running a held blockade

- A freighter fleet (freighters and no warships) runs a held blockade when it
  moves within the system's gravity well (900 su), so one bound for an outer
  world crosses it too. That covers the owner's own freighters, and an ally's
  that the besieger may engage. The run zone lies inside the 1,200 su ring
  where deliveries wait, so a waiting delivery never rolls. A fleet
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
- **What a raid keeps** (every cargo raid, §holds): the raiders' own
  freighters take what they have room for; the rest sinks with the wreck. With
  Boarding Parties the captured freighters keep their cargo aboard as the
  prize. Pirates carry it all.
- **The Combat Freighter** (§holds, Oct 27 2026; Peter named it) is the
  raiding freighter:

  | | Combat Freighter | Tiny Freighter | Interceptor |
  |---|---|---|---|
  | Speed | 70 | 40 | 100 |
  | Hold | 80, loot only | 50 | none |
  | Hull (mass) | 2,000 | 1,500 | 200 |
  | Attack / Defense | 0 / 2 | 0 / 1 | 3 / 2 |
  | Slots | 1 utility | 1 utility | 2 |
  | Yard | Shipyard II | Shipyard I | Shipyard I |

  Recipe 25 Alloys · 10 Machinery · 6 Semiconductors · 5 Armaments · 1 Warp
  Core, 20 s, no research. Its hold is **loot only**
  (`ShipKind::is_loot_only`): plunder, seized cargo and finds (salvage,
  signals, discoveries) fill it; a dock, a market, a standing order, a route,
  mining and station loads never do (`Fleet::trade_capacity` leaves it out;
  `Fleet::cargo_capacity` counts it). Cargo is pooled, so loot aboard takes a
  mixed fleet's trade room too. It unloads at home or the Hub like any hull.
  Prize Holds makes the hold 120; Cargo Pods double it. It doesn't broadcast
  and has no sensors. It is raided like any freighter
  (`ShipKind::is_freighter_hull`): a lone one is a civilian contact, its loot
  changes hands, Boarding Parties can take it, and pirates hunt it. A raid
  fleet is drawn as its Interceptors. Art: its own hull since Oct 27 2026
  (wedge bow, armoured hold, four grapple arms, three engines;
  docs/ship-model-integration.md).
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
