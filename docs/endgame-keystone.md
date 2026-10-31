# Endgame: the season arc and the Keystone

**Status: BUILT Oct 29 2026, uncommitted, protocol v75, on :8080 (not
production).** See **As built** at the end for the numbers chosen for the open
decisions. Designed with Peter, Oct
22–23 2026 (the Keystone race), revised Oct 28–29 2026: no calendar (the
galaxy's maturity sets the pace), relics lead to the Precursor worlds, war
comes when an Authority spy leaks that you know the Keystone's secret, and the
Authority's tightening grip (after Sid Meier's Colonization) is why players
want rid of it, told through its own news network. The older review brief is
[endgame-scenarios.md](endgame-scenarios.md).

## In one paragraph

The Authority squeezes harder as the galaxy grows: levies, embargoes,
requisitions, inspections and crackdowns, some of them staged. Every squeeze
builds a corporation's Resolve. Corporations set on breaking free attune
relics, whose bearings triangulate the sealed Precursor worlds. Finding one
reveals the secret: its Keystone can close every wormhole and cut the
Authority off. An Authority spy leaks the discovery and the Authority declares
war. Rebel syndicates raise a Keystone through ten levels; completing it wakes
four powers and starts a public 48-hour stand. If they hold, the Seal fires
and the syndicate is Sovereign. Loyalists, who profit from the Authority and
hunt rebels, win if no Seal fires. The Authority's Galactic News Network tells
the whole season its way; the rebels' Free Signal answers.

## Pacing: maturity, not the calendar

- **Galaxy maturity** (0–100%) is measured from what the active corporations
  have built: their colonies and research. It drives the season; no stage has
  a date.
- The Authority's grip, relics waking, attunement speed, pirate pressure and
  the Authority's war fleet all follow maturity.
- **Registration closes** when the first Precursor world is found.
- **The end clock starts with the race:** the first discovery starts a 10-day
  race; a Seal ends it sooner.
- **Backstop:** if no world is found by 12 weeks, the Authority reseals them
  and the Charter Holds.
- A rushing syndicate can start early but attunes slowly; the field catches
  up.

## Lore

- The Authority didn't build its wormhole. It found an ancient Precursor gate
  network and forced gates open. The Nexus is the first; every hub is another
  forced gate.
- The Keystone is the network's control machine, sealed on a few Precursor
  worlds. The oldest clause of every charter forbids landing on them.
- The galaxy outgrows the Authority. It tightens its grip: "The Authority will
  not forget what you owe."
- Relics are the network's resonators; their resonance points to the sealed
  worlds.
- Whoever completes a Keystone holds the network itself, freed from light
  delay and distance, and can seal every gate.
- After the season, the Authority forces a gate into another galaxy: the next
  season's prologue.

## The Authority's grip

Stages advance with maturity, each announced by an Authority decree (a GNN
headline and a story pop-up).

| Grip | Maturity | The Authority |
|---|---|---|
| I · Charter | start | Freight fee and standing ladder (as today) |
| II · Levies | 25% | A levy on every Hub sale and purchase, from 5%; requisitions begin |
| III · Inspections | 45% | Colony licence fees; cargo inspections at the Hub; a registry tax on warships |
| IV · Martial Charter | 65% | Garrisons in the Hub district; syndicate dues; requisitions backed by enforcement fleets |

### Levy and embargo (Colonization's tax rises and boycotts)
- After a big Hub trade, the Authority raises **your** levy a few points.
- **Accept**, or **refuse**: refused, that good is **embargoed** for you at the
  Hub until you pay the arrears. Embargoes push trade into your own and your
  syndicate's freight.

### Requisitions
- Periodic demands: "Deliver 300 Alloys in 24h", or credits.
- Paid: standing holds. Refused: standing falls and the levy rises; repeated
  refusals bring an Authority enforcement blockade.

### Resolve (Colonization's rebel sentiment)
- Each corporation's will to break free, 0–100%.
- Rises with every levy and requisition paid, every embargo and crackdown, and
  from a new structure that rallies the colonists.
