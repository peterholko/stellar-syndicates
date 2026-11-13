# Scarcity step 1 runs

> **K was backed out on Nov 11 2026** (Peter: balance the progression,
> not a global multiplier). Kept as a record of how the bots behave when
> builds cost more.

Nov 11 2026. Step 1 of [the scarcity plan](../scarcity-staples-plan.md) (K on
build costs) against the [K 1 baseline](scarcity-baseline.md). Every run: 8
bots, seed 12648430, 3 game days, release build. Bot harness only; nothing
on :8080 or production changed.

## Result: K = 60

K 60 passes every check a 3-day bot run can judge. The two that need bots to
develop past the charter (first colony, level III at day 14) can't be
judged, because the bots stall after the charter even at K 1.

| Check | Target | K 1 | K 80 | K 60 |
|---|---|---|---|---|
| Bots finish the charter | within 300 min | 8/8, 47–228 | 8/8, 51–207 | 8/8, 51–253 |
| Time to afford the next build, days 1–3 (median) | 1–8 hours | 33 min, 3.1 h, 2.3 h | 20 min, 5 h, 10.9 h | 19 min, 8.3 h, 2.1 h |
| Hub sales as a share of output value | 25% or less, from day 7 | 54%, 51%, 51% | 13%, 10%, 56% | 13%, 32%, 56% |
| First colony | by day 7 | none by day 3 | none | none |
| Level III structures | half of K 1's | 5 at day 3 | 7 | 8 |

Day 1's short waits are the charter's own builds at today's price. Day 2's
8.3 hours at K 60 is the one reading just over the band.

## What K does

| | K 1 | K 80 | K 60 |
|---|---|---|---|
| Net credits an hour, day 2 (median) | 60,037 | −25 | 405 |
| Credits at the end of day 2 (median) | 2,857,117 | 578 | 11,625 |
| Output value an hour, day 2 (median) | 139,451 | 101,280 | 98,411 |
| Builds queued, all bots, days 1/2/3 | 67/40/39 | 54/3/13 | 59/12/19 |

With K the bots spend what they earn. At K 1 they bank 60,000 Cr an hour and
end day 3 with 2–5 million each. Output stays about the same; the money goes
into builds instead of the bank.

## Two charter holes, found and fixed

The first K 80 run failed the charter: only 4 of 8 bots finished (303, 453,
730 and 1,251 minutes). Neither cause was K's size:
- **Step 23's Survey Probe wasn't a charter build.** At 80× it cost 170–320
  Semiconductors, and six bots stalled there. It's now the 18th charter
  build: Survey Probe or Scout.
- **A charter ship lost to pirates rebuilt at K.** One bot lost its
  Interceptor and Tiny Freighter at step 23 and sat for three days. Now,
  while the charter runs, a charter ship you have none of, afloat or on the
  slips, rebuilds at today's price (`World::charter_rebuild`). One at a time,
  and only until the charter ends.

All results above are with both fixes.

## What the bots get wrong

Read the post-charter numbers with care:
- **Interceptors at K.** At K 80 most bots spent day 2 on one Interceptor,
  which needs 80 fission reactors, 80 Zephyr drives and about 236 pulse
  lasers. They sold and bought about a million credits of goods a day at the
  Hub without finishing it. That churn is why Hub share climbs to 56% on day
  3, and why day 3's waits ran 28–46 hours at K 80.
- **The same stall as K 1:** starving, one research programme in three
  days, no colonies (separate task: "Fix bots stalling after the charter").

So the bots are a fair check on the charter and on how quickly money runs
out. They are no check on whether K 60 feels right after the charter.

## Next

1. Set :8080 to `COST_SCALE=60`, on your go. A fresh or resumed galaxy picks
   it up at start; no reset is needed for step 1.
2. Playtest past the charter: whether a Mining Complex I (83,640 Cr of goods
   at K 60) and an Interceptor feel like decisions, not chores.
3. Then step 2 (staples), which needs the reset you approved.
