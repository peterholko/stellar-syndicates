# Stellar Syndicates: endgame design (draft for review)

Status: design proposal, Oct 23 2026, now **on hold**. Nothing in this document
is built yet. The design to build from is [endgame-keystone.md](endgame-keystone.md).
It describes the endgame for the game's "seasons" so that it can be reviewed
by someone with no other context. Section 13 lists the questions the review
should answer.

Markers used below: **Decided** (the designer has agreed), **Proposed**
(the current recommendation), **Open** (needs a decision).

---

## 1. The game in brief

Stellar Syndicates is a persistent, browser-based space strategy MMO, heavily
inspired by Travian: Legends. It is played asynchronously over weeks: the
server simulates the galaxy continuously at 1× real time, and players check in
a few times a day.

- **Scale.** A galaxy holds about 20 player corporations around one wormhole
  hub (the Market Hub). Server-run bot corporations fill empty seats. More hubs
  are planned so a galaxy can grow past 20 players; each hub is meant to anchor
  a district of about 20 homes.
- **Light-speed information.** The core rule. Every player sees the galaxy as
  light reaches their command center, so distant events arrive late. Orders
  also travel out at light speed before a fleet obeys. Travel takes real time,
  from minutes to hours, and costs fuel.
- **Economy.** Corporations settle planets, build industry and trade goods.
  The only market is at the Hub. The Authority also runs paid freight, settler
  liners and the Hub warehouse.
- **The Terran Charter Authority (the Authority).** The game's governing NPC
  power.
  - It opened the wormhole from the Milky Way to this galaxy and runs the Hub
    station (the Authority Nexus).
  - It grants each corporation its charter: the licence to settle.
  - It keeps a standing score, 0–200 (100 is full standing). Low standing means
    tariffs and market penalties; at the bottom a corporation is Revoked, then
    Proscribed, and Authority enforcement fleets hunt Proscribed corporations.
  - No combat may start within 900 units of the Hub.
  - The opening slideshow's line: "The Authority will not forget what you owe."
- **Syndicates.** Voluntary alliances of corporations: mutual non-aggression,
  shared intelligence, garrisons, a chat channel and mail, and a weekly
  project ladder that buys lasting bonuses.
  - A syndicate may hold at most a third of the galaxy's active human
    corporations, with a minimum of two.
  - **Decided:** syndicates stay flat during play. No kings, governors or
    tribute.
- **Combat and conquest.**
  - Fleet battles resolve on a tactical engine.
  - Conquest is a siege: blockade a colony, hold the blockade while a siege
    clock runs (hours at 1×), then land troops to capture it. Capture halves
    the colony's structure levels.
  - **Home worlds can never be captured**, and stockpiles keep a protected
    floor.
- **NPC threats.** Pirate hideouts, depots and strongholds raid players and
  their freight. Pirate pressure is planned to grow week by week through a
  season.
- **Exotic nodes.** About 16% of star systems hold an exotic node with a bonus
  for whoever owns the system:
  - Relay Anchor: faster orders nearby.
  - Veil: harder-to-detect fleets.
  - Deep Scan: exact enemy fleet compositions.
- **The Precursors.** A vanished ancient civilisation. Their traces exist today
  as Precursor fragments (random "Signal" events), Precursor caches and
  precursor artefacts at exploration sites.

## 2. The season

**Proposed:** each galaxy runs an 8-week season at 1×, first tested at 4
weeks. The calendar is set in fractions of the season length, so a 4-week
test compresses it.

| Week | What happens |
|---|---|
| 1 | Founding and first claims. Registration open. |
| 2–3 | Syndicates form. Pirate pressure rises. Registration closes at the end of week 3. |
| 4 | **Relics wake:** exotic nodes become contested prizes, guarded by pirates. |
| 5 | Relic war; the Authority's levies rise. |
| 6 | **The endgame opens:** the Precursor worlds are revealed and the Keystone race begins. |
| 7 | Keystone race. |
| End of 8 | Hard cap. A short epilogue, then results go to a permanent hall of fame and a fresh galaxy opens (the next season). |

Also planned for the season:
- Corporations that stop playing become derelict colonies that others can
  salvage or capture.
- Each extra colony needs a colony licence bought from the Authority, at a
  rising price.

