# Reactors and warp drives

Status: BUILT Nov 7 2026 on `main`, uncommitted; the galaxy on :8080 was
reset for it. Replaces the one-part warp core of
docs/shipbuilding-overhaul-plan.md §1 and adds the research gate the Oct 28
rule asks for. Drive icons are placeholders (prompts in
docs/shipbuilding-art-prompts.md); the reactor icons reuse the core masters.

Peter (Nov 7 2026): "the player should not even know about the other fuel
types until it has been unlocked from research"; "I want 4 levels of warp
drives. Each level requiring higher levels of power output hence requiring a
better reactor. The player could run a fusion reactor with a warp drive level
1"; names "more creative" than level 1–4.

## The parts

An engine is a **reactor** feeding a **warp drive**. A hull carries
`ShipKind::engines()` of each: Interceptor 1 + 1, Tiny Freighter 2 + 2, Mining
Barge 3 + 3, Titan 74 + 74. The reactor sets the fuel, the power and the
energy aboard; the drive sets the speed and what the hull can do. **A drive
needs a reactor of at least its rung** (the power rule); a better reactor runs
any lower drive.

Reactors, one family for ships and plants:

| Rung | Reactor | Fuel | Energy a unit | Energy aboard | Hub | Known after |
|---|---|---|---|---|---|---|
| 1 | Fission Reactor | Uranium | 1 | ×1 | 145 | start |
| 2 | Fusion Reactor | Deuterium | 4 | ×2 | 310 | Bunkerage (Propulsion I) |
| 3 | Advanced Fusion Reactor | Helium-3 | 24 | ×4 | 450 | Helium-3 Cycle |
| 4 | Antimatter Reactor | Antimatter | 96 | ×8 | 570 | Antimatter Synthesis |

Warp drives:

| Rung | Drive | Speed | Energy a distance | Brings | Hub | Known after |
|---|---|---|---|---|---|---|
| 1 | Zephyr Drive | ×1.0 | ×1.0 | Slow and Full | 70 | start |
| 2 | Gale Drive | ×1.2 | ×1.44 | Stealth | 120 | Bunkerage |
| 3 | Tempest Drive | ×1.4 | ×1.96 | | 200 | Helium-3 Cycle |
| 4 | Maelstrom Drive | ×1.6 | ×2.56 | the jump | 330 | Antimatter Synthesis |

The names (Peter, Nov 7 2026: "something more creative" than level 1–4, and
not the first set, Warp / Slipstream / Tachyon / Fold): the winds of the
age of sail the charter, privateer and blockade lore already borrow, rising
in force, with the vortex for the jump. Icons: one reactor body and one drive body, four
fuel tints each (amber, blue-white, gold, violet).

Energy a distance follows the law the throttle already uses (Slow is half the
speed for a quarter of the burn), so a faster drive is a hungrier one, and
"needs more power" is literal. Energy aboard doubles a rung, so a matched pair
gains both speed and range, and a mismatch trades one for the other:

