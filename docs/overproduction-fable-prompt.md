# Review request: overproduction in Stellar Syndicates, compared with Travian

You're reviewing an economy diagnosis and a fix plan for **Stellar Syndicates**, a persistent, real-time multiplayer space strategy game inspired by Travian. It's built as a Rust sim and server with a TypeScript/Pixi client.

The designer, Peter, says the game overproduces: players are rarely short of resources. Claude, another model, compared the game with Travian and drafted a plan. Your job is to check that work independently and say where it's wrong.

How to answer:
- Be blunt and terse.
- Peter wants one definite plan (what to do, in order, with go/no-go checks), not a menu of options.
- Numbers beat adjectives.

Sources:
- If you have the repository, verify the game facts against the cited files (paths are relative to the repo root). If you don't, treat them as given.
- The Travian figures are from memory, at x1 speed unless noted. Correct any you know to be wrong.

Constraints from Peter:
- Late-game sinks (the Keystone, a Wonder-like project) don't count. He wants early and mid-game sinks for both goods and credits.
- Syndicates stay voluntary; playing solo stays a full choice.
- Resetting the galaxy is fine until launch.

## 1. Stellar Syndicates today

### Production

- **Formula:** `output = base · tier_throughput · staffing · skill · food` (`crates/sim/src/production.rs`).
  - A deposit's base is its richness: `DEPOSIT_BASE_RICHNESS` = 0.45 (`crates/sim/src/galaxy.rs:1361`).
  - That's about 0.4 raw units/s per deposit at tier I.
- **Converters** (`production.rs` CONVERTERS). They run input-bound at home extraction rates.

  | Converter | Inputs → output |
  |---|---|
  | Smelter | 1.5 ore → 1 Alloy |
  | Agroplex | 1 Biomass → 1 Provisions |
  | Machine Works | 1.2 Alloys + 0.6 Semiconductors → 1 Machinery |
  | Semiconductor Fab | 0.5 Conductive Metals + 0.1 Rare Elements + 0.6 Silicates → 1 Semiconductor |

  The code calls Semiconductors `Electronics`.
- **Tiers I–VI.** Tier IV is the ceiling without research; V–VI need research.

  | Tier | I | II | III | IV | V | VI | Source |
  |---|---|---|---|---|---|---|---|
  | Output (`TIER_THROUGHPUT`) | 1 | 2.2 | 3.8 | 6.0 | 8.6 | 11.5 | `production.rs:26` |
  | Cost (`TIER_COST_MULT`) | 1 | 1 | 1.6 | 2.56 | 4.096 | 6.5536 | `crates/sim/src/build.rs:518` |
  | Build time (`TIER_TIME_MULT`) | 1 | 1 | 20 | 180 | 900 | 2,700 | `build.rs:521` |

### Build costs

Tier-I recipes (`build.rs`, roughly lines 870–1200). They're paid up front and refunded on cancel (`crates/sim/src/world/construction.rs`). Times are base build times.

| Build | Cost |
|---|---|
| Mining Complex | 12 Machinery + 25 Alloys, 18 s |
| Smelter | 15 Machinery + 30 Alloys, 20 s |
| Machine Works, Shipyard | 20 Machinery + 40 Alloys + 15 Semiconductors |
| Warehouse | 30 Alloys + 8 Machinery |
| Corvette | 25 Alloys + 12 Semiconductors + 12 Armaments + 1 Warp Core |
| Colony Ship | 45 Alloys + 15 Machinery + 20 Polymers + 30 Provisions + 2 Warp Cores |

Base prices in credits (`crates/sim/src/market.rs`, around line 140):

| Good | Price (Cr) |
|---|---|
| Metallic Ore | 8 |
| Provisions | 9 |
| Polymers | 16 |
| Alloys | 26 |
| Semiconductors | 34 |
| Armaments | 56 |
| Machinery | 62 |
| Warp Core | 215 |

### Capacity and limits

- **Storage** is one shared total per system (`build.rs:1760–1790`, `galaxy.rs:1106`).
  - Until a Warehouse is built, the landing holds store 1,000 (+300 with the Storehouse).
  - Warehouse I holds 700, +400 per tier (2,700 at VI). An Orbital Warehouse adds 2,000 per tier.
  - At the cap, production stops; nothing is destroyed.
- **Slots:** 3–5 development slots per system (`build.rs:1760`). The census counts 8 colonies as fully grown (`crates/sim/src/season.rs:22`).
- **Workforce:** structures need staff from the population, and posts are capped at free workforce.
- **The Hub** is the NPC exchange at the galaxy centre (`market.rs:56`).
  - Bulk goods: the book is 1,600 units deep and refills 8 units/s per good (~690k/day), with a 1% half-spread.
  - Rare goods: 600 deep, refilling 2/s.
  - Standing logistics orders can ship a stockpile to the Hub and sell it on arrival, automatically (`crates/sim/src/standing.rs`).
- **Season:** maturity reaches 100% within ten weeks (`season.rs:27`).

### Derived numbers