## 3. The lore spine

- The Authority didn't build its wormhole. It found an ancient Precursor gate
  network and learned to force gates open. The Nexus is the first; every
  future hub is another gate the Authority has forced open.
- The Precursors left one control machine for the whole network, **the
  Keystone**. It is sealed on a few **Precursor worlds** in this galaxy. The
  Authority knows, and the oldest clause in every charter forbids landing on
  them.
- As the season goes on, the galaxy outgrows the Authority. Colonies become
  self-sufficient and need it less. The Authority answers by tightening its
  grip: "what you owe" comes due as rising levies.
- In week 4 the exotic nodes wake. They are **relics**: the Precursor
  network's resonators. By week 6 their resonance breaks the seals and the
  Precursor worlds appear on every chart.
- Whoever wakes a Keystone gains the network itself:
  - sight without light delay;
  - orders that arrive at once;
  - fleets that step through gates.
  
  Those are the two limits every other player lives under (light delay and
  distance). Fully woken, the Keystone seals every gate. The Authority is cut
  off, and its holder rules the galaxy.
- Afterwards the Authority forces a gate into another galaxy and charters new
  settlers. That is the next season's opening.

## 4. The two endings

**Decided:** the endgame is a race of Keystones. (An earlier cooperative
"unity" path was dropped because it created too little conflict between
players; see §11.)

| Ending | How it happens | Who wins | Everyone else |
|---|---|---|---|
| **A. The Sovereign** | A syndicate fully wakes a Keystone and fires the Seal | That syndicate | Subjugated |
| **B. The Charter Holds** | No Keystone fires by the season cap | The corporations that did most to stop the Keystones | The Authority keeps the galaxy |

Either way the season ends. All results go to the hall of fame, which outlives
every galaxy and shows on each corporation's card.

## 5. The Keystone race

**The Precursor worlds** (Proposed)
- Three per galaxy of about 20 players, sealed until week 6, then revealed to
  everyone.
- Placed in the band between the Hub and the homes, so no one starts next to
  one. With several hubs later, possibly one per district.
- Guarded: their defenders must be cleared before a colony ship can land.
- Each holds one Keystone, so up to three syndicates can race at once.

**The point of no return** (Decided)
- The moment a corporation lands a colony ship on a Precursor world, the
  Authority declares **permanent war** on it for the rest of the season:
  - Proscribed: no Hub market, no Authority freight, no docking at the Hub.
  - Standing frozen, with no way to buy it back.
  - Authority crackdown fleets attack its Keystone and its other worlds.
  - The Authority posts bounties on its fleets, freight and Keystone, open to
    every other corporation.
- Any corporation that sends goods to a Keystone joins the war too. Helping is
  treason.

**Waking a Keystone** (Proposed)
- Ten levels, each needing goods the builders haul themselves; Authority
  freight is closed to them, so every supply run can be raided.
- Levels 6–10 also need relics held, possibly by two different members of the
  syndicate. This mirrors Travian's second construction plan.
- Authority crackdowns at levels 3, 6 and 9, plus the bounties.
- Completing level 10 wakes the Keystone: its power unlocks (§6) and the
  Seal's public 48-hour charge begins.
- Firing the Seal ends the season (Ending A).

**Counterplay** (Open)
- Siege the Precursor world: a held blockade knocks a level off at each siege
  tick.
- Possibly capture it: the captor inherits the Keystone at half its level, and
  the Authority's permanent war.
- Alternatively, copy Travian: once a Keystone reaches level 1 its world can't
  be captured, only sieged down.

## 6. The Keystone's power

**Decided:** the Keystone gives its syndicate enormous power, but **only when
it is completed**. While it is being built there are no powers: an earlier
proposal unlocked a power every two levels, and that snowballed into an
unbeatable leader. The ten levels are a pure race of relics, hauling and
sieges.

**On completion (level 10), every member of the holding syndicate gains, all
at once** (the powers are Proposed):

| Power | Effect |
|---|---|
| **Network Sight** | Every fleet in the galaxy seen as it is now, with no light delay; stealth doesn't work against them |
| **Instant Command** | Their orders arrive at once, anywhere |
| **Gate Step** | Their fleets jump instantly between the Keystone world and any relic system a member holds |
| **Precursor Fleet** | A war fleet sleeping in the world's vaults wakes and obeys the syndicate: three Precursor warships (Proposed), each stronger than any ship players can build |

