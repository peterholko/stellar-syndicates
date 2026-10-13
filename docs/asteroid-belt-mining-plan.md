# Asteroid belt mining: plan

Status: phases 0 and 1 built (Oct 7 2026); a first cut of phase 3, the Mining
Barge, with the founding charter's barge lesson (Oct 8 2026); phase 2,
prospecting and pockets, and the rest of phase 3 (Oct 9 2026); the barge's own
art, and phase 4, the Mining Belt Station (Oct 10 2026); phases 5 and 6,
rare materials put to use and the bots (Oct 10 2026). All uncommitted.
Phases 7 onward are still a proposal.

**Built so far:**

- **Belts:** random belts as `BodyKind::AsteroidBelt` bodies, rolled from
  `belts-v1` keyed streams (`crates/sim/src/belts.rs`,
  `world/asteroid_belts.rs`).
- **Ores:** Olivine, Bornite and Ilmenite, with Smelter recipes and their own
  icons. Belt types use their own chunk art.
- **Discovery:** belts are withheld until the viewer surveys the system, and a
  survey notice names the belts it found.
- **Homes:** every home has the same Stony Crystalline belt.
- **Mining:** belts take Resource slots only, so a Mining Complex works them.
  Strip Rigs is wired to moons and belts. Outposts refuse belts.
- **Client:** the System View draws belts from data between orbits, with hover,
  click and the belt's world panel. Belts also appear in the Worlds rows and
  battle backdrops.
- **Phase 0 report:** `cargo test -p sim --release --test galaxy_density
  belt_report -- --ignored --nocapture`. Per 4-player galaxy:
  - 63 belts, 23 of them bare on sparse stars;
  - 26 stony, 18 metallic, 17 icy and 2 shattered;
  - +25% ore content and +19% value over the planets.

  The colony-discovery band test still passes.

**Built Oct 8 2026: the Mining Barge (phase 3, first cut)**

- **Hull:** `ShipKind::MiningBarge` (`mining_barge`). Hold 150, speed 30, hull
  mass 3,500, crew 15, no guns, one utility berth. Built at a Shipyard I from
  Alloys 40, Machinery 20, Polymers 12 and Electronics 6 in 30 s.
  - **No research gate** (the plan's Belt Mining node is dropped for now), so
    the founding charter can teach it.
  - **Art:** its own approved Mining Barge model, shipped as the 4,970-triangle
    LOD2 with dedicated map, panel and battle artwork. The full-detail source
    remains in `client/art-src/ship-models/mining-barge-textured-v1/`.
- **Order:** `MineBelt { fleet, system, body, ore }`, light-delayed.
  - It needs a barge in the fleet, and a system that is unowned or yours.
  - The belt must be charted and hold the ore.
  - A pirate enclave is not refused, since that would reveal it.
- **The job:** `FleetIndustry::Mine` (`world/industrial_expansion.rs`).
  - The barge flies to the star and works within 1,500 su.
  - **Rate:** 0.25 bulk/s per barge × Deep Bores × Strip Rigs, converted by
    `ore_bulk_ratio`.
  - **Shared cap:** all barges on one deposit share 3 × its natural rate.
  - **Reserves:** finite ones deplete.
  - **Where the ore goes:** your stockpile in your own system, otherwise the
    hold. A full barge goes home and unloads with the ordinary cargo controls.
  - **Statuses:** travelling, mining, full, depleted, unsafe, claimed.
  - **Loudness:** the survey ×1.5 signature while mining.
  - **Research credit:** mined units count toward Deep Crust.
- **Client:**
  - With a barge selected, clicking a charted star whose belt holds ore
    stages a MineBelt order for the most valuable ore.
  - A belt's world panel has a Ship mining section, with a Mine button per ore.
  - The fleet line reads "Mining Cuprite → Sif Belt · mining · 37 cut".
- **Bots** build a barge and send it in the charter's lesson
  (`bots/opening.rs` `belt_lesson`).
- **Not yet:** the Mining Belt Station (phase 4).

**Built Oct 9 2026: prospecting and pockets (phase 2), and the rest of phase 3**

- **Pockets:** every belt rolls 0–2 hidden pockets (a shattered world 1–3, a
  bare belt rarely one) from its own keyed stream, `belt-pockets-v1`, after
  every belt exists, so nothing else moves (`belts::roll_pockets`,
  `World::seed_belt_pockets`).
  - **Ores by type:**
    - Stony: Cuprite, Bornite.
    - Metallic: Titanium 0.45+, Iridium 0.60+, Rare-metal 0.70+.
    - Icy: rich Volatiles, Helium-3 ice 0.55+.
    - Shattered: Titanium, Iridium, Rare-metal, Void crystal 0.65+.
  - **Richness:** the planet formula with a ×1.0–1.5 vein and no lean factor.
  - **Reserves:** a quarter of a finite planet seam.
  - **Every home belt** hides the same Cuprite pocket: ×0.30, 3,000 units.
- **Pocket data:** pockets live in `Body::pockets`, never in `deposits`, so
  nothing that reads deposits can leak one. Each has a `found_by` set.
  `Body::prospected` counts each corporation's passes and how many have been
  reported.
- **Prospecting:**
  - **Order:** `ProspectBelt { fleet, system, body }`, for a Scout or a Mining
    Barge, at any charted belt.
  - **Job:** `FleetIndustry::Prospect` flies to the star and dwells 60 s within
    1,500 su. It's loud (×1.5) while dwelling. Danger pauses it, and leaving
    station restarts it.
  - **Find:** each pocket's chance is 50% per pass, keyed per (belt,
    corporation, pass).
  - **Report:** the report heads home by light (`World::pending_pocket_reports`).
    Only on arrival does the find join `found_by` and the pass count toward
    "Prospected N×". Finds are private: there is no ally relay.
