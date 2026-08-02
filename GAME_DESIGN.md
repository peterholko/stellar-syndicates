# Stellar Syndicates — Game Design Document

An asynchronous, multiplayer (4–12 player), continuous-time 4X strategy game about
corporate trade and conflict across a wormhole-linked galaxy. You command a chartered
corporation from a home star system, expand into the dark, ship goods to a central
market, and raid — or defend — the convoys that carry them.

**This document describes the game as it is built.** The code in `crates/sim`,
`crates/server`, and `client/` is authoritative; where the two ever disagree, the code
wins and this document is wrong. Numbers quoted here are the shipped constants, all of
which are marked `Tunable` at their definitions and are first-pass playtest values, not
balanced ones. §20 lists what is designed but not implemented; §21 lists what is still
undecided.

### How the code refers to this document

Code comments carry `§` tags of two vintages. Numbered tags (`§6`, `§9`, `§14`) point at
sections here. Named tags (`§TCA`, `§economy`, `§tactical`) name the feature handoff a
subsystem was built from, and map to sections as follows:

| Code tag | Section |
|---|---|
| `§TCA`, `§law` | §9 — the Exchange, the warehouse, freight, the law |
| `§economy`, `§bodies`, `§buildings`, `§step1`, `§industrial-headroom` | §10 — colonies and industry |
| `§tactical`, `§modules`, `§fitting`, `§ladder`, `§arena`, `§battles-take-time`, `§battle-records`, `§theater`, `§engagement`, `§roster` | §8 — ships and combat (`§roster` = the per-ship record and persistent damage, §8.7) |
| `§FLEETS`, `§fleets` | §13 — the fleet as the unit of command |
| `§explore`, `§scout` | §4.3 — the geology knowledge ladder |
| `§node` | §4.4 — exotic nodes |
| `§contestable-territory`, `§blockade`, `§offensive-orders` | §11 — territory and conflict |
| `§ground` | §11.5 — garrisons, bombardment, marines, the settle/conquer split, and the landing engine + its replay |
| `§research` | §12 |
| `§syndicates` | §17 |
| `§pirates` | §18 |
| `§rankings` | §19 |
| `§order-lifecycle`, `§Part 4` | §6.2 and §6.3 |
| `§perf`, `§single-click`, `§market-ux`, and other UI tags | client presentation, not design |

Two numbered tags in the code predate this revision and no longer resolve: `§14.1` and
`§14.2` mean kinematics and the fleet formation rule (now §7 and §13.1), and `§26.2`
means the combat strength weights (now §8.2). `§23.2` means standing orders (§15).

---

## 1. High Concept

You are a remote commander, not a god moving pieces on a board.

The defining mechanic is **lightspeed-delayed observation and command**. You never see
the galaxy as it is now; you see it by the light that has reached your command center.
You cannot steer a distant ship in real time; your orders travel out at *c* and arrive
late. You read reports from the dark and send instructions into it, hoping they arrive
in time and remain relevant.

One line holds the whole design together: **correlation is instant; knowledge is slow.**
You can *settle* a trade across any distance immediately, because settlement is quantum
correlation — but you must always wait for the light to learn what your action meant.

---

## 2. Design Pillars

Five load-bearing commitments. Every mechanic serves at least one; none may violate one.

**1. Async-first. Presence buys awareness, not advantage.** A player who logs in twice a
day is on near-equal footing with one watching constantly. You set intent and doctrine;
the world carries it out without you. This is protected *by physics* — lightspeed lag
makes real-time intervention impossible, so being logged in confers no twitch advantage,
only earlier awareness.

**2. Legibility. You always know exactly how blind you are.** Outcomes resolve from named,
visible factors. Combat does use bounded, seeded, battle-isolated randomness — to-hit
rolls, ±15% damage variance, torpedo interception — but it is *published* randomness: the
distributions are game rules, the pre-commit calculator samples the very same engine to
show win odds and loss bands, and the same seed replays the same battle for every viewer.
The UI states how stale each piece of information is and how long commands will take. You
have certainty about the *extent* of your ignorance and uncertainty about its *contents*,
never the reverse.

**3. Distance is the antagonist.** One spatial variable — distance from the hub and from
your home — drives travel time, fuel cost, information lag, raiding viability, resource
value, and fog. There are no zones and no boundaries; every property varies smoothly with
distance and is read directly off the map.

**4. Decisions are front-loaded. Doctrine over micro, across the void.** For everything
light-delayed, the meaningful decisions happen *before* contact: where to commit, what
route, what pre-authorized behaviour. Three coarse mid-battle verbs exist (withdraw,
reinforce, change doctrine) and they are themselves light-delayed, so they extend the
commitment game rather than replacing it — there is no tactical input layer and never
will be. **The deliberate carve-out: colony development is hands-on and per-body.** Inside
your own gravity well there is no light lag: you site each structure on a specific planet
or moon and staff its lines. Doctrine governs the void; you govern your worlds.

**5. Coherent physics: one speculative leap, rigorously followed.** The universe rests on
exactly one impossible thing (the wormhole lattice). Everything else is real physics
applied straight, so mechanics are consequences players can reason about rather than
rules they must memorize.

---

## 3. The Fiction

The fiction is not decoration. It generates the rules about what is instant and what is
delayed.

### The one impossible thing

When the wormhole opened into the virgin galaxy, its formation crystallized a lattice of
paired singularities throughout the region, like frost spreading from a point. Each is
one end of an entangled pair whose twin is anchored at the **Wormhole Hub** at the
galaxy's center. The lattice is fixed geography: it does not move, it cannot be
manufactured, and it predates every corporation. Whoever administers it controls the only
instant connection across space. That is why charters were sold.

### The Quantum Ledger

The Exchange at the hub runs on the **Quantum Ledger**, built on that hub-anchored
lattice. On chartering, a corporation is issued **settlement keys** — its half of
entangled pairs whose twins sit in the Exchange's vaults. Committing a trade is a local
measurement of a key, which instantaneously collapses the correlated twin at the hub,
settling the transaction across any distance. The collapse is *correlation*, not
information, so it is instant and breaks no law: the no-communication theorem permits
exactly this.

You cannot learn the result instantly. The collapse settles the trade, but *what price
you got* is information, and information travels at light. The Exchange broadcasts prices
outward as ordinary signal; you read an old copy.

Two real quantum principles give the keys their flavour — no-cloning (keys cannot be
copied, so market access is unforgeable) and measurement-destroys-state (a key is
consumed by use). **Keys are fiction only.** They are not a tracked resource and there is
no key economy; they exist to justify why settlement is instant and price information
is not.

### The master principle

One rule adjudicates every "is this instant or delayed?" question:

> **Anything pre-arranged at a fixed, known point is instant. Anything novel, or directed
> at something mobile, travels at the speed of light.**

Standing trade authority at the hub: instant. Setting doctrine or a standing order:
instant (it changes only your own private policy). Redirecting a ship mid-flight:
lightspeed. Seeing a distant price or enemy: lightspeed. A defense platform firing on
whatever enters its radius: instant, because it is a fixed point acting locally.

Corporations can build a faster physical communications network on the hyperspace lanes.
**Comm structures light up the lanes around them:** every structure is a full relay, so a
signal may enter, ride, or leave anywhere in its covered arc, including through overlapping
coverage at lane junctions. A **Hyperspace Buoy** is the expensive long-throw size (80,000
su); a **Hyperspace Repeater** is the cheap short-throw size (40,000 su). Home is not a relay;
each corporation begins with one ordinary, destructible buoy on the lane nearest home.

Own fleets have three presentation states, all decided in the picture the command center can
actually see. **Live:** while the served delayed replay remains inside a comm structure's 2D
throw-radius bubble (with a small hysteresis band), the full sprite advances at true hull
speed. **Dark:** once that replay exits, a heading arrow pins to the final report at the
nominal circle and advances only when strictly arrival-gated warp-light reports reach home.
**Re-entry:** only when that dark stream delivers a report back inside the bubble does the
stale arrow streak to the newly served edge position and become the full sprite. True hull
position never flips the marker early. The live replay is deliberately mildly optimistic for
a receding own fleet; that accepted presentation exception never applies outside a bubble or
to rivals.

The same 2D bubble is the binary boundary for **outbound orders**. If the order's solved
meeting point is inside any owned comm circle, it may ride the covered wire; if the meeting
point is outside every circle, the command center sends one direct warp-light chord with no
partial lane assist. Inbound own-fleet reports retain their per-emission channel, and lane-arc
coverage still governs how an inside report rides home. Dedicated Hyperspace Sensors still
hear rival wakes through their separate tripwire path; a hull never extends relay coverage.

This principle is *generative* — players internalize it once and can then predict the
behaviour of any new situation.

---

## 4. The Galaxy

### 4.1 Space and generation

One continuous 2D radial space, procedurally generated from a seed, with the Wormhole Hub
fixed at the origin. There is no node graph: fleets have real positions and move freely
in any direction.

| Knob | Value | Note |
|---|---|---|
| Galaxy radius | `4000 × √players` su | area scales with player count, so the dark between homes stays proportional |
| System count | `12 + 4 × players` | |
| Home ring | 0.62 × radius | one pre-generated home slot per player |
| System placement | area-uniform in `[0.12R, 0.96R]` | no zones, no structured regions |
| Speed of light | 400 su/s | |
| Sensor bubble | 2200 su | ≈28% of a 4-player galaxy radius |
| Tick rate | 30 Hz | fixed timestep, `dt = 1/30` |

