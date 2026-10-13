# Asteroid mining: plan

Status: parked (Oct 7 2026). Asteroid belts inside star systems come first;
see `docs/asteroid-belt-mining-plan.md`. Proposed Oct 6 2026; nothing is built.
The numbers marked *derived* are arithmetic from today's constants; nothing in
the code states them.

## In one paragraph

Asteroid fields are places in deep space. A freighter fitted with a **Mining
Rig** parks at a field and fills its hold with ore. The ore then has to be
hauled home or to the Hub.

Fields are finite and shared:

- Small starter fields lie near every home.
- Crystalline, Cuprite and Titanium lie farther out.
- Later, a few guarded fields hold Rare-metal ore.

Mining is loud, so it draws the threats that already exist. Fuel already burns
by cargo mass, so distance does the balancing for free: cheap ore only pays
close to home, and valuable ore pays anywhere.

## Why

- **Freighters get a second job.** It needs no claim, no population and no
  planet slot, so it fills the early game between export runs.
- **Asteroid contacts stop being one-shot.** Each home's ore cluster and icy
  comet now hold a single surface cache: 120 Ferrite plus 20–44 Rare Elements,
  or 100 Volatiles plus 20 Fuel. After one Recover the contact is "Depleted"
  forever.
- **A home-region corporation gets a risky route to scarce ore.** Titanium
  (frontier 0.45 or more) and Rare-metal (0.70 or more) come only from frontier
  systems today.
- **It is built from rules the game already has:**
  - the hold is the bottleneck;
  - fuel burns by mass;
  - a scout identifies a contact;
  - depletion travels by light;
  - the Rare-metal book is thin;
  - dwelling is loud.

## What it builds on

| Need | Already in the game |
| --- | --- |
| Places, identification, fog | Asteroid exploration sites: about 15 in a 4-player galaxy, two per home at 14–24k su. A Scout identifies them with Investigate. Light-delayed site reports re-send when a site's `revision` changes. |
| Continuous on-station work | `FleetIndustry::Skim` fills the tank, then the hold, in whole units. Its statuses are travelling, skimming, full and unsafe. |
| Danger while working | `autonomous_exploration_safety`, which withdraws a fleet 25k su. The ×1.5 dwell signature (`SURVEY_SIGNATURE_FACTOR`). Discovery guardians, leashed at 8k su. |
| A freighter fitting | The player freighter's utility berth (Cargo Pods, Fuel Transfer Rig), and the existing `role-mining` icon. |
| Ore value | The ore ladder: prices 6 / 8 / 24 / 110 / 1,700, `ore_bulk_ratio`, the frontier windows, and the thin Rare-metal book. |
| Hauling | System and Hub unload, sell on arrival, `HaulToMarketHub`, Economy cruise. |
| Map and panels | The `site-asteroids` glyph, cyan contact rings, state pips and hover briefs. The site detail on the Operations route. |
| Demand | `MarketDelivery { commodity, units }` contracts. |

## Design

### 1. A field is an asteroid site with a reserve

Add `field: Option<AsteroidField>` to `SiteDetails`. Every site report then
carries a snapshot of the reserve, already light-delayed.

```rust
pub struct AsteroidField {
    pub ore: Commodity,  // one ore per field in v1
    pub richness: f64,   // bulk units/s per rig, like a deposit's richness
    pub reserve: u32,    // ore units left
    pub initial: u32,    // report steps and the "about 60% left" band
}
```

- **The surface cache stays.** Recover takes the surface cache and Mine takes
  the reserve. Neither touches the other's pool.
- **One ore per field.** That keeps the panel, the hover and the balance
  readable. Variety comes from having more fields.
- **Finite in v1.** Fields neither regrow nor respawn, and a spent field greys
  out like any depleted site. Respawn waits for phase 3, when the harness shows
  how fast fields run dry.
- **No Ferrite fields.** Every home already mines Ferrite, and the economics
  below show it doesn't pay to haul. Fields carry the ores a home lacks:
  Cuprite and Crystalline feed Electronics, and Volatiles feed Fuel and
  Polymers.

**Seeding.** Fields are seeded on a new galaxy only, with `--reset-galaxy` and
no migration.

