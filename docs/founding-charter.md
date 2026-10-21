# Founding charter v5

Status: v4 built Oct 14 2026, the privateer den added Oct 15; v5 (Oct 16
2026) replaced the counterfeit semiconductors and the ambushed Freighter
export with a pirate raid on the colony. Later on Oct 16, Peter's asks added
"Export your ore" after the den (step 12) and had the trader leave during the
den hunt (**The trader's last call**); neither needs a reset. Uncommitted.
Protocol v53
(`stellar.msgpack.v53`): deploy the client and server together, and start a
fresh galaxy (`--reset-galaxy`), because the landing, the home ore, the kit
and the den system are set when the galaxy is made or a corporation joins.
The v4 design and its decisions are in
[founding-charter-v4.md](founding-charter-v4.md); v5 changes only its
middle chapters (see **Why the raid**).

This replaces v3 (Oct 8 2026: a Freighter and an Interceptor at the start,
nine steps in three chapters). The rules in [local-opening.md](local-opening.md)
for the Service Berth, shopping caps, refining floors, focus while landing and
the welcome-back report still hold.

## The landing

- **Only the Command Center.** The colony ship lands on the habitable planet
  and becomes the Command Center (`StarSystem::bare_landing`). Nothing else
  stands anywhere in the system: no Warehouse, farms, Habitat, power plant,
  mine or ships.
- **Holds:** the ship's holds are the colony's storage, 1,000 goods
  (`build::LANDING_HOLDS`). The camp's Storehouse project adds 300
  (`STOREHOUSE_HOLDS`). A Warehouse is a later build for when storage fills,
  and it holds its full 700 on the camp world too.
- **No free power** ([power-plants-plan.md](power-plants-plan.md)): no planet
  has a free base any more. A Command Center's reactor gives its own planet
  1 MW (`power::COMMAND_CENTER_MW`); everything else needs a plant. The
  reactor runs Mining Complex I; a Shipyard needs a Solar Array.
- **Home ore:** a basic Ferrite Ore deposit on the habitable planet, so the
  first mine stands beside the Command Center. Planet III keeps its own ore
  for step 14.
- **Housing:** the 6,000 settlers live aboard until the Habitat stands. The
  camp's Habitat I houses 8,000 (12,000 with Quarters).
- **Workforce (Oct 16 2026):** 6,000 settlers are six workforce, one for each
  post the charter staffs: Bioharvester, Agroplex, Mining Complex, Shipyard,
  Academy, the second Mining Complex. Posts are capped at the free workforce
  (the + stops; the sim clamps `SetAssignment`), so nothing is shared thin.
  A Managed home calls for migrants on a staffing shortage: every worker
  posted and a built post still empty.
- **No ships.** Every hull is built at the founder's own Shipyard and fills
  its tanks at the dock.

**The kit** (`galaxy::charter_kit`), in the holds with 2,000 Cr:

| Good | Units | For |
| --- | ---: | --- |
| Alloys | 410 | every charter build, plus the Uranium Mill |
| Machinery | 163 | the same |
| Polymers | 40 | Habitat 20, a Tiny Freighter 8, Barge 12 |
| Provisions | 100 | about 5½ minutes of food for 6,000 while the farms go up, plus the Academy's 20 |
| Uranium | 120 | 100 in the kit and the home's 20 seed: a Tiny Freighter (52.5), the Survey Probe and most of the Barge |
| Deuterium | 10 | the home seed |
| Armaments | 15 | the Interceptor; the raid takes them (`KIT_ARMAMENTS`) |
| Warp Cores | 3 | Interceptor, Barge, a Tiny Freighter |
| Semiconductors | 40 | the raid takes them (`KIT_SEMICONDUCTORS`) |

The kit covers every charter build except semiconductors, including the
Tiny Freighter of step 12's export, about 860 units of the 1,000.

## Eighteen steps in five chapters

| Chapter | Step | Completes when (server facts) |
| --- | --- | --- |
| Landfall | 1. Harvest biomass | A Bioharvester is built and staffed. |
| | 2. House the settlers | A Habitat is built. |
| | 3. Make provisions | An Agroplex is built and staffed. |
| | 4. Bring the mine online | The home's site report shows a staffed Mining Complex. |
| Raid | 5. Pirates raid the colony | The report of what the raid took reaches command (see **The raid**). |
| | 6. Sell ore to the trader | Any Ferrite Ore sold to the free trader, or the trader has gone. |
| | 7. Power the Shipyard | A Solar Array is built at home. |
| | 8. Build a Shipyard | A Shipyard is built and staffed. |
| Pirates | 9. Build an Interceptor | The corporation owns an Interceptor. |
| | 10. Destroy the privateer | The privateer's defeat is reported and the 4,500 Cr bounty paid. |
| | 11. Eliminate the privateer den | The report that the den fell reaches command. |
| Expansion | 12. Export your ore | Word that ore is on its way to the Market Hub reaches command (see **The export**). |
| | 13. Start research | A programme is running or finished and an Academy is staffed. |
| | 14. Expand mining operations | Another world of the home system runs a staffed Mining Complex beside its own power plant. |
| | 15. Chart a nearby system | A survey report of the den's system arrives. |
| Reach | 16. Build a Mining Barge | A barge exists. |
| | 17. Mine the belt | The report that a barge is cutting ore at a belt away from home reaches command. |
| | 18. Bring the ore home | The report that a barge unloaded or delivered ore arrives. |

Steps 1–10 run in `world/opening.rs` (`advance_local_opening`), the raid in
`world/founding_raid.rs`, the rest in `advance_founding_programs`. The export
is noted by `note_founding_export` (`world/opening.rs`).

**Landfall:** Bioharvester 25 Alloys + 12 Machinery; Habitat 30 Alloys + 20
Polymers + 8 Machinery; Agroplex 30 Alloys + 15 Machinery; Mining Complex 25
Alloys + 12 Machinery.

**Why the raid** (Peter, Oct 16 2026): with the trader docked at the colony
and buying its ore, a Freighter run to the Hub 125k su away made no sense; it
existed only to be ambushed. The counterfeit added a second, unrelated
villain. The raid gives one enemy and one cause: the pirates take what the
colony needs for ships, the trader sells it back dear and buys ore cheap, and
the den holds the loot. Peter chose the raid that takes **all 40 semiconductors
and all 15 Armaments** ("option 1"), so the founder must sell about 200 ore.

**The raid** (step 5, `world/founding_raid.rs`)
- **Launch:** the moment the mine comes online, the Rogue Privateer sets out
  from beyond the colony's sensors on its den's side
  (`RAID_START_SENSOR_RANGES` 1.25 sensor ranges out, never past the den), its
  den rising with it (`ensure_founding_den`). A home without a den sends it
  from the dark side, away from the Hub.
- **Seen:** the first time the Command Center or a Sensor Array detects it,
  by the shared rule, stamped on its light home (`raid_seen_at`). From then
  on the colony tracks it wherever it goes (`view::tracked_contacts`).
- **Over the colony** inside its gravity well's edge, it freezes what it may
  take: every unit of Semiconductors and Armaments in the home stockpile
  (`RAID_TAKES`), and loads 4 units a second (`RAID_LOOT_PER_SEC`), the
  dearest first, into its own hold. Nothing else is touched; no blockade,
  no battle unless the founder already has a warship (it holds off loading
  while engaged), and it never stays past `RAID_MAX_HOLD_S` (90 s).
- **Report:** when it breaks off, a `FoundingRaided` event carries what it
  took; the timeline line "Pirates raided {home} and took 40 semiconductors
  and 15 armaments." raises the raid's pop-up (`piratesEvent`), and its
  arrival (`raid_report_at`) completes the step and calls the trader.
- **Timing** (one seed, the den 15,000 su out): seen 19 s after the mine
  comes online, over the colony at 31 s, report at 44 s, the trader docked
  at 84 s; the privateer is home at its den and lurking by about 2 minutes.
- **Home:** it unloads the loot into the den's stores (`Enclave::plunder`),
  then lurks 35% of the way from the den toward the colony
  (`RAID_LURK_FRACTION`): beyond the colony's sensors, but tracked.
- Persisted (`FoundingProgram::raid`): a reload neither repeats the raid nor
  refills what it may take.

**The trader and the ore** (step 6)
- The free trader is called when the raid's report arrives and docks about
  40 s later ([traveling-merchant.md](traveling-merchant.md)). It now sells
  Armaments too (15), and takes up to 300 ore (`ORE_CAP`, was 150).
- The guide: "Sell ore to the trader · We'd better sell ore to the trader to
  buy semiconductors for our ships. [Sell ore]" (Peter's wording), which opens
  the stall on its Buying tab with all the ore it will take filled in
  (`merchant-ore`). Until the trader's own pop-up has shown, the card is
  "Find semiconductors · The Solar Array needs 8" at the Market.
- **The money:** 2,000 Cr start + 300 (mine) + 300 (raid) + 400 (Shipyard
  staffed) = 3,000 Cr, against about 3,810 Cr to buy back 50 semiconductors
  (Solar Array 8, Shipyard 15, Shipyard II 15, Interceptor 12) and 15
  Armaments at the trader's markup: about 810 Cr short, about 200 ore at its
  4 Cr (half the Hub's 8). The mine makes 23 ore a minute.
- The step also completes if the trader leaves without a sale, so it is never
  a dead end.

**The Interceptor** (step 9): Interceptors keep the Shipyard II gate.
Shipyard II costs what Shipyard I does (40 Alloys, 20 Machinery, 15
semiconductors); the Interceptor costs 20 Alloys, 12 semiconductors, 15
Armaments and a Warp Core.

**The privateer** (step 10)
- The guide first offers the reserve captain: the founding captain waits at
  home, and `AssignCaptain` puts them aboard the Interceptor docked there.
  Guide-only: the kill completes the step, captain or not.
- Then a one-click attack (`AttackFleet`) on the lurking privateer, tracked
  since the raid. A docked Interceptor with an empty tank gets "Fuel the
  Interceptor · Buy Deuterium" from the trader first (`merchant-fuel`).
- The kill promotes the captain aboard: their experience is topped up to
  level 2's 100 (`PRIVATEER_PROMOTION_LEVEL`; the kill alone earns 85). Any of
  the founder's fleets in the fight counts.

**The den** (step 11)
- **Where:** each home's rocky local survey stop is now its den system
  (`HomeSlot::den_system`, set in `seed_exploration_stars`). It holds only an
  asteroid belt, its deposits ×1.8 (`DEN_BELT_RICHNESS_MULT`), one of the
  home's three doorstep stars 15–20k su out ([hub-district.md](hub-district.md)).
- **When:** the den rises with the privateer as the raid sets out
  (`ensure_founding_den`), so nothing stands near home before the pirates come.
- **What:** a permanent pirate enclave (`Enclave::founder_den`): three Raiders
  at 75% hull flying the privateer's weak guns and slow hulls, no defense
  platforms. It never grows, launches raids or offers an Authority bounty, and
  once cleared it stays cleared. One Interceptor wins: 27–54 s over eleven
  seeds, 45% hull left at worst.
- **Revealed** by the privateer's log: an intel snapshot taken where and when
  it died, so the den reaches command with the kill report (`☠ PRIVATEER DEN`
  on the map, "Privateer den" in the inbox).
- **Cleared** by the ordinary pirate assault: an idle combatant within 220 su
  of the den system fights the garrison. Done when the clearance report
  reaches command (`den_report_at`). Its stores, 40 Alloys and 15
  semiconductors plus everything the raid took, are offered back as salvage
  contracts (`offer_site_plunder`), to recover with any hold.
- **Then:** the den system is the survey stop (step 15) and its belt the
  barge's (steps 16–18); a den cleared for good no longer bars a belt. Its
  stores wait for a Freighter; the charter does not send one.
- A home without a den system (an overflow home) skips the step.

**The land grant** (until step 14)
- During the charter, structures in the home system can only go on the landing
  world. Anywhere else the sim refuses the order: "Charter land grant: {world}
  only, until you expand mining" (`FoundingProgram::land_grant_open`).
- The build sheet shows other worlds' structures locked with the same reason.
- Ships and the Survey Probe are unaffected; they build at the yard.

**Expanding the mine** (step 14): a Solar Array on another home world with ore
first (a separate grid with no free power), then a Mining Complex beside it,
staffed. Planet III is the usual one; the guide picks a planet before a belt.

**The trader's last call** (Peter, Oct 16 2026: "Teo should leave while the
player is eliminating the pirates")
- Once the privateer's kill is reported, with its bounty, the trader stays
  3 minutes more (`merchant::LAST_CALL_S`); the den's fall cuts that short,
  so it is gone by step 12. The stall's clock shows the time left.
- After it, only the Market Hub sells what the kit no longer covers: the raid
  took its semiconductors, and the builds after the den need 33 (Academy 15,
  Solar Array 8, Survey Probe 4, Mining Barge 6). So the bounty buys them
  now. The guide's card: "Stock up before the trader leaves · 33
  Semiconductors for the builds ahead. [Buy from trader]" (`merchant-ahead`),
  with the den step's own card beside it, and a "Trader leaves in m:ss" line.
  The prefill counts the whole recipes of the Academy, the expansion's Solar
  Array and Mining Complex, the probe and the barge, less what home holds,
  bounded by the trader's stock (`AHEAD` in `core/derive/merchant.ts`).
- Teo's greeting during the last call: "Last call, friend! I'm off before the
  shooting starts." (after a deal: "Anything else? The clock's ticking.").
- **Why:** measured with a bot (Oct 16 2026). With the trader leaving at the
  bounty and no stock-up, research slipped from 21.7 to 46.2 game-minutes and
  mining had not expanded by minute 70: the chips came by Authority freight
  from the Hub.
- The timeline line on departure: "Teo Brannick's Long Odds left {home}."

**The export** (step 12, Peter, Oct 16 2026: "insert an export ore after
step 11")
- **Done when** word that ore is bound for the Market Hub reaches command
  (`export_dispatched_at`): a Freighter carrying Ferrite Ore past the home's
  gravity well on its way to the Hub (light-delayed from the ship), or an
  Authority freight booking of ore from home to the Hub warehouse.
- **The guide:** "Build a Tiny Freighter · 20 Alloys · 8 Machinery · 8
  Polymers · 1 Warp Core. It fuels with Uranium at the dock." (the kit covers
  it; the reason gives the Hub's ore price), then "Export your ore · Load
  Ferrite Ore and send the Freighter to the Market Hub. [Plan the shipment]",
  which opens Logistics on the first-shipment card: 50 ore out, the
  Academy's goods back within a spending cap, one trip. Under way: "Shipment
  under way" and a progress line with its ETA, which stays in view after the
  step. "Make room at home" if home storage blocks it.
- **No escort after the bounty:** the first shipment no longer takes the
  Interceptor by default once the privateer is dead. A route waits until its
  escort is beside it and fuelled for the whole leg.
- **Signals open here** (Oct 20 2026, [signals.md](signals.md)): the first
  two appear near home as step 12 begins, for the Interceptor's captain to
  answer between charter steps (the silent probe's 1,500 Cr and a second
  captain).
- The stipend for the den ("Privateer den eliminated", 600 Cr) is paid on
  reaching this step; a home without a den skips it.

**Research, survey, Reach:** as v3. The Academy costs 25 Alloys, 15
semiconductors and 20 Provisions; the Survey Probe 6 Alloys and 4
semiconductors; the Mining Barge 40 Alloys, 20 Machinery, 12 Polymers and 6
semiconductors.

## Stipends

| Stage reached | Title | Stipend |
| --- | --- | ---: |
| Mine step | Colony fed and housed | 300 Cr |
| Raid step | Mine online | 300 Cr |
| Interceptor step | Shipyard staffed | 400 Cr |
| Privateer step | Interceptor launched | 300 Cr |
| Export step | Privateer den eliminated | 600 Cr (only with a den) |
| Expansion step | Research under way | 300 Cr |
| Survey step | Mining expanded | 400 Cr |
| Barge step | Nearby system charted | 500 Cr |
| Mining step | Mining Barge launched | 300 Cr |
| Haul step | Belt mining under way | 300 Cr |
| Complete | Founding charter fulfilled | 800 Cr |

Plus the 4,500 Cr privateer bounty: 9,000 Cr in all (`founding.rs`,
`charter_stipend`; `FoundingProgram::stipend` skips the den's without one).
v5 dropped v4's 500 Cr "Counterfeit claim settled". Before the Interceptor,
the start and the stipends leave the founder about 800 Cr short of buying back
the raid's haul; the ore sale makes it up.

## Opening build times (Oct 15 2026)

While the charter runs, level I of the Mining Complex, Bioharvester, Habitat,
Agroplex and Solar Array on the landing world (the home system's Command
Center world) builds in 10 seconds, whatever the world's construction speed
(`founding::OPENING_BUILD_SECS`, applied when the order is received). Higher
levels, other worlds and every build after the charter keep their normal
times. The client mirrors it (`openingBuild` in `core/derive/market.ts`), so
the build sheet, the queue and mobile quote 10 s.

## Semiconductors

Electronics are shown as **Semiconductors** everywhere, and the Electronics
Fabricator as the **Semiconductor Fab**. Display only: the slug stays
`electronics` in saves, commands and the wire.

## The guide

`foundingPlan` (`client/src/core/derive/founding.ts`) shows the chapter's
checklist (the server's own wording, `tutorial::Step::label`), what is under
way with its time left, and one next step. No spoilers (Oct 15 2026): a
twist is news only once it is told, and a twist step stays out of the
checklist until then even when it is the current step. Until command sees the
raid, step 5 reads "The mine is running · Ferrite Ore is coming in." and the
chapter is called "Industry"; then "Raider inbound" (with its ETA) and "Raider
over the colony". "Sell ore to the trader" stays out until the trader's pop-up
has shown. The shield tooltip names pirates only after the raid's report. Its
build buttons go through `foundingBuildTarget`, which puts every build on the
landing world while the land grant is closed. When the trader is docked and a
build is short, the step offers its stall first, with the shortfall filled in.

**Long-term goal** (Peter, Oct 16 2026): once the den falls, steps 12–14 carry
a quieter card under the next step: "Survey the belt at {den} · The den
guarded a belt that looks rich in minerals." with **Build Survey Probe**, or
**Send the survey** when a probe or Scout is free (`denBeltGoal`). It goes
once the survey is under way (a progress line takes over) or the belt is
charted; a survey sent early completes step 15 the moment it is reached. The
den's clearance line now reads "Privateer den at {den} destroyed. Its
asteroid belt looks rich in minerals."

Actions: `build-bioharvester`, `build-habitat`, `build-agroplex`,
`build-power`, `build-shipyard-2`, `build-interceptor`,
`build-expansion-power`, `build-expansion-mine`, the staffing actions
`bioharvester`, `agroplex`, `mine` and `expansion-mine`, `home-world`,
`privateer` (select it), `den` (open the den's system), the export's
`build-convoy`, `opening-imports` (Logistics) and `shipment-trip`, and the
trader's `merchant-{build}`, `merchant-ore` (sell ore), `merchant-fuel`
(Deuterium for the Interceptor) and `merchant-ahead` (the last call's
stock-up). One-click orders: `assign-captain`, `attack-privateer`
and `assault-den` (the Interceptor to the den), plus `survey-stop`,
`mine-belt`, `barge-home` and `barge-unload` as v3.

## Retired stages

Old saves are not migrated: start a fresh galaxy. The retired enum variants
remain so serialized stages still parse, and each hands on to its successor:

- v4's `BuySemiconductors` goes to `PowerShipyard`.
- v4's `BuildFreighter` and `BuildConvoy` go to `BuildInterceptor`.
- `PrepareExport` is step 12 again; `LocalRecovery` and `ExportProduction`
  go to it.
- `CompleteExport`, `GrowBusiness` and `BuildSecondFreighter` go to
  `BuildShipyard`.
- `BuildAcademy` goes to `FirstResearch`.
- `BuildScout` goes to `SurveyCandidates`.
- `BuildColony` and `EstablishColony` go to `Complete`.

The v3 lost-Freighter replacement is gone: there is no starting Freighter to
insure.

## Bots, funnel and harness

- **Bots** play the same charter (`bots/opening.rs`): the farms, the Habitat
  and the mine; then nothing from the kit until the raid has struck and the
  ore is sold (`raid_pending`); at step 6 they sell the trader all the ore it
  will take (`charter_ore_sale`); then the Solar Array, the Shipyard,
  Shipyard II and the Interceptor, buying semiconductors and Armaments from
  the trader and selling it ore toward any shortfall, and no Hub imports of
  goods the docked trader sells. Their Freighter waits for the privateer's
  bounty. At step 10 the Interceptor attacks the tracked privateer
  (`charter_privateer`); short of Deuterium, it buys some from the trader. At
  the den step it flies to the den (`charter_den`) and fights to the end,
  while the bounty buys from the trader's last call what the builds ahead
  lack, plus the Interceptor's Deuterium (`charter_stock_up`). From then on
  their optional works (storage) leave those builds their goods
  (`charter_reserve`). The export is their ore-first Freighter route to the
  Hub. During Landfall they import no food: the kit lasts until the farms
  run, and their food buffer counts the farms' own output (`food_grown`). In
  the end-to-end test (Oct 17 2026) a bot is raided at 2 game-minutes, sells
  its first ore at 3.4, launches its Interceptor at 17.7 (254 ore sold to the
  trader), destroys the privateer by 19.9 and the den by 21.3, exports at
  21.8 and expands by 29.4 (27.7 while the trader stayed; its home's exotic
  node eats the Deuterium it bought, which holds its development about
  6 minutes).
- **Admin funnel** (`crates/server/src/tutorial.rs`): sign up, enter, found,
  then the 18 steps. Keys: `bioharvester`, `habitat`, `agroplex`, `mine`,
  `raid`, `sell_ore`, `power`, `shipyard`, `interceptor`, `privateer`, `den`,
  `shipment` ("Export your ore"), `research`, `expansion`, `survey`, `barge`,
  `belt_mining`, `haul`. Records under v4's `semiconductors` and `freighter`
  keys are skipped.
- **Harness:** not yet run on v5.
- **Exotic node at home** (found Oct 16 2026, not fixed): a home whose star is
  exotic holds a node; when nodes wake at 180 s its upkeep (4 Provisions and
  2 Deuterium a second, `NODE_UPKEEP_PER_SEC`) eats the home's 10 Deuterium in
  5 s. Such a founder's Interceptor launches with an empty tank; the guide's
  "Fuel the Interceptor" covers it.
