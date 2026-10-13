# Power plants: plan

Status: built Oct 10 2026, uncommitted, protocol v48. Phases 1–5 are in; art
(phase 6) is briefed in [power-plant-art-prompt.md](power-plant-art-prompt.md)
and stands in with the Fuel Refinery's. The phase 0 dry run couldn't use bot
data (bots stall in the charter), so the numbers below are the starting ones.
Every open decision took the recommended option.

**Built:**

- **Sim** (`crates/sim/src/power.rs`, `world/power_grids.rs` tests):
  - `StructureKind::PowerPlant`, last in `ALL`, slot-free (`uses_slot`).
    Combustion recipe for I–IV; `FUSION_PLANT_RECIPE` (with Iridium) for V–VI.
    Tier titles "Power Plant" and "Fusion Plant".
  - `Body.grid` is settled each tick at the start of production: demand is
    each powered structure's tier throughput × its crewed share; supply is 3
    MW plus what the plant's fuel covers; power is rounded down to a whole
    percent.
  - Power multiplies mine and factory output, yard work, Foundry repairs and
    Academy research.
  - **Events:** `Brownout` and `PowerRestored` (latched), with timeline lines
    "Brownout · Freya II · 68%" and "Power restored · Freya II".
  - **Fusion Reactors** (`mat_foundry_iv_fusion_reactors`, Materials Foundry
    IV) opens tiers V–VI. Gas-giant Volatile Harvesters then draw 1 Helium-3
    ice per 120 Volatiles.
  - **Landing commands bring a plant:** every new Command Center (home
    founding, colony landing) lands with a Power Plant I on its planet
    (`seed_command_centers`), at Peter's suggestion. It burns nothing until
    that planet passes its base. Existing colonies get none; reset for that.
  - **Charter ledger:** "Power a second planet" (two plants, 400 Cr) follows
    "Run a Smelter". The command's own plant can't pay it.
- **Server:**
  - owner-only `BodyView.power` (demand, supply, power, burn, fuel, no_fuel);
  - `AssignmentView.power` and `AcademyRow.power`;
  - the plant's six levels served in Welcome.
- **Client:**
  - **Grid lines:** "⚡ 4.4 / 3 MW · Power 68%" on the world page, the system
    roster and the mobile planet sheet and rows.
  - **Status:** brownout chips in the System View and an inbox item with its
    fix.
  - **Build sheet:** facts for draw, supply, fuel and the grid after the order.
  - **Power shares:** on production lines and Academy rows; yards' build rate
    includes power.
- **Bots:**
  - build or raise the plant on their shortest planet (`power_project`);
  - stock plant fuel;
  - research Fusion Reactors when a tier IV plant runs short.
- **Harness:** a `power={…}` line per bot (planets, plants, burn, brownout
  minutes).

Peter's calls:

- **Structures require power.** A bonus-only plant was rejected: "otherwise
  there isn't much point to the power plants."
- **One per planet:** each planet has its own grid and its own plant, not one
  per system.

## In one paragraph

Every planet has its own power grid. Mines, factories, yards and Academies
draw power; civic structures (food, housing, storage, defence) don't. Every
planet supplies a small base for free, enough for tier I structures, so the
founding charter and a fresh colony need no plant. Growing a planet past its
base needs that planet's Power Plant: one per planet, in its own slot. Tiers
I–IV burn Fuel. Tiers V–VI are fusion, opened by research, and burn Helium-3.
A planet that outgrows a tier IV plant has to go fusion, so big worlds
eventually run on Helium-3. When a planet's demand outruns its supply, every
powered structure on it slows by the same share (a **brownout**), shown in
every factor chain as "Power ×0.75". Nothing ever stops completely: the base
never fails.

## Facts that shape the plan

- **Output today:** a factory's rate is rate × tier throughput × staffing ×
  skill × food × research (`world.rs`, the converter loop). Power is one more
  factor, and shown math stays equal to accrued math.
- **Precedent:** fed map nodes already draw upkeep and pause their bonus
  when unfed. Staffing already dilutes every line by the same share when crews
  run short. A brownout reuses both ideas.
