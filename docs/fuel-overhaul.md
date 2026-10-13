# Fuel overhaul: plan

Status: **built Oct 12 2026**, uncommitted, protocol v49. Every phase (1–7) is
in, and all 16 open decisions took the recommended option. It needs a fresh
galaxy (`--reset-galaxy`). Not measured: the balancing harness has not run.
See [As built](#as-built) for where the build differs from the plan below.

Proposed Oct 12 2026 for Peter's review, and revised the same day. The
revision keeps Deuterium at today's Fuel cost; the Fusion Freight research
then makes it the freight fuel in mid-game. It changes the power plants
([power-plants-plan.md](power-plants-plan.md), built Oct 10, uncommitted,
protocol v48) and replaces most of the Oct 11 bot fuel-supply fix.

Peter's calls:

- **Power:** planets get electricity from structures, solar first, fusion
  later.
- **Travel costs credits:** ships have a fuel component.
- **Jumps** cost more and burn something exotic: antimatter.
- **Antimatter is the last tech.**
- **Lore:** colonies can't make warp drives or other advanced technology at
  first. The Authority imports it through the wormhole.
- **Starter fuel:** new players make it early, and it allows only slow warp.
  He agreed to Uranium from Ferrite Ore.
- **Every fuel has a Hub price.** Free charge from the planet grid was ruled
  out: "that will cause no fuel cost".
- **Freighters move to Deuterium in mid-game** (Oct 12). Before that, Uranium
  is the freight fuel, and Deuterium is the player's choice when time is
  critical.
  - He approved keeping early Deuterium at today's Fuel cost, so that an
    export backlog alone doesn't make it pay.

## In one paragraph

Ships carry two fuels. Uranium, milled at home from Ferrite Ore, drives slow
warp: half the speed, for what Economy cruise costs today. Deuterium is
today's Fuel renamed, at today's price, and drives full warp; a full-speed
trip costs ten times a slow one. In mid-game, the Fusion Freight research cuts
freighters' Deuterium burn to a tenth, and Deuterium becomes the freight fuel.
Helium-3, after Fusion Reactors, packs six times Deuterium's usable energy into
the same tank space, so it is the long-range fuel. Jumps burn Antimatter, which
only the Authority sells until the last research, Antimatter Synthesis.
Planets run on Solar Arrays (tiers I–II, free), then Fusion Plants on
Deuterium (III–IV) and on Helium-3 (V–VI). Factories draw power and burn no
fuel. Warp Cores and Fusion Cores come from the Authority until Fusion
Reactors lets you make them. A depot rule keeps a port stocked by Authority
freight.

## Facts that shape the plan

- **Fuel does four jobs today:**
  - ship propellant;
  - power-plant fuel (tiers I–IV);
  - a process input: every Smelter recipe (0.1–0.5 per unit), Machine Works
    (0.4), Drive Works (1.0) and three colony projects;
  - a build material: 11 hulls, the Drive Works and the Deep Space Sensor kit.
  - Outposts also burn 72 an hour, and exotic nodes 7,200 an hour.
- **Homes make none.** A home holds only Biomass and Ferrite Ore; 0 of 800
  homes across 100 seeds have Volatiles. The kit's 60 Fuel is the runway.
- **Full is dear and Economy nearly free.**
  - A Tiny Freighter carrying 50 Alloys burns 45 Fuel (632 Cr) to the Hub at
    Full, or 4.5 Fuel (63 Cr) on Economy.
  - So freight runs Economy, and warships, which can't use it, pay full price.
- **Every empty hull has the same range:** 35k su on thrusters, 175k in warp.
  - Tank = hull mass × 0.035; burn = 1e-6 × mass × distance.
  - Cargo adds 28 mass a unit, so a loaded Bulk Freighter reaches only 22k su
    at Full. Economy's tenfold saving is what lets big freighters haul at all.
- **Fuel appears from nothing** in three places: new hulls leave the slip
  with a full tank, the Authority rescue delivers it, and Ramscoop skims it.
- **Jumps are free** (`JUMP_FUEL_FACTOR` = ∞). Only Interceptors and Scouts
  carry jump drives.
- **Helium-3** fills capitals' tanks at 6 Fuel a unit and fuels Fusion Plants
  V–VI. Its gas-giant harvester trickle almost never fires, because
  generated gas giants carry no Volatiles.
- **Distance:** homes sit 68–80k su from the Hub (74k in the production seed).
- **Research:** only the Line school goes past tier V, and a test enforces it.
- **Bodies keep no orbit radius,** only their order in the system.

## The ladder

| Stage | Research | Ships | Planets | Source |
| --- | --- | --- | --- | --- |
| Frontier | none | Uranium (slow); Deuterium (full, ten times the cost) | Solar Array I–II | Uranium: your mill. Deuterium: the Hub |
| Ice | Bunkerage (Propulsion I) | your own Deuterium | Fusion Plant III–IV on Deuterium | ice worlds, icy belts |
| Freight | Fusion Freight (Line Haul III) | freighters burn a tenth of the Deuterium: Uranium's cost at twice the speed | — | — |
| Fusion | Fusion Reactors (Foundry IV) | Helium-3: six times the range per tank | Fusion Plant V–VI on Helium-3 | icy pockets, gas giants (Ramscoop), the Hub |
| Antimatter | Antimatter Synthesis (Foundry VI, new) | your own Antimatter; Jump Drive module | unchanged; the Antimatter Plant draws huge power | your fusion worlds |

Interceptors and Scouts can jump from day one, on Authority antimatter.

## Rules

### Fuels and prices

**Deuterium is today's Fuel, renamed.** Its units, its 14 Cr price, its burn,
tank sizes and ranges are all unchanged.

| Fuel | Energy per unit | Hub price | Store | Burned by |
| --- | --- | --- | --- | --- |
| Uranium | 1 | 5.6 | fission store | slow warp |
| Deuterium | 1 | 14 | fusion tank | full warp, Stealth, Fusion Plant III–IV |
| Helium-3 | 6 (as today) | 75 (unchanged) | fusion tank | full warp and Stealth after Fusion Reactors; Fusion Plant V–VI |
| Antimatter | — | 120 | antimatter bottle | jumps |

- **Why Uranium costs 5.6:** slow warp burns a quarter of the energy per
  distance, so a slow trip costs exactly what Economy costs today
  (¼ × 5.6 = 0.1 × 14). A full-speed trip on Deuterium costs ten times as
  much.
- **Helium-3 stays worth 6 Deuterium.** It holds about four times
  Deuterium's energy per unit, and about six times the usable energy, because
  Deuterium loses a third of its energy to neutrons. It costs a little less
  than Deuterium per unit of energy (12.5 Cr against 14), and a tank of it
  goes six times as far.

### Ships: two stores

- **Fission store** (Uranium) and **fusion tank** (Deuterium and Helium-3),
  each the size of today's tank (hull mass × 0.035). Bunkerage (×1.25),
  Extended Tanks (×1.75) and the Survey Drive (×0.6) apply to both.
  - Tiny Freighter 52.5 each, Small 87.5, Interceptor 7, Scout 2.8, Survey
    Probe 1.4, Mining Barge 122.5, Colony 210.
  - Ships of the line (Battleship, Dreadnought, Titan) carry no fission store.
    A Titan's fusion tank holds 1,120.
- **Transit modes:**

  | Mode | Speed | Burns | Energy per distance | Empty range |
  | --- | --- | --- | --- | --- |
  | Full | 1 | Deuterium, Helium-3 | 1 | 175k su (1.05M on Helium-3) |
  | Stealth | ½ | Deuterium, Helium-3 | 1 (same trip energy, as today) | as Full |
  | Slow (replaces Economy) | ½ | Uranium | ¼ | 700k su |

  A loaded Bulk Freighter reaches 88k su on Uranium, enough for the Hub.
- **Who can go Slow:** every hull except ships of the line. Economy today is
  for freighters only.
- **Escorts** burn the same fuel as the fleet they guard, when they can.
- **Running out:** a fleet whose fuel runs out under way switches to its
  other store, at that store's speed, and the owner gets a notice. It stalls
  only when both are empty. Today it stalls at once.
- **Helium-3 in the tank:** after Fusion Reactors, a fleet at your own dock
  takes Helium-3 first. Each unit fills one unit of space with six Deuterium
  of energy. Deuterium burns first.
- **Refuelling:**
  - Your own and allied docks fill all stores from local stock.
  - At the Hub, a fleet buys only the fuel its current mode burns.
- **New hulls leave the slip empty** and fill at the yard's dock, and recipes
  drop their Fuel. The starting fleet and the Authority's replacement
  Freighter still arrive full.
- **Interdiction Field:** 20 Deuterium, as today.

**One-way trips, home → Hub (74k su), before Fusion Freight:**

| Hull | Full: Deuterium | Slow: Uranium |
| --- | --- | --- |
| Tiny, empty | 23.4 (327 Cr) | 5.8 (33 Cr) |
| Tiny, 50 cargo | 45.2 (632 Cr) | 11.3 (63 Cr) |
| Small, 150 cargo | 104.3: over its 87.5 tank | 26.1 (146 Cr) |
| Interceptor | 3.1 (44 Cr) | 0.8 (4 Cr) |
| Mining Barge, empty | 54.5 (763 Cr) | 13.6 (76 Cr) |

The two columns cost the same as Full and Economy today.

### Freight: Uranium early, Deuterium from mid-game

**Early**, a Deuterium trip costs ten times a Uranium trip, as Full does
against Economy today. Freight runs on Uranium even with a backlog.
Deuterium pays only for a deadline, or for cargo worth more than about 36 Cr
a unit (Machinery and up), like air freight against sea freight.

| Tiny Freighter with cargo waiting, early | Earns per hour on Uranium | On Deuterium |
| --- | --- | --- |
| Ferrite Ore (50 units, 400 Cr) | about 690 Cr | loses money |
| Alloys (1,300 Cr) | about 2,700 Cr | about 1,500 Cr |
| Electronics (1,700 Cr) | about 3,600 Cr | about 3,300 Cr |
| Machinery (3,100 Cr) | about 6,800 Cr | about 9,600 Cr |

**Mid-game: Fusion Freight.**
- **Slot:** it takes the Line Haul III slot of Logistics Doctrine, whose
  flag nothing reads, so the tree's shape and counts stay.
- **Gate:** any Propulsion II and 30 convoy deliveries.
- **Effect:** player freighters burn a tenth of the Deuterium.
  - A Deuterium trip costs the same as a Uranium trip at twice the speed, so
    Deuterium becomes the freight fuel.
  - Freighters' Deuterium range grows tenfold, so even a loaded Bulk
    Freighter reaches the Hub.

**Who it doesn't touch:**
- Warships, barges and colony ships pay today's Full cost on Deuterium.
- Warships other than ships of the line can patrol on Uranium at a tenth of
  that.
- Uranium keeps early freight, patrols and expeditions.

| Tiny Freighter, home ↔ Hub, 50 Alloys out | Uranium | Deuterium |
| --- | --- | --- |
| Early | 96 Cr, about 26 min | 959 Cr, about 13 min |
| After Fusion Freight | 96 Cr, about 26 min | 96 Cr, about 13 min |

**Choosing per trip:** the order bar and the route editor show both options
with time and cost, e.g. "Slow · 26 min · 96 Cr" and "Full · 13 min · 959 Cr".

### Uranium Mill

- **New structure**, last in `ALL`: Ferrite Ore → Uranium, two Uranium per
  ore.
- **Output:** 0.05 Uranium a second at tier I (180 an hour, from 90 ore).
  That covers about ten Tiny round trips on Uranium.
- **No research.** Industrial slot; it draws power, needs crew and goes to
  tier IV.
- **Build:** Alloys 20, Machinery 8, 15 s. The kit grows by that much, so
  the charter still fits.
- **Value:** one ore (8 Cr) makes two Uranium (11.2 Cr). The mill's job is
  fuel at home, not profit.

### Deuterium

- **Fuel becomes Deuterium** in code, on the wire and in the UI.
- **The Fuel Refinery becomes the Deuterium Refinery:** Volatiles →
  Deuterium, one for one at 0.8 a second, unchanged. Hydrocarbon Seas and
  Volatile Pockets still boost it, and Bunkerage still unlocks it with the
  Volatile Harvester.
- **Uses:** full warp, Stealth, Fusion Plant III–IV, and exotic-node upkeep
  (2 a second, as today).
- **The Hub sells it** as now.

### Helium-3

- **Worth 6 Deuterium,** as today.
- **Burning it needs Fusion Reactors,** for ships and plants alike. Today
  capitals burn it without research.
- **Sources:**
  - icy-belt pockets;
  - the Hub's thin book;
  - **Ramscoop**, which now skims Helium-3 at gas giants instead of Fuel: a
    sixth of a unit a second, the energy of today's 1 Fuel a second.
- **The gas-giant harvester trickle goes:** it almost never fires.

### Planet power

Grids, the free 3 MW base, one plant per planet, plant output (8 / 17.6 /
30.4 / 48 / 68.8 / 92 MW) and brownouts all stay.

| Tier | Plant | Runs on | Burn above base | Build |
| --- | --- | --- | --- | --- |
| I–II | Solar Array | sunlight | none | today's plant recipe |
| III–IV | Fusion Plant | Deuterium | 18 a MWh, as today | today's recipe + 1 Fusion Core |
| V–VI | Fusion Plant | Helium-3 (needs Fusion Reactors) | 3 a MWh, as today | today's fusion recipe + 1 Fusion Core |

- **Fuel costs match today's plants:** a full tier IV burns 864 Deuterium an
  hour (12,100 Cr). What changes is that tiers I–II burn nothing.
- **Landing Commands bring a Solar Array I,** so a new home never browns out
  for lack of fuel.
- **Solar output is flat.** Scaling it by orbit would need new body data.

### Industry runs on power

- **Fuel leaves every recipe:**
  - all Smelter recipes, the Machine Works and the Drive Works;
  - the three colony projects: Orbital Assembly, Agricultural Export and Deep
    Extraction;
  - 11 hull recipes, the Drive Works build and the Deep Space Sensor kit.
- **Heat-heavy works draw double power instead:** the Smelter, Machine Works
  and Drive Works draw 2 MW per unit of tier throughput.
- **Outposts** drop their Fuel upkeep. Provisions and Machinery stay.
- **The Smelter's refining floor** keeps ore only.
- **Prices:** the input baskets get cheaper (Alloys 16.2 → 12 Cr, Machinery
  57.2 → 51.6 Cr). Prices stay, so margins widen.

### Jumps and Antimatter

- **Antimatter** is a new good. The Authority sells it at the Hub on a thin
  book: base 120, refilling slowly, so supply is limited.
- **Jump cost:** Antimatter = mass × distance × 1e-7.
  - An Interceptor's 50k su jump burns 1 (120 Cr). Warping that far costs 2
    Deuterium (28 Cr).
  - A Scout's 50k su jump burns 0.4.
- **Antimatter bottle** on jump-capable hulls: 2 for an Interceptor, 1 for a
  Scout. Docks fill it like the other stores.
- **Antimatter Synthesis:** a new research programme, Materials · Foundry,
  tier VI.
  - It needs a Foundry V and a running tier V Fusion Plant. 240 h.
  - **Antimatter Plant** (new structure, Industrial slot): 2 Deuterium → 1
    Antimatter at 0.005 a second. It draws 10 MW per unit of tier throughput,
    ten times a factory.
  - **Jump Drive module:** any hull except ships of the line can jump.
  - The research tree's "only the Line goes past V" rule, its tier VI gate
    and its programme counts all change.

### Authority cores

- **Warp Core** (Drive Assemblies renamed, 215 Cr): every warp hull needs it.

  | Warp Cores | Hulls |
  | --- | --- |
  | 0 | Survey Probe |
  | 1 | Tiny and Small Freighters, Interceptor, Corvette, Scout, Mining Barge, Construction Ship |
  | 2 | Medium Freighter, Colony, Troop Transport, Destroyer, Mining Belt Station |
  | 3 | Large Freighter |
  | 5 | Heavy Freighter |
  | unchanged | Bulk Freighter 16, Cruiser 7, Battleship 14, Dreadnought 31, Titan 70 |

  That adds 12–19% to a small hull's cost.
- **Fusion Core** (new, about 300 Cr): one per Fusion Plant tier, III–VI.
- **Import-only until Fusion Reactors** (Foundry IV). Fusion Reactors now
  unlocks the Drive Works, moved there from Autoforges, with a recipe for
  each core.
- **The Hub always stocks cores:** a deep book that never sells out.
- **The kit carries one Warp Core** for the charter's Mining Barge.

### Fuel depot rule

- **Logistics → Standing rules:** "Keep [N] [fuel] at [port]", for Uranium,
  Deuterium, Helium-3 or Antimatter.
- **Billing:** when stock plus inbound falls below N, the Authority books
  the difference on its next departure. It charges the Hub price plus the
  freight fee (6% plus distance).
- **Delivery is physical:** light delay, storage headroom and raids all
  apply. Corporations whose exchange is closed and systems under blockade get
  nothing.

### Rescue and tenders

- **Authority rescue:** unchanged at 1,000 Cr plus three times the price. It
  delivers the fuel the fleet's mode burns.
- **Fleet tenders** pump Uranium or Deuterium cargo into the matching store.

## Founding kit and charter

- **Kit:**
  - 20 Uranium and 10 Deuterium instead of 60 Fuel (built; the plan said 40
    and 20, which overfilled the camp's 250-unit storage once the mill's goods
    and the Warp Core joined the kit);
  - plus the mill's 20 Alloys and 8 Machinery;
  - plus one Warp Core (phase 5).
- **Starting fleet arrives full:**
  - Tiny Freighter: 52.5 Uranium, 52.5 Deuterium.
  - Interceptor: 7 Uranium, 7 Deuterium, 2 Antimatter.
- **First shipment** runs Slow, escort included. The return basket drops its
  10 Fuel.
- **No new charter step by default.** The guide points to the mill when home
  Uranium runs low.
- **Survey Probe:** its recipe loses the 3 Fuel, and the probe fills at the
  dock.

## Bots

- Build the mill right after the mine.
- Freight and escorts run Slow until Fusion Freight, then Full. Urgent runs,
  such as a starving colony or a blocked project, go Full earlier.
- Deuterium is kept stocked by a small depot rule. Helium-3 comes after
  Fusion Reactors.
- Research Fusion Freight once freight volume justifies it.
- **Retired:**
  - Hub fuel runs and fuel shopping budgets;
  - the Bunkerage-first research rule;
  - development held on a single "fuel critical" flag. `fuel_target` splits
    per fuel.
- Buy Warp and Fusion Cores before queuing hulls and plants (phase 5).
- Jump only when a raid or an escape is worth the antimatter.
- **Harness:** a `fuel=` line and critical minutes per fuel.

## Client

- **Fleet panel:**
  - Uranium and Deuterium gauges, plus Antimatter on jump-capable hulls;
  - range and leg cost per mode;
  - the mode label "Slow · ½ speed · Uranium".
- **Mirrored constants:**
  - new values: Slow ½ speed and ¼ burn, Fusion Freight ×0.1, jump 1e-7;
  - remove the Economy constants.
- **Market:** Uranium, Deuterium, Antimatter, Fusion Core, Warp Core.
- **Build sheet:**
  - Uranium Mill;
  - Solar Array and Fusion Plant tiers ("Burns Deuterium", "Burns Helium-3");
  - Antimatter Plant.
- **Logistics:**
  - depot rules;
  - reserves per store in the route editor;
  - Harvesting becomes Helium-3.
- **Copy to replace:**
  - "25% speed · 10% Fuel";
  - "Jump fuel is unlimited";
  - "Keep 50 ore + 20 Fuel";
  - the outpost's "1.2 Fuel/min";
  - the kit line in `settlement.ts`.
- **Art:**
  - Uranium Mill, standing in with the Deuterium Refinery's art;
  - rewrite the power-plant brief: solar fields (I–II), reactor halls (III–VI);
  - Antimatter Plant;
  - icons for the new goods.

## What goes

- Fuel as a process input, a build material and outpost upkeep.
- Economy cruise, replaced by Slow.
- Free fuel on new hulls.
- Combustion power plants and their art brief.
- The gas-giant Helium-3 trickle.
- Most of the Oct 11 bot fuel fix.
- `asteroid-belt-mining-plan.md`'s "Hub bunkerage stays Fuel. Nothing
  requires Helium-3."

## Phases

0. **Dry run** (no code): recompute the trip, plant and kit tables against
   the harness routes, and settle the open decisions. The harness runs only
   when Peter says.
1. **Ship fuels** (sim, server, client, bots and tests):
   - the Deuterium rename;
   - Uranium: the mill, the two stores, Slow, the fallback;
   - Fusion Freight in place of Logistics Doctrine;
   - refuelling and empty launches;
   - Fuel out of every recipe and upkeep; double power draw for heat-heavy
     works;
   - the kit. Protocol v49.
2. **Power ladder:**
   - Solar and Fusion tiers, burn rates, a Solar Array I at landing;
   - Helium-3 behind Fusion Reactors;
   - Ramscoop skims Helium-3.
   - Fusion III–IV run without a core until phase 5.
   - Phases 1 and 2 ship together, on one reset.
3. **Antimatter jumps:** the good, the bottles, the jump cost.
4. **Depot rule.**
5. **Authority cores:** Warp Core, Fusion Core, the Drive Works move, the
   kit's core.
6. **Antimatter Synthesis:** the programme, the Antimatter Plant, the Jump
   Drive module, research-tree tests.
7. **Art and copy.**

## Decisions (recommended option first; all took it)

1. **Slow speed:** half; or today's quarter.
2. **Who can go Slow:** every hull except ships of the line; or freighters
   only.
3. **Uranium range:** a fission store the same size as the fusion tank (four
   times Deuterium's range); or smaller.
4. **Mill:** no research, Industrial slot, paid from the kit; or slot-free
   and landed with the Command Center, like the Solar Array.
5. **New hulls:** leave the slip empty; or full, as today.
6. **Helium-3:** burned only after Fusion Reactors; or capitals keep burning
   it without research. It stays worth 6 Deuterium either way.
7. **Ramscoop:** skims Helium-3; or Deuterium.
8. **Solar:** flat output; or by orbit position, which needs new body data.
9. **Early jumps:** Authority antimatter, about 120 Cr per Interceptor jump;
   or free until the last tech.
10. **Warp Cores:** every warp hull except the Survey Probe; or the probe too.
11. **Electronics:** stays local; or becomes a fourth core.
12. **Depot rule:** Authority freight, fuels only; or any good; or instant
    pay-at-the-pump.
13. **Charter:** no new step; or "Build a Uranium Mill" as step 2.
14. **Heat-heavy works:** double power draw; or unchanged.
15. **Fusion Freight timing:** Line Haul III, in mid-game; or tier II, earlier.
16. **Fusion Freight strength:** a tenth, so Deuterium freight costs the same
    as Uranium; or a fifth, so it costs twice as much and some freight stays
    on Uranium.

## As built

Built Oct 12 2026. Where it differs from the plan above, or adds to it:

- **Kit:** 20 Uranium and 10 Deuterium, not 40 and 20 (storage).
- **First shipment:** the basket buys no fuel, so the whole 50-unit hold
  carries goods; a Uranium row stays in the basket for the player who wants
  some at home.
- **Bots** build the mill right after the mine, fly freight and escorts Slow
  until Fusion Freight and Full after, and research Fusion Freight right after
  Freight Frames. Two plan items are not built:
  - they top up Deuterium through their existing Hub imports, not a depot
    rule;
  - urgent runs don't go Full early.
- **Drive Works:** the server serves both recipes (Warp Core, Fusion Core) as
  the line's choices, and both shells pick a product the way they pick the
  Smelter's ore.
- **Orbital Assembly** needs Fusion Reactors, like the Drive Works, since it
  makes Warp Cores.
- **Smelter reserve:** the policy's fuel floor no longer does anything (no
  smelter burns fuel). The client sends 0 and shows the ore floor only.
- **Art:** stand-ins until briefs land — the Uranium Mill and Antimatter Plant
  draw the Fuel Refinery's art; Uranium, Antimatter and the Fusion Core show
  glyphs; Deuterium and the Warp Core keep the Fuel and Drive Assemblies
  icons. Briefs: [power-plant-art-prompt.md](power-plant-art-prompt.md)
  (rewritten: Solar I–II, reactor halls III–VI) and
  [fuel-overhaul-art-prompt.md](fuel-overhaul-art-prompt.md).
- **Tests:** sim 1,003, server 241, client 48 scripts, all passing. New sim
  tests in `crates/sim/src/world/fuel_overhaul.rs`.

### Review fixes (Oct 12 2026)

A source review found eight problems; all are fixed:

1. **Fusion keeps the solar fields.** A Fusion Plant's first 17.6 MW (Solar
   Array II's output) stay free; the reactor burns only above base + solar
   (20.6 MW), and a dry reactor falls back to solar, not the 3 MW base.
   Upgrading never costs free power.
2. **Stealth runs dark.** Stealth now multiplies a fleet's signature by 0.6
   on top of the half-speed term; Slow keeps the speed term only. Full 1.0,
   Slow 0.4, Stealth 0.24. The View reads it per sample, like survey noise.
3. **Fusion Freight is per hull.** The cut applies to each player freighter
   and the cargo it carries, weighed by mass, so an escort riding along costs
   its own share, as it would flying apart. After Fusion Freight only player
   freighters launch on Full; barges, colony and construction ships stay Slow.
4. **Hub bunkerage rounds up.** Whole units, rounded up (overshoot under one
   unit spills), so a Scout's 0.2-of-1 bottle refills.
5. **One dock rule.** Docks and the Hub always fill the Uranium store and the
   bottle; the fusion tank only for a fleet flying on it (Full or Stealth).
   A colony dock never draws a fusion fuel below an hour of its Fusion
   Plants' current burn. "Ship production to Hub" keeps Helium-3 with the
   other fuels.
6. **A short bottle cancels the jump** (the fleet goes Idle, not stalled, so
   no misleading AAA offer), and tenders now pump Antimatter and Helium-3
   cargo too.
7. **Torch Regime works.** Its burn moved to warship hulls only (no
   freighters, no jumps), and the speed research is wired: Drive Tuning +10%
   every hull, Military Drives +20% and Torch Regime +30% warships.
8. **Displays:** the move confirmation checks a Slow trip against Uranium;
   the fusion tank reads as space ("12 + 40.5 He-3 / 52.5"), never energy
   over capacity; jump confirmations show the Antimatter cost.

Corrections to the earlier summary: Antimatter can sell out (a 120-unit
book that refills slowly); only cores never do. Bunkerage and Extended Tanks
don't enlarge Antimatter bottles.

## Risks

- **Balance is untested.** The cost targets come from formulas, and the bots
  need phase 1 before a harness run means anything.
- **Two stores** mean two gauges wherever a fleet shows, mobile included.
- **Cores slow early growth:** a second freighter needs an imported Warp Core.
- **Fusion Freight is a big step:** Deuterium freight gets ten times cheaper
  at once (decision 16).
- **Reset:** no save migration, per the no-old-galaxy-compat rule.

## Found while reading (not part of this plan)

- Fixed Oct 12 (review fix 7): Torch Regime's burn now taxes only warship
  cruise, and Drive Tuning, Military Drives and Torch Regime's speed work.
- Long Patrol and Express Charters still have no effect: nothing reads their
  modifiers.
- A comment says the Fuel Refinery runs at 0.40 a second; the code runs 0.8,
  and the test that checks it is ignored.
- From reading, not tested: a stalled fleet still holding MoveTo isn't
  "docked", so it won't refuel at a berth until told to hold.
- From reading, not tested: blockaders burn fuel while on station, and a dry
  blockade still counts.
- `can_cover_fuel` compares against a hard-coded Medium Freighter tank.
- Four commands whose comments say "not a gate" actually refuse.
- Stale docs and comments:
  - the `fuel.rs` module doc and `MOVEMENT_FUEL`;
  - `FuelShortfall`;
  - the `TransitMode` note that pursuit is always Full;
  - `fuel_total`;
  - the protocol version in `founding-charter.md`.
