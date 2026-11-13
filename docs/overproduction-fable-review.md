# Overproduction: Fable's review of the diagnosis and plan

Nov 4 2026. Facts verified against the repo (crates/sim/src). Travian figures from memory, x1.

## Verdict

The diagnosis is right: costs are 10–50× too low against output, and cost is the only gate out of scale. Build time is already Travian-shaped (tier IV 1 h, V 5 h, VI 15 h, one line per world).

The plan's direction is right (×K prices, charter rate). As written it deadlocks storage at charter end, leaves tier II a free lunch, misses the claim cost, and defers the repeating sinks that the hard caps (5 slots × 6 tiers) make mandatory.

## Corrections

Game facts

1. Agroplex is 1 Biomass → 1.2 Provisions (rate 1.2), not 1:1. Converters have their own rates: Machine Works 0.3/s, Semiconductor Fab 0.5/s, Smelter 1.0/s. Only the Smelter is input-bound at home ore rates.
2. A home has three deposits, not one: Biomass 0.38–0.52/s, system ore 0.32–0.45/s, garden Ferrite 0.38/s (galaxy.rs:1618–1705). Post-charter home income at tier I is 10–13 Cr/s, not 3.2.
3. Workforce is not a gate. Each built tier is one 1,000-person post (build.rs:286); Managed migration sends a 1,000-person liner within ~6 min of an empty post, three in flight (migration.rs:23–27). Only Habitat levels (12,500 per level at home) and food pace it. Today's real gates are build time and slots.
4. Tier II costs ×1.0 and takes ×1.0 for ×2.2 output: marginal payback ~5 min, the cheapest step in the game. The plan's curve leaves it there.
5. Upkeep sinks already exist and were missed: Fusion plant fuel, 0.005 Deuterium per MW·s above the solar fields (power.rs:39). A tier-VI planet drawing ~80 MW burns ~27k Deuterium/day ≈ 380k Cr ≈ 4% of its output. Quarry upkeep 0.03 Provisions + 0.005 Machinery per tier per second ≈ 50k Cr/day per tier. Both small; the diagnosis stands.
6. Credit sink missed: claim cost = 600 + 45 × value-rate (galaxy.rs:1364) ≈ 700–900 Cr for a home-grade system, four minutes of one mine. Travian's village (3 settlers + Residence 10 + CP) is 80–100k resources, its biggest mid-game sink. Colony Ships are in the ×K; claims are not.
7. The Hub is not an unbounded credit source. External demand per good is 1,600 units + 8/s refill, galaxy-wide (market.rs BULK_LIQUIDITY): 690k units/day per good shared by every corp. One tier-VI planet with five mines makes 23 ore/s. Sustained flow at the cap moves the price only ~25–33% off base (reversion 0.02/s vs exp(8/1600)); quantity is what caps. Phase 0 must log refused sales or it will misread "credits stop climbing".
8. Ships eat their price fast today: a Corvette (1,945 Cr) eats 138 Provisions/day = 1,245 Cr/day, its price in 1.6 days (Travian 10–30). After ×50 it is 78 days. Note only.
9. Food and research are not "sized well"; they are rate-limited drains (headcount, Academy hours), not elastic sinks. A richer player cannot spend more on them. That is why they absorb no surplus.

Travian

10. Equites Imperatoris costs 1,410 (550/440/320/100) and eats 3. 2,170 is the Equites Caesaris, which eats 4.
11. Settlers: Roman 23.8k each, Teuton 20k, Gaul 18.1k.
12. "Top producer ≈ 5.5× level 1" understates the ladder. Level 10 → 19 d, level 15 ≈ 70 d, level 20 ≈ 4.2M res / +450 per h ≈ 390 d. The full ladder is ~100× level 1. On a year-long server a level-15 field pays back in ~20% of the season; SS tier VI should sit there: ~2 weeks, not 1.
13. Travian also time-gates: a level-10 field ≈ 1.5–2 h, level 20 ≈ 9–10 h with Main Building 1. SS tier IV 1 h / V 5 h / VI 15 h is already that shape.
14. "A full store holds roughly a day": level-10 fields make ~900/h per resource, so 80k ≈ 3.5 days; early warehouses (1,200–4,000) hold 1–3 days.

Maths