| Field | Where | Ore | Richness | Reserve (ore units) |
| --- | --- | --- | --- | --- |
| Home ore cluster (existing site i = 2) | 14–24k su from home | Cuprite (authored) | 0.30 | 2,000 |
| Home icy comet (existing site i = 3) | 14–24k su from home | Volatiles (authored) | 0.40 | 3,000 |
| New: 2 per home, mid ring | 30–60k su from home | one Crystalline, one Cuprite (authored) | deposit formula | rolled |
| Existing interstellar and star-side asteroid sites | 0.25–0.85 R | rolled | deposit formula | rolled |

- **Why the mid ring is authored.** Homes sit near the core, at a frontier of
  about 0.08. Cuprite only starts at 0.20 and reaches full weight at 0.35, so a
  roll there would almost always give Crystalline. Authoring it gives every home
  the same two Electronics ores within reach.
- **Rolled ore.**
  `roll_raw_deposit_where(rng, frontier, |c| matches!(c, CrystallineOre | CupriteOre | TitaniumOre))`.
  Frontier comes from position, as it does for home neighbourhoods:
  `(|pos| / R − 0.12) / 0.84`. The Titanium window (from 0.45) and its ramp then
  apply on their own.
- **Richness.** The deposit formula: `0.45 × (0.70 + 0.55·frontier) × vein ×
  vein_mult`.
- **Rolled reserve.** A quarter of a planetary finite reserve: 5–10k bulk
  units, converted to ore units with `ore_bulk_ratio`. That gives Crystalline
  5–10k, Cuprite 3.3–6.7k and Titanium 1.3–2.7k.
- **Nebula sites stay as they are.** The Condensate pocket and the Shielded
  remnant recovery remain authored Extract expeditions.

### 2. The Mining Rig, a utility module

- **Fit.** Player freighters only, at 2 fitting points, in the utility berth.
  A rigged hull gives up Cargo Pods, Extended Tanks or a Fuel Transfer Rig. A
  rigged Convoy holds 400, not 800, and carries no extra tanks.
- **Build.** At a staffed Shipyard I. The starting recipe is Machinery 10,
  Alloys 16 and Electronics 4. For comparison, Cargo Pods cost Alloys 18,
  Polymers 12 and Machinery 6.
- **Unlock.** The first rig is found: the home ore cluster's surface cache
  gains one Mining Rig. Recovering it is the move players already learn on the
  founders' beacon. Building more needs a cheap tier-1 Materials node,
  "Prospecting Rigs" (`mat_prospecting_rigs`).
- **Art.** None for v1. The existing `role-mining` icon covers it.

### 3. The Mine order

- **Command.** `MineField { fleet_id, site_id }`, light-delayed like any fleet
  order.
- **When it is valid.**
  - The fleet holds a rigged freighter and isn't disposable.
  - Its owner has identified the site.
  - The site has a field, and the field isn't guarded.
- **State.** `FleetIndustry::Mine { site, mined, fraction, status }` is ticked
  in `tick_industrial_expansion` beside Skim.
  - The fleet works while it is within `SITE_RANGE` (300 su), holding position,
    not engaged and not refitting.
  - Each tick it mines `rigs × richness × ore_bulk_ratio × extraction research
    × DT`.
  - The fraction carries over, and whole units move from the reserve into the
    hold.
  - `UnitsExtracted` counts the bulk-equivalent units, so miners also progress
    the Deep Crust school.
- **Status.** One of travelling, mining, full, depleted, guarded or unsafe. A
  full or depleted fleet holds station. A new course ends the job, as it does
  for skimming.
- **Loud.** `Fleet::surveying()` is also true while mining. The existing ×1.5
  signature then applies everywhere it already does: rival defence,
  weapons-free fire, pirate patrols and research observation.
- **Danger.** `autonomous_exploration_safety` covers mining fleets too. A
  non-combatant miner that sees a threat withdraws 25k su, and the timeline
  says "Mining stopped: threat detected".
- **Reports.** `revision` bumps only when the reserve crosses a 10% step, and
  at zero. That is at most 11 extra reports per field, so other corporations
  learn of depletion by light without report spam.
- **Explicit.** A rigged freighter sent to a field still only does Recover on
  arrival. Mining is loud and has to be ordered. This keeps the existing rule
  that ordinary travel never opts into deep or risky work.

### 4. Hauling and range

Mined ore sits in the hold. In v1 the player hauls it by hand, either unloading
at home or using `HaulToMarketHub` with sell on arrival. Phase 2 adds the loop.

