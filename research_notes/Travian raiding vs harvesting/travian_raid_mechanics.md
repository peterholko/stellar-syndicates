# Travian: Legends raiding mechanics and troop economics (raid income, unit payback, raid risk)

Source key used throughout:
- **Official** = Travian: Legends Help Center (support.travian.com), read Oct 2026.
- **kirilloid** = travian.kirilloid.ru, the long-running community calculator. Its server picker files "T4.6 1x/2x/3x/5x/10x" under the optgroup **"Legends"**, and its default model version is 4.6 ([troops.php](http://travian.kirilloid.ru/troops.php), [server.js](http://travian.kirilloid.ru/js/server.js)). Its formulas are reverse-engineered by the community, not official. Where the Help Center also gives a number, the two agree (checks below).
- Numbers are for x1 servers unless stated. The speed-server rule is given separately in Q1/Q2.

## Q1. Tribes in Legends (2025–2026) and the raiding units' stats (cost, upkeep, carry, speed, training time, building-level scaling)

### Takeaway
Legends worlds run with 3 tribes (Romans, Teutons, Gauls), 5 tribes (adds Huns and Egyptians; common on international worlds) or 6 tribes (adds Spartans, with Vikings as an alternative to Teutons; special gameworlds). The unit table below comes from kirilloid's T4.6 ("Legends") data. The official Vikings table matches it exactly on every value it gives (cost, upkeep, speed, training time), which supports the dataset. Training time at building level L is the level-1 time × 0.9^(L−1). Great Barracks and Great Stable train the same units in parallel at 3× the resource cost and cannot be built in the capital.

### Cited Findings
**Tribes and availability**
- Help Center tribe setups: "6 tribes: Romans, Teutons or Vikings, Gauls, Huns, Egyptians, and Spartans (special gameworlds only)". "5 tribes: Romans, Teutons, Gauls, Huns, Egyptians (common on international worlds)". "3 tribes: Romans, Teutons, Gauls (traditional setup)". Vikings appear only as an alternative to Teutons in the 6-tribe setup. — [Official: Game Versions and Speed](https://support.travian.com/en/articles/20-game-versions-and-speed)
- Wikipedia (from a search-result summary): the Vikings came in as a temporarily playable tribe in the spring-2025 special round "Travian: Northern Legends". The Spartans came from the special server "Travian: Glory of Sparta" and "has been incorporated into regular servers as well". The Travian App Store listing names Romans, Egyptians, Huns, Spartans, Teutons and Gauls. — [Wikipedia: Travian](https://en.wikipedia.org/wiki/Travian); [App Store listing](https://apps.apple.com/us/app/-/id1544118325). These sources **conflict** with the Help Center wording "(special gameworlds only)" on whether Spartans run on regular worlds.
- The Help Center has a dedicated "Vikings in Travian: Legends" article with a full unit table (see below), so Vikings are a live Legends tribe on the worlds that offer them. — [Official: Vikings in Travian: Legends](https://support.travian.com/en/articles/139-vikings-in-travian-legends)

**Raiding-relevant units (x1, base training time at Barracks/Stable level 1, no Smithy upgrades)**
Source for every row: [kirilloid units.js](http://travian.kirilloid.ru/js/units.js). For the three classic tribes, kirilloid's "t4" training-time overrides apply. For Egyptians, Huns, Spartans and Vikings the base array applies, since these tribes exist only from T4 on. The Viking rows are cross-checked against the [official Vikings table](https://support.travian.com/en/articles/139-vikings-in-travian-legends), which matches on attack, defence, speed, upkeep, cost and training time but gives no carry capacity. Upkeep is crop per hour.

| Tribe | Unit | Type | Off | Def inf/cav | Speed (fields/h) | Carry | Cost wood/clay/iron/crop (sum) | Upkeep | Train lvl 1 |
|---|---|---|---|---|---|---|---|---|---|
| Romans | Legionnaire | inf | 40 | 35/50 | 6 | 50 | 120/100/150/30 (400) | 1 | 0:26:40 |
| Romans | Imperian | inf | 70 | 40/25 | 7 | 50 | 150/160/210/80 (600) | 1 | 0:32:00 |
| Romans | Equites Imperatoris | cav | 120 | 65/50 | 14 | 100 | 550/440/320/100 (1410) | 3 | 0:44:00 |
| Romans | Equites Caesaris | cav | 180 | 80/105 | 10 | 70 | 550/640/800/180 (2170) | 4 | 0:58:40 |
| Teutons | Clubswinger (kirilloid: "Maceman") | inf | 40 | 20/5 | 7 | 60 | 95/75/40/40 (250) | 1 | 0:12:00 |
| Teutons | Axeman | inf | 60 | 30/30 | 6 | 50 | 130/120/170/70 (490) | 1 | 0:20:00 |
| Teutons | Paladin | cav | 55 | 100/40 | 10 | 110 | 370/270/290/75 (1005) | 2 | 0:40:00 |
| Teutons | Teutonic Knight | cav | 150 | 50/75 | 9 | 80 | 450/515/480/80 (1525) | 3 | 0:49:20 |
| Gauls | Phalanx (defensive, context) | inf | 15 | 40/50 | 7 | 35 | 100/130/55/30 (315) | 1 | 0:17:20 |
| Gauls | Swordsman | inf | 65 | 35/20 | 6 | 45 | 140/150/185/60 (535) | 1 | 0:24:00 |
| Gauls | Theutates Thunder | cav | 100 | 25/40 | 19 | 75 | 350/450/230/60 (1090) | 2 | 0:41:20 |
| Gauls | Haeduan | cav | 140 | 60/165 | 13 | 65 | 500/620/675/170 (1965) | 3 | 0:52:00 |
| Egyptians | Slave Militia (defensive, context) | inf | 10 | 30/20 | 7 | 15 | 45/60/30/15 (150) | 1 | 0:08:50 |
| Egyptians | Khopesh Warrior | inf | 65 | 50/20 | 7 | 45 | 170/180/220/80 (650) | 1 | 0:24:00 |
| Egyptians | Anhur Guard | cav | 50 | 110/50 | 15 | 50 | 360/330/280/120 (1090) | 2 | 0:42:40 |
| Egyptians | Resheph Chariot | cav | 110 | 120/150 | 10 | 70 | 450/560/610/180 (1800) | 3 | 0:54:00 |
| Huns | Mercenary | inf | 35 | 40/30 | 6 | 50 | 130/80/40/40 (290) | 1 | 0:13:30 |
| Huns | Steppe Rider | cav | 120 | 30/15 | 16 | 75 | 290/370/190/45 (895) | 2 | 0:40:00 |
| Huns | Marksman | cav | 110 | 80/70 | 15 | 105 | 320/350/330/50 (1050) | 2 | 0:41:20 |
| Huns | Marauder | cav | 180 | 60/40 | 14 | 80 | 450/560/610/140 (1760) | 3 | 0:49:50 |
| Spartans | Hoplite | inf | 50 | 35/30 | 6 | 60 | 110/185/110/35 (440) | 1 | 0:28:20 |
| Spartans | Twinsteel Therion | inf | 90 | 55/40 | 6 | 50 | 130/200/400/65 (795) | 1 | 0:35:12 |
| Spartans | Elpida Rider | cav | 55 | 120/90 | 16 | 110 | 555/445/330/110 (1440) | 2 | 0:46:56 |
| Spartans | Corinthian Crusher | cav | 195 | 80/75 | 9 | 80 | 660/495/995/165 (2315) | 3 | 0:57:12 |
| Vikings | Thrall | inf | 45 | 22/5 | 7 | 55 | 95/80/50/40 (265) | 1 | 0:13:20 |
| Vikings | Berserker | inf | 70 | 30/25 | 5 | 75 | 235/220/200/70 (725) | 2 | 0:25:50 |
| Vikings | Huskarl Rider | cav | 45 | 95/100 | 12 | 110 | 385/295/290/85 (1055) | 2 | 0:44:10 |
| Vikings | Valkyrie's Blessing | cav | 160 | 50/75 | 9 | 80 | 475/535/515/100 (1625) | 2 | 0:51:00 |

- Official unit names in kirilloid's English list: Romans (Legionnaire, Praetorian, Imperian, Equites Legati, Equites Imperatoris, Equites Caesaris…), Teutons ("Maceman" = Clubswinger, Spearman, Axeman, Scout, Paladin, Teutonic Knight…), Gauls (Phalanx, Swordsman, Pathfinder, Theutates Thunder, Druidrider, Haeduan…), Egyptians (Slave Militia, Ash Warden, Khopesh Warrior, Sopdu Explorer, Anhur Guard, Resheph Chariot…), Huns (Mercenary, Bowman, Spotter, Steppe Rider, Marksman, Marauder…), Spartans (Hoplite, Sentinel, Shieldsman, Twinsteel Therion, Elpida Rider, Corinthian Crusher…), Vikings (Thrall, Shield Maiden, Berserker, Heimdall's Eye, Huskarl Rider, Valkyrie's Blessing…). — [kirilloid troops.php](http://travian.kirilloid.ru/troops.php)
- Other kirilloid units for context: Praetorian 30 off, carry 20; Spearman carry 40; Druidrider 45 off, carry 35; Ash Warden 30 off, carry 50; Bowman 50 off, carry 30; Shieldsman 40 off, carry 40; Shield Maiden 20 off, carry 40. Scouts have carry 0. — [kirilloid units.js](http://travian.kirilloid.ru/js/units.js)
- Help Center tribe notes relevant to raiding: Teuton troops are "cheap, fast to train" with "the best carrying capacity in the game" ([Official: Teutons](https://support.travian.com/en/articles/8-the-teutons-and-their-advantages)). Berserkers eat double crop, and each Berserker killed while attacking or raiding kills one enemy unit in return, calculated after the main battle ([Official: Tribes](https://support.travian.com/en/articles/3-the-tribes-and-their-advantages); [Official: Vikings](https://support.travian.com/en/articles/139-vikings-in-travian-legends)). Spartan troops are "slow and costly to train, but crop-efficient", and Roman units "take longer to train and cost more" ([Official: Tribes](https://support.travian.com/en/articles/3-the-tribes-and-their-advantages)).
- Hero speed bonuses: a mounted Gaul hero gives "+5 speed" ([Official: Tribes](https://support.travian.com/en/articles/3-the-tribes-and-their-advantages)). A mounted Hun hero adds "+3 speed to mounted armies", which scales with gameworld speed ([Official: Huns](https://support.travian.com/en/articles/7-the-huns-and-their-advantages)).
- Viking ships on harbour (naval) worlds travel 24 fields/h through deep water at x1, against 18 for other tribes. This bonus applies to raids only. — [Official: Vikings](https://support.travian.com/en/articles/139-vikings-in-travian-legends)

**Training-time formula and modifiers**
- kirilloid's training-time code: time = base_time ÷ server_speed × (1 − alliance recruit bonus) × (1 − helmet bonus, infantry or cavalry) × artefact factor × (1 − 1% × Horse Drinking Trough level, Roman cavalry only), then × **0.9^(building level − 1)**, rounded. The same factor applies to barracks, stables, workshop and residence/palace. — [kirilloid troops.js](http://travian.kirilloid.ru/js/troops.js)
- The base buildings model confirms `train = 0.9 ** (lvl - 1)`. — [kirilloid GitHub base/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/base/buildings.ts)
- Horse Drinking Trough (Roman): −1% cavalry training time per level. It cuts crop upkeep by 1 for Equites Legati at level 10, Equites Imperatoris at 15 and Equites Caesaris at 20. — [kirilloid troops.js](http://travian.kirilloid.ru/js/troops.js) (the `hdp >= (idx-1)*5` rule)
- Great Barracks / Great Stable: "Allows to build infantry simultaneously with normal barracks, but for 3x more resources". Great Stable is "the same as great barracks, but for cavalry". Both require Barracks or Stable level 20, are flagged "not capital", and have 20 levels. — [kirilloid build.php](http://travian.kirilloid.ru/build.php)
- The Help Center confirms Great Barracks and Great Stables can't be built in the capital and are removed if a village becomes the capital (per search-result summary of [Official: Capital Village](https://support.travian.com/en/articles/50-capital-village)). A defensive-play guide argues that for the same cost you could run three normal Barracks across three villages (per search-result summary of [Official: Defensive Play](https://support.travian.com/en/articles/172-defensive-play-build-right-arrive-fast-feed-always)).
- Queued training keeps the time it had when queued, whatever later changes to building level, items or artefacts. — [kirilloid build.php (Barracks note)](http://travian.kirilloid.ru/build.php)

**Server-speed scaling (official)**
- Troop speed: x1 normal, x2 ×2, x3 ×2, x5 ×2, x10 ×4. Training time: /2, /3, /5, /10. Construction time /speed, resource production ×speed. — [Official: Game Versions and Speed](https://support.travian.com/en/articles/20-game-versions-and-speed)
- kirilloid's model uses the same troop-speed map `{1:1, 2:2, 3:2, 5:2, 10:4}`. — [kirilloid server.js](http://travian.kirilloid.ru/js/server.js)

**Version flags (older numbers, do not use for Legends)**
- kirilloid's base (T3) training times differ: Legionnaire 2000 s, Clubswinger 900 s, Theutates Thunder 3100 s, Paladin 3000 s, Teutonic Knight 3700 s. The T4 overrides are 1600, 720, 2480, 2400 and 2960 s. — [kirilloid units.js](http://travian.kirilloid.ru/js/units.js)
- kirilloid's "T5" set is Travian **Kingdoms**: units cost no crop (e.g. Clubswinger 85/65/30/0). — [kirilloid units.js](http://travian.kirilloid.ru/js/units.js)
- Travian Kingdoms announced a 2026 "Summer Mayhem" rebalance listing Clubswinger at carry 90, speed 8, 13:30. That is **Kingdoms, not Legends**. — [Kingdoms support (search-result summary)](https://support.kingdoms.com/en/articles/202-summer-mayhem-2026-troop-rebalancing)

### Inferences
- Training times at higher building levels (my calculation, t × 0.9^(L−1), x1, no other modifiers; level 10 = ×0.387, level 20 = ×0.135):

| Unit | Lvl 1 | Lvl 10 | Lvl 20 |
|---|---|---|---|
| Clubswinger | 0:12:00 | 0:04:39 | 0:01:37 |
| Thrall | 0:13:20 | 0:05:10 | 0:01:48 |
| Mercenary | 0:13:30 | 0:05:14 | 0:01:49 |
| Legionnaire | 0:26:40 | 0:10:20 | 0:03:36 |
| Hoplite | 0:28:20 | 0:10:59 | 0:03:50 |
| Khopesh Warrior / Swordsman | 0:24:00 | 0:09:18 | 0:03:15 |
| Steppe Rider / Paladin | 0:40:00 | 0:15:30 | 0:05:24 |
| Theutates Thunder / Marksman | 0:41:20 | 0:16:01 | 0:05:35 |
| Equites Imperatoris | 0:44:00 | 0:17:03 | 0:05:57 |
| Huskarl Rider | 0:44:10 | 0:17:07 | 0:05:58 |
| Elpida Rider | 0:46:56 | 0:18:11 | 0:06:20 |
| Teutonic Knight | 0:49:20 | 0:19:07 | 0:06:40 |
| Marauder | 0:49:50 | 0:19:18 | 0:06:44 |
| Valkyrie's Blessing | 0:51:00 | 0:19:46 | 0:06:53 |

- A Great Barracks/Stable runs alongside the normal building with the same per-level time factor. It doubles training throughput for 3× the resource cost per unit trained there.
- Carry per 100 resources of unit cost is the cleanest "raid capital efficiency" metric. Clubswinger 24.0 and Thrall 20.8 lead, then Mercenary 17.2, Hoplite 13.6 and Legionnaire 12.5. Among cavalry: Paladin 10.9, Huskarl Rider 10.4, Marksman 10.0, Steppe Rider 8.4, Elpida Rider 7.6, Equites Imperatoris 7.1, Theutates Thunder 6.9. Heavy hitters sit at 3–5 (Equites Caesaris 3.2, Haeduan 3.3, Corinthian Crusher 3.5). Calculated from the table above.

### Gaps
- No official Legends table with **carry capacity** for any tribe; the Help Center tribe articles fetched give none. Carry values rely on kirilloid alone.
- No official cross-check for the Romans, Teutons, Gauls, Egyptians, Huns and Spartans rows. Only the Viking rows were verified against the Help Center. Attack values in particular (e.g. Theutates Thunder 100) were not cross-checked; they matter for risk, not for income.
- The exact multiplier for alliance recruitment bonuses and helmets in Legends was not researched.
- Spartan availability on regular (non-special) worlds in 2026 is unresolved (Wikipedia vs Help Center wording).

## Q2. Worked numbers: resources per hour per unit at 5/10/20 fields, the Tournament Square beyond 20, payback hours

### Takeaway
At full load, a unit earns carry × speed ÷ (2 × distance) resources per hour, so income halves each time distance doubles. At 10 fields (x1), the best raiders per unit earn about 70–90 res/h: Elpida Rider 88, Marksman 79, TT 71, Equites Imperatoris 70, Huskarl 66. Cheap infantry earns 15–21 res/h (Clubswinger 21). On capital efficiency, Clubswinger, Marksman, Thrall and Steppe Rider repay their cost fastest: about 12–15 h of full-load raiding at 10 fields and about 24–30 h at 20 fields. Beyond 20 fields the Legends Tournament Square adds +20% speed per level on the part past 20 fields (5× at level 20). Even so, a 50-field raid at TS 20 earns only about 77% of a 20-field raid.

### Cited Findings
- Distance-based travel: time = 3600 × effective_distance ÷ unit_speed seconds. Effective distance = d if d ≤ arena distance (20 fields in T4+). Beyond that it is (d − 20)/(1 + 0.1 × level) + 20, where Legends ("double arena" versions, including 4.6) doubles the level. Boots add their percentage additively. Left-hand item speed bonuses multiply unit speed. — [kirilloid distance.js](http://travian.kirilloid.ru/js/distance.js); [kirilloid server.js](http://travian.kirilloid.ru/js/server.js) (`arenaDistance` 20 for T4+, 30 for older; `isDoubleArena` true for 4.6)
- kirilloid's building table gives the Tournament Square in T4.3/4.5/4.6 as "100 + 20×level %" speed, described as "Increases army speed, but only for part of distance exceeding 20 squares. Doesn't affect merchants." — [kirilloid build.php](http://travian.kirilloid.ru/build.php)
- Help Center (from a search-result summary): each Tournament Square level adds +20% speed beyond 20 fields. Level 20 means troops move 5× base speed after the first 20 fields (the summary words this as "+500%"). It requires Rally Point level 15. Boots and Tournament Square bonuses add together and apply only beyond 20 fields; artefacts and standards multiply. Trips under 20 fields are unaffected. — [Official: Troops Speed Increase & Tournament Square](https://support.travian.com/en/articles/71-troops-speed-increase-and-tournament-square)
- Hero item speeds: Boots of the Mercenary/Warrior/Archon give +25/50/75% army speed for distances over 20 fields. Small Map/Map/Large Map make the return 30/40/50% faster. — [kirilloid items.php](http://travian.kirilloid.ru/items.php)
- Troop speed multiplier by server speed: ×1 (x1), ×2 (x2, x3, x5), ×4 (x10). — [Official: Game Versions and Speed](https://support.travian.com/en/articles/20-game-versions-and-speed)
- Unit carry, speed, cost and upkeep inputs: Q1 table — [kirilloid units.js](http://travian.kirilloid.ru/js/units.js)

### Inferences
All tables below are **my calculations** from the cited inputs. They assume every trip returns full, no losses, no idle time between trips, no cranny, no hero or items, x1 speed. Round-trip hours = 2d ÷ speed (d ≤ 20). Gross res/h = carry ÷ round trip = carry × speed ÷ 2d. Net = gross − crop upkeep. Payback hours = unit cost ÷ res/h.

Round trips (h) at 5/10/20 fields: speed 5 → 2.0/4.0/8.0; 6 → 1.67/3.33/6.67; 7 → 1.43/2.86/5.71; 9 → 1.11/2.22/4.44; 10 → 1.0/2.0/4.0; 12 → 0.83/1.67/3.33; 14 → 0.71/1.43/2.86; 15 → 0.67/1.33/2.67; 16 → 0.625/1.25/2.5; 19 → 0.53/1.05/2.11.

| Unit | Gross res/h @5 | @10 | @20 | Net @20 | Payback h (gross) @5 | @10 | @20 | Payback h (net) @20 |
|---|---|---|---|---|---|---|---|---|
| Clubswinger | 42 | 21 | 10.5 | 9.5 | 6.0 | 11.9 | 23.8 | 26.3 |
| Thrall | 38.5 | 19 | 9.6 | 8.6 | 6.9 | 13.8 | 27.5 | 30.7 |
| Marksman | 158 | 79 | 39 | 37 | 6.7 | 13.3 | 26.7 | 28.1 |
| Steppe Rider | 120 | 60 | 30 | 28 | 7.5 | 14.9 | 29.8 | 32.0 |
| Theutates Thunder | 142.5 | 71 | 36 | 34 | 7.6 | 15.3 | 30.6 | 32.4 |
| Huskarl Rider | 132 | 66 | 33 | 31 | 8.0 | 16.0 | 32.0 | 34.0 |
| Elpida Rider | 176 | 88 | 44 | 42 | 8.2 | 16.4 | 32.7 | 34.3 |
| Paladin | 110 | 55 | 27.5 | 25.5 | 9.1 | 18.3 | 36.5 | 39.4 |
| Mercenary | 30 | 15 | 7.5 | 6.5 | 9.7 | 19.3 | 38.7 | 44.6 |
| Equites Imperatoris | 140 | 70 | 35 | 32 | 10.1 | 20.1 | 40.3 | 44.1 |
| Hoplite | 36 | 18 | 9 | 8 | 12.2 | 24.4 | 48.9 | 55.0 |
| Phalanx | 24.5 | 12 | 6 | 5 | 12.9 | 25.7 | 51.4 | 61.5 |
| Legionnaire | 30 | 15 | 7.5 | 6.5 | 13.3 | 26.7 | 53.3 | 61.5 |
| Slave Militia | 10.5 | 5 | 2.6 | 1.6 | 14.3 | 28.6 | 57.1 | 92.3 |
| Anhur Guard | 75 | 37.5 | 19 | 17 | 14.5 | 29.1 | 58.1 | 65.1 |
| Marauder | 112 | 56 | 28 | 25 | 15.7 | 31.4 | 62.9 | 70.4 |
| Axeman | 30 | 15 | 7.5 | 6.5 | 16.3 | 32.7 | 65.3 | 75.4 |
| Imperian | 35 | 17.5 | 9 | 8 | 17.1 | 34.3 | 68.6 | 77.4 |
| Berserker | 37.5 | 19 | 9.4 | 7.4 | 19.3 | 38.7 | 77.3 | 98.3 |
| Swordsman | 27 | 13.5 | 7 | 6 | 19.8 | 39.6 | 79.3 | 93.0 |
| Khopesh Warrior | 31.5 | 16 | 8 | 7 | 20.6 | 41.3 | 82.5 | 94.5 |
| Teutonic Knight | 72 | 36 | 18 | 15 | 21.2 | 42.4 | 84.7 | 101.7 |
| Valkyrie's Blessing | 72 | 36 | 18 | 16 | 22.6 | 45.1 | 90.3 | 101.6 |
| Haeduan | 84.5 | 42 | 21 | 18 | 23.3 | 46.5 | 93.0 | 108.4 |
| Resheph Chariot | 70 | 35 | 17.5 | 14.5 | 25.7 | 51.4 | 102.9 | 124.1 |
| Twinsteel Therion | 30 | 15 | 7.5 | 6.5 | 26.5 | 53.0 | 106.0 | 122.3 |
| Equites Caesaris | 70 | 35 | 17.5 | 13.5 | 31.0 | 62.0 | 124.0 | 160.7 |
| Corinthian Crusher | 72 | 36 | 18 | 15 | 32.2 | 64.3 | 128.6 | 154.3 |

- Beyond 20 fields (Legends Tournament Square, effective distance = 20 + (d−20)/(1+0.2·TS)), gross res/h per unit:

| Unit | 30 f TS0 | 30 f TS10 | 30 f TS20 | 50 f TS0 | 50 f TS10 | 50 f TS20 |
|---|---|---|---|---|---|---|
| Clubswinger | 7.0 | 9.0 | 9.5 | 4.2 | 7.0 | 8.1 |
| Thrall | 6.4 | 8.2 | 8.8 | 3.9 | 6.4 | 7.4 |
| Legionnaire | 5.0 | 6.4 | 6.8 | 3.0 | 5.0 | 5.8 |
| Paladin | 18.3 | 23.6 | 25.0 | 11.0 | 18.3 | 21.2 |
| Equites Imperatoris | 23.3 | 30.0 | 31.8 | 14.0 | 23.3 | 26.9 |
| Theutates Thunder | 23.8 | 30.5 | 32.4 | 14.2 | 23.8 | 27.4 |
| Steppe Rider | 20.0 | 25.7 | 27.3 | 12.0 | 20.0 | 23.1 |
| Marksman | 26.2 | 33.8 | 35.8 | 15.8 | 26.2 | 30.3 |
| Elpida Rider | 29.3 | 37.7 | 40.0 | 17.6 | 29.3 | 33.8 |
| Huskarl Rider | 22.0 | 28.3 | 30.0 | 13.2 | 22.0 | 25.4 |
| Teutonic Knight | 12.0 | 15.4 | 16.4 | 7.2 | 12.0 | 13.8 |

  A level-20 TS makes 30 fields cost about the same as 22 fields, and 50 fields about the same as 26 fields. TS 10 makes 50 fields cost about the same as 30 fields.
- Adding Boots of the Archon (+75%) to a TS 20 gives 1 + 4.0 + 0.75 = ×5.75 beyond 20 fields. The bonus is additive per the Help Center and kirilloid. Boots and maps are hero equipment, so they only help the one party the hero rides with.
- A Large Map (return 50% faster) cuts a round trip from 2d/v to d/v × (1 + 1/1.5) = 1.667 d/v, about +20% income for the party the hero accompanies.
- Speed servers: raid income per unit per hour scales with the troop-speed factor (×2 on x2/x3/x5, ×4 on x10), while field production scales ×speed. Relative to fields, raiding is therefore ×1 as attractive on x2, ×2/3 on x3, ×2/5 on x5 and ×4/10 on x10. Training is speed-times faster, so payback **in hours** halves (×2 speed) while fields produce 3×/5×/10×.
- Income is capped by target supply, not just carry. An unoccupied Type-1 oasis produces about 71 res/h total at x1 (40+10+10+11; see Q5), so sustained farming of one such oasis absorbs only about 3.4 Clubswingers' worth of 10-field carry (21 res/h each). Beyond that, each extra unit on the same target earns only the stockpile already accumulated there.
- Payback figures exclude losses, wasted trips (empty or crannied targets), and the training time before a unit starts earning. Real payback is longer.

### Gaps
- No reliable source on real-world "fill rates" (what fraction of trips return full) or on typical income per raider from experienced players; any such number would be a guess.
- Whether a hero's boots and maps apply to the whole party (assumed: the movement the hero is part of) was not confirmed on an official page.
- Travel-time rounding on Legends (kirilloid rounds to the second; the in-game rounding is unconfirmed).

## Q3. Raid vs normal attack: how casualties are calculated, how loot is computed, what happens when the target is empty

### Takeaway
Both modes compute the same strength ratio, x = (attacker's final offence ÷ defender's final defence)^K, with K ≈ 1.5 in battles up to about 1,000 units. A **raid** splits losses: attacker loses 1/(1+x), defender loses x/(1+x). A **normal attack** fights to the end: the winner loses (1/x) of its troops (or x if the defender wins) and the loser is wiped out. Raids don't fire catapults, damage walls with rams, or reduce loyalty. Loot is capped by surviving carry capacity and by what lies above the cranny. Against an empty, wall-less village (base defence 10), raid losses are effectively zero, except that a single unit with under 83 attack always dies. Walls and the Residence or Palace add passive defence that can kill a meaningful share of small raid parties even with no troops home.

### Cited Findings
**Official rules**
- Raid: "Catapults do not fire", rams don't damage walls, loyalty isn't reduced. Attack: a full battle that ends only when one side loses all troops; catapults, rams and administrators work. If only scouts are sent they scout, whether raid or attack was chosen. — [Official: Battle Mechanics](https://support.travian.com/en/articles/63-battle-mechanics)
- In a raid the attacker usually keeps "at least 1 surviving soldier". — [Official: Interacting with Other Players](https://support.travian.com/en/articles/11-interacting-with-other-players)
- "A lone attacking unit with fewer than 83 offence points always dies, regardless of the defender's losses." — [Official: Battle Mechanics](https://support.travian.com/en/articles/63-battle-mechanics). kirilloid's code uses `off × morale < 84.5` for the same rule ([kirilloid GitHub base/combat/combat.ts](https://github.com/kirilloid/travian/blob/master/src/model/base/combat/combat.ts)).
- An empty village has base defence 10. A Residence, Palace or Command Center at level 20 adds 800 passive defence (infantry and cavalry). Walls give a percentage defence bonus by tribe and level. — [Official: Battle Mechanics](https://support.travian.com/en/articles/63-battle-mechanics)
- Resources can be stolen from the Warehouse, Granary and unsold Marketplace offers. Crannies protect resources. Resources in the hero's inventory cannot be stolen. — [Official: Battle Mechanics](https://support.travian.com/en/articles/63-battle-mechanics)
- Raided resources go to the attacker's Warehouse and Granary, not the hero inventory. Resources in transit (incoming trades, alliance support) cannot be stolen; storage and open trade offers can. — [Official: Guide: Resources Explained](https://support.travian.com/en/articles/214-guide-resources-explained)
- Teuton heroes give +20% plunder by default, and hero items can raise it further (see Q4 and Q7). — [Official: Battle Mechanics](https://support.travian.com/en/articles/63-battle-mechanics)
- The **morale bonus** (the population-difference bonus for smaller defenders) "was removed from the game in 2022". — [Official: Battle Mechanics](https://support.travian.com/en/articles/63-battle-mechanics)
- Brewery (Teutons): +1% attacker strength per level; administrators lose 50% persuasion; catapults hit random targets. — [Official: Battle Mechanics](https://support.travian.com/en/articles/63-battle-mechanics)
- The official page states outgoing troops are capped at 20,000 per avatar, counting attacks, raids, adventures and reinforcements. This is the WebFetch summary's wording; whether it counts units or movements is unclear. — [Official: Battle Mechanics](https://support.travian.com/en/articles/63-battle-mechanics)

**Community-modelled formulas (kirilloid's open-source combat model; T3+ rules are inherited by T4)**
- Final defence = (Σ defenders' defence, weighted by the attacker's infantry/cavalry offence shares + 10 base + Residence/Palace def 2·level² + wall flat def) × (1 + wall bonus). Attacker offence = Σ unit attack (+ hero), × morale where applicable. — [kirilloid GitHub base/combat/combat.ts](https://github.com/kirilloid/travian/blob/master/src/model/base/combat/combat.ts); [base/combat/fns.ts](https://github.com/kirilloid/travian/blob/master/src/model/base/combat/fns.ts); [base/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/base/buildings.ts) (`residence`/`palace` def = 2·lvl²)
- x = (final off ÷ final def)^K. Raid: offLosses = 1/(1+x), defLosses = x/(1+x). Normal: offLosses = min(1/x, 1), defLosses = min(x, 1). — [kirilloid base/combat/combat.ts](https://github.com/kirilloid/travian/blob/master/src/model/base/combat/combat.ts) and [fns.ts](https://github.com/kirilloid/travian/blob/master/src/model/base/combat/fns.ts) (`raid(x) = [1/(1+x), x/(1+x)]`)
- K ("immensity") = 3.7184 − 2·N^0.015, clamped to [1.2578, 1.5], where N = total units in the battle. That gives K = 1.5 up to about 1,000 units, 1.42 at 10,000 and 1.34 at 100,000. — [kirilloid GitHub t3/combat/fns.ts](https://github.com/kirilloid/travian/blob/master/src/model/t3/combat/fns.ts) (formula); K values my calculation
- Historical morale (pre-2022): if attacker pop > defender pop, offence × max(0.667, (attPop/defPop)^(−0.2·min(off/def, 1))), so attacks on smaller players were at most −33.3% weaker. — [kirilloid base/combat/fns.ts](https://github.com/kirilloid/travian/blob/master/src/model/base/combat/fns.ts). **Outdated for Legends since 2022.**
- Trapped-troop accounting in kirilloid's report parser: lost = losses + trapped − round(0.75 × trapped), consistent with 25% of freed prisoners dying. — [kirilloid utils.js](http://travian.kirilloid.ru/js/utils.js)

### Inferences
- Loss fraction by strength ratio R = off/def (my calculation, K = 1.5):

| R | Raid: attacker loses | Raid: defender loses | Normal: attacker loses | Normal: defender loses |
|---|---|---|---|---|
| 0.5 | 73.9% | 26.1% | 100% | 35.4% |
| 1 | 50.0% | 50.0% | 100% | 100% |
| 2 | 26.1% | 73.9% | 35.4% | 100% |
| 3 | 16.1% | 83.9% | 19.2% | 100% |
| 5 | 8.2% | 91.8% | 8.9% | 100% |
| 10 | 3.1% | 96.9% | 3.2% | 100% |
| 20 | 1.1% | 98.9% | 1.1% | 100% |

  Raid and normal attacker losses converge when the attacker is much stronger. Raids mainly protect the attacker when the fight is close or lost: survivors always remain to carry loot and the attacker is never wiped out.
- "Empty" targets still have passive defence. Expected raid losses (my calculation, unupgraded Clubswingers at 40 off, K = 1.5, using kirilloid's wall formulas from Q8):

| Target (no troops) | Def | 10 clubs lose | 50 clubs lose | 200 clubs lose |
|---|---|---|---|---|
| No wall | 10 | 0.04 units | 0.02 | 0.01 |
| Teuton Earth Wall 10 | 85 | 0.9 (9.0%) | 0.44 (0.9%) | 0.22 |
| Gaul Palisade 10 | 115 | 1.3 (13.4%) | 0.68 (1.4%) | 0.35 |
| Roman City Wall 10 | 148 | 1.8 (18.3%) | 0.98 (2.0%) | 0.50 |
| Residence 10 only (+200) | 210 | 2.8 (27.6%) | 1.65 (3.3%) | 0.85 |
| Residence 10 + Roman wall 10 | 417 | 5.2 (51.5%) | 4.3 (8.7%) | 2.3 (1.2%) |
| Palace 20 (+800) | 810 | 7.4 (74.2%) | 10.2 (20.5%) | 6.2 (3.1%) |

  When x ≫ 1, absolute losses ≈ N·(D/(N·a))^1.5 ∝ 1/√N. Bigger parties lose fewer units in total, which is why raiders "over-send" against walled inactives.
- Loot per raid = min(Σ carry of **surviving** attackers, Σ resources above the (dipped) cranny). Losses therefore cut income twice: once in unit cost, once in carry.
- An empty target (nothing above the cranny) returns troops with nothing. Losses still follow the passive-defence math above, so raiding a walled empty village is net negative.

### Gaps
- **How loot is split across wood, clay, iron and crop** when carry is less than the available stock is not documented in any official page found. A forum thread asks exactly this without a confirmed answer, per a search-result summary. The common community assumption of an even split with leftovers redistributed is **unverified**.
- How fractional losses become whole units (rounding vs randomness) is unconfirmed.
- Whether Legends uses exactly kirilloid's K formula and defence weighting is not officially documented; the formulas are community reverse-engineering.

## Q4. Cranny: capacity per level, maximum, number allowed, tribe modifiers; other ways defenders hide resources

### Takeaway
In Legends a cranny hides 200 of each resource at level 1 and 2,000 at level 10 (Gauls 300/3,000, ×1.5). Once one cranny reaches level 10 you may build more, with no stated limit. A Teuton hero accompanying the raid reduces the cranny to 80% effectiveness. Thief items on any hero add a further 10–20% dip. Defenders also protect resources by spending them, sending them away, keeping them in the hero inventory, or avoiding open marketplace offers.

### Cited Findings
- Level 1 hides 200 of each resource (300 for Gauls). Level 10 hides 2,000 (3,000 for Gauls). Gaul crannies hide 1.5× more. After one cranny reaches level 10 you can build more in the same village, with no stated limit. Crannies cannot be targeted directly by catapults, only hit by random catapult targets. — [Official: Hiding Resources & Cranny](https://support.travian.com/en/articles/35-hiding-resources-and-cranny)
- Teutons: if a Teuton hero accompanies the raid, the target's cranny protects only 80% of normal capacity. The reduction applies only when the Teuton hero is in the raiding force. — [Official: Hiding Resources & Cranny](https://support.travian.com/en/articles/35-hiding-resources-and-cranny). The same appears as "Hero provides a plunder bonus of 20% cranny dip" ([Official: Teutons](https://support.travian.com/en/articles/8-the-teutons-and-their-advantages)) and "+20% for all troops attacking with the hero".
- The "Bag of Plunder" hero item increases the amount that can be raided from any village (no figure on the official page). — [Official: Hiding Resources & Cranny](https://support.travian.com/en/articles/35-hiding-resources-and-cranny). kirilloid gives Pouch/Bag/Sack of the thief as +10/15/20% "plunder bonus" ([kirilloid items.php](http://travian.kirilloid.ru/items.php)). kirilloid's T4 cranny note reads: "Crannies have 20% less capacity against teuton raids with hero + 10-20% from any hero with sack item" ([kirilloid build.php](http://travian.kirilloid.ru/build.php)).
- Per-level capacity (kirilloid): cranny(L) = 10·round(10·1.2917^(L−1)), doubled from version 4.4 on. That gives 200, 260, 340, 440, 560, 720, 920, 1,200, 1,540, 2,000 for levels 1–10, matching the official level 1 and level 10 values. Gaul ×1.5 gives 300, 390, 510, 660, 840, 1,080, 1,380, 1,800, 2,310, 3,000. — [kirilloid build.php](http://travian.kirilloid.ru/build.php) (`function cranny` and `buildings['t4.4']`); intermediate values my calculation
- Version history (kirilloid): T3 cranny was 100–1,000, Gaul 2×, and the Teuton dip needed no hero. In T4.2 cranny capacity was multiplied by server speed. From T4.4 it is ×2 (200–2,000) with Gaul 1.5×. — [kirilloid build.php](http://travian.kirilloid.ru/build.php). **T3/T4.2 numbers are outdated for Legends.**
- Cranny build cost at level 1: 40/50/30/10 resources, cost factor ×1.28 per level, 10 levels. — [kirilloid build.php](http://travian.kirilloid.ru/build.php)
- Other protection: resources in the hero inventory can't be raided; marketplace offers can be raided until merchants leave. — [Official: Hiding Resources & Cranny](https://support.travian.com/en/articles/35-hiding-resources-and-cranny). Keep stores low when attacks are expected by queueing upgrades, training troops and sending resources to allies; resources in transit can't be stolen. — [Official: Guide: Resources Explained](https://support.travian.com/en/articles/214-guide-resources-explained)
- During beginner's protection, players can post worse-than-1:1 marketplace offers that nobody can accept, which the Help Center suggests as extra storage. — [Official: Beginner's Protection](https://support.travian.com/en/articles/12-beginner-s-protection)

### Inferences
- Effective hidden amount per resource = Σ crannies × (1 − dip). Dip = 0.20 with a Teuton hero, plus 0.10/0.15/0.20 with a thief item, so up to about 0.40. Example: one level-10 cranny hides 2,000 normally, 1,600 against a Teuton hero, and about 1,200 against a Teuton hero with Sack of the thief (my calculation, assuming the dips add, as kirilloid's note implies).
- A fully developed village with several level-10 crannies (e.g. 3 × 2,000 = 6,000 per resource, Gaul 9,000) makes small inactive farms worthless until stock exceeds that. Raid income from active players is mostly overflow above the cranny.
- The cranny does not scale with server speed in kirilloid's T4.4+ model while production does. On fast servers the same cranny therefore covers fewer hours of production (inference; see Gaps).

### Gaps
- Whether Legends cranny capacity scales with server speed is not stated on the official page. kirilloid's T4.4 entry suggests a fixed ×2, but the version-inheritance chain was not fully verified.
- Whether the Teuton hero dip and thief-item dip add or multiply is not officially specified.

## Q5. Protections and limits on raiding: beginner's protection, morale, small-player caps, truces, Natars and robbers, unoccupied oases

### Takeaway
Beginner's protection lasts 5 days plus an optional 3-day extension at x1 (shorter on faster worlds). It cannot be ended early. Protected players can neither attack players nor be attacked, but can raid Natars and unoccupied oases. The morale bonus was removed in 2022, and no rule protecting small players was found. Truces turn landing attacks into "greetings" and bar attacks on Natars and oases. Robber hideouts and robber camps are Travian **Kingdoms** features; no Legends equivalent was found. Unoccupied oases produce about 71–111 res/h at x1 into 1,000–2,000 storage. Killing their animals pays the hero 40 of each resource per point of animal upkeep.

### Cited Findings
**Beginner's protection**
- Duration: x1 5 days + 3-day extension; x2 3+3; x3 3+3; x5 2+2; x10 1+1. You can extend once; after it ends it can't be regained; "Protection cannot be ended early." — [Official: Beginner's Protection](https://support.travian.com/en/articles/12-beginner-s-protection)
- Protected players can't attack, raid or scout other players, or send them reinforcements. Others can't attack, raid, scout or reinforce them. Protected players **can** attack Natars and unoccupied oases. — [Official: Beginner's Protection](https://support.travian.com/en/articles/12-beginner-s-protection)
- Protected players can't send resources. They can receive them from unprotected players. Under 200 population they can only accept trades at 1:1 or better. — [Official: Beginner's Protection](https://support.travian.com/en/articles/12-beginner-s-protection)

**Who can be attacked; small-player limits**
- You can attack players who are not in beginner's protection, on vacation, or banned. No population threshold or "much smaller player" rule is listed. — [Official: Interacting with Other Players](https://support.travian.com/en/articles/11-interacting-with-other-players)
- The morale bonus (population-difference) was removed in 2022. — [Official: Battle Mechanics](https://support.travian.com/en/articles/63-battle-mechanics)
- Vacation days available per round: x1 15, x2 8, x3 5, x5 3, x10 2. — [Official: Game Versions and Speed](https://support.travian.com/en/articles/20-game-versions-and-speed). Vacation mode can't be used during beginner's protection (search-result summary of [Official: Vacation Mode](https://support.travian.com/en/articles/61-vacation-mode)).

**Truce**
- Truces are temporary breaks around major holidays, with announced start and end times. Attacks landing during a truce only "submit greetings". Attacks on Natar villages and oases are not allowed (daily tasks for these are prefilled). Adventures, settling and scouting are allowed. No starvation during the truce and for 24 h after. Natars still attack settlers in the grey area. — [Official: Truce](https://support.travian.com/en/articles/62-truce)

**Natars**
- Natar attack delay in the grey area: 24 h (x1), 12 h (x2), 8 h (x3), 4:48 (x5), 2:24 (x10). — [Official: Game Versions and Speed](https://support.travian.com/en/articles/20-game-versions-and-speed)
- kirilloid Natar unit stats: e.g. Pikeman 20/35/50, Thorned Warrior 65 off, Guardsman 100/90/75. — [kirilloid units.js](http://travian.kirilloid.ru/js/units.js)

**Robber hideouts and robber camps (Kingdoms, not Legends)**
- Robber hideouts (personal, scale with the account's offence, production and population; about 2.4 per day at x1 on days 1–14, about 1 per day after day 50) and robber camps (shared kingdom challenges) are documented on the **Travian Kingdoms** help site (from search-result summaries). — [Kingdoms: Robber Hideouts](https://support.kingdoms.com/en/articles/15-robber-hideouts); [Kingdoms: Robber Camps](https://support.kingdoms.com/en/articles/21-robber-camps)
- A Help Center Huns article mentions a "weekly robbers ranking", which appears to be a raiding leaderboard (from a search-result summary only). — [Official: 5 things to consider about Huns](https://support.travian.com/en/articles/147-5-things-to-consider-about-huns)

**Unoccupied oases**
- Unoccupied oasis production at x1 (per hour) and capacity:

| Type | Production (wood/clay/iron/crop) | Capacity |
|---|---|---|
| Type 1 (25% of one resource), e.g. lumber | 40/10/10/11 | 1,000 |
| Type 2 (resource + 25% crop) | 40/10/10/41 | 2,000 |
| Type 3 (50%, Natarian area only) | 80/10/10/11 | 2,000 |
| Crop Type 1 | 10/10/10/41 | 1,000 |
| Crop Type 2 | 10/10/10/81 | 2,000 |

  Oases produce nothing until beginner's protection ends. — [Official: Oasis](https://support.travian.com/en/articles/48-oasis)
- A second official page phrases it as main resource 40/h and each non-focus resource 10/h at x1 (example: wood+crop oasis 40/10/10/40), doubling at x2 and tripling at x3. Production starts only after all originally spawned animals are removed. — [Official: Early Game Oasis Farming](https://support.travian.com/en/articles/190-early-game-oasis-farming). The two official pages differ slightly (crop 11 vs 10; production start condition).
- Animal kill bounty goes to the hero inventory: "each supply point gives 40 of each resource". Rats 40, wild boars 80, bears 120, crocodiles and tigers 120, elephants 200 (each resource). The full bounty is paid even if all attackers including the hero die. — [Official: Early Game Oasis Farming](https://support.travian.com/en/articles/190-early-game-oasis-farming); [Official: Oasis](https://support.travian.com/en/articles/48-oasis)
- No animal respawns during the initial protection window (x1 5 days, x2/x3 3, x5 2, x10 1). Afterwards animals spawn "immediately and very quickly", weak ones (rats, spiders, boars) first. Spawns by oasis: iron and iron/crop get rats, spiders, bats; clay and clay/crop get rats, spiders, wild boars; wood and wood/crop get wild boars, wolves, bears; crop gets rats, snakes, bears, crocodiles, tigers. — [Official: Early Game Oasis Farming](https://support.travian.com/en/articles/190-early-game-oasis-farming)
- Nature unit stats (off / def inf / def cav / upkeep): Rat 10/25/20/1; Spider 20/35/40/1; Serpent 60/40/60/1; Bat 80/66/50/1; Wild boar 50/70/33/2; Wolf 100/80/70/2; Bear 250/140/200/3; Crocodile 450/380/240/3; Tiger 200/170/250/3; Elephant 600/440/520/5. — [kirilloid units.js](http://travian.kirilloid.ru/js/units.js)
- Raiding a **player-owned** oasis steals up to 10% of its stored resources, which fully regenerate over 10 minutes (raiding after 7 minutes yields 7%). Raiding your own oasis gives nothing. — [Official: Oasis](https://support.travian.com/en/articles/48-oasis)
- Official oasis-farming guidance: farm only if "resource cost of your troop losses is at least twice lower" than the gains. Hero farming is efficient from hero level 30+. Easy oases for cavalry: iron, iron-crop, clay, clay-crop; medium: wood, wood-crop; hard: crop, double crop. Suggested party sizes are 20–30 units for iron/clay oases and 50–100 for wood/crop. Top Hun farmers use 6/6/6 or 6/6/3 Steppe Riders/Marksmen/Marauders on wood/crop oases. — [Official: Oasis Farming (After Beginner Protection)](https://support.travian.com/en/articles/185-oasis-farming-after-beginner-protection)

### Inferences
- A protected new player's only raid income is Natars and oases. Since oases produce nothing until protection ends (and, per the other page, until the original animals are cleared), the first-days "raid income" is mainly animal-kill bounty paid to the hero, e.g. 160 total per rat and 800 total per elephant.
- Sustained income from one unoccupied Type-1 oasis is at most about 71 res/h at x1 (about 101 for Type 2, 111 for Type 3), plus up to 1,000–2,000 of accumulated stock. Respawning animals add both bounty and losses.
- With morale gone and no small-player cap found, the only structural shields for small or inactive players in Legends are beginner's protection, vacation mode, the cranny and passive defence (wall plus Residence or Palace).

### Gaps
- Animal respawn rates and maximum counts per oasis are not given officially.
- Whether the 1,000/2,000 oasis capacity is per resource or total is unclear from the summary.
- Raiding Natar villages for loot (stockpiles, defences) was not researched; the official "Natars: Villages, Troops and Game Progress" article was found but not read.
- No Legends equivalent of robber hideouts or camps was found in the Help Center. This is absence of evidence, not proof.

## Q6. Farm lists: free vs Gold Club, slot limits, how they work in Legends, automation

### Takeaway
In Legends the Farm List is a Gold Club feature. The Gold Club costs a one-time 200 Gold and lasts the whole round. Without it the Rally Point's Farm List tab shows as gold (locked). A list is tied to one village and stores target plus troops per entry. You can launch all entries in one click or selected entries, and deactivate risky targets. No scheduled or repeating sends are documented, and no official list or entry limits were found.

### Cited Findings
- The Farm List is a "premium feature included in the Gold Club". It's at the Rally Point's Farm List tab, which appears gold if you haven't bought the Gold Club. Lists are grouped by village, each sending from one village. Each entry specifies a target (village or oasis) and units. Targets come from typed coordinates or are imported from recent raids from that village. Raids go to all targets with one click or to selected ones. Entries can be deactivated and reactivated. A scouts-only entry sends a scouting mission instead of a raid. Each list shows active targets and last raid times. — [Official: Farm Lists](https://support.travian.com/en/articles/69-farm-lists)
- Gold Club: 200 Gold, one-time, lasts the whole round, non-refundable. It includes farm list, message/report archive, crop finder, trade routes, troop evasion (capital) and merchants that run up to three times. — [Official: Gold Club Overview](https://support.travian.com/en/articles/128-gold-club-overview)
- The official farm-list page mentions "automating raids" as a use case but describes no automation beyond one-click sending. — [Official: Farm Lists](https://support.travian.com/en/articles/69-farm-lists)
- **Community, unverified:** a Legends forum thread about three years old reports lists capped at 100 farms, with players asking for more (per search-result summary). — [Travian: Legends Forum thread](https://wbb.forum.travian.com/index.php?thread%2F502981-farmlists-question-segguestions%2F=)
- For contrast, **Travian Kingdoms** Travian Plus gives "+100x100 Farm list": 100 extra entries, each up to 100 villages instead of 10. Kingdoms only. — [Kingdoms support (search-result summary)](https://support.kingdoms.com/en/support/solutions/articles/7000078806-travian-plus-a-bonusy-produkce)
- Third-party browser extensions advertise farm-list tooling for Legends. Traviadator manages, exports and imports raid lists ([Chrome Web Store: Traviadator](https://chromewebstore.google.com/detail/nhmididelcdpojlfklajapmhjegjcbkg)). Travian Helper builds lists of inactive farms and raids them automatically ([Chrome Web Store: Travian Helper](https://chromewebstore.google.com/detail/gejjgdldpbiadmodpjcpphghcpaagnph)). Both are third-party tools, not game features; their standing under the game rules was not researched.

### Inferences
- In Legends, scaled raiding is in practice a paid convenience (200 Gold per round). Raid income per **player-hour** depends heavily on the Gold Club, even though raid income per **unit-hour** does not.
- Since sends are manual, the realistic raid cycle is bounded by when the player is online to click "start". The theoretical res/h in Q2 assumes continuous re-sending.

### Gaps
- Current official limits on the number of farm lists and entries per list in Legends: not found.
- Whether any 2023–2026 Legends update added timed or auto re-send was not found. The Gold Club page lists no changes.

## Q7. Hero: adventures as an income source; hero items that boost plunder or carrying

### Takeaway
Adventures appear about 3/day early on, falling to about 1/day after day 64 at x1 (×3 on x3). They give resources (into the hero inventory, where they can't be raided), silver, items or troops, but the Help Center publishes no amounts. Plunder items are the thief's Pouch/Bag/Sack (+10/15/20%), which act as a cranny dip on top of the Teuton hero's built-in 20%. Speed items (boots +25–75% beyond 20 fields; maps 30–50% faster return) raise raid throughput for the party the hero rides with. No item raising troop carry capacity was found.

### Cited Findings
- Adventure spawns at x1: every player starts with 3. About 3/day on days 0–2, 2/day on days 3–16, 1.5/day on days 17–63, 1/day from day 64; roughly 5–6 in the first 24 h. At x3: 9/day on day 0, 6/day to day 5.33, 4.5/day to day 21, then 3/day. They spawn around the capital and villages with a Hero's Mansion and never expire. Rewards depend on creation time. — [Official: Adventures](https://support.travian.com/en/articles/46-adventures)
- Possible rewards: nothing (damage and experience only), items, silver, resources (hero inventory) or scattered troops. "Resources, silver, nothing, and common consumables" are most common. Valuable items are rare, "a few in about 100 adventures". The first 10 are fixed: Horse, Resources, Units, Silver, Ointment, Book of Wisdom, Resources, Silver, Cages, Experience (resources on crafting worlds). Normal adventures mean low damage and experience; heavy ones mean more damage and double experience. — [Official: Adventures](https://support.travian.com/en/articles/46-adventures)
- Silver buys and sells items at the Auction House and can be exchanged for gold. Hero item tier 2 appears at day 70 and tier 3 at day 140 on x1 (scaled on faster worlds). — [Official: Adventures](https://support.travian.com/en/articles/46-adventures) (search-result summary); [Official: Game Versions and Speed](https://support.travian.com/en/articles/20-game-versions-and-speed)
- Resources in the hero's inventory can't be raided and can be moved into any village without merchants. — [Official: Hiding Resources & Cranny](https://support.travian.com/en/articles/35-hiding-resources-and-cranny); [Official: Guide: Resources Explained](https://support.travian.com/en/articles/214-guide-resources-explained)
- Plunder items: Pouch of the thief +10%, Bag of the thief +15%, Sack of the thief +20% "plunder bonus". — [kirilloid items.php](http://travian.kirilloid.ru/items.php). The Help Center calls it "Bag of Plunder" and says it increases what can be raided from any village. — [Official: Hiding Resources & Cranny](https://support.travian.com/en/articles/35-hiding-resources-and-cranny). kirilloid's model stores this as `raid?: number // plunder bonus` on T4 items. — [kirilloid GitHub t4/items.ts](https://github.com/kirilloid/travian/blob/master/src/model/t4/items.ts)
- Teuton hero: +20% plunder by default ("cranny dip"), raised further by items. — [Official: Battle Mechanics](https://support.travian.com/en/articles/63-battle-mechanics); [Official: Teutons](https://support.travian.com/en/articles/8-the-teutons-and-their-advantages)
- Speed items: Boots of the Mercenary/Warrior/Archon +25/50/75% army speed beyond 20 fields. Small Map/Map/Large Map 30/40/50% faster return. Pennants 30/40/50% faster between own villages. Standards 15/20/25% faster between alliance members. — [kirilloid items.php](http://travian.kirilloid.ru/items.php). Boots add to the Tournament Square bonus — [Official: Tournament Square (search-result summary)](https://support.travian.com/en/articles/71-troops-speed-increase-and-tournament-square)
- Other tribe hero bonuses: Egyptian hero boosts resource production; Roman hero +100 strength per point (vs 80); Spartan hero +50% from Spartan weapons; Viking hero +5% loyalty damage. — [Official: Tribes](https://support.travian.com/en/articles/3-the-tribes-and-their-advantages); [Official: Vikings](https://support.travian.com/en/articles/139-vikings-in-travian-legends)
- Hero oasis farming: recommended early kit is Horse, Small Spurs, Small Map, any right-hand weapon, Helmet of Awareness, Light Scale Armour, with stocks of ointments and a water bucket; efficient from hero level 30+. Send a small squad right after the hero to collect stored oasis resources. — [Official: Oasis Farming (After Beginner Protection)](https://support.travian.com/en/articles/185-oasis-farming-after-beginner-protection)

### Inferences
- Adventures are a lumpy, unquantified supplement. Officially, resources are only "most common outcomes" with no amounts, so adventure income can't be computed per hour from official data.
- The Teuton hero + Sack of the thief combination (about 40% total dip) matters most against active players with large crannies, and only for the one party the hero accompanies.

### Gaps
- Resource and silver amounts from adventures (and how they scale with world age) are not published in the pages read.
- No hero item increasing troop carry capacity was found in kirilloid's T4 item list or the Help Center.
- Whether the hero itself carries loot in Legends was not confirmed.

## Q8. Defensive counters that make raiding costly: traps, wall, defensive troops, dodging, healing

### Takeaway
Gaul Trappers capture up to 400 attackers per level-20 Trapper, with more Trappers allowed. Raids can't free prisoners, and freeing them by normal attack kills 25%. Walls multiply all defence (Romans +80.6% at level 20, Teutons +48.6%, Gauls/Egyptians +63.9%, Huns +34.7%) and add flat defence. The Residence or Palace adds 2·level² passive defence (800 at level 20). Even troop-less villages can therefore bleed small raid parties (see Q3). Defenders also dodge with Gold Club troop evasion (capital) and vacation mode. Hospitals and the Spartan Asclepeion heal part of the losses.

### Cited Findings
**Trapper (Gauls)**
- Gaul-only. One Trapper holds up to 400 traps. After one reaches level 20, more can be built in the same village. Trapped troops can't harm the village and keep eating crop from their home village. **Raids cannot free trapped troops; only a successful normal attack can**, and then 25% of the trapped troops die and all used traps are destroyed. If the Trapper owner releases captives, destroyed traps are repaired for free. If the troop owner disbands trapped units, they die. — [Official: Trapper](https://support.travian.com/en/articles/41-trapper)
- Gaul trappers capture enemy troops during raids and attacks; a full attack tries to free your or your allies' trapped troops; 25% of freed troops die. — [Official: Battle Mechanics](https://support.travian.com/en/articles/63-battle-mechanics)
- Traps per level (kirilloid T3.5+ formula: L ≤ 10: (L²+21L−2)/2; L > 10: (L²+19L+20)/2): 10, 22, 35, 49, 64, 80, 97, 115, 134, 154, 175, 196, 218, 241, 265, 290, 316, 343, 371, 400 for levels 1–20. — [kirilloid GitHub t3.5/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/t3.5/buildings.ts) (values my calculation; level-20 value matches the official 400)
- Trapper cost 100/100/100/100 at level 1 (×1.28 per level). kirilloid notes: "Multiple trappers don't build/repair traps simultaneously. Repair time is always 1 minute and doesn't depend on server speed or level of trapper." — [kirilloid build.php](http://travian.kirilloid.ru/build.php)

**Walls and passive defence (kirilloid T4 values)**
- Wall effect = flat def + multiplier. Roman City Wall +10 per level, ×1.030^L. Teuton Earth Wall +6 per level, ×1.020^L. Gaul Palisade +8 per level, ×1.025^L. Egyptian Stone Wall +8 per level, ×1.025^L. Hun Makeshift Wall +6 per level, ×1.015^L. — [kirilloid GitHub t4/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/t4/buildings.ts); [t4.fs/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/t4.fs/buildings.ts); [kirilloid build.php](http://travian.kirilloid.ru/build.php)
- At level 20 (my calculation from those formulas): Roman +80.6% and +200 flat; Teuton +48.6% and +120; Gaul +63.9% and +160; Egyptian +63.9% and +160; Hun +34.7% and +120. At level 10: Roman +34.4%/+100; Teuton +21.9%/+60; Gaul/Egyptian +28.0%/+80; Hun +16.1%/+60.
- Wall durability notes (kirilloid): City Wall 1×, Earth Wall 5×, Palisade 2×, Stone Wall 5×, Makeshift Wall 1×, Viking Barricade 2×. — [kirilloid build.php](http://travian.kirilloid.ru/build.php). Help Center: Earth Wall "nearly indestructible, but only a small defensive bonus"; Makeshift Wall weak; Spartan Defensive Wall weak; Viking Barricade gives a defensive bonus (no figures). — [Official: Tribes](https://support.travian.com/en/articles/3-the-tribes-and-their-advantages); [Official: Teutons](https://support.travian.com/en/articles/8-the-teutons-and-their-advantages)
- Residence or Palace passive defence = 2·level² (kirilloid), i.e. 800 at level 20, matching the official "800 passive defence points" at level 20. Empty-village base defence is 10. — [kirilloid GitHub base/buildings.ts](https://github.com/kirilloid/travian/blob/master/src/model/base/buildings.ts); [Official: Battle Mechanics](https://support.travian.com/en/articles/63-battle-mechanics)

**Defensive troops** (def vs infantry / def vs cavalry, from [kirilloid units.js](http://travian.kirilloid.ru/js/units.js))
- Phalanx 40/50 (cost 315); Spearman 35/60; Praetorian 65/35; Legionnaire 35/50; Druidrider 115/55; Haeduan 60/165; Ash Warden 55/40; Anhur Guard 110/50; Resheph Chariot 120/150; Shieldsman 85/45; Elpida Rider 120/90; Shield Maiden 50/30; Huskarl Rider 95/100. Defence is weighted by the attacker's infantry/cavalry offence mix (Q3).

**Dodging, evasion, healing**
- Gold Club "Troop Evasion" moves troops away from your capital before an incoming attack (mechanics not given). — [Official: Gold Club Overview](https://support.travian.com/en/articles/128-gold-club-overview)
- Players on vacation can't be attacked. — [Official: Interacting with Other Players](https://support.travian.com/en/articles/11-interacting-with-other-players)
- The Hospital heals wounded troops at the same cost as training in half the time. Equipped bandages revive a share of troops right after battle. — [Official: Battle Mechanics](https://support.travian.com/en/articles/63-battle-mechanics). kirilloid: Hospital "Heals up to 40% troops lost in battle" ([kirilloid build.php](http://travian.kirilloid.ru/build.php)). Spartan Asclepeion "restores up to 60% of lost troops" ([Official: Tribes](https://support.travian.com/en/articles/3-the-tribes-and-their-advantages)).
- Viking Berserkers: each Berserker killed while attacking or raiding kills one enemy unit in return, calculated after the main battle. — [Official: Vikings](https://support.travian.com/en/articles/139-vikings-in-travian-legends)

### Inferences
- Against Gauls, raiding is uniquely risky because a raid can neither free trapped troops nor avoid the traps. Each trapped unit loses its upkeep-bearing value until freed by a normal attack that kills 25% of it. Small farm-list parties sent at a Gaul with a few hundred traps can lose whole parties to capture.
- Because raid losses fall roughly with 1/√(party size) against fixed passive defence (Q3), the wall plus Residence or Palace of an inactive or active village sets a minimum efficient party size. That trades against Q2's income-per-unit, since a bigger party means more idle carry if the target holds little.
- Hospitals lower the attacker's effective loss cost (up to 40%, or 60% for Spartans, healed at training cost in half the time), but healing still costs resources. Expected-loss cost per raid falls by at most that share.

### Gaps
- Spartan Defensive Wall and Viking Barricade bonus percentages: not found in kirilloid's T4.6 data or the Help Center.
- Troop-evasion rules (conditions, which troops, return time) are not described on the official page read.
- Bandage percentages and the Legends hospital's exact rules (which losses count as "wounded", attacker vs defender) were not researched in detail; the "Hospital and Asclepeion" article exists but was not read.
- No official Legends per-level trap table was read. The per-level values come from kirilloid's T3.5 formula, consistent only with the official level-20 maximum.