The first three lift the two limits every other player lives under (light
delay and distance) for one syndicate. The Precursor Fleet gives the final
stand its muscle:
- It fights at the Keystone, and through Gate Step it can reach any relic
  system a member holds.
- It is tied to the Keystone: if the Keystone drops below level 10, the fleet
  goes dormant in the vaults.
- It is finite: warships destroyed stay destroyed for the season, so killing
  them is real progress for the attackers.
- After a capture, surviving ships sleep until a new holder completes the
  Keystone. For scale, homes sit 100–150k su from the
Hub and fleets travel for hours.

**The final stand.** Completion also starts the Seal's public 48-hour charge.
The holder has to keep the Keystone at level 10 until the charge finishes:

- **If they hold it for 48 hours**, the Seal fires: every gate closes, the
  Authority is cut off, and the syndicate rules as Sovereign. The season ends.
- **If attackers siege it below level 10**, the powers vanish and the charge
  stops. The holder has to complete it again, and the charge starts again from
  zero (Open: or resumes).
- **If attackers capture the world** (if capture is allowed, §12), they take
  the Keystone at half level, with no powers, and must build it back to level
  10 themselves.
- **The Authority throws everything at a completed Keystone:** its largest
  bounties, and its own fleets alongside the attackers.

So the season's climax is one empowered syndicate against the rest of the
galaxy and the Authority, for 48 hours that every time zone can join.

## 7. Ending B: The Charter Holds

- If no Keystone fires by the cap, the Authority keeps the galaxy.
- Titles go to the corporations that did most to stop the Keystones:
  - Keystone levels knocked down;
  - bounties claimed;
  - Keystone worlds captured.
- So defending the galaxy is a real goal, not only a way to deny someone
  else's win.

## 8. How the race plays out

- **Relics are the central prize:** every Keystone needs them for its top
  levels.
- **Every Keystone is everyone's problem.** If it fires, everyone else loses
  the season, so non-builders, rival builders and the Authority all attack it.
- **The climax is the final stand:** one completed, empowered Keystone against
  everyone for 48 hours. Before completion, no syndicate has a power edge.
- **Rival builders fight each other** as well as the galaxy and the Authority.
- **The Authority is the NPC antagonist**, the way Travian's Natars attack
  Wonders of the World. Players are the main threat.
- **Committing is risky.** Landing on a Precursor world or feeding a Keystone
  means permanent war, even if that Keystone later falls.

## 9. The Travian reference: the Wonder of the World

Travian rounds end when the first alliance builds a **Wonder of the World** to
level 100.
- The Natars, an NPC tribe, hold the Wonder villages from the start. Alliances
  must conquer one, and once a Wonder reaches level 1 its village can no
  longer be conquered.
- **Construction plans** appear around day 180 on normal-speed worlds. A hero
  must capture one, and plans can be stolen like artefacts.
  - Levels 1–49 need one plan held in the alliance.
  - Levels 50–100 need two plans, held by two different players.
- Each level costs hundreds of thousands to millions of resources, fed by
  merchant routes from the whole alliance. A full build takes about 45 days at
  normal speed.
- The Natars attack at levels 5, 10, 15 and every fifth level after, then after
  every level from 95. Rival alliances are usually the bigger threat.
- Rounds have ended around day 210–215, with a hard cap at day 250.
- Unlike our Keystone, a Wonder grants no powers. Travian's artefacts do: held
  bonuses that make their holders targets.

How Travian's counterplay maps onto ours:

| Travian | Stellar Syndicates |
|---|---|
| Conquer the Wonder village before level 1 | Clear the guardians and land on the few Precursor worlds first |
| Steal construction plans; a second plan holder is needed past level 49 | Take the relic systems; the top levels need relics, possibly from two members |
| Catapult the Wonder to knock levels off | A held siege at the site knocks levels off |
| Break the defence stacked in the village | Battles at the site |
| Hit the merchant logistics and warehouses | Raid builders' supply runs, which have no Authority freight |
| Natar waves every 5 levels | Authority crackdowns at levels 3, 6 and 9 |

## 10. Principles and constraints

