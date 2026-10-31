# Traveling merchant

Status: phase 1 built Oct 14 2026 (uncommitted), protocol v50: the client and
server deploy together. See **As built** at the end.

**Charter v5 (Oct 16 2026, protocol v53,
[founding-charter.md](founding-charter.md)):** the trader is called when the
report of the privateer's raid reaches command (charter step 5), not by the
mine; under v4 it was the reveal of the kit's fake semiconductors. It appears
2,500 su out and docks as it enters the home's 900 su hyperlimit, about 12
seconds later (Oct 27 2026; it used to crawl on to the star at impulse and dock
there, about 40 seconds after the call). It carries 100
semiconductors (was 60), enough for every charter build, and since v5 the 15
Armaments the raid took; it takes up to 300 Ferrite Ore a visit (was 150), so
a founder who spent the stipends elsewhere can still raise the buy-back.
Step 6 is "Sell ore to the trader" (`FoundingProgram::ore_sold`). The guide's
"Fill for" targets add the Solar Array and the Interceptor, and two errands:
`merchant-ore` (sell all the ore it will take) and `merchant-fuel` (Deuterium
for an Interceptor with an empty tank). The rest of this page describes the
v3 cue.

**Teo Brannick (Oct 16 2026):** Peter gave the trader a personality: Teo
Brannick, master of the Long Odds, a big, loud, theatrical haggler (braided
grey beard, a brass loupe over one eye, rings on every finger). Peter chose
the light version of negotiation: Teo talks, the prices stay the sim's.
- **The table** (Deck pop-out and mobile sheet): Teo's portrait, name and
  ship with the time left; Teo's word on the offer in a quote that takes the
  mood (neutral, pleased, scoff); "You give" (Ferrite Ore, with the ore you
  have at home on its own line, "212 at home" — Peter, Oct 20 2026 — and the
  box capped at it, and **Max** under it: all you hold, up to what Teo takes
  — Peter, Oct 21 2026) beside "You get"
  (Teo's goods), both at once, no tabs; the terms ("You pay 868 Cr · storage
  +23", "You receive 160 Cr") and **Trade** (was "Shake on it" until Oct 21
  2026) pinned at the foot. The Deck's table keeps its full height under an
  open founding guide, which stays open and floats over its lower left
  (Peter, Oct 21 2026); the total sits beside **Trade** so the guide never
  hides it.
