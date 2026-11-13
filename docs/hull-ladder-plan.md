# Hull ladder: EVE-shaped hull costs

Status: PLAN, Nov 7 2026, written at Peter's go; nothing built. Follows
docs/reactor-drive-split.md (reactors and drives, built) and supersedes the
engine repricing proposed the same day. The Scout is left as it is ("quasi
hide it for now"); guns stay fixed kit parts (Pulse Laser, Beam Battery), as
settled Nov 6 and re-confirmed Nov 7.

Peter (Nov 7 2026): "I want the player to start building interceptors and
then the costs to build next hull should be very large increase in costs …
I believe it is quasi exponential"; "let's up it to 5x and dreadnought and
titan should be 10x"; the Corvette "should be the next hull after
interceptor"; "I want the cost of the reactor and drive to be 50%
approximately of the cost of the ships, the rest being the hull and weapons";
and the raid and trader changes in §5.

## 1 · EVE's ladder, measured

eve-kill.com 90-day Jita averages, one faction line (Minmatar), Nov 2026:

| Class | Hull | ISK | Step |
|---|---|---|---|
| Frigate | Rifter | 0.30M | |
| Destroyer | Thrasher | 1.14M | ×3.8 |
| Cruiser | Rupture | 10.4M | ×9.1 |
| Battlecruiser | Hurricane | 52M | ×5.0 |
| Battleship | Tempest | 154M | ×2.9 |
| Dreadnought | Naglfar | 2,480M | ×16 |
| Supercarrier | Hel | 43,290M | ×17 |
| Titan | Ragnarok | 143,870M | ×3.3 |

Frigate to titan ×480,000 over eight steps: a geometric mean of ×5 a class,
×16 into each capital tier. A T1 freighter (Fenrir, 1,470M) costs a
dreadnought. Our ladder today runs Interceptor 2,062 → Titan 97,980, ×48 over
five steps (×2.2 a class), because every hull is the same parts in counts
that grow linearly with mass.

## 2 · The ladder

Warships ×5 a rung, ×10 into each capital rung; freighters ×3 (hold grows only
×2.4–3 a step, so ×5 would price the big haulers out per unit carried); the
rest placed by role. The Tiny Freighter keeps Peter's two engines, so it is
the one hull over half.

| Hull | Yard | Today | Target | Engines | Engine value | Kit total | Engine share |
|---|---|---|---|---|---|---|---|
| Interceptor | Shipyard I | 2,062 | 2,000 | 1 light | 1,000 | 2,003 | 50% |
| Corvette | Shipyard II | 1,853 | 10,000 | 5 light | 5,000 | 9,975 | 50% |
| Destroyer | Orbital Shipyard I | 5,906 | 50,000 | 25 light | 25,000 | 50,041 | 50% |
| Cruiser | Orbital Shipyard II | 10,126 | 250,000 | 5 heavy | 125,000 | 250,511 | 50% |
| Battleship | Orbital Shipyard III | 22,940 | 1,250,000 | 25 heavy | 625,000 | 1,250,101 | 50% |
| Dreadnought | Capital Slipway I | 49,340 | 12,500,000 | 10 capital | 6,250,000 | 12,464,081 | 50% |
| Titan | Capital Slipway II | 97,980 | 125,000,000 | 100 capital | 62,500,000 | 125,036,461 | 50% |
| Tiny Freighter | Shipyard I | 1,445 | 3,000 | 2 light | 2,000 | 2,917 | 69% |
| Small Freighter | Shipyard I | 2,671 | 9,000 | 4 light | 4,000 | 8,943 | 45% |
| Medium Freighter | Shipyard II | 5,258 | 27,000 | 13 light | 13,000 | 26,993 | 48% |
| Large Freighter | Orbital Shipyard I | 10,877 | 81,000 | 40 light | 40,000 | 80,981 | 49% |
| Heavy Freighter | Orbital Shipyard II | 20,713 | 243,000 | 5 heavy | 125,000 | 242,025 | 52% |
| Bulk Freighter | Orbital Shipyard III | 40,108 | 729,000 | 15 heavy | 375,000 | 728,021 | 52% |
| Combat Freighter | Shipyard II | 2,520 | 6,000 | 3 light | 3,000 | 5,957 | 50% |
| Colony Ship | Shipyard I | 5,082 | 20,000 | 10 light | 10,000 | 19,939 | 50% |
| Transport | Garrison | 5,988 | 30,000 | 15 light | 15,000 | 30,011 | 50% |
| Builder | Shipyard I | 2,867 | 8,000 | 4 light | 4,000 | 7,941 | 50% |
| Mining Barge | Shipyard I | 3,422 | 6,000 | 3 light | 3,000 | 5,957 | 50% |
| Belt Station | Shipyard II | 18,224 | 60,000 | 30 light | 30,000 | 60,035 | 50% |

Interceptor → Titan is ×62,500 (EVE ×480,000); Tiny → Bulk Freighter ×243.
Prices are kit values at today's part prices; the Hub sells every part at
about that, the trader at twice.

## 3 · The mechanism

**One engine price, counts that climb, three sizes.** A light engine — a
rung-1 pair, Fission Reactor 650 + Zephyr Drive 350 — costs 1,000. The rung
ladder keeps today's proportions: Fusion 1,300 + 700, Advanced Fusion 1,960 +
1,060, Antimatter 2,720 + 1,470. A hull carries as many engines as make half
its price: Interceptor 1, Corvette 5, Destroyer 25. Above that the counts
would run away (a Titan would need 62,500), so engines and the frame come in
three **sizes**, each ×25 the one below, like EVE's small / large / capital
components:

| Size | Engine (rung 1) | Hull Section | Hulls |
|---|---|---|---|
| Light | 1,000 | 128 | Interceptor, Corvette, Destroyer, Tiny–Large Freighter, Combat Freighter, Colony Ship, Transport, Builder, Barge, Belt Station |
| Heavy | 25,000 | 3,200 | Cruiser, Battleship, Heavy and Bulk Freighter |
| Capital | 625,000 | 80,000 | Dreadnought, Titan |

A heavy part is assembled at the same works from 25 light ones (a capital
from 25 heavy), so a Titan's 100 capital engines are 62,500 light engines'
worth of Drive Works time spread over many lines — capital shipbuilding
becomes an industrial project, which is the EVE shape. Goods added: heavy and
capital reactors and drives (4 rungs × 2 sizes × 2 = 16) and two Hull Section
sizes. Every one is hidden until researched (§4), so a new player sees none.

**The other half** is Nav Computer, frame (Hull Sections of the hull's size),
plating (Composites, light hulls mostly), systems (Precision Components,
Semiconductors, Machinery, capped on big hulls) and the fixed guns. Per
thousand tonnes the frame goes 5, 15, 35 Hull Sections up the first three
rungs: the rising density is the exponential. Bulk skins of Alloys and loose
Weapon Mounts leave the kits; Weapon Mounts stay the material a gun is built
from.

**Reference kits** (starting values, tunable):

- **Interceptor** (2,003): 1 light engine (1,000) · 1 Nav Computer (215) · 1 light Hull Sections (128) · 6 Pulse Lasers (660)
- **Corvette** (9,975): 5 light engines (5,000) · 1 Nav Computer (215) · 16 light Hull Sections (2,048) · 32 Composites (1,600) · 3 Precision Components (276) · 9 Semiconductors (306) · 5 Machinery (310) · 2 Pulse Lasers (220)
- **Destroyer** (50,041): 25 light engines (25,000) · 1 Nav Computer (215) · 85 light Hull Sections (10,880) · 169 Composites (8,450) · 18 Precision Components (1,656) · 47 Semiconductors (1,598) · 26 Machinery (1,612) · 1 Pulse Laser (110) · 1 Beam Battery (520)
- **Cruiser** (250,511): 5 heavy engines (125,000) · 1 Nav Computer (215) · 33 heavy Hull Sections (105,600) · 124 Composites (6,200) · 45 Precision Components (4,140) · 121 Semiconductors (4,114) · 66 Machinery (4,092) · 1 Pulse Laser (110) · 2 Beam Batteries (1,040)
- **Battleship** (1,250,101): 25 heavy engines (625,000) · 1 Nav Computer (215) · 179 heavy Hull Sections (572,800) · 400 Composites (20,000) · 109 Precision Components (10,028) · 294 Semiconductors (9,996) · 161 Machinery (9,982) · 4 Beam Batteries (2,080)
- **Dreadnought** (12,464,081): 10 capital engines (6,250,000) · 1 Nav Computer (215) · 77 capital Hull Sections (6,160,000) · 400 Composites (20,000) · 109 Precision Components (10,028) · 294 Semiconductors (9,996) · 161 Machinery (9,982) · 2 Pulse Lasers (220) · 7 Beam Batteries (3,640)
- **Titan** (125,036,461): 100 capital engines (62,500,000) · 1 Nav Computer (215) · 781 capital Hull Sections (62,480,000) · 400 Composites (20,000) · 109 Precision Components (10,028) · 294 Semiconductors (9,996) · 161 Machinery (9,982) · 12 Beam Batteries (6,240)
- **Tiny Freighter** (2,917): 2 light engines (2,000) · 1 Nav Computer (215) · 2 light Hull Sections (256) · 4 Composites (200) · 1 Semiconductors (34) · 1 Machinery (62) · 1 Cargo Bay (150)
- **Small Freighter** (8,943): 4 light engines (4,000) · 1 Nav Computer (215) · 15 light Hull Sections (1,920) · 30 Composites (1,500) · 3 Precision Components (276) · 8 Semiconductors (272) · 5 Machinery (310) · 3 Cargo Bays (450)
- **Medium Freighter** (26,993): 13 light engines (13,000) · 1 Nav Computer (215) · 44 light Hull Sections (5,632) · 88 Composites (4,400) · 9 Precision Components (828) · 25 Semiconductors (850) · 14 Machinery (868) · 8 Cargo Bays (1,200)
- **Large Freighter** (80,981): 40 light engines (40,000) · 1 Nav Computer (215) · 133 light Hull Sections (17,024) · 264 Composites (13,200) · 27 Precision Components (2,484) · 74 Semiconductors (2,516) · 41 Machinery (2,542) · 20 Cargo Bays (3,000)
- **Heavy Freighter** (242,025): 5 heavy engines (125,000) · 1 Nav Computer (215) · 29 heavy Hull Sections (92,800) · 110 Composites (5,500) · 40 Precision Components (3,680) · 108 Semiconductors (3,672) · 59 Machinery (3,658) · 50 Cargo Bays (7,500)
- **Bulk Freighter** (728,021): 15 heavy engines (375,000) · 1 Nav Computer (215) · 90 heavy Hull Sections (288,000) · 336 Composites (16,800) · 109 Precision Components (10,028) · 294 Semiconductors (9,996) · 161 Machinery (9,982) · 120 Cargo Bays (18,000)
- **Combat Freighter** (5,957): 3 light engines (3,000) · 1 Nav Computer (215) · 9 light Hull Sections (1,152) · 18 Composites (900) · 2 Precision Components (184) · 5 Semiconductors (170) · 3 Machinery (186) · 1 Cargo Bay (150)
- **Colony Ship** (19,939): 10 light engines (10,000) · 1 Nav Computer (215) · 34 light Hull Sections (4,352) · 68 Composites (3,400) · 7 Precision Components (644) · 19 Semiconductors (646) · 11 Machinery (682)
- **Transport** (30,011): 15 light engines (15,000) · 1 Nav Computer (215) · 52 light Hull Sections (6,656) · 103 Composites (5,150) · 11 Precision Components (1,012) · 29 Semiconductors (986) · 16 Machinery (992)
- **Builder** (7,941): 4 light engines (4,000) · 1 Nav Computer (215) · 13 light Hull Sections (1,664) · 26 Composites (1,300) · 3 Precision Components (276) · 7 Semiconductors (238) · 4 Machinery (248)
- **Mining Barge** (5,957): 3 light engines (3,000) · 1 Nav Computer (215) · 9 light Hull Sections (1,152) · 18 Composites (900) · 2 Precision Components (184) · 5 Semiconductors (170) · 3 Machinery (186) · 1 Cargo Bay (150)
- **Belt Station** (60,035): 30 light engines (30,000) · 1 Nav Computer (215) · 105 light Hull Sections (13,440) · 208 Composites (10,400) · 22 Precision Components (2,024) · 58 Semiconductors (1,972) · 32 Machinery (1,984)

Guns are unchanged in count and weight, so combat power is exactly today's;
only price moves.

## 4 · Research and visibility

- Light rung-1 parts are known from the start (the charter). Rung 2–4 parts
  follow docs/reactor-drive-split.md (Bunkerage, Helium-3 Cycle, Antimatter
  Synthesis).
- Heavy parts become known with the Orbital Shipyard's research (the Cruiser's
  yard); capital parts with the Capital Slipway's (the Dreadnought's). Hidden
  and unbuyable until then, as the Oct 28 rule asks.