- **Syndicates stay flat during play** (Decided Oct 19 2026). "Subjugated" is
  how a season ends, not a tribute system while it runs.
- **Players don't build wormholes or gates** (Decided). More Authority hubs are
  planned for scaling past 20 players, and a player-built gate would read as a
  new hub.
- **Asynchronous play with real light delay.** Big moments need long warning
  windows so every time zone can respond, hence the 48-hour charge.
- **Homes are never captured**, so no one is wiped out.
- **The rules must work in small galaxies:** playtests may have 2–5 human
  players plus bots.
- **Terse in-game text.** Lore lands in short lines and illustrated pop-ups,
  not walls of text.

## 11. Alternatives considered

| Idea | Outcome |
|---|---|
| An Authority gate project at the Hub, built per syndicate like the Wonder | Dropped: a player-built gate clashes with the planned multi-hub scaling |
| A plain monument (an "Authority Beacon") as the race | Too thin: no reason for it to exist |
| A superweapon that destroys the Nexus | Folded into the Keystone |
| A unity path: a Declaration of Independence and a shared Free Assembly that wins for every signatory | **Dropped Oct 23:** mostly players against an NPC, too little conflict between players |
| Rival Declarations: coalitions racing to unite the galaxy under their own charter | Not pursued, in favour of the simpler Keystone race |
| Powers unlocked every two levels while a Keystone is built | **Dropped Oct 23:** snowballs into an unbeatable leader. Power now comes only on completion |
| Gate control alone as the reason the Sovereign rules | Rejected: mature colonies don't need imports. Replaced by instant sight, command and reach |
| Different endgames every season | Proposed instead: one endgame for the first seasons, varied by per-season modifiers (§12) |

## 12. Open decisions

1. **Keystone counterplay.** Can a Keystone be captured (taken at half level),
   or is it locked from level 1 so it can only be sieged down?
2. **The completion powers.** Are Network Sight, Instant Command, Gate Step and
   the Precursor Fleet right? How big and how strong should the fleet be:
   three warships, or one titan-class flagship? Do they make a 48-hour stand hard but possible? (A "Gate Lock"
   power, closing the Hub to one corporation for 6 hours a day, was proposed
   and rejected.)
   **The charge.** If a completed Keystone is knocked below level 10 and
   rebuilt, does the 48-hour charge restart from zero or resume?
3. **After the Seal.** Does the season end at once, or is there a short
   "Dominion" epilogue (about 24 hours) in which the Sovereign's rule is shown
   before the galaxy closes?
4. **Guardians of the Precursor worlds.** Ancient Precursor sentinels (a new
   NPC faction, needing new art) or pirate warlords (existing pirates)?
5. **Precursor world count and placement.** Three per 20 players, or one per
   hub district?
6. **Relic rules.** How many relics, held by how many different members, for
   which levels?
7. **Ending B rewards.** How is "stopping the Keystones" scored, and should
   its winners get more than titles?
8. **Cross-season rewards.** Hall of fame titles only, or cosmetics or other
   carry-overs? Should a Sovereign's win shape the next galaxy's story?
9. **Season modifiers.** Should each season vary the dials (Keystone count,
   relic count, Authority strength, pirate timing, one special rule) while the
   core endgame stays the same?
10. **Bots.** Do bot corporations build Keystones, attack them or hunt bounties?
11. **Independence pressure.** A galaxy-wide meter driven by everyone's
    development and the Authority's levies, or simply the calendar?

## 13. What the reviewer should assess

1. Is the Keystone race fun, tense and achievable within an 8-week asynchronous
   season?
2. Is the completed Keystone's power enough to make the 48-hour final stand
   dramatic, while still leaving the rest of the galaxy a real chance to break
   it? What would you change?
3. Are the incentives right for each kind of player:
   - builders;
   - rival builders;
   - non-builders;
   - solo corporations?
4. Is the lore coherent and convincing? In particular: why the Keystone holder
   dominates, why the Authority wars on rebels, and why the galaxy outgrows the
   Authority.
5. Which open decisions in §12 would you settle, and how?
6. What exploits, degenerate strategies or failure modes do you see?
   Consider, for example:
   - kingmaking;
   - small-galaxy edge cases;
   - time zones.
7. What would you simplify or cut for a first version?
