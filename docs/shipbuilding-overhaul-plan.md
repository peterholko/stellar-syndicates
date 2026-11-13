# Shipbuilding overhaul — plan

Nov 6 2026, revised the same night with weapons, munitions and the Armaments
rename. Agreed in discussion between Peter and Fable; **built Nov 7 2026**
(Phases 1–5, see Status). Credits, the price layer (levy / bill-of-materials /
K) and fleet upkeep are deliberately out of scope here: "don't worry about the
credits right now". They land on top of this once the harness baseline exists.
Exact numbers in this document are starting values, not decisions.

## Status (Nov 7 2026)

**Nov 7 2026, later:** the one-part warp core was split into a reactor and a
warp drive (Zephyr / Gale / Tempest / Maelstrom, each needing a reactor of its
rung), with every rung hidden and unbuyable until researched: see
docs/reactor-drive-split.md (built). §1's cores, Status items 1, 2 and 7 and
§11's "the Authority sells all four cores from day one" are superseded by it.

Phases 1–5 are implemented on `main`, uncommitted, with placeholder icons for
every new or renamed good (`client/public/art/goods-placeholder/`, prompts for
the masters in docs/shipbuilding-art-prompts.md). The galaxy on :8080 was reset
for the Phase 3 charter kit. Phase 6 (modules as goods, credits and upkeep)
stays deferred. Calls made while building, where the sections below say
otherwise:

1. **Tanks hold energy, not units.** A grade's tank per hull mass is
   `FUEL_PER_HULL_MASS / energy_per_unit` (Fold at least its jump bottle), so
   every grade gives the same range per unit of hull mass and a better grade
   simply burns fewer, dearer units. §1's per-unit tank sizes are superseded.
2. **Default cores on scripted spawns:** Raiders and Scouts spawn on Fold
   (NPC jumps keep working), everything else on Fusion; yard builds pick the
   grade (`BuildShip.core`), and `RefitCores` swaps them at an owned or allied
   Shipyard. Ships of the line never mount Fold.
3. **Survey Probe carries no Nav Computer**, keeping it under half a Scout.
4. **Capital Weapon Mounts skins raised** so the efficiency invariant and the
   balance tiers hold: Destroyer 30, Cruiser 55, Battleship 120, Dreadnought
   300, Titan 540 (hull values 5,906 / 10,126 / 22,940 / 49,340 / 97,980).
5. **rules_v1 Destroyer attack weight 2.4 → 2.5** (1 Beam Battery + 1 Pulse
   Laser); the tactical fingerprints were re-pinned. Battle fixtures regenerate
   with `BATTLE_FIXTURES_WRITE=1 cargo test -p server battle_fixtures`.
6. **Modules stay a ledger** (Phase 2's modules-as-goods refactor deferred to
   Phase 6); armour recipes moved off Weapon Mounts as planned.
7. **Charter:** the Landfall kit carries six Fission Cores and the Interceptor
   and Tiny Freighter parts; the raid takes the cores first; the trader's
   founding stock has three Fission Cores with the first featured at half the
   Hub mid (`FEATURED_SHARE`); the Deuterium home seed is gone; later builds'
   parts are bought at the Hub (the test keeps that bill under 4,500 Cr).
8. **Munitions (Phase 5) as built:** `crates/sim/src/munitions.rs`; rules
   **v17**. Racks hold 40 Torpedoes, drivers 60 Slugs, one crystal per beam
   gun; a crystal wears 0.001 per gun per shot (a thousand shots fresh → worn)
   to a floor of 50%. A new gun launches with its crystal; racks and drivers
   load at the dock. Docks, the Hub (72 / 30 / 22 Cr base) and tenders supply
   munitions through the fuel path (`Fleet::take_fuel`, `supplies_aboard`).
   Munitions Lines already gates the Armaments Complex, so the three recipes
   need no gate of their own; Hardened Magazines adds +25% to the rack and
   driver magazines. The fleets' magazines are recorded battle inputs
   (`Input::Magazines`), so archives replay exactly; NPC fleets carry none and
   fight as v16. Fleet panel and mobile card show one gauge per magazine
   ("dry · kit beams" when a rack is empty); the stores pop-out has a
   Munitions group.