- **Strikes:** a barge cutting ore strikes a hidden pocket at 10% per
  barge-hour. The find reports home the same way but isn't a pass.
- **Mining pockets:**
  - A Mining Complex (or Volatile Harvester) works the pockets its owner has
    found, on top of the belt's ores.
  - `MineBelt` takes `pocket: true` to work a found pocket. Barges share the
    cap per pocket, as per deposit.
  - Pockets deplete.
- **Rare ores:** Iridium ore, Helium-3 ice and Void crystal refine in the Smelter
  into Iridium, Helium-3 and Exotic Matter.

  | | Iridium ore | Helium-3 ice | Void crystal |
  | --- | --- | --- | --- |
  | Price | 630 | 260 | 1,680 |
  | Units per batch | 0.1 | 0.2 | 0.05 |
  | Refined price | Iridium 90 | Helium-3 75 | Exotic Matter 120 |
  | Value per bulk unit | ~42 | ~35 | ~56 |

  - Value per bulk unit sits beside Rare-metal ore's 57.
  - All six trade on thin books.
  - Helium-3 ice is harvested with Volatiles (Volatile Harvester).
  - Phase 5 put them to use (see below).
- **View:**
  - A belt body carries `pockets` (the viewer's own finds only) and
    `prospected`.
  - The timeline says "Prospected Veles Belt: titanium ore pocket found." or
    "Barge struck a … pocket in …".
  - The View's loudness now uses `loud_work()`, so mining and prospecting
    fleets are as loud to rival sensors as the sim's pickets think.
- **Client:**
  - The belt panel lists found pockets ("Titanium Ore pocket · 1,800 left") and
    "Unprospected" or "Prospected 3× · 1 pocket found".
  - Ship mining has a Prospect button per Scout or barge fleet, and Mine buttons
    for pockets.
  - The System View draws a four-point glint per found pocket on the band, and
    the hover adds "· 1 pocket found".
  - Mobile has the same rows and buttons.
  - The fleet line reads "Prospecting Veles Belt · 42 s left".
- **Left of phase 3:**
  - The Belt Mining research gate stays dropped (the charter teaches the
    barge).
  - The barge still borrows the Small Freighter's art. Its own GLB and sprite
    are in `art-src` and need baking into the ship-model set.
- **Protocol v45. A galaxy reset is needed** for existing galaxies to have
  pockets.

**Built Oct 10 2026: the barge's own art, and phase 4, the Mining Belt Station**

- **Barge art:**
  - **Model:** it has its own LOD 2 model (4,970 triangles), baked map renders
    and engine hardpoints. Peter's other model built them; the stand-in entry is
    gone.
  - **Map size:** it draws between the Small and Medium Freighters. It had fallen
    back to Scout size.
- **Station hull:** `ShipKind::MiningBeltStation` (`mining_belt_station`).
  - **Stats:**
    - hold 3,000;
    - speed 15;
    - mass 12,000;
    - no guns;
    - a cruiser-grade hull (defence weight 6);
    - crew 60;
    - signature 8;
    - no module slots.
  - **Build:** Shipyard II, from Alloys 160, Machinery 50, Polymers 30 and
    Electronics 25, in 180 s.
  - **Research:** behind a new tier-2 Materials programme, Belt Stations
    (`mat_belt_stations`).
  - **Art:** its own approved Mining Belt Station model, shipped as the
    8,420-triangle LOD2 with dedicated map, panel and System View artwork. All
    seven docking/anchor sockets are retained; no engine or weapon hardpoints.
- **Anchoring** (`world/belt_stations.rs`, `industry::Anchor`):
  - **Orders:** `AnchorStation { fleet, system, body }` and
    `UnanchorStation { fleet }`, both light-delayed.
  - **Phases:** approaching (flying to the star), anchoring (60 s within
    1,500 su), anchored, then unanchoring (60 s).
  - **Fixed in place:** from anchoring on, movement orders are refused
    (`OrderRejectReason::StationAnchored`). So are hauls, freight routes and
    other jobs that would move it, and merging or splitting.
  - **Where:** only a lone station, at a charted belt, in an unowned or allied
    system. Not your own colony, which stockpiles its own ore, and not a
    rival's. One per corporation per belt; rivals may anchor beside you. A
    pirate enclave is not refused, as for barges, because that would reveal it.
- **Barges:**
  - **Where ore goes:** your stockpile in your own system, then your anchored
    station at that belt, then the barge's hold.
  - **Fleet line:** `Mine.to_station` makes it read "into station".
  - **When full:** with both the station and the hold full, the barge stops
    with "station full".
  - **Allies:** barges may now mine in allied systems too, as the plan said.
- **Collecting:**
  - **By hand:** `StationLoad { fleet, station, commodity, units }`. The freighter
    must be idle within 260 su of the station, and neither may be in a battle.
    It loads up to `units`.
  - **On a route:** a freight-route stop can be `FreightPort::Station { id }`.
    It only loads, straight from the station's hold, and waits there with
    "Waiting for ore at the station" until the manifest is aboard.
- **System changes hands:**
  - **A rival claims it:** the barges stop with "claimed" and the station stays
    anchored, idle.
  - **It becomes your colony:** the station unloads into the new stockpile as
    far as storage allows (`StationReleased`), then comes free.
- **Destroyed:** a destroyed station's ore is lost with it.
- **Client:**
  - **Belt panel:** your station there, with its hold and an Open button, or an
    "Anchor station" button for each free station.
  - **Fleet panel:** a Station section with the anchor status, the hold, the
    number of barges unloading, Unanchor, and "Load N Ore into …" for each
    freighter beside it.
  - **Map:** with a station selected, clicking a charted belt system stages
    AnchorStation.
  - **Route editor:** stations appear as stops, load-only.
  - **System View:** your anchored station and the barges working its belt sit
    on the band ("belt" placement).
  - **Mobile:** the same rows and buttons.
  - **Timeline:** "Station anchored at …", "Station unanchored …", "… is your
    colony: the station unloaded …", and "Loaded N … from your station."
- **Protocol v46.** No galaxy reset is needed for the station itself.

**Built Oct 10 2026: rare materials put to use (phase 5)**

Every use swaps the rare material in at equal base value, so no hull, module,
factory basket or research programme costs more credits than before. The
combat-cost pins (`build.rs`, `combat_balance.rs`) hold unchanged.

- **Iridium (90):**
  - **Ships of the line:** Battleship 13, Dreadnought 26, Titan 65, each in
    place of Alloys (13 Iridium for 45 Alloys). About 5% of the hull's value.
    Lighter hulls take none.
  - **Drive Works:** 0.1 Iridium per Drive Assembly, in place of 0.18
    Composites. The basket is still 179 against 215.
  - **Orbital Assembly** (the colony project that also makes Drive
    Assemblies): 0.012 Iridium a second, in place of 0.02 Composites.
- **Helium-3 (75):** capital fuel.
  - At a colony dock, a fleet's ships of the line (`is_siege_anchor`:
    Battleship and up) fill their share of the tank, by hull mass, from
    Helium-3 first. One unit fills 6 of Fuel (`fuel::HELIUM3_FUEL_EQUIV`), a
    little cheaper than Fuel at base prices. Fuel covers the rest.
  - Hub bunkerage stays Fuel. Nothing requires Helium-3.
  - *Open:* this is the low-risk version. The stronger ones are Helium-3 as
    required capital fuel, or a reactor upkeep.