- **Mining Complex I:** ≈1,400 Cr of goods buys ~0.4 ore/s (≈3.2 Cr/s). It pays back in ~7 min, or ~16 min at the tutorial trader's ore price of 3.6 Cr.
- **Smelter I:** pays back in ~8 min of value added.
- **Tier ladder:** marginal payback rises only ~2.3× from tier I to tier VI (going to VI costs 6.55× for +2.9 output).
- **Storage:** one tier-I mine fills the 1,000-unit landing holds in ~40 min.

### Consumption already in place

- **Food:** people eat 40 Provisions per million per second (`crates/sim/src/colony.rs:80`).
  - The founding 7,000 people eat 0.28/s, about 24k a day.
  - Ship crews eat the same ration: a Corvette's 40 crew eat ~138 a day, a Battleship's 812 about 2,800.
- **Research** (`crates/sim/src/research.rs:692–740`) takes 2 h of work at tier I up to 240 h at tier VI, drawing a basket of goods every second.
  - A tier-IV programme totals ~10.4k Semiconductors, 3.1k Machinery and 1.6k Rare Elements.
  - A tier-VI programme totals ~52k Semiconductors, 17k Machinery, 8k Rare Elements and 1.7k Exotic Matter.
  - There are 108 programmes.
- **Fuel** is burned for travel. The Asteroid Quarry draws upkeep in Provisions and Machinery.
- **Ships rarely die after the tutorial.**
  - The season-long pirate ramp (`docs/r11-pirate-pressure-plan.md`) isn't built yet.
  - Fighting between players is limited by protection shields, a Leave option and a 25%-per-day loot cap.

### Credits

- **Sources:**
  - Hub sales
  - the travelling trader buying ore
  - tutorial bounties (e.g. 5,300 for the pirate den)
  - stipends
  - signals (private missions)
  - a daily contract board
- **Sinks:**
  - Hub and trader purchases
  - freight fees (6% + distance; tariffs up to ×3)
  - an exchange penalty of 0–10%
  - a season levy of 5–10% that can rise by up to 15 more points
  - arrears (25%)
  - requisitions (every 2 days, 4% of valuation, minimum 2,000)
  - reinstatement fees
  - research grants
  - specialists (800 each)
  - Sol modules
  - fuel rescue

### The tutorial ("charter")

- 26 steps, tuned so most of it fits the first session. Bots finish by ~300 min.
- It uses tier I–II builds, a travelling trader, six crates, stipends and bounties.
- Full storage already shows up during the tutorial (999/1,000).

## 2. Travian, for comparison (from memory)

- **Fields** cost ~1.67× more each level and produce ~1.4× more (level 9 → 10: 145 → 200 per hour). A level-10 Woodcutter costs ~25k resources in total.
  - Level 1 pays back in ~3 days at x1 (~1 day at x3).
  - Level 10 pays back in ~19 days (~6 days at x3).
- **Buildings** cost ~1.28× more each level. Every level adds population, and each person eats 1 crop an hour.
- **Storage:** a level-20 warehouse or granary holds 80k per resource, roughly a day of output at matching levels. Anything over the cap is lost.
- **Troops:** for example, a Legionnaire costs 400 resources and eats 1 crop an hour; an Equites Imperatoris costs ~2,170 and eats 3.
  - Upkeep runs 1–6 crop an hour per troop, so a troop eats its own price in ~10–30 days.
  - Troops die attacking and defending, and starve when crop runs negative.
  - For active players this is where most mid-game resources go.
- **Expansion:** a new village takes 3 settlers (~18–20k resources each), Residence or Palace levels, and culture points.
  - The culture-point bar rises with each village.
  - Celebrations turn resources into culture points, again and again: a small one is ~20k for 500 CP, a great one ~100k for 2,000 CP.
- **Minor sinks:** Smithy upgrades (20 levels per troop type), hero revives, traps, rebuilding after catapults and rams.
- **The Wonder of the World** is endgame, for top alliances only.
- **Gold** (real money) buys:
  - NPC 1:1 exchange
  - finish-now
  - +25% production
  - the Plus build queue
  - Gold Club farm lists

  Apart from resources there's no in-game currency; silver is only used for the hero auction.

## 3. Claude's findings

| | Stellar Syndicates | Travian (x1 / x3) |
|---|---|---|
| Cheapest producer pays back | ~7 min | ~3 days / ~1 day |
| Top producer pays back | ~2.3× tier I | level 10 ≈ 5.5× level 1 |
| Cost per level | ×1.6 from tier III | ×1.67 fields, ×1.28 buildings |
| A full store holds | ~40 min of one tier-I mine | roughly a day of output |
| What holds up the next upgrade | build time, slots, workforce | resources |
| Sinks with no ceiling | none | troops, celebrations |

1. **Root cause:** production pays back in minutes, while Travian's pays back in days.
   - Wealth compounds until it hits the hard caps: slots, the tier-VI ceiling, storage and build time.
   - Past that point every unit is surplus. It either sits idle at the cap or goes to the Hub and becomes credits.
   - So the goods surplus and the credit surplus are one problem.