- Bonuses at 50% and 100% (production, crew).
- **Attuning a relic needs Resolve 50%**: only those set on freedom seek the
  Keystone.

### Loyalists
- Corporations that keep Resolve low pay a lower levy and get Authority
  contracts, escorts and bounties for hunting rebels.
- They are the Charter Holds ending's natural winners, which keeps players in
  conflict with one another (why the shared Declaration was dropped).

### The Authority's war fleet (Colonization's expeditionary force)
- Grows with maturity and with the galaxy's total Resolve; its size is public.
- When a spy's leak ends in war, it sails at that syndicate.

## Events and false flags

Triggered by the galaxy, not dates. Each is a GNN headline, a story pop-up and
a Log entry.

| Event | Trigger | Crackdown |
|---|---|---|
| Sabotage at the Hub | Grip III, rising Resolve | 12h of cargo inspections, doubled Hub fees |
| Convoy massacre | An Authority convoy destroyed | The Authority names a culprit (the nearest syndicate above 50% Resolve): bounties, an enforcement fleet |
| Quarantine | Disease along a busy trade route | The systems blockaded for 12h |
| Relic seizure | Relics waking | Holders hand relics over or lose standing |
| Emergency decree | The galaxy's total Resolve passing 50% | The grip jumps a stage early; levies rise |

**False flags (the Authority's):**
- Some sabotage and massacre events are staged by the Authority. Nobody can
  tell which.
- Each leaves evidence at the scene: a signal (the Signals system) a Scout can
  investigate within 24h.
- **Exposed:** the crackdown lifts at once, every corporation's Resolve +10%,
  loyalists lose some Authority favour, and GNN runs a retraction.
- **Not exposed:** the crackdown runs its course and the lie stands.

**Player false flags — later, and only against the Authority:** a "False
Colors" research lets a fleet attack Authority targets under pirate colors
(the Authority blames pirates); unmasked if anyone's scout or sensors catch it
up close, which means war. Never between players: framing would point
reprisal at the innocent and let griefers target newcomers.

## Galactic News Network

- **GNN:** the Authority's public feed of galaxy events, separate from each
  corporation's private Log. Items reach a corporation when the Hub's
  broadcast light reaches its command.
- **It reports:** decrees and levy rises; crackdowns (the Authority's
  version); wanted lists and bounties; wars declared; the Keystone race
  (levels, holders, the 48h countdown); big battles, captures and relic
  seizures; market swings, weekly medals and new corporations.
- **The Free Signal:** rebel broadcasts in the same feed: exposed false flags,
  rebel wins, Resolve milestones. Louder as the galaxy's Resolve rises.
- **Spin:** GNN calls rebels "insurgents"; the Free Signal calls them
  "liberators". An exposed false flag forces a GNN retraction.
- **Where:** a one-line headline ticker in the desktop top bar; a News tab in
  the Log (both shells); a click goes to the system or fleet; stories about
  your own corporation are highlighted.

## The relic journey: finding a Precursor world

- **Relics wake** as maturity rises: a first wave at 30%, all by 60%. They
  become guarded, contested prizes.
1. **Attune.** A relic you hold fills with resonance (about a day in a young
   galaxy, about 4h in a mature one; Resolve 50% needed). Full, it gives a
   **bearing**: a line on your and your syndicate's map toward the nearest
   sealed world.
2. **Triangulate.** Bearings from two different relics cross in a search zone
   of a few systems; a third narrows it to one.
3. **Find it.** Arriving in the right system with a bearing through it reveals
   the world, to you first.
4. **Break the seal.** Clear the guardians; land a colony ship.
- The guide carries it as a journey card: "Attune Varn's Spire · 6h", "One
  more bearing crosses it", "Survey the zone · 4 systems".

### Precursor worlds
- Three per galaxy of about 20 players, between the Hub and the homes.
- Sealed until found; public once anyone lands.
- Guarded: clear the defenders before a colony ship can land.

## The secret and the spy

- Finding a world reveals what its Keystone does: it can close every wormhole.
  The finder's syndicate learns it together (finds are shared).
- An Authority spy in the syndicate leaks it: "The Authority will know in
  2h 00m". In that window the syndicate can still trade at the Hub, bring
  freight home and set its defences.
- Then the Authority declares **permanent war** on the syndicate:
  - proscribed: no Hub market, Authority freight or docking;
  - standing frozen, no buy-back;
  - crackdown fleets and the Authority's war fleet;
  - Authority bounties on its fleets, freight and Keystone, open to everyone.
- After the first landing the world and its secret are public: anyone who
  lands on a Precursor world or feeds a Keystone is at war at once.
- The war outlasts the Keystone: losing it doesn't end the war.

## Building the Keystone

- Ten levels on the Precursor world, paid in goods the builders haul
  themselves (Authority freight is closed to them). Costs scale with the
  galaxy's size.
- Levels 6–10 also need relics held.
- Authority crackdowns at levels 3, 6 and 9.
- **No powers while building** (a per-level ladder snowballed).
- Every Keystone's level and holder are public (and on GNN).

## Completion powers

| Power | Effect |
|---|---|
| **Network Sight** | Every fleet seen live (no light delay, no stealth). Fleets only |
| **Instant Command** | Orders arrive at once, anywhere |
| **Gate Step** | Instant jumps between the Keystone world and any relic system a member holds |
| **Precursor Fleet** | Warships stronger than anything players build; losses permanent |

All four last only while the Keystone stays at level 10.

## The final stand

- Completion starts the Seal's public 48-hour charge.
- **Held at level 10 for 48 hours:** the Seal fires, every gate closes, the
  Authority is cut off, and the syndicate rules as Sovereign.
- **Knocked below 10:** the powers and the charge stop.
- A held siege knocks a level off each siege tick. The Authority throws its
  largest bounties and its war fleet at a completed Keystone.

## Endings

| Ending | When | Winners | Everyone else |
|---|---|---|---|
| **The Sovereign** | A Seal fires | The holding syndicate | Recorded as subjugated |
| **The Charter Holds** | No Seal before the race or backstop ends | Loyalists and the corporations that did most to stop the Keystones (levels knocked down, bounties claimed, Keystone worlds captured) | The Authority keeps the galaxy |

Results go to a hall of fame that outlives every galaxy. Syndicates stay
flat: "subjugated" is a record, not tribute.

## Decided and rejected

- **Decided:** one endgame, the Keystone race; maturity pacing, no calendar;
  the relic journey; war on the spy's leak; the grip (levy, embargo,
  requisitions, inspections, martial charter); Resolve and loyalists; the
  Authority's war fleet; crackdown events and the Authority's false flags;
  GNN and the Free Signal; power only on completion; a 48-hour stand; the
  default ending rewards loyalists and Keystone-stoppers.
