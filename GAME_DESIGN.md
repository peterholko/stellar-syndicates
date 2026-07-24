# Stellar Syndicates — Game Design Document

> A new game, designed from scratch. It inherits the economic spine and design
> philosophy of *Stellar Charters* but is a clean break in time model, space model, and
> core fiction. Working title is provisional.

---

## 1. High Concept

An **asynchronous, multiplayer (4–12 players), continuous-time 4X space strategy game** about
corporate trade and conflict across a wormhole-linked galaxy. You are a chartered corporation
commanding from a **home system** (and, later in the game, from a relocatable command center you can
push toward the front), expanding into the dark, shipping goods to a central market, and raiding (or
defending) the convoys that carry them.

The defining mechanic — the thing that makes this game unlike others — is **lightspeed-delayed
observation and command**. You never see the galaxy as it is *now*; you see it by the light that
has reached your chair. You cannot command a distant ship in real time; your orders travel out at
the speed of light and arrive late. You are not a god moving pieces on a board. You are a remote
commander reading reports from the dark and sending instructions into it, hoping they arrive in
time and remain relevant.

The one-line soul of the game: **correlation is instant; knowledge is slow.** You can *act* across
any distance immediately — but you must always *wait for the light* to learn what your action meant.

---

## 2. Design Pillars

These are the load-bearing commitments. Every mechanic must serve at least one; no mechanic may
violate one.

1. **Async-first. Presence gives awareness, not advantage.** A player who logs in twice a day must
   be on near-equal footing with one watching constantly. You act *once* (set intent, set doctrine)
   and the world carries it out without you. You never have to babysit execution. This is protected
   *by physics* — lightspeed lag makes real-time intervention impossible, so being logged in confers
   no twitch advantage, only earlier awareness.

2. **Legibility. You always know exactly how blind you are.** Inherited from *Stellar Charters*'
   shown-math rule. Outcomes resolve from named, visible factors — never hidden dice. Battle
   resolution does use **bounded, seeded, battle-isolated randomness** (to-hit rolls, ±15% damage
   variance, torpedo interception — the spice that makes small skirmishes tense while big fleet
   actions converge on the math), but it is *published* randomness: the distributions are game
   rules, the pre-commit calculator samples the very same engine to show you win odds and loss
   bands, and the same seed replays the same battle for every viewer. Doctrine-over-input is
   untouched — no mid-battle commands exist, so a roll can season an outcome but never substitute
   for your commit-time decisions. The lightspeed model is honest: the UI always tells you how
   stale your information is and how long your commands will take. You have *certainty about the
   extent of your ignorance* and *uncertainty about its contents* — never the reverse. A loss must
   always trace to a decision you made, never to the game concealing something it should have
   shown.

3. **Distance is the antagonist.** A single spatial variable — distance from the hub (and from your
   home) — drives nearly everything: travel time, fuel cost, information lag, settlement reliability,
   raiding viability, resource value, and fog. The further out you operate, the richer the rewards
   and the deeper the dark. There are **no zones and no boundaries** — every property varies smoothly
   and continuously with distance, read directly from how the map renders.

4. **Decisions are front-loaded. Doctrine over micro — across the void.** Because you can't control
   distant assets in real time, the meaningful decisions for everything LIGHT-DELAYED and
   interstellar — fleets, logistics, combat — happen *before* contact: where to commit, what route,
   what pre-authorized behavior ("engage only if I outgun them," "burn evasive if pursued"). Your
   ships act on standing orders when you're out of reach. The skill is in the preparation, not the
   reflexes. **The deliberate carve-out: colony development is hands-on and per-planet.** Inside
   your own gravity well there is no light lag and no doctrine — planets and moons are real, owned
   places: you site each structure on a body, staff its lines, and grow its population, planet by
   planet. Doctrine governs the void; you govern your worlds.

5. **Coherent physics: one speculative leap, rigorously followed.** The universe rests on exactly
   one impossible thing (the wormhole lattice). Everything else is real physics — special relativity
   and quantum information theory — applied straight. Mechanics are therefore *consequences players
   can reason about*, not arbitrary rules they must memorize.

---

## 3. The Fiction: The Lattice and the Ledger

The fiction is not decoration. It *generates* the mechanics — every rule about what is instant and
what is delayed falls out of the physics below.

### The one impossible thing

When the wormhole opened into the virgin galaxy, its formation **crystallized a lattice of paired
singularities** throughout the region — like frost spreading from a single point. Each is one end
of an entangled pair; its twin is anchored at the **Wormhole Hub** at the galaxy's center. This
lattice is fixed geography: it does not move, it cannot be manufactured, and it predates every
corporation. Whoever administers it controls the only instant connection across space. That is why
charters were sold.

### The Quantum Ledger (the market substrate)

The central Exchange runs on the **Quantum Ledger**, built on the hub-anchored entangled lattice.
On chartering, a corporation is issued **settlement keys** — its half of entangled pairs whose twins
live in the Exchange's vaults. Committing a trade is a *local measurement* of a key, which
**instantaneously collapses the correlated twin at the hub** — settling the transaction across any
distance. This is real physics: the collapse is *correlation*, not *information*, so it is instant
and breaks no law (the no-communication theorem permits exactly this).

Two real quantum principles give the keys their game properties:
- **No-cloning** → keys cannot be copied. Trades cannot be forged, authority cannot be duplicated,
  settlements cannot be replayed. Market access is inherently un-forgeable.
- **Measurement destroys state** → a key is *consumed* when used. Each key is one transaction, then
  spent; you draw fresh keys from your charter allotment. (This opens a future resource economy —
  see §11.)

But you **cannot learn the result instantly**. The collapse settles the trade, but *what price you
got* is information, and **information travels only at light**. The Exchange broadcasts prices
outward as ordinary signal; you read an old copy. This is the no-communication theorem verbatim:
the correlation is instant, the readout is lightspeed-bound.

### Coherence

The lattice is strongest — most **coherent** — at the hub and at each player's **home anchor**, and
its coherence **decays continuously with distance** from those peaks. Coherence governs *information
freshness* and *settlement fidelity* (far-out settlement is lossy and key-expensive), but **not
execution speed** (collapse is distance-independent). The galaxy is therefore an archipelago of
clarity — bright at the hub and at each home — dimming into dark between and at the rim.