15. K=50 with today's storage deadlocks. Storage is one shared total. At charter end: 1,000 landing holds + 300 Storehouse (+700 if Warehouse I). A standard Mining I = 1,850 units, Smelter I 2,250, Machine Works I 3,750, Warehouse I itself 1,900. Nothing standard fits, builds are paid up front, and the plan keeps Warehouse I–II "as they are".
16. Marginal paybacks under the plan's curve at K=50: I 6 h, II 5 h, III 11 h, IV 1 d, V 2.6 d, VI 7 d. Tier II is cheaper per output than tier I.

## Plan

One scalar K on recipes, storage capacities follow K, the charter untouched.

**0. Baseline.** One 14-day harness run on today's build. Per corp per day: output value, Hub sales accepted and refused, credits, time-to-afford the next build, builds blocked by storage. Set K from day-1 post-charter time-to-afford of a Mining I: target 1.5–3 h of whole-home income (Travian's first field ≈ 8 h at 30/h; SS's season is 5× shorter). Expect K ≈ 50.
Go: K lands in 30–80. No-go: surplus appears inside the charter → reprice the charter crates, not a charter rate.

**1. Costs.**
- COST_SCALE K=50 on every recipe: structures, upgrades, the explicit ladders (CC, plant, station from level 2), ships, modules, Colony Ship, Academy training, specialists (800 → 40k).
- TIER_COST_MULT = [1, 2, 6, 18, 54, 162] (×3 from II; time multipliers unchanged). Mining marginal payback at K=50:

  | Tier | I | II | III | IV | V | VI |
  |---|---|---|---|---|---|---|
  | Cost ×K×mult (Cr) | 70k | 139k | 418k | 1.25M | 3.8M | 11.3M |
  | Marginal payback | 6 h | 10 h | 23 h | 2 d | 5 d | 14 d |

  Mirror in the client's TIER_CURVE (client/src/core/derive/market.ts:121) or serve scaled costs.
- Warehouse and Orbital Warehouse recipes exempt (×1, ×1.6 curve as today). Capacities ×K: Warehouse I 35k, +20k per tier, Orbital 100k per tier. Landing holds stay 1,000 (+300) so the charter's storage lesson stands. Invariant to test: Warehouse n holds 1.5× the dearest tier-n structure (I: 35k vs 3,750 ✓). Tier V–VI builds (Mining VI = 300k units) need Orbital Warehouses by design.
- Claim cost = (600 + 45 × value-rate) × K × 1.6^(systems already owned): 2nd system ≈ 70k Cr, 5th ≈ 275k.
- Charter rate, scoped: tier I–II structures and the charter's own ship steps, home system only, while the charter runs AND under 72 h since founding. Build card shows both prices ("12 · std 600"). The chapter-6 last-call crate holds the goods for one standard Mining I + Warehouse I.
- Leave alone: Hub liquidity, bounties, stipends, signals, contracts, ladder (K devalues fixed credit grants 50× in real terms, which is wanted), research baskets, food, requisitions (4% of valuation, self-scaling), combat balance (uniform K cancels in the equal-cost harness).

Go: days 1–3 time-to-afford 1–8 h; Hub sales ≤ 25% of output value from day 7; bots finish the charter ≤ 300 min, claim ≥ 1 colony by day 7, reach ≥ 50% of today's tier-III count by day 14. No-go → change K only, rerun once.

**2. Expansion as the repeating goods sink** (Travian CP + celebrations). Colony Points gate claims: 2nd 1,000, 3rd 2,500, 4th 6,000, 5th 15,000 (×2.5). Points come from Festivals at the Assembly Hall (exists: build.rs ASSEMBLY_HALL_RECIPE). Small = ~1 h of a home's tier-I output (800 Provisions, 600 Alloys, 200 Machinery, 100 Polymers ≈ 37k Cr) → 500 pts. Great = 5× → 3,000 pts. 8 h cooldown per Hall; +migration appeal for 24 h so a solo corp wants it anyway. Repeats forever, buys progress, async-safe. This is the parked Colony Charter idea.
Go: used share ≥ 75% of output value by day 7; median corp ≥ 1 festival/day.