- **Rejected:** a player-built gate at the Hub; a plain monument; the shared
  Declaration of Independence (too little conflict between players); rival
  Declarations; a per-level power ladder; Gate Lock; gate control alone as
  the holder's edge; a fixed weekly calendar; player-against-player false
  flags.

## Open decisions

1. The maturity formula, and the levy, requisition and Resolve numbers.
2. The structure that rallies colonists (name, cost, Resolve per hour).
3. Leak delay (2h pencilled in); does a member who leaves before it ends escape?
4. Capture at half level, or locked from level 1 (siege-down only)?
5. Precursor Fleet: three warships or one titan-class flagship, how strong?
6. After a knock-down, does the charge restart or resume?
7. Guardians: Precursor sentinels or pirate warlords?
8. Worlds per galaxy: three per 20 players, or one per hub district?
9. Relic needs for levels 6–10.
10. Race length (10 days pencilled in) and the backstop (12 weeks).
11. Charter Holds and loyalist scoring.
12. Season end at the Seal, or a short epilogue? Cross-season carry-over?
13. What bots do: build, hunt bounties, stay loyal?

## Building it

None of the pieces needs an engine rewrite; each hooks into something that
exists.

| Piece | Hooks into | Size |
|---|---|---|
| Maturity and the grip | New `season` state on `World` (maturity, grip stage, decrees) | Moderate |
| Levy and embargo | Hub trade settlement (`market.rs`, `tca.rs` tariffs and standing) | Moderate |
| Requisitions | Authority operations (`operation.rs`) and enforcement expeditions | Moderate |
| Resolve and its structure | Per-corp state; a new structure (new kinds go last in `ALL`) | Moderate |
| Authority war fleet | The enforcement AI (`TCA_ENFORCEMENT`), scaled by maturity and Resolve | Moderate |
| Events and false flags | Season events; evidence as signals (`signals.rs`) | Moderate |
| GNN and the Free Signal | A public timeline beside the Log (`timeline.rs`), Hub-light delivery; ticker and News tab | Moderate |
| Relics waking, bearings | `node_awakening_time` → maturity; bearings as per-corp map lines | Moderate |
| Precursor worlds, discovery | A sealed system trait; revealed by arrival charting with a bearing | Moderate |
| The spy's leak and permanent war | Proscription plus a `rebel_since` flag; leak countdown events | Moderate |
| The Keystone | A new structure: level = tier, contributions like syndicate projects | Moderate |
| Instant Command | The Relay Anchor's per-owner delay factor (`relay_factor`) = 0 | Small |
| Network Sight | The per-viewer filter (`picture_for_with_arrays`), fleets only | Moderate |
| Gate Step | The jump-drive relocation; a "Step to…" control | Moderate |
| Precursor Fleet | An existing hull class with a Precursor flag | Moderate |
| Charge, Seal, endings | Season state, a Postgres hall of fame, in-process rollover to a fresh galaxy | Large |