**The light-game invariant.** Light must comfortably outrun the fastest hull, or intel and
orders arrive uselessly stale and raiders feel faster than light. `c ≥ 2 × fastest ship
speed` is asserted at config construction and in tests; the shipped `c` clears it at
about 3.5× (400 against the Scout's 115).

There are no discrete zones. "Core" and "frontier" are relative words players use, not
game regions. Danger, resource value, and lag all rise smoothly with distance from the
hub, and the gradient is read off how the map renders.

**Every player sees a different map** — their own command center's lightspeed-delayed,
fog-filtered reconstruction, not objective truth (§6).

### 4.2 Star systems, bodies, and deposits

A star system is a real place with **planets and moons as first-class entities**. Each
body carries its own kind (rocky, terrestrial, ocean, ice, gas giant), habitability,
deposits, structures, population, and production assignments. The roster is generated
deterministically from the system id, with deposit placement obeying affinity: volatiles
on ice worlds and the icy moons of gas giants, biomass on habitable and ocean worlds,
minerals on rocky ones.

What stays pooled at the *system* level is deliberate: the stockpile (convoys dock at
systems, not at planets), the workforce and specialist pools (labour commutes freely
inside a gravity well), the food state, and the module ledger.

**Deposits carry the frontier gradient.** A system's deposit count runs 1 near the hub to
3 at the rim, and the commodity mix skews toward valuable goods the farther out it sits.
Only the five raw commodities occur as deposits. Deposits are renewable and never
deplete.

### 4.3 What you know about geology

Geology is fogged behind a knowledge ladder rather than published at join.

| Rung | What you learn | How |
|---|---|---|
| Public | position, name, star type, and a richness **band** (Poor / Fair / Rich) | free, identical for everyone — the spectral read from home |
| Surveyed | the exact deposit table | pre-surveyed within 1200 su of your home; otherwise a Survey order, or holding the system |
| Trait | the system's hidden trait | revealed only by owning it |

The band is a pure function of static deposits (`Σ richness × base price`, bucketed at the
galaxy's terciles), so it never changes and is safe to publish. It should predict roughly
70% of a system's worth; the survey buys the rest.

**Surveying** requires a fleet containing a Scout to close within 120 su of the star and
dwell 20 seconds uninterrupted — all or nothing, and the fleet runs 1.5× louder while it
sensors actively. The knowledge then travels home at *c*. Survey data never stales
(deposits are static) and is never lost, even if the system is.

**Hidden traits** sit on a quarter of systems, one each, revealed only by ownership — the
blind claimer's gamble resolving *is* the reveal. Effects are always-on ground truth
regardless of when the owner learned:

| Trait | Effect |
|---|---|
| Bonus Vein | one commodity's richness ×1.5 |
| Deep Deposits | base richness ×1.5, but the extraction tier bonus applies as `^(tier−1)` — the first tier is spent breaking through |
| Unstable Geology | structure recipes cost ×1.25 here (the lemon a survey cannot see) |
| Volatile Pockets | Fuel Refinery output ×1.3 |
| Precursor Cache | a one-time 40 Alloys grant at claim, latched so a capture cannot re-mint it |

### 4.4 Exotic nodes

About 16% of systems have an exotic star — black hole, magnetar, pulsar, or binary. They
are cosmetic until the **awakening**, a single galaxy-wide announced event at a configured
sim time (180 s at playtest scale), after which each becomes a capturable **node** granting
exactly one tactical bonus to whoever holds it, within 1800 su.

| Star | Bonus | Effect |
|---|---|---|
| Black hole | Relay Anchor | command delay (and its echo) ×0.5 inside the region |
| Magnetar | Veil | the holder's *dark* fleets in the region halve their signature |
| Pulsar / binary | Deep Scan | the holder resolves exact composition on any already-visible fleet in the region |

Bonuses unlock tactics, never economic multipliers — the anti-snowball rule. Holding a
node costs 4 Provisions + 2 Fuel per second from that system's own stockpile; starve it
and the bonus suspends until it is fed again, destroying nothing. A corporation draws a
bonus from at most one node at a time, so excess holdings deny rivals but grant nothing
extra. Capture and loss are announced galaxy-wide, light-delayed.

---

## 5. Time & Cadence

**Continuous asynchronous play.** The world runs on its own clock at 30 Hz and evolves
continuously. There is no shared turn boundary and players never wait on each other.

Subsystems run at the cadence that suits them:

| Subsystem | Cadence |
|---|---|
| Simulation tick | 30 Hz |
| Client updates | ~10 Hz (every third tick) |
| Price drift | 1 Hz |
| Limit-order batch clearing | 20 s |
| Corporate valuations | 60 s |
| Standing-order evaluation | 5 s per rule |
| Tactical battle steps | 1 Hz (2 Hz when `battle_target_secs < 120`) |
| Freight departures | 120 s per destination |
| World snapshots | 10 s |

Population growth, production, research, construction, and automation all advance on this
same continuous clock, online or off.

### 5.1 The fairness mitigations

Continuous progress would naively reward whoever logs in most often, violating Pillar 1.
Three mechanisms make progress presence-independent, and all three are mandatory:

- **Queue-ahead.** Research runs from an indefinitely deep queue that auto-advances on
  completion with no login required.
- **Offline accrual.** Rate-based progress — production, population, refining, standing
  orders, autonomous defense — accrues on the server whether or not you are connected.
  There is nothing to collect.
- **No login-gated bonuses, anywhere.** Nothing rewards acting at a specific moment. No
  completion decays if uncollected, no window closes, no re-queue bonus exists.

The corollary, applied without exception across the codebase: **shortages suspend, they
never destroy.** A starving colony underperforms and stops growing; population never
falls and nobody dies. A fuel-poor fleet holds its order rather than losing the ship. A
full stockpile idles production rather than voiding it. An over-posted workforce dilutes
every line by the same share rather than deadlocking. A rejected build never eats its
recipe. A returning player finds their empire hungry and idle, exactly as large as they
left it.

What checking in more often legitimately buys is awareness and decision quality — reacting
to the market sooner, redirecting on fresher intel, re-planning the queue. Never raw
progress. Every future mechanic must pass the test: *does this reward being online at a
particular moment?*

---

## 6. The Information Model

This is what makes the game itself. **There is no objective view of the galaxy.**
Everything you know arrives as light from the vantage of your command center.

### 6.1 The command center

Your command center is the origin of your light cone. It dictates your fog of war and the
delay on both the information you receive and the commands you send. It sits at your home
star system, which is granted free at join along with its command seat.

The command center is a separate, relocatable field on the corporation precisely so it
*can* be moved — but no relocation command exists yet. See §20.

### 6.2 The three clocks, and the order lifecycle

Every distant interaction splits across three moments separated by distance:

1. when you send a command,
2. when the fleet receives it and acts (your command propagated outward at *c*),
3. when you observe the result (the light of the outcome propagated back at *c*).

For a frontier action the round trip can exceed the duration of the event itself: your
command can arrive after the thing it was meant to affect has already resolved.

The sim exposes both timestamps per pending order, owner-only, so the UI can show exactly
where an order sits in its round trip:

| Phase | Until | Reads as |
|---|---|---|
| In transit | `delivered_at` | pure intention — the fleet does not know yet |
| Awaiting echo | `echo_at = delivered_at + distance(delivery point → command center)/c` | executing, unconfirmed |
| Confirmed | — | observed |

Confirmation fires exactly at `echo_at`. A fleet destroyed before then drops silently
rather than producing a phantom confirmation.

### 6.3 Who sees what

Your assets stream sensor data to your command center continuously, each feed delayed by
that asset's light-distance. This produces two correct fog regimes: **your own forces**
appear as a delayed-but-coherent picture, like a broadcast on a known tape delay; **rival
forces** are seen only through your assets' feeds, sharp while you hold contact and
decaying into a growing uncertainty cone the moment it is lost.

Positional uncertainty is `age × speed` and applies to **your own fleets as well as
rivals'**. There is no FTL tether to your own ships: a distant fleet of yours is exactly
as uncertain as a rival's at the same distance. Certainty tracks proximity to your command
center, never ownership.

**Broadcast versus dark.** Under the Galactic Convention, civilian and capital hulls
broadcast identity and position galaxy-wide (light-delayed): convoys, corvettes, colony
ships, Authority freighters, and every capital. Raiders and Scouts run **dark** — visible
only inside a rival's sensor coverage. A fleet broadcasts if *any* member kind does, so
you cannot hide a freighter by parking a raider beside it.

**The detection rule** for dark fleets is one shared function used by both the server's
view filter and the sim's own picket sensing, evaluated from the *retarded* sample's
velocity so a fleet that sprinted then coasted is caught by its old flare:

```
detected  ⇔  distance ≤ sensor_capability(observer) × signature(target)

signature = size_mult × speed_mult × cloak_mult
```

- **size** — per-kind signal summed over the composition, with range scaling as `√signal`:
  six raiders are seen √6 ≈ 2.45× farther than one, louder but sub-linearly. Scout 0.5,
  Raider 1.0 (the reference), Corvette 2, Destroyer 3, Convoy 4, Cruiser 4.5, Colony and
  Freighter 5, Battleship 6.5, Dreadnought 9, Titan 13.
- **speed** — quietest (1/2.5) at or below half speed, ramping continuously to 1.0 at full.
  Flank speed lights you up.
- **cloak** — a research hook, currently a provable no-op at 1.0.
- **capability** — bubble range × a sensor-tech hook, also 1.0 today.

**The normalization anchor: a single raider at full speed is exactly 1.0**, so its
detection radius is the plain bubble.

A fleet carries a **transit throttle** — Full (default) or Stealth, which travels at half
formation speed for roughly double the trip time and the corresponding drop in signature.
Pursuit is always Full.

**Sensor coverage** is the union of bubbles from your command center, every one of your
fleets (a Scout projects 1.5×, i.e. 3300 su), and every Sensor Array on a system you own
(2200 su at tier 1, +880 per tier after). One coverage function feeds all three consumers:
the view filter, picket sensing, and the client's rendering.

### 6.4 The intel ladder

On top of the lightspeed delay, what a rival learns about your fleet comes in tiers:

| Tier | What | When |
|---|---|---|
| Count class | an estimated-size bucket: `1 · 2–3 · 4–7 · 8–15 · 16–30 · 31+` | always, on any visible fleet |
| Composition | exact kinds and counts | only inside sensor coverage (or your own fleet) |
| Cargo | a convoy's manifest | only inside sensor coverage |
| Condition | how beaten up the formation is (an aggregate, never per-hull) | only inside sensor coverage |

Buckets rather than ± ranges, deliberately: an exact N cannot be inverted out of a bucket
the way it could from "±2". You know a hammer is inbound and roughly how big long before
you learn what is in it. Dark fleets are omitted entirely outside coverage, so when seen
at all they show full composition — there is no half-seen dark fleet.

**Scout intel snapshots.** A Scout within 1300 su of a rival-owned system captures that
system's defense and shipyard tiers with a timestamp and the capture position. Delivery
obeys light: the snapshot is knowledge *on the scout*, withheld until its light reaches
the owner's command center. It is a snapshot, not a feed — out of range it ages and never
updates. You know what *was*, not what *is*. The scouted party learns nothing; a
never-detected scout leaves no trace.

### 6.5 The epistemic contract

You always know precisely how stale your information is; the UI declares it everywhere.
You never know exactly what changed in the gap. This is the line between an honest
universe you reason about under defined uncertainty and a game that hides things. The map
renders staleness as a visible property — uncertainty cones that swell between
observations and snap tight on reacquisition — so blindness is something you *see as
shape*, not a number you read.

A loss must always trace to a decision you made, never to the game concealing something
it should have shown.

---

## 7. Movement & Kinematics

Movement is **constant-velocity and piecewise-linear**, at a fixed per-kind speed. Travel
time is simply `t = d / v` and interception is analytic: a closed-form lead point against
a constant-velocity target, not a feedback controller.

An acceleration model (flip-and-burn, `t ≈ 2√(d/a)`) was built and then removed after
playtest. At async check-in cadence the burn was invisible, and its square-root travel law
defeated the mental arithmetic a lightspeed-prediction game needs. The convoy-versus-raider
feel it was meant to produce is now expressed as a flat speed gap instead of an
acceleration-and-mass one. Constant speeds also make the whole information model tractable:
uncertainty is exactly `age × speed`.

| Kind | Speed (su/s) | Hull mass |
|---|---|---|
| Scout | 115 | 80 |
| Raider | 100 | 200 |
| Corvette | 65 | 800 |
| Destroyer | 55 | 2,000 |
| Cruiser | 45 | 4,000 |
| Convoy | 40 | 4,500 |
| Battleship | 36 | 8,000 |
| Colony Ship | 33 | 6,000 |
| Freighter | 32 | 6,000 |
| Troop Transport | 30 | 7,000 |
| Dreadnought | 29 | 16,000 |
| Titan | 23 | 32,000 |

**Fuel is the economic governor.** Dispatching an operation draws Fuel proportional to
`distance × fleet mass` (at `1e-6` per unit-mass-unit-distance) from the owner's system
nearest the dispatch origin that can cover the *full* cost, with a deterministic
`(distance, id)` tiebreak. Cargo adds 28 mass per unit, so a laden convoy costs more to
move but is no slower.

A shortfall **limits, never destroys**: the operation simply does not dispatch, and the
ship, order, or goods are never lost. Fuel hauls are exempt from the charge — otherwise a
fuel-starved colony could never be resupplied. So are recalls, patrols, autonomous
defense, and market deliveries: nothing may strand a fleet or block a defense. A new
corporation's home is seeded with 300 Fuel as its opening runway, because homes produce
no Fuel of their own.

Holding fuel-bearing systems, and refining Volatiles into Fuel near your theater, is
therefore a strategic concern rather than a bookkeeping one.

### 7.1 Standing upkeep — the ceiling on force

Fuel is what it costs to **move**. Provisions are what it costs to **exist**: every fleet
draws Provisions every second, wherever it is, online or off.

This is the ceiling on force, and without it there was none — a hull was a one-off purchase
and an idle navy was free forever, so building up and sitting was strictly dominant and the
economy never pushed back. Now a fleet is a standing commitment measured against what your
colonies actually grow, and the two sinks are complementary: you pay to act, and you pay to
keep.

The charge is on **crew, not tonnage** — a Titan is a city under arms while a convoy is
mostly empty hold — so the sink lands on warfleets and leaves logistics cheap. Scale it
against a fresh home (one staffed Agroplex against a 2.0M population): the spare capacity
feeds roughly eighteen raiders, or ten corvettes, and not one Titan. **A home alone
supports a raiding wing; a line of battle needs colonies behind it.**

**Where it is paid from** mirrors the fuel rule exactly: the owner's nearest system that can
cover the whole draw. A fleet far from any stocked colony is a fleet about to go hungry, so
a forward supply line is what keeps a distant navy in the field.

**A shortfall immobilizes; it never destroys.** An unsupplied fleet takes no new movement or
offensive order, but it keeps its guns, finishes the leg it is flying, defends itself
normally, and recovers the tick food arrives. Nothing is ever lost — a week-offline player
finds a hungry, idle navy, not a smaller one. This is the §5.1 rule applied to the military:
shortages suspend, they do not destroy. There is deliberately no starvation mechanic here;
Travian kills troops that go unfed, and this design will not.

**Garrisons pay the same rent.** Standing troops eat too, drawing from the stockpile of the
system they defend — so a fortified border is a permanent line item, not a one-off purchase,
and the same rule governs it: an unfed garrison stops counting until supply returns, and
loses nothing (§11.5).

Exempt: the neutral sentinels (pirate packs and Authority freighters are nobody's payroll)
and ally garrisons, whose host already feeds them — charging the owner too would bill one
hull twice.

Whether a fleet is fed is **owner-only**. A rival never learns you cannot feed your navy;
that would be a gift.

---

## 8. Ships & Combat

### 8.1 The hulls

Twelve kinds. Five are the working roster a corporation builds from the start; five are
the research-gated capital ladder; one carries troops and is built at a Garrison rather
than a yard; one belongs to the Authority and can never be built.

| Kind | Role | Yard (tier) | Broadcasts | Slots / points |
|---|---|---|---|---|
| **Convoy** | the bulk hauler, 250 units per hull* | Shipyard 1 | yes | 0 / 2 |
| **Scout** | active intel: 1.5× sensor bubble, no cargo, dies deterministically in any engagement | Shipyard 1 | no | 1 / 2 |
| **Colony Ship** | settlement — claiming is physical | Shipyard 1 | yes | 0 / 2 |
| **Raider** | the hunter — a fleet needs one aboard to raid, attack, or blockade | Shipyard 2 | no | 2 / 4 |
| **Corvette** | the dedicated defender; screens, cannot raid | Shipyard 2 | yes | 2 / 5 |
| **Destroyer** | the first ship of the line (beam affinity ×1.20) | Drydock 1 | yes | 3 / 8 |
| **Cruiser** | the armored core (protection ×1.20) — the ladder's efficiency peak | Drydock 2 | yes | 4 / 12 |
| **Battleship** | the siege anchor (driver ×1.20); shortens a siege clock by ×1.25 | Drydock 3 | yes | 4 / 18 |
| **Dreadnought** | the fleet screen made flesh (interception ×1.30 — platform-grade PD) | Slipway 1 | yes | 5 / 28 |
| **Titan** | the flagship: one per syndicate, nameable, ×1.10 to every weapon family | Slipway 2 | yes | 6 / 45 |
| **Troop Transport** | 40 marines; unarmed and slow — an invasion everyone sees coming | Garrison 1 | yes | 2 / 1 |
| **Freighter** | the Authority's scheduled common carrier — never buildable | — | yes | 0 / 0 |

\* The 250-unit capacity bounds the *manual* load commands only. The auto-spawned trade
convoys — production shipments and standing-order dispatches — predate any capacity rule and
are deliberately left uncapped, since retrofitting the limit there would silently change
existing economy behaviour.

**The capital ladder buys presence and role, never efficiency.** Combat weight per
Armaments spent peaks at Destroyer/Cruiser and strictly declines through Battleship,
Dreadnought, and Titan — an invariant pinned by test, not hoped for. Build times run 8
hours for a Destroyer to 8 days for a Titan: a capital under construction is a season
event and a siege target. Rare Elements enter at Cruiser and climb steeply, so the deep-
crust economy is the capital economy.

Four hulls have unusual, deliberate properties. A **Scout** is destroyed with no roll in
any engagement — its defense is speed and darkness, not armor, and losing one is an
acceptable cost. A **Corvette** cannot raid; it defends by being *there* (§8.5). A
**Colony Ship** is consumed on arrival at an unclaimed system, becoming the colony — and
*only* an unclaimed one (§11.2). A **Troop Transport** contributes nothing to a ship
battle: it is a fat hull full of soldiers, it needs escorting, and its whole purpose lands
on the ground.

### 8.2 Strength weights

Force-ratio comparisons — doctrine checks, the pre-commit estimate — use weighted
strength, not head counts.

| Kind | Attack | Defense |
|---|---|---|
| Raider | 3.0 | 2.0 |
| Corvette | 1.0 | 4.0 |
| Destroyer | 2.4 | 2.6 |
| Cruiser | 4.5 | 5.5 |
| Battleship | 8.0 | 12.0 |
| Dreadnought | 12.0 | 26.0 |
| Titan | 24.0 | 44.0 |
| Convoy / Colony / Freighter | 0 | 1.0 |
| Scout | 0 | 0 |
| Defense Platform, per tier | 3.0 | 3.0 |

### 8.3 The tactical engine

Battles are simulated as **individual ships** with positions, range bands, live torpedo
projectiles, and bounded seeded randomness. Three laws govern it.

**Identity.** *(This supersedes the original containment law, which held that fleets were
count-stacks outside a battle and individuals only inside one.)* Every ship in the game is
a real record — its own id, its own fit, its own remaining hull — and a fleet is a
**roster**, not a histogram. Combat is the same engine it always was, but each combatant
now *is* one of those hulls: it enters at that ship's actual health and its remaining
health is written straight back to that ship when the fight ends.

The consequence is §8.7: **damage persists.** Nothing is pooled, nothing is apportioned,
and a hull that never left the reinforcement queue takes no damage at all.

The strategic layer still reads counts everywhere — detection, buckets, movement, fuel,
economy, fog are untouched — because `composition` and the loadout partition are kept as a
derived cache over the roster, rebuilt on every change and pinned to it by test.

**Seeded, isolated randomness.** Every engagement derives its own RNG stream from
`(world_seed, battle_id)`. Same seed, same battle, byte-identical for every viewer — and
the battle stream never touches the world's RNG, so adding or removing a battle shifts no
unrelated draw. This is test-enforced.

**No input creep.** Role scripts are few, dumb, published constants — game rules, not AI.
There is no per-player scripting and no formation editor, and there never will be.

The arena is battle-local, 1000 su in radius. The defender anchors at the origin; the
attacker deploys from their real map-approach bearing at 900 su standoff. Five role
scripts: Anchor (capitals hold the line's center), Line (advance to the preferred band and
fire), Screen (PD ships interpose on the torpedo threat axis), Skirmish (fast hulls orbit
the flanks), Withdraw (burn for the edge under literal pursuit fire).

**Where the emergence lives.** To-hit rises with target *mass* and falls with target
*speed*, per weapon family. Beams track well and are nearly flat; drivers punish big slow
hulls and whiff on darting corvettes; torpedoes near-guarantee against capitals and
struggle against small fast ships. The capital-hunting torpedo and the wolfpack answer to
it are emergent from tracking, not bolted-on multipliers.

| | Beam | Driver | Torpedo |
|---|---|---|---|
| Range | 650 (hitscan) | 350, falling off to 650 | launched from 800, flies at 140/step |
| Cooldown | 1 step | 2 steps | 4 steps |
| Base to-hit | 0.85 | 0.70 | 0.90 |
| Mass sensitivity | 0.05 | 0.35 | 0.50 |
| Speed sensitivity | 3.60 | 4.40 | 5.40 |

To-hit is clamped to [0.05, 0.95], damage varies ±15%, and the first five steps carry a
×1.25 opening bonus. Sides are capped at 300 combatants; fleets beyond it hold as
reinforcement waves committing heavies first, so huge fleets fight in echelons.

**Battles take time, at a configured scale.** `battle_target_secs` is the duration two
equal reference forces take to grind to their retreat thresholds — 45 s at playtest scale,
2700 s in production, where battles run at the scale of light delays and relief travel.
Lopsided fights still end fast. Two safety rails bound the extremes: a raid ends after at
most 35% of the target duration (raids stay smash-and-grabs even when battles are slow),
and an engagement running 2× the target without either side retreating forces a mutual
disengage.

**Battles anchor.** On contact both sides drop to near-zero velocity: a stationary event
that suspends prior missions, so a slow hammer can be pinned while relief travels.
Survivors resume their course afterward. Doctrine evaluates immediately on contact — a
fleet on Avoid that gets jumped takes a brief parting-shot exposure and then the speed
table decides whether it escapes. A raider outruns corvettes; a colony ship outruns
nothing. Only fleets that *accept* battle stay anchored.

Three coarse mid-battle verbs exist, all light-delayed: **withdraw** (physical disengage at
formation speed, escape decided by the speed table), **reinforce** (a friendly fleet
arriving joins its side's pool and shifts the ratio), and **change doctrine**. Defender
home-field advantage falls out of the physics: command delay is shorter near your own
command center.

An engagement is a persistent, observable entity. It is light-gated in the view like
anything else — a third party sees "battle raging, as of N ago" only by their own light —
and weapons fire reveals *all* participants at the site, even dark fleets.

### 8.4 Modules and fitting

Few modules, qualitative hard counters, no quality tiers. Every weapon has exactly one
counter, and the matrix is one-to-one by rule.

| Module | Family | Points | Does |
|---|---|---|---|
| Mass Driver | Driver | 2 | fires drivers at ×1.3 |
| Torpedo Rack | Torpedo | 3 | fires torpedoes at ×1.6 — the hardest hit, and interceptable |
| Point-Defense Screen | Interception | 2 | rolls intercepts on torpedoes crossing its bubble; its own beam runs at ×0.5 |
| Reflective Plating | Protection | 2 | blunts incoming beam by 35% |
| Whipple Armor | Protection | 3 | blunts incoming driver by 45% |

Beams travel at *c*: nothing intercepts or jams a weapon that arrives with its own light,
so reflection is their only counter. Torpedoes ignore both armors; interception is their
answer, and it costs a weapon slot to field. ECM is deliberately absent so PD owns
anti-torpedo alone.

**PD is literal, not a share calculation.** Each PD-fitted ship rolls intercepts (base
0.35) against torpedoes crossing its screen radius — 180 su, or 400 for a Dreadnought or a
platform. Screening is positional truth: the corvette actually standing between the
torpedo axis and your battleship intercepts more.

**Fitting is allocation, not marks.** A hull has both a slot count and a fitting-point
budget, and depth comes from how you spend the budget — there are no numbered module
tiers, ever. Duplicates of a chosen weapon stack linearly; the budget is the brake. The
classic trade-offs bind on subcapitals by construction: a torpedo corvette cannot also
armor up (3+3 > 5), driver-plus-Whipple fits a corvette exactly (2+3 = 5), and a torpedo
raider is a glass cannon (3 of 4 points, and nothing costs 1). Capitals are where
combinations live.

Modules are manufactured items, not abstractions: built from Armaments and Electronics at
an Armaments Complex, pooled in a per-system ledger, shipped by raidable convoy (12 crates
per hull), and installed at an Ordnance Foundry. Refitting takes 3 s per hull and pulls them
safely out of combat into the yard. Sol sells modules at 2× their goods value and buys back at
0.5× — a deliberately steep round trip, so modules are for fitting rather than arbitrage,
and local manufacture is always cheaper.

A syndicate can save up to 24 named **doctrine fits** (hull + loadout), validated against
slots and budget at save time, so every stored fit is legal by construction.

### 8.5 Standing defense

Defense runs on the server clock whether or not the owner is connected. Four layers, in
the order a raid meets them.

**Corvette screens.** Every friendly corvette within 1300 su of a raid contact on a
civilian ship duels the attacker first, nearest first. Shadowing a convoy makes it an
escort; parked at an owned system it is a garrison. Corvette losses are real ships.

**Defense Platforms.** Within 1300 su of the system, a hostile raider making contact with
one of the owner's convoys must fight through the platform's tiers first, resolved as
sequential duels. A lost unit costs a tier (damage, and the slot frees up — the system is
never destroyed); killing the raider or driving it off stops the raid; fighting through
every tier reaches the convoy. The platform senses exactly its own radius, because the
contact is physically inside it. Its existence never leaks in the view: a stopped raid
reports through the ordinary channel, and the attacker learns only "destroyed" or "driven
off". Deterrence has to be discovered the hard way.

**Autonomous pickets.** A patrolling raider senses hostiles within its own bubble (or any
covering Sensor Array of its owner), adopts a nearby friendly convoy as its charge, and
breaks off to intercept threats on an intercept course toward it. Where a patrol sits
decides what it can defend. Governed by corp doctrine (§16).

**Per-fleet posture.** A fleet set to Weapons Free auto-commits on rivals detected in its
*own* bubble — forward autonomy that needs no command-center round trip — subject to the
corp doctrine's force-ratio and retreat gates.

### 8.6 Records, replay, and the pre-commit estimate

**Battle records** capture participants and truth keyframes, retained 7 days with a floor
of 25 per corporation and a global cap of 2000, targeting 40 recorded rounds per battle.
Fidelity follows the fog: a participant sees detail, a distant observer sees buckets.
Records stream incrementally — each round is sent once, not re-sent forever.

**The Battle Theater** replays those records in a Pixi scene: real ship sprites sized by
mass, per-family weapon effects, torpedoes curving into flak, armor glints versus spall,
deaths at their exact recorded positions, drifting debris. It is strictly a replayer — all
effect volume derives from the record, all cosmetic placement from a PRNG seeded on
`(battle id, round)` — so two renders produce the identical scene, scrubbing back replays
identically, and nothing in it can influence resolution.

**The pre-commit calculator** is Monte Carlo over the real engine: 32 seeded rollouts of
the same headless `simulate_engagement`, run on the observer's own view data. Your fleet
exact; the target's ghost at its retarded state, exact inside sensor coverage and
otherwise a typical warfleet of the *bucket midpoint* — provably never the true count;
their defenses from your aging scout snapshot, or marked unknown. It reports a win rate,
interquartile loss bands, and the age of every input. It must call the shared combat
function (so it can never drift from reality) and must not touch authoritative state. It
is reality's exact function, sampled, on stale inputs.

---

### 8.7 Damage persists

A ship that survives a battle carries its wounds out of it. Hull damage lives on the
individual hull (§8.3), rides the snapshot, travels with the ship through a merge or a
split, and does not heal on its own — anywhere, ever.

What this changes: attrition is now permanent until serviced, so a war grows a logistics
tail; a pyrrhic win costs something real; and **withdrawing becomes genuinely attractive**
— preserve the hulls, mend them, come back. That last one is the point. It converts
battles from isolated events into campaigns.

Damage does exactly one thing mechanically: your next fight starts with those ships already
hurt, so they die sooner. It does *not* reduce attack or defense weights, slow a ship, or
change its detection signature — keeping it out of the strategic layer is what leaves force
ratios, doctrine, and the fog model untouched.

**Fog.** How beaten up a fleet is rides the same gate as its exact composition: visible
inside sensor coverage, invisible outside. A wounded formation is therefore the best target
on the map *and* something you must get close to discover. What crosses the wire is a
single aggregate fraction — never the per-hull roster, which would hand a distant observer
the exact count the size bucket exists to withhold (§6.4).

**Repair** is the relief valve, and it is deliberately geographic. An **Ordnance Foundry**
services any of the owner's idle fleets docked at it, restoring hull on the same factor
chain as every production line (`tier × staffing × skill`) and paying goods per hull point
from that system's stockpile. An unstaffed or unsupplied yard simply mends less — down to
nothing — and resumes the moment supply returns: a shortfall never destroys, because damage
comes from combat and never from neglect. A forward foundry keeps a fleet in the fight; a
rear one costs you the trip home.

Two boundaries keep repair from becoming free:

- **A fleet under fire is not in the yard.** An engagement only anchors a fleet — it keeps
  whatever order it held — so a fleet parked at its own foundry and jumped there would
  otherwise heal as fast as it was being shot, making a defended foundry system absurdly
  hard to crack. Engagement participants are excluded; the yard gets back to work the moment
  the shooting stops.
- **A refit is not a repair.** The hulls that enter the yard are the hulls that leave it —
  re-fitted, but no healthier. (The queue carries the ships themselves for exactly this
  reason. Carrying counts instead would mint fresh full-health hulls on completion, and
  since a yard takes the most-damaged hulls first, that would be an optimally efficient free
  repair.)

This is not async-unfair: nothing decays while you are away. An unrepaired fleet is simply
still hurt when you get back.

## 9. The Market & the Terran Charter Authority

### 9.1 Execution versus information

**Execution is instant everywhere.** Committing a trade collapses your settlement key's
twin at the hub; settlement is correlation and therefore distance-independent. There is no
clearing-batch wait on a market order, anywhere.

**Price information lags.** The Exchange ticker is a lightspeed broadcast: its staleness
is the hub-to-command-center light delay, and it is disclosed as such. This applies at
home too — commanding from your home star system does not exempt you from the hub's light
delay. The displayed price is a guide; execution happens at the true current price.

Prices walk with flow along a simple elasticity curve (1600 units of flow moves a price by
about 100%), mean-revert toward a base at 2% per drift step with a little noise, and never
fall below 0.5.

### 9.2 Trade and haulage are separate acts

The Exchange settles against your **Charterhouse warehouse**, a private stock you hold at
the station. A buy deposits into it; a sell and a sell-side limit escrow draw only from it.
**Nothing about a trade moves goods across space, in either direction.** Both sides are
therefore symmetric and price-certain: the goods are already at the Exchange, so there is
no crossing and no price-on-arrival gamble.

This replaced an earlier asymmetry in which a buy conjured a free delivery convoy home and
a sell committed goods to the crossing first, clearing at whatever the price was on
arrival. That coupling made trading and hauling one indivisible act you could not opt out
of. Splitting them keeps the danger while letting a player *choose* when to expose goods
to the dark. Convenience survives as composition rather than coupling: a buy carries an
optional "deliver to system X" that books Authority freight for the lot the instant it
settles, and if the booking cannot be honoured the goods simply stay in the warehouse and
you are told why.

**There are exactly two places a corporation's goods can sit:** the Charterhouse
warehouse, and an owned system's stockpile. An earlier third store — a per-corp pool at
the home anchor — was retired: it lost its purpose when the Exchange moved onto the
warehouse and survived only as a dead-end pocket goods could enter and never leave. Its
inflows now land in the home *system's* stockpile, a real place the player already manages.

**Order types.** Market orders execute instantly against the standing price. Limit orders
rest and clear in a periodic uniform-price call auction every 20 s — the anti-sniping
mechanism, since within a clearing arrival order is irrelevant and everyone clears at one
price. Scoping the batch to limit orders only is what preserves the instant market-order
feel. A limit order placed against a stale book is accepted as-is: there is deliberately
no stale-price protection, only the Pillar-2 requirement that the UI always show *that*
the data is stale and *how* stale.

### 9.3 Where the danger lives

The danger is in the **crossing**, not in the knowing. Price advantage and delivery risk
are decoupled on purpose: a great fill on goods that then get raided is not a clean win.
This is why information-relay exploits — a smurf at the hub relaying live prices — gain
little. They buy a slightly better forecast on a trade you still cannot speed up or
deliver safely. The prize was never the price; it is the safe crossing.

The crossing is an explicit choice between two channels, and choosing between them is the
logistics game:

| | **Authority freight** | **Your own convoy** |
|---|---|---|
| Who flies it | the Authority's scheduled carrier | a hull you built and loaded |
| Cost | a fee, charged at booking and destroyed | free — you already own the ship |
| Timing | fixed 120 s timetable; 400 units per corp per departure | whenever you like |
| Risk | someone else's hull, but your goods are aboard, and it can be raided | yours to escort, route, and lose |
| Reward | — | counts as trade throughput on the leaderboard |

Neither is strictly better. Freight is the low-attention default that keeps a distracted
empire running; flying it yourself is cheaper, faster to schedule, and the only way to
escort what matters.

### 9.4 The Authority

The **Terran Charter Authority** is the home-galaxy body on the far side of the wormhole
that issued every charter. It operates the **Charterhouse** — the hub station and its
Exchange — and a scheduled common-carrier freight service. It is a neutral institution,
not a player: it holds no territory, never appears in rankings, and takes no side. It owns
physical freighter hulls through a sentinel id, exactly as the pirate enclaves do.

- **The warehouse** is the Exchange's only counterparty. No capacity limit and no storage
  fee, for now.
- **Scheduled freight** books a lot outbound (warehouse → an owned system) or inbound (an
  owned system → warehouse, optionally sold the moment it lands). Goods are escrowed and
  the fee charged at booking; the fee is a pure credit sink, destroyed rather than paid to
  anyone, and never refunded. It has an ad-valorem part (6% of value) and a distance part
  (1e-4 per unit per su), so long hauls cost more. Departures run one freighter per
  destination that has anything waiting in either direction. An oversized lot is never
  refused; it rides several consecutive departures. **The terms are the Authority's price
  list** — both the fee and the cap are uniform across destinations, and nothing the
  destination has built moves either number, because how much a freighter lifts belongs to
  the hull.
- **Freighters are real objects.** They broadcast like any civilian hull and can be raided
  (the manifest is stolen) or destroyed (everything aboard is lost). A manifest is two-tier
  per entry: you always see your own lots, anyone else's only from inside sensor range.
  Pirates ignore Authority hulls.
- **The Authority holds your goods.** A lot that cannot land — the system changed hands, or
  its storage is full — rides back to your warehouse rather than being destroyed.
  Deliberately friendlier than the convoy cargo-lost rule. Freight also respects the
  storage cap, so it cannot smuggle goods past a limit convoys obey.
- **Light-honest refusals.** The Charterhouse refuses bookings to a system it *believes*
  blockaded, on its own light-delayed knowledge: it keeps accepting until the blockade's
  light reaches the hub and keeps refusing until the lift's does. Freight already in flight
  carries on — it launched on information that was true when it left.
- **Sovereignty.** No engagement may open within 900 su of the Charterhouse, for either
  party. Fleeing into it is sanctuary, by design.
- **Sol's off-map industry** lists all twelve commodities from day one, plus specialist
  contracts at 800 credits and modules at a 2× premium. This is the bootstrap: early
  Machinery comes from Sol, and the intended arc is extract → sell raws → buy Machinery →
  build industry → make your own.

### 9.5 The law: standing, citations, and enforcement

The Authority's protection of its own hulls is **retributive, not preventive**. It runs no
patrols and posts no escorts, and the frontier stays lawless. What it does instead is
*remember* and *price*.

**This is priced outlawry, not prohibition.** Every consequence is a cost a player can
knowingly pay. None is a wall. If a band ever makes attacking Authority freight strictly
irrational, the tuning is wrong.

**Charter standing** is one number per corporation, starting at 100 and regenerating
unconditionally at 0.02/s in *every* band — time served is time served, so nobody is ever
locked out by arithmetic alone. Each incident costs 10 points; destroying an enforcement
vessel costs 20. The five statuses are derived from standing and never stored, so there is
no cached copy to desync.

| Band | At | What it costs you |
|---|---|---|
| Good Standing | 100 | nothing — no tariff, no fee, nothing withheld |
| Sanctioned | below 100 | freight tariff and an Exchange penalty fee, both ramping with the fall |
| Suspended | ≤ 60 (≈4 incidents) | …and no *new* freight bookings; already-booked shipments still complete |
| Revoked | ≤ 20 (≈8 incidents) | …and the Exchange is closed; resting orders are grandfathered and your warehouse is still yours to fetch from |
| Proscribed | ≤ −20 (≈12 incidents) | …and the Authority sends enforcement expeditions |

Both penalties ramp linearly to their ceiling at Revoked — tariff ×3.0, Exchange penalty
10% of trade value — and then clamp. The deeper bands answer with expeditions, not an
ever-steeper bill. A corporation in Good Standing pays *exactly* nothing, which is what
keeps the economy's clearing invariants untouched.

**Citations arrive at *c*.** Killing a freighter changes nothing at the scene. The incident
travels to the Charterhouse at lightspeed; only on arrival does standing move and a public
bulletin issue naming the culprit, which then radiates outward to every player at *c*. A
spree deep on the frontier drags a visible light cone of consequences toward the map's
center behind you. The reputational hit and the legal one ride the same wavefront.

**The Authority protects only its own hulls.** Raiding a rival's convoy is ordinary
frontier business and produces no citation, ever — asserted directly by test.

**Enforcement expeditions** are scripted, announced, and survivable: six corvettes sail
from the hub every 420 s to blockade a proscribed corporation's nearest holding, using the
ordinary blockade mechanic unmodified. Corvettes rather than raiders, because raiders run
dark and an *announced* expedition must be visible. The announcement's light outruns the
sub-light squadron, so the warning genuinely arrives first — that is the lead time. An
expedition can be fought (destroying it ends it early, at the cost of a graver citation),
waited out (it withdraws after 180 s on station), or called off by paying up. It always
withdraws long before any siege clock could capture anything: it costs a proscribed
corporation economy-time, never a colony, and that safety margin is asserted by test.

**Reinstatement** buys standing back at 25 credits a point, burned as a sink, clamped so
you are only charged for points actually restored. Paying visibly calls off an inbound
expedition — the most direct expression of the whole design: the law is a bill, and you may
settle it.

Deliberately *not* built: privateering and letters of marque, syndicate-shared standing,
Authority bounties or escorts, and any standing effect from player-versus-player combat.

---

## 10. Colonies & Industry

Nothing produces by itself. A deposit needs its extraction structure *staffed*; a
converter needs crews and inputs. Every output is one legible factor chain:

```
output = base × tier_throughput × staffing × skill × food
```

where `base` is a deposit's richness (extraction) or the converter's rate, and every
factor is a number the player can read in the colony panel and act on.

### 10.1 The industrial web

Twelve commodities in three rungs. Five raws occur as deposits; five processed goods are
made from raws; two advanced goods cap the chains.

| Commodity | Rung | Base price | Made by | From |
|---|---|---|---|---|
| Biomass | raw | 5 | Bioharvester | deposit |
| Silicates | raw | 6 | Mining Complex | deposit |
| Metallic Ore | raw | 8 | Mining Complex | deposit |
| Volatiles | raw | 9 | Volatile Harvester | deposit |
| Rare Elements | raw | 22 | Mining Complex | deposit |
| Provisions | processed | 9 | Agroplex, 1.2/s | 1.0 Biomass |
| Fuel | processed | 14 | Fuel Refinery, 0.8/s | 1.0 Volatiles |
| Polymers | processed | 16 | Chemical Works, 0.8/s | 1.0 Volatiles + 0.8 Biomass |
| Alloys | processed | 26 | Smelter, 1.0/s | 1.5 Metallic Ore + 0.3 Fuel |
| Electronics | processed | 34 | Electronics Fabricator, 0.5/s | 0.8 Rare Elements + 0.8 Silicates |
| Armaments | advanced | 56 | Armaments Complex, 0.35/s | 1.0 Alloys + 0.5 Electronics + 0.5 Polymers |
| Machinery | advanced | 62 | Machine Works, 0.3/s | 1.2 Alloys + 0.6 Electronics + 0.4 Fuel |

Rates are units of *output* per second at throughput 1.0. Two invariants are test-enforced
against this same table, so recipes and prices cannot drift apart: every processed and
advanced good's base price clears its input basket at base prices (industry is worth doing
without making raw-selling worthless), and no basket sums to less than 1.0 unit per output
(so conversion never net-adds units and industry cannot overflow a storage cap).

At home-geology extraction rates converters run input-bound. The rate ceiling starts to
matter when bulk raws are *imported*, which is exactly what makes a supplied forge world an
engine — and its supply line a target.

Every commodity has a job: Metallic Ore and Alloys build, Fuel operates, Provisions
sustain, Volatiles refine, Rare Elements feed capitals, Machinery and Armaments cap the
chains.

### 10.2 Structures and slots

Twenty structures across three slot pools. Slot budgets are **derived, never stored**, so
the whole system is migration-free by construction.

| Pool | Structures | Slots per body |
|---|---|---|
| Resource | Mining Complex, Volatile Harvester, Bioharvester | `min(deposits, 4)` — a bare rock hosts no extraction |
| Industrial | Smelter, Electronics Fabricator, Chemical Works, Fuel Refinery, Machine Works, Armaments Complex, **Shipyard, Naval Drydock, Capital Slipway, Ordnance Foundry** | 2 (0 on a gas giant — nowhere to stand) + population tier |
| Infrastructure | Agroplex, Habitat, Orbital Warehouse, Sensor Array, Defense Platform, Academy, **Garrison** | 1, +1 if habitable, +1 once developed |

Each *distinct* built structure consumes one slot; tiers deepen in place, so the budget
prices breadth rather than depth. A slot-full system soft-rejects: no debit, no job, and an
owner-only notice. Ships are units, never slot-gated.

The Agroplex sits in Infrastructure on purpose: food security is civic, so a Habitat plus
an Agroplex makes a self-feeding outpost on the base slots with no industrial investment.

Throughput per tier is `[0, 1.0, 2.2, 3.8, 6.0, 8.6, 11.5]` — deliberately superlinear, so
one deep tier out-produces spreading the same slots wide and a focused colony reads
differently from a sprawling one. Tier 4 is where an unresearched colony tops out; tiers 5
and 6 are research prizes.

Several structures do something other than produce goods.

**The yard family** builds hulls, and it is a ladder rather than one tall building — the
cost of a capital fleet is slots and geography, not just tiers. Each yard needs the one
below it standing somewhere on the same *system* (the ladder belongs to a shipbuilding
world, so it may spread across bodies), and each is gated separately in §8.1:

| Yard | Builds | Needs |
|---|---|---|
| **Shipyard** | Convoy/Scout/Colony (I), Raider/Corvette (II) | — |
| **Naval Drydock** | Destroyer (I), Cruiser (II), Battleship (III) | Shipyard ≥ 2 here |
| **Capital Slipway** | Dreadnought (I), Titan (II) | Naval Drydock ≥ 3 here |
| **Ordnance Foundry** | installs refits · **repairs damaged hulls** (§8.7) | Shipyard ≥ 1 here |

**A yard's tier is also its slipway count** — a tier-N yard holds N hulls on the stocks at
once, and each yard kind counts its own slips, so a Shipyard busy with convoys never blocks
the Drydock's line warships. Before this, ship jobs were unbounded and tier was a pure gate.
A full yard soft-rejects on *timing*: nothing is spent, and the same order lands the moment a
slip frees. Every home generates with Shipyard 1 pre-built, so convoys and scouts build on
day one (one at a time) and raiders are earned. A staffed yard builds up to 25% faster,
locked in when the job starts, from the crews on the yard that gates that hull.

The **Ordnance Foundry** is the outfitting-and-maintenance yard: it changes what a hull
carries and mends what a battle took out of it (§8.7), rather than laying new hulls — so a
forward system can service a fleet without being a construction base.
(Module *manufacture* stays at the Armaments Complex — armaments make the crates, the
foundry installs them.)

The **Garrison** is the ground war's only building, and it faces both directions at once
(§11.5). Standing troops defend the body they sit on and lengthen a besieger's clock; the
same barracks builds the Troop Transports you invade someone else with. It is barracks and
armories rather than heavy industry, so it costs Infrastructure slots and sits on a
habitable body — soldiers need somewhere to live. It eats Provisions like any standing
force, and an unfed garrison defends *nothing* until supply returns (§5.1: shortages
suspend, they never destroy).

The three remaining non-producing structures:

- **Orbital Warehouse** raises the system's total storage cap: 700 base, +400 per tier. A
  full system's production *idles* at the cap and resumes when goods ship out. Over-cap
  stockpiles are grandfathered — the cap blocks new inflow only and never destroys what is
  stored. An inbound delivery fills the headroom and the same convoy carries any excess
  onward to sell at the hub.
- **Sensor Array** projects a standing sensor bubble for its owner (§6.3).
- **Defense Platform** is the priciest development, because fortification is an investment
  (§8.5).

Everything advanced needs Machinery, and early Machinery comes from Sol. That is the
intended bootstrap loop, not an oversight.

### 10.3 Population, food, and workforce

Population lives on bodies, in their Habitats, and is measured in millions. **Population
never decreases** — the hard rule. Famine walks the food ladder down and freezes growth; it
never kills.

- Habitats are the only source of capacity: 4M per tier. No Habitat, no growth, so a
  ship-founded outpost holds at its founding size until housing goes up.
- Growth is linear and flat at 0.002M/s, only while Well Supplied and under capacity.
- A colony ship plants 0.5M — a hungry mouth first, not an instant workforce. A home starts
  at 2.0M with 60 Provisions banked.
- Population eats 0.06 Provisions per second per million.
- Workforce units are `floor(population / 0.8)`. A tier-N structure wants N crews for full
  throughput; under-crewing runs it pro rata. Over-posting the colony's workforce is legal
  and simply dilutes every line by the same share — fair, legible, and deadlock-free.

The **food state** is a four-rung ladder recomputed each tick from how many seconds of
demand the local stockpile covers:

| State | Coverage | Efficiency |
|---|---|---|
| Well Supplied | ≥ 30 s | 1.00 — the only state in which population grows |
| Rationing | ≥ 8 s | 0.85 |
| Critical | > 0 | 0.50 — advanced industry stops here |
| No Provisions | empty | 0.00 |

Degradation is immediate; improvement requires clearing the next rung by a 1.5× margin, so
a colony hovering at a boundary never flickers or spams notices. Primary-sector work —
extraction and the Agroplex — never drops below half rate: miners feed themselves off the
land, and the Agroplex floor is what makes famine *recoverable*. Without it, an empty
larder would be a death spiral.

### 10.4 Specialists

Specialists are rare people, not goods: a large optional multiplier that must be physically
transported. Five professions, each with an affinity table naming the structures its skill
applies to — Geologist, Petrochemical Engineer, Xenobiologist, Industrial Engineer, Naval
Architect. Off-affinity a specialist still counts as generic crew, never a penalty.

A fully specialist-staffed line reaches ×1.75 skill, pro rata below. Two sources: a Sol
contract at 800 credits (price-certain, delivery-risky — the personnel convoy from the hub
is an ordinary raidable hull) or an Academy course (40 s, 20 Provisions + 10 Electronics),
which is the patient road and keeps Sol from being a permanent monopoly.

Passengers ride the same two-tier fog rule as cargo: the broadcast never includes them, a
sensor-revealed manifest does. Only logistics hulls have berths — 4 on a convoy, 8 on a
colony ship. They are lost with the ship, folded in by a merge, and disembarked into a new
colony when a colony ship is consumed.

---

## 11. Territory & Conflict

### 11.1 Claiming is physical

There is no instant credit purchase of a star system. To claim, you build a **Colony Ship**
and send it. On arrival at a still-unclaimed, non-reserved system, ownership transfers and
the ship is consumed — it *became* the colony, leaving no wreck — and the claim's light
propagates outward normally.

This makes expansion telegraphed, raidable, and escortable: a colony ship broadcasts, it is
the slowest hull flying, and destroying it in transit means the colonists are lost and no
claim ever lands. **The race** goes to the earlier arrival tick, with a deterministic
ship-id tiebreak on the same tick. The loser *holds* at the spot — intact and redirectable,
with one owner-only light-delayed notice per hold — and settles elsewhere when re-sent.

An earlier instant `ClaimSystem` command was removed. The colony ship's recipe absorbed its
economics; the `claim_cost` field survives on the wire as a deprecated system-value scalar
that charges and gates nothing.

### 11.2 Blockade, siege, and capture

A fleet containing at least one raider can take station on a rival system and strangle its
logistics. Blockade orders are fuel-charged and light-delayed like any move, and the target
system's standing defenses engage the blockader as any hostile contact. Inbound convoys to
a blockaded system halt at 900 su — held off, not destroyed.

**A held blockade strips the stockpile it strangles.** Goods come off the colony's books at
a steady rate and land in the blockader's own hold, so they still have to be carried home
across contested space — the crossing stays the danger, exactly as it is for a raided
convoy. A warship's prize hold is small; bringing hulls built for hauling is how you carry a
colony's output away properly.

This is bounded three ways: the per-second rate, the room in the hold, and — the important
one — a **per-commodity reserve the besieger can never strip below**. A colony always keeps
a working floor, so it goes on producing, its people go on eating, and it can recover when
the siege lifts. Nothing here may starve a colony out; §5.1 binds a besieger too.

*Why a blockade rather than a raid order:* a stockpile is a fatter, stationary target than a
convoy, so a hit-and-run verb against one would be strictly better than chasing convoys and
would hollow out the raiding loop this is meant to complete. Requiring a **held** blockade —
visible to the victim from the moment it establishes, contestable, fought through the
system's platforms and garrison — keeps convoy raiding the fast option and stockpile theft
the committed one. It also gives blockade standalone value: before this it was only ever a
step toward a siege.

The victim learns of the theft by **light from the system**, exactly as they learn of the
blockade itself. Owning ground grants no faster-than-light knowledge of what is happening
on it.

An unbroken blockade with the defenses suppressed starts a **siege clock**: 8 ×
`battle_target_secs` (about 6 minutes at playtest scale, hours in production, long enough
that a realistic check-in cadence can mount relief). A Battleship-or-heavier hull on
station divides that by 1.25 — the siege anchor; a standing **Garrison** multiplies it back
up, because troops on the ground make a siege slower as well as bloodier (§11.5). The clock
starts when the conditions first hold and resets if they lapse. Progress requires no
garrison combatant on station and a suppressed Defense Platform.

Serving the clock does not take the system. It opens the door: the fleet overhead has won
the *orbit*, and the ground is a separate problem (§11.5). What the captor eventually
inherits is deliberately a *damaged* base:

- every structure tier is halved, rounded down, freeing slots per the ordinary damage rule;
  the defense pool is cleared;
- the stockpile stays on the system as plunder, snapshotted first so the defender's report
  itemizes exactly what was lost;
- **population stays** — people do not vanish with the flag, so never-decrease holds even
  here. A halved Habitat may leave the colony over capacity, which just freezes growth;
- the geology survey knowledge transfers as spoils, and holding the system reveals its
  hidden trait (a Precursor Cache that already paid cannot be re-minted by a flip);
- the old owner's in-progress builds are dropped — they paid, but they no longer own the
  ground.

### 11.3 Homes are protected, and nobody is eliminated

**A home system can be blockaded but never captured.** A beaten player always keeps a
producing base. There is no elimination and there is no victory condition; standing is
expressed through the rankings (§19).

This is a deliberate departure from the original design, which had victory by conquest of
rival homes. See §20 for what that would still require.

### 11.4 Raid versus attack

Two verbs with different intents, both reusing the same intercept-commit pursuit:

- **Raid** (raider-only) steals. On contact it opens a brief skirmish under the raid cap
  and seizes cargo.
- **Attack** (fleet needs a raider) destroys. On contact it opens a full-duration battle
  regardless of the target's kind, and a destroyed fleet's cargo is lost with it.

Both are fuel-charged and light-delayed through the order lifecycle. You commit on a stale
ghost; the fleet then pursues the target's *true* position with better, fresher information
than you have. You watch the chase unfold late and can intervene only through
lightspeed-lagged commands that usually arrive too late to matter.

A raider can die with your unheard recall still in flight, and that is the design working —
*provided* the loss traces to your commit-time decisions (intel and doctrine) and never to
latency-as-randomness. The defender's experience mirrors it: they learn their convoy is
hunted from stale information, can do almost nothing in the moment, and survive or die on
the routing, escort, and doctrine they chose at dispatch. Both players are equally remote,
equally pre-committed, equally watching. That symmetry is what makes it fair despite being,
from each chair, a story about the limits of one's reach.

**Design imperative:** the commit-time risk readout must be excellent. Danger priced
*before* you let go is the difference between every loss reading as "my gamble" and "the
game robbed me". This is what §8.6's calculator exists for.

### 11.5 The ground war

**Why ground troops exist at all when the orbit has heavy guns.** The honest objection is
that a fleet with capital weapons parked over a colony should be able to simply take it —
and if capture were only about *destruction*, that would be right. But the thing being taken
is a working colony: its people, its industry, its stockpile. Guns can suppress that. They
cannot occupy it. Wrecking a colony from orbit is easy and worthless; what a conqueror wants
is the colony intact, and putting soldiers on it is the only act that means "this is mine
now" rather than "this is rubble now".

Mechanically the split exists to make conquest **two decisions instead of one**. Before it,
a colony ship was a magic key: win the orbit, land the key, done — one uncontested step, and
the defender's last chance to resist ended the moment the siege clock started. Now the
defender gets a second front. A besieger who has served the clock still has to answer "how
many boots do I have, and how many does this ground demand", and both of those numbers are
things the defender can change.

Three pieces:

**Settling and conquering are different acts, with different hulls.** A **Colony Ship**
settles *unclaimed* ground and is consumed doing it. It can no longer take anyone's system —
pointing one at a besieged colony now does nothing at all. Taking held ground requires
**Troop Transports**: 40 marines each, unarmed, slow, and visible. The two acts had been
conflated into one hull, which made peaceful expansion and armed conquest cost the same
thing and read the same on the map.

**A Garrison is what the ground demands.** Each standing, fed tier fields **25 troops**, so
a Garrison III musters 75 — and stretches the besieger's siege clock besides. The building
is a real defensive investment with a real running cost (§7.1), and — like everything in
§5.1 — an unfed garrison suspends rather than dies: it counts for nothing while starved, and
stands back up intact the moment Provisions arrive.

**A landing is fought, not resolved.** The drop is irreversible — the men are committed —
and from there the fight runs on its own clock in seeded rounds, on the same terms as a
fleet action: bounded per-round variance, an isolated RNG stream derived from the assault
id, and no tactical input of any kind. What the attacker chooses is how many marines to
land and whether to keep firing. That is the entire input surface, and it stays that way.

Because the fight is resolved over time rather than compared once, the numbers behave the
way Lanchester's square law says they must: two forces annihilate at `M² = D²·(1−s)`, so the
**break-even landing is `D·√(1−s)`**. Half-suppressing a garrison discounts the landing by
29%, not 50%. That is the intended shape — **bombardment discounts a landing, it never
replaces it** — and it means the figure both sides read is a *threshold of odds*, not of
outcome. Land exactly that many and it is a coin flip; margin above it buys confidence.
Below it, commanders refuse to come down at all, which costs nothing: refusing to land and
landing-and-dying are different outcomes, and only the second loses you the transports.

**Bombardment is a window rather than a wound, and the code now means it.** Suppression is
re-read *every tick of the fight*, not sampled at the drop. A fleet holding station keeps
the garrison pinned in cover; the moment the guns stop, suppression bleeds off and those
troops walk back into the firing line — mid-landing, under the attacker. A besieger must
therefore hold the orbit for the *whole* invasion, and the defender's counter-play does not
end when the transports arrive: **relief that breaks a blockade can turn a fight already
being lost.** This is the second front the earlier design lacked entirely, when the outcome
was sealed the instant boots touched the ground.

The defender's counters are therefore a real menu: feed the garrison, deepen it, break the
blockade to end the suppression, or relieve the system while the fight is still running.
Each is a different kind of investment, which is the point.

**The odds are on the table before the men are.** Since a landing is rolled, committing one
blind would make every loss read as the game cheating. So a besieger with marines in orbit
is quoted a **pre-commit estimate** sampled from the real ground engine (§8.6's sibling —
the same discipline: run the actual thing, never an approximation of it): the chance of
taking the ground, the expected cost in men and seconds, and — the decisive figure — *the
same landing's odds if the bombardment stops.* A wide gap between the two is the warning
that the landing belongs to the blockade rather than to the troops, and will last exactly as
long as the orbit is held. The defender is deliberately not shown the attacker's odds; that
would hand them the attacker's order of battle.

**And a landing leaves a replayable account of itself** (§8.6's ground counterpart). Both
participants can watch it round by round — each side's strength, the fraction of the
garrison pinned at that moment, and the derived beats: where the guns lifted, where the
lead changed hands. A third party who merely holds sensor coverage of the site sees the
*shape* of the fight without its arithmetic — troop strengths are exactly the intel an
onlooker should have to earn. Every round arrives on its own light, so a landing being
fought right now is watched as a chase of one's own light cone, never faster.

---

## 12. Research

A syndicate-wide tech layer on **programme boards with schools**. Six fields, each running
Tier I (open) → Tier II (field verb gate) → two schools with their own gates and Tiers
III–V. The Hulls field's Line school extends to Tiers VI–VIII for the capital ladder.

| Field | Schools |
|---|---|
| Propulsion & Logistics | Line Haul · Expedition |
| Materials & Fabrication | Deep Crust · Foundry |
| Computation & Sensors | Watch · Shadow |
| Weapons & Ordnance | Strike · Countermeasures |
| Hulls | Line · Corsair |
| Life & Habitation | Growth · Talent |

**Nothing is exclusive.** Every programme is researchable by every syndicate. Identity comes
from the *order you chose* on a one-at-a-time continuous clock, not from locked branches.
111 authored programmes, so a focused season yields roughly a third of the tree.

Ten of those programmes are **live placeholders**: on the tree, researchable, and described,
but inert until the content pass that gives them something to unlock — the four utility
modules, the Lance Array, a convoy refit variant, and a few view and extraction features that
cannot be expressed with today's enums. Two further entries (Salvage Rigs, Boarding Parties)
are hidden and not researchable at all. Everything else has real effects wired at real sites.

**Gates are verbs, not currency.** A tier opens on what your corporation has actually done —
light-years flown, convoy deliveries, raw units extracted, systems scouted, rival fleets
observed, battles won, hull mass destroyed or absorbed, warships commissioned, population
grown, specialists trained. Three gate kinds: cumulative counters, instantaneous state
metrics, and sustained metrics that must hold for a duration.

**The ladder rule:** Tier N+1 opens when *any one* Tier-N programme on that ladder completes.
Rushing deep means skipping siblings — you can return for them later, at the cost of
everything else you are not researching.

Programmes pay in one of five effect kinds: numeric mods keyed to a single application site,
module unlocks, hull unlocks, structure-tier unlocks (the superlinear tiers 5–6), and
capability flags — 36 of them, each with exactly one enforcement point.

Board discipline, enforced as rules:

1. Within a tier, programmes are sidegrades: no ordering is strictly dominant.
2. Percentages live in Tiers I–III; Tiers IV–V pay primarily in capabilities.
3. New hulls live only in the Hulls field.
4. Nothing violates lightspeed; entangled keys carry no information.
5. The counter matrix stays one-to-one — no research may let a weapon ignore its counter or
   double-counter a weapon. This is asserted by test with all Weapons research complete.
6. Pacing is the balance surface. With no exclusivity, concurrency (one programme at a time,
   corp-wide) and the duration curve are the only anti-convergence knobs.

**Research is goods-funded and geographically distributed.** A programme's cost is measured in
throughput-seconds — 2 h at Tier I, then 8, 24, 72, 168, and 240 / 336 / 480 for the capital
tiers, so a Titan is the deepest research in the game. The rate comes from every Academy in the
syndicate, each running the same factor chain as any other production line (tier × staffing ×
skill × food), with a field-matched specialist driving the skill term. Each contributing Academy
**drips a funding basket from its own system's stockpile** — Electronics on every programme,
Machinery from Tier II, Rare Elements from Tier III, plus field flavour (Weapons wants
Armaments, Life wants Biomass and Provisions, Hulls wants Machinery).

Two Academies therefore research about twice as fast as one, and blockading one system's inputs
drops the rate by exactly that lab's contribution. A lab whose stockpile cannot cover its drip
suspends its contribution softly and resumes when supplied — the same suspend-never-destroy rule
as everything else.

Research is owner-only on the wire; nothing leaks to rivals. The static catalog ships once at
join rather than riding every view update.

---

## 13. Fleets

### 13.1 The fleet is the unit

The map and sim unit is a **fleet**: one or more ships of mixed kinds moving, fighting, and
being observed as a single entity. A fleet-of-one is the N=1 case and behaves exactly as a
single ship. Composition is a deterministic kind→count map, never empty for a live fleet.

**The formation rule: the slowest member sets the pace.** Fleet speed is the minimum over
present kinds; total mass is `Σ hull_mass × count + cargo`, so fuel scales with the whole
formation. A raider hammer escorting a colony ship lumbers at the colony's pace,
telegraphing itself by physics rather than by rule.

Broadcast, detection signature, and the intel ladder all read the composition (§6.3, §6.4).

### 13.2 Management

Fleets compose at an owned system, never in flight:

- **Build** forms a new fleet, or joins one already docked at that shipyard.
- **Merge** folds a co-located idle fleet into another.
- **Split** detaches named counts into a new idle fleet beside the source.
- **Colony claim** consumes one colony ship from the arriving fleet; the rest of the fleet —
  escorts, extra colonists — persists and parks at the new holding.

Every management command soft-rejects an invalid request: not yours, not idle, not at an
owned system, empty or over-drawing split. There is no in-flight detachment.

The client draws each fleet as one sprite (the flagship by precedence) with a count badge:
exact for your own fleets and for rivals inside coverage, the bucket label for rivals
outside it, drawn dimmer as an honest estimate. A fleet-of-one shows no badge.

---

## 14. Technical Architecture

A self-hosted Rust authoritative server with four pieces:

```
            ┌───────────────────────────────────────────────────────┐
            │  server (Tokio)                                       │
  client ───┤  ┌────────────┐   intents    ┌──────────────────────┐ │
  (Pixi) ◄──┤  │ ws conn    │ ───────────► │ game loop — single   │ │
    WS      │  │ (axum)     │ ◄─────────── │ owner of World +     │ │
            │  └────────────┘  per-player  │ Sessions; 30 Hz tick │ │
            │        ▲          streams    └──────────┬───────────┘ │
            │        │                                │ events,     │
            │        │                        ┌───────▼──────────┐  │
            │        │                        │ persistence task │  │
            │        │                        │ (sqlx → Postgres │  │
            │        │                        │  or no-op stub)  │  │
            │        │                        └──────────────────┘  │
            └────────┼──────────────────────────────────────────────┘
                     │ uses (pure, no I/O)
             ┌───────▼───────┐
             │   sim crate   │  World + step(commands) → events
             └───────────────┘
```

- **A pure deterministic simulation core.** No I/O, no async, no networking, no database. It
  takes a `&mut World` plus a slice of commands and produces the next state plus the events
  that occurred. Determinism comes from a seeded RNG and a fixed timestep. This is both the
  determinism guarantee and the oracle a headless balance harness would run against.
- **A single-owner game-loop task.** One Tokio task owns the `World` and the session
  registry, so there are no locks and no data races on game state — enforced by
  construction, not discipline. It ticks at 30 Hz, folds intents into commands at tick
  boundaries, pushes per-player messages at ~10 Hz, and hands events and snapshots to
  persistence.
- **axum + WebSockets as pure I/O.** Connections receive intents and push filtered state.
  They never touch game state.
- **Async Postgres (sqlx) off the hot path.** Append-only event log plus periodic full-state
  snapshots. Restart loads the latest snapshot. Never blocks the tick loop. Without
  `DATABASE_URL` the persistence layer is a no-op stub and the server still runs.

**The per-player lightspeed view filter is a first-class component, not a detail.** Between
ground-truth events and each player's socket sits a per-player delivery scheduler that holds
each event until its light-travel time to that player's command center has elapsed, and
filters by what that player's assets can observe. This is the code embodiment of the entire
information model and the novel, risky core. It must be deterministically testable: *player
X could not have known Y at time T.*

**The client** renders the per-player filtered stream with Pixi.js — a WebGL-accelerated 2D
scene, chosen because the map is continuous space with many simultaneously moving elements.
It holds no authoritative state and performs no game logic. The visual grammar of the
information model lives here: staleness as fade, contacts as last-known markers with
uncertainty cones that grow between observations and snap tight on reacquisition.

The wire protocol is versioned (currently 7) and announced at join so a stale client can
detect a newer server. Static tables — the charter band ladder, the research catalog — ship
once in the welcome message rather than riding every update; slow-moving per-player sections
are signature-gated and re-sent only on change; battle records stream incrementally.

**Art** is a cohesive custom set under `client/public/art/` — celestial sprites, ship
sprites, a full UI icon set, and lore illustrations, in a dark-graphite / cyan-teal /
red-threat / gold palette. It replaced an earlier borrowed placeholder set. Sprites are a
visual layer over existing data only: tint comes from the existing ownership flag, and
nothing about the art layer can leak information the view filter withheld.

---

## 15. Standing Logistics Orders

The first layer of automation: constrained, non-scripting rules that execute on the server
clock, online or off, so a player manages policy rather than micro.

Each rule ships one commodity from a source system to a destination — another system, the
hub, or home — when a trigger holds:

| Trigger | Fires |
|---|---|
| Above threshold | ship the whole stockpile once the source reaches a level |
| Percent of surplus | ship P% of everything above a floor, reserving working stock |
| Maintain at destination | ship the destination's deficit against a target, counting goods already in flight so it cannot over-ship |

Firing spawns an ordinary raidable sub-light convoy through the existing machinery. The
lightspeed law is untouched: only the *decision* to dispatch is automated, and rivals still
learn of the convoy by delayed light. Setting a rule is instant local administration that
reveals nothing.

Two anti-spam gates bound a rule to at most one in-flight convoy, re-evaluated at most every
5 s, so a permanently-satisfied trigger can never flood the map. A rule delivering to the
hub chooses whether to sell on arrival or bank the goods in the warehouse to trade later.
Standing-order dispatch exempts Fuel hauls from the fuel charge, and silently refunds and
retries on a shortfall.

---

## 16. Fleet Doctrine

The second layer: a corporation-wide combat and logistics policy the server runs every tick,
online or off. A closed menu of enums — no scripting — so it is trivially deterministic.
**Every default is the pre-doctrine behaviour**, so a corp that never touches it plays
exactly as before.

| Axis | Options |
|---|---|
| Engagement | Avoid · **Defensive-only** · Engage-weaker-when-favourable · Engage-any |
| Retreat threshold | withdraw below a 25% / 50% / 75% local force ratio, or **never** |
| Escort | **guard nearest** convoy · guard richest convoy · hold station on a fixed picket route |
| Lost supply | **drop** the cargo · return home · re-route to the hub to sell |

The force ratio is friendly ÷ (friendly + hostile) weighted combatant strength within the
picket's own sensor bubble, counting itself. It is checked before committing *and again while
engaged*, so enemy reinforcements can tip a fight and trigger a mid-battle retreat.

Per-fleet **posture** (Passive / Defensive / Weapons Free) composes with this rather than
replacing it: the posture picks *who* a fleet pursues on its own local detection, while the
doctrine's force-ratio and retreat gates decide *whether*. A favourable-only doctrine
therefore shadows an unfavourable contact instead of suiciding into it, and Avoid vetoes all
autonomous offense regardless of posture.

Pickets sense only what is in range, and the ships they command stay sub-light, raidable, and
light-revealed. Doctrine is one of the three coarse verbs usable mid-battle.

---

## 17. Syndicates

The social layer: corporations band into mutual non-engagement pacts that also share intel
and defend each other.

A syndicate is an **affiliation, never an owner**. Fleets and systems still belong to
individual corporations, and battles, doctrine, and intel stay per-corp. Membership changes
only friend-versus-foe and what a viewer's picture reveals.

- **Size cap:** at most a third of active corporations, floored at 2 so a small galaxy can
  still form a pact. One coalition cannot absorb the galaxy.
- **Founder-managed:** the founder invites and dissolves; if they leave, the seat passes to
  the next member and an emptied syndicate dissolves.
- **Membership propagates like ownership.** A two-state history means a distant player learns
  of a join or leave only after the light from that corporation's command center arrives.
  (The pact itself takes effect immediately — an alliance is a mutual agreement both parties
  consented to.)
- **Research is syndicate-wide** (§12), as are the 24 saved doctrine fits and the single
  Titan.
- **Ally garrisons** stationed at a host system eat 0.05 Provisions per ship per second from
  the *host's* stockpile. Hosting a coalition shield means feeding it: a cut supply line
  unfeeds the garrison and suspends its defense contribution until fed. Nothing is destroyed.

---

## 18. Neutral Factions

Two neutral factions own real hulls through sentinel player ids, so they reuse the entire
fleet, combat, and raid pipeline by owner comparison rather than needing bespoke AI. Name
hashing is guarded so no real corporation can ever collide with either sentinel.

**Pirate enclaves** fill the empty dark with ambient danger, safe combat practice, and
objectives that do not require farming another human. Three hidden bases are seeded at
unclaimed systems in the mid ring (30–72% of the radius), never within 2600 su of a home
slot. A base stays dark until a scout snapshots it. It launches a dark raider pack every 90 s
that hunts broadcasting convoys within its radius (2600 su, +900 per escalation tier), grows
a tier every 300 s while unsuppressed to a maximum of 3, and is suppressed by assaulting the
base itself — a platform-equivalent defense pool of 2 tiers per enclave tier. A destroyed
base lies dormant 600 s and respawns weaker.

The ramp is deliberately Civ-barbarian shaped: a fresh enclave opens with a *lone* bandit and
only becomes a real pack if ignored. Nothing hunts at all for the first 300 s, and each
corporation gets a 240 s grace window measured from **its own join**, so a latecomer dropping
into an escalated galaxy gets the same undefended onboarding a founder got. Pirates steal,
never siege and never capture, so standing defense handles them fully offline, and their loss
rates are bounded by the same raid caps as players.

**The Terran Charter Authority** is documented in §9.4 and §9.5. It owns freighter hulls and
enforcement squadrons, holds no territory, never appears in rankings, and takes no side.
Pirates ignore Authority hulls: the enclaves prey on syndicate shipping, not on the flag that
hunts them.

---

## 19. Rankings

There is no victory condition, so standing is expressed as a published leaderboard —
identical for every player by design, snapshotted on the valuation close. Nine categories, so
that different ways of playing are legible as success:

Valuation · Trade Throughput · Market Profit · Cargo Captured · Cargo Protected · Battle
Efficiency · Systems Developed · Intel Gathered · Recovery

Counters are cumulative campaign statistics incremented at the events that already fire, and
persist with the corporation. Authority freight deliberately earns *no* trade throughput —
that counter is for your own convoys taking the risk — and an instant warehouse sale earns
none either, since it moves nothing and would otherwise be farmable risk-free.

---

## 20. Designed But Not Built

These are design commitments the code does not implement. Nothing depends on them.

**Warp lanes (player-built infrastructure).** The design called for buildable speed-up
corridors that work by reducing a ship's effective mass, so one cause generates both a speed
bonus and a fuel saving, with the benefit scaling with mass — transformative for convoys,
marginal for raiders, making convoys lane-dependent and raiders open-space roamers by physics
rather than by rule. Lanes would also restore chokepoint control: a built lane is a known,
fixed, preferred corridor, which is exactly where a raider lies in wait, giving raiding both
camping and pursuit flavours. Lanes were to be public-access (building one partly benefits
your rivals and partly endangers you — free-rider dynamics as a feature, not a bug), with
every player starting with one home→hub lane.

**None of this exists.** There is no lane type, no construction, and no mass reduction, and
the fuel-and-speed model it rested on has changed: acceleration was removed (§7), so
"mass-reduction raises acceleration" no longer has a mechanism. Any lane implementation would
need a new speed rule.

**The movable command center.** The design has you relocating your command center onto a
capital ship or a forward system in the mid-to-late game, so you see a contested front fresher
and command it faster — a pure upgrade gambled against decapitation, with a killed command
center falling back home rather than losing the game. The field exists on the corporation and
is deliberately kept separate from `home` for exactly this, but there is no command to move it
and it never leaves the home system. The fog rules would not change at all: the origin simply
becomes a variable instead of a constant.

**Conquest and a victory condition.** The design had victory by taking rival homes, with home
assaults hard, slow, and telegraphed by lightspeed. The code implements a *different*
resolution: homes can be blockaded but never captured, there is no elimination, and there is
no victory condition (§11.3). Rankings stand in for it (§19). Making home conquest real would
need a deliberate decision about elimination, plus the offense/defense balance that was always
the open question.

**Coherence as a mechanic.** The fiction describes lattice coherence peaking at the hub and at
each home and decaying with distance, governing information freshness and settlement fidelity.
The code has none of this: light delay is pure `distance / c` from the command center, and
settlement is exactly lossless everywhere. In particular, a player at home reads hub prices
delayed by their home→hub light time — there is no coherence-peak exemption from lag. Coherence
survives as flavour for why homes are bright spots.

**Smaller gaps.** No fill-price range or "abort if fill exceeds X" guard on market orders (the
staleness is disclosed, the guard is not built). No settlement-key resource economy (§3). No
warehouse capacity or storage fee. `Endpoint::Hub` cannot be a standing-order *source*.
Standing-order convoys are still free auto-spawned hulls rather than booked Authority freight.
Salvage and boarding are defined as capability flags with hidden catalog entries and no
enforcement.

---

## 21. Open Design Questions

**Balance, everywhere.** Every number in this document is a first-pass playtest value. The
constants are grouped into `Tunable` blocks per subsystem specifically so one edit re-paces a
whole layer once there is real multi-session data. Pacing is the primary balance surface for
research; siege duration and the stakes layer are explicitly awaiting pacing data.

**Two known tuning suspicions.** Six corvettes for an enforcement expedition was sized against
the pre-capital combat model and is a much softer obstacle against a real warship ladder. And
battle duration is *emergent* from the step cadence and the to-hit/damage calibration — it must
not be forced back to a target by retuning the calibration constant, because that constant is
what preserves the engine's other invariants.

**Market microstructure.** How an instant market order walks the posted price along the
elasticity curve is implemented; what the periodic limit clearing should do to a price that has
*already* moved from instant flow all interval is not settled (re-anchor to a fresh equilibrium,
or process resting limits at the current walked price?). Related: what strategic role limit
orders ultimately play — bets on future movement, or slippage-splitting for large orders.

**Home assault balance,** if conquest is ever built (§20). Too easy and the game becomes a rush
where the economic and expansion layers never matter; too hard and the conflict layer is
toothless. The fit consistent with the rest of the design is a major campaign, telegraphed by
lightspeed, as the climax of a game rather than a cheap early shock.

**Map and detection tuning.** Whether home slots stay fixed at generation or are chosen in an
opening phase. Generation parameter values. Contact-decay handoff (how a lost contact ages,
extrapolates, and reacquires). The cloak and sensor-tech multipliers are live no-op hooks
waiting for the research entries that will drive them.

**Fairness validation.** Pillar 1 is now maintained by design vigilance rather than guaranteed
by a tick boundary, which is the price of the more elegant continuous model. It needs a human
playtest with deliberately uneven check-in patterns — one tester constant, one once-daily. They
should end at comparable *strategic* progress. If the frequent checker leads on progress rather
than just on tactical wins, a login-gated advantage has leaked in.

---

## 22. What's Inherited from Stellar Charters

This is a new game standing on proven foundations:

- **The corporate economic spine** — chartered corporations, a central exchange, commodity
  trade, corporate valuations, the "global price, local logistics" tension.
- **The design philosophy** — shown-math resolution, legibility as a cardinal rule, risk
  communicated as history and factors, no hidden probabilities.
- **A pure, I/O-free simulation core**, so a headless bot-balance harness stays possible.
- **Procedural generation** — every galaxy fresh from a seed.
- **Standing orders**, here promoted from a convenience to the primary interface and given an
  in-fiction justification: command across light-minutes.

The clean break: discrete daily turns became continuous async; lane-graph movement became
continuous space; abstract strategy became lightspeed-bound observation and command; and a new
core fiction generates the rules instead of decorating them.