9. **Bots and the charter (found running the bot charter test):** a freight
   route's arrival reserve is energy and is now checked against every tank
   aboard (a fission hull has no Deuterium tank, so the old check refused
   every route of the charter's Tiny Freighter); bots price a freighter's Hub
   cycle at the throttle freight flies (Slow) rather than the hull's current
   one, keep a Uranium floor only while a hull of theirs burns Uranium, hold
   Hub runs from the den step through the haul so the first freighter stays
   free for the stash, see the stash during the Full-speed step, and price a
   Survey Probe's legs and tank at nothing (it has no warp core, so it flies
   free; the old reach check kept every bot's probe at home). A bot now
   plays the charter: raid 1.3 min, trader 3.2, Interceptor 6.1, den 8.7,
   loot home 15.0, export 15.9, Small Freighter ≈55, mining expanded ≈62.
   Harness (seed 42, 8 bots, 180 min): four bots complete the charter, one
   is hauling ore and one prospecting its belt (the last chapters), and two
   that lost their Interceptor to pirates sit at the Small Freighter step
   waiting on Hub-bought parts and Uranium (the October baseline was 8/8 by
   300 min; the Hub round trips are what changed).
   **Open for Peter:** the fission Tiny flies the Hub leg Slow (Full would not
   reach the Hub loaded), about 16 minutes each way, and the Small Freighter's
   kit parts come from the Hub by Authority freight (≈16 minutes a delivery),
   so the charter's back half waits on the Hub; the trader's last call could
   carry those parts instead (Hull Sections 3, Nav Computer 1, Cargo Bays 3).

## What changes, in one paragraph

A hull stops being a pile of Alloys and becomes a **kit**: engines, structure,
systems, guns, plus a thin skin of bulk goods. The engine is a **warp core named
by its fuel** — Fission (Uranium), Fusion (Deuterium), Helium (Helium-3), Fold
(Antimatter) — one fuel per core, one tank per fuel aboard. Speed is a throttle
on any core: Full at rated speed, Slow at half speed for a quarter of the fuel
per distance. A hull's engine **count** follows its mass; its **grade** is
chosen at build and changed by a yard refit. The hull's guns become parts too
(**Pulse Lasers** and **Beam Batteries**), so firepower is something you build,
buy, haul and lose. Modules stay the swappable layer on top. The bulk good
Armaments is renamed **Weapon Mounts** and, in a later phase, three munitions
goods (**Torpedoes, Slugs, Beam Crystals**) give fighting a supply line. The
founding charter turns the engines into its story: the first raid steals them,
the trader sells one back cheap, the Interceptor it builds kills the den, the
bounty buys the freighter's pair, and the freighter hauls the stolen engines
home.

## The three layers

| Layer | What it is | Changed when | Sets |
|---|---|---|---|
| **Kit** | cores, Nav Computer, Hull Sections, Cargo Bays, guns, skin | built once; cores swappable by refit | how it moves, how hard it hits, how big it is |
| **Fit** | modules within slots and fitting points | any yard refit | what kind of fire, what it shrugs, what it screens |
| **Supply** | fuel per core kind; later munitions per weapon kind | every dock | how far it goes, how long it fights |

Research and captains multiply all three and change nothing physical.

## 1 · The cores

| Core | Fuel (base Cr) | Energy a unit | Inputs per core (base Cr) | Hub | Made after |
|---|---|---|---|---|---|
| Fission Core | Uranium 5.6 | 1 | 1.0 Precision Components, 0.8 Composites, 0.5 Machinery (165) | 215 | Drive Works I, Autoforges (Materials II) |
| Fusion Core | Deuterium 14 | 4 | 2.0 Precision Components, 1.5 Composites, 0.5 Iridium, 0.5 Titanium (335) | 430 | Drive Works II |
| Helium Core | Helium-3 75 | 24 | 3.0 Precision Components, 1.5 Composites, 1.0 Iridium, 0.5 Titanium (470) | 650 | Helium-3 Cycle (Materials IV, today's "Fusion Reactors") |
| Fold Core | Antimatter 120 | 96 | 4.0 Precision Components, 2.0 Composites, 1.0 Iridium, 1.0 Exotic Matter (680) | 900 | Antimatter Synthesis (Materials VI) |

- One glyph, four tints: amber, blue-white, gold, violet. "Helium Core" on the
  card, "advanced fusion" in the tooltip.
- The Authority sells all four from day one. Research gates **making**, never
  buying. Hub books: Uranium and Deuterium bulk, Helium-3 thin, Antimatter its
  own 120-unit book — left scarce, so Fold hulls stay rare.
- Energy densities are chosen so a fission hull on Slow burns exactly what slow
  warp burns today and a fusion hull on Full burns what full warp burns today
  (today's SLOW_FUEL_MULT 0.25 recast as Uranium 1 : Deuterium 4). Helium-3
  keeps its ×6 over Deuterium (HELIUM3_FUEL_EQUIV).
- Drive Works recipes, one at a time like a Smelter: the four cores plus the two
  plant reactors (§5). Rate: Fission 0.005/s at tier I (one every 3 min 20 s),
  the others slower in proportion to inputs.

### Engine count

`engines = round(mass / 650)`, warships × 1.5, **working hulls** (Mining Barge,
Builder, Belt Station — station-keeping drives) `mass / 1,300`, minimum 1,
Survey Probe 0.

| Hull | Mass | Engines | | Hull | Mass | Engines |
|---|---|---|---|---|---|---|
| Scout | 80 | 1 | | Tiny Freighter | 1,500 | 2 |
| Interceptor | 200 | 1 | | Combat Freighter | 2,000 | 3 |
| Corvette | 800 | 2 | | Small Freighter | 2,500 | 4 |
| Destroyer | 2,000 | 5 | | Medium Freighter | 4,500 | 7 |
| Cruiser | 4,000 | 9 | | Colony Ship | 6,000 | 9 |
| Battleship | 8,000 | 18 | | Transport | 7,000 | 11 |
| Dreadnought | 16,000 | 37 | | Large Freighter | 8,000 | 12 |
| Titan | 32,000 | 74 | | Heavy Freighter | 14,000 | 22 |
| Mining Barge (working) | 3,500 | 3 | | Bulk Freighter | 24,000 | 37 |
| Builder (working) | 2,500 | 2 | | Belt Station (working) | 12,000 | 9 |

Count affects cost only; tank size stays `FUEL_PER_HULL_MASS × hull mass`.

### Throttle, tanks, fleet rules

- `TransitMode::Full` = rated speed, fuel × 1.0 per distance.
  `TransitMode::Slow` = half speed, fuel × 0.25 per distance. Any core.
- **Stealth: Fusion and up.** The first upgrade buys something besides range.
- A hull's tank holds its core's fuel. The fleet carries a tank per fuel
  (`BTreeMap<Commodity, f64>`), capacity per kind = Σ hull mass of the hulls on
  that core × FUEL_PER_HULL_MASS × research. A fleet is fuelled while **every**
  kind aboard has fuel; otherwise it holds, exactly as today's dry fleet does.
  The same per-kind store carries the munitions magazines in Phase 5.
- Docks fill each kind the fleet needs from the local stockpile (same reserve
  rule: never below an hour of the planets' fusion burn). The Hub bunkers each
  kind by purchase. The "Helium-3 first" rule goes: no tank takes two fuels.
- **Jumps** need every hull on a Fold Core. Jump cost = hull mass × 0.005
  Antimatter from the same tank (two full jumps per fresh bottle, as today).
  Raider and Scout lose their built-in jump: they jump only when built or
  refitted with Fold Cores. The Jump Drive module is retired.
- Ships of the line (Battleship and up) **cannot mount Fold Cores**. A capital
  arrives, it never chases. They lose their no-Uranium exception: a fission or
  fusion capital may idle home on Slow.
- `FuelResearch.helium3`, `burns_uranium`, `formation_burn`, `can_slow` and the
  siege-anchor / jump-drive store filters collapse into one check on the cores
  aboard. Bunkerage, Efficient Burns, Fusion Freight, Torch Regime, Long Patrol
  and Drive Tuning still multiply the same burn.
- **Refit** at a yard swaps every engine of a hull to another grade; the old
  cores return to the stockpile. The upgrade ladder is Fission → Fusion →
  Helium → Fold.

## 2 · The other parts

No new structure. Every part is a second recipe on a works that exists.

| Part | Made at | Inputs per unit (base Cr) | Who needs it |
|---|---|---|---|
| **Hull Section** (exists) | Hull Fabricator | 1.2 Composites, 0.5 Machinery, 0.4 Titanium (115) | every hull but Scout and Probe: warships ≈ mass/330, civil ≈ mass/660 |
| **Cargo Bay** (new) | Hull Fabricator, second recipe | 3 Alloys, 1 Polymers, 0.5 Machinery (125) | 1 per 50 hold units; its berth is the freighter's utility slot |
| **Nav Computer** (new) | Electronics Fabricator, second recipe | 2 Semiconductors, 1 Precision Components, 0.2 Rare Elements (180) | every hull 1 (Probe included), Battleship 2, Dreadnought 3, Titan 4 |
| **Pulse Laser** (new) | Armaments Complex | 1 Weapon Mounts, 0.5 Semiconductors, 0.25 Precision Components (≈95) | light gun, firepower 0.5 |
| **Beam Battery** (new) | Armaments Complex | 4 Weapon Mounts, 2 Semiconductors, 1 Precision Components, 1 Machinery (≈445) | line gun, firepower 2.0 |

- Hull Fabricator adds Cargo Bays at 0.25/s. Electronics Fabricator adds Nav
  Computers at 0.10/s. Armaments Complex gains gun recipes at 0.20/s (Pulse
  Laser) and 0.05/s (Beam Battery). One recipe at a time on each works, like a
  Smelter; a second works runs a second recipe. Drive Works as in §1.
- **Life Support was considered and cut**: flat at 1 on everything below a
  Destroyer; add later if Colony Ship / Transport want a people part.
- The earlier **Hardpoint** part and **Fitting Works** structure are dropped. A
  mount that did nothing in battle was the hollow version of the gun; the gun
  is the honest one.
- A thin **bulk skin** stays in every hull recipe (Alloys plating; Weapon Mounts
  and Semiconductors on warships, read as magazines and fire control) so the
  Smelter still feeds the yard directly and the charter's "Smelter → Alloys →
  Interceptor" still reads.
- Parts are goods. They sit in the stockpile, ride freighters, are raided by
  price and sheltered by the Vault first. The yard waits on stock; the build
  card shows the missing part with a Buy line. Yard build times unchanged.
- The Authority sells every part. Parts are 95–900 Cr a unit and ride Hub →
  yard, which makes them the convoy worth ambushing and a yard's shelf the
  mid-game home target. Every gun is priced under a Fission Core so a home raid
  still takes cores before guns.

### Guns and firepower

The hull's **attack weight becomes the sum of its guns**: Pulse Laser 0.5, Beam
Battery 2.0. Counts are chosen to reproduce today's weights, not to look tidy,
so the balance matrix stands.

| Hull | Pulse Lasers | Beam Batteries | Firepower | Today |
|---|---|---|---|---|
| Interceptor | 6 | | 3.0 | 3.0 |
| Corvette | 2 | | 1.0 | 1.0 |
| Destroyer | 1 | 1 | 2.5 | 2.4 |
| Cruiser | 1 | 2 | 4.5 | 4.5 |
| Battleship | | 4 | 8.0 | 8.0 |
| Dreadnought | 2 | 7 | 15.0 | 15.0 |
| Titan | | 12 | 24.0 | 24.0 |

The Destroyer is the only hull that moves (+4%). Capitals carrying a few Pulse
Lasers read as secondary batteries. Scouts, freighters, the Combat Freighter
and all working hulls carry no guns, matching their zero attack weight. Hull
affinities (Destroyer beams ×1.2, Battleship drivers ×1.2, …) are unchanged
and apply on top, as today.

### The eight reference hulls

| | Interceptor | Corvette | Destroyer | Battleship | Tiny | Small | Medium | Large |
|---|---|---|---|---|---|---|---|---|
| Engines (any grade) | 1 | 2 | 5 | 18 | 2 | 4 | 7 | 12 |
| Nav Computer | 1 | 1 | 1 | 2 | 1 | 1 | 1 | 1 |
| Pulse Lasers | 6 | 2 | 1 | | | | | |
| Beam Batteries | | | 1 | 4 | | | | |
| Hull Sections | 1 | 2 | 6 | 40 | 2 | 3 | 6 | 14 |
| Cargo Bays | | | | | 1 | 3 | 8 | 20 |
| Alloys | 10 | 10 | 20 | 40 | 8 | 15 | 30 | 60 |
| Weapon Mounts (skin) | 8 | 6 | 15 | 60 | | | | |
| Semiconductors | 4 | 4 | 10 | 20 | | | 5 | 10 |
| Machinery | | | 5 | 15 | 3 | 6 | 10 | 15 |
| Precision / Rare / Iridium | | | 4 / / | / 9 / 13 | | | | 5 / / |

Yes, the Interceptor needs a Hull Section: otherwise the Hull Fabricator means
nothing until the Destroyer. The charter crate carries that one section.

## 3 · The fit (modules) — unchanged rules, one refactor

Modules stay exactly as they are. Written down here so the whole ship system
is in one place.

| Hull | Slots | Points | | Hull | Slots | Points |
|---|---|---|---|---|---|---|
| Interceptor | 2 | 4 | | Destroyer | 3 | 8 |
| Corvette | 2 | 5 | | Cruiser | 4 | 12 |
| Scout | 1 | 2 | | Battleship | 4 | 18 |
| Freighters, Barge, Combat Freighter | 1 | 2 | | Dreadnought | 5 | 28 |
| Builder, Probe, Belt Station | 0 | 0 | | Titan | 6 | 45 |

Fitting costs: Mass Driver 2, Torpedo Rack 3, Point Defense 2, Reflective 2,
Whipple 3, Lance Array 3, Stasis Web 2, Warp Stabilizer 2, Interdiction Field
3, every utility 2. Both budgets bind at build and refit. Utilities one per
hull; weapon duplicates stack.

- **Weapon modules replace the stock beam, they never add to it.** A
  Battleship with two Torpedo Racks fires two torpedoes a round at 1.6 × its
  firepower each and its Beam Batteries are silent. Priority when several are
  aboard: torpedo, driver, lance, point-defense beam, stock beam. Under the kit
  this reads as **re-arming the mounts**: the guns set how hard each shot
  hits, the module count sets how many shots. Player-facing line: *weapon
  modules change what your guns fire; armour and utilities add to the hull.*
- Armour blunts one type into that ship (Reflective halves beams, Whipple takes
  60% off drivers). Point Defense screens torpedoes and fires a half-strength
  beam. Control: Stasis Web, Warp Stabilizer, Interdiction Field (Destroyer).
  Utilities: Extended Tanks, Cargo Pods, Recon Suite, Escort Datalink, Fuel
  Transfer Rig, Survey Drive, Nebula Spectrometer. Jump Drive retires (§1).
- **Reflective Plating and Whipple Armor move off Weapon Mounts** (to
  Composites, Silicates, Alloys, Machinery) so Weapon Mounts is only ever in
  things that shoot.
- **Modules become goods** (`Commodity` variants, one per `ModuleKind`). The
  separate per-system module ledger, the fleet `modules` map and the
  `BuyModule` / `SellModule` / `TransferModules` commands fold into the
  stockpile, the Hub book and freight routes. Fitting debits the stockpile;
  removed modules return to it. One store, one Hub book, one way to haul, and
  a Torpedo Rack at ~760 Cr becomes the thing a raider takes first from a
  home. A refactor, not a rule change; Phase 2.

## 4 · Goods

- **Armaments → Weapon Mounts** (display rename, internal id stays
  `Armaments`, as Electronics shows as Semiconductors). Mounts, launch tubes,
  magazines, fire-control gear. Goes into: guns (§2), Torpedo Rack, Mass
  Driver, Point Defense, Lance Array, Stasis Web, stations, and the warship
  skin. Never into armour.
- The **Armaments Complex keeps its name**: "armaments" is the category the
  plant covers — Weapon Mounts, guns, and the munitions of Phase 5. Munitions
  Lines still unlocks it.
- Alternates considered for the name: Weapon Components (blander), Ordnance
  (reads as ammunition, the problem being fixed).

## 5 · Plants

Ships have **cores**, planets have **reactors**. Both families come off the
Drive Works.

- **Fusion Reactor** = today's Fusion Core good, renamed. Deuterium Fusion
  Plants III–IV, one per tier, as now.
- **Advanced Fusion Reactor** (new): Helium-3 Fusion Plants V–VI, one per tier,
  in place of today's Fusion Core. Unlocked by Helium-3 Cycle.
- Antimatter Plant unchanged (2 Fusion Reactors + 20 Exotic Matter).
- A Fission Plant I–II on a Fission Reactor is **not** in this plan.

## 6 · Research

- `mat_foundry_iv_fusion_reactors` → **Helium-3 Cycle**: "Helium Cores and
  Fusion Plants V–VI burn Helium-3, six times the energy a unit. Drive Works:
  Helium Cores and advanced reactors."
- Drive Works prerequisite moves from Fusion Reactors (IV) to **Autoforges
  (II)**. Fission Cores at Drive Works I, Fusion Cores at Drive Works II, Helium
  Cores after Helium-3 Cycle, Fold Cores after Antimatter Synthesis.
- Nav Computers with the Electronics Fabricator (Signal Libraries, I) — the
  first part a player can make. Cargo Bays with the Hull Fabricator (Prefab
  Construction, II). Guns and munitions with the Armaments Complex (Munitions
  Lines, II).
- Weapons programmes unchanged: Magnetic Accelerators, Warhead Yields, Fire
  Discipline, the Strike and Countermeasure trees. Phase 5 adds a magazine
  effect to Hardened Magazines.
- No other programme moves.

## 7 · The charter arc

Order as it stands today (v5, 29 steps). Only the goods and three cards move.

1. **Landfall kit** carries the charter's engines: **six Fission Cores**
   (Interceptor 1, Tiny Freighter 2, Mining Barge 3), the Semiconductors and
   Weapon Mounts as now, 20 Uranium as now. **The 10-unit Deuterium seed
   goes**: no charter hull is a fusion hull.
2. **The first raid** (founding_raid.rs, `RAID_TAKES`) takes the Fission Cores
   first, then Semiconductors and Weapon Mounts as today. It leaves the
   Uranium. Guide line: "Lost: 6 Fission Cores, …". The Laser Battery story
   card after the raid is unchanged.
3. **Trader, step 8**: featured card, one Fission Core at half the Hub price
   (~110 Cr). One core is one Interceptor.
4. **Shipyard I, Interceptor** (step 11) as now. The crate carries its Hull
   Section, Nav Computer and **six Pulse Lasers**. The Interceptor is built
   unfitted and fights the den on its stock beams, as today.
5. **The den** (steps 11–12) as now; its stash becomes the stolen goods: the six
   cores plus the taken Semiconductors and Weapon Mounts (pad with Machinery
   so the stash stays over 50 units and the two-load haul lesson holds). Bounty
   5,300 as now.
6. **Trader, after the den**: second card, two Fission Cores at the full price,
   paid from the bounty. The Interceptor has no hold, so the pair is bought,
   not recovered.
7. **Tiny Freighter** (step 14) with the pair + crate parts; **Full speed**
   (step 15) now taught on a fission hull burning four times the Uranium;
   **Haul the stash** (step 16) in two loads, as now.
8. **Reach**: the recovered six cores go three into the Mining Barge and three
   toward the Small Freighter's four. The player buys the fourth at the Hub:
   the first engine they ever pay the Authority for, at the Small Freighter
   step (19/21).

Three sources in one chapter: the trader's discount, a bounty-funded purchase
at the going price, and recovery by force. No module and no munition appears
in the charter.

## 8 · Munitions (Phase 5)

Fighting gets a supply line. The granularity lives inside the battle, where
the engine already fires one torpedo per rack per round; the player sees
magazines, never missiles.

| Good | Fired by | Behaviour | Inputs (sketch) | One use for |
|---|---|---|---|---|
| **Torpedoes** | Torpedo Rack | one unit one shot; exhausts | Weapon Mounts, Precision Components, Volatiles | the hardest hit |
| **Slugs** | Mass Driver | one unit one shot; exhausts | Alloys, Titanium | cheap, heavy, armour-answerable |
| **Beam Crystals** | every beam gun and Lance Array | one per gun; **wears**, never runs out | Crystalline Ore, Rare Elements | Crystalline Ore's first use beyond Silicates |

- **Magazines per kind on the fleet**, the per-kind store from §1. Capacity:
  40 Torpedoes per rack, 60 Slugs per driver, one crystal per beam gun
  (tunable). Filled free at owned or allied docks from the stockpile, bought at
  the Hub, topped up in the field from a freighter's hold (the Fuel Transfer
  Rig's order generalised to fuel and munitions).
- **In battle** (rules v17): each torpedo and slug decrements its magazine. A
  dry rack goes silent and the hull **falls back to its kit guns** — the Beam
  Batteries come back online. A worn crystal fires at reduced strength (floor
  50%), so a fleet is never disarmed and beams keep their role as the weapon
  that gets you home. Fresh crystals are what a well-supplied fleet brings.
- Made at the Armaments Complex after Munitions Lines, three more recipes.
  Hardened Magazines gains +25% magazine capacity beside its cost effect.
- NPC pirates, the den, Authority fleets and sentinels fight unfitted on fresh
  crystals and are exempt, like sentinels from upkeep. The charter is untouched.
- Munitions are cheap per unit, so a home raid still takes cores, guns and
  modules first; a tender full of Torpedoes in deep space is the convoy to hit.
- Stores bar rows under a Munitions pin, hidden until Munitions Lines. Fleet
  panel: a magazine bar per kind. Battle theatre: a "dry" mark on a silent
  rack.
- What it buys: the Travian preparation step (fill magazines, bring a tender,
  or go in on beams); the defender's home stockpile between waves; a sink that
  grows with war; a legible build-card trade-off (torpedoes hit hardest and
  need supply, drivers between, beams never run dry); syndicate specialisation
  across three lines.
- Not in it: a fourth "missile" weapon family (a combat change, not a goods
  change); per-shot bookkeeping outside the engine; crystals that exhaust.

## 9 · Client

- Build card: parts list with counts and a Buy line per missing part; grade
  picker for the engines (Fission default until the corp can fuel better); the
  module picker as today.
- Fleet panel: `Drives Fission` / `Fusion` / `Helium` / `Fold` and the tank per
  fuel in the fuel line; `Guns 4 Beam Batteries · firing Torpedoes` when racks
  are fitted. Phase 5 adds the magazine bars.
- Hub book: one **Warp Cores** row with four prices; a **Parts** group; a
  **Modules** group after the §3 refactor; a **Munitions** group in Phase 5.
- Stores bar: cores, parts and modules under a Parts pin; Weapon Mounts label.
- Shipyard view: the refit verb beside Repair.
- Research panel: Helium-3 Cycle copy; Drive Works emblem; Munitions Lines
  copy names the guns and munitions.
- Icons (Peter's pipeline, another model): core glyph (one master, four
  tints), Nav Computer, Cargo Bay, Pulse Laser, Beam Battery, Advanced Fusion
  Reactor (badge on the Fusion Core master), Weapon Mounts (re-cut of the
  Armaments master), and in Phase 5 Torpedoes, Slugs, Beam Crystals.

## 10 · Order of work, go/no-go

Phases in order; art in parallel from Phase 2.

**Phase 1 — fuel cores and tanks (sim).** Four core commodities; per-ship core
kind; fleet per-kind store (tanks now, magazines later); throttle decoupled
from fuel; densities; jump from Fold Cores; capitals capped at Helium; refit
command; Drive Works recipes and gate; Helium-3 Cycle rename;
FuelResearch.helium3 removed.
*Go/no-go:* fuel, refuel, tender and jump tests green; a loaded fission Tiny
Freighter reaches the Hub on Slow (≈260k su) and not on Full (≈65k su); a
mixed-core fleet holds when one kind runs dry and moves when the dock refills
it; a Battleship cannot take Fold Cores.

**Phase 2 — parts, guns, goods (sim).** Nav Computer, Cargo Bay, Pulse Laser,
Beam Battery commodities; second recipes on the Electronics Fabricator, Hull
Fabricator and Armaments Complex; attack weight derived from gun counts (§2);
every hull recipe rewritten to the §2 tables + bulk skin; armour modules off
Weapon Mounts; reactors renamed/added; Armaments → Weapon Mounts label;
modules become goods (§3); Authority catalogue for every part and module.
*Go/no-go:* every hull buildable at the right yard tier from Authority goods
alone; part counts match §2; attack weights equal today's except the Destroyer
at 2.5; `combat_balance` targets re-run and hold within their bands; module
tests green on the stockpile store; a freight route hauls a Torpedo Rack; a
home raid takes it before Alloys; the Armaments-efficiency test in build.rs
counts Weapon Mounts through the parts.

**Phase 3 — charter arc (sim).** Kit, raid take list, den stash, two trader
cards, crates (six Pulse Lasers in the Interceptor crate), step copy, Deuterium
seed removed; bot opening buys the trader's core and stocks parts before
BuildShip.
*Go/no-go:* charter walkthrough on the isolated :8097 server; harness 8/8 bots
finish the charter by 300 min; the stash still takes two Tiny loads.
`--reset-galaxy` on :8080 after this phase (new commodities, new kit).

**Phase 4 — client.** §9 minus munitions. *Go/no-go:* test-charter-crates,
test-market-workbench, test-fleet-panel, test-shipyard, test-research-workbench
updated and green; fixture captures of the build card and fleet panel.

**Phase 5 — munitions (sim + client).** §8: three goods and recipes, magazines
on the per-kind store, dock fill, Hub lines, field transfer, rules v17 with the
dry fallback and crystal wear, Hardened Magazines effect, bot refills, fleet
panel bars, stores rows, theatre mark.
*Go/no-go:* a torpedo fleet that fights twice without docking fires beams in
the second fight; a dock refill and a tender top-up each restore it; crystals
wear across fights and a dock renews them; `combat_balance` single fights on
full magazines unchanged; bots and the den fight unaffected; a beam-only
raider pays nothing per raid.

**Phase 6 — later, not this plan.** Price layer (levy or BOM, K inside the
parts recipes), parts and station upkeep, Fission Plant, Life Support, station
hardpoints as Beam Batteries, ammunition types with range/damage trade-offs.
Needs the Phase 0 harness baseline first.

## 11 · Calls made in this plan

Peter decided: fuel-named cores, one fuel per core, throttle burns ×4, Helium
Core as its own rung, engine counts by mass (Interceptor 1, Tiny 2), the raid /
trader / den arc, Hull Sections on the Interceptor, guns in the kit as the base
layer, munitions split into separate goods, Armaments renamed.

Fable decided, overrule freely: Stealth from Fusion up; capitals without Fold;
working hulls at half the engine density; Life Support cut; bulk skin kept;
Antimatter Plant and Hub Antimatter book unchanged; Drive Works at Autoforges;
the Authority sells all four cores from day one; Jump Drive module retired;
gun counts from today's attack weights (Destroyer 2.4 → 2.5); two gun parts
rather than one; weapon modules keep replacing the beam; Hardpoint part and
Fitting Works dropped; Nav Computer on the Electronics Fabricator; the name
Weapon Mounts; armour off Weapon Mounts; modules become goods; munitions split
by damage type (no missile family); crystals wear instead of exhausting; NPCs
exempt from munitions; munitions after the kit as Phase 5.