| Pair | Speed | Full range | Fuel a 1,000 mass·su | Cr a 1,000 mass·su |
|---|---|---|---|---|
| Fission + Zephyr (the charter) | ×1.0 | 1.0 | 1.0 Uranium | 5.6 |
| Fusion + Zephyr (Peter's example) | ×1.0 | 2.0 | 0.25 Deuterium | 3.5 |
| Fusion + Gale | ×1.2 | 1.4 | 0.36 Deuterium | 5.0 |
| Advanced Fusion + Tempest | ×1.4 | 2.0 | 0.08 Helium-3 | 6.1 |
| Antimatter + Maelstrom | ×1.6 | 3.1 | 0.03 Antimatter | 3.2 |

Range 1.0 is today's Fission hull. A Fusion Reactor on a Zephyr Drive is the
cheap long-legged hauler; the matched pairs buy speed for about the same
credits a distance. Numbers are starting values.

Rules: an engine costs what a core costs today (215 / 430 / 650 / 900 for
the matched pairs). A fleet flies at its slowest hull. Stealth needs a
Gale or better on every hull; a jump needs a Maelstrom Drive on every hull.
Ships of the line mount up to a Tempest Drive, any reactor. Research speed
and burn modifiers (Drive Tuning, Efficient Burns, Torch Regime, …) multiply
on top. `RefitEngines` swaps reactors and/or drives at an owned or allied
Shipyard under the power rule; the old parts return to the stockpile.
Scripted spawns: Raider and Scout on Antimatter + Maelstrom, everything else
on Fusion + Zephyr; the privateer's and the raid's tuned speed constants are
re-derived so their measured timings hold.

Goods: Fission Core → **Fission Reactor**, Fusion Core → **Fusion Reactor**
(merged into the plant good), Helium Core → **Advanced Fusion Reactor**
(merged), Fold Core → **Antimatter Reactor**, plus the four drives. Drive
Works products: eight (reactor and drive of each rung; rungs 1–2 at tiers
I–II, 3 after Helium-3 Cycle, 4 after Antimatter Synthesis). Plants: Fusion
Plant III–IV one Fusion Reactor, V–VI one Advanced Fusion Reactor, as today;
the Antimatter Plant one Antimatter Reactor + 20 Exotic Matter (was two Fusion
Reactors).

## Hidden until researched

Peter (Oct 28 2026): nothing a research has not unlocked is rendered outside
the Research screen. Applied to engines and fuels, and enforced in the sim:

- `known(good)`: rung 1 and Uranium always; rung 2 and Deuterium after
  Bunkerage; rung 3 and Helium-3 after Helium-3 Cycle; rung 4 and Antimatter
  after Antimatter Synthesis.
- Sim: `BuildShip` and `RefitEngines` with an unknown part are refused;
  `MarketBuy` of an unknown good is refused; the trader stocks none. NPCs are
  exempt.
- Client: Hub book rows, Stores pins, the Drive Works product list, the
  Shipyard chips and the fleet panel filter on `known`. The Shipyard shows a
  Reactor chip row and a Drive chip row only once a second rung is known;
  until then the cost list alone ("Fission Reactor 2 · Zephyr Drive 2"). A drive
  the chosen reactor cannot power is not offered.
- Power Plant III–IV (Deuterium) unlock with Bunkerage; today they need no
  research. Solar I–II stay free, V–VI stay on Helium-3 Cycle. Otherwise a
  plant would show whose reactor and fuel are hidden.
- The trader's founding stock drops its 80 Deuterium: nothing in the charter
  burns it.

## Charter

> §hull-ladder (docs/hull-ladder-plan.md §5, later on Nov 7 2026) supersedes
> the charter bullets below: the kit carries six reactors and SEVEN drives, the
> raid takes the drives too, the trader sells four reactors, three drives and
> the Small Freighter's kit, the den's loot is 100 units in two loads, and the
> den contract pays 8,000.

- Landfall crate: six Fission Reactors + ten Zephyr Drives, one for every
  charter hull's engine (Interceptor 1, Tiny 2, Small Freighter 4, barge 3),
  so no drive is ever a Hub import (was six Fission Cores).
- The raid takes the reactors (`RAID_TAKES`: Fission Reactor, Semiconductors,
  Weapon Mounts); the drives stay, useless without them. Card: "Lost: 6 Fission
  Reactors, 20 Semiconductors, 5 Weapon Mounts".
- Trader: three Fission Reactors, the first featured at half the Hub mid
  (~72 Cr), the Tiny Freighter's pair at full price from the bounty.
- Den: the six reactors; loot stays 97 units, two loads. `STASH_LOAD_ORDER`
  swaps the good.
- Reach: the barge's three reactors are the den's; its three drives landed
  with the colony.
- Bots build on rung 1 and buy the trader's reactor in the opening.
  Refounding seeds 20 reactors + 20 drives.

## Work, in order

A. Sim goods and parts: cargo, fuel, ship, build recipes, world build / refit /
   jump / stealth checks, speed and burn by drive, energy aboard by reactor,
   Drive Works products, plants, market prices, the `known` gate on build,
   refit and buy, NPC speed constants. Needs a galaxy reset.
B. Charter: founding, merchant, story cards, tutorial copy, refounding, bot
   opening.
C. Client: protocol, derive/fleet, picker, fleet panel, Hub book, stores,
   icons (reactor placeholders from the core tints, four drive placeholders,
   prompts in docs/shipbuilding-art-prompts.md).
D. Suites, client build, docs, `--reset-galaxy` on :8080.

Go/no-go after A + B: sim and server suites green (the pre-existing captain
caps failure allowed), raid and privateer timing tests unchanged. After D: a
fresh charter's Shipyard shows one cost line and no chips.

Found by the bot charter harness while landing this: the bots' docked-trader
trades held the Hub market slot for a light round trip, so a one-Semiconductor
top-up every visit starved the Uranium imports and the charter overran its 80
minutes; `spend_at_home` keeps the slot free. The kit carries ten Zephyr
Drives so no drive is a Hub import (the first cut carried six and the Small
Freighter's fourth cost a quarter hour of freight).

## Calls made here, overrule freely

1. One reactor per drive, not one per hull: every hull's engine bill stays at
   today's figure and the raid's "six" stays six.
2. Drive rungs buy speed (×1.0 / 1.2 / 1.4 / 1.6) at the throttle's quadratic
   burn; Stealth from rung 2, the jump at rung 4. The top stops where the
   light-game invariant does: a Scout on a Maelstrom (184 su/s) keeps light
   (400 su/s) at the 2× floor the information game needs.
3. Energy aboard doubles a reactor rung, so the ladder gains range as well as
   speed (today every grade has the same range).
4. Each rung becomes known with one programme, reactor and drive together, so
   no part is ever shown that nothing aboard can power.
5. Ships and plants share one reactor family; the raid takes reactors only.
6. Power Plant III–IV move behind Bunkerage.
7. Matched-pair prices equal today's core prices; the Hub sells only known
   parts.
9. Detection stays in Raiders-at-rated-speed: a drive's speed bonus makes a
   hull no louder (`Fleet::core_output` divides the drive out), so sensor
   ranges and the raid's warning windows hold. The privateer constants
   (`PRIVATEER_SPEED_MULT`, `FOLLOW_UP_PIRATE_SPEED_MULT`) are divided by the
   Maelstrom's 1.6 so the measured chases hold; a scripted Raider's approach
   otherwise reads 1.6× faster (the guide's "Raider closing" ETA).
8. Internal names follow: `CoreGrade` → `ReactorGrade`, `Hull.core` →
   `reactor`, new `Hull.drive: DriveRung`; wire fields too (the client is
   ours).
