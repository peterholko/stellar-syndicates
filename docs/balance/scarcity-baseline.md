# Scarcity baseline (step 0)

> Nov 11 2026: K is backed out; this run is today's economy and the
> starting data for the progression balance.

Nov 10 2026. Step 0 of [the scarcity plan](../scarcity-staples-plan.md): today's
economy (K 1), before any scarcity change.

**Run:** release build frozen at 17:23, 8 bots, seed 12648430, 3 game days
(4,320 minutes), 3 h 7 min wall time, 5.6 GB. Saved each game day, so it can
resume to 7 or 14 days.

The plan asked for 20 bots × 14 days × 3 seeds. That doesn't fit: 20 bots
ran less than one game hour in 17 minutes, and 8 bots take 2.5–4 wall
minutes a game hour.

## What a home earns

Real cash, the credits a bot gains in an hour, is the measure here. Output
value counts each stage of a chain (ore, then the alloys made from it), so
it runs about twice as high.

| Bot | Charter done (min) | Net Cr/h, day 2 | Net Cr/h, day 3 | Hub sales Cr/h, day 2 | Credits, end of day 3 |
|---|---|---|---|---|---|
| 1 | 65 | 69,404 | 70,473 | 77,317 | 4,813,162 |
| 2 | 69 | 49,532 | 50,186 | 58,367 | 3,541,618 |
| 3 | 65 | 67,650 | 69,250 | 77,950 | 4,402,393 |
| 4 | 47 | 46,167 | 49,222 | 54,762 | 3,432,698 |
| 5 | 64 | 57,105 | 75,356 | 71,276 | 4,782,392 |
| 6 | 83 | 42,601 | 47,812 | 49,733 | 2,274,657 |
| 7 | 228 | 79,176 | 83,381 | 91,983 | 5,463,375 |
| 8 | 62 | 62,969 | 60,830 | 73,803 | 4,536,499 |
| **Median** | **65** | **60,037** | | **72,540** | |

Most of it is ore sold to the Hub. At minute 2,880 a home's own mines made
30,000–53,000 Cr an hour, the Mining Barge 6,000–10,000, and belt pockets and
colonies the rest. Hub sales were about half of output value on every day
(54%, 51%, 51%). The Hub refused 989 units on day 2 and 1,100 on day 3, all
at two bots. With 8 bots its 8 units a second per good barely binds.

A standard Mining Complex I (25 Alloys, 12 Machinery) is worth 1,394 Cr. At
K 1 it costs about 1.4 minutes of a median home's net income.

## K

The plan's rule: from day 1 after the charter, a standard Mining Complex I
should take 1.5–3 hours of home income, and K should land between 30 and 80.

| Income measure | K for 1.5 h | K for 2.25 h | K for 3 h |
|---|---|---|---|
| Net cash, median, day 2 (60,037 Cr/h) | 65 | 97 | 129 |
| Hub sales, median, day 2 (72,540 Cr/h) | 78 | 117 | 156 |

**K = 80.** It's the only value inside the 30–80 band that gives 1.5–3 hours
on both measures: 1.86 hours of net income, 1.54 hours of sales. Income
still rises on day 3, so it errs toward too cheap. At K 80 a Mining Complex
costs 111,520 Cr of goods at level I and 18 million at level VI (the steep
curve's ×162).

## The bots don't develop after the charter

All eight finish the charter quickly (47–228 minutes). After that, with
millions in hand, they barely move:

| | Day 1 | Day 2 | Day 3 |
|---|---|---|---|
| Builds queued, all bots | 67 | 40 | 39 |
| Median wait for a build's goods (min) | 33 | 185 | 139 |
| Structures at level III or higher, all bots | 2 | 4 | 5 |
| Colonies | 0 | 0 | 0 |

- **Starving:** seven bots starved more than 4,100 of 4,320 minutes (bot 7:
  2,849). Population stays at the founding 8,000–12,000.
- **Research:** six bots finished one programme in three days. Bot 3
  finished 3 and bot 7 finished 9.
- **Ship parts:** most stall on "awaiting zephyr drive 1", "awaiting hull
  sections 13" or "awaiting machinery 13", goods they never source.
  Meanwhile some keep building Interceptors (bot 5 has 28).
- **Expansion:** never ready at any bot.

So credits aren't what limits the bots, and K won't change that. The step 1
checks that need bots to develop, first colony and level III counts, already
fail at K 1. Until the bots can feed themselves, research and source ship
parts, those checks can't judge K.

## Go/no-go baseline for step 1

| Check | Target | Baseline (K 1) |
|---|---|---|
| Time to afford the next build, days 1–3 | 1–8 hours | median 33, 185, 139 min |
| Hub sales as a share of output value | 25% or less, from day 7 | 51–54% (days 1–3) |
| Bots finish the charter | within 300 minutes | all 8, 47–228 min |
| First colony | by day 7 | none by day 3 |
| Level III structures | half of today's, at day 14 | 5 at day 3 |