### The master principle

From all of the above, one rule adjudicates every "is this instant or delayed?" question in the game:

> **Anything pre-arranged at a fixed, known point is instant. Anything novel, or directed at
> something mobile, travels at the speed of light.**

Standing trade authority at the hub: instant. Ship doctrine set before launch: instant. Redirecting
a ship mid-flight: lightspeed (novel command, mobile target). Seeing a distant price or enemy:
lightspeed (incoming broadcast). A home defense firing on whatever enters its range: instant (fixed
point, local). This principle is *generative* — players internalize it once and can then predict the
behavior of any new situation, so nothing ever feels arbitrary.

---

## 4. The Galaxy Map

**One continuous 2D radial space**, procedurally generated per game from a seed, with the Wormhole
Hub fixed at the center. No node graph — ships have real positions and move freely in any direction.

- **Star systems** are randomly placed across the space (no zones, no structured regions). Each
  carries resources, population potential, and claim properties (inherited from *Stellar Charters*).
- **The hub** sits at the center — the shared market commons through which all corporations trade,
  and the anchor of the entire lattice. It is special not as a uniquely safe place but as the
  uniquely *shared* one.
- **Home anchors** — one per player — are distributed around the galaxy as bright spots of high
  coherence. Each is a player's private window of clarity: live prices, instant lossless settlement,
  frictionless routine trade. Players command permanently *from* their home anchor.
- **No discrete zones.** "Core" and "frontier" are loose relative words players use, not game regions
  with rules. Danger, resource value, information lag, and fog all increase *smoothly* with distance
  from the hub. The richest resources tend to lie far out; so does the deepest dark. The gradient is
  read directly from how the map renders — bright and crisp near peaks, dim and fogged toward the rim.
- **Every player sees a different map.** What you see is *your home anchor's lightspeed-delayed,
  fog-filtered reconstruction* of the galaxy — not objective truth (see §6). Crisp and current near
  home and the hub; progressively staler and fuzzier outward; islands of clarity wherever you have
  assets; fog and darkness at the edges.

**Generation parameters** (the tuning knobs): galaxy radius (**should scale with player count** so
the dark space between homes stays proportional across 4–12 players), home-anchor spacing (the
pacing lever — tight = early conflict, wide = long buildup), system density, resource-concentration
gradient (how strongly value rises with distance), and **coherence falloff rate** (the master dial on
how much the information model dominates play — steep = small islands of light in a vast dark; gentle
= most of the map workable).

---

## 5. Time & Turn Structure

**Continuous asynchronous play.** The world runs on its own clock and evolves continuously; there is
no shared turn boundary. Players act whenever they are online and never wait on each other.

**Multi-rate clocks**, not one global tick — each subsystem runs at the cadence that suits it:
- **Market:** instant execution; limit orders clear on a periodic batch (see §9).
- **Movement:** continuous (fine background advance).
- **Valuations:** slow (e.g. 6-hour or daily close) to avoid share-price noise.
- **Strategic arc** (research, population, construction): **continuous real-wall-clock progress** — no
  daily boundary; research, growth, and construction advance on the same continuous clock as everything
  else. This is the elegant, consistent choice (one tempo for the whole game), but its fairness rests
  entirely on three **required** mitigations that prevent login-frequency from compounding into a
  structural lead (see §5.1).

### 5.1 Continuous progress and the fairness mitigations (required)

Continuous strategic progress would, naively, reward a player who logs in more often (they collect
completions and re-queue sooner, compounding over a long game) — which would violate Pillar 1. Three
mechanisms make progress **presence-independent**, and all three are mandatory, not optional:

