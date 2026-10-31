# Stellar Syndicates: raiding and plundering vs producing (working tree, Oct 28 2026)

Paths are relative to `/Users/peter/projects/stellar-syndicates` unless absolute. **BUILT** means it is in the sim code in the working tree (much of it uncommitted in git). **PLANNED** means it exists only in a doc or memory note. Prices are Market Hub base prices (`crates/sim/src/market.rs:141-201`). Production runs at `SIM_PACING` 1.0 (sim second = wall second) ([crates/server/src/game_loop.rs:42](crates/server/src/game_loop.rs#L42)). The shipped battle timescale is 45 s ([crates/sim/src/config.rs:139-141](crates/sim/src/config.rs#L139), asserted at [config.rs:247](crates/sim/src/config.rs#L247)); the 2,700 s "production preset" exists only in a test ([config.rs:271-273](crates/sim/src/config.rs#L271)).

## 1. Can a player raid or plunder another player's goods today, how, how much, and where does it go?

### Takeaway
Yes, by three BUILT routes. (1) **Convoy raid:** an Intercept on an unescorted freighter. No war is needed, but the victim gets a 12 h right of reprisal. The raider keeps the whole manifest only if it destroys every freighter in about 16 s, and only as much as its own freighter holds can carry. (2) **Blockade plunder:** needs a war or a reprisal and the defense beaten first. It strips the system stockpile at 1.5 units/s per held system, most valuable good first, never below 40 of any good, into the besieger's freighter holds. (3) **Siege capture:** colonies only, never homes. It takes 6 h × (1 + 0.3 × garrison tier), then the whole stockpile and half-tier structures change hands. The Market Hub warehouse can never be raided.

### Cited Findings
- **Design intent (BUILT):** stockpile theft deliberately requires a HELD blockade, not a hit-and-run raid verb. The code comment says a raid verb against a stockpile "would be strictly better than chasing convoys", so "convoy raiding [stays] the fast option and stockpile theft the committed one" — [crates/sim/src/world.rs:751-764](crates/sim/src/world.rs#L751)
- **Convoy raid (Intercept = `Command::CommitRaid`):** a contact on a freighter hull (or the Authority's Freighter) is a cargo RAID by default, unless the order is an explicit Attack. An Attack destroys the convoy and its cargo is lost — [world.rs:5103-5109](crates/sim/src/world.rs#L5103)
- A convoy covered by a friendly Corvette screen or a covering platform becomes a battle, not a raid ("fighting the escort is a battle, not a cargo raid") — [world.rs:5110-5140](crates/sim/src/world.rs#L5110)
- **Raid time cap:** a raid engagement ends after `RAID_CAP_FRAC` = 0.35 × `battle_target_secs` — [crates/sim/src/combat.rs:72-77](crates/sim/src/combat.rs#L72); applied at [world.rs:5904-5912](crates/sim/src/world.rs#L5904). With the shipped 45 s that is 15.75 s.
- **Seizure rule:** loot is taken only when a defender convoy loses every freighter hull that tick. The attacker then "loots its complete manifest" — [world.rs:5666-5683](crates/sim/src/world.rs#L5666)
- **Where convoy loot goes (§holds, Oct 27 2026):** into the seizer's own attacking fleets' holds, in order, as room allows. "The rest is lost with the wreck." Pirates have unlimited room — [world.rs:13873-13895](crates/sim/src/world.rs#L13873); [world.rs:5720-5734](crates/sim/src/world.rs#L5720); [docs/blockades.md:109-112](docs/blockades.md#L109)
- **Boarding Parties (Corsair V research):** the stripped convoy's freighters join the raider as a prize fleet with the cargo aboard, at 25% hull and 25% fuel — [world.rs:5710-5755](crates/sim/src/world.rs#L5710); [world.rs:266-267](crates/sim/src/world.rs#L266) (`PRIZE_HULL_FRAC` 0.25); [world.rs:13897-13915](crates/sim/src/world.rs#L13897); [crates/sim/src/research.rs:1880-1888](crates/sim/src/research.rs#L1880)
- A stripped convoy counts as a successful raid for Corsair research gating — [world.rs:5739-5740](crates/sim/src/world.rs#L5739)
- **Blockade order:** needs an interceptor in the fleet and a system another corporation owns. It also needs a declared war or a live right of reprisal, with no pact protecting the owner. The order is rechecked every tick — [docs/blockades.md:11-17](docs/blockades.md#L11); `blockade_allowed` at [world.rs:6687-6706](crates/sim/src/world.rs#L6687); order handler at [world.rs:11274-11298](crates/sim/src/world.rs#L11274)
- **Stages:**
  - **Contested** while the system's defense stands: a station hardpoint firing, warships within 1,300 su of platform cover, or fed allied garrisons. "Nothing is stripped and no clock runs."
  - **Held:** deliveries wait outside a 1,200 su ring, and the Hub refuses bookings to and from the system.
  - **Siege:** held, and not the owner's home.
  — [docs/blockades.md:25-55](docs/blockades.md#L25)
- **Plunder rate and rules (held stage):**
  - `PLUNDER_UNITS_PER_SEC` = 1.5 ([world.rs:765-767](crates/sim/src/world.rs#L765)), × the RaidSteal research modifier (Prize Holds = 1.50) ([world.rs:8132-8133](crates/sim/src/world.rs#L8132); [research.rs:1840-1848](crates/sim/src/research.rs#L1840)).
  - It takes the MOST VALUABLE good by base price that stands above the floor ([world.rs:8138-8150](crates/sim/src/world.rs#L8138)).
  - Each tick it is bounded by the rate, the hold's room and stock − 40 ([world.rs:8154-8159](crates/sim/src/world.rs#L8154)).
- **Per-good floor:** `PLUNDER_FLOOR_UNITS` = 40, "a colony always keeps a working floor" — [world.rs:768-772](crates/sim/src/world.rs#L768)
- **One taker per held system per tick:**
  - The besieger's fleets load in id order. The first fleet with room is the taker ([world.rs:8095-8119](crates/sim/src/world.rs#L8095)), and loot goes into its hold ([world.rs:8166-8177](crates/sim/src/world.rs#L8166)).
  - Capacity is `plunder_capacity`: freighter holds only for corporations, at least 60 a fleet for pirates ([world.rs:773-783](crates/sim/src/world.rs#L773)).
  - "Warships carry none" — [docs/blockades.md:39-44](docs/blockades.md#L39)
- **The Authority** blockades but never strips — [world.rs:8100-8102](crates/sim/src/world.rs#L8100); [docs/blockades.md:38](docs/blockades.md#L38)
- **Victim's view of plunder:** a `SystemPlundered` event, one timeline line a minute with the haul — [docs/blockades.md:156-162](docs/blockades.md#L156); [world.rs:8178-8193](crates/sim/src/world.rs#L8178)
- **Blockade running:**
  - Catch chance per besieger hull on station: interceptor 30%, corvette/destroyer 10%, cruiser 5%, battleship/dreadnought/titan 3%, Stasis Web 10%. "Three interceptors catch 66% of runners."
  - A caught freighter gets a cargo raid "with the usual short cap" from the blockade's lowest-id free ship.
  — [docs/blockades.md:85-112](docs/blockades.md#L85)
- **Siege capture:**
  - Clock: 6 h base (`siege_capture_secs`) × (1 + 0.3 × effective garrison), ÷ 1.25 with a battleship-class hull on station. Marines from an idle troop-transport fleet take the system. "A home can be blockaded but never captured." — [docs/blockades.md:47-52](docs/blockades.md#L47); [config.rs:126-128](crates/sim/src/config.rs#L126); [world.rs:790-798](crates/sim/src/world.rs#L790)
  - On capture the stockpile stays on the system as plunder (now the captor's), every structure tier is halved, and population stays — [world.rs:14814-14892](crates/sim/src/world.rs#L14814)
- **Loot delivery:** loot unloads at home or at the Hub like any hull's cargo — [docs/blockades.md:131](docs/blockades.md#L131)

### Inferences
- **Per-action caps:**
  - Blockade plunder at a held system: at most 1.5 units/s = 90/min = 5,400/h. With Prize Holds: 2.25/s = 8,100/h. Bringing more freighters raises the cap, not the rate.
  - In practice the cap is the victim's stock above 40 per good (see §6 storage caps: about 1,300–2,000 units total at a charter-end home).
  - A convoy raid yields the destroyed convoy's whole manifest, capped by the raider's freighter room.
- A convoy raid with no freighter in the raiding fleet seizes nothing for a corporation: warships carry no holds since Oct 27 2026. The victim still loses the cargo, which sinks.
- The Market Hub warehouse (the second goods store) is outside every plunder path: plunder reads only `sys.stockpile`, and the Hub is never blockaded. Goods parked at the Hub are therefore safe from raiding.

### Gaps
- I did not trace partial convoy losses, where only some of the freighters die before the 15.75 s cap. The code seizes only when all freighter hulls die that tick, so partial kills appear to yield nothing, but I did not confirm what happens to the surviving cargo.
- I found no measured data (harness or live) on how often players actually raid or blockade each other, or what they take.
- I did not check whether bots ever raid or blockade players.

## 2. Ships involved: capacities, speeds, costs, fuel, build times

### Takeaway
The Combat Freighter (BUILT Oct 27 2026; deployed to production Oct 28 per memory) is the dedicated raiding hull:
- 80-unit loot-only hold (120 with Prize Holds), speed 70, unarmed and dark.
- About 1,970 Cr of goods, 20 s at Shipyard II, no research.
- It must ride with warships (Interceptors: 100 speed, 2 slots, no hold).

Fuel is mass × distance. A loaded return leg on Deuterium full warp can cost more than an ore cargo is worth.

### Cited Findings
- **Combat Freighter stats** — table at [docs/blockades.md:113-138](docs/blockades.md#L113):
  - Hold 80, loot only ([crates/sim/src/ship.rs:188-201](crates/sim/src/ship.rs#L188)).
  - Speed 70 ([ship.rs:262-264](crates/sim/src/ship.rs#L262)).
  - Mass 2,000 ([ship.rs:222](crates/sim/src/ship.rs#L222)).
  - Attack 0 / defense 2 ([ship.rs:363](crates/sim/src/ship.rs#L363), [ship.rs:392](crates/sim/src/ship.rs#L392)).
  - 1 utility slot ([ship.rs:457](crates/sim/src/ship.rs#L457)).
  - Needs Shipyard II ([crates/sim/src/build.rs:1556](crates/sim/src/build.rs#L1556)).
- **Combat Freighter recipe:** 25 Alloys, 10 Machinery, 6 Electronics (Semiconductors), 5 Armaments, 1 Warp Core; 20 s; no research — [build.rs:795-807](crates/sim/src/build.rs#L795)
- **Loot-only hold:**
  - `is_loot_only` ([ship.rs:175-180](crates/sim/src/ship.rs#L175)).
  - `cargo_capacity` counts it; `trade_capacity` (docks, markets, routes, mining) does not ([ship.rs:2065-2080](crates/sim/src/ship.rs#L2065)).
  - Prize Holds widens it via `fuel_research.prize_hold` ([ship.rs:2092-2101](crates/sim/src/ship.rs#L2092)).
  - "Prize Holds makes the hold 120; Cargo Pods double it." It does not broadcast, has no sensors, and is raided like any freighter — [docs/blockades.md:126-136](docs/blockades.md#L126)
- **Interceptor (`ShipKind::Raider`):**
  - Speed 100 ([ship.rs:255](crates/sim/src/ship.rs#L255)); mass 200 ([ship.rs:215](crates/sim/src/ship.rs#L215)); attack 3 / defense 2 ([ship.rs:353](crates/sim/src/ship.rs#L353), [ship.rs:380](crates/sim/src/ship.rs#L380)); no hold ([ship.rs:199](crates/sim/src/ship.rs#L199)).
  - Recipe 20 Alloys, 12 Electronics, 15 Armaments, 1 Warp Core; 10 s ([build.rs:767-776](crates/sim/src/build.rs#L767)).
  - Shipyard I builds it ([docs/founding-charter.md:214-217](docs/founding-charter.md#L214)).
- **Freighter holds:** Tiny 50, Small 150, Medium (Convoy) 400, Large 1,000, Heavy 2,500, Bulk 6,000 — [ship.rs:188-201](crates/sim/src/ship.rs#L188)
- **Freighter speeds:** 40 / 40 / 40 / 38 / 34 / 30 — [ship.rs:247-253](crates/sim/src/ship.rs#L247)
- **Freighter recipes:**
  - Tiny: 20 Alloys, 8 Machinery, 8 Polymers, 1 Warp Core; 12 s.
  - Small: 45 Alloys, 16 Machinery, 16 Polymers, 1 Warp Core; 24 s.
  - Large: 180 Alloys, 45 Machinery, 25 Electronics, 12 Hull Sections, 3 Warp Cores; 100 s.
  - Medium: 90 Alloys, 28 Machinery, 20 Polymers, 10 Electronics, 2 Warp Cores; 50 s.
  — [build.rs:730-766](crates/sim/src/build.rs#L730)
- **Corvette:** 25 Alloys, 12 Electronics, 12 Armaments, 1 Warp Core; 14 s ([build.rs:829-838](crates/sim/src/build.rs#L829)); speed 65 ([ship.rs:256](crates/sim/src/ship.rs#L256)). "Corvettes still need Shipyard II" — [docs/founding-charter.md:217](docs/founding-charter.md#L217)
- **Shipyard II:** structure tiers I–II use the base recipe (×1 cost, ×1 time); costs rise ×1.6 per tier from III, and time ×20/×180/×900/×2,700 for III–VI ([build.rs:463-472](crates/sim/src/build.rs#L463)). The Shipyard base recipe is 20 Machinery, 40 Alloys, 15 Electronics, 20 s ([build.rs:1002-1009](crates/sim/src/build.rs#L1002)).
- **Warp:**
  - `WARP_FACTOR` = 5 ([crates/sim/src/transit.rs:14](crates/sim/src/transit.rs#L14)); a fleet moves at its slowest hull's speed ([ship.rs:236-246](crates/sim/src/ship.rs#L236)).
  - Slow warp on Uranium runs at 0.5 speed and burns 0.25× per distance ([crates/sim/src/fuel.rs:83-87](crates/sim/src/fuel.rs#L83)).
- **Fuel:**
  - Burn = 1e-6 × mass × distance (`FUEL_PER_MASS_DISTANCE`), integrating to k · mass · length / factor under warp ([fuel.rs:43-46](crates/sim/src/fuel.rs#L43), [fuel.rs:108-128](crates/sim/src/fuel.rs#L108)).
  - Cargo adds 28 mass per unit ([ship.rs:154-157](crates/sim/src/ship.rs#L154)).
  - Deuterium costs 14 Cr and Uranium 5.6 Cr ([market.rs:171-175](crates/sim/src/market.rs#L171)).
- **Distances:**
  - Homes are 125,000 su from the hub and at least 60,000 su apart ([crates/sim/src/galaxy.rs:1491-1495](crates/sim/src/galaxy.rs#L1491)).
  - Light (signals) travels 2,000 su/s, so home to Hub takes about 62 s ([crates/sim/src/config.rs:162-165](crates/sim/src/config.rs#L162); [docs/treasury.md:3-8](docs/treasury.md#L3)).

### Inferences
- **Base-price values of the goods in each hull:**

  | Hull | Base value |
  |---|---|
  | Combat Freighter | 1,969 Cr |
  | Interceptor | 1,983 Cr |
  | Corvette | 1,945 Cr |
  | Tiny Freighter | 1,359 Cr |
  | Small Freighter | 2,633 Cr |
  | Shipyard I→II upgrade | 2,790 Cr |

- **Example raid trip:** 1 Interceptor + 1 Combat Freighter between neighbouring homes (60,000 su).
  - Warp speed 350 su/s (the Combat Freighter sets the pace): about 2.9 min each way at Full, about 5.7 min at Slow.
  - Fuel at Full warp on Deuterium: about 26 out (mass 2,200), about 53 back loaded with 80 units (mass 4,440). That is about 80 Deuterium, roughly 1,100 Cr, for the round trip.
  - On Slow warp: about 20 Uranium, roughly 110 Cr.
  - So an 80-unit Ferrite haul (640 Cr) does not pay for a full-warp round trip. Refined goods do.
  - These are my arithmetic from the formulas above, not measured values.

### Gaps
- I did not check the Cargo Pods module's own cost, or whether it fits a Combat Freighter's one utility berth alongside other choices.
- I did not verify the transport/marine recipe or the marine counts a capture needs.

## 3. What protects a player from being raided?

### Takeaway
BUILT protections:
- 24–48 h founder protection. A founder may give it up from 30 min, after the privateer report; firing on another player then ends it permanently.
- Leave, 2–15 days.
- Pacts, and syndicate membership.
- A 900 su no-combat zone around the Market Hub, which is never blockaded.
- The war requirement for blockades: declaration + 2 h notice, or a 12 h reprisal right.
- Orbital defense stations, warships, garrisons and escorts.
- Sensors.
- A universal 40-unit floor per good.

Raiding another corporation's own freighters costs no Authority standing. Only killing Authority hulls does. There is no Travian-style cranny beyond the 40-unit floor; that idea awaits a go (see §4).

### Cited Findings
- **Founder protection** — [crates/sim/src/founding.rs:13-22](crates/sim/src/founding.rs#L13); [founding.rs:697-732](crates/sim/src/founding.rs#L697):
  - A founder may end it voluntarily from 30 min (`FOUNDER_PROTECTION_MIN_S`), and only after the privateer report.
  - It lasts at least 24 h (`BASE`), and until 24 h after charter completion (`AFTER_COMPLETE`).
  - Hard ceiling 48 h (`MAX`); an unfinished charter keeps the full 48 h.
- **Enforcement:**
  - A protected target refuses any PvP order (`OrderRejectReason::FounderProtection`).
  - A protected attacker past the minimum may fire, which permanently sets `protection_broken`.
  — `allow_pvp_order` at [crates/sim/src/world.rs:19836-19897](crates/sim/src/world.rs#L19836)
- **Leave:** 2 to 15 days, 7-day cooldown ([crates/sim/src/leave.rs:16-20](crates/sim/src/leave.rs#L16)). "Neither side of a corporation on leave fights" ([world.rs:19850-19862](crates/sim/src/world.rs#L19850); `pvp_shielded` [world.rs:19674-19677](crates/sim/src/world.rs#L19674)). Leave also blocks pirate home raids ([crates/sim/src/world/pirate_home_raids.rs:9-14](crates/sim/src/world/pirate_home_raids.rs#L9)).
- **Neutral attacks are legal, with reprisal:**
  - Intercept and Attack on another corporation's fleet need no war. The gates are founder protection, leave, ally/pact protection and the sovereign zone ([world.rs:9669-9720](crates/sim/src/world.rs#L9669), [world.rs:11405-11444](crates/sim/src/world.rs#L11405)).
  - "A neutral attack is legal, but it gives the defender a limited right of reprisal" — 12 h (`REPRISAL_S`), starting when the news reaches the victim's command center ([world.rs:21996-22030](crates/sim/src/world.rs#L21996); called at [world.rs:5159](crates/sim/src/world.rs#L5159); [crates/sim/src/diplomacy.rs:23-26](crates/sim/src/diplomacy.rs#L23)).
- **War notice:** a war goes live `WAR_NOTICE_GRACE_S` = 2 h after the declaration reaches its target ([diplomacy.rs:14-17](crates/sim/src/diplomacy.rs#L14); [crates/sim/src/config.rs:118-120](crates/sim/src/config.rs#L118)). Former syndicate members can't attack each other for 5 min ([diplomacy.rs:21-22](crates/sim/src/diplomacy.rs#L21)).
- **Syndicate allies** can't be raided, attacked or blockaded while allied (`DiplomaticProtection`) — [world.rs:9692-9705](crates/sim/src/world.rs#L9692); [world.rs:11274-11286](crates/sim/src/world.rs#L11274)
- **Hub sovereign zone:**
  - Radius `TCA_SOVEREIGN_RADIUS` = 900 su ([crates/sim/src/tca.rs:70](crates/sim/src/tca.rs#L70); [world.rs:17257-17259](crates/sim/src/world.rs#L17257)). No engagement may be opened against something inside it ([world.rs:9706-9720](crates/sim/src/world.rs#L9706)).
  - The Hub is never blockaded ([world.rs:8219-8224](crates/sim/src/world.rs#L8219)). "Nothing opens inside the Market Hub's sovereign zone" ([docs/engagement-rules.md:29-31](docs/engagement-rules.md#L29)).
- **Authority standing:**
  - Only Authority (TCA) hulls produce culprits. Raiding an Authority freighter costs 10 standing per incident, and destroying enforcement costs 20. Citations travel to the Hub by light ([world.rs:5796-5853](crates/sim/src/world.rs#L5796); [tca.rs:91-96](crates/sim/src/tca.rs#L91); [world.rs:17168-17227](crates/sim/src/world.rs#L17168)).
  - Thresholds: sanctioned below 100, suspended at 60, revoked at 20, proscribed at −20 ([tca.rs:103-109](crates/sim/src/tca.rs#L103)).
- **Defense stations:** orbital defense levels 1–5 (Defense Platform → Star Fortress), costing 2,260 to 217,640 Cr of goods per level and 22 s to 192 h to build — [docs/orbital-defense.md:10-33](docs/orbital-defense.md#L10). A station contests a blockade while any hardpoint fires, so plunder can't start until it is silenced — [docs/blockades.md:27-33](docs/blockades.md#L27)
- **Garrison:** each fed garrison tier stretches the siege clock by 0.30. It eats 0.2 Provisions/s per tier from the system's own stock — [world.rs:785-798](crates/sim/src/world.rs#L785)
- **Bombardment** only suppresses the garrison, as a window that decays. "No tier is destroyed, and no population is touched." — [world.rs:800-816](crates/sim/src/world.rs#L800)
- **Homes are never captured** — [world.rs:6649-6654](crates/sim/src/world.rs#L6649); [docs/blockades.md:47-52](docs/blockades.md#L47)
- **Escorts:** a Corvette screen or a covering platform turns a convoy raid into a battle, with no seizure — [world.rs:5110-5140](crates/sim/src/world.rs#L5110)
- **Blockade policy:** freighters wait out held blockades by default. Warp Stabilizers and Blockade Runners research (×0.5 catch) help a runner — [docs/blockades.md:85-104](docs/blockades.md#L85); [world.rs:684](crates/sim/src/world.rs#L684)
- **Sensors** — [docs/sensors.md:22-63](docs/sensors.md#L22):
  - Detection rule: seen ⇔ distance ≤ min(sensor range × √(core output), 200,000 su).
  - Command Center 6,000 su; Sensor Array 20,000 su, +10,000 per tier (II 30k … VI 70k).
- **Who is dark:**
  - Raiders run dark; Corvettes, capitals, Colony ships, Transports and the Authority Freighter broadcast ([crates/sim/src/ship.rs:278-304](crates/sim/src/ship.rs#L278)).
  - The Combat Freighter "doesn't broadcast and has no sensors" ([docs/blockades.md:132-133](docs/blockades.md#L132)).
- **The cranny-like floor:** 40 units of every good can never be stripped by a blockade, a pirate home raid or the vendetta — [world.rs:768-772](crates/sim/src/world.rs#L768); [world/pirate_home_raids.rs:119-120](crates/sim/src/world/pirate_home_raids.rs#L119), [world/pirate_home_raids.rs:209-210](crates/sim/src/world/pirate_home_raids.rs#L209)
- **Pirate-specific protections** — [crates/sim/src/pirate.rs:101-112](crates/sim/src/pirate.rs#L101), [pirate.rs:195-201](crates/sim/src/pirate.rs#L195); [world/pirate_home_raids.rs:9-14](crates/sim/src/world/pirate_home_raids.rs#L9), [world/pirate_home_raids.rs:49-58](crates/sim/src/world/pirate_home_raids.rs#L49), [world/pirate_home_raids.rs:197-202](crates/sim/src/world/pirate_home_raids.rs#L197):
  - Convoys are invisible to pirate hunting for 240 s after joining.
  - Home raids:
    - Wait 30 min after joining, and never hit a founder-protected or on-leave corporation.
    - Warn first: departure waits for the warning's light + 90 s.
    - Leave a 15 min per-victim cooldown.
    - Steal nothing while any defense platform tier or owner combatant is within platform cover.

### Inferences
- No PvP blockade plunder can surprise a defender: the war needs 2 h notice after it reaches them, or comes as a reprisal after their own aggression.
- A neutral convoy raid, though, can happen any time after founder protection lapses. Its only coded consequence is the victim's 12 h reprisal right; there is no standing loss.
- Goods held at the Market Hub warehouse act as a practical safe store, since they can't be blockaded or engaged. This is a side effect of the market design, not a designed cranny.

### Gaps
- I did not verify how the "held" stage treats an orbital station whose hardpoints are only partly knocked out, beyond the doc's statement that any firing hardpoint contests.
- I did not check `diplomacy_protects` for the exact pact list beyond the doc's wording (non-aggression, ceasefire, separation window).

## 4. What is designed but not built yet?

### Takeaway
PLANNED, nothing built, awaiting Peter's go:
- A **Raid order**: blockade rules, loads until full or contested, auto-returns.
- **Warehouses raise the protected floor**: a Travian-style hiding place, i.e. "sealed stores".

Also:
- **R11 pirate pressure** (Oct 23 2026) is a full PLAN, unbuilt.
- **Pirate fuel** is BUILT but switched OFF.
- **Recapture** was dropped.
- The **"Prize Hauler"** concept was BUILT under the name Combat Freighter.

### Cited Findings
- **Awaiting go (memory note):**
  - A Raid order ("blockade rules, loads until full or contested, auto-returns").
  - Warehouses that raise the protected floor ("a Travian-style hiding place").
  - "Recapture was dropped: loot now rides in freighters, which can be raided."
  — [/Users/peter/.claude/projects/-Users-peter-projects-stellar-syndicates/memory/blockades-running.md:53-56](/Users/peter/.claude/projects/-Users-peter-projects-stellar-syndicates/memory/blockades-running.md)
- **Naming:** Peter "rejected the names Prize Hauler and Clipper, named it Combat Freighter", and that was the go; it was built Oct 27 — [memory/blockades-running.md:52](/Users/peter/.claude/projects/-Users-peter-projects-stellar-syndicates/memory/blockades-running.md); [memory/combat-freighter.md:11-19](/Users/peter/.claude/projects/-Users-peter-projects-stellar-syndicates/memory/combat-freighter.md) (deployed to production Oct 28 2026, release -20261028-log, v71).
- **Conflicting status:** the MEMORY.md index line for blockades still says "Prize Hauler spec, Raid order, sealed stores await go". That line is stale for the Prize Hauler: the two detailed notes above say the design shipped as the Combat Freighter.
- **R11 plan (status PLAN, Oct 23 2026)** — [docs/r11-pirate-pressure-plan.md:1-132](docs/r11-pirate-pressure-plan.md#L1):
  - Season-week clock (weeks 1–8).
  - Pack size = tier + ⌊(week − 1)/2⌋ light hulls. Pirate destroyers ("Reaver") from week 4, cruisers ("Warbarge") from week 6.
  - Raids by share instead of a flat 60: share = 10% + 2.5% per week (27.5% in week 8).
  - Carry: 20 per light hull, 60 per destroyer, 150 per cruiser.
  - Load: 0.5/1.5/3 units/s per light hull/destroyer/cruiser over the 90 s hold. A week-8 maximum raid takes 390 units.
  - Colonies become targets from week 3 (24 h new-colony grace; undefended systems first).
  - Two more hideouts at weeks 3 and 5.
- **R11, further items:**
  - Relic guards from week 4 (Destroyer + Corvette + 3 platform tiers).
  - Syndicate assaults from week 3: 1/week, 2/week from week 6, 20 min warning. A successful one hauls up to capacity and holds twice as long.
  - Bot changes and a harness run.
  — [docs/r11-pirate-pressure-plan.md:138-287](docs/r11-pirate-pressure-plan.md#L138)
- **R11's timing statement is outdated:** it says "at the public server's 4× pacing a week is 42 wall hours" ([docs/r11-pirate-pressure-plan.md:45](docs/r11-pirate-pressure-plan.md#L45)), but production pacing is now 1× (`DEFAULT_SIM_PACING` 1.0, [crates/server/src/game_loop.rs:42](crates/server/src/game_loop.rs#L42)).
- **Pirate fuel:** built, but `PIRATES_BURN_FUEL` is false. "Loot weighs 28 a unit. A raider carrying 60 units reaches about 90,000 su", while production raids ran 280,000–330,000 su each way. Before turning it on, Peter must choose between closer raid targets and bigger tanks — [docs/pirate-dens.md:45-60](docs/pirate-dens.md#L45)
- **Comparison point for a Raid order:** the current code comment argues against a stockpile raid verb, saying it would be "strictly better than chasing convoys" — [crates/sim/src/world.rs:756-762](crates/sim/src/world.rs#L756)

### Inferences
- A Raid order with blockade rules would let a player loot a stockpile without holding a siege. It cuts against the rationale in the world.rs comment, so it is a real design decision, not just plumbing.
- "Sealed stores" would raise the 40-unit floor through Warehouses. Today the floor is a flat 40 per good regardless of storage.

### Gaps
- I found no doc giving the Raid order's or sealed stores' numbers: how much a Warehouse would protect, or the raid's hold limits. Only the one-line memory summaries exist.

## 5. NPC pirates: what they take from players, and what players get from beating them

### Takeaway
BUILT pirate home raids:
- Each takes at most 60 units: at most 10% of each good, never below 40 of any.
- Loading runs at 1.5/s, for at most 90 s over the home, only after beating the defense.
- At most one raid per corporation every 15 min.
- An absent, undefended player was raided about every 17 min, 427 times, on the dev server.

The founding raid takes the kit's 40 Semiconductors + 15 Armaments.

Players earn from pirates:
- Authority bounties: 600–1,100 Cr for hideouts, 3,000 for a depot, 8,000 for a stronghold.
- Counter-raid bounties: 400 Cr × defense tiers.
- Hauled-back plunder + wreck Alloys + gear.
- The charter's 4,500 Cr privateer bounty, and the vendetta's 6,000 Cr bounty + salvage.

### Cited Findings
- **Home raid caps:**
  - `HOME_RAID_GRACE_S` 30 min, `HOME_RAID_WARNING_S` 90 s, `HOME_RAID_COOLDOWN_S` 15 min, `HOME_RAID_HOLD_BATTLE_MULT` 2.0, `HOME_RAID_MAX_LOOT` 60, `HOME_RAID_STOCK_FRAC` 0.10 — [crates/sim/src/pirate.rs:101-112](crates/sim/src/pirate.rs#L101)
  - At arrival the allowance freezes per good: floor(min(10% × stock, stock − 40)) ([world/pirate_home_raids.rs:117-124](crates/sim/src/world/pirate_home_raids.rs#L117)).
  - The raid ends at 2 × battle length or once 60 units are stolen ([world/pirate_home_raids.rs:125-130](crates/sim/src/world/pirate_home_raids.rs#L125)).
  - Loading runs at 1.5 units/s, most valuable good first ([world/pirate_home_raids.rs:190-226](crates/sim/src/world/pirate_home_raids.rs#L190)).
- **Raid targets:**
  - Homes only, one expedition per corporation, oldest eligible target first.
  - A target needs stock above the floor and must lie outside the Hub zone.
  — [world/pirate_home_raids.rs:28-44](crates/sim/src/world/pirate_home_raids.rs#L28)
- **Pack size:** = enclave tier (1–3 Raiders) ([pirate.rs:184-189](crates/sim/src/pirate.rs#L184), [pirate.rs:272-275](crates/sim/src/pirate.rs#L272); top-up at [world/pirate_home_raids.rs:45-48](crates/sim/src/world/pirate_home_raids.rs#L45)).
- **Hideouts:**
  - Three ambient hideouts; one grows a tier every 300 s while ignored, to a maximum of 3.
  - Packs launch every 90 s; the first launch comes after 300 s.
  - A cleared hideout goes dormant for 600 s and returns at tier 1.
  — [pirate.rs:140-210](crates/sim/src/pirate.rs#L140)
- **Where the stolen goods go:** the pack carries them home into the hideout's `plunder` store — [world/pirate_home_raids.rs:77-89](crates/sim/src/world/pirate_home_raids.rs#L77)
- **Victim feedback:** a "PIRATE RAID" badge, "up to 10% of each good, 60 units in all", and toasts such as "Raided · Nergal · Took 46 Ferrite Ore · 14 Provisions" — [docs/blockades.md:71-83](docs/blockades.md#L71)
- **Observed frequency (memory):** Peter's :8080 test corporation's home was raided "~every 17 min (427 while away), each taking ≤60 units (46 ore + 14 provisions that time)" — [memory/blockades-running.md:57](/Users/peter/.claude/projects/-Users-peter-projects-stellar-syndicates/memory/blockades-running.md)
- **Founding raid (charter step 5):**
  - Takes every unit of Electronics and Armaments in the home stockpile, loading at 4 units/s, and holds at most 90 s ([crates/sim/src/founding.rs:24-39](crates/sim/src/founding.rs#L24); [docs/founding-charter.md:132-189](docs/founding-charter.md#L132)).
  - The kit holds 40 Semiconductors and 15 Armaments ([founding.rs:65-69](crates/sim/src/founding.rs#L65)).
  - "The money": buying back 35 Semiconductors + 15 Armaments at the trader's markup costs about 3,045 Cr ([docs/founding-charter.md:201-207](docs/founding-charter.md#L201)).
- **Privateer and den:**
  - The privateer bounty is 4,500 Cr ([founding.rs:96-100](crates/sim/src/founding.rs#L96)).
  - The den: 3 Raiders at 75% hull ([founding.rs:74-80](crates/sim/src/founding.rs#L74)). Its stores, 40 Alloys + 15 Semiconductors plus everything the raid took, are offered as salvage contracts to haul home ([founding.rs:81-83](crates/sim/src/founding.rs#L81); [docs/founding-charter.md:250-257](docs/founding-charter.md#L250)).
- **Vendetta (charter step 19):**
  - The uncle brings a Corvette + 2 Raiders at ×0.3 damage.
  - If unopposed he loads like a home raid (a tenth of each good, ≤60 units) for at most 120 s.
  - Killing him pays a 6,000 Cr bounty plus salvage of 60 Alloys, 20 Machinery and 15 Armaments.
  — [founding.rs:385-400](crates/sim/src/founding.rs#L385); [docs/pirate-vendetta.md:18-24](docs/pirate-vendetta.md#L18), [docs/pirate-vendetta.md:179-193](docs/pirate-vendetta.md#L179)
- **Authority pirate bounty** (`PirateBounty`): hideout = 350 + 250 × tier Cr (600/850/1,100), depot 3,000, stronghold 8,000; standing +2 × tier; captain XP 40 × tier — [crates/sim/src/world.rs:20650-20664](crates/sim/src/world.rs#L20650)
- **Counter-raid chain** after a home raid: a Scout survey (30 captain XP, 20 insight) → strike bounty of 400 × the hideout's defense tiers + 90 XP → recovery contracts. "Bring a Freighter for the loot." — [crates/sim/src/world/counter_raids.rs:10-75](crates/sim/src/world/counter_raids.rs#L10); hideout defense = tier × 2 ([pirate.rs:181-183](crates/sim/src/pirate.rs#L181), [pirate.rs:268-271](crates/sim/src/pirate.rs#L268))
- **Clearing a hideout:**
  - The victor gets the enclave's accumulated plunder plus wreck Alloys (8 × defense tiers), as salvage that must be loaded on site and hauled ([world.rs:6971-6979](crates/sim/src/world.rs#L6971); [pirate.rs:211-213](crates/sim/src/pirate.rs#L211); [world/pirate_progression.rs:59-79](crates/sim/src/world/pirate_progression.rs#L59)).
  - A 30% chance of field gear at a hideout; a relic at a depot or stronghold, plus an equipment cache and a 30% research dossier ([pirate.rs:214-256](crates/sim/src/pirate.rs#L214); [world.rs:6980-6996](crates/sim/src/world.rs#L6980)).
- **Killing loaded raiders recovers nothing:** seizure only happens on freighter hulls ([world.rs:5671-5683](crates/sim/src/world.rs#L5671)), and pirate raiders are Raiders/Corvettes. Loot comes back only by clearing the base.
- **Pirate Stash signals** (unlock on world day 7; hazardous): 40% a Mass Driver or Torpedo Rack; otherwise 800–1,500 Cr × scale plus 10–20 Armaments × scale (+15% field gear). Scale is 1.0/1.6/2.5 by world age — [crates/sim/src/signals.rs:106](crates/sim/src/signals.rs#L106), [signals.rs:304-358](crates/sim/src/signals.rs#L304)
- **Pirates hunting convoys:** within 2,600 su of their base, +900 su per tier. Bases sit at least 70,000 su from any home, in the 0.30–0.72 galaxy-radius band — [pirate.rs:172-204](crates/sim/src/pirate.rs#L172), [pirate.rs:264-267](crates/sim/src/pirate.rs#L264)

### Inferences
- **Pirate drain on an absent, undefended home:** at most about 60 units per 15–17 min, so 210–240 units/h. That is about 15–17% of one home Mining Complex I's 1,380 ore/h (see §6). In Ferrite terms, about 1.7–1.9k Cr/h at base price; mixed refined goods are worth more per unit.
- **The founding raid** took 2,200 Cr of goods at base price (40 × 34 + 15 × 56).
- **Goods value of the pirate rewards at base price:**
  - The privateer bounty plus the den's stores and recovered loot: 4,500 Cr + about 3,750 Cr of goods, hauled.
  - The vendetta's salvage: 3,640 Cr (60 × 26 + 20 × 62 + 15 × 56).

### Gaps
- I did not quantify how much plunder a typical hideout accumulates: it depends on raid history, minus 12 units per patrol (`CAMPAIGN_PATROL_COST`, [pirate.rs:94](crates/sim/src/pirate.rs#L94)).
- No measured rate of home raids across many players. The 17-minute figure comes from one dev-server corporation.

## 6. Production side: output rates, storage caps, credits, prices

### Takeaway
- One home Mining Complex I makes about 23 Ferrite Ore a minute (1,380/h ≈ 11,000 Cr/h at the Hub's base price of 8; half that at the trader's 4).
- Tiers multiply output ×2.2/3.8/6.0/8.6/11.5. Tier II is cheap and fast; tier III and above cost more and take minutes to hours.
- A charter-end home stores only about 1,300–2,000 units in all, and production idles when full.
- A bot measurement (Oct 10 2026, now dated) put home planet mining at 24–38k Cr/h at base prices.
- Credits come only from sales, stipends, bounties, contracts and signals. There is no passive tax income.

### Cited Findings
- **Production formula:** `output = base · tier_throughput · staffing · skill · food (· traits)` — [crates/sim/src/production.rs:1-12](crates/sim/src/production.rs#L1). Applied for extraction at [crates/sim/src/world.rs:15234-15262](crates/sim/src/world.rs#L15234), where staffing = min(workers, tier) / tier; for converters at [world.rs:15403-15420](crates/sim/src/world.rs#L15403).
- **Tier throughput** `TIER_THROUGHPUT` = [0, 1.0, 2.2, 3.8, 6.0, 8.6, 11.5]. "Freely buildable stops at tier 4"; tiers V–VI need research — [production.rs:19-31](crates/sim/src/production.rs#L19)
- **Extraction rates:** `DEPOSIT_BASE_RICHNESS` 0.45 units/s is "a baseline home worker's natural deposit output" ([crates/sim/src/galaxy.rs:1317-1322](crates/sim/src/galaxy.rs#L1317)). A natural site is capped at 3× base ([crates/sim/src/explore.rs:83](crates/sim/src/explore.rs#L83), [explore.rs:248-256](crates/sim/src/explore.rs#L248)). "At home-geology extraction rates (~0.4 raw/s) converters run INPUT-BOUND" ([production.rs:296-302](crates/sim/src/production.rs#L296)).
- **Measured home mine:** "The mine makes 23 ore a minute" (Mining Complex I at home, charter) — [docs/founding-charter.md:204-205](docs/founding-charter.md#L204)
- **Converter rates per second at tier I:**
  - Smelter: 1.0 Alloys from 1.5 Ferrite.
  - Electronics Fabricator: 0.5.
  - Chemical Works: 0.8 Polymers.
  - Fuel Refinery: 0.8 Deuterium from 1.0 Volatiles.
  - Agroplex: 1.2 Provisions from 1.0 Biomass.
  - Uranium Mill: 0.05.
  — [production.rs:303-345](crates/sim/src/production.rs#L303); ore recipes at [production.rs:212-242](crates/sim/src/production.rs#L212)
- **Uranium Mill:** 180 Uranium an hour from 90 ore — [docs/fuel-overhaul.md:206](docs/fuel-overhaul.md#L206)
- **Upgrade cost curve:**
  - Tiers I–II use the base recipe.
  - From tier III, cost ×1.6/2.56/4.10/6.55.
  - Time ×20/180/900/2,700: "a 20 s structure takes about 7 minutes at III, an hour at IV, 5 hours at V and 15 hours at VI".
  — [crates/sim/src/build.rs:463-472](crates/sim/src/build.rs#L463)
- **Mining Complex recipe:** 12 Machinery + 25 Alloys, 18 s — [build.rs:931-934](crates/sim/src/build.rs#L931)
- **Storage caps** — [build.rs:1685-1707](crates/sim/src/build.rs#L1685); [galaxy.rs:1064-1088](crates/sim/src/galaxy.rs#L1064); [docs/founding-charter.md:27-30](docs/founding-charter.md#L27):
  - One total cap is shared by all goods; "production simply IDLES at the cap".
  - Landing holds 1,000, plus the Storehouse project 300.
  - Warehouse I 700, +400 per tier (VI = 2,700). The Warehouse adds to the holds.
  - Orbital Warehouse 2,000 per tier (research).
- **Workforce at the charter:** 6,000 settlers = 6 workforce for six posts (Bioharvester, Agroplex, Mining Complex, Shipyard, Academy, second Mining Complex) — [docs/founding-charter.md:38-45](docs/founding-charter.md#L38)
- **The charter's 18 steps** end with a second world's staffed Mining Complex, a survey, a Mining Barge, belt mining and ore hauled home — [docs/founding-charter.md:66-87](docs/founding-charter.md#L66)
- **The charter kit** (in the holds with 2,000 Cr): 410 Alloys, 163 Machinery, 40 Polymers, 100 Provisions, 120 Uranium, 10 Deuterium, 15 Armaments, 3 Warp Cores, 40 Semiconductors — [docs/founding-charter.md:49-64](docs/founding-charter.md#L49)
- **Bot harness (Oct 10 2026,** 8 bots, seed 42, six sim-hours):
  - "A home's planet mining earns 24–38k Cr/h" at base prices.
  - One barge on an ordinary belt: 1.4–3.4k Cr/h. A found pocket: 7–10k Cr/h. Barges average 6.6–8.4k Cr/h over 12 h.
  - A 3,000-Cuprite home pocket is worth about 72k Cr and takes 6–8 h to mine dry.
  - All eight bots finish the charter by 300 minutes.
  — [docs/asteroid-belt-mining-plan.md:272-294](docs/asteroid-belt-mining-plan.md#L272)
- **Converter ceiling (planning figure):** an "industrial planet" Fuel Refinery II makes 6,336 Fuel an hour, if supplied — [docs/power-plants-plan.md:163-177](docs/power-plants-plan.md#L163)
- **Credits, start and stipends:**
  - Start: 2,000 Cr ([docs/founding-charter.md:49](docs/founding-charter.md#L49); [docs/treasury.md:45-47](docs/treasury.md#L45)).
  - Charter stipends: 300 + 300 + 400 + 300 + 600 + 300 + 400 + 500 + 300 + 300 + 800 = 4,500 Cr in all ([crates/sim/src/founding.rs:41-63](crates/sim/src/founding.rs#L41)).
- **Credits, other sources:**
  - Privateer bounty 4,500 Cr; vendetta bounty 6,000 Cr ([founding.rs:100](crates/sim/src/founding.rs#L100), [founding.rs:398](crates/sim/src/founding.rs#L398)).
  - The silent probe signal: 1,500 Cr ([docs/founding-charter.md:321-324](docs/founding-charter.md#L321)).
  - Contract board: 4 contracts a day; survey 400 Cr; delivery 8 Cr/unit ([crates/sim/src/operation.rs:37-47](crates/sim/src/operation.rs#L37)).
  - "Paid at home as before: stipends, bounties, contract rewards, signal finds"; Hub sales wire home by light ([docs/treasury.md:29-39](docs/treasury.md#L29)).
  - A grep found no tax, income or wage constants.
- **Market base prices** (Cr/unit):

  | Good | Price |
  |---|---|
  | Ferrite Ore | 8 |
  | Crystalline Ore | 6 |
  | Cuprite Ore | 24 |
  | Titanium Ore | 110 |
  | Rare-metal Ore | 1,700 |
  | Biomass | 5 |
  | Provisions | 9 |
  | Uranium | 5.6 |
  | Deuterium | 14 |
  | Polymers | 16 |
  | Alloys | 26 |
  | Semiconductors (Electronics) | 34 |
  | Armaments | 56 |
  | Machinery | 62 |
  | Hull Sections | 128 |
  | Warp Core | 215 |

  — [crates/sim/src/market.rs:118-201](crates/sim/src/market.rs#L118)
- **Market depth:**
  - Prices walk with flow; bulk goods have depth 1,600 units per e× move, an external cap of 1,600 and a refill of 8 units/s. Each trade pays a 1% half-spread.
  - Scarce goods trade on a thin book (depth 600).
  — [market.rs:1-26](crates/sim/src/market.rs#L1), [market.rs:55-72](crates/sim/src/market.rs#L55)
- **The free trader** buys ore at 4 Cr, "half the Hub's 8", up to 300 ore — [docs/founding-charter.md:191-205](docs/founding-charter.md#L191)

### Inferences
- **Mine output by tier:**

  | Mining Complex | Ore | Value at 8 Cr |
  |---|---|---|
  | I | 1,380/h | about 11,000 Cr/h |
  | II (2 workers) | about 3,040/h | about 24,300 Cr/h |
  | III | about 5,240/h | about 42,000 Cr/h |

  The tier II and III rows multiply the documented 23/min by 2.2 and 3.8.
- **At charter end:** two staffed Mining Complex I plus a barge give roughly 2,800–3,600 ore/h, about 22–29k Cr/h at base price. This is consistent with the 24–38k Cr/h harness band.
- **Storage fills fast:** a 2,000-unit home store fills in about 1.5 h from one mine. Most of a player's output must be shipped or sold, which is why the standing stockpile, the only raidable store at home, is small next to hourly production.
- **Base price overstates sales:** realized income is lower, because ore must travel 125,000 su to the Hub (or sell to the trader at half price) and big sales walk the price down.

### Gaps
- The 24–38k Cr/h figure predates the Oct 12 fuel overhaul, the Oct 14 bare start, charter v5 and the Oct 16 workforce cap. I found no current harness number for a charter-end player's income.
- I did not find the exact ore and richness of the second home world (Planet III), or the barge's actual cutting rate on the den belt (the doc gives `min(0.25, 3 × richness)` units/s at [docs/asteroid-belt-mining-plan.md:295-299](docs/asteroid-belt-mining-plan.md#L295)).

## 7. Putting numbers together: one Combat Freighter raid vs hours of production

### Takeaway
One Combat Freighter carries 80 units (120 with Prize Holds), and a held blockade fills it in about 53 s.
- **Value:** 640 Cr for ore up to about 4,960 Cr for Machinery at base prices.
- **Against one home Mining Complex I** (about 11,000 Cr/h): about 3.5 to 27 minutes of output.
- **Against the bots' 24–38k Cr/h home income:** about 1 to 12 minutes.

Getting there needs a war (2 h notice) or a reprisal, and beating the defense first. On full warp, an ore cargo doesn't cover the trip's Deuterium. A blockade that strips a whole store takes at most about 1,300–2,000 units (less 40 a good), roughly 1–2 hours of the victim's production.

### Cited Findings
- **Combat Freighter:** hold 80, 120 with Prize Holds, doubled by Cargo Pods — [docs/blockades.md:126-133](docs/blockades.md#L126); [crates/sim/src/ship.rs:198](crates/sim/src/ship.rs#L198)
- **Plunder rate:** 1.5 units/s (×1.5 with Prize Holds), one taker per held system, 40 units per good kept — [crates/sim/src/world.rs:765-783](crates/sim/src/world.rs#L765), [world.rs:8154-8159](crates/sim/src/world.rs#L8154)
- **One home mine:** 23 Ferrite/min ([docs/founding-charter.md:205](docs/founding-charter.md#L205)); Ferrite 8 Cr, Machinery 62, Alloys 26 ([crates/sim/src/market.rs:146](crates/sim/src/market.rs#L146), [market.rs:177](crates/sim/src/market.rs#L177), [market.rs:192](crates/sim/src/market.rs#L192))
- **Home planet mining income:** 24–38k Cr/h, harness, Oct 10 2026 — [docs/asteroid-belt-mining-plan.md:276-278](docs/asteroid-belt-mining-plan.md#L276)
- **War notice:** 2 h; reprisal 12 h — [crates/sim/src/diplomacy.rs:14-26](crates/sim/src/diplomacy.rs#L14)
- **Pirate home raid:** at most 60 units, about every 17 min on an absent dev-server corporation — [crates/sim/src/pirate.rs:111-112](crates/sim/src/pirate.rs#L111); [memory/blockades-running.md:57](/Users/peter/.claude/projects/-Users-peter-projects-stellar-syndicates/memory/blockades-running.md)

### Inferences
These are my arithmetic on the cited constants, not measurements.

- **One Combat Freighter load, by what the victim holds above 40 per good** (most valuable first):

  | Load | Value (base) | Mine-minutes (one MC I) | Minutes of 24–38k/h home income |
  |---|---|---|---|
  | 80 Ferrite Ore | 640 Cr | about 3.5 | about 1.0–1.6 |
  | 80 Alloys | 2,080 Cr | about 11 | — |
  | 80 Armaments | 4,480 Cr | about 24 | — |
  | 80 Machinery | 4,960 Cr | about 27 | about 7.8–12.4 |

  With Prize Holds (120 units), scale by 1.5.
- **Victim's side:** a charter-end home starts with the kit's 410 Alloys and 163 Machinery, so an early raid would take Machinery first, then Alloys.
- **Attacker's costs and risks for one Combat Freighter raid:**
  - Goods sunk: about 1,970 Cr for the Combat Freighter plus about 1,980 Cr per escorting Interceptor (they can be lost).
  - Fuel between neighbouring homes (60,000 su): about 80 Deuterium (about 1,100 Cr) on full warp, or about 20 Uranium (about 110 Cr) on slow warp.
  - Unload time: about 53 s per Combat Freighter after the blockade holds.
  - Exposure: the loaded Combat Freighter can be raided on its way home.
- **A whole-store strip** (several Combat Freighters or a big freighter in the blockading fleet):
  - A 2,000-unit home store minus floors goes in about 20 min at 1.5/s.
  - Worth about 16k Cr if it is all ore, up to about 50k Cr if it is all Alloys. That is roughly 0.5–2 hours of the victim's home income at 24–38k Cr/h.
  - A home can't be captured; a colony siege adds 6+ h and transfers the colony's whole stockpile and half-tier structures.
- **Convoy raid** (no war needed): the take is the victim's in-transit cargo, capped by the raider's holds.
  - Example: a Small Freighter carrying 150 Alloys (3,900 Cr) yields 80 Alloys (2,080 Cr) to one Combat Freighter; the rest sinks.
  - The victim also loses the hull (about 2,630 Cr of goods).
- **Pirate drain on an absent player:** at most about 210–240 units/h, about 15–17% of one Mining Complex I's output.
- **Pirate payouts** (one-off, on clearing): 600–8,000 Cr bounties, 800–2,400 Cr counter-raid bounties, plus hauled plunder.

### Gaps
- No measured PvP raid outcomes exist. Every SS raid-value figure above is derived from constants, not observed play.
- Hub sale proceeds sit below base price for large sales (walked price, 1% spread, freight fees). I did not model them, so the Cr values here are upper bounds.
- The production comparison leans on an Oct 10 2026 harness figure that predates several economy changes. A fresh harness run would be needed for a current Cr/h figure; per the constraints I did not run it.
