# Progression balance

Nov 11 2026. From the charter's end to the season's end, across a growing
empire. Every number comes from [progression-sheet.md](progression-sheet.md),
which the sim generates (`crates/sim/src/world/progression_sheet.rs`; rerun
after any tuning). Base prices throughout.

## Built (Nov 11, uncommitted)

Peter's go on the plan below. Three changes:
- **Output ÷20.** `production::OUTPUT_SCALE` (0.05) on every extraction,
  value-making converter, barge, outpost, colony project and Helium-3 skim.
  Production upkeep (quarries, outposts) and food per person
  (`colony::PROVISIONS_PER_MILLION_PER_S`) scale with it. The heavy and
  capital part assemblies keep their rates: they only repackage light parts.
  The camp's food reserve keeps its units (`settlement::CAMP_FOOD_RESERVE_S`
  is ten minutes at the tuned rate): at today's rate it fell to its 40-unit
  floor, under the landing crate's 100 Provisions, and the first Bioharvester
  sat paused from landing ("Food reserve full"). Fixed Nov 11, no reset.
- **Income structures' levels cost what they return**
  (`build::PRODUCTION_TIER_COST_MULT` [1, 3.5, 9, 24, 44, 65], served as
  `BuildOptionView.earns`). On a typical world a level pays back in 2.2 h,
  6.3 h, 12.1 h, 23.5 h, 36.4 h and 2 days. Everything else keeps the gentle
  curve.
- **Claims cost what the colony returns:** 1,000 + about a day and a half of
  its level-I output, then the Authority's ×1.6 a colony held.
- **The charter keeps its pace with an Expansion crate**, released at step 19
  (the Academy). It carries 106 Alloys, 30 Machinery, 10 Polymers and 20
  Provisions for the four builds home production used to pay for: the
  Academy, the second world's Solar Array and Mining Complex or Quarry, and
  the Survey Probe. As with every crate, their Semiconductors are the
  trader's, and the trader still sells no Alloys (Peter, Nov 6). The bot
  charter test passes again.

**After, in hours of that stage's income:**

| Stage | Net Cr/h | All worlds +1 level | Next colony | Programme at stage tier | Stage warship |
|---|---|---|---|---|---|
| Charter end | 1,604 | 9.1 h | 4.0 d | 2.1 h | Interceptor 1.2 h |
| Grip II | 42,963 | 12.8 h | 3.7 h | 47 min | Corvette 14 min |
| Grip III | 58,205 | 15.8 h | 5.6 h | 3.3 h | Destroyer 1.0 h |
| Grip IV | 93,948 | 17.4 h | 5.0 h | 8.8 h | Cruiser 3.3 h |
| Full maturity | 160,229 | — | 9.0 h | 15.9 h | Battleship 9.7 h |

**Still below the band:** single mid-game hulls (a Corvette at grip II, a
Destroyer at grip III) and tier-II research at grip II. Fleets are bought
many hulls at a time, and their upkeep isn't in the sheet yet; the hull
ladder is Peter's from Nov 7, so it's left as is pending his playtest.

**Also seen:** maturity counts programmes, not tiers, so 23 tier-I
programmes (about 77,000 Cr) push a corporation's research half to 46%.

**Galaxy reset:** claim costs are generated with each system, so :8080 was
restarted with `--reset-galaxy` on Nov 11, 14:47, on this build (accounts
kept).

## First read (before the changes)

## Stages

A corporation's growth is half colonies (8 is full) and half research (50 is
full). The grip follows the galaxy's maturity, with a time floor of 100% at
day 70.

| Stage | By day | Colonies | Research | Extractor level | People to crew it | Net Cr/h |
|---|---|---|---|---|---|---|
| Charter end | 0 | 0 | 2 | I | 3,000 | 32,081 |
| Grip II · Levies | 17.5 | 2 | 12 | IV | 36,000 | 859,255 |
| Grip III · Inspections | 31.5 | 4 | 22 | IV | 60,000 | 1,164,098 |
| Grip IV · Martial | 45.5 | 5 | 32 | V | 90,000 | 1,878,966 |
| Full maturity | 70 | 8 | 50 | VI | 156,000 | 3,204,578 |

Income is the ore of the home plus its best colonies (the best of the 12
nearest systems with a habitable world, in a generated galaxy), less the
Provisions their crews eat. Refining roughly doubles a metal's value on top.

## Where the curves break

**1. Production pays for itself in minutes.** Every Mining Complex level
returns its cost in 5–15 minutes, a fed Smelter level in 2–5. Levels I–IV
need no research and take under an hour to build. So a home goes from 36,000
to 216,000 Cr/h of ore within a day, and growth waits only on build and
research times, never on money.

**2. Income multiplies 27× by grip II and 100× by full maturity.** Two
multipliers stack:
- The throughput ladder: level IV makes 6× level I, and VI makes 11.5×.
- Colonies: the best nearby colony is worth about twice the home.

**3. What each stage buys hardly moves.** In hours of that stage's income:

| Stage | Next extractor level | Next colony | Next programme | Stage warship |
|---|---|---|---|---|
| Charter end | 3 min | 49 min | 6 min | Interceptor 4 min |
| Grip II | 0 min | 2 min | 0 min | Corvette 1 min |
| Grip III | 0 min | 2 min | 0 min | Destroyer 3 min |
| Grip IV | 0 min | 1 min | 1 min | Cruiser 10 min |
| Full maturity | — | 1 min | 4 min | Battleship 29 min |

**4. Colonies are the best deal in the game.** A claim costs 820–1,490 Cr
for a system worth 18,000–71,000 Cr/h at level I.

**5. The steep ladders are sound; they're just dwarfed.**
- Research programmes: 3,354 / 33,974 / 189,518 / 831,341 / 2.55M Cr for
  tiers I–V, over 2 / 8 / 24 / 72 / 168 hours.
- Hulls: Interceptor 2,003 → Destroyer 60,049 → Cruiser 308,548 →
  Battleship 1.55M → Dreadnought 15.65M → Titan 156.6M (licences included).

**Also found:**
- **Credits are capped by the Hub's books:** about 8 units a second per bulk
  good at roughly 0.8× base, and well under 1 a second for dense ores. Goods
  matter more than credits.
- **Contracts, bounties and signals are rounding errors:** the daily board
  pays at most about 4,400 Cr, a hideout bounty 600–1,100.
- **Four of the Authority's grip costs are decree text only** and never
  charged: colony licence fees, cargo inspections, the warship registry tax
  and syndicate dues.

## The plan (built above)

**Target.** After the charter, a stage's next meaningful step (an extractor
level, a colony, a programme, the stage's warship) costs a session to a day
of that stage's income: 4–24 hours.

**1. Gathering pays less.** Cut extraction about 20×, and the Provisions a
person eats with it so no world starves. Then the charter's end nets about
1,600 Cr/h: an Interceptor is about an hour, a colony about 16 hours, and
bounties and contracts become real early income.
  This alone fixes the late game (a Battleship about 10 hours at full
  maturity, a Dreadnought about 4 days) but not the middle: at grip II a
  colony would still cost 38 minutes and a Corvette 14.

**2. Growth costs what it returns.** Price extractor and refinery levels by
the output they add, so a level pays back in about a day rather than
minutes. Price claims by the colony's worth, not 600 + 45 × its value-rate.
That stops income multiplying 27× by grip II for a few thousand credits.

**3. Rerun the sheet** until every stage lands in 4–24 hours, then playtest.

**The charter** stays an accelerated start, not balanced. Its
production-bound steps get their goods from its crates and the trader so the
first session still fits.