- **Tiers:** throughput is 1 / 2.2 / 3.8 / 6 / 8.6 / 11.5 for tiers I–VI. The
  base ceiling is IV; research opens V–VI for each kind
  (`BASE_MAX_STRUCTURE_TIER`, `UnlockStructureTier`).
- **One of each kind per planet:** a structure deepens in tiers in place, so
  one plant per planet falls out of the existing rule.
- **Slot-free precedent:** the Command Center and Service Berth use no pool
  slot (`uses_slot`). The plant joins them, so power never costs a factory
  slot. That matters on small moons with two industrial slots, and on belts
  and gas giants, which have none.
- **Charter-complete homes spread out:** in all eight bot homes (harness, seed
  42), the Mining Complex, Shipyard and Academy stand on two or three bodies
  other than the Command Center's. Each of those bodies runs one or two tier I
  powered structures (2 MW at most). So a base on the Command Center's planet
  alone would cut the first mine's power; every planet needs its own.
- **Stockpiles are per system:** a plant burns from its system's stockpile,
  like every other structure.
- **Helium-3 today:** it comes only from Icy pockets, about 8,000 per 4-player
  galaxy and finite (`belt_report`), plus the Hub's thin book (600 at once,
  refilling 2 a second). Capitals already refuel from it at a colony dock.

## Rules

**What draws power**

- **Powered:** Mining Complex, Volatile Harvester, every factory (Smelter to
  Drive Works, the Fuel Refinery included), Shipyard, Orbital Shipyard,
  Capital Slipway, Ordnance Foundry, and the Academy.
- **Not powered:** the food chain (Bioharvester, Agroplex, Living Terraces),
  Habitat, Warehouses, Command Center, Service Berth, Garrison, Orbital
  Defense and Sensor Array. Food and housing never brown out, so a power
  failure can't starve a colony.
- **Draw:** a powered structure draws its tier throughput in MW (1, 2.2, 3.8,
  6, 8.6, 11.5). Power follows work: a tier VI factory makes 11.5 times a tier
  I and draws 11.5 times as much.
- **Construction:** building doesn't draw power. Only running structures do.

**Supply, per planet**

- **Base:** every planet supplies 3 MW of its own, free and fuel-free. Belts
  and gas giants too.
- **Power Plant:** one per planet, in its own slot (no Resource, Industrial or
  Infrastructure slot). It can stand on any planet, moon, belt or gas giant.
  - **Output:** 8 × tier throughput: 8 / 17.6 / 30.4 / 48 / 68.8 / 92 MW.
  - **Combustion (I–IV):** burns Fuel.
  - **Fusion (V–VI):** burns Helium-3 at a sixth of the Fuel volume (1
    Helium-3 = 6 Fuel, the rate capitals refuel at). It is opened by a research
    programme, Fusion Reactors: the plant's own tier V–VI unlock.
  - **Ceiling on Fuel:** a tier IV plant plus the base is 51 MW. A planet that
    needs more must go fusion, or brown out.
- **Burn follows load:** a plant burns only for power actually used above its
  planet's base: 0.005 Fuel per MW-second (18 Fuel per MW-hour), or 3 Helium-3
  per MW-hour on fusion. Spare capacity costs nothing.
- **Out of fuel:** a plant supplies only what the system's stock covers this
  tick.
- **No sharing:** planets don't share power. A surplus on one planet doesn't
  help another.

**Brownout**

- **Power** = min(1, supply ÷ demand) for each planet, multiplying the rate of
  every powered structure on it: mine and factory output, yard build speed,
  Academy research rate.
- **Never zero:** every planet's base means a colony that runs out of fuel
  still works its refinery and mines at a share. Freighters need no power, so
  imports always get through: no deadlock.

All numbers are starting points for phase 0.

## Economics

*Derived* from the starting numbers at base prices (Fuel 14, Helium-3 75), for
one planet.

