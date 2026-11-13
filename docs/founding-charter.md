# Founding charter v6

**Nov 11 2026, §sell-alloys (no reset):** step 9 is "Sell Alloys to the
trader" (Peter: "we should just change the sell ore to sell alloy"). The
trader buys Alloys, not Ferrite Ore: up to 10 a visit at half the Hub
(`merchant::BUYS`, `BUY_CAP`, `BUY_PRICE_SHARE`), and any sale completes the
step (`FoundingProgram::alloys_sold`). The Smelter's 50-ore floor for the old
ore sale (`CHARTER_ORE_RESERVE`) is gone: it held a home with less ore than
that at no Alloys, which stalled a repair at step 13. The Landfall crate
carries the trader's 10 Alloys on top of its builds' (245), so the most it
takes never costs the Shipyard. The stage keeps its id (`sell_ore`) for saves
and the admin funnel. Bot charter run: Alloys sold 2.9 min, den 6.2, export
10.9, mining expanded 27.2.

**Nov 8 2026, §habitat-first (`--reset-galaxy`):** Landfall is ordered
around power. Step 1 is the Habitat, which draws nothing: the colony ship's
reactor keeps its people alive. Step 2 is the Solar Array ("Light the
colony"). Then come the Bioharvester (3) and the Agroplex (4), which now draw
power like a mine (one tier-throughput each, 1 MW at level I;
`power::powered`), the mine (5) and the Smelter (6). One Solar Array I (8 MW)
runs the landing world through the charter: farms 2 + mine 1 + Smelter 2 +
Shipyard 1 + Academy 1 = 7 MW, so nothing browns out in the opening, and the
second mine still stands beside its own plant. The crate's 100 Provisions
last about six minutes for 7,000 settlers; the two builds ahead of the food
chain take 10 s each. The legacy test sandbox's free base went from 3 to 6
MW (`LEGACY_BASE_MW`) so its homes' farms keep their output. The landing
card now ends "House your settlers first." Still twenty-nine steps.
Then (Peter: "the first pirate raid should arrive after the smelter is
built"): the privateer sets out when step 7 opens, the Smelter built and
staffed, so it arrives about half a minute after "Smelter running".
§smelter-raid (Nov 9 2026, Peter: "start moving towards the player's system
when they immediately start building their smelter and then the warning &
watch occurs at the same distance"): it now sets out as the Smelter's order
goes in, from the same point, so it is seen at the same range about as the
Smelter stands (a 20 s build) and is over the colony some 30 s after. The mine
is an ordinary 10 s opening build again (`CHARTER_MINE_SECS`,
`raid_on_mine_start` and `finish_charter_mine` are gone), and step 7's card
reads "The Smelter is running" until command sees the raider. Charter bots
no longer buy at the Hub an ore their own mine makes (a 95-ore buy for the
Smelter, right after the trader's sale, had left the expansion without cash
for its fuel: 36.5 minutes). Bot charter run: raid 1.6 min, den 6.6, export
14.1, mining expanded 26.1.

**Nov 7 2026, §charter-v6 (`--reset-galaxy`):** the goal is spoken first and
the raid breaks it. The landing card and the mine's step say the charter —
mine Ferrite, smelt Alloys, ship them to the Market Hub — and that the holds
carry the first freighter's engines. The Command Center's reactor runs the
colony and nothing on the grid (`power::COMMAND_CENTER_MW` = 0), so step 4 is
the Solar Array, step 5 the mine, step 6 the Smelter ("Smelt your first
Alloys"), and the raid at step 7 takes the six reactors and seven drives
those engines were. The Landfall crate carries the Solar Array's 8
Semiconductors on top of the raid's 20 (`PLANT_SEMICONDUCTORS`). After the
den the freighter step reads "Your engines are back", and step 18 is "Export
your Alloys": the Smelter's, the charter's promise (the Smelter still keeps
50 ore unsmelted, now for the trader's sale). The bot harness reaches
"Mining expanded" at 25 minutes. Still twenty-nine steps.

Status: v4 built Oct 14 2026, the privateer den added Oct 15; v5 (Oct 16
2026) replaced the counterfeit semiconductors and the ambushed Freighter
export with a pirate raid on the colony. Later on Oct 16, Peter's asks added
"Export your ore" after the den (step 12) and had the trader leave during the
den hunt (**The trader's last call**); neither needs a reset. Nov 5 2026: the
den pays through an Authority contract and the trader stays for the whole
charter (**The den's contract**, **The trader stays for the charter**).
Uncommitted.
Protocol v53
(`stellar.msgpack.v53`): deploy the client and server together, and start a
fresh galaxy (`--reset-galaxy`), because the landing, the home ore, the kit
and the den system are set when the galaxy is made or a corporation joins.
The v4 design and its decisions are in
[founding-charter-v4.md](founding-charter-v4.md); v5 changes only its
middle chapters (see **Why the raid**).

**Oct 30 2026, §gun-platform (protocol v78, `--reset-galaxy`):** step 6 is
now "Build a Gun Platform" (see **The Laser Battery**), and losing the uncle's
finale is game over with a restart
([pirate-vendetta.md](pirate-vendetta.md), "The finale and the fall").

**Oct 31 2026, §charter-crates (protocol v79, `--reset-galaxy`):** the kit
became six **Charter crates** released a chapter at a time, and step 22 is
"Secure your stores" (a Warehouse and a Vault), so the charter has 23 steps
and the uncle is step 23.

**Nov 2–3 2026, §stash (protocol v83, `--reset-galaxy`):** the den's
aftermath is three steps (Peter, Nov 3: "split it for this hauling
exercise"): 13 Repair the Interceptor (the den's dying blast mauled it), 14
Build a Tiny Freighter, 15 Haul back the den's loot (what the raid took, one
hold exactly). See **The den's aftermath**. The charter has 26 steps;
everything after the den moves down three.

**Nov 4 2026 (protocol v84, `--reset-galaxy`):** no crate after the raid
(Peter: "it doesn't make sense"); its 95 Alloys and 42 Machinery land with the
colony in the Landfall crate. Two steps: 15 Switch to Full speed (a new Tiny
Freighter flies Slow on Uranium; Full speed takes Deuterium at the dock, 25 of
it now in the Expansion crate) and 19 Answer a signal (while Enrichment runs;
bots skip it). The charter has 28 steps.

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
  has a free base any more. Since §charter-v6 the Command Center's reactor
  runs the colony and nothing on the grid (`power::COMMAND_CENTER_MW` = 0),
  so every powered structure needs a plant: since §habitat-first the farms
  too (Bioharvester and Agroplex, 1 MW each), not the Habitat. One Solar
  Array I (8 MW) covers the landing world's charter builds (7 MW).
- **Home ore:** a basic Ferrite Ore deposit on the habitable planet, so the
  first mine stands beside the Command Center. Planet III keeps its own ore
  for step 16.
- **Housing:** the 7,000 settlers live aboard until the Habitat stands.
  Habitat I houses 12,500 on the home world: 8,000 a level × Large 1.25 ×
  Terran 1.25, the same as any Habitat (§habitat-rebase, Nov 1 2026; the
  camp's smaller Founding Quarters and its Quarters project are gone).
  Habitat II houses 25,000, IV 50,000 (Major).
- **Workforce (Oct 16 2026):** 6,000 settlers are six workforce, one for each
  post the charter staffs: Bioharvester, Agroplex, Mining Complex, Shipyard,
  Academy, the second Mining Complex. Posts are capped at the free workforce
  (the + stops; the sim clamps `SetAssignment`), so nothing is shared thin.
  A Managed home calls for migrants on a staffing shortage: every worker
  posted and a built post still empty, counting crews already aboard liners
  bound for the system (one empty post, one liner; docs/migration.md).
- **No ships.** Every hull is built at the founder's own Shipyard and fills
  its tanks at the dock.

**Charter crates** (Peter, Oct 31 2026: "the player starts with everything
they need to complete the tutorial which I think is wrong because their
storage is basically full at the very beginning"). The charter's supplies
come as three crates (`founding::CharterCrate`; six until the raid's went, Nov 4,
and five until the Pirates and Expansion crates went, Nov 5). The Authority
holds them sealed, and a sealed crate takes no storage. Each is released at
its chapter's first build. §crate-panel (Peter, Nov 3 2026: "I don't want
it to look like just a step in the tutorial"): a released crate pops up in
the middle of the screen, 2 s after its release so its step's story goes
first: each good's icon and amount, storage before →
after, a line tying it to what released it ("Jace Marrow struck. Arm the
colony and strike back."; `CRATE_TEXT`, pirates by name), and one button,
Accept (`ClientMsg::CollectCharterCrate`). (Its "TERRAN CHARTER AUTHORITY"
seal band went Nov 9 2026, with the advisor's: "doesn't make sense".) Escape does
nothing; an accepted crate the server hasn't reported collected after 5 s
pops up again (story-events `crateEvent`, `CrateOffers`).

| Crate | Released at | Holds | For |
| --- | --- | --- | --- |
| Landfall | landing (open in the holds) | 235 Alloys, 104 Machinery, 20 Polymers, 100 Provisions, 28 Semiconductors (the raid's 20 and the Solar Array's 8, §charter-v6), 5 Weapon Mounts, 6 Fission Reactors, 7 Zephyr Drives, 2 Nav Computers, 3 Hull Sections, 6 Pulse Lasers, 1 Cargo Bay, 4 Composites (§hull-ladder) | Bioharvester, Habitat, Agroplex, Mining Complex, Laser Battery, Solar Array, Shipyard, Smelter (§smelt-from-landing, Nov 6 2026: +30 Alloys, +15 Machinery); food until the farms run (`KIT_PROVISIONS`); the ship parts the raid takes (`KIT_SEMICONDUCTORS`, `KIT_ARMAMENTS`); the charter's six Fission Reactors (the Interceptor's, the Tiny Freighter's and the barge's) and the ten Zephyr Drives of every charter hull |
| Reach | 24. Build a Mining Barge | 40 Alloys, 18 Machinery, 1 Nav Computer, 9 Hull Sections, 1 Cargo Bay, 18 Composites, 2 Precision Components, 50 Uranium (its three reactors are the den's, its drives landed; §hull-ladder kit) | Mining Barge, Uranium Mill; the Uranium fills most of the barge's store until the Mill runs |
| Vendetta | 27. Secure your stores | 70 Alloys, 18 Machinery | Warehouse, Vault |

- **No Pirates crate** (Peter, Nov 5 2026: "Let's just give the player
  another warp core to start"): the first Interceptor's Fission Reactor (and Zephyr Drive) lands with
  the colony; its 20 Alloys come from the trader with its Semiconductors and
  Armaments ("Buy the Interceptor parts"), paid for by the mine's stipend
  and ore sold to the trader (§three-stipends, Nov 6 2026; "Shipyard staffed"
  paid 1,200 for them before). The guide's prefill counts a crate only once
  it is released.
- **No Expansion crate** (Peter, Nov 5 2026: "the pirate den to hold what the
  player needs for the next steps and if they need to make two trips in their
  freighter so be it!", then "Let's have the Pirate raid only take 25
  units"): see "The den's stores" below. The Tiny Freighter that hauls them
  comes with its Fission Reactors and Zephyr Drives at landing and its parts and fuel from the trader,
  who stays for the charter — and, since §hull-ladder (Nov 7 2026), sells the
  Small Freighter's frame, bays, Nav Computer, plating and precision parts too
  (a Hub import is a light round trip and a freight run, an hour in the
  charter harness). The barge's Uranium moved to the Reach crate;
  the farms make the Provisions and the Smelter the Alloys after it. Nothing
  lands spare for a later step: the Academy's Alloys tried it and the step-10
  prefill (and the bots) spent them on the Interceptor, leaving the Smelter
  short with no seller near.

- **At landing** the holds carry the Landfall crate and the home's fuel seeds
  (20 Uranium, 10 Deuterium): 471 of the 1,000 units (357 before the Raid
  crate joined it, Nov 4), where the old kit filled 938. The mine has room for
  about 21 minutes of ore.
- **A crate always opens.** Collected, it lands in the home stockpile whole,
  past the storage cap if it must; production then waits for room. So a crate
  never locks away the goods for the Warehouse that would make room for it.
- **What they don't carry:** the Semiconductors and Armaments of later builds,
  bought from the trader as each build comes up. The bots' stock-up
  (`charter_stock_up`) counts the goods still sealed, so they never buy what a
  crate will bring.
- **Bots** collect a crate as it is released. A finished charter opens any
  crate still sealed.
- **Against the old kit:** 106 more Alloys (the Warehouse and Vault's 70, 26
  the late Interceptors used to take from the Smelter, 10 for repairs), 11
  more Machinery and the Academy's 20 Provisions on top of the 100.
- `each_crate_carries_its_builds` (sim) keeps every crate equal to its
  builds' recipes.

## Twenty-nine steps in six chapters

Since Nov 6 2026 (§smelt-from-landing, §freight-frames): the Smelter needs no
research and comes right after the ore sale, before the Solar Array and the
Shipyard (step 8; Peter: "maybe we should do it before the shipyard", then
"before the solar array too"), so the colony smelts its own
Alloys from the Interceptor on; the research step researches Freight Frames,
and its Small Freighter is step 21. Step numbers below are current; older
sections of this page may give the numbers of their day (one or two lower
from step 10 on).

| Chapter | Step | Completes when (server facts) |
| --- | --- | --- |
| Landfall | 1. House the settlers | A Habitat is built. It draws no power (§habitat-first, Nov 8 2026). |
| | 2. Build a Solar Array | A Solar Array is built at home ("Light the colony"; §charter-v6: the Command Center's reactor runs the colony, not the grid). Its 8 MW run the farms, the mine, the Smelter, the Shipyard and the Academy. |
| | 3. Harvest biomass | A Bioharvester is built and staffed (1 MW). |
| | 4. Make provisions | An Agroplex is built and staffed (1 MW). |
| | 5. Bring the mine online | The home's site report shows a staffed Mining Complex. |
| | 6. Smelt your first Alloys | A Smelter is built and staffed at home. The privateer sets out as its order goes in (§smelter-raid), so the raid can come while this is still the step. No research. The Smelter keeps no ore back (§sell-alloys, Nov 11 2026; the 50-ore `CHARTER_ORE_RESERVE` is gone). |
| Raid | 7. Pirates raid the colony | The report of what the raid took reaches command (see **The raid**): the six Fission Reactors and seven Zephyr Drives, the Semiconductors and the Weapon Mounts. |
| | 8. Build a Laser Battery | A station of any level stands at the home (see **The Laser Battery**). |
| | 9. Sell Alloys to the trader | Any Alloys sold to the free trader (up to 10 a visit), or the trader has gone. Ferrite Ore until Nov 11 2026. |
| | 10. Build a Shipyard | A Shipyard is built and staffed. |
| Pirates | 11. Build an Interceptor | The corporation owns an Interceptor and a captain commands it (or no captain is free). |
| | 12. Destroy the privateer | The privateer's defeat is reported and the 8,000 Cr bounty paid (by the Authority's contract on him since Nov 9 2026). Since Nov 12 2026 he is at his den, and one assault takes both. |
| | 13. Eliminate the privateer den | The report that the den fell reaches command (since Nov 12 2026 the same battle as step 12's, so it follows at once). |
| Expansion | 14. Repair the Interceptor | Every hull the den's blast hurt is whole again (repairs are automatic at a staffed Shipyard). |
| | 15. Build a Tiny Freighter | The corporation owns a freighter (or the loot is already loaded). |
| | 16. Switch to Full speed | A freighter of the corporation flies Full speed, or the loot is already aboard. |
| | 17. Haul back the den's loot | The loot is in the home stockpile, or its freighter was lost. |
| | 18. Export your Alloys | Word that Alloys are on their way to the Market Hub reaches command, or 90 s have passed since the freighter left its dock (see **The export**; Ferrite Ore until §charter-v6). |
| | 19. Research Freight Frames | Freight Frames is running or finished and an Academy is staffed (`CHARTER_RESEARCH`; Enrichment until Nov 6 2026). |
| | 20. Answer a signal | A captained fleet is answering a signal or has answered one (bots, and a corporation with no open signal or no captained fleet, skip it). |
| | 21. Build a Small Freighter | The corporation owns a Small Freighter (150 cargo, three Tiny holds). Its card waits on the research. |
| | 22. Expand mining operations | Another world of the home system runs a staffed Mining Complex beside its own power plant. |
| | 23. Chart a nearby system | A survey report of the den's system arrives. |
| Reach | 24. Build a Mining Barge | A barge exists. |
| | 25. Prospect the belt | The report of the barge's find reaches command (a belt hiding no den pocket skips it). |
| | 26. Mine the belt | The report that a barge is cutting ore at a belt away from home reaches command. |
| | 27. Bring the ore home | The report that a barge unloaded or delivered ore arrives. |
| Vendetta | 28. Secure your stores | A Warehouse and a Vault stand at home (see **Secure your stores**). |
| | 29. Defeat the privateer's uncle | The report of how it ended reaches command ([pirate-vendetta.md](pirate-vendetta.md)). His oath comes after the survey, and he strikes a freighter or the second mine, then the barge, before this ("The campaign"). If his raid leaves nothing fighting for the colony it falls: game over, and the founder restarts. |

## The den's aftermath (steps 13–16)

Peter (Nov 2 2026): "have the player build their first tiny freighter to get
the pirate den's stash", then "why not just reduce the pirate den's stash to
50 units exactly", and "at the same time, let's have the interceptor return to
home to get repair to teach the player how to repair their ships, make sure it
gets damaged around 75%".

- **The stash** is what the raid took and §den-stores the den's stores. The
  raid takes 25 (Peter, Nov 5 2026): the kit's 20 Semiconductors and 5
  Armaments (`KIT_SEMICONDUCTORS`, `KIT_ARMAMENTS`; 35 and 15 before). When
  the founder's own den falls its plunder becomes `FoundingProgram::stash`,
  not salvage contracts.
- **The den's stores** (`founding::DEN_STORES`, in the den's plunder from the
  day it rises). Peter (Nov 6 2026): "it should really have high value items
  only" and "max 100 units of loot so 2 trips". 41 Machinery (the second
  mine's Solar Array and Mining Complex, the Small Freighter and a unit for
  repairs), 25 Weapon Mounts and 3 Fission Reactors (the Small Freighter's, and with the
  raid's 5 Armaments the uncle's two Interceptors' guns and cores). No
  Alloys: the colony smelts its own (§smelt-from-landing; 57 until Nov 6).
  With the raid's 25: 94 units, two Tiny Freighter loads (124 and three
  before). A hold takes what the charter needs first (`STASH_LOAD_ORDER`): 20
  Semiconductors (the Academy's) and 30 Machinery; then 11 Machinery, 3 Warp
  Cores and 30 Armaments. A round trip at Full burns ~19 Deuterium (22k su
  den): a tank (52.5) and a refill at home.
  **§hold-room (Peter, Nov 10 2026: "I don't like how the player has to make
  2 freighter trips to the den"):** hold room now goes by class (ore 1;
  refined bulk, Provisions and fuel 0.5; parts, modules, Semiconductors and
  munitions 0.25; see [fuel-overhaul.md](fuel-overhaul.md)). Today's 100 units
  (37 Machinery, 30 Weapon Mounts, 20 Semiconductors, 6 Fission Reactors, 7
  Zephyr Drives) fill 41.75 of the Tiny Freighter's 50: one trip.
- **The blast.** As the den falls its reactor blows: every founder hull in the
  den system is left at most a quarter of its hull (`DEN_BLAST_HULL_FRAC`
  0.25, never destroyed), and each fleet it leaves damaged goes into
  `den_wounded`. The den's log line adds "Its reactor blew as it fell."
- **The steps** (Peter, Nov 3 2026: "aren't those 4 separate steps?!"; "Yes
  split it for this hauling exercise"): 13 Repair the Interceptor, 14 Build a
  Tiny Freighter, 15 Haul back the den's loot. A step is one fact the server
  checks; the guide's cards are the moves toward it. The three overlap:
  `World::advance_founding_stash` runs in each, so the loot loads and travels
  whichever is current, and a later step already done completes when reached.
  - An owned freighter at rest in the den system with room loads what fits,
    gets a `DeliverToSystem` mission home and sets off by itself
    (`FoundingStashLoaded`, "Loot loaded at {den}: … Bound for {home}.").
    The haul home unloads it as any haul home does. What a load takes leaves
    `stash` for `stash_aboard`; the rest waits at the den.
  - When it berths at home the next load waits for a freighter; the stash is
    home with the last load, or when a carrier is lost. The view's `goods`
    is all of it not home yet, `left` what waits at the den.
  - A wounded fleet leaves the list once it is whole, or gone.
  - Step 13 is done when the wounded list is empty, 14 when a freighter is
    owned (or the loot already loaded), 15 when the loot is home. A home
    without a den skips all three.
- **Repairs at home.** A staffed Shipyard mends docked hulls at 6 hull points a
  second per tier (`REPAIR_HP_PER_SEC_PER_TIER`), so a quarter-hull
  Interceptor is whole in about 25 s for 1.5 Alloys and 0.3 Machinery.
  Repairs used to read the yard's crew and power from the system's primary
  planet (`site_for`), so the charter's Shipyard on the landing world mended
  nothing; they now read each yard's own world.
- **The guide** (client `foundingPlan` cases 13–15):
  - 13: Bring the Interceptor home [Send home] (`repair-home`), "Interceptor
    heading home · ETA", "Repairing the Interceptor · 64%"; an unstaffed
    Shipyard shows "Staff the Shipyard". The freighter's next order (Build,
    Fetch) waits under "While you wait".
  - 14: Build a Tiny Freighter ("For the den's loot: 100 units.", hint "One
    trip."; trips are counted by room, "N trips: a hold takes 50." only when
    the loot outgrows one hold). While the trader is docked: "Buy the Tiny
    Freighter parts · 20 Alloys · 8 Machinery · 8 Polymers · 53 Uranium · 57
    Deuterium" (its parts and the fuel it fills at the dock, less what home
    holds; `merchant-tiny_freighter`, `TINY_FREIGHTER_FUEL`).
  - 16: Fetch the loot [Fetch] (`stash-fetch`, what waits at the den),
    "Freighter en route to {den}", "Freighter bringing the loot home · ETA"
    (what is aboard; hint "74 more at {den}"), then Fetch again.
  - Bots fly the haul at Full (`charter_stash` sets it), and their
    `charter_reserve` now holds back the Smelter's goods and counts only the
    den's stash on its way (an optional Warehouse once took the Smelter's
    Alloys). A bot run expands by ~29 game-minutes (17.9 with the crate).
  - Repairs are automatic: docked at a staffed Shipyard, a ship mends on its
    own; the cards' tooltip says so (Peter, Nov 3 2026: "make sure the head
    home tutorial step also has a repair step").
- **Money.** The stash brings back what the raid took, so the trader no
  longer sells the 35 semiconductors and 15 Armaments (3,045 Cr at its
  price); the bots' stock-up counts the stash still on its way. The
  privateer's bounty went from 8,300 to 5,300 Cr to keep the margin; the
  supply test bounds it both ways. The den's contract pays 3,500 Cr when its
  fall reaches command (**The den's contract**).
- **Bots** send a freighter with room to the den (`charter_stash`), hold Hub
  runs while the stash waits, bring the wounded home and keep their Shipyard
  crewed until it is whole. Bot e2e: den 7.6 min, stash and repairs done
  13.7, mining expanded 18.7.
- **Funnel:** keys `repair`, `freighter` and `loot`, chapter Expansion.

**Why expand, why the barge** (Peter, Oct 30 2026: Expansion and Reach "need
to be justified better to the player"):
- **Refining (steps 17–19).** Step 17 is Enrichment
  (`founding::CHARTER_RESEARCH`), because it unlocks the Smelter. The
  Authority's grant funds only that programme during the charter, and it now
  leaves 3 minutes of work (`FIRST_RESEARCH_REMAINING_S`, was 12).
  - Step 18 builds a Smelter, Ferrite Ore into Alloys, on the landing world's
    free industrial site. It draws 2 MW of the 9 the landing world has.
  - A staffed Smelter eats more ore than one mine makes, so the stores bar
    shows ore falling. Step 19's cards say "The Smelter is short of ore." once
    it is starved, and the second mine answers it.
  - The Smelter's crew is a seventh post, so founders land with 7,000 people
    (`HOME_FOUNDING_POP`, was 6,000).
- **The den's pocket (steps 21–23).** Every den belt hides a Titanium Ore
  pocket (`founding::den_pocket`, seeded in `seed_belt_pockets` as every home
  belt's Cuprite pocket is).
  - It holds 600 units at 110 Cr a unit against Ferrite's 8, about 66,000 Cr
    at base price, near the home pocket's worth.
  - §stash (Nov 2 2026): the den's loot no longer carries a Titanium Ore
    sample; the den's log line ("Its asteroid belt looks rich in minerals")
    is the hook.
  - Step 22's prospecting pass finds a den's pocket for certain; other
    pockets stay at 50% a pass. Step 23 then mines it (`MineBelt` with
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
`client/src/story-events.ts`). Since Nov 6 2026 (§three-stipends) a chapter
opens as the served stage reaches its step (`CHAPTER_STAGES` in
`client/src/core/session.ts`); it used to open on the stipend line that
started it, and those stipends are gone:

| Chapter | Opens at | Pop-up | Painting |
| --- | --- | --- | --- |
| Pirates | step 11, Build an Interceptor ("Shipyard staffed" until Nov 6) | Time to Strike Back · "The Shipyard stands. {Nephew} is still out there." (Nov 9 2026: the advisor's own scorn instead, docs/charter-advisor.md) | `chapter-pirates` |
| Expansion (step 13) | none since Nov 5 2026: the den's payout ("{Family} den eliminated · contract") opens "The den's loot" (`denLootEvent`, `den-loot`) instead; "Lanes open" is kept for later | Lanes open · "The den is gone. The lanes to the Market Hub are clear." | `chapter-expansion` |
| Reach | step 24, Build a Mining Barge ("Nearby system charted" until Nov 6) | Into the belt · "The survey is in: the den's old belt is rich in ore." | `chapter-reach` |

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

  | Structure | Opens at step (Nov 8 2026) |
  |---|---|
  | Habitat | 1 |
  | Solar Array (Power Plant) | 2 |
  | Bioharvester | 3 |
  | Agroplex | 4 |
  | Mining Complex | 5 |
  | Smelter | 6 |
  | Laser Battery (station) | 8 |
  | Shipyard | 10 |
  | Academy | 19 |
  | Asteroid Quarry | 22 |
  | Vault | 28 |
- The build check refuses an early order: "The charter opens this at step N".
- `FoundingView.charter_gates` lists the structures still closed. The build
  sheet shows them locked: "Charter step N".
- Bots skip them (`Observation::charter_allows`).
- Structures the charter doesn't build aren't gated; most wait on research
  anyway.
- Why: a smart player used the kit's 40 Semiconductors and 15 Armaments to
  build a Solar Array, Shipyard and Interceptor before the mine, so the raid
  found almost nothing and met an Interceptor.

**The raid** (step 7, `world/founding_raid.rs`)
- **Launch** (§smelter-raid, Nov 9 2026, Peter: "I want the Pirates to start
  moving towards the player's system when they immediately start building
  their smelter and then the warning & watch occurs at the same distance
  right now"): as the Smelter's order goes in at home during step 6 (or once
  one stands; `founding_raid_due`), the Rogue Privateer sets out from beyond
  the colony's sensors on its den's side, 1.25 sensor ranges out
  (`RAID_START_SENSOR_RANGES`). Its den rises with it (`ensure_founding_den`).
  - It is seen about 22 s later, at the same range as before (about
    3,200 su), as the Smelter's 20 s build ends. It is over the colony about
    54 s after the order.
  - A founder slow to staff the Smelter meets the raid at step 6. The guide
    then shows the raid card, with the Smelter's build or crew beside it.
    Step 7 hands straight on to step 8 once the Smelter is staffed, if the
    raid's report is already in. The Laser Battery's card waits for step 8.
  - Nov 8–9 2026 it set out when step 7 opened, the Smelter built and
    staffed (§habitat-first), arriving about half a minute after the
    "Smelter running" stipend.
- **Watch the pirate** (§watch-the-pirate, Nov 9 2026, Peter: "a 'Watch the
  Pirate' step as soon as the Pirate Warning appears and only ends once the
  Pirate has disappeared"): from the sighting until the colony loses the
  privateer after its raid (`FoundingView.raid_lost`, from the sim's
  `raid_lost_at`, protocol v88), the guide's card is "Watch the pirate",
  amber and quiet. Its line says "Closing on Freya · ETA", "Stripping the
  stockpile." or "Leaving with what it took.", and its button watches it.
  Whatever step is current (6, 7 or 8) comes back once the raider is gone;
  nothing waits beside it (Nov 9 2026, Peter: "let's get rid of the While
  you wait · 1 when the pirate is raiding"). It replaced the "Raider closing on…" / "Raider over the colony"
  cards. As it goes, the Charter Advisor's large "The raider is gone" panel
  pops up (docs/charter-advisor.md).
- **The Laser Battery's card** (§laser-battery-card; Nov 9 2026, Peter: "The
  recommendation to build a Laser Battery should come from the Authority
  Advisor and make it snarky, cynical and quasi insulting to the player. The
  idea is to instill some hate for the Authority in the player early"): the
  painted "Build a Laser Battery" pop-up is the Charter Advisor's: their
  face beside their own line (`Advisor.battery`)
  in place of "It would be wise to build some defenses after that pirate
  raid…" (kept for a charter without an advisor). It comes once the watch
  ends, 2 s behind the raider-gone panel (`AdvisorDesk.leftOwed`); until
  Nov 9 it came straight after the raid's report, while the guide still
  watched the raider.
  - A home without a den sends it from the dark side, away from the Hub.
  - Oct 26 – Nov 8 2026 it set out as the charter's mine started building
    and the mine rose in its 30 s flight (`CHARTER_MINE_SECS`,
    `raid_on_mine_start`, `finish_charter_mine`, all gone).
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
  took; the timeline line "Pirates raided {home} and took 35 semiconductors
  and 15 armaments." raises the raid's pop-up (`piratesEvent`), and its
  arrival (`raid_report_at`) completes the step. The trader is called later
  (see **The trader and the ore**).
- **Timing** (one seed, the den 15,000 su out): seen 19 s after the mine
  comes online, over the colony at 31 s, report at 44 s, the trader docked
  at 84 s; the privateer is home at its den and lurking by about 2 minutes.
- **Home:** it unloads the loot into the den's stores (`Enclave::plunder`)
  and docks with the den's garrison (§den-retreat, below). A home without a
  den has it lurk just outside the colony's sensors: 1.1 Command Center
  sensor ranges out (6,600 su) on its den's side (`RAID_LURK_SENSOR_RANGES`).
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
  - Since §den-retreat (Nov 12 2026) this is the den-less case only: with a
    den, the den marks the spot and the amber marker isn't drawn.
- Persisted (`FoundingProgram::raid`): a reload neither repeats the raid nor
  refills what it may take.

**The trader and the ore** (step 9)
- The free trader is called once the colony has lost sight of the privateer
  (§trader-call, Nov 5 2026) and, since Nov 9 2026 (§trader-battery, Peter:
  "the free trader shouldn't start flying to the player's home planet until
  the laser defense is under construction"), the home's Laser Battery is
  under construction or standing. It docks 9–15 s later, before the
  battery's 20 s build ends ([traveling-merchant.md](traveling-merchant.md)).
  The battery is built from the Landfall kit's Alloys and Machinery, which
  the raid leaves alone, so the wait can't deadlock. Its docking pop-up is
  the Charter Advisor's announcement since Nov 9 2026 (docs/charter-advisor.md),
  in place of "{Trader}'s {ship} has docked at your colony. Word is its hold
  carries semiconductors." It now sells
  Armaments too (15), and takes up to 300 ore (`ORE_CAP`, was 150).
- The guide: "Sell ore to the trader · We need semiconductors for our ships.
  [Sell]" (Peter's line, "We'd better sell ore to the trader to buy
  semiconductors for our ships. [Sell ore]", trimmed Oct 27 2026 because it
  repeated the title), which opens the stall on its Buying tab with all the
  ore it will take filled in (`merchant-ore`). Until the trader's own pop-up has shown, the card is
  "Find semiconductors · The Solar Array needs 8" at the Market.
- **The money (Nov 6 2026, §three-stipends):** 2,000 Cr start + 2,000 (mine
  online) = 4,000 Cr, against about 5,100 Cr at twice the Hub for 35
  semiconductors (Solar Array 8, Shipyard 15, Interceptor 12), 15 Armaments
  and the Interceptor's 20 Alloys. That leaves about 1,100 Cr short, about
  275 ore at its 4 Cr (half the Hub's 8), inside its 400. The mine makes 23
  ore a minute. See **Stipends**.
- **The money before Nov 6:** 2,000 Cr start + 300 (mine) + 300 (raid) + 400 (Shipyard
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
Weapon Mounts, a Fission Reactor and a Zephyr Drive. Corvettes still need Shipyard II.
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
  since the raid. A docked Interceptor whose reactor's tank is empty (Uranium
  on Fission) gets "Fuel the Interceptor · Buy Uranium" from the trader first
  (`merchant-fuel`), while the trader stocks it. Nov 8 2026: it had read the
  Deuterium tank, which a Fission hull doesn't have, and blocked every hunt.
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

**The privateer's contract** (Peter, Nov 9 2026: "The Authority Advisor gets a
transmission from the Authority that there just happens to be a contract out
to eliminate the Pirate. Can we introduce the player to the Contracts at this
point?"). The charter's first contract, and the player's introduction to
Contracts.
- As the hunt begins (step 12, the Interceptor under its captain), the
  Authority posts a private contract on him: "Eliminate {Nephew}", a
  `PrivateerPatrol` at his last known position with a `FounderPrivateer`
  briefing ("{Nephew} raided your colony. Last seen lying in wait nearby." ·
  One raider · One Interceptor), 8,000 Cr (`PRIVATEER_CREDIT_BOUNTY`), no
  standing (`World::post_founding_privateer_contract`). It is posted at the
  colony, so its offer is news at once, and it is **offered**, not taken on.
- Its offer's log line, "Authority contract offered · Eliminate {Nephew} ·
  8000 Cr.", opens the Charter Advisor's transmission: a large advisor panel
  in their own words (docs/charter-advisor.md), the terms under it, and one
  button, **Open Contracts**. Without an advisor, the Authority's seal says it.
  It shows once a session while the contract waits to be taken on, so a page
  loaded after its news still hears it (boot.ts `offerPrivateerContract`).
- The guide's step 12 starts with **Take the Authority's contract** (Open
  Contracts) while it is offered. Contracts (Operations, both shells) pins it
  on top under "Authority contract" with its Accept ringed ("Accept the
  contract here."). Once taken on, the hunt goes on as before, its reason
  "Contract: 8,000 Cr." (it was "Bounty: 8,000 Cr.").
- It never lapses and can't be dropped once taken. The kill completes it,
  taken on or not (`complete_founding_privateer_contract`): no one loses the
  bounty for skipping Accept, and bots, which never accept, are paid the same
  way. A kill before its offer posts it quietly, completed. Its report rides
  the kill's light and pays where the bounty's stipend did, as "{Nephew}
  destroyed · contract · 8,000 Cr" (it was "… · bounty").
- Protocol v89 (`follow_up` "founder_privateer"). Preview:
  `/scripts/story-event-fixture.html?story=advisor-privateer-contract&advisor=0..2`
  and `/scripts/standing-fixture.html?charter=1` (Operations, pinned and ringed).

**The den stands in its belt** (Peter, Nov 9 2026: "why the battle at the
pirate den occurred at the star instead of their pirate den in the asteroid
belt"). Its guard rises at the point of the belt facing the colony
(`belt_point` toward home, held there by `at_world`), where an Interceptor
flying in from home comes in. Every pirate base's assault now opens where
its pack holds (`epos` in the enclave tick), the star only while the pack is
away or gone; it had opened at the star while both sides sat out on the
belt. Dens raised before Nov 9 hold the belt's east point.

**The den retreat** (Peter, Nov 12 2026: "let's have the first pirate raid
retreat back to their den instead of the hunting area"; picked "One assault
at the den")
- **Home:** the privateer flies home from the raid, unloads, and docks with
  the den's garrison: his ships join its fleet (`Fleet::absorb_ships`), and
  that fleet is the privateer from then on (`FoundingProgram::privateer`),
  so its fall is his kill (`World::advance_founding_raid`, Returning).
- **Garrison:** two Raiders at 75% hull (`DEN_GARRISON_RAIDERS`, three
  before), so with him home the den holds three again, his at full hull.
- **Known as the hunt begins:** step 12 opens with the den revealed (intel
  snapshot) and both contracts posted, on him and on the den
  (`World::advance_local_opening`). The server sends `FoundingView.den`
  from step 12 on.
- **One battle:** the Interceptor at the den fights them all. The kill
  completes step 12, and the den's fall, noted in the same tick
  (`note_and_tell_founding_den_cleared`), completes step 13 on the kill's
  light. The blast, the den's loot and the uncle's beat follow as before.
- **Balance:** one Interceptor wins over eight seeds, 44% hull left at worst
  before the blast and 68 s at longest (the old den fight: 45%, 27–54 s;
  `one_interceptor_takes_the_privateer_and_his_den_in_one_assault`).
- **Guide:** step 12's card is "Attack the den at {den}" · "The privateer and
  his crew: three raiders. No defenses." · Send Interceptor (`assault-den`),
  then "Interceptor en route to {den}" and "Interceptor at the den"; the
  advisor's line "He went home to his den. His bounty funds the colony."
  The den's pop-up reads "{Nephew} went home to his den at {den}. The
  Authority has a contract out on it." The "Hunting area" marker is not
  drawn while a den is known.
- **Bots:** their target is the den's fleet, and its last-seen point the
  den, so `charter_privateer` sends the Interceptor there unchanged.
- A home without a den keeps the hunt above (lurk outside the sensors, last
  seen, log reveals nothing).

**The den's contract** (Peter, Nov 5 2026: "destroying the privateer provides
coordinates to the pirate den and it is noticed that there is an Authority
contract to eliminate the pirate's den by his name")
- The privateer's log names his den (as before), and with it comes the
  Authority's contract on the den: "Eliminate the {Family} den", a private
  `PirateBounty` with a `FounderDen` briefing ("{Nephew}'s base at {den},
  from his log. No defenses." · Three damaged raiders · One Interceptor).
  It is posted when the kill is processed and rides the kill's light home
  (`World::post_founding_den_contract`), once per den.
- It comes taken on: the colony holding the coordinates is the contractor,
  so its first report home says "active", not "offered". It never lapses
  (`expires_at` = `f64::MAX`; the client shows "no deadline") and can't be
  abandoned (`apply_abandon_operation` refuses it; no Abandon button).
- The den's fall completes it like any bounty (`process_operation_events`);
  a den found cleared without its fall in hand completes it anyway
  (`tick_operations`). It pays `DEN_CONTRACT_CREDITS` (8,000 Cr since
  §hull-ladder, Nov 7 2026, when a light engine became 1,000 Cr and the Small
  Freighter's kit moved to the trader; 5,000 that morning, 3,500 before) and 2
  standing when that report reaches command, the same light as the den
  step's own report.
- It replaced the den's charter stipend at the same sum. The payout is the
  charter's reward moment: "Charter milestone · {Family} den eliminated ·
  contract · 3500 Cr · +2 standing.", which opens "The den's loot" panel
  (Expansion's "Lanes open" until later that day).
- What the player sees: after the kill's "Last transmission", an Authority
  pop-up under its seal ("Authority contract" · "{Nephew}'s log gives the
  coordinates of his den at {den}. The Authority has a contract out on it:
  3,500 Cr.", the seal's line the contract's title), from the log line
  "Authority contract · {title} · 3500 Cr.". The contract shows under
  Contracts → Active and as an Authority tag on the den's system. Step 12's
  card says "Contract: 3,500 Cr." (hint: "Authority contract. The den holds
  what the raid took.").
- Bots get the same contract and pay-out; they never touch contracts
  otherwise.

**The trader stays for the charter** (Peter, Nov 5 2026: "The free trader
doesn't leave after building your own freighter that way the player can buy
the necessary parts from free trader")
- It stays docked until the charter is fulfilled, then leaves at once.
  `merchant::MAX_STAY_S` is now a week (3 hours before), a safety net for a
  colony left mid-charter; the finale alone can wait a day
  (`VENDETTA_DEADLINE_S`). No countdown shows: the stall reads "Docked" and
  the top-bar chip "Free trader".
- Every build after the den buys its parts as it comes up: the guide's
  "Buy the {build} parts" card (`viaMerchant`) on the Academy, the Smelter,
  the expansion's Solar Array and Mining Complex (or Asteroid Quarry), the
  Survey Probe, the Mining Barge and the uncle's Interceptors.
- Step 13: the den-mauled Interceptor's repair spends Alloys and Machinery
  (0.01 and 0.002 a hull point, `build::REPAIR_COST_PER_HP`), none of which
  is home before the den's first load. Docked at a staffed Shipyard with the
  parts short, the card is "Buy the repair parts · 2 Alloys · 1 Machinery"
  (`merchant-repair`, its hull % as the reason), then "Repairing the
  Interceptor".
- Gone: the last-call stock-up card ("Stock up before the trader leaves",
  `merchant-ahead`), its countdown, `AHEAD`, the client's
  `RESEARCH_RESERVE`, `LAST_CALL_S`, and the traders' last-call lines. The
  bots keep their stock-up (`charter_stock_up`) as a habit; it no longer
  races a departure.
- History: Oct 16 2026 (Peter: "Teo should leave while the player is
  eliminating the pirates") it left 3 minutes after the privateer's kill
  report, the den's fall cutting that short, and the guide had the bounty
  stock up for the builds ahead. **Why** then: measured with a bot, leaving at
  the bounty with no stock-up let research slip from 21.7 to 46.2
  game-minutes. Nov 5 (§den-stores) it stayed until the Tiny Freighter was
  built, then gave the 3-minute last call.
- The timeline line on departure: "Teo Brannick's Long Odds left {home}."

**The export** (step 16 since §stash; Peter, Oct 16 2026: "insert an export ore after
step 11")
- **Done when** word that ore is bound for the Market Hub reaches command
  (`export_dispatched_at`): a Freighter carrying Ferrite Ore past the home's
  gravity well on its way to the Hub (light-delayed from the ship), or an
  Authority freight booking of ore from home to the Hub warehouse.
- **The guide:** "Build a Tiny Freighter · 20 Alloys · 8 Machinery · 8
  Polymers · 1 Fission Reactor · 1 Zephyr Drive [Build]" (the kit covers it; the reason gives the
  Hub's ore price, the Uranium fuelling is on hover), then "Export your ore
  [Plan the shipment]", which opens Logistics on the first-shipment card: 50
  ore out, the Academy's goods back within a spending cap, one trip. A hold
  ("Waiting for fuel…", "Waiting for escort") is its own card, "Shipment
  waiting [Open Logistics]". "Make room at home" if home storage blocks it.
- **The follow** (Peter, Nov 2 2026: "the player has to follow the tiny
  freighter for 90 seconds max"): the moment the loaded freighter leaves its
  dock for the Hub (`export_departed_at`), the client takes the player to it
  the way the raid's first sighting does (`focusShipment`, `trackShipment`):
  out to the galaxy map, the freighter selected and framed with home, the
  tracking window on it with "FIRST SHIPMENT · WARP IN m:ss". The card is the
  wait: "Follow the Freighter · warp in m:ss [Follow]" (Follow re-engages the
  camera after a drag). It ends when the ship clears the hyperlimit and warps,
  under 45 s from any orbit at 40 su/s, and the step completes on its light
  home. `founding::EXPORT_FOLLOW_MAX_S` (90 s after departure) caps it: the
  step completes then whatever the ship has done. After the step the trip
  stays in view as a progress line, "Shipment heading to the Market Hub · ETA".
- **No escort after the bounty:** the first shipment no longer takes the
  Interceptor by default once the privateer is dead. A route waits until its
  escort is beside it and fuelled for the whole leg.
- **Signals open here** (Oct 20 2026, [signals.md](signals.md)): the first
  two appear near home as step 16 begins, for the Interceptor's captain to
  answer between charter steps (the silent probe's 1,500 Cr and a second
  captain).
- The den's contract (3,500 Cr) pays as its fall reaches command; a home
  without a den has neither.

**Research, survey, Reach:** as v3. The Academy costs 25 Alloys, 15
semiconductors and 20 Provisions; the Survey Probe 6 Alloys and 4
semiconductors; the Mining Barge 40 Alloys, 20 Machinery, 12 Polymers and 6
semiconductors.

## Secure your stores (step 25)

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
  Interceptors under the Laser Battery, 120 Titanium Ore at home): every fight
  ends the same with and without the Vault. The fight decides it, not the
  loot.

## The Laser Battery (step 6)

Peter (Oct 30 2026): "introduce 1 step after the very first pirate raid to
build some very basic planetary defenses". He chose the orbital option, the
Gun Platform. On Nov 1 2026, after playing it, he wanted "a ground based
defensive structure after the initial pirate raid": the same level, renamed
Laser Battery and standing on the ground (no reset; ids stay
`build_gun_platform` / `gun_platform`). A ground laser can hit ships in low
orbit, and in battle it fires from the station's spot as every level does
([orbital-defense.md](orbital-defense.md), "On the ground").

- **What it is.** The station ladder's new first level
  (`station::LEVELS[0]`, so the ladder is now six levels: Laser Battery,
  Defense Platform, Orbital Battery, Defense Station, Citadel, Star Fortress).
  One hardpoint, 300 hull, attack weight 1.5 (an Interceptor has 200 hull and
  3). It costs 25 Alloys and 10 Machinery and goes up in 20 s.
  - It needs no research. Fire Control now unlocks level 2, and each later
    programme unlocks the level after the one it used to.
  - The kit carries its cost, since the raid takes only the semiconductors and
    Armaments.
  - It later rises into the Defense Platform, in orbit, which replaces it.
- **No site.** It takes no building site (`StructureKind::uses_slot`); the
  planet scene gives it a ground footprint of its own, as it does the Command
  Center. Without that, the Gun Platform took the
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

Peter (Nov 6 2026): "There are too many milestones, let's have it when the
Mine is staffed and online, so make that like 2000cr and the Smelter Running
and the 2nd Mine Running. Remove the other ones." Three stipends, 2,000 Cr
each (§three-stipends):

| Stage reached | Title | Stipend |
| --- | --- | ---: |
| Smelter step (the mine staffed and online, step 5 done) | Mine online | 2,000 Cr |
| Raid step (the Smelter staffed, step 6 done) | Smelter running | 2,000 Cr |
| Survey step (the second world's mine running) | Second mine running | 2,000 Cr |

Gone since Nov 6: Colony fed and housed (300), Shipyard staffed (1,200),
Interceptor launched (300), Research under way (300), Nearby system charted
(500), Mining Barge launched (300), Pocket found (300), Belt mining under way
(300) and Founding charter fulfilled (800); "Mining expanded" (400) became
"Second mine running". The Pirates and Reach chapters, which opened on two of
them, open on their steps.

Plus the 8,000 Cr privateer bounty (§hull-ladder, Nov 7 2026; 5,300 before, 6,400 before §stash, 4,500 before Oct 29 2026)
and the den's contract, 8,000 Cr (§hull-ladder; 3,500 at §den-contract, Nov 5 2026, when it replaced the den's stipend,
600 Cr until that day's §den-stores raised it): 22,000 Cr in all (`founding.rs`, `charter_stipend`, `DEN_CONTRACT_CREDITS`).
§hull-ladder: the trader also sells the Small Freighter's kit beyond its engines (frame, bays, Nav Computer, plating,
precision parts, ~8,700 Cr at its twice) and a fourth Fission Reactor, so no charter hull waits on a Hub import; the
uncle's two Interceptors are the first parts a player buys at the Hub.
v5 dropped v4's 500 Cr "Counterfeit claim settled".

The trader charges twice the Hub since Nov 6 (Peter: "the trader should
charge more like 100% because of the cost/time for the player";
`merchant::FOUNDING_MARKUP`, +50% before) and still buys only Ferrite Ore,
at half the Hub, now up to 400 a visit (`ORE_CAP`, 300 before: "the free
trader will only take so much low value ore before not buying anymore").
- **Before the Interceptor** (§smelt-from-landing, Nov 6 2026): the start
  (2,000 Cr) and the mine's and the Smelter's stipends (4,000) against about
  4,060 Cr of the trader's parts for the Solar Array, the Shipyard and the
  Interceptor (35 Semiconductors, 15 Armaments); the Smelter makes the
  Interceptor's Alloys. No ore wait; the ore sale is the lesson and spare
  money, inside the trader's cap. A deal short of credits offers the ore that
  pays for it ("Sell N ore", `merchantBlock`). Before the Smelter moved, the
  start and the mine's stipend left about 1,100 Cr short (about 275 ore, 11
  minutes of the mine for a bot).
- **After the den:** the bounty, the den's contract and the second mine's
  stipend (18,000 Cr since §hull-ladder) against what the trader sells the charter:
  the Tiny Freighter's Machinery, Polymers and fuel, the Small Freighter's
  Polymers, and the Semiconductors of the later builds and the uncle's two
  Interceptors. The trader sells no Alloys.

## Opening build times (Oct 15 2026)

While the charter runs, level I of the Mining Complex, Bioharvester, Habitat,
Agroplex and Solar Array on the landing world (the home system's Command
Center world) builds in 10 seconds, whatever the world's construction speed
(`founding::OPENING_BUILD_SECS`, applied when the order is received). Higher
levels, other worlds and every build after the charter keep their normal
times. The client mirrors it (`openingBuild` in `core/derive/market.ts`), so
the build sheet, the queue and mobile quote 10 s.

From Oct 26 to Nov 8 2026 the charter's mine was the exception: it rose in
the privateer's 30 s flight. Since §habitat-first the raid waits for the
Smelter and the mine takes the ordinary 10 s. See "The raid" above.

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
- Choices (the trader's parts) are buttons that say the whole
  step ("Buy from trader", "Build a Solar Array") under the card's own copy.
- While-you-wait items show their titles. A second Interceptor already under
  construction shows only as its progress line.
- The Log's Inbox keeps side rows (nodes, survey reports, idle stockpiles,
  invites…) out until the privateer's bounty (`CHARTER_QUIET`, docs/log.md).

**The Founding Camp waits** (Peter, Oct 28 2026: "Settlement Projects to be
unlocked after the tutorial was complete"). Its projects (prospect,
surface workings, storehouse; quarters until Nov 1) shared the home world's build slot
and starting kit with the charter's own steps. While the charter runs:
- no "Founding Camp · N/4 developments" card on the home world (desktop and
  phone; `campOpen` in `core/derive/founding.ts`), so no way into the projects;
- no "Develop your settlement" card under the guide and no coach marks;
- "Make room at home" (storage full while the Freighter is out) builds a
  Warehouse (`build-warehouse`) instead of opening the camp.

Once the charter is complete the card appears, and "Your next chapter" offers
"Develop your settlement" (`postVictoryHandoff`, goal `camp`) until all three
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
trader's `merchant-{build}`, `merchant-repair` (the den-mauled Interceptor's
parts), `merchant-sell` (sell the Alloys it takes; `merchant-ore` until Nov
11 2026) and `merchant-fuel` (Deuterium for the Interceptor). One-click orders: `assign-captain` (since Oct 21 2026 it opens the
captain's introduction panel first, see **Assign a captain** below), `attack-privateer`
and `assault-den` (the Interceptor to the den), plus `survey-stop`,
`mine-belt`, `barge-home` and `barge-unload` as v3.

## Retired stages

Old saves are not migrated: start a fresh galaxy. The retired enum variants
remain so serialized stages still parse, and each hands on to its successor:

- v4's `BuySemiconductors` goes to `PowerShipyard`.
- v4's `BuildFreighter` and `BuildConvoy` go to `BuildInterceptor`.
- `PrepareExport` is step 16 (13 before §stash); `LocalRecovery` and `ExportProduction`
  go to it.
- `CompleteExport`, `GrowBusiness` and `BuildSecondFreighter` go to
  `BuildShipyard`.
- `BuildAcademy` goes to `FirstResearch`.
- `BuildScout` goes to `SurveyCandidates`.
- `BuildColony` and `EstablishColony` go to `Complete`.

The v3 lost-Freighter replacement is gone: there is no starting Freighter to
insure.

## Bots, funnel and harness

- **Bots** play the same charter (`bots/opening.rs`): the Habitat, the Solar
  Array, the farms, the mine and the Smelter (§habitat-first); then nothing from the kit until the raid has struck and the
  ore is sold (`raid_pending`), except the Laser Battery at step 6
  (`gun_platform`); at step 7 they sell the trader all the ore it
  will take (`charter_ore_sale`); then the Solar Array, the Shipyard and
  the Interceptor, buying semiconductors and Armaments from
  the trader and selling it ore toward any shortfall, and no Hub imports of
  goods the docked trader sells. Their Freighter waits for the privateer's
  bounty. At step 11 the Interceptor attacks the tracked privateer
  (`charter_privateer`); short of Deuterium, it buys some from the trader. At
  the den step it flies to the den (`charter_den`) and fights to the end,
  while the bounty buys from the trader what the builds ahead lack, plus the
  Interceptor's Deuterium (`charter_stock_up`). From then on
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