- **Exotic Matter (120):**
  - **Research:** from Tier V, every basket takes 0.001 Exotic Matter per
    tier above IV per rate-second, in place of 0.0012 Rare Elements. A Tier V
    programme draws about 605 over its 168 research-hours.
  - **Special modules:** Prismatic Lance (Precision Components 4, Armaments
    8, Silicates 6, Exotic Matter 2) and Interdiction Field (Precision
    Components 3, Electronics 5, Machinery 6, Exotic Matter 2).
- **Supply:** the Hub sells all three on the thin book (600 at once, refilling
  2 a second), so a corporation without a pocket buys them. The galaxy's
  pockets hold about 8,900 Iridium, 8,000 Helium-3 and 1,300 Exotic Matter per
  4-player galaxy (`belt_report`), so the Hub stays the main source of Exotic
  Matter.
- **Research panel:** each catalog entry now carries its goods basket
  (`ProgrammeInfo.supplies`, protocol v47). The desktop detail and the mobile
  card show "Supplies /h": goods per hour at research rate 1.
- **Tests:** `rare_materials_go_into_capitals_drives_special_modules_and_top_research`
  (uses and equal value) and `capitals_refuel_from_helium3_first_at_a_colony_dock`.

**Built Oct 10 2026: bots and the harness (phase 6)**

- **Counters:** `Corporation.mining` (`industry::MiningStats`) adds up ore by
  source (barges, belt colonies, planet colonies, pockets), prospecting
  passes, pockets found, strikes, and ore hauled from stations.
- **Bots** (`bots/belts.rs`), only once the charter is complete:
  - **Barges:** one for the home belt, three once a station is anchored. Each
    picks the best target by ore value, efficiency and crowding. It prospects
    first (two passes at home, one remote), then mines. A full barge goes
    home, and one at home unloads.
  - **Station:** with a barge working, a bot researches Belt Stations, builds
    a station and anchors it at the best unowned belt within 40k su.
  - **Hauling:** a freighter runs a repeating "Belt station run" (load at the
    station, unload at home). Other freight logic leaves it alone.
- **Harness:** `multi_seed_bot_balancing_report` prints a `belts={…}` line
  per bot per report: barges, stations and holds, credits an hour by activity
  at base prices, ore units, passes, finds, find rate, strikes and ore hauled.
- **Measured Oct 10 2026** (8 bots, seed 42, six sim-hours, after the bots'
  opening-stall fix; see the bot memory notes):
  - **Charter:** all eight bots finish it, the last by 300 minutes. Before the
    fix, none did in 14 hours.
  - **One barge on an ordinary belt** earns about 1.4–3.4k Cr/h at base
    prices (Olivine, Crystalline, Ferrite, Volatiles). A home's planet mining
    earns 24–38k Cr/h, so a lone barge adds about a tenth.
  - **A found pocket** pays 7–10k Cr/h: two bots found their home belt's
    Cuprite pocket and mined it.
  - **Prospecting:** 16 passes found 2 pockets (12.5%). Most passes went to
    the charter's remote belts, which mostly have none; the home belt's
    guaranteed pocket needs its two passes there.
  - **Strikes:** none, and no bot had built a station by six hours (Belt
    Stations research comes first).
  - **Power:** no brownouts; each home's Command Center plant covers its
    planet, and one bot built a second plant.
- **Balance pass, Oct 10 2026** (a full game-day, then a 12-hour check):
  - **Bots now prospect their own belt first.** Six of eight find the home
    pocket in one or two passes. Mining it dry (3,000 Cuprite, about 72k Cr)
    takes six to eight hours.
  - **Fixed:** a barge whose home deposit or pocket ran dry idled for the
    rest of the day; now it takes the belt's ore or another belt. Barges
    then earn about 6.6–8.4k Cr/h averaged over twelve hours.
  - **No rate change needed:** on the same deposit a barge cuts as fast as
    a tier I mine (`min(0.25, 3 × richness)` against `richness`), without
    workers. A lone barge at a remote belt spends about two thirds of its
    time hauling: that is what the station and a freighter in the fleet
    are for.
  - **Stations come late:** none in a full game-day. Belt Stations is a
    tier II programme (8 research-hours) and needs a Shipyard II; bots
    finished only 2–4 programmes a day.
  - **The real bottleneck is the bots' fuel.** By the end of the day five
    of eight bots had no Fuel at home despite 400k–1M Cr, importing it on
    Tiny Freighters 50 at a time. That stalls development (and so stations)
    and caused hours of brownouts.