| Planet | Demand | Plant | Plant load | Burn |
| --- | --- | --- | --- | --- |
| Charter done: Mining I and Shipyard I on one body | 2 MW | none (base 3) | 0 | 0 |
| First upgrades there: Mining II, Shipyard II | 4.4 MW | Plant I, 8 MW (Power 68% without) | 1.4 MW | 25 Fuel/h |
| Industrial planet: Mining III, Smelter III, Machine Works II, Electronics II, Fuel Refinery II | 14.2 MW | Plant II, 17.6 MW | 11.2 MW | 202 Fuel/h (2.8k Cr/h) |
| Fuel's ceiling | 51 MW | Plant IV, 48 MW | 48 MW | 864 Fuel/h (12k Cr/h) |
| Prize world: eight tier VI structures | 92 MW | Plant VI, fusion, 92 MW | 89 MW | 267 Helium-3/h (20k Cr/h) |

- **The tutorial is untouched:** no charter-complete planet uses more than two
  thirds of its base.
- **The first plant** is needed at the first round of upgrades after the
  charter, on the planet being upgraded.
- **The industrial planet** burns about 3% of what its own Fuel Refinery II
  makes (6,336 an hour). A colony without a refinery imports it: about four
  Tiny Freighter loads an hour.
- **Prize worlds** need fusion: past 51 MW there is no Fuel option. That is
  the "eventually Helium-3".
- **The galaxy:** one prize world burns 267 Helium-3 an hour. The pockets'
  8,000 last about 30 prize-world hours, and the Hub's refill carries about 27
  prize worlds at once, at climbing prices. That is why phase 4 adds a
  renewable source.

## Helium-3 from gas giants

- A Volatile Harvester on a gas giant also yields a trickle of Helium-3 ice.
  The rate is tuned in phase 0 so that one gas giant roughly feeds one fusion
  world.
- It adds no new deposit and no RNG draw (the ore ladder keeps two draws per
  deposit), and gives gas giants, which host no industry today, a reason to
  be settled and held.
- A gas giant can host its own fusion plant and burn its own Helium-3.
- *Alternative:* Ramscoop skimming also takes Helium-3. That is fleet-based
  and is open decision 5.

## UI

Terse labels; hints go in tooltips. No new map circles.

- **Planet panel and planet rows:** a grid line "⚡ 4.4 / 8 MW" (demand /
  supply). During a brownout it reads "Power 68%" in amber.
- **System panel:** each planet's row carries its own grid line; only
  browning-out planets turn amber.
- **Build sheet:**
  - The plant has its own slot on every planet.
  - Each powered card shows its draw ("2.2 MW"), and plant cards show their
    output ("+17.6 MW").
  - Picking an upgrade previews that planet's grid after it ("⚡ 5.6 / 3 →
    Power 54%").
- **Shown math:** "Power ×0.75" in production rows, Academy rows and yard
  rows.
- **Top bar:** the colony chip shows ⚡ when any planet is browning out.
- **Notices** (latched, owner-only, like production outages):
  - "Brownout · Freya II · 68%"
  - "Freya II plant · no Fuel"
  - "Power restored · Freya II"
- **Mobile:** the same rows.

## Bots

- Before an upgrade would push a planet past its supply, a bot builds or
  upgrades that planet's plant.
- Plant fuel joins the industry buffers: two delivery windows of burn.
- They go fusion once the research is done and the plant reaches IV, buying
  Helium-3 at the Hub.
- **Harness:** a `power={…}` line per bot with each planet's demand and
  supply, minutes in brownout, and the Fuel and Helium-3 burned.
- **Prerequisite:** the bots' opening stall must be fixed first (see
  [asteroid-belt-mining-plan.md](asteroid-belt-mining-plan.md), phase 6).

## Charter and guide

- **No new charter step:** every planet's base covers it.
- **Guide:** when a planet's demand reaches 80% of its supply, the
  next-objectives list adds "Build a Power Plant on Freya II" with a reward.

## Sieges and blockades

- Plants run on refined or imported Fuel. A blockade that stops imports drains
  them, and each planet falls back to its base.
- Defenders stockpile fuel; attackers have a new reason to blockade.
- Defence stations are not powered in v1, so a brownout never silences a
  colony's guns (open decision 3).