## Art needed when it resumes

- A Precursor world (map sprite, planet-stage painting) and the Keystone in
  stages.
- A Precursor warship hull (sprite, five views, GLB) in its own ancient style.
- Guardians, if Precursor sentinels.
- The colonists' rallying structure.
- Story pop-ups: each decree; sabotage, massacre, quarantine, relic seizure,
  emergency decree; "Evidence found"; "The Authority knows" (the spy's leak);
  "The Precursor worlds stir"; "The Seal fires"; "The Charter holds".
- GNN and Free Signal mastheads; a map glyph for a Precursor world or bearing.
- Already made and unused (from the dropped gate): gate stages 1–5
  (`client/art-src/season-gate-v1/`), the gate glyphs, the `gate-opens`
  illustration.

## Not blocked by the hold

Relics waking, pirate pressure (`docs/r11-pirate-pressure-plan.md`, to be
re-keyed from weeks to maturity), registration closing, derelict
corporations, colony licences, goal ladders and captain gear can go ahead on
their own.

## As built (Oct 29 2026)

Built in five phases, each one playable on :8080. The sim lives in
`crates/sim/src/season.rs` (data and numbers) and
`crates/sim/src/world/season.rs` (systems and tests). The server's season view
is `SeasonView`/`SqueezeView` in `protocol.rs`, with Log lines in
`timeline.rs`. The client's derive module is `core/derive/season.ts`, the
ticker is `shell/deck/news.ts`, and both shells have a News tab in the Log and
grip, Keystone and race sections on the Faction page. Tests are in
`world::season::tests` (27) and `client/scripts/test-season.mjs`.
No galaxy reset is needed: the season clock starts at its first census.

**Open decisions, as pencilled in:**

1. **Maturity** is the mean, over the upper half of the active corporations
   (active within a week), of each one's growth: half for colonies past home
   (8 count as full), half for completed programmes (50 count as full). The
   floor reaches 100% at 10 weeks. It is taken every 60 s and never falls.
   The grip goes II at 25%, III at 45% and IV at 65%.
   - **Levy:** 5% / 7% / 10% at grips II–IV, added to the charter's exchange
     penalty on every Hub buy, sell and fill.
   - **Levy rises:** a trade of 20,000 Cr or more draws a +2% rise, at most
     one a day, up to +15%. It is accepted after 24h unanswered.
   - **Refusals:** a refused rise embargoes the good, with arrears of 25% of
     the trade.
   - **Requisitions:** every 2 days, each worth 4% of valuation (at least
     2,000 Cr), ×1.5 at grip III and ×2 at grip IV. Every other one asks for
     the corporation's largest holding, delivered from its Hub warehouse.
   - **Refusing a requisition:** standing −8 and levy +1%. Three refusals
     bring the enforcement squadron for 12h.
2. **The Assembly Hall:** slot-free at the Command Center, three levels.
   Alloys 30, Provisions 40, Machinery 10. It rallies +1% Resolve an hour a
   level (+3%/h at most) from grip II on, and can't be built before then. It
   uses the Habitat's art until its own lands.
   - **Resolve gains:** levy paid +0.2% per 1,000 Cr. A rise accepted +3%,
     refused +5%. A requisition paid +2%, refused +4%. A crackdown on you
     +5%. An exposed false flag +10% for everyone.
   - **Resolve thresholds:** it fades 3% a day. Below 20% is loyal (levy
     −2%). 50% attunes relics and gives +5% production; 100% gives +10%.
   - **Emergency decree:** when the galaxy's Resolve reaches 50%, the grip
     jumps a stage and every levy rises 2%.
   - **War fleet:** 6 hulls, +24 × maturity, +20 × galaxy Resolve.
   - **Crackdowns:** sabotage comes at grip III once the galaxy's Resolve
     reaches 25%. A staged massacre is blamed on the nearest rebel, and a
     quarantine falls on the busiest freight destination. At most one of
     each every 3 days; each runs 12h. Half of sabotage and massacres are
     staged.
   - **Evidence:** a Scout within 3,000 su of the scene reads it within 24h.
3. **Leak:** 2h. The war takes the finder's syndicate as it stands when the
   leak lands; a member who left before then escapes.
4. **Capture:** locked from level 1, so it can only be sieged down. A held
   blockade by anyone outside the holder's syndicate knocks off a level every
   2h. Taking the world stops the charge.
5. **Precursor Fleet:** a Titan and two Dreadnoughts, raised once.
6. **The charge restarts** after a knock-down.
7. **Guardians:** a pirate warband (2 Destroyers and 2–6 Corvettes, by
   maturity) wakes when a world is found. Colony ships wait until it is gone.
8. **Worlds:** three per 20 home slots, spread between 35% and 80% of the
   home ring, never a home, a neighbour, a relic or a pirate base.
9. **Relics for levels 6–10:** 1 to 5 awake relics held by the syndicate.
10. **Race and backstop:** 10 days from the first find; 12 weeks if nothing is
    found. Registration closes at the first find.
11. **Charter Holds winners:** the top 3 stoppers, then every active loyalist.
    Stoppers score 10 a level knocked off, 1 a rebel hull sunk and 30 a
    Keystone world taken. Bounties pay 1,500 Cr a rebel hull.
12. **Season end:** the season freezes (census and news only), the ending
    pops up and is written to `hall_of_fame` (migration 0007). There is no
    automatic rollover; the next galaxy is a `--reset-galaxy`.
13. **Bots:** stay loyal. They accept rises, pay credit requisitions they can
    afford and never build an Assembly Hall.

**Relics** are the exotic-node systems, as in R11. Their tactical node bonuses
are unchanged. The relic layer is separate: a first third of them wakes at 30%
maturity and the rest by 60%. Attuning a relic takes 24h at 30% maturity,
falling to 4h at 100%. A bearing is a ±0.10 rad wedge, and the client takes
its search zone to be the systems inside two or more wedges.

**Powers:**
- **Instant Command:** `relay_factor` = 0.
- **Network Sight:** the per-viewer fleet filter samples at infinite c and
  counts every fleet as detected.
- **Gate Step:** a `GateStep` command between the Keystone world and the
  syndicate's awake relics.
- **Precursor Fleet:** raised at completion.

**Not built yet:**
- Art: everything under "Art needed" uses stand-ins.
- A dedicated Gate Step control on the fleet panel (it is on the desktop
  Faction page).
- Player false flags.
- Automatic rollover to a new galaxy.
- Mobile ticker (mobile has the News tab).