2. **The sinks are the right kind but the wrong size.**
   - Food and research are sized well.
   - Construction is one-off and small.
   - Ships are cheap and rarely lost.
3. **Why Travian's sinks work:**
   - Prices are set against output.
   - The biggest sinks (troops, celebrations) are used up and repeat.
   - Every sink buys power or progress.

   A Travian player is always short of resources. A Stellar Syndicates player is held back by time, slots and workforce, and nearly always has too many goods.
4. **Late-game sinks won't help:** they can't fix early and mid-game overproduction.

## 4. What I want from you

Form your view from sections 1–3 before you read the appendix.

1. **Diagnosis:** is it right? What's wrong or missing, whether facts, maths or causes?
2. **Main lever:** pick one and say why.
   - Raise prices by a factor K, with tutorial builds kept at today's prices.
   - Lower output: cut extraction and converter rates, and rescale consumption to match.
   - Something else.
3. **Targets:** are the targets in the appendix the right shape and size for a ten-week, real-time season?
4. **Travian transfer:** which Travian sinks carry over to a real-time game with light delay and asynchronous play, and which don't?
5. **Credit sinks:** payroll, finish-now, or something better? In what order?
6. **Risks:** what are the top risks and exploits? For example, a player stalling the tutorial to keep the cheap tutorial prices.

Answer in this order:
1. **Verdict:** two lines.
2. **Corrections:** facts or maths that are wrong.
3. **Your plan:** numbered, in order, with go/no-go checks.
4. **Claude's plan, item by item:** keep, change or drop, one line each.
5. **Top 3 risks.**

## Appendix: Claude's plan

### Targets

Measured with the balancing harness over days 1–14 after the charter:

1. **Payback:** tier I ≈ 6 h, tier IV ≈ 1 day, tier VI ≈ a week.
2. **Goods, not build time,** hold up at least half of an active player's upgrades.
3. **Storage:** a system sits at its cap less than 10% of the time.
4. **Use:** from day 2, at least 75% of goods output is used (built, eaten, burned or lost) rather than sold.
5. **Credits:** balances stop climbing after day 7.
6. **Charter:** keeps its pace (bots finish by ~300 min).

### Phase 0: baseline

- Run the harness once on the current build. Record per corporation per day:
  - output by good
  - use by sink
  - Hub sales
  - time at the storage cap
  - credit balance
- Compute payback per tier from the recipes.
- This shows where the surplus starts (inside the charter or after) and gives the starting K.
- **Go/no-go:** if the surplus starts inside the charter, the charter gets its own pass after Phase 1.

### Phase 1: price against output (the main lever)

1. **Standard prices ×K** for every build: structures, upgrades, ships, modules and Colony Ships. Start at K ≈ 50, which takes tier-I payback from ~7 min to ~6 h; the harness sets the final K.
2. **Charter rate:** while the charter runs, tier I–II builds keep today's prices. The build card says "Charter rate", and the rate ends with the charter. Kits, crates, trader deals and bounties stay as they are.
3. **Steeper tiers:** ×3 a tier from III instead of ×1.6. Tier VI then costs 81× tier I (now 6.6×) and pays back ~28× slower than tier I.
4. **Storage follows prices:** Warehouse III–VI capacity scales so a system's best Warehouse holds its next big build, the way Travian's warehouse gates upgrades. Landing holds and Warehouse I–II stay as they are.
5. **Rescale everything priced in goods after the charter:**
   - trader stock, contracts, signals and the weekly ladder
   - Keystone baskets and bounties
   - requisitions
   - combat-balance values

**Why prices, not output:**
- It's one multiplier, not a dozen rates.
- Food and research keep their size.
- Goods bought at the Hub cost K× the credits, so the price rise also works as a credit sink.
- The charter keeps its numbers.

**Check:** harness against targets 1–4, then a live playthrough.

### Phase 2: sinks with no ceiling

6. **Ships die:** build the pirate ramp (packs from week 1, heavier pirates in weeks 4–6, raids on colonies from week 3).
7. **The weekly syndicate ladder becomes Travian's Celebrations:**
   - goals are set per member, sized to output, and rise each week
   - solo corporations get their own ladder
8. **Upkeep from tier III**, only if target 4 still fails:
   - tier III+ structures draw Machinery spares, more at each tier, as the Quarry already does
   - short upkeep slows the structure but never destroys it

**Check:** harness against target 4.

### Phase 3: credits (only if target 5 still fails)

9. **Payroll:** posts and crews draw credits per person. Unpaid posts stop working.
10. **Finish now:** pay credits to finish construction, priced by the time left.

### Not doing

- **Cutting output rates.**
- **The Keystone as the fix.**
- **Storage overflow as a sink.**
- **Paying for builds during construction:** builds stay paid up front, so the warehouse gates them.

### Known risks

- **The price jump** at charter end needs explaining.
- **Stalling the charter** to keep the charter rate. Tier III+ is always at standard prices, so the gain is small.
- **Bots** were tuned to cheap prices.
- **Display:** K multiplies every cost the UI shows.