## Saves

- `StructureKind::PowerPlant` goes last in `ALL`, as new kinds must.
- Protocol bump.
- **Galaxy reset:** existing colonies have no plants and would brown out on
  load. Reset rather than migrate.

## Art

Power Plant tiers I–VI for the planet stage and build cards, in two looks:

- **I–IV, combustion:** turbine halls, stacks and fuel tanks.
- **V–VI, fusion:** reactor domes, field coils and cryogenic Helium-3 tanks.

The plant also stands on belts and gas giants, which have no ground, so it
needs an orbital variant: a platform version of each look.

Briefs use measured framing (buildings about 94% wide, foot at 88%). Until
the art lands, the plant borrows the Fuel Refinery's. A small ⚡ icon is
needed for the grid line.

## Phases

0. **Dry run.**
   - A report from the bot harness and the opening fixture shows each planet's
     demand against its base over time, plant load, Fuel and Helium-3 per hour,
     the share of freight, and the galaxy's Helium-3 balance.
   - It sets the base, MW per plant tier, the burn rate and the gas-giant
     yield.
1. **The grid in the sim:**
   - the Power Plant (I–IV, Fuel) in its own slot, and each planet's base;
   - per-planet demand, and the Power factor on mines, factories, yards and
     Academies;
   - plant burn and notices;
   - view fields and tests;
   - a protocol bump and a galaxy reset.
2. **Client:** the grid line on planets and system rows, the plant's slot,
   build-sheet draw, output and preview, Power in shown math, the top-bar chip
   and mobile. Stand-in art.
3. **Fusion:** the Fusion Reactors research (the plant's tier V–VI unlock),
   the Helium-3 burn, and Iridium in the V–VI build costs.
4. **Helium-3 from gas giants.**
5. **Bots and the harness.**
6. **Art.**

## Tests

**Sim:**

- **Demand:**
  - only powered kinds count, at tier throughput;
  - each planet's base is 3 MW;
  - plant output by tier;
  - planets never share power.
- **Placement:** one plant per planet in its own slot, on any planet, moon,
  belt or gas giant, never taking a pool slot.
- **Brownout:** Power multiplies mine and factory output, yard build speed
  and Academy rate on its own planet only, and shown math equals accrued.
- **Burn:**
  - plants burn only the load above their planet's base;
  - an empty plant supplies nothing, and that planet browns out;
  - fusion burns Helium-3 at a sixth of the Fuel volume.
- **Ceiling:** without fusion, a planet past 51 MW browns out.
- **No deadlock:** a fuel-starved colony still runs its Fuel Refinery at the
  base's share and refuels itself.
- **The charter:** no charter-complete planet browns out.
- **Food:** the food chain and housing are never powered.
- **Notices** latch once and clear on recovery.

**Server:** the view's per-planet grid fields and the factor chains.

**Client:**

- the grid line and brownout amber;
- the plant's slot;
- the build-sheet draw, output and preview;
- Power in production, Academy and yard rows.

**Bots:** a plant before an upgrade that would brown out its planet; plant
fuel in the buffers.

## Decisions still open

1. **What's exempt.**
   - Recommended: food, housing, storage, the Command Center, Service Berth,
     Garrison, defence and sensors.
   - Alternative: everything draws.
2. **Brownout.**
   - Recommended: proportional, every powered structure on the planet slows by
     the same share.
   - Alternative: a priority order the player sets.
3. **Defence stations.**
   - Recommended: not powered in v1.
   - Alternative: powered, as a siege lever.
4. **Fusion's entry.**
   - Recommended: Power Plant V–VI, through the plant's research unlock.
   - Alternative: a separate Fusion Reactor building from tier I.
5. **Renewable Helium-3.**
   - Recommended: gas-giant Volatile Harvesters.
   - Alternative: Ramscoop skimming.
6. **Teaching it.**
   - Recommended: a guide objective after the charter.
   - Alternative: a tenth charter step.
7. **The base.**
   - Recommended: a flat 3 MW on every planet.
   - Alternative: scaled by planet size, or by the Command Center on its own
     planet.