- **Teo's lines** (`client/src/core/derive/trader.ts`, `traderLine`): a greeting
  ("Raided! Terrible. Lucky I came by, eh?", later "Back for more? Good,
  good."), a take on ore, chips, Armaments, a big order or ore and goods
  together, and every refusal in Teo's voice (`refusalLine`: "Credits, friend.
  Or ore. I take both.", "I've a hold, not a moon.", "And where would you put
  it all?"). After a deal: "Done! You'll tell your grandchildren.", unless
  the server's own line ("Free trader: trade refused — …") says otherwise
  (`traderResult`). Mobile updates Teo's line in place as you type, so typing
  is never re-rendered over.
- **The portrait:** delivered Oct 16 2026 in
  `client/art-src/free-trader-portrait-v1/`: neutral (holds up a
  semiconductor), pleased (laughs, open palm) and scoff (strokes a beard
  braid). `buildTraderPortraitArt` derives a 320-pixel WebP of each, cropped
  to the same square around the face (`TRADER_PORTRAIT_CROP`) so it reads at
  90 px on mobile and 140 px on the Deck (25% larger since Oct 21 2026; 320
  keeps 140 px sharp on 2× screens). A missing expression falls back to
  the neutral portrait, a missing portrait to the Long Odds (`traderPortrait`
  probes each painting once).
- The docking pop-up names Teo ("Teo Brannick's Long Odds has docked at your
  colony…"), and so does the top-bar chip's tooltip. Preview:
  `/scripts/merchant-fixture.html` on the vite dev server.

**The last call (Oct 16 2026,** Peter: "Teo should leave while the player is
eliminating the pirates"**):** once the privateer's kill is reported, with
its bounty, Teo stays 3 minutes more (`merchant::LAST_CALL_S`), less if the
den falls first, so charter step 12 ("Export your ore") sells at the Market
Hub. The stall's clock counts it down.
- The guide's card during it reads "Stock up before the trader leaves · 65
  Semiconductors, 15 Armaments for the builds and research ahead. [Buy from trader]"
  (`merchant-ahead`), with the den's own card beside it and a "Trader leaves
  in m:ss" line.
- The prefill is what the builds after the den lack (the Academy, the
  expansion's Solar Array and Mining Complex, the Survey Probe and the Mining
  Barge, as whole recipes). Since Oct 26 2026 it adds a 20-semiconductor
  reserve for research, which draws them from home stock
  (`RESEARCH_RESERVE`). Since Oct 29 2026 it adds the vendetta's second
  Interceptor (12 semiconductors, 15 Armaments). Home stock is subtracted,
  and the total is bounded by the trader's stock: 65 semiconductors and 15
  Armaments on a kit nobody touched.
- The founding stock covers it: 115 semiconductors (35 for the buyback after
  the raid plus these 65 makes 100) and 30 Armaments (the first
  Interceptor's buyback and the second's).
- Teo's greeting: "Last call, friend! I'm off before the shooting starts."
  (after a deal: "Anything else? The clock's ticking.").
- The departure line: "Teo Brannick's Long Odds left {home}."
- Bots buy the same goods, semiconductors first, plus Deuterium for the
  Interceptor (`charter_stock_up`). From then on their optional works leave
  the builds ahead their goods (`charter_reserve`).
- Why (measured with a bot): leaving on the bounty with no stock-up pushed
  research from 21.7 to 46 game-minutes, and mining had not expanded by 70.

**Four traders (Oct 27 2026,** Peter: "shouldn't we have like 3 different
merchant characters at least, that way not all the players see the same
merchant", then "can we have 4th male free trader"**):** the server deals one
of four per corporation.
- **The cast:**
  - Teo Brannick of the Long Odds, the big, loud, theatrical haggler above.
  - Ines Varga of the Clean Slate, a cool, exact former Authority customs
    auditor who quotes and never haggles ("The numbers don't close.").
  - Mags Halloran of the Second Wind, a warm, unhurried old hauler who calls
    you "pet" and is shrewder than she looks ("Slow ships get there too,
    pet.").
  - Rafi Mensah of the Small Fortune, a quick, cheeky young hustler who calls
    you "chief" ("Ore? Sure. 4 Cr a unit. Practically charity.").
- **Dealt:** `merchant::FreeTrader::of` is dealt like the pirate family,
  from the galaxy seed, the corporation and its join tick. It is never
  saved, and a new galaxy deals afresh. `World::free_trader(owner)` serves
  `FoundingView.trader` (`teo`, `ines`, `mags` or `rafi`; protocol v67).
- **The server's own lines name the dealt trader:**
  - the departure line, e.g. "Ines Varga's Clean Slate left {home}.";
  - the lost-crate signal, via `SignalKind::name_for`: "Ines's lost crate"
    in the New signal and report lines, and its cache's name.
- **The client** (`core/derive/trader.ts`):
  - `TRADERS` holds each trader's name, ship, portrait stem and a full
    voice: every greeting, take, refusal and parting shot above, in their
    own words. `traderOf(founding)` picks the dealt one, Teo until the
    server says.
  - The stalls, the docking pop-up, the chip and the lost-crate card
    (`signalInfo`) all take it. The paintings are shared.
- **Portraits:** Ines's, Mags's and Rafi's three portraits each are prompted in
  `client/art-src/free-trader-portrait-v1/` with Teo's framing, so
  `TRADER_PORTRAIT_CROP` fits them all. All twelve are painted; a missing one
  falls back to the trader's own ship.
- **Ships (Oct 27 2026):** each trader flies their own: Teo the Long Odds
  (`free_trader` art), Ines the Clean Slate, Mags the Second Wind, Rafi the
  Small Fortune.
  - They are art kinds, not ship kinds: all four fly an Authority Freighter
    hull, at its footprint (`ART_VARIANTS`).
  - The server latches the trader on the merchant's ghost track and serves
    `GhostView.trader` (protocol v70).
  - `traderShipArt` (client `protocol.ts`) picks the art for the map sprite and
    close-up model, the fleet icons, the docking pop-up's stand-in and the
    stall's portrait fallback.
  - Sources: `client/art-src/ship-models/{clean-slate,second-wind,small-fortune}-textured-v1/`.
    LOD2s are 3,964, 3,968 and 3,894 triangles.
  - The docking painting (`free-trader-inbound`) still shows the Long Odds.

## The problem

Every charter step after the mine needs goods from the Market Hub, 74k su away
(Oct 14; since Oct 15 homes sit 100–150k su out and every Hub trip is longer,
see [hub-district.md](hub-district.md)):

| Step | Needs from the Hub |
| --- | --- |
| Shipyard | 15 Alloys, 13 Machinery, 15 Semiconductors (beyond the kit) |
| Academy | 25 Alloys, 15 Semiconductors |
| Survey Probe | 4 Semiconductors |
| Mining Barge | 40 Alloys, 20 Machinery, 12 Polymers, 6 Semiconductors |

The first shipment's round trip is about 26 minutes on slow warp, and
Authority freight is slower. Building is quick (the mine, the Shipyard and the
Academy each take about 20 s). The Shipyard can't start until about 26 minutes
in, so most of the charter falls outside a first session.

## The idea

A traveling merchant arrives at a new player's home while the charter runs.
It sells the founding goods and buys a little ore, instantly, in orbit. The
first shipment still flies to the Hub (income, the privateer fight, the
bounty), but nothing waits for its return.

After the charter it moves on, and later it travels between colonies on a
schedule (phase 2).

## Phase 1: the founding visit

**Arrival**
- It drops out of warp near the home once the Mining Complex is staffed
  (charter step 1), and docks within about a minute.
- It's a real ship on the map: a free trader under Authority protection.
  Attacking it draws citations, as attacking Authority freight does.
- One merchant per corporation, during its founding charter. Bots play the
  same charter, so they get one too. Corporations past their charter don't
  (phase 2 covers them).

**Stay**
- Until the charter is fulfilled, at most 3 hours. Then it leaves with a
  timeline line ("The merchant leaves for … · back in about 6 h" once phase 2
  exists).

**Catalogue (founding visit)**

| Good | Stock | Sells at | Buys at |
| --- | ---: | --- | --- |
| Alloys | 120 | Hub mid +50% | — |
| Machinery | 50 | Hub mid +50% | — |
| Semiconductors | 60 | Hub mid +50% | — |
| Polymers | 30 | Hub mid +50% | — |
| Provisions | 40 | Hub mid +50% | — |
| Deuterium | 30 | Hub mid +50% | — |
| Ferrite Ore | — | — | Hub mid −50%, up to 150 units a visit |

- **The merchant is a last resort.** Peter (Oct 13): "the markup should be like
  50-100%, the idea the player should only rely on it when needed." Later
  visits roll each good's markup between +50% and +100%. The founding visit
  pins it at +50% so the charter stays affordable and predictable.
- Prices are fixed for the visit: the Hub mids when it left the Hub, which is
  what light-delayed news would show anyway. A round trip through the merchant
  always loses money.
- Founding cost: about 8,750 Cr at +50% for everything the charter needs beyond
  the kit, against about 8,800 Cr of starting money, stipends and bounty by the
  barge step. That's tight on purpose: the first shipment's return basket, at
  Hub prices, is the cheaper way to cover the barge's parts.
- The Shipyard top-up is about 2,560 Cr, which the player has by then (2,000
  start + 300 mine stipend + 500 shipment stipend).
- At +100% the Shipyard would wait for the privateer bounty (about 6 minutes in)
  and the barge for the return basket (about 30 minutes in), stretching the
  charter to about 40 minutes. That's why the founding visit stays at +50%.

**Trading**
- Instant: it's docked at the command's own system, so there's no light delay.
- Goods go straight into the home stockpile, limited by storage room. Credits
  settle at once.
- The wallet shows them at once: the treasury sits at home
  ([treasury.md](treasury.md), Oct 27 2026). Before, credits were the Hub's
  report, about a minute late at a home, so a second deal within that minute
  was priced against the old balance, passed the stall's check and was
  refused.
- One command, `MerchantTrade { buys, sells }`, checked against stock, the ore
  cap, credits and storage.

**Guide**
- Shipyard step: "Buy the parts from the merchant" opens the merchant with the
  Shipyard's shortfall filled in. The Authority-freight fallback stays for a
  player who prefers it.
- Academy, Survey Probe and Mining Barge steps: the same shortcut for their
  shortfalls.
- The first-shipment step is unchanged. Steps the player finishes early already
  complete the moment they come up, so the guide suggests the next build while
  the Freighter flies.

**Expected first session**

| Time | Step |
| --- | --- |
| ~2 min | Mine online; merchant arrives |
| ~3 min | First shipment leaves; Shipyard parts bought |
| ~4 min | Shipyard built and staffed |
| ~6 min | Privateer defeated, bounty paid |
| ~8 min | Academy built, research started |
| ~11 min | Nearby system charted |
| ~13 min | Mining Barge launched |
| ~25 min | Belt ore home: charter fulfilled |
| ~29 min | Freighter back from the Hub with the first sale |

Today the Shipyard alone waits about 26 minutes.

As built (Oct 14), the guide opens the Shipyard step only once the
privateer's defeat is reported (about 6 minutes in), so the Shipyard lands
nearer 7 minutes and later steps shift about 3 minutes. A Shipyard built
during the shipment's flight still completes the step the moment it opens;
the guide just doesn't suggest it then. Unmeasured.

## Phase 2: traveling (later decision)

Full design (Oct 14 2026, parked the same day: phase 1 only for now):
[traveling-merchant-phase2.md](traveling-merchant-phase2.md). It recommends
hailed visits instead of the timetable below.

- After the founding, the merchant visits each corporation's colonies on a
  rough schedule: about every 6 hours, staying 30 minutes, announced about
  30 minutes ahead.
- Each visit brings a rotating special: an Authority core at a premium, a rare
  material, or a discounted stock line.
- Possible events: pirates chase the merchant into your system; defend it for
  a reward.

## Build sketch (phase 1)

- **Sim:** `merchant.rs` (the visit: fleet, host system, price snapshot,
  stock, ore bought) and `world/merchants.rs` (arrival on the mine step,
  docking, `MerchantTrade`, departure). Events: arrived, traded, departed.
  A new neutral owner, or the Authority's, for the ship.
- **Server:** the ship in the View like other NPC fleets; the catalogue and
  prices owner-only on the home system; timeline lines; admin funnel unchanged.
  Protocol bump; client and server deploy together.
- **Client:** a merchant panel (Market-style list: stock, price, buy/sell
  amounts, storage room, credits); a "Merchant in orbit" chip on the home and
  top bar; guide shortcuts; mobile sheet; the ship on the map (stand-in art).
- **Bots:** buy project shortfalls from a merchant in orbit before the Hub.
- **Art:** a merchant ship brief for your model pipeline; the Authority
  freighter stands in until it lands.
- **Tests:** arrival on the mine step; prices and the ore cap; storage and
  credit checks; departure on charter completion; a founding playthrough
  reaching the Shipyard in under 10 game-minutes.

## Decided

- **Markup:** +50% to +100% (Peter, Oct 13): a last resort, not a second Hub.

## Open decisions (recommendation first)

1. **Founding visit markup:** pinned at +50%; or rolled +50–100% like later
   visits (the charter then takes 25–40 minutes depending on the roll).
2. **Buys ore:** yes, capped at 150 units a visit at −50%; or sells only.
3. **Arrives:** when the mine is staffed; or at landing.
4. **Stays:** until the charter is fulfilled, at most 3 hours; or a fixed hour.
5. **Owner:** an independent free trader under Authority protection; or an
   Authority ship.
6. **Phase 2 (traveling visits):** later, as a separate decision; or build it
   now.

## As built (Oct 14 2026)

All decisions took the recommendation: +50% founding markup, Ferrite Ore
bought at −50% up to 150 a visit, arrives with the mine, stays until the
charter is fulfilled (3 hours at most), Authority-protected, phase 2 later.

**Sim**
- `crates/sim/src/merchant.rs`: the visit (`MerchantVisit`), its founding
  catalogue and prices, and `quote()`, the one check shared by the sim, the
  View and the bots.
- `crates/sim/src/world/merchants.rs`: `advance_merchants` runs each tick after
  the charter's own check.
  - It calls the merchant once the "Mine online" milestone is stamped
    (`FoundingProgram.merchant_called` makes it once per charter). §charter-v4:
    once the fakes are revealed instead; §charter-v5: once the raid's report
    arrives (`raid_report_at`).
  - It drops in 5,000 su out on the Hub side (§charter-v4: 2,500 su). It
    docks as it crosses the home's hyperlimit (Oct 27 2026, Peter: the long
    gap between its arrival in orbit and the pop-up). It comes to rest at the
    star, and the next tick's movement berths it at the home world. Until
    then it docked only when idle at the star, after a ~28 s crawl across
    the well.
  - It leaves when the charter is fulfilled, its 3 hours are up or the home
    changes, and vanishes 6,000 su out. §charter-v5: also when its last call
    ends (3 minutes after the privateer's kill report, or the den's fall).
- `merchant_trade` handles `Command::MerchantTrade`: one atomic exchange, or a
  refusal naming the first thing that doesn't fit (stock, ore cap, home ore,
  storage room, credits, charter sanctions).
- The command routes to the merchant's system like other system commands. For
  a founder that's the command center's own system, so there's no delay.
- **The ship:** an Authority-owned Freighter hull (`PlayerId::TCA`), so pirate
  AI leaves it alone and attacking it draws citations. A merchant destroyed in
  combat takes its stall with it.
- **Events:** `MerchantArrived`, `MerchantTraded`, `MerchantTradeRefused` and
  `MerchantDeparted`, located at the home.

**Server**
- **View:** `View.merchant` (owner-only `MerchantView`: phase, leave-by,
  markup, goods with stock, price and Hub mid, ore price and room) and the
  public `GhostView.merchant` flag. The flag is latched like `rescue_service`
  and `#[serde(default)]` in the saved track, so older checkpoints load.
- **Berth (Oct 15 2026):** while it trades, the View reports it docked at the
  home (`World::merchant_berth`). Before this it sat on the home star: its
  galaxy sprite covered the star, which blocked the zoom into the System View,
  and the System View drew it on the star instead of beside the home world.
  Reported docked, it leaves the galaxy map like any berthed fleet and orbits
  the home world (`fleetBerth` in `systemlayout.ts`) in the System View and the
  planet's sky. The sim's own `dock_of` is unchanged (ownership-based), so it
  never refuels from the home's stockpile.
- **Timeline:** "A free trader docked at …", "Free trader: bought … for … Cr",
  "Free trader: trade refused — …" and (Oct 16 2026) "Teo Brannick's Long
  Odds left …".
- **Bots** (`bots/logistics.rs`, `merchant()`): while it's docked they buy the
  current project's shortfall only, from spare cash, one purchase per round
  trip.

**Client**
- **Helpers:** `core/derive/merchant.ts`: quote, shortfall prefill, docked check.
- **Desktop:** a "merchant" pop-out (`shell/deck/merchant.ts`, route title
  "Free Trader").
  - No "Fill for" row (removed Oct 15 2026): the guide's "Buy from trader"
    opens the stall with that build's shortfall filled in.
  - One row per good (its price beside the Hub's) and an ore row.
  - A live quote with the first refusal reason, and one Trade button.
  - Oct 15 2026: two tabs, "For sale" and "Buying" (each with a count of the
    lines entered); amounts on both go into the one Trade. The quote and Trade
    are pinned to the panel's foot, so nothing needs scrolling. Mobile
    matches (both lists stay in the sheet, one hidden, so Trade reads every
    amount).
  - Oct 15 2026: the markup is the trader's own business. The stall shows
    its prices and stock only: no markup, no Hub price beside its own, and
    no credits or storage stats (the top-bar chip's tooltip too). Mobile
    matches.
  - A top-bar "Free trader · time left" chip while it's here.
- **Mobile:** a "merchant" sheet with the same rows, read on Trade.
- **Guide:** while it's docked, the Shipyard, Academy, Survey Probe and Barge
  steps offer "Buy from trader" with the shortfall filled in, and keep the
  ordinary build step beside it.
- **Told first (Oct 16 2026):** the guide, its shortcuts and the top-bar chip
  name the trader only once its pop-up has shown (`ViewState.storyTold`, set by
  `StoryEventUI` as each pop-up appears), not on the server's docking report:
  the pop-up waits 3 s (15 s until Oct 27 2026) after the previous one (the raid's, since v5) closes,
  and the guide used to name the trader in that gap. A docking line from an earlier session counts as
  told.
- **Labels:** "Free Trader" on the galaxy map, the system view, map picks,
  fleet panels and docked lists.

**Tests**
- **Sim:** pricing and the ore cap; arrival on the mine, once only; leaving on
  fulfilment and on time-out; the last call and the den cutting it short; whole-or-refused trades with each reason; only
  its customer, only while docked.
- **Server:** a bot buys only its project's shortfall; the wire round-trips
  `MerchantTrade`.
- **Client:** `test-merchant.mjs` (quotes, refusals, prefill, the desktop
  stall); `test-progression.mjs` (the guide's trader step).

**Not done yet**
- No harness run.
- ~~No art~~ Art landed Oct 14: the free trader flies its own hull on the
  map, in the System View and in the planet view's orbit
  ([free-trader-art-prompt.md](free-trader-art-prompt.md), **As built**).
- No arrival toast: the chip and the guide point to it.
