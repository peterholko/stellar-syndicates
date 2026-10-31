# Travian: Legends — the "just wait and build" economy (own resource production, hard numbers, x1 base)

> Source legend used throughout. **Official** = Travian: Legends Help Center (support.travian.com, 2025–2026 pages). **kirilloid model** = the source code behind kirilloid's Travian tables/calculators (github.com/kirilloid/travian). Its server list maps "legends → regular" to model `t4` and "legends → speed" to `t4-x3`. Its newest Legends-specific overrides are dated in-code to Fire and Sand (2017) and Path to Pandora (2018), so treat it as **T4/Legends-era community data that may predate 2019–2026 tweaks**. **Corroboration** = two independent community codebases that hard-code the same Legends tables: Pillage First (a Travian-like game) and voaneves/travian-building-order (a Travian crop-ROI optimiser). Where a table is "computed", I expanded the kirilloid formula myself with a script. Checks against the independent tables are noted.

## 1. Resource field production per hour by level (0–20) at x1, and which levels are capital-only

### Takeaway
In Legends every field type (woodcutter, clay pit, iron mine, cropland) uses the same production curve: the old T3 curve ×1.4, rounded. That gives 3/h at level 0, 7 at L1, 46 at L5, 280 at L10, 1,120 at L15 and 3,430 at L20. Non-capital villages stop at level 10 (12 for "cities" on special worlds), so levels 11–20 are capital-only.