**Loaded range binds before profit does.** The tank is sized by hull mass, each
unit of cargo adds 28 mass, and the dispatch check keeps a 15% margin. An empty
hull of any size reaches about 152k su, because mass cancels out. Derived warp
range with a full hold, at Full speed:

| Hull (hold) | Full-hold range |
| --- | --- |
| Tiny (50) | ~79k su |
| Small (150) | ~57k su |
| Convoy (400) | ~43k su |
| Large (1,000) | ~34k su |
| Heavy (2,500) | ~25k su |
| Bulk (6,000) | ~19k su |

- **Economy cruise** runs at 25% speed and burns 10% of the fuel per distance,
  so it carries ten times as far at four times the travel time.
- **Past about 43k su**, a loaded Convoy has three options: cruise on Economy,
  meet a tender, or unload at a forward system.
- **That forward depot is the expansion hook.** Claim a frontier system near
  the Titanium fields and haul there.

### 5. Economics

*Derived.* One rig, base prices, no research, no market impact, Fuel at 14,
travel at Full unless noted.

| Case | Fill | Cycle | Gross/h | Fuel/h | Net/h |
| --- | --- | --- | --- | --- | --- |
| Reference: T1 Mining Complex on home Ferrite | – | – | 9–13k | – | 9–13k, plus 1,000 workers and a slot |
| Convoy, home Cuprite cluster, 20k su | 33 min | 37 min | 15.7k | 1.9k | **13.9k** |
| Small, the same cluster | 12.5 min | 16 min | 13.6k | 2.0k | **11.7k** |
| Convoy, a Ferrite field like home's (0.40), 20k su | 17 min | 20 min | 9.6k | 3.4k | **6.2k** (why no Ferrite fields) |
| Convoy, mid-ring Cuprite, 45k su, Economy when loaded | 27 min | 46 min | 12.6k | 1.0k | **11.6k** |
| Convoy, frontier Titanium ~160k su out, Economy both ways | 46 min | 2.5 h | 17.3k | 0.4k | **16.9k** |
| Convoy, Titanium 20k su from a forward depot | 46 min | 50 min | 53k | 1.4k | **~52k**, before Authority freight (~7%) |

What the table says:

- **Fuel per unit picks the ore.** Hauling a unit of ore 20k su burns about
  1.6 cr of fuel. That is 20% of Ferrite's price but 1.5% of Titanium's.
- **The rig is the unit of output.** Many small rigged hulls do about as well
  as a few big ones. Big hulls just make fewer trips.
- **The starter cluster roughly matches a T1 Mining Complex,** with no claim
  and no workers, and it lasts about three rig-hours.
- **Mining doesn't try to keep pace with developed worlds.** A T4 complex makes
  six times a T1. Mining is the early job, and the frontier job.
- **A forward depot triples frontier income.** That is the reason to claim
  frontier systems.

### 6. Risk

- **In v1:** loudness, the existing lookout and withdrawal, rivals under normal
  diplomacy, and long blind Economy legs.
- **In phase 3:**
  - Titanium fields spawn with a guardian pack (`spawn_fixed_pirates` plus the
    discovery-guardian leash). They can only be mined once the pack is cleared.
  - Some frontier fields sit near a pirate base, whose guarded supply and
    fire-control sites stand about 14k su out.
  - A Supernova-remnant Rare-metal field burns Polymers as shielding for every
    hour mined. Its ×1.5 signature stacks with mining's own ×1.5.

### 7. UI, with no new rings

- **Map.**
  - An identified field keeps its cyan ring and `site-asteroids` glyph.
  - The pip is green while reserve remains, and the field greys out when spent.
  - The hover brief adds the ore, the amount left and the report's age:
    "Cuprite field · ~1,840 left · report 3 min ago".
  - A mining fleet's label reads "Mining 62%".
- **Site detail (Operations route).**
  - A Field section shows the ore, the reserve as of the report, the rate per
    rig, and a fill estimate for each rigged freighter fleet.
  - Each fleet that can mine gets a "Mine with …" button.
  - Otherwise the section gives the reason: unidentified, no rig, guarded,
    depleted or hold full.
- **Fleet panel.** A status line such as "Mining Cuprite · 0.20/s · hold
  212/400 · full in ~16 min", with a Stop button.
- **Equipment.** "Mining Rig: mines asteroid fields · replaces Cargo Pods".

### 8. Research

- **Wired now:**
  - Prospecting Rigs (new, tier 1) unlocks building rigs.
  - Deep Bores (+15% `ExtractionRate`) applies to rigs.
  - Efficient Burns (−25% fuel) helps hauling.
