# Overproduction plan

Nov 4 2026. Proposal; nothing built. Review with Fable first ([prompt](overproduction-fable-prompt.md)).

Nov 10 2026: the charter rate became per-step pricing (Phase 1, item 2), so
nothing is cheaper for staying in the charter. Founder protection is a flat
24h from founding and requisitions start when it ends (both built).

## Problem

Players are rarely short of goods. Stores sit at the cap, the surplus goes to the Hub, and credits pile up.

| | Stellar Syndicates | Travian (x1 / x3) |
|---|---|---|
| Cheapest producer pays back | ~7 min: Mining Complex I = 12 Machinery + 25 Alloys (≈1,400 Cr) for ~0.4 ore/s (≈3.2 Cr/s) | level-1 field: ~3 days / ~1 day |
| Top producer pays back | ~2.3× tier I (VI: cost ×6.6, output ×11.5) | level 10 ≈ 5.5× level 1 (~19 / ~6 days) |
| Cost per level | ×1.6 from tier III | ×1.67 fields, ×1.28 buildings |
| A full store holds | ~40 min of one tier-I mine (1,000 holds) | roughly a day of output |
| What holds up the next upgrade | build time (×2,700 at VI), slots, workforce | resources |
| Sinks with no ceiling | none | troops (eat crop, die), celebrations |

Already sized well: food (the founding 7,000 eat ~24k Provisions/day) and research (a tier-IV programme draws ~10k Semiconductors, tier VI ~52k).

Everything else runs out:
- Construction stops once 3–5 development slots per system reach tier VI.
- Research ends after 108 programmes.
- Ships rarely die after the charter.

**Root cause:** production pays back in minutes, so wealth compounds until it hits the hard caps (slots, tier ceiling, storage, build time). Past that point, every unit is surplus.

## Targets

Measured with the balancing harness over days 1–14 after the charter:

1. **Payback:** tier I ≈ 6 h, tier IV ≈ 1 day, tier VI ≈ a week. This is Travian x3's shape; a season matures within ten weeks.
2. **Goods, not build time,** hold up at least half of an active player's upgrades.
3. **Storage:** a system sits at its cap less than 10% of the time.
4. **Use:** from day 2, at least 75% of goods output is used (built, eaten, burned or lost) rather than sold.
5. **Credits:** balances stop climbing after day 7.
6. **Charter:** keeps its pace (bots finish by ~300 min).

## Plan

Order: Fable review → Phase 0 → 1 → 2 → 3.
- Claude builds each phase.
- Peter OKs every harness run and every phase.

### Phase 0: baseline (one harness run, Peter's OK)

- Run the harness on the current build. Record per corp per day:
  - output by good
  - use by sink: construction, ships, research, food, fuel, upkeep, losses
  - Hub sales
  - time at the storage cap
  - credit balance
- Compute payback per tier from the recipes.
- This answers where the surplus starts (inside the charter or after) and gives the starting K.
- **Go/no-go:** if the surplus starts inside the charter, the charter gets its own pass after Phase 1.

### Phase 1: price against output (the main lever)

1. **Standard prices ×K** for every build: structures, upgrades, ships, modules and Colony Ships. Start at K ≈ 50, which takes tier-I payback from ~7 min to ~6 h; the harness sets the final K.
2. **Charter steps at today's prices:** each charter step's own build (the structure and level, or the ship, that the step asks for) costs today's price, once. Every other build costs the standard price from day one, charter or not. The build card says "Charter step". Kits, crates, trader deals and bounties stay as they are. Peter (Nov 10 2026) didn't want any incentive "to slow play and drag out their tutorial completion": a time-boxed rate on every tier I–II build paid a staller.
3. **Steeper tiers:** ×3 a tier from III instead of ×1.6. Tier VI then costs 81× tier I (now 6.6×) and pays back ~28× slower.
4. **Storage follows prices:** Warehouse III–VI capacity scales so a system's best Warehouse holds its next big build, the way Travian's warehouse gates upgrades. Landing holds and Warehouse I–II stay as they are.
5. **Rescale everything priced in goods after the charter:**
   - trader stock, contracts, signals and the weekly ladder
   - Keystone baskets and bounties
   - requisitions (4% of valuation)
   - combat-balance values

**Why prices, not output:**
- It's one multiplier, not a dozen rates (extraction, converters, food, research, fuel, upkeep).
- Food and research keep their size.
- Goods bought at the Hub cost K× the credits, so the price rise also works as a credit sink.
- The charter keeps its numbers.

**Check:** harness against targets 1–4, then a playthrough on :8097. Reset the :8080 galaxy when it ships.

### Phase 2: sinks with no ceiling (Travian's troops and celebrations)

6. **Ships die:** build R11 ([plan](r11-pirate-pressure-plan.md)). At ×K prices, a lost Corvette is a real loss.
7. **The weekly ladder becomes our Celebrations:**
   - goals are set per member, sized to output, and rise each week
   - solo corps get their own ladder
8. **Upkeep from tier III**, only if target 4 still fails:
   - tier III+ structures draw Machinery spares, more at each tier, as the Quarry already does
   - short upkeep slows the structure but never destroys it
   - Travian equivalent: every building level adds population that eats crop

**Check:** harness against target 4.

### Phase 3: credits (only if target 5 still fails)

9. **Payroll:** posts and crews draw credits per person. Unpaid posts stop working.
10. **Finish now:** pay credits to finish construction, priced by the time left.

## Not doing

- **Cutting output rates:** this touches every rate, the charter and the bots.
- **Keystone as the fix:** it's late game.
- **Storage overflow as a sink.**
- **Paying for builds during construction:** builds stay paid up front, so the warehouse gates them.

## Risks

- **Two prices from day one:** a step's own build shows "Charter step" and today's price; everything else shows the standard price. Nothing jumps when the charter ends.
- **Stalling the charter gains nothing:** the discount is one build per step, founder protection is a flat 24h from founding, and requisitions start when protection ends.
- **Bots** were tuned to cheap prices (see the opening-stall fix) and must learn to save.
- **Display:** K multiplies every cost the UI shows, so number formatting needs a check.