**3. Pirate ramp** (docs/r11-pirate-pressure-plan.md, written, not built): ships die. Weeks 1–2 packs on freighters, week 3 colony raids, weeks 4–6 Reaver/Warbarge.
Go: ≥ 1 hull lost per active corp per 2 days from week 2; no corp loses > 25% fleet value per day; solo bots alive at week 6.

**4. Credits, in order.** (a) Claims, step 1, the main one. (b) Finish-now, tier III+ and ships: price = 2 × remaining fraction × basket at live Hub prices, floor 5k. A pressure valve for 5–15 h builds, not a main sink. (c) Specialists ×K, step 1. Not payroll (see risk 2).
Go: day-14 median balance < 2× day-7.

**5. Only if used share still < 75%:** Machinery spares upkeep for tier III+ at 0.0017 × tier² per second (III 0.015/s, VI 0.061/s ≈ 8–10% of a mine's output value). Short → 50% rate, nothing destroyed.

Not doing: output cuts (the charter is tuned to today's rates; 14 converters, 10 refiners, food, fuel burn and Hub books would all move), payroll, storage loss, pay-during-construction, Keystone.

Travian transfer: carries over — troops die → NPC pirate ramp (PvP losses are unreliable under shields, Leave and light delay); crop upkeep → food (already, crews included); CP + celebrations → Colony Points + Festivals; settlers + Residence → Colony Ship ×K + claims ×K; Smithy → research baskets (already); gold finish-now → credit finish-now. Does not carry: overflow loss and catapult destruction (async-fair), starvation deaths, +25% gold bonus, NPC 1:1 exchange, Wonder.

## Claude's plan, item by item

Targets
- Payback I 6 h / IV 1 d / VI 1 wk → change: I 6 h, IV 2 d, VI 14 d.
- Goods hold up ≥ 50% of upgrades → keep; measure as time-to-afford.
- Storage < 10% at cap → drop; trivial with capacities ×K. Replace: no build blocked by storage > 1 h when the Warehouse is one tier ahead.
- ≥ 75% used from day 2 → keep, by value, from day 7 (early homes legitimately sell ore to import Semiconductors).
- Credits stop climbing after day 7 → change: day-14 median < 2× day-7.
- Charter 300 min → keep; add ≥ 1 colony by day 7.

Phase 0 → keep; add refused Hub sales and time-to-afford.

Phase 1
- ×K all builds, K ≈ 50 → keep.
- Charter rate → keep, scoped (home, structures I–II + charter ship steps, charter running AND < 72 h), both prices shown.
- ×3 from III → change: ×3 from II.
- Warehouse III–VI scale, I–II as is → change: capacities ×K at every tier, recipes exempt. Otherwise deadlock.
- Rescale trader stock, contracts, signals, ladder, Keystone, requisitions, combat → drop all but the last-call crate.
- Add claim cost ×K × 1.6^n and specialists ×K.

Phase 2
- Pirate ramp → keep, unconditional, step 3.
- Weekly ladder as Celebrations → change: Festivals → Colony Points gating claims. Ladder stays cosmetic.
- Upkeep from III, conditional → keep conditional, numbers above.

Phase 3
- Payroll → drop.
- Finish-now → keep, tier III+ and ships, 2× basket at live prices.

Not doing → keep all four.

Risks → all four real; add the three below.

## Top 3 risks

1. **Storage deadlock at charter end** under the plan as written. Fix: capacities ×K from tier I plus the last-call crate. Test: a fresh post-charter bot builds Warehouse I then Mining I with no Hub trip.
2. **Hub quantity cap.** 8 units/s per good galaxy-wide. At K=50, 20 corps importing Semiconductors for Machine Works and Shipyards (750–13.5k each) hit refusals in weeks 2–3. A credit payroll would be fatal here: it forces every corp to sell into that cap, and latecomers can't pay. Watch refused volume; if > 10% of attempts, raise BULK replenish to 20/s, not depth.
3. **Charter stall for the cheap rate.** At ×50 a stalled player builds 5 slots × tier II plus Interceptors at 1/50th price. The 72-h clock, home-only and structures-only scope close it; shields end with the charter anyway. Secondary: a colony's first standard Mining I is 1,850 units = 37 Tiny or 2 Large Freighter loads. Acceptable (Travian hauls 25k per field with 500-unit merchants); watch bot colony founding time.