- **Candidates among existing effects that do nothing yet:**
  - `ProspectingCharters`: field contacts show their ore without a scout.
  - `TraceRefining`: a Rare Elements trace from Cuprite fields.
  - `ExtractionMoons` (Strip Rigs): could become +25% at fields.

### 9. Bots and the harness

- **Bots, from phase 2.** A bot past GrowBusiness with an idle freighter fits a
  rig. It runs Mine & haul on the nearest identified, unguarded field whenever
  the estimate beats its freight route.
- **The harness** logs credits per hour for each activity: complexes, freight
  and mining. That turns the derived table above into measured numbers.

## Phases

0. **Numbers.** A throwaway `mining_economics` test prints the table above from
   the real constants, for each hull, field and distance. It is used to pick
   the rig rate and the reserves.
1. **Mine a field (the MVP).**
   - Sim:
     - field data and seeding;
     - the rig module, its recipe and its research;
     - `MineField` and `FleetIndustry::Mine`;
     - loudness, safety, report steps and research credit.
   - Protocol and view.
   - Client:
     - the site's Field section, with Mine buttons and reasons;
     - the fleet status line and Stop;
     - the hover brief and pip;
     - the equipment entry.
   - A galaxy reset.

   Done when a new player can recover the cluster, fit the rig they found, mine
   Cuprite and haul it home, and a second corporation sees the depletion
   arrive by light.
2. **Mine & haul.**
   - A loop inside `FleetIndustry::Mine`:
     - when full, fly home or to the Hub, optionally selling there, on whichever
       cruise the fuel check allows;
     - unload, refuel, return and resume;
     - stop on depletion or danger.
   - The panel shows the cycle estimate and credits per hour.
   - Bots use the loop, and the harness logs it.
3. **Frontier and risk.**
   - Guarded Titanium fields.
   - The remnant Rare-metal field and its shielding cost.
   - Fields placed near patrols.
   - Respawn of spent frontier fields.
   - `MarketDelivery` ore contracts that point at fields.
4. **Optional depth.**
   - A Mining Barge hull (needs an art brief).
   - Field claims, as a beacon emplacement.
   - The idle research effects listed above.
   - Deep-zoom rocks drawn from the unused `asteroid_a–d` sprites.

## Tests

**Sim:**

- The rig fits only player freighters and takes the utility berth.
- Mining conserves units: the reserve falls by exactly what the hold gains, in
  whole units, with the fraction carried over.
- Full, depleted, guarded and unsafe each stop work with the right status.
- Mining needs identification, and Recover never takes the reserve.
- When two corporations share a field, the second learns of the depletion only
  by light, and revisions step at 10%.
- The signature is ×1.5 while mining, and a threat withdraws a non-combatant
  miner.
- Deep Bores raises the rate, and extracted units count toward research.
- Seeding:
  - every home gets both starter fields;
  - rolled fields respect the frontier windows;
  - v1 has no Ferrite or Rare-metal fields;
  - spacing holds;
  - the same seed gives the same fields.

**Client:**

- The field brief, status, Mine reasons and fill estimate.
- The panel buttons and the fleet line.
- A field in the markers fixture.

## Decisions for Peter

1. **Where fields live.**
   - Recommended: deep space, as upgraded asteroid contacts.
   - Alternative: make the decorative system belts real. That changes body
     generation and mostly adds more Mining Complex play.
2. **Who mines.**
   - Recommended: a rig on existing freighters now, a barge hull later.
   - Alternative: the barge first, which waits on art.
3. **Starter cluster ore.**
   - Recommended: Cuprite. The home lacks it, and it feeds Electronics.
   - Alternative: Crystalline.
4. **How rigs unlock.**
   - Recommended: the first rig is found in the cluster cache, and cheap
     research builds more.
   - Alternative: research only.
5. **Explicit order.**
   - Recommended: an explicit Mine order.
   - Alternative: mining starts automatically when a rigged freighter arrives.
6. **Rare-metal fields.**
   - Recommended: phase 3, guarded.
   - Alternative: include them in v1.

## Art

None is needed for phases 0–2. Later briefs:

- the Mining Barge (GLB, top-down sprite, hero);
- field panel art variants (metallic, icy, remnant), since every asteroid site
  shares one painting today;
- deep-zoom rocks, if the existing `asteroid_a–d` sprites don't fit.
