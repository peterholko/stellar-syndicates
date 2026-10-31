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

**Oct 30 2026, §gun-platform (protocol v78, `--reset-galaxy`):** step 6 is
now "Build a Gun Platform" (see **The Gun Platform**), and losing the uncle's
finale is game over with a restart
([pirate-vendetta.md](pirate-vendetta.md), "The finale and the fall").

**Oct 31 2026, §charter-crates (protocol v79, `--reset-galaxy`):** the kit
became six **Charter crates** released a chapter at a time, and step 22 is
"Secure your stores" (a Warehouse and a Vault), so the charter has 23 steps
and the uncle is step 23.

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
  for step 16.
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

**Charter crates** (Peter, Oct 31 2026: "the player starts with everything
they need to complete the tutorial which I think is wrong because their
storage is basically full at the very beginning"). The charter's supplies
come as six crates (`founding::CharterCrate`), one per chapter. The Authority
holds them sealed, and a sealed crate takes no storage. Each is released at
its chapter's first build and collected from the guide card ("Charter crate ·
95 Alloys · 42 Machinery [Collect]", `ClientMsg::CollectCharterCrate`).

| Crate | Released at | Holds | For |
| --- | --- | --- | --- |
| Landfall | landing (open in the holds) | 110 Alloys, 47 Machinery, 20 Polymers, 100 Provisions, 40 Semiconductors, 15 Armaments | Bioharvester, Habitat, Agroplex, Mining Complex; food until the farms run (`KIT_PROVISIONS`); the ship parts the raid takes (`KIT_SEMICONDUCTORS`, `KIT_ARMAMENTS`) |
| Raid | 6. Build a Gun Platform | 95 Alloys, 42 Machinery | Gun Platform, Solar Array, Shipyard |
| Pirates | 10. Build an Interceptor | 30 Alloys, 2 Machinery, 1 Warp Core | the Interceptor and its first repairs |
| Expansion | 13. Export your ore | 176 Alloys, 47 Machinery, 8 Polymers, 20 Provisions, 100 Uranium, 3 Warp Cores | Tiny Freighter, Academy, Smelter, the second mine and its Solar Array, Survey Probe, and two Interceptors for the uncle (his oath comes at step 17); the Uranium fills the freighter, the probe and most of the barge |
| Reach | 18. Build a Mining Barge | 60 Alloys, 28 Machinery, 12 Polymers, 1 Warp Core | Mining Barge, Uranium Mill |
| Vendetta | 22. Secure your stores | 70 Alloys, 18 Machinery | Warehouse, Vault |

- **At landing** the holds carry the Landfall crate and the home's fuel seeds
  (20 Uranium, 10 Deuterium): about 360 of the 1,000 units, where the old kit
  filled 938. The mine has room for about 28 minutes of ore.
- **A crate always opens.** Collected, it lands in the home stockpile whole,
  past the storage cap if it must; production then waits for room. So a crate
  never locks away the goods for the Warehouse that would make room for it.
- **What they don't carry:** the Semiconductors and Armaments of later builds,
  bought back from the trader and the Market Hub as before. The trader's
  stock-up (`merchantShortfall`) and the bots' (`charter_stock_up`) count the
  goods still sealed, so nobody buys what a crate will bring.
- **Bots** collect a crate as it is released. A finished charter opens any
  crate still sealed.
- **Against the old kit:** 106 more Alloys (the Warehouse and Vault's 70, 26
  the late Interceptors used to take from the Smelter, 10 for repairs), 11
  more Machinery and the Academy's 20 Provisions on top of the 100.
- `each_crate_carries_its_builds` (sim) keeps every crate equal to its
  builds' recipes.

## Twenty-three steps in six chapters

| Chapter | Step | Completes when (server facts) |
| --- | --- | --- |
| Landfall | 1. Harvest biomass | A Bioharvester is built and staffed. |
| | 2. House the settlers | A Habitat is built. |
| | 3. Make provisions | An Agroplex is built and staffed. |
| | 4. Bring the mine online | The home's site report shows a staffed Mining Complex. |
| Raid | 5. Pirates raid the colony | The report of what the raid took reaches command (see **The raid**). |
| | 6. Build a Gun Platform | A station of any level stands over the home (see **The Gun Platform**). |
| | 7. Sell ore to the trader | Any Ferrite Ore sold to the free trader, or the trader has gone. |
| | 8. Power the Shipyard | A Solar Array is built at home. |
| | 9. Build a Shipyard | A Shipyard is built and staffed. |
| Pirates | 10. Build an Interceptor | The corporation owns an Interceptor and a captain commands it (or no captain is free). |
| | 11. Destroy the privateer | The privateer's defeat is reported and the 8,300 Cr bounty paid. |
| | 12. Eliminate the privateer den | The report that the den fell reaches command. |
| Expansion | 13. Export your ore | Word that ore is on its way to the Market Hub reaches command (see **The export**). |
| | 14. Research Enrichment | Enrichment is running or finished and an Academy is staffed. |
| | 15. Build a Smelter | A Smelter is built and staffed at home (once Enrichment is in). |
| | 16. Expand mining operations | Another world of the home system runs a staffed Mining Complex beside its own power plant. |
| | 17. Chart a nearby system | A survey report of the den's system arrives. |
| Reach | 18. Build a Mining Barge | A barge exists. |
| | 19. Prospect the belt | The report of the barge's find reaches command (a belt hiding no den pocket skips it). |
| | 20. Mine the belt | The report that a barge is cutting ore at a belt away from home reaches command. |
| | 21. Bring the ore home | The report that a barge unloaded or delivered ore arrives. |
| Vendetta | 22. Secure your stores | A Warehouse and a Vault stand at home (see **Secure your stores**). |
| | 23. Defeat the privateer's uncle | The report of how it ended reaches command ([pirate-vendetta.md](pirate-vendetta.md)). His oath comes at step 17, and he strikes a freighter or the second mine, then the barge, before this ("The campaign"). If his raid leaves nothing fighting for the colony it falls: game over, and the founder restarts. |

**Why expand, why the barge** (Peter, Oct 30 2026: Expansion and Reach "need
to be justified better to the player"):
- **Refining (steps 14–16).** Step 14 is Enrichment
  (`founding::CHARTER_RESEARCH`), because it unlocks the Smelter. The
  Authority's grant funds only that programme during the charter, and it now
  leaves 3 minutes of work (`FIRST_RESEARCH_REMAINING_S`, was 12).
  - Step 15 builds a Smelter, Ferrite Ore into Alloys, on the landing world's
    free industrial site. It draws 2 MW of the 9 the landing world has.
  - A staffed Smelter eats more ore than one mine makes, so the stores bar
    shows ore falling. Step 16's cards say "The Smelter is short of ore." once
    it is starved, and the second mine answers it.
  - The Smelter's crew is a seventh post, so founders land with 7,000 people
    (`HOME_FOUNDING_POP`, was 6,000).
- **The den's pocket (steps 18–20).** Every den belt hides a Titanium Ore
  pocket (`founding::den_pocket`, seeded in `seed_belt_pockets` as every home
  belt's Cuprite pocket is).
  - It holds 600 units at 110 Cr a unit against Ferrite's 8, about 66,000 Cr
    at base price, near the home pocket's worth.
  - The den's loot carries 10 Titanium Ore (`DEN_PLUNDER`), the hook.
  - Step 19's prospecting pass finds a den's pocket for certain; other
    pockets stay at 50% a pass. Step 20 then mines it (`MineBelt` with
    `pocket`).
  - A barge cuts Titanium at 4 units a minute, about 440 Cr a minute against
    Ferrite's 120.
  - The pocket is generated with the galaxy, so it needs `--reset-galaxy`.

Steps 1–11 run in `world/opening.rs` (`advance_local_opening`), the raid in
`world/founding_raid.rs`, the rest in `advance_founding_programs`. The export
is noted by `note_founding_export` (`world/opening.rs`).

**Chapter openings** (Peter, Oct 29 2026: "event art for each chapter of the
tutorial"). Landfall opens with the prologue's landing, the Raid with the raid
pop-up and the Vendetta with the uncle's arrival. Pirates, Expansion and
Reach now open with a pop-up of their own (`chapterEvent` in
`client/src/story-events.ts`), on the milestone line that starts them; its
toast still shows:

| Chapter | Milestone line | Pop-up | Painting |
| --- | --- | --- | --- |
| Pirates (step 10) | "Shipyard staffed" | Strike back · "The Shipyard stands. {Nephew} is still out there." | `chapter-pirates` |
| Expansion (step 13) | "Privateer den eliminated" | Lanes open · "The den is gone. The lanes to the Market Hub are clear." | `chapter-expansion` |
| Reach (step 17) | "Nearby system charted" | Into the belt · "The survey is in: the den's old belt is rich in ore." | `chapter-reach` |

The paintings landed Oct 30 2026 (prompts in `docs/chapter-art-prompts.md`).
If one fails to load, the chapter's hull stands in (Interceptor, Tiny
Freighter, Mining Barge).

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
Since Oct 25 2026, when Shipyard I builds the Interceptor, only about 11 ore
of that sale is strictly needed (see "The money" below).

**Charter gates** (Oct 26 2026, Peter: "have the tutorial gate the
construction of the structures")
- A founder can't build a structure the charter builds before the step that
  opens it (`founding::charter_step`, `FoundingProgram::allows`):

  | Structure | Opens at step |
  |---|---|
  | Bioharvester | 1 |
  | Habitat | 2 |
  | Agroplex | 3 |
  | Mining Complex | 4 |
  | Solar Array (Power Plant) | 7 |
  | Shipyard | 8 |
  | Academy | 13 |
  | Asteroid Quarry | 14 |
- The build check refuses an early order: "The charter opens this at step N".
- `FoundingView.charter_gates` lists the structures still closed. The build
  sheet shows them locked: "Charter step N".
- Bots skip them (`Observation::charter_allows`).
- Structures the charter doesn't build aren't gated; most wait on research
  anyway.
- Why: a smart player used the kit's 40 Semiconductors and 15 Armaments to
  build a Solar Array, Shipyard and Interceptor before the mine, so the raid
  found almost nothing and met an Interceptor.

**The raid** (step 5, `world/founding_raid.rs`)
- **Launch** (Oct 26 2026, Peter): the moment the charter's mine starts
  building (step 4), the Rogue Privateer sets out from beyond the colony's
  sensors on its den's side. Its den rises with it (`ensure_founding_den`).
  - It starts where its flight takes `founding::CHARTER_MINE_SECS` (30 s):
    drive spool, then warp to the well edge. That is about 1.23 sensor ranges
    out, never past the den.
  - The mine is built in those 30 s, and finished on the privateer's arrival
    in any case (`raid_on_mine_start`, `finish_charter_mine`), so it comes
    online as the raider arrives.
  - The guide's step 4 shows "Raider closing on {home} · ETA" once command
    sees it, and the build sheet shows the mine at 30 s.
  - A home without a den sends it from the dark side, away from the Hub.
  - The old launch, at step 5 when the mine comes online, remains as a
    fallback (`RAID_START_SENSOR_RANGES`, 1.25 sensor ranges).
- **Seen:** the first time the Command Center or a Sensor Array detects it,
  by the shared rule, stamped on its light home (`raid_seen_at`). From then
  on the colony tracks it wherever it goes (`view::tracked_contacts`).
- **Focus** (Oct 26 2026, Peter): the sighting takes the player to the
  privateer, on desktop and mobile alike.
  - The view leaves any planet or system view for the galaxy map, selects the
    privateer and opens its panel (its sheet on mobile).
  - The camera follows it (`Renderer.follow`) until the player drags the map,
    resets the view or selects something else. Zooming keeps the follow.
  - Only a sighting that happens while the page is open triggers it
    (`noticeRaid` in `boot.ts`); a battle view keeps the player.
  - The guide's raider ETA is measured to the well edge, where the raid
    begins.
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
  then lurks just outside the colony's sensors: 1.1 Command Center sensor
  ranges out (6,600 su) on its den's side (`RAID_LURK_SENSOR_RANGES`).
- **The hunt (Oct 27 2026,** Peter: "show up as a last known contact or
  something and the player moves their interceptor there and then the pirate
  appears"; "maybe the pirate should move just outside of the player's home
  system sensor range"**):**
  - The colony tracks it only during the raid: from the first sighting until
    the raid's report reaches command (`view::tracked_contacts`).
  - After that, its hiding spot shows as an amber "Last seen" marker
    (`FoundingView.privateer_last_seen`) until its kill is reported, hidden
    while the privateer itself is in view.
  - The guide's "Find the privateer" sends the Interceptor there (Hunt). Lying
    quiet, the privateer shows to a 6,000 su sensor only within about 1,300
    su, so the Interceptor finds it on arrival, and the step turns to Attack.
- Persisted (`FoundingProgram::raid`): a reload neither repeats the raid nor
  refills what it may take.

**The trader and the ore** (step 7)
- The free trader is called when the raid's report arrives and docks about
  40 s later ([traveling-merchant.md](traveling-merchant.md)). It now sells
  Armaments too (15), and takes up to 300 ore (`ORE_CAP`, was 150).
- The guide: "Sell ore to the trader · We need semiconductors for our ships.
  [Sell]" (Peter's line, "We'd better sell ore to the trader to buy
  semiconductors for our ships. [Sell ore]", trimmed Oct 27 2026 because it
  repeated the title), which opens the stall on its Buying tab with all the
  ore it will take filled in (`merchant-ore`). Until the trader's own pop-up has shown, the card is
  "Find semiconductors · The Solar Array needs 8" at the Market.
- **The money:** 2,000 Cr start + 300 (mine) + 300 (raid) + 400 (Shipyard
  staffed) = 3,000 Cr, against about 3,045 Cr to buy back 35 semiconductors
  (Solar Array 8, Shipyard 15, Interceptor 12) and 15 Armaments at the
  trader's markup. That leaves about 45 Cr short, about 11 ore at its 4 Cr
  (half the Hub's 8). The mine makes 23 ore a minute.
  - Before Oct 25 2026 the buyback also covered Shipyard II's 15
    semiconductors: 3,810 Cr, about 810 short, about 200 ore.
  - The stipends were left as they were. Founders now finish the opening
    about 765 Cr richer, and the guide still sells all the ore the trader
    will take.
- The step also completes if the trader leaves without a sale, so it is never
  a dead end.

**The Interceptor** (step 10): Shipyard I builds it (Peter, Oct 25 2026;
it needed Shipyard II before). The step goes straight to "Build an
Interceptor". The Interceptor costs 20 Alloys, 12 semiconductors, 15
Armaments and a Warp Core. Corvettes still need Shipyard II.
- Its captain isn't optional (Peter, Oct 30 2026). The step completes once a
  captain commands the Interceptor, or when no captain is free.
- Once it's built, the card is "Assign a captain": the founding captain
  waits at home, and `AssignCaptain` puts them aboard the Interceptor docked
  there. A captain boards only a ship at rest, so an Interceptor away from
  home gets "Bring the Interceptor home", and one on Guard or Defend duty
  gets "Stand the Interceptor down" (both `interceptor-captain`). There is
  no skip.
- Bots assign it too (`officers`). Their escort rule (`guards`) keeps the
  Interceptor at rest until its captain is aboard.

**The privateer** (step 11)
- A replacement Interceptor takes the reserve captain first, with no skip.
  The kill completes the step, captain or not.
- Then a one-click attack (`AttackFleet`) on the lurking privateer, tracked
  since the raid. A docked Interceptor with an empty tank gets "Fuel the
  Interceptor · Buy Deuterium" from the trader first (`merchant-fuel`).
- The kill promotes the captain aboard: their experience is topped up to
  level 2's 100 (`PRIVATEER_PROMOTION_LEVEL`; the kill alone earns 85). Any of
  the founder's fleets in the fight counts.

**The den** (step 12)
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
- **Repairs** (Oct 21 2026): docked at home, the Interceptor repairs at the
  Shipyard that built it. A staffed Shipyard now repairs as a staffed Ordnance
  Foundry does (`REPAIR_YARDS`): Alloys and Machinery from the system, and the
  faster yard sets the rate. Before, only a Foundry did, and the charter
  builds none.
- **Revealed** by the privateer's log: an intel snapshot taken where and when
  it died, so the den reaches command with the kill report (`☠ PRIVATEER DEN`
  on the map, "Privateer den" in the inbox).
- **Cleared** by the ordinary pirate assault: an idle combatant within 220 su
  of the den system fights the garrison. Done when the clearance report
  reaches command (`den_report_at`). Its stores, 40 Alloys and 15
  semiconductors plus everything the raid took, are offered back as salvage
  contracts (`offer_site_plunder`), to recover with any hold.
- **Then:** the den system is the survey stop (step 17) and its belt the
  barge's (steps 18–21); a den cleared for good no longer bars a belt. Its
  stores wait for a Freighter; the charter does not send one.
- A home without a den system (an overflow home) skips the step.

**The land grant** (until step 16)
- During the charter, structures in the home system can only go on the landing
  world. Anywhere else the sim refuses the order: "Charter land grant: {world}
  only, until you expand mining" (`FoundingProgram::land_grant_open`).
- The build sheet shows other worlds' structures locked with the same reason.
- Ships and the Survey Probe are unaffected; they build at the yard.

**Expanding the mine** (step 16): a Solar Array on another home world with ore
first (a separate grid with no free power), then a Mining Complex beside it,
staffed. Planet III is the usual one; the guide picks a planet before a belt.

**The trader's last call** (Peter, Oct 16 2026: "Teo should leave while the
player is eliminating the pirates")
- Once the privateer's kill is reported, with its bounty, the trader stays
  3 minutes more (`merchant::LAST_CALL_S`); the den's fall cuts that short,
  so it is gone by step 13. The stall's clock shows the time left.
- After it, only the Market Hub sells what the kit no longer covers. The raid
  took the kit's semiconductors, and the bounty buys back what comes next:
  - 33 for the builds after the den: Academy 15, Solar Array 8, Survey
    Probe 4, Mining Barge 6.
  - 20 more for research (`merchant::RESEARCH_RESERVE`, Oct 26 2026). A
    running programme draws semiconductors from home stock, about 0.6 a
    minute at tier I. Before the reserve, research ate the barge's last
    semiconductors once Teo had gone (Peter: "buy a stockpile of
    semiconductors before Teo leaves").
  - The second Interceptor the uncle's vendetta needs (Oct 29 2026): its 12
    semiconductors and 15 Armaments. Before, the stock-up bought only
    semiconductors and the kit's Warp Cores ran out, so the step-19 card
    asked for a hull nothing could supply (Peter: "later on it asks to build
    another interceptor which requires armaments"). The kit now carries a
    fourth Warp Core, Teo carries 30 Armaments and 115 semiconductors, and
    the bounty rose from 4,500 to 6,400 Cr to pay for it.
- The guide's card reads "Stock up · trader leaves in m:ss · Then only the
  Market Hub sells them. · 65 Semiconductors, 15 Armaments for the builds and
  research ahead." with **Buy from trader** (`merchant-ahead`) and the den step's own
  button beside it (its title, e.g. **Destroy the den at Tarn**). A card that
  only waits (the Interceptor en route) gives way to it, its line staying
  under way. Its log and Command title stays "Stock up before the trader
  leaves".
- The prefill is the whole recipes of the Academy, the expansion's Solar Array
  and Mining Complex, the probe, the barge and the second Interceptor, plus the
  reserve, less what home holds, bounded by the trader's stock (`AHEAD` and
  `RESEARCH_RESERVE` in `core/derive/merchant.ts`). The bots buy the same but
  the Interceptor (`charter_goods_ahead`): they skip the vendetta.
- Teo's greeting during the last call: "Last call, friend! I'm off before the
  shooting starts." (after a deal: "Anything else? The clock's ticking.").
- **Why:** measured with a bot (Oct 16 2026). With the trader leaving at the
  bounty and no stock-up, research slipped from 21.7 to 46.2 game-minutes and
  mining had not expanded by minute 70: the chips came by Authority freight
  from the Hub.
- The timeline line on departure: "Teo Brannick's Long Odds left {home}."

**The export** (step 13, Peter, Oct 16 2026: "insert an export ore after
step 11")
- **Done when** word that ore is bound for the Market Hub reaches command
  (`export_dispatched_at`): a Freighter carrying Ferrite Ore past the home's
  gravity well on its way to the Hub (light-delayed from the ship), or an
  Authority freight booking of ore from home to the Hub warehouse.
- **The guide:** "Build a Tiny Freighter · 20 Alloys · 8 Machinery · 8
  Polymers · 1 Warp Core [Build]" (the kit covers it; the reason gives the
  Hub's ore price, the Uranium fuelling is on hover), then "Export your ore
  [Plan the shipment]", which opens Logistics on the first-shipment card: 50
  ore out, the Academy's goods back within a spending cap, one trip. Under
  way: "Shipment heading to the Market Hub · ETA" as the card's title, and as
  a progress line, which stays in view after the
  step. "Make room at home" if home storage blocks it.
- **No escort after the bounty:** the first shipment no longer takes the
  Interceptor by default once the privateer is dead. A route waits until its
  escort is beside it and fuelled for the whole leg.
- **Signals open here** (Oct 20 2026, [signals.md](signals.md)): the first
  two appear near home as step 13 begins, for the Interceptor's captain to
  answer between charter steps (the silent probe's 1,500 Cr and a second
  captain).
- The stipend for the den ("Privateer den eliminated", 600 Cr) is paid on
  reaching this step; a home without a den skips it.

**Research, survey, Reach:** as v3. The Academy costs 25 Alloys, 15
semiconductors and 20 Provisions; the Survey Probe 6 Alloys and 4
semiconductors; the Mining Barge 40 Alloys, 20 Machinery, 12 Polymers and 6
semiconductors.

## Secure your stores (step 22)

Peter (Oct 31 2026): "Have a step to increase storage at the end and at the
same time build the player's Vault."

- **Where:** after "Bring the ore home", before the uncle. The barge's
  Titanium has just come home, and his raid is next.
- **Done when** a Warehouse and a Vault stand at home
  (`FoundingStage::SecureStores`). A Warehouse built earlier through "Make
  room at home" counts. A home whose survey charted no belt reaches it
  straight from step 17.
- **The guide:** "Build a Warehouse · 30 Alloys · 8 Machinery" ("+700
  storage."), then "Build a Vault · 40 Alloys · 10 Machinery" ("Raiders can't
  reach it."), from the Vendetta crate. The Warehouse goes on a home world
  with a free site, the landing world last (`bestStoreBody`), so the landing
  world's last site stays free. The Vault goes under the Command Center.
- **The Vault** opens at this step (`charter_step`), not with the finished
  charter. It keeps the 200 most valuable units out of any raid, so it guards
  the Titanium from the uncle.
- **The finale with a Vault** (`vendetta_sizing_sweep`, 6 seeds, 0–4
  Interceptors under the Gun Platform, 120 Titanium Ore at home): every fight
  ends the same with and without the Vault. The fight decides it, not the
  loot.

## The Gun Platform (step 6)

Peter (Oct 30 2026): "introduce 1 step after the very first pirate raid to
build some very basic planetary defenses". He chose the orbital option.

- **What it is.** The station ladder's new first level
  (`station::LEVELS[0]`, so the ladder is now six levels: Gun Platform,
  Defense Platform, Orbital Battery, Defense Station, Citadel, Star Fortress).
  One hardpoint, 300 hull, attack weight 1.5 (an Interceptor has 200 hull and
  3). It costs 25 Alloys and 10 Machinery and goes up in 20 s.
  - It needs no research. Fire Control now unlocks level 2, and each later
    programme unlocks the level after the one it used to.
  - The kit carries its cost, since the raid takes only the semiconductors and
    Armaments.
  - It later rises into the Defense Platform like any station level.
- **No site.** A station stands in orbit and takes no building site
  (`StructureKind::uses_slot`). Without that, the Gun Platform took the
  landing world's one free Infrastructure site and the Academy had nowhere
  to go.
- **Gated.** The charter opens it at step 6 (`charter_step`), so the first
  raid always meets an undefended home.
- **Pirates leave it alone.** Pirate packs now treat any owned system with a
  firing station like one with legacy platforms, and won't hunt targets
  within 1,300 su of it. The uncle's strikes avoid the home planet too; only
  his finale comes for it.
- **In the finale** its gun joins every fight in the home's well. With no
  warship near home, the uncle assaults the platform itself.
- **Art:** one sprite master, `client/art-src/orbital-defense-v1/gun-platform.png`
  (landed Oct 31 2026; brief in `client/art-src/defense-stations-v1/prompts.md`
  §4, exact prompts in `gun-platform.prompt.json` beside it).
  `build-station-art.mjs` derives every surface's sprite from it as level 1;
  station art moved to `stations/v2/level-1…6`.

## Stipends

| Stage reached | Title | Stipend |
| --- | --- | ---: |
| Mine step | Colony fed and housed | 300 Cr |
| Raid step | Mine online | 300 Cr |
| Interceptor step | Shipyard staffed | 400 Cr |
| Privateer step | Interceptor launched | 300 Cr |
| Export step | Privateer den eliminated | 600 Cr (only with a den) |
| Smelter step | Research under way | 300 Cr |
| Expansion step | Smelter running | 300 Cr |
| Survey step | Mining expanded | 400 Cr |
| Barge step | Nearby system charted | 500 Cr |
| Prospect step | Mining Barge launched | 300 Cr |
| Mining step | Pocket found | 300 Cr |
| Haul step | Belt mining under way | 300 Cr |
| Complete | Founding charter fulfilled | 800 Cr |

Plus the 6,400 Cr privateer bounty (4,500 before Oct 29 2026): 11,500 Cr in all (`founding.rs`,
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

Since Oct 26 2026, step 4's mine is the exception: it takes the privateer's
30 s flight (`CHARTER_MINE_SECS`, mirrored as `CHARTER_MINE_SECS` in
`market.ts`). See "The raid" above.

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
raid, step 5 reads "The mine is running" and the chapter is called
"Industry"; then "Raider closing on {home} · ETA" and "Raider over the
colony". "Sell ore to the trader" stays out until the trader's pop-up
has shown. The shield tooltip names pirates only after the raid's report. Its
build buttons go through `foundingBuildTarget`, which puts every build on the
landing world while the land grant is closed. When the trader is docked and a
build is short, the step offers its stall first, with the shortfall filled in.

**Copy** (Peter, Oct 27 2026: "reduce the repetitive text in the tutorial
steps"). One line per fact:
- The button never repeats the title: "Build a Bioharvester" has **Build**,
  "Assign a captain" **Assign**. A title that doesn't name the thing keeps the
  whole label ("Power Freya III" has **Build Solar Array**).
- The copy line is figures (the recipe, a count, "Ada Voss → Interceptor") or
  nothing. No "Next" eyebrow; a blocked card keeps "Needs attention".
- Why goes on hover over the title (`FoundingStep.hint`, desktop only): "An
  unstaffed building does nothing.", the 1 MW reactor, the land grant, the
  report that travels at light speed. The visible `reason` is kept for rewards
  and obstacles: the bounties, the den's loot, the Hub's ore price, full
  storage, a lost barge.
- A card that only waits takes its progress line, ETA and all, as its title
  (`FoundingStep.status`, made by `waitOn`), and the line leaves the under-way
  list: "Bioharvester under construction · 9s · Then staff it. [Open Sirius
  I]". The Log, Command and the welcome-back keep the plain title.
- Choices (the trader's parts, the last call) are buttons that say the whole
  step ("Buy from trader", "Build a Solar Array") under the card's own copy.
- While-you-wait items show their titles. A second Interceptor already under
  construction shows only as its progress line.
- The Log's Inbox keeps side rows (nodes, survey reports, idle stockpiles,
  invites…) out until the privateer's bounty (`CHARTER_QUIET`, docs/log.md).

**The Founding Camp waits** (Peter, Oct 28 2026: "Settlement Projects to be
unlocked after the tutorial was complete"). Its four projects (prospect,
surface workings, storehouse, quarters) shared the home world's build slot
and starting kit with the charter's own steps. While the charter runs:
- no "Founding Camp · N/4 developments" card on the home world (desktop and
  phone; `campOpen` in `core/derive/founding.ts`), so no way into the projects;
- no "Develop your settlement" card under the guide and no coach marks;
- "Make room at home" (storage full while the Freighter is out) builds a
  Warehouse (`build-warehouse`) instead of opening the camp.

Once the charter is complete the card appears, and "Your next chapter" offers
"Develop your settlement" (`postVictoryHandoff`, goal `camp`) until all four
are done. Client only; the sim still accepts the orders.

**Open your home system** (Peter, Oct 21 2026): before step 1 the guide asks
the founder to click their home system ("Click {home} on the map.", "Tap" on
mobile, with an **Open {home}** button). Either flies the camera into the star
at an even zoom, then hands off into its System View as the wheel does at the
doorway (`Renderer.flyIntoSystem`); a pan, a zoom or Esc takes over, and Back
returns to where the flight began. It is a client step, not one of the
server's eighteen, so the funnel doesn't count it: Landfall reads 0/5 until it
is done. It is done once the session has opened the home System View (by any
path), or once anything is built or queued at home, so a reload past it never
asks again (`homeStepPending`). Only while it is the current step does a
click on the home star fly in; otherwise a click selects, as before.

**Assign a captain** (Peter, Oct 21 2026: "there should be a Panel not just a
small confirm"). The guide's **Assign captain** opens a panel, in both shells,
that introduces the reserve captain: their portrait at its age, name, rank and
specialty, a line or two of background, the four attributes with what their
points do (the specialty outlined), and level and authority. **Assign to
Interceptor** sends the order through the usual path; **Not now** sends
nothing. The backgrounds and specialties of all 21 captains are in
`client/src/core/derive/captain-lore.ts` (the specialties mirror
`Captain::founding`), and the panel is `client/src/captain-intro.ts`.

**No long-term goal card** (Peter, Oct 29 2026: "Remove this"). From Oct 16
2026 steps 12–14 carried a dashed "Long-term goal" card, "Survey the belt at
{den} · Looks rich in minerals." (`denBeltGoal`); it is gone from both
shells, and step 15 alone sends the survey. A survey sent early still
completes step 15 the moment it is reached. The den's clearance line still
reads "Privateer den at {den} destroyed. Its asteroid belt looks rich in
minerals."

Actions: `open-home` (fly into the home system), `build-bioharvester`, `build-habitat`, `build-agroplex`,
`build-power`, `build-shipyard-2`, `build-interceptor`,
`build-expansion-power`, `build-expansion-mine`, the staffing actions
`bioharvester`, `agroplex`, `mine` and `expansion-mine`, `home-world`,
`privateer` (select it), `den` (open the den's system), the export's
`build-convoy`, `opening-imports` (Logistics) and `shipment-trip`, and the
trader's `merchant-{build}`, `merchant-ore` (sell ore), `merchant-fuel`
(Deuterium for the Interceptor) and `merchant-ahead` (the last call's
stock-up). One-click orders: `assign-captain` (since Oct 21 2026 it opens the
captain's introduction panel first, see **Assign a captain** below), `attack-privateer`
and `assault-den` (the Interceptor to the den), plus `survey-stop`,
`mine-belt`, `barge-home` and `barge-unload` as v3.

## Retired stages

Old saves are not migrated: start a fresh galaxy. The retired enum variants
remain so serialized stages still parse, and each hands on to its successor:

- v4's `BuySemiconductors` goes to `PowerShipyard`.
- v4's `BuildFreighter` and `BuildConvoy` go to `BuildInterceptor`.
- `PrepareExport` is step 13 again; `LocalRecovery` and `ExportProduction`
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
  ore is sold (`raid_pending`), except the Gun Platform at step 6
  (`gun_platform`); at step 7 they sell the trader all the ore it
  will take (`charter_ore_sale`); then the Solar Array, the Shipyard and
  the Interceptor, buying semiconductors and Armaments from
  the trader and selling it ore toward any shortfall, and no Hub imports of
  goods the docked trader sells. Their Freighter waits for the privateer's
  bounty. At step 11 the Interceptor attacks the tracked privateer
  (`charter_privateer`); short of Deuterium, it buys some from the trader. At
  the den step it flies to the den (`charter_den`) and fights to the end,
  while the bounty buys from the trader's last call what the builds ahead
  lack, plus the Interceptor's Deuterium (`charter_stock_up`). From then on
  their optional works (storage) leave those builds their goods
  (`charter_reserve`). The export is their ore-first Freighter route to the
  Hub.
  - §refining and §den-pocket (Oct 30 2026): at step 14 they research
    Enrichment first (`next_research`), and they build and staff the Smelter
    at step 15. Their barge prospects the belt before it mines, then mines
    the pocket it found (`belt_lesson`).
  - During the charter they recruit no captains and train no specialists:
    the kit's semiconductors are the charter's. A recruit for the Tiny
    Freighter once ate the expansion's Solar Array's.
  - In the end-to-end test a bot researches Enrichment at 13 minutes, runs
    its Smelter at 16 and expands at 17.6 (15.2 before).
- During Landfall bots import no food: the kit lasts until the farms
  run, and their food buffer counts the farms' own output (`food_grown`). In
  the end-to-end test (Oct 17 2026) a bot is raided at 2 game-minutes, sells
  its first ore at 3.4, launches its Interceptor at 17.7 (254 ore sold to the
  trader), destroys the privateer by 19.9 and the den by 21.3, exports at
  21.8 and expands by 29.4 (27.7 while the trader stayed; its home's exotic
  node eats the Deuterium it bought, which holds its development about
  6 minutes).
- **Admin funnel** (`crates/server/src/tutorial.rs`): sign up, enter, found,
  then the 21 steps. Keys: `bioharvester`, `habitat`, `agroplex`, `mine`,
  `raid`, `sell_ore`, `power`, `shipyard`, `interceptor`, `privateer`, `den`,
  `shipment` ("Export your ore"), `research` ("Research Enrichment"),
  `smelter`, `expansion`, `survey`, `barge`, `prospect`, `belt_mining`,
  `haul`, `vendetta`. Records under v4's `semiconductors` and `freighter`
  keys are skipped.
- **Harness:** not yet run on v5.
- **Exotic node at home** (found Oct 16 2026, not fixed): a home whose star is
  exotic holds a node; when nodes wake at 180 s its upkeep (4 Provisions and
  2 Deuterium a second, `NODE_UPKEEP_PER_SEC`) eats the home's 10 Deuterium in
  5 s. Such a founder's Interceptor launches with an empty tank; the guide's
  "Fuel the Interceptor" covers it.