- Yard gates are unchanged: Shipyard I builds the Interceptor; the Corvette
  (Shipyard II) is the first saved-for warship at 10,000.

## 5 · The charter

- **The raid takes reactors and drives** (Peter: "obviously the most valuable
  items"), then Semiconductors and Weapon Mounts. The Landfall kit carries six
  Fission Reactors and seven Zephyr Drives (every charter hull's engine less
  the trader's three); the den returns them. Den stores drop Machinery 41 → 37
  so the loot stays two Tiny loads (100 units).
- **The trader fronts one reactor and one drive** for the Interceptor at half
  the Hub price (325 + 175 = 500, "debt" the den's Authority contract repays),
  then the Tiny Freighter's pair of each at its usual twice (4,000), a fourth
  reactor (the barge's three are the den's six less the Interceptor's and the
  Tiny's) and — BUILT, after the harness — **the Small Freighter's kit beyond
  its engines**: 15 Hull Sections, 3 Cargo Bays, a Nav Computer, 30 Composites
  and 3 Precision Components (~8,700 Cr at its twice). The first cut had the
  Small Freighter as "the first Hub-fitted hull"; the charter harness put that
  step at 50–57 minutes (a Hub order is a light round trip to the Hub, then an
  Authority freight run home, ~25 minutes a cycle), so no charter hull waits
  on a Hub import now. The uncle's two Interceptors (§vendetta) are the first
  parts a player buys at the Hub.
- **Credits**: bounty 8,000, den contract 8,000 (5,000 in the first cut, before
  the trader took the Small Freighter's kit), stipends unchanged: 22,000 Cr of
  Authority money against ~17,500 Cr of trader purchases through the barge
  (`the_kit_and_the_trader_supply…` keeps the margin between 500 and 3,500).
- Charter hull prices: Interceptor 2,003, Tiny 2,917, Small Freighter 8,943,
  Mining Barge 5,957.

## 6 · What else moves

- **Balance yardstick.** Equal-cost fleet balance stops being the measure for
  capitals: a Titan at 62,500 Interceptors' price does not win an equal-cost
  fight and should not. Combat values are untouched; the balance matrix keeps
  its tactical results and loses its equal-cost targets above the Destroyer. A
  later balance pass sets role targets (what a Battleship must beat). BUILT:
  `combat_balance::TIERS` are 50k / 250k / 500k (the light ladder, under the
  300-a-side roster); Battleship archetypes and up sit every tier out; the
  tackle regression (`interceptors_hold_what_cruisers_cannot_catch…`) fights
  the v13 fleet shapes by hand (2 Battleships v 4 Cruisers / 27 / 19
  Interceptors), no longer equal-cost.
- **Stations not re-priced (Peter's call).** The orbital defence ladder kept
  its recipes, so the whole six-level ladder (~280k Cr) now costs about a
  Cruiser while its capstone out-guns a Titan (130M). Defence this cheap next
  to the capitals it stops is not a settled design; `station::tests` pins the
  ratio to "about a Cruiser" until it is.
- **Raid payback** (`a_raid_fleet_pays_raiding…`): the raid fleet is 2.4×
  dearer and loot still Hub-priced, so a slow-warp raider repays in ~3.6 days
  (was under 3); pinned at four.
- **Assembly fee.** A heavy part is priced at its 25 light parts plus 2%, a
  capital part at 25 heavy plus 2% again (`fuel::SIZE_ASSEMBLY_FEE`), so the
  Drive Works clears its basket like every converter. Kit values with the fee:
  Cruiser 257,123, Battleship 1,289,457, Dreadnought 13,042,445, Titan
  130,470,653 (+2.8% to +4.4% on the §2 targets).
- **Hub liquidity.** Engines and the heavy and capital parts trade on the
  Authority's bottomless book (`CORE_LIQUIDITY`); light Hull Sections stay a
  colony good on the ordinary book (a bottomless book for a Hull Fabricator's
  frames would be the Drive Works credit faucet again).
- **Rare materials** stay in the capital kits: Rare Elements from the Cruiser
  (20) up ×5 a rung (Titan 2,500); Iridium on the siege anchors (Battleship
  60, Dreadnought 300, Titan 1,500). Under 1.5% of any kit.
- **Hull value tests** (`rare_materials_go_into_capitals…`, the ±1% capital
  values) are re-pinned to the new kits; the efficiency invariant (firepower
  per credit declining up the ladder) holds trivially now.
- **Bots** price hulls from recipes already; the strategy's hull wants and
  cash gates need the new numbers, and the charter harness is the go/no-go.
- **Client**: build cards, Hub book rows and stores groups take the new goods
  through the existing lists; placeholder icons for heavy and capital parts.
- **Prizes and loot**: a captured hull's engines are worth far more, which is
  the point of the ladder.

## 7 · Order of work, go/no-go

0. **Income check.** Run the balancing harness to 300 minutes and read each
   bot's credits per hour at the end; extrapolate to a mid-season syndicate.
   Go if a Battleship (1.25M) is a few hours of a strong economy and a
   Dreadnought (12.5M) a day or two; otherwise scale the top rungs down
   before building anything.
1. **Parts and prices** (sim): engine 1,000 and the rung ladder; heavy and
   capital reactors, drives and Hull Sections with assembly recipes; the new
   kits; visibility gates; hull value tests re-pinned. Galaxy reset.
2. **Charter** (sim + bots): raid takes both parts, seven kit drives, den
   stores 37 Machinery, trader fronts reactor + drive, bounty 8,000, den
   contract 5,000; the charter bot finishes ExpandMining inside 80 minutes.
3. **Client**: goods lists, icons, copy.
4. Suites, client build, `--reset-galaxy` on :8080.

## 8 · Calls made here, overrule freely

1. Sizes at ×25 with assembly from the size below, rather than more light
   parts (a Titan with 62,500 engines) or a single "capital components" good
   (which would break the engines-at-half rule on capitals).
2. Freighters at ×3, Colony Ship 20,000, Transport 30,000, Builder 8,000,
   Barge 6,000, Belt Station 60,000, Combat Freighter 6,000.
3. The Tiny keeps two engines (69% share); every other hull sits at 45–51%.
4. Guns stay fixed kit parts; Weapon Mounts remain the gun material and leave
   the skins.
5. Bounty 8,000 and den contract 5,000 as starting values for the harness.
6. Heavy parts known with the Orbital Shipyard's programme, capital parts with
   the Capital Slipway's.

## 9 · Status (Nov 7 2026, night) — BUILT, uncommitted

Everything in §2–§5 is in: `fuel::EngineSize` (light / heavy / capital, ×25,
the 2% assembly fee), the 18 sized goods, sized prices, the Drive Works and
Hull Fabricator assemblies, every hull recipe from the §3 kits (with the
capitals' rare materials), `ShipKind::engines` / `engine_size`, the size gates
on build, refit, market and research visibility (sim and client), the §5
charter (raid takes reactors and drives; kit 6 + 7; trader 4 reactors + 3
drives, the first of each featured at half, plus the Small Freighter's kit;
den Machinery 37; bounty 8,000; den contract 8,000), the Landfall crate's Tiny
plating and the Reach crate's barge kit, and the client's lists, values,
colours, size-aware `goodKnown`, shipyard picker and placeholder icons (a
sized part wears its light part's icon until its master arrives).

Tests: the sim suite is green but for the pre-existing captain cap test
(`all_effects_stay_inside_the_published_caps`, 1.12 v 1.1, untouched by this
work); the server suite and the charter bot harness pass (the bot reaches
"Mining expanded" inside the 80-minute budget — see the §5 trader note and
the bot's parallel Hub orders in `logistics.rs`); every client script and
`tsc` pass. `crates/sim/src/world/opening.rs` was lost to a write-before-read
one-liner on the way and rebuilt from the session transcripts (memory:
safe-file-rewrites); its 19 tests pass.

Open for Peter: the station ladder's price next to the capitals (§6), the
heavy and capital parts' own icons (docs/shipbuilding-art-prompts.md), and
whether the raid-payback and Small-Freighter-at-the-trader calls stand.