- **Bot fuel supply fix, Oct 11 2026** (built and unit-tested; not yet
  measured in a harness run). What the day log showed, and the fix for each:
  - **Storage pressure took every market slot.** Full homes (one had 1,366
    Provisions) cleared room first, and production refilled it between slots,
    so Fuel was never bought; freighters came back from the Hub empty.
    - Clearing room now books the biggest pile first (up to 300).
    - **Fuel runs:** a freighter leaving the Hub fills its spare hold with the
      Fuel the home lacks, bought on the spot (the route's shopping budget).
      There is no market slot and no Authority lead time. A home short of Fuel
      sends an idle hull even with nothing to sell, unless holds already bound
      for the Hub cover it (never the charter's first-shipment Freighter).
  - **The reorder point sat below the critical line.** Outside a crisis bots
    wanted half the fuel target and reordered at half of that, so every refill
    started "critical", which holds all development. Now they hold the whole
    target and reorder at three quarters (critical stays at half).
  - **Research:** Bunkerage, then Freight Frames, come first (after a range
    blocker). Either satisfies the charter's research step.
  - **Bigger holds:** with Freight Frames, a fleet of Tiny Freighters gets one
    Small Freighter (150): three Tiny loads of Fuel per run.
  - **Fuel line:** a Volatile Harvester is worth building only with Volatiles
    at home, and a Fuel Refinery only beside one (refining imported Volatiles
    saves no freight). Neither is held by a fuel shortage any more. One bot sat
    all day on a stocked refinery that the critical-fuel gate blocked.
  - Not changed: the fuel target itself. Warships still count a full tank, so
    big navies (two bots) read critical with 400+ Fuel; the faster supply should
    reach their target. Check `critical_minutes` before touching it.
  - **Harness:** a `fuel=` line per bot (stock vs target, critical minutes,
    inbound, carried home, lacking, holds bound for the Hub, the two
    programmes, home Volatiles, harvester and refinery tiers, freighter hulls).

**The founding charter's barge lesson** (`world/founding_belts.rs`)

Charter v3 ([founding-charter.md](founding-charter.md)) makes the barge its
last chapter, **Reach**:

- **Build a Mining Barge** (`BuildBarge`).
- **Mine the belt** (`MineBelt`): done when the report that a barge is cutting
  ore at a belt away from home reaches the command center.
- **Bring the ore home** (`HaulOre`): done when the report that a barge unloaded
  or delivered ore arrives. "Founding charter fulfilled" then pays 800.

**Target:** the programme points at the belt the survey charted.

- The survey step charts one stop: the home's local stop that keeps a belt
  with ore (`founding_lesson_stop`).
- Each home's rocky stop, 18–33k su out, keeps one Stony or Metallic belt with
  ore (`local-stop-belt-v1` keyed stream). Sparse stars otherwise stay bare.
- A barge with a full hold comes home from there on one tank; beyond about
  34k su it cannot.
- **Fallbacks:** the nearest charted belt within sensor range, then the home
  belt. A home-system belt completes the haul step at once.

**Harness:** `opening_balance` plays Industry and Reach. The charter completes
at about 28 game-minutes of sim time.

- Numbers marked *derived* are arithmetic from today's constants; nothing in
  the code states them.
- The deep-space version (`docs/asteroid-mining-plan.md`) is parked: belts come
  first.
- Mining Outposts on barren planets are set aside for now.

## In one paragraph

Every galaxy rolls its own belts. A belt is a real part of its system, with a
type and ores, and it stays hidden until you discover it.

- **New ores.** Belts bring six ores that planets don't have.
  - Three mixed ores refine into goods the game already makes.
  - Three rare ores turn up only as pockets, and each refines into a new
    material for top-end builds.
- **Discovery.** A survey charts a system's belts. Prospecting a belt then
  hunts for hidden pockets of rarer ore. In belts, Titanium, Rare-metal and the
  three rare ores only ever turn up in pockets.
- **Mining in your own colony.** A Mining Complex on the belt, staffed by
  colonists, does the work.
- **Mining anywhere else.** Mining Barges, a new mining ship, do the work.
  - They unload into an anchored **Mining Belt Station** if you have one there;
    freighters collect from the station.
  - Otherwise they fill their own small hold, or that of a freighter flying
    with them.
- **Claims:** none yet. Selling them through the Authority comes later.

## How a belt gets mined

| The system is… | How you mine its belt |
| --- | --- |
| Yours (a colony) | A Mining Complex on the belt, staffed by colonists. Barges are optional and unload straight into the stockpile. No station is needed. |
| Unowned, or an ally's | Mining Barges. Without a station they fill their own hold or a freighter's. With a station anchored there, they unload into it and keep mining while freighters haul the ore away. |
| A rival's | Nothing in v1. |

## Ores

Planets keep today's five ores, which are tuned and locked in by tests. Belts
carry those ores (Biomass aside) plus six new ones, so the variety lives where
the discovery happens.

**Today's ores**

| Resource | Price | Refines into | In belts |
| --- | --- | --- | --- |
| Crystalline ore | 6 | Silicates, plus a Rare Elements trace | Stony belts |
| Ferrite ore | 8 | Alloys | Stony, Metallic and Shattered belts |
| Cuprite ore | 24 | Conductive Metals, plus Alloys | Metallic and Shattered belts; pockets in Stony belts |
| Titanium ore | 110 | Titanium, plus Alloys | pockets only, in Metallic and Shattered belts |
| Rare-metal ore | 1,700 | Rare Elements, plus Conductive Metals | pockets only, in Metallic and Shattered belts |
| Volatiles | 9 | Fuel; Polymers (with Biomass) | Icy belts |
| Biomass | 5 | Provisions; Polymers | never |

**New mixed ores.** These refine into goods the game already makes, so no new
production chains are needed.

| Ore | Refines into | Why you'd want it | Found in |
| --- | --- | --- | --- |
| Olivine | Silicates and Alloys | two common goods from one deposit | Stony belts |
| Bornite | Conductive Metals and Rare Elements | two of the three inputs Electronics needs | Metallic belts, frontier 0.25+ |
| Ilmenite | Titanium and Alloys, at lower purity | Titanium closer in than Titanium ore | Metallic belts, frontier 0.30+ |

**New rare ores.** These occur only as pockets, and each refines into a new
material.

| Ore | Refines into | Used for | Found in |
| --- | --- | --- | --- |
| Iridium ore | Iridium | capital hulls and drive assemblies | Metallic and Shattered pockets, frontier 0.60+ |
| Helium-3 ice | Helium-3 | high-energy fuel for capital ships; jump drives if jump fuel is switched back on | Icy pockets, frontier 0.55+ |
| Void crystal | Exotic Matter | top research and special modules, such as the Prismatic Lance and Interdiction Field | Shattered-world pockets only, frontier 0.65+ |

**Rules for the new ores**

- **Prices** follow today's rule: raw ore sells for about 70% of what it
  refines into.
- **Income per second of mining** stays between Crystalline's and Rare-metal's.
  The rare ores sit at the top beside Rare-metal, and rarer ores are denser, as
  today.
- **Refining.** All six refine in the Smelter, through its existing recipe
  choice. Helium-3 ice could move to the Fuel Refinery later.
- **Markets.** The rare ores and their materials trade on thin books, like
  Rare-metal ore and Rare Elements.
- **Frontier windows** apply to every ore, pockets included, so scarcity holds.
- **Uses.** The rare materials have none until phase 5 adds them to top-end
  recipes in a balance pass. Until then they can be refined and sold.

## Belt types

| Type | Base ores | Hidden pockets | Where |
| --- | --- | --- | --- |
| **Stony** | Crystalline, Olivine, Ferrite | Cuprite, Bornite | Anywhere; the commonest type near the core |
| **Metallic** | Ferrite, Cuprite, Bornite, Ilmenite | Titanium (frontier 0.45+), Iridium (0.60+), Rare-metal (0.70+) | Mid-rim outward |
| **Icy** | Volatiles | Rich Volatiles; Helium-3 ice (frontier 0.55+) | Anywhere; commoner at the rim |
| **Shattered world** (rare) | A richer Metallic mix | Titanium, Iridium, Rare-metal and Void crystal, each within its frontier window; more pockets than other belts | Frontier 0.45+, a few per galaxy |

Each type gets its own chunk sprite in the System View; the prompts are in
`docs/asteroid-belt-art-prompts.md`. Until those arrive, the existing chunk is
tinted per type: grey-brown, steel, blue-white or rust.

## Facts that shape the plan

- **No positions inside a system.** Bodies have no orbit or position in the
  sim; every body sits at the star. A belt can be a body without any
  intra-system movement. Barges and stations work within 1,500 su of the star,
  as skimmers do.
- **The workforce is pooled per system.** A Mining Complex on an uninhabited
  body is staffed by people living anywhere in the system (`body.rs:4-9`).
- **Surveys already hide things until they're done.** Deposits and geology are
  survey-gated. A Scout dwells 20 s, and the report comes home by light. Only
  your home is surveyed at the start.
- **The client's belts today are decoration.** They're drawn from the system id
  alone, so every galaxy would show the same ones. They are replaced by real
  belts, drawn only once charted.
- **A fleet's ships share one hold and move at the slowest ship's speed.**
- **Cargo only changes fleets by merging,** which works at your own systems and
  into an empty hold. Stations need their own unload and load rules.
- **Upkeep doesn't limit miners.** Ships cost only Provisions, so ship mining
  needs a cap of its own.
- **The Authority already runs a legal system.** It has standing, citations
  that reach the Hub by light, penalty bands and enforcement fleets.
- **Strip Rigs research ("+25% extraction on moons") exists, but nothing reads
  it.**

## Design

### 1. A belt is a body, rolled per galaxy

- **Kind.** Add `BodyKind::AsteroidBelt` (wire `asteroid_belt`), plus
  `belt: Option<Belt { kind, radius, width }>` on the body.
- **Random per galaxy.** Every roll comes from a keyed stream,
  `Rng::keyed_id(seed, "belts-v1", system)`. Two galaxies never share a layout.
  The roster stream gets no extra draws (the ore-ladder rule), so no star or
  planet moves.
- **How many.**
  - Per system: 40% none, 50% one, 10% two (tunable).
  - The belt type is weighted by frontier, using the table above.
  - The radius and width are rolled for the picture.
- **Base deposits.**
  - One or two from the type's list, never a scarce ore outside its frontier
    window.
  - Lean: the planet deposit formula × `BELT_RICHNESS_MULT` 0.6.
  - Reserves follow `reserves_for`.
- **Hidden pockets.** Each belt has 0–2 (40% none, 45% one, 15% two). A
  Shattered belt has 1–3.
  - A pocket is a small, finite, rich deposit of the type's pocket ore.
  - Richness is the formula × 1.0–1.5, with no lean factor.
  - It holds a quarter of a planet seam's reserves.
- **Ids** follow the moons. **Names:** "Veles Belt", "Veles Outer Belt".
- **Profile.** Always uninhabitable, with rolled geology and no special.
- **Homes.** Every home gets the same belt: Stony, Crystalline at richness 0.25,
  with one hidden Cuprite pocket.
  - It's fair to every player, and it teaches prospecting early.
  - 0.25 stays below the home Ferrite floor of 0.315, so the tutorial's first
    mine still lands on the Ferrite world.
- **Exploration stars** roll belts with no base deposits. Pockets are rare
  there.
- **Slots.** Resource slots only. Outposts refuse belts for now.
- **Kind checks to review.** There are 2 exhaustive matches and about 25
  `==`/`matches!` checks. The ones that matter most:
  - `site_for` must skip belts;
  - the size table, special eligibility and the slot bases;
  - the client's `visualKindFor`.

### 2. Discovery

1. **Charting.** A belt is uncharted until your corporation surveys its system.
   - Until then it isn't in your System View, your Worlds list or your view.
     The server withholds the belt bodies.
   - The existing Scout survey charts it. The report adds, for example, "Belt
     discovered: Metallic belt (Fair) · Ferrite ×0.24, Cuprite ×0.19".
   - Allies receive surveys through the existing relay. Your own colonies are
     always charted.
2. **Prospecting.** A Scout or a Mining Barge prospects a charted belt.
   - The order is `ProspectBelt { fleet_id, system_id, body_id }`: a 60 s dwell
     within 1,500 su, loud at ×1.5 like a survey.
   - Each pass has a `POCKET_FIND_CHANCE` 50% chance of finding each hidden
     pocket. The roll comes from a keyed stream per (belt, corporation, pass).
   - The find comes home by light and is private. Rivals must prospect for
     themselves.
3. **Strikes.** Barges also find pockets slowly while they mine: about a 10%
   chance per barge-hour (tunable). Mining is slow prospecting.

- **What a pocket is in the data.** It's a deposit marked hidden, with a
  per-corporation `found_by` set. Barges and Mining Complexes mine only the
  pockets their corporation has found.
- **Knowledge is the edge.** A rich pocket found before rivals learn of it is
  the reason to buy a belt charter later.

### 3. Your own systems: a Mining Complex on the belt

- **Today's code covers it:**
  - the build gate with a matching deposit;
  - workers from the system pool and Geologists;
  - tiers to IV (VI with research);
  - the storage cap and suspension;
  - the factor chain and research credit.
- **Pockets you've found** join the complex's output, because a complex works
  every deposit on its body.
- **Strip Rigs** becomes "+25% extraction on moons and belts".
- **Barges are optional here.** They mine on top of the complex and unload into
  the stockpile.

### 4. The Mining Barge

A new hull, the miner of the belt line. It is like EVE's Retriever or Procurer.

- **Role:** it mines and prospects. It is a poor hauler and has no guns.
- **Starting stats:**
  - mines `BARGE_RATE` 0.25 bulk units a second;
  - hold 150;
  - speed 30;
  - hull mass 3,500;
  - crew 15.
- **Build:** a Shipyard I. Starting recipe: Alloys 40, Machinery 20, Polymers 12,
  Electronics 6.
- **Research:** a new tier-1 Materials node, Belt Mining. *(Dropped in the
  first cut so the founding charter can teach the barge.)*
- **Order:** `MineBelt { fleet_id, system_id, body_id, ore }`, light-delayed.
  - **Valid when:**
    - the system is yours, unowned or an ally's;
    - it isn't a pirate enclave;
    - the belt is charted;
    - the ore is a base ore or a pocket you have found.
  - The fleet works within 1,500 su. A new course ends the job.
- **Rate:** 0.25 bulk/s × Deep Bores × Strip Rigs, converted with
  `ore_bulk_ratio`. Whole units go in and the fraction carries over. Finite
  reserves deplete.
- **The shared cap.** All barges on one deposit share at most `SHIP_YIELD_MULT`
  3 × its natural rate, whatever corporation they belong to.
  - That's about three barges per ore on a lean belt.
  - It's the limit cheap upkeep can't provide, and it makes corporations
    compete.
- **Where the ore goes**, in order:
  1. your stockpile, in your own system;
  2. your anchored station at that belt;
  3. the fleet's own hold, including any freighter with it.
- **Status:**
  - travelling, mining, full or depleted;
  - **unsafe** pauses the job during a blockade or a nearby battle;
  - **claimed** stops it when another corporation takes the system.
- **Loud:** ×1.5 while mining.
- **Research credit:** mined units count toward the Deep Crust school.
- **Raids:** barges count as freighters for raids.

### 5. The Mining Belt Station

A stationary unit with a large ore hold, anchored at one belt. It's like EVE's
Rorqual with its industrial core running.

- **It's a ship that anchors.** It is built as a ship hull, so it inherits:
  - the build queue;
  - travel and fuel;
  - a hold;
  - damage;
  - visibility to rivals;
  - a 3D model;
  - the fleet panel.

  The only new state is anchoring.
- **Starting stats:**
  - ore hold 3,000;
  - speed 15;
  - tough hull, no guns;
  - crew 60.
- **Build:** a Shipyard II. Starting recipe: Alloys 160, Machinery 50,
  Electronics 25, Polymers 30.
- **Research:** a new tier-2 Materials node, Belt Stations.
- **Anchoring.** `AnchorStation { fleet_id, system_id, body_id }`.
  - It flies to the star and anchors within 1,500 su in 60 s.
  - While anchored it can't move, merge or split.
  - `UnanchorStation` takes 60 s.
- **Where it may anchor:**
  - Charted belts in unowned or allied systems.
  - Not a pirate enclave, not a rival's system, and not your own.
  - One per corporation per belt; rivals may anchor beside you.
- **Barges unload** into it continuously, up to its free space. When it is
  full, they fill their own holds and stop with "station full".
- **Freighters collect.**
  - **By hand:** `StationLoad { fleet_id, station_id, commodity, units }`, for a
    freighter idle within 260 su of the station.
  - **Automatically:** a freight-route stop can be a station
    (`FreightPort::Station`).
- **Risk.** It's a big, fixed target that every sensor in range sees, so
  warships guard it. In v1 its ore is lost if it is destroyed.
- **When the system changes hands:**
  - **A rival claims it:** barges stop and the station idles. Your freighters
    can still empty it.
  - **You colonize it:** it unloads into the new stockpile and moves on.

### 6. Hauling and range

- **Fleet speed.** A fleet moves at its slowest ship's speed. A station lets
  freighters shuttle at their own speed instead of barge pace.
- **Full-hold range at Full speed** (derived; an empty hull of any size reaches
  about 152k su):

  | Hull | Range |
  | --- | --- |
  | Convoy | ~43k su |
  | Large | ~34k su |
  | Heavy | ~25k su |
  | Bulk | ~19k su |

- **Choosing a hauler.** Big haulers suit belts near home; distant belts need
  Convoys or Economy cruise.
- **Fuel cost.** Hauling Cuprite 40k su costs about a quarter of its value in
  fuel.

### 7. Claims, later: Authority Belt Charters

*Skipped: Peter decided against belt claims (Oct 10 2026). Kept for the
record.*

None in v1. Later, belt claims can be bought from the Authority and plug into
its legal system.

- **The charter.** A Belt Charter is bought at the Hub: a lease on one belt for
  a set time, priced from the belt's value with the old claim-cost formula.
- **Trespass.** Mining a belt someone else has chartered becomes a citation,
  like raiding Authority freight.
  - The Authority learns of it by light.
  - The trespasser loses standing.
  - The existing bands escalate in turn: tariffs, then suspended freight, then a
    revoked exchange licence, then enforcement fleets.
- **The frontier is lawless by itself.** News of trespass far out reaches the
  Hub late, so charters near the core are strong and frontier ones weak.
- **Defence is still yours.** The Authority punishes trespassers; it doesn't
  guard your station.

### 8. Economics

*Derived.* The example is a lean Cuprite deposit with a natural rate of 0.226
bulk/s, so the ship cap is 0.678. Base prices, Fuel at 14, Full speed, no
research, no pockets.

| Case | Cuprite/h | Gross/h | Fuel/h | Net/h |
| --- | --- | --- | --- | --- |
| Your colony: T1 Mining Complex on the belt | 542 | 13.0k | – | 13.0k, needs 1,000 workers |
| Your colony: T4 Mining Complex on the belt | 3,254 | 78k | – | 78k, needs 4,000 workers |
| One barge plus a Convoy in one fleet, belt 40k su from home | 517 | 12.4k | 3.3k | **9.1k** |
| Station plus three barges, a Convoy shuttling 40k su | 1,627 | 39.0k | 9.2k | **29.8k** |
| The same operation, belt 20k su from home | 1,627 | 39.0k | 4.6k | **34.4k** |

- **A single barge earns about what a T1 mine does,** with no workers.
- **A station operation earns two to three times a T1 mine.**
- **A T4 complex makes 2.5 times a station operation,** so settling is still the
  way to scale.
- **Pockets are the upside.** A found Titanium pocket pays several times a
  Cuprite base deposit, while it lasts.

### 9. UI (no new rings)

- **System View.**
  - Only charted belts are drawn, tinted by type.
  - Found pockets show as glints on the band.
  - Belts can be hovered and clicked, through an annulus hit test after ships
    and worlds. The hover reads: "Veles Belt · Metallic · Ferrite ×0.24,
    Cuprite ×0.19 · 1 pocket found".
  - Stations sit on the band with their barges.
- **Survey report:** "Belt discovered: …". A pocket find is a private timeline
  entry.
- **Belt panel** (world route).
  - The gas giant's orbital scene, with a belt strip built from the chunk
    sprite.
  - The base ores, the pockets found ("Titanium ×0.9 · 1,800 left") and
    "Unprospected" or "Prospected 3×".
  - A Ship mining section:
    - Prospect with a Scout or barge;
    - Mine with a barge;
    - your station.
- **Station panel:**
  - the hold by ore, the fill rate and the time until full;
  - the barges working there;
  - Anchor or Unanchor;
  - "Load into …".
- **Barge fleet line,** for example: "Mining Cuprite at Veles Belt · 0.17/s ·
  unloading to station".
- **Freight routes:** a station can be a stop.
- **Worlds tab:** a belt row for each charted belt.
- **Galaxy map:** a surveyed system's hover adds its belts.
- **Mobile:** the same.

## Phases

0. **Dry run.** A throwaway report rolls belts for a few seeds and prints:
   - belts and types per galaxy;
   - pockets;
   - ore added compared with planets;
   - richness-band shifts;
   - the colony-discovery band (target 10–15%);
   - the economics.

   It sets the counts, the type weights, `BELT_RICHNESS_MULT`, `BARGE_RATE` and
   `SHIP_YIELD_MULT`.
1. **Random belts, charted by survey, mined by your colonies.**
   - The belt body, the types, generation and the home belt.
   - The three mixed ores: commodities, prices, Smelter recipes and icons.
   - Withholding uncharted belts.
   - The survey report line.
   - Resource-only slots, the kind-check review and Strip Rigs.
   - The client draws charted belts from data, tinted by type, with hover,
     click, the belt panel and the Worlds row.
   - A galaxy reset.

   Done when a survey reveals a new system's belts, and your home's belt takes
   a staffed Mining Complex that produces.
2. **Prospecting and pockets.**
   - `ProspectBelt` for Scouts, with the find chance.
   - Per-corporation `found_by`.
   - Private reports.
   - Pocket glints and the panel rows.
   - The home pocket.
   - The three rare ores and their materials: commodities, prices, thin books,
     Smelter recipes and icons. They can be refined and sold, nothing more yet.

   Done when prospecting your home belt finds its Cuprite pocket and your
   Mining Complex starts mining it, while a rival who hasn't prospected sees
   nothing.
3. **Mining Barge.**
   - The hull, with its approved LOD2 model and model-derived sprite.
   - Belt Mining research.
   - `MineBelt` and `FleetIndustry::Mine`, with the shared cap, the statuses,
     unloading in your own system, hold pooling, loudness and research credit.
   - Barges can prospect, and they strike pockets while mining.
   - The Mine buttons and the fleet line.
4. **Mining Belt Station.**
   - The hull, with its approved Mining Belt Station LOD2 and baked art.
   - Belt Stations research.
   - Anchoring, barge unloading, `StationLoad`, the route stop, the station
     panel and its place in the System View.
   - *Built Oct 10 2026; see the status above.*
5. **Rare materials put to use.**
   - Iridium joins capital hull and drive-assembly recipes.
   - Helium-3 becomes capital fuel.
   - Exotic Matter goes into top research and the special modules.
   - A balance pass on capital ships, the combat-cost tests and research
     costs.
   - *Built Oct 10 2026, all at equal value; see the status above.*
6. **Bots and the harness.**
   - Bots survey, prospect, and run barges with a station at their best belt.
   - The harness logs credits per hour for each activity, ore supply and how
     often pockets are found.
   - *Built Oct 10 2026; measured after the bots' opening-stall fix (see the
     status above).*
7. **Authority Belt Charters.** *Skipped (Peter, Oct 10 2026).*
8. **Later:**
   - station upgrades: refuelling barges, a mining boost, refining on site;
   - a light Prospector (EVE's Venture) and an Ore Tender (EVE's Orca);
   - raiding a station's hold;
   - poaching at war;
   - belt stage art.

## Tests

**Sim:**

- **Generation:**
  - belts are rolled from the galaxy seed, so the same seed gives the same
    belts and a new seed different ones;
  - no roster or star moves;
  - type windows and frontier windows hold, for pockets too;
  - every home has the same belt and pocket;
  - ids follow the moons;
  - belts have Resource slots only and are never habitable;
  - outposts refuse belts.
- **Discovery:**
  - an unsurveyed corporation's view has no belt bodies, and a survey adds them;
  - prospecting finds pockets at the set chance, deterministically per pass;
  - finds are private;
  - strikes happen while mining;
  - nobody mines a pocket they haven't found.
- **Colonies:**
  - a belt Mining Complex is staffed from the system pool, extracts, and picks
    up found pockets;
  - `site_for` puts only mines on belts;
  - the founding mine still lands on the Ferrite world.
- **Barges:**
  - the rate, and the cap split across corporations;
  - whole units, with the fraction carried over;
  - finite reserves deplete;
  - the unloading order;
  - the statuses;
  - the ×1.5 signature;
  - rival systems are refused.
- **Station:**
  - anchoring and unanchoring;
  - where it may anchor;
  - barges unload into it;
  - `StationLoad` and the route stop;
  - a rival's claim idles it;
  - when destroyed, its ore is lost.
- **Strip Rigs** applies to belts and moons.
- **Ores:**
  - the ladder order and the 70% price rule hold for every ore;
  - income per mining second stays in today's spread;
  - converter baskets never net-add units, except for dense ores;
  - the rare books are thin;
  - each new ore refines into its listed goods.

**Client:**

- Only charted belts are drawn, tinted by type, with pocket glints.
- Hover and click.
- The belt panel's discovery rows and actions.
- The barge line and the station panel.

## Decisions still open

1. **Uncharted belts.**
   - Recommended: hidden until surveyed.
   - Alternative: visible as dust, with contents unknown.
2. **Pocket finding.**
   - Recommended: a 50% chance per pass, plus strikes while mining.
   - Alternative: guaranteed after a set number of passes.
3. **Home belt.**
   - Recommended: the same Stony belt with one hidden Cuprite pocket.
   - Alternatives: no pocket, or no home belt.
4. **Rare materials.**
   - Recommended: the three above, with the uses listed.
   - Alternative: fewer rare ores, or other uses.
5. **Station:**
   - anchors in unowned and allied systems;
   - its ore is lost when destroyed;
   - it is built at a shipyard. A Builder ship constructing it on site is the
     alternative.

The starting numbers are for phase 0 to confirm.

## Art

Every prompt is in `docs/asteroid-belt-art-prompts.md`, ready to paste into
ChatGPT:

- **Icons:** nine ore and material icons in the current ore-icon style.
- **Belt chunk sprites:** Metallic, Icy and Shattered, plus an optional Stony
  regenerated to match the set.
- **Hulls:** the Mining Barge and the Mining Belt Station, each a top-down
  sprite and then a five-view reference sheet. They go through the existing
  Python reconstruction to a GLB (Y up, −Z forward).
  - Barge: about 5–8k triangles, with sockets at the arm tips for later mining
    beams.
  - Station: about 10–16k triangles, with sockets at the barge berths and the
    hauler berth.

The Mining Barge and Mining Belt Station are integrated with their approved
models and baked art. The remaining stand-in is the belt chunk sprite, tinted
per type.

**Later:**

- a belt stage painting per type;
- an orbital look for the Mining Complex on a belt;
- a belt icon for the Worlds rows.