- **Queue-ahead (primary).** Players set an indefinitely deep *queue* of research/construction; when
  one item completes, the next begins **automatically, no login required**. An offline player's queue
  advances through completions exactly as fast as an online player's. This is the standing-orders
  philosophy applied to the strategic layer. Ideally queue entries can be *conditional* ("research X,
  then if at war research Y else Z") — the strategic echo of combat doctrine.
- **Offline accrual.** Rate-based progress (population growth, resource accumulation) accrues
  continuously on the server whether or not the player is logged in. Nothing to "collect"; your
  population is simply whatever it has grown to when you next look.
- **No login-gated bonuses, anywhere.** Nothing on the strategic layer may reward acting *at a specific
  moment* — no completions that decay if uncollected, no closing windows, no re-queue-within-X bonuses.
  Completions wait patiently; queues auto-advance; accrual never caps or decays from neglect.

What checking in *more* legitimately buys is **tactical responsiveness and decision quality** (react to
the market sooner, redirect raiders on fresher intel, re-plan the queue as the situation develops) —
i.e. *awareness*, which Pillar 1 permits — never raw *progress*, which it forbids. The fairness is now
maintained by **design vigilance** (every future strategic mechanic must pass "does this reward being
online at a moment?") rather than guaranteed by a tick boundary — that discipline is the price of the
more elegant model. **Validate specifically in human playtest** with deliberately uneven check-in
patterns (one tester constant, one once-daily): they should end at comparable *strategic* progress; if
the frequent checker leads on progress (not just tactical wins), a login-gated advantage leaked in.

**Standing orders / doctrine are the primary interface.** At a continuous, fast cadence — and for
distant assets that can't be reached in real time — manual per-step control is infeasible *and*
physically impossible. So the common loops (buy-and-deliver, sell-output-at-hub, keep-stocked,
patrol, engage-under-these-conditions) are pre-authorized intents the world services on its own.
This is also the in-fiction command doctrine of a corporation spread across light-minutes.

---

## 6. The Information Model (the centerpiece)

This is what makes the game itself. **There is no objective view of the galaxy.** Everything you know
arrives as light, from the vantage of your **command center** — and where that command center sits is
a strategic decision (see §6.1). Early game it is your home anchor; later you can relocate it forward.
Its location is the origin of your light-cone: it dictates your fog of war and the delay on both the
information you receive and the commands you send.

### 6.1 The movable command center

You begin commanding from your **home anchor** (the safe, economically-perfect default). In the mid-to-
late game you can **relocate your command center** — onto a **capital ship** (riding with your fleet to
the front) or to a forward star system. The command center's location is the single origin point from
which *all* of your fog-of-war and lightspeed delay are computed.

- **Why move it:** a forward command center sits *closer to the contested front*, so you see the action
  there **fresher** and your commands reach your forward ships **faster** (shorter light-distance). The
  fixed-home commander sees the frontier late and commands it slowly; the forward commander does not.
- **It is a pure upgrade — gambled against decapitation.** Relocating forward grants the awareness/
  command-latency advantage with **no economic-clarity penalty** (settlement and market access are
  unaffected). The *only* counterweight is risk: a ship-borne command center can be **killed**.
  - *Watch-flag:* because forward command is otherwise a pure upgrade, the **decapitation risk must
    stay meaningful** or "command from home" becomes a dead option nobody picks. If playtest shows
    forward command is always correct, add back a small economic-clarity cost to make "stay home" a
    real choice.
- **Decapitation → fall back to home.** If the capital ship carrying your command center is destroyed,
  your command center **falls back to your home anchor** — you lose the forward position and its
  advantages (at the worst possible moment, mid-battle, snapping you back to slow home-command), but
  **not the game**. Forward command is therefore a *gamble*, not a death sentence. (But note: home is
  then your last fallback — and home itself can be conquered; see §11.)
- **Mechanically minimal, strategically large.** The fog/delay rules do not change at all — they are
  still "information and commands propagate at *c* from the command center." You have simply made the
  *origin* a variable instead of a constant. The whole game opens up over its arc: from fixed-home
  remote command early, to projecting your center of awareness into contested space late.

### The three clocks

Every distant interaction is split across three separated-by-distance moments:
1. **When you send** a command,
2. **When the ship receives it and acts** (your command propagated outward at *c*),
3. **When you observe the result** (the light of the outcome propagated back at *c*).

For a frontier action, the round trip can exceed the duration of the event itself — your command can
arrive *after* the thing it was meant to affect has already resolved.

### Asset-based fog of war

Your ships and colonies **stream their sensor data to your command center continuously**, each feed
delayed by that asset's light-distance from the command center. This produces two distinct, correct
fog regimes:

- **Your own forces** appear as a **delayed-but-coherent live picture** — like a continuous broadcast
  on a fixed tape-delay. You always know what they saw; just late, by a known offset.
- **Enemy/neutral forces** are seen **only through your assets' feeds** — sharp while one of your
  assets holds contact, **decaying into a growing uncertainty cone the moment contact is lost**, and
  simply dark where you have no eyes. Keeping an asset *on* an enemy (so its contact doesn't decay)
  is a real reason to scout and to maintain forward presence.

### The epistemic contract

You **always know precisely how stale your information is** (the UI declares it everywhere — "this
sector: light delay ~14 min," "contact last seen 6 min ago"). You **never know exactly what changed**
in the gap. This is the line between *immersive* (an honest universe you reason about under defined
uncertainty) and *infuriating* (a game that hides things). The map renders staleness as a *visible
property* — uncertainty cones that swell between observations and snap tight on reacquisition — so
blindness is something you *see as shape*, not a number you read.

### Information as a shrinkable fog (emergent)

Because assets at different distances see the same event with different lag, **information is
positional and tradeable.** Forward scouts, listening posts, and allied intel-sharing let a coalition
assemble a fresher, more complete picture than any member has alone. You cannot reduce the *delay*
(light is light), but you can buy *clarity* by placing eyes forward. This is an emergent strategic
layer, free from the core rule.

---

## 7. Movement & Physics

**Continuous-space movement**, tuned hard for async legibility.

> **IMPLEMENTATION NOTE (post-playtest, §14.1):** the flip-and-burn *acceleration*
> model below was tried and **removed** — at the async check-in cadence the burn
> was invisible, and its `t ≈ 2√(d/a)` law defeated the mental arithmetic a
> lightspeed-prediction game needs. The build uses the original GDD §14.1 model:
> **constant-velocity, piecewise-linear movement with a per-kind constant speed**
> (Scout 115 · Raider 100 · Corvette 65 · Convoy 40 · Colony 33), so `t = d / v`
> and interception is analytic. The acceleration prose is kept below as the
> historical design rationale for the convoy-vs-raider *feel* (now expressed as a
> flat speed gap rather than an accel/mass one).

- **Acceleration to a G-limit, with automatic flip-and-burn.** *(SUPERSEDED — see
  the note above.)* Ships accelerate to a midpoint, flip, and decelerate to arrive
  at rest — the engine *always* plans the arrival burn, so **the player never
  manages momentum** (no overshoot, no Newtonian misery). Travel time is
  **non-linear**: roughly `t ≈ 2·√(distance / acceleration)`. Doubling acceleration
  divides travel time by √2, so distance has diminishing effect and acceleration
  compounding effect on who-arrives-first.
- **Emergent danger gradient for free:** because travel time scales with √distance and higher-G ships
  gain more on long hauls, **long frontier convoys are structurally more vulnerable than short core
  runs** — the frontier is dangerous by physics, not by rule.
- **Convoys vs. raiders is a two-parameter dial, not a special rule** — and it is reinforced by the
  lane fiction (see §10): warp lanes work by **reducing a ship's effective mass** while it travels
  along them. Since acceleration = thrust ÷ mass, lower mass means *higher acceleration and lower fuel
  burn from one cause*. The benefit therefore **scales with the ship's mass**:
  - *Base acceleration / G-limit:* raiders high, convoys low. A raider can run down or cut a chord to
    intercept a convoy.
  - *Mass-dependence on lanes:* **trade convoys are the largest, most massive ships in the game** (a
    locked design fact — the lane-value logic rests on it). Mass-reduction transforms them, so they
    cling to lanes; a light raider gains little and roams open space, cutting chords to intercept
    lane-bound convoys.
  The behavior (convoys hug lanes, raiders cut across open space) **emerges from the physics** — the
  heaviest ships are the most lane-dependent, automatically. (Emergent bonus: because lanes cut a
  convoy's mass, a convoy that ducks onto a lane *accelerates and decelerates harder*, changing the
  intercept geometry — lanes don't just speed convoys up, they alter the pursuit math around them.)
- **Fuel / reaction-mass is the economic governor.** Burning harder costs fuel. High-G pursuit is
  expensive, so an aggressive raider spends real reaction mass — naturally limiting raiding tempo and
  **coupling raiding pressure to the fuel market** (starve fuel, suppress raiding; fuel glut enables a
  raiding wave). Fuel is a traded commodity, so violence and commerce are economically linked.

---

## 8. Combat & Raiding

Raiding is the central conflict, and under continuous space it becomes a **pursuit / intercept
geometry** problem rather than positional chokepoint control — with player-built lanes adding a
chokepoint layer back on top (see §10), giving raiding two complementary flavors.

- **Intercept-commit.** A raider *commits* to a target; the engine solves the trajectory geometry —
  given both ships' positions, velocities, and accelerations, can the raider reach a common point
  before the convoy reaches safety? The result is deterministic and **previewable** (it can be shown
  before commit), fitting the shown-math rule. No real-time piloting. This is chosen specifically
  because it is async-native: a decision made once, resolved by the engine.
- **The convoy's tactical choice.** Ride the fast but *predictable* lane (and risk a camped intercept)
  or flee through *evasive* but slow/fuel-expensive open space. This choice is the heart of the
  cat-and-mouse.
- **Outcomes resolve on pre-set doctrine.** Because commands can't reach a distant ship in real time,
  the fight is governed by the standing orders the ship carried at commit: engage/break-off
  conditions, fuel ceilings for pursuit, target priorities. *Doctrine is the survival lever, not
  real-time reaction.*
- **The lightspeed drama is intrinsic.** You commit a raider on a *stale ghost* of the convoy's
  position; the raider acts on better (closer, fresher) information than you have; you watch the chase
  unfold late and can intervene only via lightspeed-lagged commands that usually arrive too late to
  matter. A raider can die with your unheard retreat order still in flight — and that is the design
  *working*, not failing, **provided** the loss traces to your commit-time decisions (intel + doctrine)
  and never to latency-as-randomness. The defender's experience mirrors this: they learn their convoy
  is hunted from stale information, can do almost nothing in the moment, and survive or die on the
  routing, escort, and doctrine they chose *at dispatch*. Both players are equally remote, equally
  pre-committed, equally watching — a clash of two absent commanders' earlier decisions, adjudicated
  by physics in the gap between them. This symmetry is what makes it *fair* despite being, from each
  chair, a story about the limits of one's reach.

**Design imperative:** the dispatch-time / commit-time **risk readout** must be excellent — danger
priced *before* you let go (route exposure, recent raider traffic, light-delay to the zone, "you
cannot recall past this point"). This screen is the difference between every loss reading as "my
gamble" versus "the game robbed me."

---

## 9. The Market & Economy

The economic spine is inherited from *Stellar Charters* (corporate trade, a central Exchange, equity
/ valuations) but restructured around instant settlement and lagged information.

### Execution vs. information (the core split)

- **Execution is instant everywhere.** Committing a trade collapses your settlement key's twin at the
  hub — settlement is correlation, which is distance-independent. There is **no clearing-batch wait on
  a market order**, anywhere, ever. Click buy/sell at market → it fills.
- **Price information lags.** The Exchange ticker is a lightspeed broadcast; far from a coherence peak
  you read stale prices. So a market order is a **commit to the *true* hub price on arrival**, not to
  the stale number you saw — the UI shows a *fill-price range* whose width scales with your staleness
  (the market's uncertainty cone), plus an optional guard ("abort if fill > X").
- **At home (a coherence peak), there is no lag** — live prices, instant true fills, frictionless
  routine commerce. Since players command from home, the *common case* is crisp. Lag only bites when
  trading on conditions out in the dark. This is deliberate: friction is placed where it is dramatic,
  not where it is routine.

### Order types

- **Market orders:** instant execution against the standing price.
- **Limit orders:** rest and clear in a **periodic batch** (a uniform-price call auction). The batch
  is the **anti-sniping mechanism** — within a clearing, arrival order is irrelevant and everyone
  clears at one price, so reacting fastest confers no edge. (Scoping the batch to *limit* orders only
  is what preserves the instant market-order feel.) A limit order placed against a **stale** book
  (likely, away from a coherence peak) is *accepted as-is* — you commit a price based on information
  you know is lagged, and the consequences are yours. This is the same act-on-delayed-information
  bargain as the rest of the game; there is deliberately **no stale-price protection** (the one
  requirement, per Pillar 2: the UI must always *show* that the price data is stale and how stale, so
  the loss reads as "I gambled on old prices," never as "the game showed me a wrong number").
- **Trade and haulage are separate acts (§TCA).** The Exchange settles against your **Charterhouse
  warehouse** — a private stock you hold *at the station*. A buy deposits into it; a sell (and
  sell-side limit escrow) draws **only** from it. Nothing about a trade moves goods across space, in
  either direction. Both sides are therefore symmetric and price-certain: the goods are already at the
  Exchange, so there is no crossing and no price-on-arrival gamble.
  - This replaces the old asymmetry, in which a buy conjured a free delivery convoy home and a sell
    committed goods to the crossing first and cleared at whatever the price was on arrival. That
    coupling made "trading" and "hauling" one indivisible act you could not opt out of. Splitting them
    keeps the danger (see below) while letting a player *choose* when to expose goods to the dark.
  - Convenience is preserved as a **composition, not a coupling**: a buy carries an optional
    "deliver to system X", which simply books Authority freight for the lot the instant it settles. One
    checkbox — and if the booking can't be honoured, the goods just stay in the warehouse and you are
    told why. **No second login** to bridge execution and shipment.

### Where the danger lives

The danger is in the **crossing to/from the Charterhouse** — the contested, raidable space — **not in
the knowing**. Price advantage and delivery risk are deliberately **decoupled**: a great fill on goods
that then get raided in transit is not a clean win. This decoupling is why information-relay exploits
(e.g. a smurf at the hub relaying live prices) gain little — they buy a slightly better forecast on a
trade you still can't speed up or deliver safely. The prize was never the price; it's the safe crossing.

Since trading no longer moves anything, the crossing is now an **explicit choice with exactly two
channels**, and choosing between them is the logistics game:

| | **TCA freight** | **Your own convoy** |
|---|---|---|
| Who flies it | The Authority's scheduled carrier | A hull you built and loaded |
| Cost | A fee, charged at booking and destroyed | Free (you already own the ship) |
| Timing | Fixed timetable; capped units per departure | Whenever you like |
| Risk | Someone else's hull — but your goods are still aboard, and it can be raided | Yours to escort, route, and lose |
| Reward | — | Counts as **trade throughput** on the leaderboard |

Neither is strictly better. Freight is the low-attention default that keeps a distracted empire
running; flying it yourself is cheaper, faster to schedule, and the only way to escort what matters.

### The Terran Charter Authority (§TCA)

The **Terran Charter Authority** is the home-galaxy body on the far side of the wormhole that issued
every corporation's charter. It operates the **Charterhouse** — the hub station and its Exchange — and
a scheduled common-carrier **freight service** to the colonies. It is a neutral institution, not a
player: it holds no territory, never appears in rankings, and takes no side.

- **The warehouse.** Every corporation has private storage at the Charterhouse. It is the Exchange's
  only counterparty and has no capacity limit or storage fee (v1).
- **Scheduled freight.** Book a lot outbound (warehouse → a system you own) or inbound (a system you
  own → warehouse, optionally sold the moment it lands). Goods are escrowed and the fee charged at
  booking; the fee is a pure **credit sink**, destroyed rather than paid to anyone, and never refunded.
  Departures run on a fixed timetable, one freighter per destination that has anything waiting in
  either direction. A per-corporation cap bounds each departure — an oversized lot is never refused, it
  simply rides several consecutive departures. A **Depot** at the destination doubles that cap and
  discounts the fee.
- **Freighters are real objects.** They broadcast under the Convention like any civilian hull, and they
  can be raided (their manifest is stolen) or destroyed (everything aboard is lost). A freighter's
  manifest is **two-tier per entry**: you always see your own lots, and anyone else's only from inside
  sensor range. Pirates ignore Authority hulls; the enclaves prey on syndicate shipping, not on the
  flag that hunts them.
- **The Authority holds your goods.** If a lot can't be delivered — the system changed hands, or its
  depot is full — the freighter carries it **back to your warehouse** rather than destroying it.
  Deliberately friendlier than the convoy cargo-lost rule.
- **Light-honest refusals.** The Charterhouse refuses bookings to a system it believes blockaded, on
  its **own light-delayed knowledge**: it keeps accepting until the blockade's light reaches the hub,
  and keeps refusing until the lift's light does. Freight already in flight carries on — it launched on
  information that was true when it left.
- **Sovereignty.** No engagement may *open* within the Charterhouse's sovereign radius, for either
  party. Fleeing into it is sanctuary, by design.
### The law: standing, citations, and enforcement

The Authority's protection of its own hulls is **retributive, not preventive** — it runs no patrols and
posts no escorts, and the frontier stays lawless. What it does instead is *remember*, and *price*.

**This is priced outlawry, not prohibition.** Every consequence below is a cost a player can knowingly
pay. None of them is a wall. If a band ever makes attacking Authority freight strictly irrational, the
tuning is wrong — raiding the Charterhouse's shipping is meant to stay a live, expensive option.

- **Charter standing** is one number per corporation, starting at full and regenerating slowly and
  unconditionally in *every* band — time served is time served, so nobody is ever locked out by
  arithmetic alone. The five **charter statuses** are derived from it, never stored:

  | Band | At | What it costs you |
  |---|---|---|
  | **Good Standing** | full | Nothing. No tariff, no fee, nothing withheld. |
  | **Sanctioned** | below full | Freight tariff and an Exchange penalty fee, both ramping with the fall. |
  | **Suspended** | ~4 incidents | …and no *new* freight bookings. Freight already booked still completes. |
  | **Revoked** | ~8 incidents | …and the Exchange is closed. Resting orders are grandfathered; your warehouse is still yours to fetch from. |
  | **Proscribed** | ~12 incidents | …and the Authority sends **enforcement expeditions**. |

- **Citations arrive at c.** Killing a freighter changes *nothing* at the scene. The incident travels
  to the Charterhouse at lightspeed; only on arrival does standing move and a **public bulletin** issue
  naming the culprit — which then radiates outward to every player at c. A spree deep on the frontier
  drags a visible light-cone of consequences toward the map's centre behind you. The reputational hit
  and the legal one ride the same wavefront.
- **The Authority protects only its own hulls.** Raiding a *rival's* convoy is ordinary frontier
  business and produces no citation, ever.
- **Enforcement expeditions** are scripted, announced, and survivable: a squadron sails from the hub to
  blockade a proscribed corporation's nearest holding, using the ordinary blockade mechanic unmodified.
  The announcement's light outruns the sub-light squadron, so the warning genuinely arrives first —
  that *is* the lead time. It can be fought (destroying it ends it early, at the cost of a graver
  citation), waited out (it stands down on its own), or **called off by paying up**. It costs a
  proscribed corporation economy-time; it can never cost them a colony.
- **Reinstatement** buys standing back at a fixed price per point, burned as a sink. Paying visibly
  calls off an inbound expedition — the most direct expression of the whole design: the law is a bill,
  and you may settle it.

Deliberately *not* built: privateering / letters of marque, syndicate-shared standing, TCA bounties or
escorts, and any standing effect from player-versus-player combat.

### Other inherited economic structure

- **Valuations update slowly** (periodic close) to keep share prices readable and prevent
  earnings-momentum noise.
- **Standing orders** handle routine business (sell output, restock, maintain inventory) so the
  economy runs unattended — the async promise applied to commerce. A rule delivering to the
  Charterhouse now chooses whether to **sell on arrival** or bank the goods in the warehouse to trade
  later. (Standing-order convoys remain free auto-spawned hulls in this phase; unifying them with
  booked Authority freight is deferred.)

---

## 10. Warp Lanes (player-built infrastructure)

Lanes are not generated terrain — they are **infrastructure players construct**, and building the
road network is a core strategic activity.

- **Lanes are speed-up corridors**, not rails — and they govern **movement only** (speed + fuel),
  *not* coherence/information (the two networks are deliberately separate; see §11). The fiction: a lane
  **reduces a ship's effective mass** while it travels along the corridor. Because acceleration =
  thrust ÷ mass and fuel burn scales with mass, the single mass-reduction effect *generates both* the
  speed bonus and the fuel saving — one cause, two mechanics (the same standard the lattice fiction
  meets). Ships can always cross open space anywhere for a cost. A lane between two systems is rarely a
  straight line; the lane path may be longer than the open-space chord, creating the central tactical
  divergence (fast-but-predictable lane vs. short-but-costly open space). The mass benefit **scales
  with ship mass**, so it is transformative for the heaviest ships (trade convoys — the largest in the
  game) and marginal for light raiders — which is exactly what makes convoys lane-dependent and raiders
  open-space roamers, by physics rather than by rule.
- **Players build lanes.** Construction is a resource/time investment; your road network is capital,
  and its shape is a readable expression of your strategy. The map starts nearly empty of infrastructure
  and fills in as players develop — giving the map a *history*.
- **Starting condition:** each player begins with **one lane from their home system to the hub**, and
  nothing else. Everyone can trade from the first moment; every *other* connection must be built. This
  is the clean, universal opening.
- **Lanes are public — anyone can use any built lane** (settled). This is the simple choice and it
  removes a whole access-control layer, but it creates a deliberate, *kept* tension: **building a lane
  partly benefits your rivals** (their convoys use it too) and **partly endangers you** (it's also a
  fast approach for raiders toward your convoys). Lane-building is therefore a real cost-benefit
  calculation — build the efficient route everyone exploits, or stay sparse to deny infrastructure —
  with public-goods / free-rider dynamics emerging for free. *Do not "fix" the free-riding with
  ownership gating; the free-riding is the interesting game.*
- **Chokepoints return (as a richer synthesis).** Because a built lane is a known, fixed, preferred
  corridor, it is exactly where a raider lies in wait — so raiding now has *both* chokepoint control
  (camp the predictable lanes) *and* pursuit geometry (run down convoys that flee into open space to
  avoid the camps). The convoy's lane-vs-open-space choice is sharpened because operators *know* which
  lanes exist and which are dangerous.

---

## 11. Conquest & Victory

**Victory is by conquest: you defeat a rival by taking their home.** This is the inherited
"hostile-takeover" endgame, made concrete — the last corporation with a home standing (or a dominance
threshold over homes/systems; minor variant to pick) wins. Two things are settled and one is the real
remaining design work.

### Settled

- **Home planets can be attacked.** There is no inviolable base. A home is both a player's
  economically-perfect coherence anchor (live prices, instant settlement) *and* their military center
  of gravity and last fallback. Lose your forward command center and you fall back home (§6.1); lose
  your *home* and you are defeated. Home is the one thing you cannot afford to lose, which is exactly
  right for a conquest 4X.
- **Conquest does not run through coherence-warfare or command-center decapitation.** Decapitating an
  enemy's forward command center only knocks them back home (§6.1) — it is a *positioning* blow, not a
  win. Winning means physically taking a home. (This is a cleaner separation: the command center is
  *how well you see and react*; home conquest is *how you win*.)

### Retired (a non-problem we no longer need to solve)

The earlier "war as the severing of light" idea — dimming an enemy's coherence to defeat a player
turtled in an untouchable home — solved a problem that does not exist once homes are attackable. There
is no turtle corner: a player hiding at home is a player whose home you can come and take, and one who
*never* projects force cedes the entire map. **The whole coherence-warfare / buildable-anchors /
key-starvation-as-a-weapon cluster is retired.** (Lanes are movement-only and do not carry coherence —
§10 — so coherence-warfare had no vehicle anyway.) This removes the most speculative machinery in the
design; nothing is lost.

### The real remaining design work: what a home assault requires

The open question is **not** "can you win by conquest" (yes) but **what taking a home actually costs** —
and that balance is the entire feel of the endgame:

- *Too easy* → the game becomes a rush; first/hardest aggressor wins; the economic and expansion layers
  never matter; a single raid ends someone's game abruptly (the 4X collapses into pure extermination).
- *Too hard* → conquest is impractical, games drag, the conflict layer is toothless.
- **The fit, consistent with the whole design:** a home assault should be **hard, slow, and
  telegraphed by lightspeed** — a major campaign, not a smash-and-grab. A fleet massing for a home
  assault is *visible to the defender as delayed light* (they see it coming — stale, but coming),
  giving them time to recall fleets and fortify, while their own defensive commands also travel at *c*.
  The endgame siege thus *is* the information model at its most tense: even defending your home is an
  exercise in acting on delayed information against a delayed threat. Home-conquest becomes the dramatic
  **climax** of a game rather than a cheap early shock. The precise offense/defense balance is
  **playtest-heavy** and the main thing to tune here.

---

## 12. What's Inherited from Stellar Charters

This is a new game, but it stands on *Stellar Charters*' proven foundations:

- **The corporate economic spine** — chartered corporations, a central Wormhole Exchange, commodity
  trade, equity / corporate valuations, the "global price, local logistics" tension.
- **The design philosophy** — shown-math resolution, legibility as a cardinal rule, "risk communicated
  as history + factors," no hidden probabilities.
- **The hostile-takeover endgame concept** — corporate consolidation as a path to victory, realized
  as **conquest of rival homes** (§11).
- **The bot simulator / headless balance harness** — bots validate balance, humans validate legibility.
  This survives the rebuild because the simulation core is kept pure and I/O-free.
- **Procedural generation** — every galaxy generated fresh from a seed.
- **Standing orders / automation** — already present as a convenience; here promoted to the primary
  interface and given an in-fiction justification (command across light-minutes).

**The clean break from Stellar Charters:** discrete daily/WeGo turns → continuous async; lane-graph
movement → continuous-space acceleration physics; abstract strategy → lightspeed-bound observation and
command; and a new core fiction (the Lattice and the Ledger) that *generates* the rules.

---

## 13. Open Design Questions

The structural keystones are now **resolved** (recorded below for the trail), leaving a short set of
self-contained elaboration and one genuine endgame-balance question.

### Resolved (was Q1–Q4 + the conquest cluster)

- **Strategic cadence (was Q1) — DECIDED:** continuous real-wall-clock progress, no daily boundary,
  with queue-ahead + offline accrual + no-login-gated-bonuses as **required** mitigations (§5.1). Game
  **length is left open**, to be found by playtest (and tuned *together with* galaxy size, since the
  two are coupled — a longer game wants a bigger galaxy or expansion saturates).
- **Remote-commander experience (was Q2) — DECIDED:** the **movable command center** (§6.1) makes
  position a recurring strategic decision and resolves the passivity risk. Forward command is a **pure
  upgrade gambled against decapitation**; a killed command center **falls back to home** (not defeat).
  *Watch-flag:* keep decapitation risk meaningful or "stay home" becomes a dead option.
- **Lanes & coherence (was Q3) — DECIDED:** lanes are **movement-only** (speed/fuel via the
  mass-reduction fiction, §10); the coherence/information field stays a **separate** placement-based
  system. Convoys are **locked as the largest ships**.
- **Market: stale prices & sell-side (was Q4, in part) — DECIDED, then SUPERSEDED:** stale-price limit
  orders are still **accepted as-is** (no protection, but staleness must be *disclosed*). The old
  **buy/sell asymmetry** (sellers clearing at price-on-arrival, not a locked launch price) has since
  been **retired** by the Charterhouse warehouse: the Exchange settles against goods already at the
  station, so both sides are symmetric and price-certain, and *hauling* is a separate, explicit act
  with its own risk. See §9 and §TCA.
- **Conquest (was the Q5 cluster) — DECIDED & SIMPLIFIED:** victory is **conquest of rival homes**;
  homes **are attackable**; the entire coherence-warfare / buildable-anchors / key-starvation cluster
  is **retired** as a solution to a non-problem (§11).

### Still open

**Endgame balance (the one with real weight):**
1. **What a home assault requires** (§11) — the offense/defense balance of the climactic siege. The
   *mechanism* is settled (take the home; assaults are hard, slow, lightspeed-telegraphed); the
   *tuning* is the open work, and it is **playtest-heavy**. Too easy → rush; too hard → toothless.

**Self-contained market detail (blocks nothing):**
2. **Standing-price ↔ clearing interaction** — how an instant market order moves the posted price
   (walks it along the elasticity curve), and what the periodic limit-order clearing does to a price
   that has *already* moved from instant flow all interval (re-anchor to a fresh equilibrium, or
   process resting limits at the current walked price?). Plus the related question of what strategic
   role limit orders ultimately play (bets on future price movement vs. slippage-splitting large
   orders). A clean logic problem; solve when speccing market microstructure.

**Lane detail (some settled, rest open):**
3. Lane **defense / interdiction** (can a lane be fortified? — where deliberate chokepoint tactics
   live); lane **cost / construction / destructibility**; lane **quality / upgrades** (buildable
   route-quality stats — *do not* let placeholder route-stability art pre-answer this; §14).
   *(Settled: lanes are public-access; player-built; movement-only; start with one home→hub lane.)*

**Map & detection (most deferrable, partly playtest):**
4. Are home anchors fixed at generation or chosen in an opening phase? How exploration/surveying
   reveals the procedural frontier. Map generation parameter *values*. Enemy-contact decay handoff
   (how a lost contact ages, extrapolation, reacquisition). Sensor ranges and detection rules
   (including any stealth/signature layer that would let convoys go quiet to evade detection).

---

## 14. Technical Architecture (settled, summarized)

Self-hosted Rust authoritative game server (moving off Cloudflare to a VPS, which removes the prior
platform constraints):

- **Pure deterministic simulation core** — no I/O, no async; takes `&mut World` + commands, produces
  next state + events. Houses all lightspeed/intercept/market logic. Testable in isolation; runs the
  headless bot-balance harness; guarantees determinism from seed + command sequence.
- **Single-owner game-loop task** — one Tokio task owns the `World` (lock-free by construction) and
  ticks it via the pure core. The heartbeat. No data races on game state, enforced at compile time.
- **axum + WebSockets** as pure I/O — connections receive player *intents* and push *filtered state*;
  they never touch game state. Read/write halves split per connection.
- **Async Postgres (sqlx) persistence off the hot path** — append-only event log + periodic full-state
  snapshots; restart = load latest snapshot, replay forward. Never blocks the tick loop.
- **Per-player lightspeed-delay + fog view filter** — a *first-class component*, not a detail. Between
  the simulation's ground-truth events and each player's socket: a per-player delivery scheduler that
  holds each event until its light-travel time to that player's home has elapsed, and filters by what
  that player's assets can observe. This is the code embodiment of the entire information model and the
  novel/risky core; must be deterministically testable ("player X could not have known Y at time T").
- **Frontend galaxy map rendered with Pixi.js.** The galaxy map is a WebGL-accelerated 2D scene drawn
  with **Pixi.js** — chosen because the map is continuous 2D space with potentially many simultaneously
  moving, animating elements (ships under acceleration, convoys, swelling/collapsing uncertainty cones,
  the coherence field as a brightness gradient, lane corridors), which is exactly the high-element-count
  real-time 2D workload Pixi's GPU sprite batching is built for, and well beyond what SVG/DOM or Canvas
  2D handle smoothly. The client receives the per-player filtered state stream over the WebSocket and
  renders it; it holds *no* authoritative state and performs *no* game logic — it is a view onto the
  delayed, fogged picture the server sends. The visual grammar of the information model lives here: the
  coherence gradient as luminosity, staleness as fade/desaturation, contacts as last-known markers with
  uncertainty cones that grow between observations and snap tight on reacquisition.
- **Placeholder graphical assets** — reuse the existing *Stellar Charters* asset set at
  `https://github.com/peterholko/stellar-charters/tree/main/assets` **for now** (programmer art /
  prototype stand-ins, to be replaced before any real release). It is a usable starting set (~130 PNGs)
  already organized into the categories this game needs: branding/key-art and corporate emblems
  (`A-branding-and-key-art/`), ship and convoy sprites (`B-ships-and-convoys/` — including raider,
  corsair-corvette, cargo-freighter, escort-cutter, survey, and a convoy-group), map and environment
  art (`C-map-and-environments/` — notably `wormhole-hub`, star-system nodes, and colony-stage
  sprites), and UI icons (`D-ui-and-icons/` — fleet icons, resource icons, action and status icons).
  Much of it maps directly onto the new design (the wormhole hub, the raider/freighter split). Ignore
  any assets tied to dropped or undecided systems — e.g. the old WeGo turn UI, and the
  warp-route-stable / warp-route-unstable art (it presumes route *stability* as a mechanic, which this
  design has not adopted; lane quality/upgrades remain an open question — see §13, and don't let the old
  sprites pre-answer it). Treat all of these as temporary — they unblock building and testing the Pixi
  map and UI without waiting on bespoke art.

---

## 15. Suggested Next Steps

The structural keystones are resolved; what remains is elaboration, one endgame-balance question, and a
shift toward building/prototyping. Highest-leverage moves:

1. **Prototype the core loop early** — "log in, read the delayed/fogged picture from your command
   center, make commit decisions, set queues and doctrine, log out." This is the cheapest way to
   validate that the *feel* (the resolution to the old Q2) actually lands before sub-systems are built
   out, and to start finding game length and galaxy size by play.
2. **Design the home-assault / siege balance** (the one remaining weighty open item, §11.1 / §13) —
   the offense/defense tuning of the climactic conquest. Mechanism is settled; the balance is
   playtest-heavy, so stand it up in a form bots and humans can stress.
3. **Spec the market microstructure detail** (standing-price ↔ clearing interaction, §13) whenever
   convenient — a self-contained logic problem that blocks nothing but should be settled before the
   market UI is built in earnest.
4. **Elaborate the lane detail** (defense/interdiction, cost, destructibility, quality — §13) on top of
   the settled movement-only lane model.
5. **Defer map/detection tuning** (§13) to last — anchor placement, exploration reveal, contact-decay,
   sensor/stealth values, and generation parameters are best set by the bot simulator and playtest, not
   on paper.

Throughout: keep the **simulation core pure** (it is both the determinism guarantee and the bot-balance
oracle), keep **fairness vigilance** on every new strategic mechanic (§5.1), and hold **Pillar 2
disclosure** (always show staleness) wherever delayed information drives a decision.

---

## 16. Build Milestones

The full design (§1–§14) is a large, multi-month build. It is delivered in milestones, each a
*coherent, testable* state — never a half-wired version of everything at once. The browser
prototypes that preceded this doc were **throwaway** feel-tests (lightspeed delay, intercept-commit,
acceleration) and are NOT the alpha codebase; the alpha begins the real implementation on the real
architecture (§14).

### ALPHA — single-player vertical slice on the real architecture (current target)

The smallest slice that is genuinely *the game* (not a tech demo) and is playable/testable by one
person. It establishes the architecture and the core that everything else hangs off, and deliberately
DEFERS the rest.

**In the alpha:**
- **Real architecture (§14):** Rust authoritative server, pure deterministic simulation core, single
  game-loop task owning the world in memory, axum WebSockets as I/O, async Postgres (snapshot + event
  log) off the hot path, Pixi.js client. Single-player (one human; the rest of the galaxy is empty or
  trivially scripted) — multiplayer deferred.
- **The lightspeed information model (§6) — the centerpiece:** the per-player delayed + fogged view
  (here, single player), continuous-space movement, asset-based delayed vision, command latency. This
  is the non-negotiable heart; it must be in the alpha because it IS the game.
- **Continuous-space movement with acceleration (§7):** flip-and-burn, convoy slow/heavy vs. raider
  fast/light.
- **The raiding loop (§8):** intercept-commit, doctrine-based resolution, the commit-time decision.
- **Minimal economy (just enough to motivate convoys):** a hub, a home anchor, a couple of commodities,
  buy/sell at the hub with instant execution and lagged price info (§9) — but only the *minimum* needed
  to give convoys a purpose and put goods on raidable routes. NOT the full market microstructure.
- **Galaxy map (§4):** continuous radial space, random systems, hub at center, home anchor; rendered as
  the player's delayed/fogged Pixi view.

**Explicitly DEFERRED past the alpha:**
- Multiplayer (4–12 players, per-player view filters for multiple players).
- Full market microstructure: limit-order batching, standing-price/clearing interaction, equity /
  corporate valuations.
- Warp-lane construction (alpha can use open-space movement only, or a single pre-placed home→hub lane).
- Conquest / home assault / victory condition.
- Settlement-key fiction surfaced as mechanics, coherence as a contestable system, exploration/survey,
  research/tech, population growth depth, standing-order/doctrine UI breadth.
- Balance tuning of any kind (alpha is to prove the core is *fun and coherent*, not balanced).

**Alpha definition of done:** one person can run the real server+client, command from a home anchor,
send a convoy to trade at the hub, commit a raider to intercept a convoy (or watch their own convoy be
threatened) under honest lightspeed delay, and form a clear judgment that the core game loop — *acting
and commanding in a delayed reality, with goods crossing dangerous space* — is engaging. Built on the
real architecture so it is the foundation of the game, not a throwaway.

### Post-alpha (rough order, each its own milestone)
1. **Multiplayer** — multiple players, per-player view filters, the shared hub as commons.
2. **Full market** — limit orders + batch clearing, equity/valuations, the standing-price mechanics.
3. **Warp-lane construction** — player-built public lanes, the mass-reduction speed model.
4. **Conquest** — home assault, victory condition, the endgame siege.
5. **Depth & breadth** — research, coherence-as-contestable, exploration, doctrine UI, settlement-key
   economy — and only then, **balance** (via the bot simulator + playtest).