### Cited Findings
- The kirilloid model defines Legends field output as `prod4 = Math.round(prod(lvl) * 1.4)`, applied identically to Clay pit, Woodcutter, Iron mine and Cropland — [kirilloid t4/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/t4/buildings.ts). The base (T3) curve is `[2, 5, 9, 15, 22, 33, 50, 70, 100, 145, 200, 280, 375, 495, 635, 800, 1000, 1300, 1600, 2000, 2450, 3050]` for levels 0–21 — [kirilloid base/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/base/buildings.ts).
- The model maps Legends regular servers to `t4` and Legends speed servers to `t4-x3`. Kingdoms is a separate model, `t5` — [kirilloid model/index.ts](https://github.com/kirilloid/travian/blob/master/src/model/index.ts).
- Independent corroboration of the Legends curve: Pillage First hard-codes `3, 7, 13, 21, 31, 46, 70, 98, 140, 203, 280, 392, 525, 693, 889, 1120, 1400, 1820, 2240, 2800, 3430` as base resource production per level — [Pillage First buildings.ts](https://github.com/jurerotar/Pillage-First-Ask-Questions-Later/blob/6970d77681a7d0ac35d26594aadb726a825119d5/packages/game-assets/src/buildings.ts). voaneves' optimiser uses the same array, except it has **691** at L13 instead of 693, and adds 4,270 at L21 — [voaneves index.html](https://github.com/voaneves/travian-building-order/blob/78657e7f76cb24c980f51fd1c4dce08ac575219d/index.html).
- The old T3 values (5/h at L1, 200 at L10, 2,450 at L20) are **outdated for Legends**. An old T3 tutorial notes that "Level 10 increases output by 55 to 200 total" — [CMU Travian upgrade-cost page](https://www.cs.cmu.edu/~bhudson/src/travian/upgrade-cost.html) (seen via search snippet only). **Travian Kingdoms** still uses the un-multiplied T3 curve (`f: prod` in [kirilloid t5/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/t5/buildings.ts)), so support.kingdoms.com numbers do not apply to Legends.
- Production table (Legends values from the kirilloid ×1.4 rule, matching Pillage First; speed columns use the official multipliers ×3/×5/×10 from [Official: Game Versions and Speed](https://support.travian.com/en/articles/20-game-versions-and-speed)):

| Level | T3 (old, outdated) /h | **Legends x1 /h** | Added vs previous level | x3 /h | x5 /h | x10 /h |
|---|---|---|---|---|---|---|
| 0 | 2 | **3** | – | 9 | 15 | 30 |
| 1 | 5 | **7** | 4 | 21 | 35 | 70 |
| 2 | 9 | **13** | 6 | 39 | 65 | 130 |
| 3 | 15 | **21** | 8 | 63 | 105 | 210 |
| 4 | 22 | **31** | 10 | 93 | 155 | 310 |
| 5 | 33 | **46** | 15 | 138 | 230 | 460 |
| 6 | 50 | **70** | 24 | 210 | 350 | 700 |
| 7 | 70 | **98** | 28 | 294 | 490 | 980 |
| 8 | 100 | **140** | 42 | 420 | 700 | 1,400 |
| 9 | 145 | **203** | 63 | 609 | 1,015 | 2,030 |
| 10 | 200 | **280** | 77 | 840 | 1,400 | 2,800 |
| 11 (capital only) | 280 | **392** | 112 | 1,176 | 1,960 | 3,920 |
| 12 (capital only; non-capital "cities" on special worlds) | 375 | **525** | 133 | 1,575 | 2,625 | 5,250 |
| 13 (capital only) | 495 | **693** (691 in one tool) | 168 | 2,079 | 3,465 | 6,930 |
| 14 (capital only) | 635 | **889** | 196 | 2,667 | 4,445 | 8,890 |
| 15 (capital only) | 800 | **1,120** | 231 | 3,360 | 5,600 | 11,200 |
| 16 (capital only) | 1,000 | **1,400** | 280 | 4,200 | 7,000 | 14,000 |
| 17 (capital only) | 1,300 | **1,820** | 420 | 5,460 | 9,100 | 18,200 |
| 18 (capital only) | 1,600 | **2,240** | 420 | 6,720 | 11,200 | 22,400 |
| 19 (capital only) | 2,000 | **2,800** | 560 | 8,400 | 14,000 | 28,000 |
| 20 (capital only) | 2,450 | **3,430** | 630 | 10,290 | 17,150 | 34,300 |

- Level caps: "in regular villages the maximum level is 10, cities allow up to level 12, and capital villages have no hard limit" — [Official: Quick Summary: Buildings and Construction](https://support.travian.com/en/articles/221-quick-summary-buildings-and-construction). The same caps (10 regular, 12 cities on special worlds, capital uncapped except by cost and storage) appear in [Official: Guide: Resources Explained](https://support.travian.com/en/articles/214-guide-resources-explained).
- The kirilloid base model sets `m:20` (max level) for woodcutter and iron mine and `m:21` for clay pit and cropland — [kirilloid base/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/base/buildings.ts).
- Starting village layout: 6 croplands and 4 each of lumber, clay and iron fields. Special spots have 9, 15 or (on some special worlds) 18 croplands — [Official: Guide: Resources Explained](https://support.travian.com/en/articles/214-guide-resources-explained).
- Production is credited continuously ("produce automatically over time"). Production above storage capacity is lost while the fields keep running — [Official: Guide: Resources Explained](https://support.travian.com/en/articles/214-guide-resources-explained).

### Inferences
- A fresh 4-4-4-6 village with all 18 fields at level 0 produces 12 wood, 12 clay, 12 iron and 18 crop per hour (54/h in total) before upkeep. In the early game the hero's default resource points produce more than this (see Q3).
- Production grows about ×1.4 per level in the mid range, while cost grows ×1.67 per level (Q2). Each extra level therefore pays back about 1.2× more slowly than the one before. An L20 field produces 12.25× an L10 field, but the cumulative cost to reach it is about 170× higher (10.6M vs 62.6k resources).
- Because all four types share one curve, the only economic difference between field types is their upgrade cost mix and their population (iron mines cost about 8% more and carry more population).

### Gaps
- I could not read the official in-game "Buildings and Resource Fields Statistics" tool (it is rendered interactively; [Official article 33](https://support.travian.com/en/articles/33-buildings-and-resource-fields-statistics) contains no table text), and travian.fandom.com returned HTTP 402. The Legends curve therefore rests on community code (kirilloid plus two independent tools that agree), not on an official table.
- I could not confirm whether a level 21 exists in Legends capitals. The official text says "no hard limit", while kirilloid caps fields at 20/21. In practice, storage makes levels above 20 unreachable (Q2/Q5).

## 2. Upgrade cost and build time per level for the four field types, and payback hours

### Takeaway
Field costs grow ×1.67 per level (rounded to 5) and build times ×1.6 per level, from tiny L1 bases. Because production grows more slowly, payback (total upgrade cost ÷ added production) rises steadily. At x1 it is about 2.6–5.6 days for L1–L6, 8–10 days for L7–L9, 13.7 days for L10, 22 days for L12, 59 days for L15, 152 days for L18 and 282 days for L20. On speed servers payback in hours divides by the speed factor, because costs do not scale.

### Cited Findings
- Cost formula: `cost(L) = round5(base × k^(L−1))` with k = 1.67 for all four field types. L1 base costs (wood/clay/iron/crop): Woodcutter 40/100/50/60, Clay pit 80/40/80/50, Iron mine 100/80/30/60, Cropland 70/90/70/20 — [kirilloid base/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/base/buildings.ts). The Legends model (t4) inherits these unchanged — [kirilloid t4/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/t4/buildings.ts).
- Build-time formula (seconds, Main Building level 1, x1): `t(L) = a × 1.6^(L−1) − 1000/3`, with a = 1780/3 (woodcutter), 1660/3 (clay pit), 2350/3 (iron mine), 1450/3 (cropland). The Main Building multiplies times by `0.964^(MB−1)`: MB5 ≈ 0.864, MB10 ≈ 0.719, MB15 ≈ 0.599, MB20 ≈ 0.498 — [kirilloid base/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/base/buildings.ts). Construction time is divided by speed (/2, /3, /5, /10) — [Official: Game Versions and Speed](https://support.travian.com/en/articles/20-game-versions-and-speed).
- Population per level (each population point eats 1 crop/h): L1 adds the base value `u`, and each later level adds `round((5u + L − 1)/10)`. u = 2 for woodcutter and clay pit, 3 for iron mine, 0 for cropland — [kirilloid base/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/base/buildings.ts). Official: "The village consumes crop each hour equal to its population", and a level 1 Woodcutter consumes "2 crop per hour" — [Official: Guide: Resources Explained](https://support.travian.com/en/articles/214-guide-resources-explained); [Official: Problems with Crop](https://support.travian.com/en/articles/75-problems-with-crop).
- Cross-check: voaneves hard-codes cropland total cost per level as 250, 415, 695, 1,165, 1,945, 3,250, 5,425, 9,055, 15,125, 25,255, 42,180, 70,445, 117,650, 196,445, 328,070, 547,880, 914,960, 1,527,985, 2,551,735, 4,261,410. That is identical to the formula expansion below, except L13 (117,640 vs 117,650, a rounding difference) — [voaneves index.html](https://github.com/voaneves/travian-building-order/blob/78657e7f76cb24c980f51fd1c4dce08ac575219d/index.html).
- Build queue: "Romans can build two things simultaneously by default: one in the village center and one in the resource fields". "With a Plus Account active, a third order can be queued", but "this third order only begins processing once one of the first two is complete" — [Official: Quick Summary: Buildings and Construction](https://support.travian.com/en/articles/221-quick-summary-buildings-and-construction).

**Per-level cost and time tables.** These are computed from the kirilloid formula above. Times are at MB level 1 and x1, rounded to 10 s. Prod is Legends x1.

Woodcutter (cumulative pop: L1 2, L5 6, L10 16, L15 26, L20 41)

| Lvl | Wood | Clay | Iron | Crop | Total | Time (MB1, x1) | Prod/h | +Prod | +Pop |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 40 | 100 | 50 | 60 | 250 | 0:04:20 | 7 | 4 | 2 |
| 2 | 65 | 165 | 85 | 100 | 415 | 0:10:20 | 13 | 6 | 1 |
| 3 | 110 | 280 | 140 | 165 | 695 | 0:19:50 | 21 | 8 | 1 |
| 4 | 185 | 465 | 235 | 280 | 1,165 | 0:35:00 | 31 | 10 | 1 |
| 5 | 310 | 780 | 390 | 465 | 1,945 | 0:59:20 | 46 | 15 | 1 |
| 6 | 520 | 1,300 | 650 | 780 | 3,250 | 1:38:10 | 70 | 24 | 2 |
| 7 | 870 | 2,170 | 1,085 | 1,300 | 5,425 | 2:40:20 | 98 | 28 | 2 |
| 8 | 1,450 | 3,625 | 1,810 | 2,175 | 9,060 | 4:19:50 | 140 | 42 | 2 |
| 9 | 2,420 | 6,050 | 3,025 | 3,630 | 15,125 | 6:59:10 | 203 | 63 | 2 |
| 10 | 4,040 | 10,105 | 5,050 | 6,060 | 25,255 | 11:14:00 | 280 | 77 | 2 |
| 11 | 6,750 | 16,870 | 8,435 | 10,125 | 42,180 | 18:01:40 | 392 | 112 | 2 |
| 12 | 11,270 | 28,175 | 14,090 | 16,905 | 70,440 | 28:54:10 | 525 | 133 | 2 |
| 13 | 18,820 | 47,055 | 23,525 | 28,230 | 117,630 | 46:18:00 | 693 | 168 | 2 |
| 14 | 31,430 | 78,580 | 39,290 | 47,150 | 196,450 | 74:08:00 | 889 | 196 | 2 |
| 15 | 52,490 | 131,230 | 65,615 | 78,740 | 328,075 | 118:40:10 | 1,120 | 231 | 2 |
| 16 | 87,660 | 219,155 | 109,575 | 131,490 | 547,880 | 189:55:30 | 1,400 | 280 | 3 |
| 17 | 146,395 | 365,985 | 182,995 | 219,590 | 914,965 | 303:56:10 | 1,820 | 420 | 3 |
| 18 | 244,480 | 611,195 | 305,600 | 366,715 | 1,527,990 | 486:21:20 | 2,240 | 420 | 3 |
| 19 | 408,280 | 1,020,695 | 510,350 | 612,420 | 2,551,745 | 778:13:20 | 2,800 | 560 | 3 |
| 20 | 681,825 | 1,704,565 | 852,280 | 1,022,740 | 4,261,410 | 1245:12:50 | 3,430 | 630 | 3 |

Clay pit (population identical to the woodcutter)

| Lvl | Wood | Clay | Iron | Crop | Total | Time (MB1, x1) |
|---|---|---|---|---|---|---|
| 1 | 80 | 40 | 80 | 50 | 250 | 0:03:40 |
| 2 | 135 | 65 | 135 | 85 | 420 | 0:09:10 |
| 3 | 225 | 110 | 225 | 140 | 700 | 0:18:00 |
| 4 | 375 | 185 | 375 | 235 | 1,170 | 0:32:10 |
| 5 | 620 | 310 | 620 | 390 | 1,940 | 0:54:50 |
| 6 | 1,040 | 520 | 1,040 | 650 | 3,250 | 1:31:10 |
| 7 | 1,735 | 870 | 1,735 | 1,085 | 5,425 | 2:29:10 |
| 8 | 2,900 | 1,450 | 2,900 | 1,810 | 9,060 | 4:02:00 |
| 9 | 4,840 | 2,420 | 4,840 | 3,025 | 15,125 | 6:30:30 |
| 10 | 8,080 | 4,040 | 8,080 | 5,050 | 25,250 | 10:28:10 |
| 11 | 13,500 | 6,750 | 13,500 | 8,435 | 42,185 | 16:48:30 |
| 12 | 22,540 | 11,270 | 22,540 | 14,090 | 70,440 | 26:56:50 |
| 13 | 37,645 | 18,820 | 37,645 | 23,525 | 117,635 | 43:10:20 |
| 14 | 62,865 | 31,430 | 62,865 | 39,290 | 196,450 | 69:07:50 |
| 15 | 104,985 | 52,490 | 104,985 | 65,615 | 328,075 | 110:39:50 |
| 16 | 175,320 | 87,660 | 175,320 | 109,575 | 547,875 | 177:07:00 |
| 17 | 292,790 | 146,395 | 292,790 | 182,995 | 914,970 | 283:26:30 |
| 18 | 488,955 | 244,480 | 488,955 | 305,600 | 1,527,990 | 453:33:40 |
| 19 | 816,555 | 408,280 | 816,555 | 510,350 | 2,551,740 | 725:45:10 |
| 20 | 1,363,650 | 681,825 | 1,363,650 | 852,280 | 4,261,405 | 1161:15:40 |

Iron mine (cumulative pop: L1 3, L5 11, L10 21, L15 36, L20 51)

| Lvl | Wood | Clay | Iron | Crop | Total | Time (MB1, x1) |
|---|---|---|---|---|---|---|
| 1 | 100 | 80 | 30 | 60 | 270 | 0:07:30 |
| 2 | 165 | 135 | 50 | 100 | 450 | 0:15:20 |
| 3 | 280 | 225 | 85 | 165 | 755 | 0:27:50 |
| 4 | 465 | 375 | 140 | 280 | 1,260 | 0:48:00 |
| 5 | 780 | 620 | 235 | 465 | 2,100 | 1:20:00 |
| 6 | 1,300 | 1,040 | 390 | 780 | 3,510 | 2:11:20 |
| 7 | 2,170 | 1,735 | 650 | 1,300 | 5,855 | 3:33:30 |
| 8 | 3,625 | 2,900 | 1,085 | 2,175 | 9,785 | 5:44:50 |
| 9 | 6,050 | 4,840 | 1,815 | 3,630 | 16,335 | 9:15:10 |
| 10 | 10,105 | 8,080 | 3,030 | 6,060 | 27,275 | 14:51:40 |
| 11 | 16,870 | 13,500 | 5,060 | 10,125 | 45,555 | 23:50:00 |
| 12 | 28,175 | 22,540 | 8,455 | 16,905 | 76,075 | 38:11:10 |
| 13 | 47,055 | 37,645 | 14,115 | 28,230 | 127,045 | 61:09:20 |
| 14 | 78,580 | 62,865 | 23,575 | 47,150 | 212,170 | 97:54:10 |
| 15 | 131,230 | 104,985 | 39,370 | 78,740 | 354,325 | 156:42:00 |
| 16 | 219,155 | 175,320 | 65,745 | 131,490 | 591,710 | 250:46:30 |
| 17 | 365,985 | 292,790 | 109,795 | 219,590 | 988,160 | 401:17:40 |
| 18 | 611,195 | 488,955 | 183,360 | 366,715 | 1,650,225 | 642:07:40 |
| 19 | 1,020,695 | 816,555 | 306,210 | 612,420 | 2,755,880 | 1027:27:30 |
| 20 | 1,704,565 | 1,363,650 | 511,370 | 1,022,740 | 4,602,325 | 1643:59:30 |

Cropland (cumulative pop: L1–L5 0, L10 5, L15 10, L20 20)

| Lvl | Wood | Clay | Iron | Crop | Total | Time (MB1, x1) |
|---|---|---|---|---|---|---|
| 1 | 70 | 90 | 70 | 20 | 250 | 0:02:30 |
| 2 | 115 | 150 | 115 | 35 | 415 | 0:07:20 |
| 3 | 195 | 250 | 195 | 55 | 695 | 0:15:00 |
| 4 | 325 | 420 | 325 | 95 | 1,165 | 0:27:30 |
| 5 | 545 | 700 | 545 | 155 | 1,945 | 0:47:10 |
| 6 | 910 | 1,170 | 910 | 260 | 3,250 | 1:18:50 |
| 7 | 1,520 | 1,950 | 1,520 | 435 | 5,425 | 2:09:40 |
| 8 | 2,535 | 3,260 | 2,535 | 725 | 9,055 | 3:30:40 |
| 9 | 4,235 | 5,445 | 4,235 | 1,210 | 15,125 | 5:40:30 |
| 10 | 7,070 | 9,095 | 7,070 | 2,020 | 25,255 | 9:08:00 |
| 11 | 11,810 | 15,185 | 11,810 | 3,375 | 42,180 | 14:40:10 |
| 12 | 19,725 | 25,360 | 19,725 | 5,635 | 70,445 | 23:31:40 |
| 13 | 32,940 | 42,350 | 32,940 | 9,410 | 117,640 | 37:41:50 |
| 14 | 55,005 | 70,720 | 55,005 | 15,715 | 196,445 | 60:22:20 |
| 15 | 91,860 | 118,105 | 91,860 | 26,245 | 328,070 | 96:39:10 |
| 16 | 153,405 | 197,240 | 153,405 | 43,830 | 547,880 | 154:41:50 |
| 17 | 256,190 | 329,385 | 256,190 | 73,195 | 914,960 | 247:34:20 |
| 18 | 427,835 | 550,075 | 427,835 | 122,240 | 1,527,985 | 396:10:10 |
| 19 | 714,485 | 918,625 | 714,485 | 204,140 | 2,551,735 | 633:55:40 |
| 20 | 1,193,195 | 1,534,105 | 1,193,195 | 340,915 | 4,261,410 | 1014:20:30 |

### Inferences
**Payback table (computed): payback hours = total upgrade cost (all four resources summed, treated 1:1) ÷ added production per hour of that level.** "Net" subtracts the level's added population upkeep (1 crop/h per pop, also treated 1:1). The x3 and x10 columns divide the x1 gross payback by speed, because production scales and costs do not. The "+25% gold" column divides x1 gross by 1.25.

| Lvl | +Prod/h (x1) | Cost: woodcutter / clay pit / cropland (≈ equal) | Cost: iron mine | **Payback h, gross (W/C/Cr)** | Payback h, iron mine | Payback h, net of pop (woodcutter) | Payback h, net (cropland) | **Payback days x1** | Payback h at x3 | Payback h at x10 | Payback h x1 with +25% gold |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 4 | 250 | 270 | 62.5 | 67.5 | 125.0 | 62.5 | 2.6 | 20.8 | 6.3 | 50.0 |
| 2 | 6 | 415–420 | 450 | 69.2 | 75.0 | 83.0 | 69.2 | 2.9 | 23.1 | 6.9 | 55.3 |
| 3 | 8 | 695–700 | 755 | 86.9 | 94.4 | 99.3 | 86.9 | 3.6 | 29.0 | 8.7 | 69.5 |
| 4 | 10 | 1,165–1,170 | 1,260 | 116.5 | 126.0 | 129.4 | 116.5 | 4.9 | 38.8 | 11.7 | 93.2 |
| 5 | 15 | 1,940–1,945 | 2,100 | 129.7 | 140.0 | 138.9 | 129.7 | 5.4 | 43.2 | 13.0 | 103.7 |
| 6 | 24 | 3,250 | 3,510 | 135.4 | 146.2 | 147.7 | 141.3 | 5.6 | 45.1 | 13.5 | 108.3 |
| 7 | 28 | 5,425 | 5,855 | 193.8 | 209.1 | 208.7 | 200.9 | 8.1 | 64.6 | 19.4 | 155.0 |
| 8 | 42 | 9,055–9,060 | 9,785 | 215.7 | 233.0 | 226.5 | 220.9 | 9.0 | 71.9 | 21.6 | 172.6 |
| 9 | 63 | 15,125 | 16,335 | 240.1 | 259.3 | 248.0 | 244.0 | 10.0 | 80.0 | 24.0 | 192.1 |
| 10 | 77 | 25,250–25,255 | 27,275 | 328.0 | 354.2 | 336.7 | 332.3 | 13.7 | 109.3 | 32.8 | 262.4 |
| 11 | 112 | 42,180–42,185 | 45,555 | 376.6 | 406.7 | 383.5 | 380.0 | 15.7 | 125.5 | 37.7 | 301.3 |
| 12 | 133 | 70,440–70,445 | 76,075 | 529.6 | 572.0 | 537.7 | 533.7 | 22.1 | 176.5 | 53.0 | 423.7 |
| 13 | 168 | 117,630–117,640 | 127,045 | 700.2 | 756.2 | 708.6 | 704.4 | 29.2 | 233.4 | 70.0 | 560.2 |
| 14 | 196 | 196,445–196,450 | 212,170 | 1,002.3 | 1,082.5 | 1,012.6 | 1,007.4 | 41.8 | 334.1 | 100.2 | 801.8 |
| 15 | 231 | 328,070–328,075 | 354,325 | 1,420.2 | 1,533.9 | 1,432.6 | 1,426.4 | 59.2 | 473.4 | 142.0 | 1,136.2 |
| 16 | 280 | 547,875–547,880 | 591,710 | 1,956.7 | 2,113.2 | 1,977.9 | 1,970.8 | 81.5 | 652.2 | 195.7 | 1,565.4 |
| 17 | 420 | 914,960–914,970 | 988,160 | 2,178.5 | 2,352.8 | 2,194.2 | 2,188.9 | 90.8 | 726.2 | 217.9 | 1,742.8 |
| 18 | 420 | 1,527,985–1,527,990 | 1,650,225 | 3,638.1 | 3,929.1 | 3,664.2 | 3,655.5 | 151.6 | 1,212.7 | 363.8 | 2,910.5 |
| 19 | 560 | 2,551,735–2,551,745 | 2,755,880 | 4,556.7 | 4,921.2 | 4,581.2 | 4,573.0 | 189.9 | 1,518.9 | 455.7 | 3,645.4 |
| 20 | 630 | 4,261,405–4,261,410 | 4,602,325 | 6,764.1 | 7,305.3 | 6,796.5 | 6,785.7 | 281.8 | 2,254.7 | 676.4 | 5,411.3 |

- **Shape of the curve.** Payback rises from about 62 h (L1) to about 130 h (L5–L6), then about 200–240 h (L7–L9), 328 h (L10), about 530 h (L12), 1,420 h (L15) and 6,764 h (L20) at x1. An x1 round's Natar Wonder completes after about 250 days ([Official: Game Versions and Speed](https://support.travian.com/en/articles/20-game-versions-and-speed)), so capital fields at L18 and above (≥151 days payback) cannot repay themselves within the round unless built very early. They also can't be built early, because they need over a million of a single resource on hand.
- **Bonuses shorten field payback proportionally.** Oases, mills and the gold bonus multiply the field's base output, so in a 15-cropper with high crop bonuses a crop field's payback falls roughly 3–4×. Example: if bonuses stack additively as +150% oases +50% mills, then ×1.25 gold, a field's output is multiplied by 3 × 1.25 = 3.75, and an L10 cropland's 328 h becomes about 87 h. This is why crop capitals push croplands far beyond L10.
- **Population drag is only material at L1.** An L1 woodcutter adds +4/h but 2 pop, so its net payback is 125 h. An L1 iron mine adds +4/h with 3 pop, so net payback is 270 h. From L2 upward the pop correction is under 10%.
- **Whole-village cost to reach a uniform level** (4-4-4-6, all 18 fields; 15-cropper is within about 1.5%): all-L1 4,580 resources → 126/h; all-L2 12,210 → 234/h; all-L3 24,980 → 378/h; all-L5 81,960 → 828/h; all-L7 240,870 → 1,764/h; all-L8 406,820 → 2,520/h; all-L10 1,146,560 → 5,040/h (base, before bonuses and upkeep).
- **Build-time floor (single field queue, x1).** Summed build time for all 18 fields of a 4-4-4-6 to L5 is about 38 h at MB1. To L10 it is about 519 h (21.6 days) at MB1, about 373 h at MB10 and about 259 h at MB20. On x3 these are about 13 h and 173/124/86 h.
- **Storage gates the high levels.** An L12 woodcutter needs 28,175 clay in one warehouse (warehouse L15, 31,300). L14 needs 78,580 clay (warehouse L20, 80,000). L15+ needs more than one maxed warehouse/granary (see Q5).

### Gaps
- Exact in-game rounding of build times. My values are the kirilloid formula rounded to 10 s. The official MB time factor was not found; 0.964 per level is kirilloid's.
- No official per-level cost table was obtainable (see Q1 gaps). Costs rest on the kirilloid formula plus voaneves' matching cropland table.

## 3. Production multipliers and their costs: mills, oases, gold production bonus / Plus, hero

### Takeaway
- **Mills:** Sawmill, Brickyard, Iron Foundry, Grain Mill and Bakery each give +5% per level to max +25% at L5, so crop can get +50% from two buildings. They need L10 fields (L5 cropland for the Grain Mill) and MB5.
- **Oases:** +25% each (or +25% resource plus +25% crop; +50% crop oases exist), up to 3 per village, unlocked at Hero's Mansion 10/15/20. They apply to base production.
- **Gold bonus:** +25% per resource for 5 gold each per 7 days at x1 (3 days on faster worlds). The official page says it applies to total production.
- **Hero:** each point in Resources gives 9 of every resource or 30 of one resource per hour at x1 (Egyptians 12/40), scaling with speed, and the +25% bonus applies to it too.

### Cited Findings
**Mills (kirilloid model; Legends inherits)**
- Base L1 costs (wood/clay/iron/crop), cost growth k = 1.80, `m:5`, benefit `percent(5)` (+5% per level). Build time `a × 1.5^(L−1) − 2400` s at MB1:

| Building | L1 cost | Build-time constant a | Cited prerequisites |
|---|---|---|---|
| Sawmill | 520/380/290/90 | 5,400 | Woodcutter 10, MB 5 |
| Brickyard | 440/480/320/50 | 5,240 | Clay pit 10, MB 5 |
| Iron Foundry | 200/450/510/120 | 6,480 | Iron mine 10, MB 5 |
| Grain Mill | 500/440/380/1,240 | 4,240 | Cropland 5, MB 5 |
| Bakery | 1,200/1,480/870/1,600 | 6,080 | Cropland 10, Grain Mill 5, MB 5 |

  Source: [kirilloid base/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/base/buildings.ts). Caveat: kirilloid's code keys the sawmill/brickyard field prerequisites with swapped 0-based IDs. The pairing above (own field type at L10) is the standard one, and Travian Kingdoms' official page states "Requirements: Woodcutter Level 10, Main Building Level 5" for the sawmill ([Kingdoms support, resource fields](https://support.kingdoms.com/en/support/solutions/articles/7000087799-resource-fields), via search snippet; sister game).
- The Grain Mill's maximum is level 5 — [Official: Quick Summary: Buildings and Construction](https://support.travian.com/en/articles/221-quick-summary-buildings-and-construction). Pillage First models resource boosters as ×1.05 per level, capped at 6 entries (L0–L5) — [Pillage First buildings.ts](https://github.com/jurerotar/Pillage-First-Ask-Questions-Later/blob/6970d77681a7d0ac35d26594aadb726a825119d5/packages/game-assets/src/buildings.ts).
- Computed per-level costs and times (kirilloid formula). Totals cross-check exactly with voaneves' Grain Mill (2,560 / 4,605 / 8,295 / 14,925 / 26,875) and Bakery (5,150 / 9,270 / 16,690 / 30,035 / 54,060) tables — [voaneves index.html](https://github.com/voaneves/travian-building-order/blob/78657e7f76cb24c980f51fd1c4dce08ac575219d/index.html):

| Building | Lvl | Wood | Clay | Iron | Crop | Total | Time (MB1, x1) | Bonus |
|---|---|---|---|---|---|---|---|---|
| Sawmill | 1 | 520 | 380 | 290 | 90 | 1,280 | 0:50:00 | +5% |
| Sawmill | 2 | 935 | 685 | 520 | 160 | 2,300 | 1:35:00 | +10% |
| Sawmill | 3 | 1,685 | 1,230 | 940 | 290 | 4,145 | 2:42:30 | +15% |
| Sawmill | 4 | 3,035 | 2,215 | 1,690 | 525 | 7,465 | 4:23:40 | +20% |
| Sawmill | 5 | 5,460 | 3,990 | 3,045 | 945 | 13,440 | 6:55:40 | +25% |
| Brickyard | 1 | 440 | 480 | 320 | 50 | 1,290 | 0:47:20 | +5% |
| Brickyard | 2 | 790 | 865 | 575 | 90 | 2,320 | 1:31:00 | +10% |
| Brickyard | 3 | 1,425 | 1,555 | 1,035 | 160 | 4,175 | 2:36:30 | +15% |
| Brickyard | 4 | 2,565 | 2,800 | 1,865 | 290 | 7,520 | 4:14:40 | +20% |
| Brickyard | 5 | 4,620 | 5,040 | 3,360 | 525 | 13,545 | 6:42:10 | +25% |
| Iron Foundry | 1 | 200 | 450 | 510 | 120 | 1,280 | 1:08:00 | +5% |
| Iron Foundry | 2 | 360 | 810 | 920 | 215 | 2,305 | 2:02:00 | +10% |
| Iron Foundry | 3 | 650 | 1,460 | 1,650 | 390 | 4,150 | 3:23:00 | +15% |
| Iron Foundry | 4 | 1,165 | 2,625 | 2,975 | 700 | 7,465 | 5:24:30 | +20% |
| Iron Foundry | 5 | 2,100 | 4,725 | 5,355 | 1,260 | 13,440 | 8:26:40 | +25% |
| Grain Mill | 1 | 500 | 440 | 380 | 1,240 | 2,560 | 0:30:40 | +5% |
| Grain Mill | 2 | 900 | 790 | 685 | 2,230 | 4,605 | 1:06:00 | +10% |
| Grain Mill | 3 | 1,620 | 1,425 | 1,230 | 4,020 | 8,295 | 1:59:00 | +15% |
| Grain Mill | 4 | 2,915 | 2,565 | 2,215 | 7,230 | 14,925 | 3:18:30 | +20% |
| Grain Mill | 5 | 5,250 | 4,620 | 3,990 | 13,015 | 26,875 | 5:17:40 | +25% |
| Bakery | 1 | 1,200 | 1,480 | 870 | 1,600 | 5,150 | 1:01:20 | +5% |
| Bakery | 2 | 2,160 | 2,665 | 1,565 | 2,880 | 9,270 | 1:52:00 | +10% |
| Bakery | 3 | 3,890 | 4,795 | 2,820 | 5,185 | 16,690 | 3:08:00 | +15% |
| Bakery | 4 | 7,000 | 8,630 | 5,075 | 9,330 | 30,035 | 5:02:00 | +20% |
| Bakery | 5 | 12,595 | 15,535 | 9,135 | 16,795 | 54,060 | 7:53:00 | +25% |

**Oases (official)**
- Types: Type 1 gives one resource "+25%". Type 2 gives one resource +25% plus +25% crop. Type 3 gives +50% but "only in the grey Natarian area" and only for lumber, clay and iron. Each village controls "up to three" oases, at Hero's Mansion L10 → 1, L15 → 2, L20 → 3. Natural oases must be within 3 tiles. The bonus "applies only to that village's base production". An unoccupied lumber oasis (Type 1) at x1 holds stock it regenerates (40 lumber, 10 clay, 10 iron, 11 crop per hour; capacity 1,000), and raiding one takes up to 10% of its stock, which regenerates within 10 minutes — [Official: Oasis](https://support.travian.com/en/articles/48-oasis).
- 50% crop oases exist: the official capital guide recommends 15-croppers with "125–150% Oases", or at least "one 50% oasis", and says "a 7-cropper with 150% oases" has "slightly better crop output than a 9-cropper +100% or 15-cropper +50%" — [Official: Types of capitals and their development](https://support.travian.com/en/articles/173-types-of-capitals-and-their-development).
- Hero's Mansion cost conflict. kirilloid's Legends model overrides the L1 cost to 80/120/70/90 (k = 1.33) — [kirilloid t4/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/t4/buildings.ts). voaneves instead uses cumulative Mansion costs of 114,240 (to L10), +383,315 (L10→15) and +1,595,070 (L15→20), which match the T3 cost of 700/670/700/240 — [voaneves index.html](https://github.com/voaneves/travian-building-order/blob/78657e7f76cb24c980f51fd1c4dce08ac575219d/index.html).
- Egyptian Waterworks: officially "Increases oasis bonuses, boosting resource production", with no figures — [Official: The Egyptians and Their Advantages](https://support.travian.com/en/articles/9-the-egyptians-and-their-advantages). In community models it is +5% per level on the oasis bonus (`percent(5)`), requires Hero's Mansion 10, and costs 650/670/650/240 (k = 1.28), revised in Path to Pandora (2018) to 910/945/910/340 (k = 1.31) — [kirilloid t4.fs/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/t4.fs/buildings.ts), [kirilloid t4.pp/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/t4.pp/buildings.ts). voaneves applies it as `oasis × (1 + 0.05 × level)`, so L20 doubles every oasis bonus. Its first-level cost of 3,105 equals the 910+945+910+340 Path to Pandora cost — [voaneves index.html](https://github.com/voaneves/travian-building-order/blob/78657e7f76cb24c980f51fd1c4dce08ac575219d/index.html).

**Gold production bonus and Plus (official)**
- "Each production bonus costs 5 Gold" and "increases the selected resource's production by 25%". It lasts "7 days on 1x speed servers" and "3 days on 3x speed servers". It can auto-extend. "The +25% boost applies to the total production of the selected resource, not just the base rate" — [Official: Gold Feature: Production Bonuses](https://support.travian.com/en/articles/129-gold-feature-production-bonuses).
- Resource-bonus and Travian Plus duration: 7 days at x1, 3 days at x2/x3/x5/x10 — [Official: Game Versions and Speed](https://support.travian.com/en/articles/20-game-versions-and-speed).
- New accounts start with "130 gold". The beginner guide advises: "Activate all 25% resource bonuses (20 gold total — 5 per resource)" — [Official: First steps in the game](https://support.travian.com/en/articles/142-first-steps-in-the-game).
- Plus itself is a convenience feature (for example, it lets a third build order be queued), not a production multiplier — [Official: Quick Summary: Buildings and Construction](https://support.travian.com/en/articles/221-quick-summary-buildings-and-construction).
- A "video bonus" is "activated for a specific resource after watching a video", described as the free alternative to the gold bonus. No size or duration is given — [Official: Guide: Resources Explained](https://support.travian.com/en/articles/214-guide-resources-explained).
- Stacking: "Free crop" counts oasis and Grain Mill/Bakery bonuses but excludes "the hero production bonus and the Plus account crop bonus" — [Official: Problems with Crop](https://support.travian.com/en/articles/75-problems-with-crop). The community optimiser models total = `fields × (1 + Σoases·(1+0.05·Waterworks) + mills·5%)`, with gold as a separate ×1.25 — [voaneves index.html](https://github.com/voaneves/travian-building-order/blob/78657e7f76cb24c980f51fd1c4dce08ac575219d/index.html).

**Hero resource production (official, current)**
- Verbatim table "Hero Resource Production", column "Resource Production per Point (x1 speed)": All tribes except Egyptians get "9 resources of each type", or "30 of the selected resource" when focused. Egyptians get "12 resources of each type" or "40 of the selected resource". The note "+25% resource bonus applies" is attached, and "Your hero's resource production increases with game speed (e.g. x2, x3 worlds)" — [Official: Hero in the early game](https://support.travian.com/en/articles/141-hero-in-the-early-game).
- The hero starts with 4 skill points, all assigned to resource production by default. Each level-up grants 4 more points. The resource target can be changed at any time — [Official: Hero overview](https://support.travian.com/en/articles/45-hero-overview).
- Hero production goes to the hero's home village — [Official: Guide: Resources Explained](https://support.travian.com/en/articles/214-guide-resources-explained). The hero "always produces 6 crop per hour" regardless of skill allocation, even when dead — [Official: Problems with Crop](https://support.travian.com/en/articles/75-problems-with-crop).
- **Older and outdated hero rates:**
  - The kirilloid T4 model computes hero resources as `points × 6` (Egyptians ×2) — [kirilloid t4/hero.ts](https://github.com/kirilloid/travian/blob/master/src/model/t4/hero.ts), [kirilloid t4.fs/hero.ts](https://github.com/kirilloid/travian/blob/master/src/model/t4.fs/hero.ts).
  - An older forum post gives 3 × points of each resource, or 10 × points of a single resource — [CodeGuru forum](https://forums.codeguru.com/showthread.php?p=2076569).
  - The current official 9/30 is three times the 3/10 rate.

**Special-world governors (2026)**
- On "Rise of Governors" worlds (an Annual Special 2026 Season Pass feature), the governor Cleopatra at level 5 gives a first +25% crop bonus with no requirements. A second +25% requires a Hero's Mansion with at least one unused oasis slot. The two bonuses are "added together" — [Official: Rise of Governors FAQ](https://support.travian.com/en/articles/230-rise-of-governors-faq). The oasis page adds that Cleopatra's bonus "is calculated from base production and added on top" — [Official: Oasis](https://support.travian.com/en/articles/48-oasis).

### Inferences
- **Mill payback** (computed: level cost ÷ (5% × base output of that resource)):
  - 4 woodcutters at L10 (1,120/h): each Sawmill level adds 56/h. Payback is 23 h (L1), 41 h, 74 h, 133 h and 240 h (L5).
  - 6 croplands at L10 (1,680/h; +84/h per level): Grain Mill takes 30 → 55 → 99 → 178 → 320 h; Bakery takes 61 → 110 → 199 → 358 → 644 h.
  - 15-cropper at L10 (4,200/h; +210/h per level): Grain Mill 12–128 h; Bakery 25–257 h.
  - At L5 fields (184/h for 4 fields) the same mills would take 139–5,876 h, which is moot because the prerequisites require L10 fields.
  - **Conclusion:** mill levels 1–3 on L10 fields pay back faster than any field upgrade above L6. Bakery L5 on a 4-4-4-6 (644 h) is about as slow as a field L12–L13.
- **Oasis payback.** A +25% crop oasis on 6 croplands at L10 adds 420 crop/h. With kirilloid's Legends Mansion cost (≈17.8k resources to L10, my approximation of the k = 1.33 series) the Mansion pays back in about 42 h. With voaneves' T3-style cost (114k) it takes about 272 h. Clearing the oasis's animals with troops is extra and not costed here. A 15-cropper at L10 with 3 × 50% crop oases gets +6,300 crop/h on 4,200 base.
- **Gold bonus value (x1).** All four bonuses cost 20 gold per 7 days. On an all-L5 4-4-4-6 (828/h) they add 207/h, or about 34,800 resources per week (about 1,740 resources per gold). On an all-L10 village (5,040/h) they add 1,260/h, or about 211,700 per week (about 10,600 per gold). On x3 an all-L10 village gets 15,120/h, so the 3-day bonus yields about 272,000 per 20 gold (about 13,600 per gold). Gold therefore buys more resources per gold the more developed the account and the faster the server.
- **Hero vs fields.** The default 4 hero points produce 36 of each resource (144/h) or 120 of one, about 2.7× a fresh village's 54/h field output. Each hero level adds 4 points; if all go to resources that is +144/h, roughly the gain from taking about 6 fields from L5 to L6. A hero point is free (comes with XP), so early-game raiding by the hero carries an opportunity cost: points put into strength produce nothing.

### Gaps
- The exact official stacking formula (additive vs multiplicative between oasis, mill and gold) is not documented. The official gold page says "total production", the oasis page says "base production", and community tools model them additively plus a separate ×1.25 gold multiplier.
- No official Waterworks numbers (the 5%-per-level figure is community-modelled), no official Hero's Mansion cost (sources conflict), and no video-bonus size or duration.
- Real-money price of gold and price of Travian Plus in gold were not found.
- The official oasis page's type naming (no "Type 3" for crop) versus "50% crop oases" in the capital guide could not be reconciled verbatim. Most likely a +50% crop oasis is the crop entry under Type 2.

## 4. Village layouts and the total hourly production a village/account reaches at milestones

### Takeaway
Layouts differ only in how the 18 fields are split, so total base output at a given uniform field level is identical. Examples: 126/h all-L1, 828/h all-L5, 2,520/h all-L8, 5,040/h all-L10. Croppers (7/9/15, and 18 on special worlds) shift that total into crop and pair with crop oases. No credible community benchmarks for day 1/7/30/90 production were found. A labelled toy model suggests a pure "wait and build" single village at x1 reaches roughly 0.3–1.3k/h by day 7 and about 5–6k/h by day 30 (non-capital cap), and a capital about 17k/h by day 90.

### Cited Findings
- Normal start = 4-4-4-6 (6 croplands, 4 each of the others). Croppers have 9, 15 or, on some special worlds, 18 croplands — [Official: Guide: Resources Explained](https://support.travian.com/en/articles/214-guide-resources-explained).
- On the map, a 4-4-4-6 tile shows two trees in opposite corners, while a 4-3-5-6 tile shows two stacks. 9- and 15-cropper tiles look almost empty but have small markings — [Official: Discovering Map Features](https://support.travian.com/en/articles/155-discovering-map-features) (via search snippet).
- Official capital guidance:
  - 15-croppers with "125–150% Oases" are "very rare and highly competitive". 15-croppers with "75–100% Oases" are more common, and at least one 50% oasis is preferred. A 15-cropper with "25–50% or No Oases" is not the best choice.
  - The balanced alternatives are a 9-cropper with "100–150% Oases" or a 7-cropper with "150% Oases".
  - Capital targets: Main Building L20, all resource fields L8, one field of each type L10, and crop fields upgraded past L10. Grain Mill and Bakery are recommended for 15-croppers only.
  
  Source: [Official: Types of capitals and their development](https://support.travian.com/en/articles/173-types-of-capitals-and-their-development).
- First-steps guide: plan the second village on a "15-cropper, 9-cropper, or 7-cropper location with nearby oases" — [Official: First steps in the game](https://support.travian.com/en/articles/142-first-steps-in-the-game). "Your first main goal in Travian: Legends is to settle a second village as soon as possible" — [Official: Developing your first villages](https://support.travian.com/en/articles/177-developing-your-first-villages).
- Round calendar at x1: artefacts on day 90, construction plans on day 180, and the Natar Wonder finishes after 250 days. Registration closes after 70 days. The 2nd village needs 2,000 CP, and accounts start with 500 CP — [Official: Game Versions and Speed](https://support.travian.com/en/articles/20-game-versions-and-speed).

### Inferences
**Base production by layout at uniform field level (x1, wood/clay/iron/crop per hour; production table from Q1):**

| All fields at | 4-4-4-6 | 3-4-5-6* | 7-cropper (e.g. 4-4-3-7)* | 9-cropper 3-3-3-9 | 15-cropper 1-1-1-15 | Total/h | Field pop (4-4-4-6 / 15c) |
|---|---|---|---|---|---|---|---|
| L0 | 12/12/12/18 | 9/12/15/18 | 12/12/9/21 | 9/9/9/27 | 3/3/3/45 | 54 | 0 / 0 |
| L1 | 28/28/28/42 | 21/28/35/42 | 28/28/21/49 | 21/21/21/63 | 7/7/7/105 | 126 | 28 / 7 |
| L3 | 84/84/84/126 | 63/84/105/126 | 84/84/63/147 | 63/63/63/189 | 21/21/21/315 | 378 | – |
| L5 | 184/184/184/276 | 138/184/230/276 | 184/184/138/322 | 138/138/138/414 | 46/46/46/690 | 828 | 92 / 23 |
| L8 | 560/560/560/840 | 420/560/700/840 | 560/560/420/980 | 420/420/420/1,260 | 140/140/140/2,100 | 2,520 | 182 / 86 |
| L10 | 1,120/1,120/1,120/1,680 | 840/1,120/1,400/1,680 | 1,120/1,120/840/1,960 | 840/840/840/2,520 | 280/280/280/4,200 | 5,040 | 242 / 128 |

\*Only "4-4-4-6", "4-3-5-6", "7-cropper", "9-cropper" and "15-cropper" are named in the official sources above. The exact permutations of the 6-crop and 7-crop types are community naming and not separately sourced here.

- With bonuses: a 15-cropper at L10 with +150% crop oases, +50% mills and +25% gold produces 4,200 × (1 + 1.5 + 0.5) × 1.25 ≈ 15,750 crop/h, assuming the community's additive stacking. Without bonuses it produces 4,200. This is the main way a single village's output is multiplied without field levels above 10.

**Toy "pure wait-and-build" model (my model, not community data).** One 4-4-4-6 village at x1 that only upgrades its lowest field with its own income. It pools all four resources 1:1, uses a single build queue at MB1 (or MB10 time factor), subtracts field population upkeep, and ignores storage limits, quests, adventures, raids and settling. Starting stock is 0. Script: kirilloid formulas above.

| Day | Fields only (no hero, no gold) | + hero 4 pts on all resources (+144/h) | + hero + gold +25% on fields | Same, capital allowed past L10 (MB10 times) |
|---|---|---|---|---|
| 1 | 62/h (avg field L0.3) | 245/h (L1.1) | 291/h (L1.2) | 291/h |
| 3 | 101/h (L1.1) | 422/h (L2.7) | 554/h (L3.0) | 554/h |
| 5 | 184/h (L1.9) | 616/h (L3.9) | 870/h (L4.3) | 870/h |
| 7 | 298/h (L2.8) | 852/h (L4.9) | 1,309/h (L5.4) | 1,309/h |
| 14 | 979/h (L5.6) | 2,040/h (L7.4) | 3,187/h (L8.1) | 3,417/h (L8.2) |
| 30 | 3,890/h (L9.3) | 4,940/h (all L10, capped) | 6,200/h (capped) | 8,138/h (avg L10.8) |
| 60 | 4,796/h (capped) | 4,940/h | 6,200/h | 13,518/h (avg L12.5) |
| 90 | 4,796/h | 4,940/h | 6,200/h | 17,340/h (avg L13.4) |

- **Reading the model.** A non-capital village saturates at about 5–6k/h of base output around day 25–30 at x1. After that, account production grows only by adding villages, oases, mills and hero levels, which is why the official guides push settling a second village early. A capital's growth slows sharply after about day 30 as payback stretches past 20–60 days per level. At x3 the same curves compress about 3× in time, with output ×3.
- The model is conservative on the hero (no level-ups, no quest or adventure resources) and optimistic on storage (no warehouse costs or caps). Treat it as an order-of-magnitude baseline, not a benchmark.

### Gaps
- I found **no credible community benchmarks** for account production at day 1/7/30/90 (Reddit and forum searches returned nothing usable). The only pages quoting such ranges were low-quality SEO calculator sites, which I excluded.
- How fast real accounts settle villages 2–N (which dominates account output after day ~15) was not researched here.

## 5. Warehouse and granary capacity by level (caps on hoarding) and crop upkeep (population and troops)

### Takeaway
A village holds 800 of each resource with no storage, and 1,200 → 11,800 (L10) → 80,000 (L20) per warehouse or granary. Storage buildings can be built more than once, and Great versions hold ×3. Overflow is lost. Each population point eats 1 crop/h; troops eat 1–6 crop/h each. When crop runs out, troops starve but buildings do not.

### Cited Findings
- Capacity formula: `round100(2120 × 1.2^L − 1320)`, giving 800 at L0 (no building). Warehouse L1 cost 130/160/90/40, granary 80/100/70/20, both k = 1.28. Both can be built multiple times (`r:{m:true}`). Build time `3875 × 1.16^(L−1) − 1875` s (warehouse) and `3475 × …` (granary) at MB1 — [kirilloid base/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/base/buildings.ts). The Great Warehouse and Great Granary hold `3 × capacity` and need MB10 — [kirilloid t3/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/t3/buildings.ts).
- Computed table (kirilloid formula):

| Lvl | Capacity (each warehouse / granary) | Warehouse cost W/C/I/Cr | Granary cost W/C/I/Cr | Warehouse time (MB1, x1) |
|---|---|---|---|---|
| 0 (none) | 800 | – | – | – |
| 1 | 1,200 | 130/160/90/40 | 80/100/70/20 | 0:33:20 |
| 2 | 1,700 | 165/205/115/50 | 100/130/90/25 | 0:43:40 |
| 3 | 2,300 | 215/260/145/65 | 130/165/115/35 | 0:55:40 |
| 4 | 3,100 | 275/335/190/85 | 170/210/145/40 | 1:09:30 |
| 5 | 4,000 | 350/430/240/105 | 215/270/190/55 | 1:25:40 |
| 6 | 5,000 | 445/550/310/135 | 275/345/240/70 | 1:44:20 |
| 7 | 6,300 | 570/705/395/175 | 350/440/310/90 | 2:06:10 |
| 8 | 7,800 | 730/900/505/225 | 450/565/395/115 | 2:31:20 |
| 9 | 9,600 | 935/1,155/650/290 | 575/720/505/145 | 3:00:30 |
| 10 | 11,800 | 1,200/1,475/830/370 | 740/920/645/185 | 3:34:20 |
| 11 | 14,400 | 1,535/1,890/1,065/470 | 945/1,180/825/235 | 4:13:40 |
| 12 | 17,600 | 1,965/2,420/1,360/605 | 1,210/1,510/1,060/300 | 4:59:10 |
| 13 | 21,400 | 2,515/3,095/1,740/775 | 1,545/1,935/1,355/385 | 5:52:10 |
| 14 | 25,900 | 3,220/3,960/2,230/990 | 1,980/2,475/1,735/495 | 6:53:30 |
| 15 | 31,300 | 4,120/5,070/2,850/1,270 | 2,535/3,170/2,220/635 | 8:04:40 |
| 16 | 37,900 | 5,275/6,490/3,650/1,625 | 3,245/4,055/2,840/810 | 9:27:10 |
| 17 | 45,700 | 6,750/8,310/4,675/2,075 | 4,155/5,190/3,635/1,040 | 11:02:50 |
| 18 | 55,100 | 8,640/10,635/5,980/2,660 | 5,315/6,645/4,650/1,330 | 12:54:00 |
| 19 | 66,400 | 11,060/13,610/7,655/3,405 | 6,805/8,505/5,955/1,700 | 15:02:50 |
| 20 | 80,000 | 14,155/17,420/9,800/4,355 | 8,710/10,890/7,620/2,180 | 17:32:10 |

  Note: Kingdoms uses a slightly different rounded table (for example 7,700 at L8 and 12,000 at L10), and Pillage First's storage array also differs. Do not mix them with Legends — [kirilloid t5/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/t5/buildings.ts).
- Official storage rules: lumber, clay and iron go in the Warehouse, crop in the Granary. "Production above the limit is lost, though the fields keep running". The Cranny hides a portion of stored resources from raiders, and resources in transit cannot be stolen. Raided resources go to the attacker's warehouse and granary, not to the hero's inventory — [Official: Guide: Resources Explained](https://support.travian.com/en/articles/214-guide-resources-explained).
- Crop upkeep rules:
  - The village consumes crop each hour equal to its population, and each stationed unit eats a set amount per hour. Net crop is production minus troop and population consumption — [Official: Guide: Resources Explained](https://support.travian.com/en/articles/214-guide-resources-explained).
  - If the granary empties while the balance is negative, troops "slowly starve and die". Starvation does not affect buildings, and it stops once crop production returns to 0 — [Official: Problems with Crop](https://support.travian.com/en/articles/75-problems-with-crop).
  - Main Building, Warehouse and Granary can be upgraded to L10 regardless of free crop if base crop production is at least 276/h — [Official: Problems with Crop](https://support.travian.com/en/articles/75-problems-with-crop).
- Per-unit crop upkeep (crop/h) and carry capacity (kirilloid model; base/T3 stats inherited by T4, plus T4.6 tribes) — [kirilloid base/units.ts](https://github.com/kirilloid/travian/blob/master/src/model/base/units.ts), [kirilloid t4.fs/units.ts](https://github.com/kirilloid/travian/blob/master/src/model/t4.fs/units.ts):
  - **Romans:** Legionnaire 1 (carries 50), Praetorian 1 (20), Imperian 1 (50), Equites Legati 2, Equites Imperatoris 3 (100), Equites Caesaris 4 (70), Ram 3, Fire Catapult 6, Senator 5, Settler 1.
  - **Teutons:** Clubswinger 1 (60), Spearman 1 (40), Axeman 1 (50), Scout 1, Paladin 2 (110), Teutonic Knight 3 (80), Ram 3, Catapult 6, Chief 4, Settler 1.
  - **Gauls:** Phalanx 1 (35), Swordsman 1 (45), Pathfinder 2, Theutates Thunder 2 (75), Druidrider 2 (35), Haeduan 3 (65), Ram 3, Trebuchet 6, Chieftain 4, Settler 1.
  - **Egyptians:** Slave Militia 1 (15), Ash Warden 1 (50), Khopesh Warrior 1 (45), Sopdu Explorer 2, Anhur Guard 2 (50), Resheph Chariot 3 (70), Ram 3, Catapult 6, Nomarch 4, Settler 1.
  - **Huns:** Mercenary 1 (50), Bowman 1 (30), Spotter 2, Steppe Rider 2 (115), Marksman 2 (105), Marauder 3 (80), Ram 3, Catapult 6, Logades 4, Settler 1.
  - Spartans and Vikings (special-world tribes) are not covered.

### Inferences
- **Storage is a hard cap on "waiting".** A village with L10 storage holds 11,800 per resource. At the all-L10 output of a 4-4-4-6 (1,120 wood/h) that fills in about 10.5 h. With L20 storage (80,000) it takes about 71 h. Above that, an absent "harvester" simply loses production, which matters for comparing passive production with raid loot.
- High field levels need several maxed warehouses at once: about 2 for an L15 woodcutter (131k clay), about 8 for L18 (611k clay) and about 22 for L20 (1.7M clay). That is effectively unreachable without Great Warehouses (×3), so L19–L20 fields are a late-game sink.
- **Population upkeep of the fields alone:** about 92 crop/h at all-L5 and about 242 crop/h at all-L10 for a 4-4-4-6 (128 for a 15-cropper). Buildings add more. A raiding army of 500 one-upkeep infantry eats 500 crop/h, about 30% of an all-L10 4-4-4-6's crop output.

### Gaps
- I did not find which building conditions (artefact, Wonder village) allow Great Warehouses and Great Granaries in current Legends. Also missing: whether storage capacity or troop upkeep scale with server speed (neither is in the official speed table; kirilloid's speed model changes only times and unit speed, which implies storage does not scale), and Legends-specific cranny capacities.

## 6. Starting conditions for a new account

### Takeaway
A new Legends account starts with a 4-4-4-6 village (54/h base if all fields start at level 0, which is my assumption — the official pages don't state starting field levels), a hero whose 4 default points already produce 36 of each resource per hour, 130 gold, 500 CP and 5 (+3 optional) days of beginner's protection at x1. Tasks, adventures and daily quests add resource grants. The exact starting stock and current task reward amounts are not published.

### Cited Findings
- New accounts get a hero, "some initial resources" (amount not stated) and "130 gold" — [Official: First steps in the game](https://support.travian.com/en/articles/142-first-steps-in-the-game). The hero starts with 4 points, all in resource production — [Official: Hero overview](https://support.travian.com/en/articles/45-hero-overview). At 9 per point that is 36 of each resource per hour at x1 — [Official: Hero in the early game](https://support.travian.com/en/articles/141-hero-in-the-early-game).
- Beginner's protection is 5 days + optional 3 at x1, 3 + 3 at x2/x3, 2 + 2 at x5 and 1 + 1 at x10 — [Official: Game Versions and Speed](https://support.travian.com/en/articles/20-game-versions-and-speed). Per the protection article: it cannot be ended early or regained; nobody can attack you; you cannot send troops or resources but can receive resources; you can still attack Natars and unoccupied oases — [Official: Beginner's Protection](https://support.travian.com/en/articles/12-beginner-s-protection) (via search snippet).
- Starting CP is 500 at x1, and the 2nd village needs 2,000 CP at x1 — [Official: Game Versions and Speed](https://support.travian.com/en/articles/20-game-versions-and-speed). Settling needs "750 of each resource" plus enough CP — [Official: Types of capitals and their development](https://support.travian.com/en/articles/173-types-of-capitals-and-their-development).
- Current task system:
  - Tasks give "resources (equal amounts of each type) and hero experience". Each task type has its own multiplier, each later task of the same type pays more, and hero level adds a bonus to all rewards. No amounts are published.
  - Field tasks trigger at levels 2, 4, 7 and 10 per field type. Group tasks trigger at 2, 3, 5, 8 and 10. "All fields to level" tasks run from 2 to 10. Mill tasks trigger at L1 and L5. Population and CP milestones are 500, 1,000, 1,500, 2,500, 5,000, 10,000, 20,000, then doubling.
  
  Source: [Official: Task System](https://support.travian.com/en/articles/17-task-system).
- Adventures: "Resources" rewards are available from the start, with another batch at 24–32 h. Also available: silver (0–8 h), ointment (8–16 h), Book of Wisdom (16–24 h) and a horse from the start. Times divide by game speed. No amounts are given — [Official: Hero in the early game](https://support.travian.com/en/articles/141-hero-in-the-early-game). Task rewards and small oasis-clearing bounties go to the hero inventory and can be moved to any village without merchants — [Official: Guide: Resources Explained](https://support.travian.com/en/articles/214-guide-resources-explained).
- **Older T4 quest line (likely outdated, pre-task-system):**
  - Rewards included: all fields to L1 → 400/460/330/270; one of each field to L2 → 240/255/190/160; Marketplace → 600 of each; Cranny → 130/150/120/100; an iron mine L1 → 1 day of Plus.
  - Summed over the economy and world lines, the resource rewards were about 41.8k; the military line about 8.8k; total about 50.6k resources.
  
  Source: [kirilloid t4/quests.ts](https://github.com/kirilloid/travian/blob/master/src/model/t4/quests.ts) (my sums of the coded reward arrays).

### Inferences
- In the first days the account's income is dominated by the hero (144/h total vs 54/h from fields) and by one-off quest and adventure grants. If the old quest scale still roughly holds, those grants (tens of thousands of resources) equal several days of early field production. Day-1 to day-3 "harvest" figures should therefore include them, or harvesting will be understated.
- Protection at x1 is 5–8 days, during which the account cannot raid players. This is the window in which pure building is the only option, apart from oases and Natars.

### Gaps
- Exact starting resource stock in Legends was not found (the classic figure of 750 each is unverified for current Legends).
- Current task reward amounts and adventure resource amounts are unpublished. The in-game values scale with hero level.
- A search snippet suggested "resource production only starts once Beginner Protection ends" ([Official: Early Game Oasis Farming](https://support.travian.com/en/articles/190-early-game-oasis-farming)). This almost certainly refers to oasis animal and resource regeneration, not village fields, but I could not verify it.

## 7. Scaling with server speed (x1–x10) and recent Legends changes (2023–2026)

### Takeaway
Speed multiplies production (fields, hero, oases) and divides build and training times, but does not change costs. Payback measured in hours therefore divides by the speed factor, while payback measured in build steps stays the same. Gold bonus and Plus last 7 days at x1 versus 3 days on all faster worlds. Recent changes visible in official pages include tripled hero production rates, special-world "cities" (L12 fields), 18-croppers, governors (2026), Spartans and Vikings worlds, video bonuses and item-rarity worlds.

### Cited Findings
- **Official speed table:**

| Factor | x1 | x2 | x3 | x5 | x10 |
|---|---|---|---|---|---|
| Resource production | normal | ×2 | ×3 | ×5 | ×10 |
| Construction time | normal | /2 | /3 | /5 | /10 |
| Training time | normal | /2 | /3 | /5 | /10 |
| Troop speed | normal | ×2 | ×2 | ×2 | ×4 |
| Beginner's protection | 5 + 3 days | 3 + 3 | 3 + 3 | 2 + 2 | 1 + 1 |
| Plus / resource-bonus duration | 7 days | 3 days | 3 days | 3 days | 3 days |

  It also lists starting CP of 500/250/167/100/50, a 2nd-village CP requirement of 2,000/800/500/300/200, Natar Wonder completion after 250/160/95/71/35.5 days, artefacts on day 90/45/30/18/9 and registration closing after 70/35/28/15/7 days — [Official: Game Versions and Speed](https://support.travian.com/en/articles/20-game-versions-and-speed).
- Hero production "increases with game speed (e.g. x2, x3 worlds)", and the 25% bonus applies to the scaled value — [Official: Hero in the early game](https://support.travian.com/en/articles/141-hero-in-the-early-game). Oasis production is "Doubled on x2, tripled on x3, etc." — [Official: Early Game Oasis Farming](https://support.travian.com/en/articles/190-early-game-oasis-farming) (via search snippet).
- kirilloid's 3× Legends model divides only building times, unit training and research times by 3 and multiplies unit speed by 2. Costs, capacities and upkeep are untouched — [kirilloid speeds.ts](https://github.com/kirilloid/travian/blob/master/src/model/speeds.ts).
- **Recent or current-version features seen in 2025–2026 official pages:**
  - Six-tribe worlds include Romans, Teutons or Vikings, Gauls, Huns, Egyptians and Spartans (special worlds). Five-tribe worlds are common internationally. Item-rarity worlds exist — [Official: Game Versions and Speed](https://support.travian.com/en/articles/20-game-versions-and-speed).
  - "Cities" allow non-capital fields up to L12, and 18-croppers exist on some special worlds — [Official: Guide: Resources Explained](https://support.travian.com/en/articles/214-guide-resources-explained); [Official: Quick Summary: Buildings and Construction](https://support.travian.com/en/articles/221-quick-summary-buildings-and-construction).
  - Rise of Governors (Annual Special 2026; Cleopatra +25% / +25% crop) — [Official: Rise of Governors FAQ](https://support.travian.com/en/articles/230-rise-of-governors-faq).
  - Video production bonus — [Official: Guide: Resources Explained](https://support.travian.com/en/articles/214-guide-resources-explained).
  - Oasis raiding takes up to 10% of an oasis's stock, regenerating within 10 minutes — [Official: Oasis](https://support.travian.com/en/articles/48-oasis).
  - Hero resource production is now 9 each / 30 single per point (12/40 for Egyptians) — [Official: Hero in the early game](https://support.travian.com/en/articles/141-hero-in-the-early-game). Older T4 figures were 3/10 per point ([CodeGuru forum](https://forums.codeguru.com/showthread.php?p=2076569)) or 6 per point in kirilloid's T4 model ([kirilloid t4/hero.ts](https://github.com/kirilloid/travian/blob/master/src/model/t4/hero.ts)).

### Inferences
- **Speed rule for this fact sheet.** Production(xN) = N × x1 production for fields, hero and oases. Cost(xN) = x1 cost. Build time(xN) = x1 time ÷ N. So payback hours(xN) = x1 payback ÷ N. Example: an L10 field takes 328 h at x1, 109 h at x3 and 33 h at x10.
  - Storage is (apparently) not scaled, so the time to fill a warehouse shrinks by N. An L20 warehouse fills in about 7 h at x10 for a village producing 1,120 wood/h at x1 rates (11,200/h).
  - Gold bonuses run 3 days instead of 7 on faster worlds, so resources per gold scale by N × 3/7. That is about 1.29× at x3, 2.1× at x5 and 4.3× at x10 compared with x1.
- Because troop speed rises only ×2 at x2–x5 (×4 at x10) while production rises ×N, raid travel time is relatively more expensive on faster worlds than production time. This is relevant when comparing raiding with harvesting across speeds.
- Hero production being 3× the older T4 rate makes the hero a much larger share of early income in current Legends than in pre-2019 guides. Older guides that dismiss hero production are outdated.

### Gaps
- No dated changelog was found for when hero production moved from 3/10 to 9/30 per point, when the video bonus and cities arrived, or other 2023–2026 balance changes. Travian blog and forum changelogs were not retrieved.
- Whether crop upkeep and storage capacity scale with speed is not stated officially.
- x2 servers are listed officially, but kirilloid models only x1 and x3 for Legends.
