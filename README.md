# Stellar Syndicates

An asynchronous, multiplayer (4–12 player) continuous-time 4X space strategy game
about corporate trade and conflict across a wormhole-linked galaxy. Its defining
mechanic is **lightspeed-delayed observation and command**: you never see the
galaxy as it is *now*, only as the light that has reached your command center —
and your orders cross space at the speed of light, arriving late.

See [`GAME_DESIGN.md`](GAME_DESIGN.md) for the full design and
[`MULTIPLAYER_PROMPT.md`](MULTIPLAYER_PROMPT.md) for the milestone build plan.

---

## Status

| Milestone | State | Notes |
|-----------|-------|-------|
| **M1 — Multiplayer architecture scaffold + sessions** | ✅ **Complete** | Full architecture skeleton, end-to-end, built for many players. |
| **M2 — True-world sim (continuous space + acceleration)** | ✅ **Complete** | Galaxy, ships, flip-and-burn physics; clients render the shared moving world. |
| **M3 — Lightspeed information model (the core)** | ✅ **Complete** | Per-player delayed/fogged views from each command center; fairness guarantee enforced & adversarially reviewed; command latency. |
| **M4 — Raiding loop (PvP)** | ✅ **Complete** | Intercept-commit pursuit; resolution in true space; delayed reports on each player's own clock; recall can miss. |
| **M5 — Full multiplayer economy** | ✅ **Complete** | Hub Exchange (instant execution, lagged ticker), market + limit orders with uniform-price batch clearing, raidable trade convoys, buy/sell asymmetry, slow equity valuations. |
| **M6 — Robust sessions, persistence, scale to 12** | ✅ **Complete** | Restart restores the galaxy from the latest snapshot; 12 players in one galaxy with the loop keeping up; corps persist + reconnect resumes. |
| **M7 — Client polish** | ✅ **Complete** | Credits/equity in the HUD, the full delayed-map + market + raid UI tied together, and a run + play guide; the core loop is playable by multiple people. |
| **System claims + resource production** | ✅ **Complete** | Star systems have resource **deposits** (richer/more valuable toward the frontier); players **claim** systems (credit cost), claimed systems **produce** over time, and that production **ships to the hub** in the same raidable convoys — so raiding now destroys real output. Ownership is light-gated to rivals; stockpiles stay private. |
| **Acceleration & proportional pursuit** | ⤳ **Superseded** (see KINEMATICS below — acceleration removed post-playtest for constant per-kind speeds) | Ship acceleration was **derived from thrust ÷ mass** (`a = F/m`), so the raider/convoy nimbleness gap emerges from mass (convoy hull ~22× the raider's) and a **laden convoy accelerates worse** (cargo adds mass). Raiders run convoys down with **proportional steer-and-correct pursuit** (no closed-form solver), easing into a clean contact. The commit shows a **crude, drifting intercept estimate** rendered as a soft/fuzzy zone (sensor-circle idiom). Tuned LOW so a chase is watchable over tens of seconds. |
| **Autonomous defensive interception** | ✅ **Complete** | A patrolling raider **escorts a friendly convoy and, on its own, intercepts a hostile raider** it senses inbound on it — server-side, every tick, **whether or not the owner is online** (defense is standing doctrine, like offline production). Detection is fog-respecting (only raiders within the picket's sensor range); engagement reuses proportional pursuit + the seeded raider-vs-raider battle; the owner learns the outcome as **delayed news on their own clock**. Patrol **positioning** decides what it can defend (tunable). First piece of a future defensive-doctrine system. |
| **Standing logistics orders (async automation, Layer 1)** *(branch `async-automation`)* | ✅ **Complete** | Constrained, non-scripting rules a player sets that execute **automatically on the server clock, online or off** — the heart of check-in-friendly async play (§15). One rule shape (source system → destination = hub/home/another system, with a trigger: **ship-above-threshold**, **% of surplus**, or **maintain-a-level-at-the-destination**) auto-dispatches the existing **raidable** convoys; a new system→system delivery mission feeds depots. Two anti-spam gates bound a rule to **one in-flight convoy** + a fixed eval cadence (no flood). Setting a rule is instant local admin (reveals nothing to rivals); the convoy it spawns is sub-light, raidable, and light-revealed like any other. Deterministic + persisted (serde); **verified offline** (credits accrue while disconnected). |
| **Fleet doctrine (async automation, Layer 2)** *(branch `async-automation`)* | ✅ **Complete** | A corp-wide, **constrained** combat & logistics policy your autonomous pickets follow **on the server clock, online or off** (§16) — you set standing intent, not micro. Four closed menus: **engagement** (avoid · defensive-only · engage-weaker-when-you-outnumber · engage-any), **retreat threshold** (withdraw home when the local sensed friendly:hostile force-ratio drops below 25/50/75% — re-checked each tick, so reinforcements can break a committed fight — or never), **escort** (guard nearest / richest convoy, or hold-station to picket a fixed chokepoint), and **lost-supply** (a supply convoy to a system you no longer hold: drop the cargo, or re-route it home / to the hub to sell — still raidable on the new leg). Generalises the autonomous-defence picket; pickets sense only what's in range (fog-respecting) and the ships they command stay sub-light + raidable + light-revealed. Every default = the pre-Layer-2 behaviour (additive). Deterministic + persisted (serde); **verified offline** (autonomous engage/retreat/escort run with no player connected) and on the wire (doctrine round-trips through the private View). |
| **Check-in loop (async automation, Layer 3)** *(branch `async-automation`)* | ✅ **Complete** | The awareness interface that makes once-a-day play viable (Pillar 1: *presence buys awareness, not advantage*). On reconnect you get a **welcome-back digest** — a per-player **timeline** of what became **observable while you were away** (your automation's dispatches/sales/deliveries and lost-supply re-routes on your own clock; distant **battles** and **rival claims** arriving **light-delayed** to your command center — the same retarded-time rule as the map). The journal is **buffered server-side across disconnects** (so the offline player's "since you were away" is real) and bounded. Alongside it, **attention items** surface the decisions waiting for you (idle stockpiles to automate, rules pointing at lost systems, producers with no standing orders) — derived purely from your own View, so they add no information and never decay. Server+client only (no sim change); awareness is strictly light-respecting. Unit-tested (offline buffering, light-delay split, away-boundary, bounding); **verified live** (events fired while disconnected appear in the reconnect digest). |
| **Client UX: unified rail + Star System view** *(branch `async-automation`)* | ✅ **Complete** | A UX overhaul (Stellar-Charters-inspired) that **declutters the map**: the scattered overlay panels are unified into one **right-docked tab rail** (System · Market · Logistics · Doctrine) — one column beside the map, one tab at a time, **closes cleanly** (Esc); check-in stays a centered modal. Built on a shared "kit" (CSS tone-tokens + `.panel/.stat/.seg/.badge/.bar/.spark` classes, string-template helpers, one delegated listener per panel). The **Star System view** is a master→detail workspace: flavor header + **light-gated** ownership badge + stat-strip (deposits / yield-per-s / stockpile[owner-only] / claim cost) + rich geology readout (richness bars, reserves) + owner-only production readout + context actions (**Claim** / **Ship→hub** / **Auto-supply from here** [deep-links Logistics] / **Open market**) + an owned-systems rail when you hold several. Client/UX only — no sim/protocol change; fog model intact (rival holdings never leak). Hotkeys: **S** system · **M** market · **O** logistics · **F** doctrine · **L** check-in · **Esc** close. Verified live. |
| **Client UX: rich Market board** *(branch `async-automation`)* | ✅ **Complete** | The Hub Exchange tab upgraded to a Charters-style board: per-commodity rows with an **observed-history sparkline** + price + **trend glyph** (▲/▼, color+glyph dual-encoded) + held, driving a Buy/Sell **order composer**. **Honest staleness**: prices are the light-delayed ticker — a live↔"~Ns stale" badge, dimmed `~`-prefixed values, sparklines built from the player's **own observed price history** (client-accumulated; the trend is *observed, not a server forecast* — no fabricated "pressure" signal). The composer's plain-language preview **surfaces the buy/sell asymmetry** (buy settles now then a raidable delivery convoy; sell dispatches a raidable convoy that clears at the price *on arrival*; a limit rests + clears in the uniform-price batch). Wallet stat-strip + resting limit orders. Client/UX only — same MarketBuy/MarketSell/PlaceLimitOrder messages, same lagged-ticker mechanics. Verified live (a buy settled through the new composer). |
| **Start owning a HOME STAR SYSTEM** *(branch `async-automation`)* | ✅ **Complete** | Travian/OGame convention: a new corporation **begins owning a developed home star system** (granted free, no claim cost), with its **command center at it** — instead of a bare floating anchor with zero systems. One co-located home `StarSystem` per home ring slot is generated with the galaxy (so its static info ships in the one-time Welcome) with **modest, reliable starter geology** (renewable provisions + ore at low richness — a stable base, *deliberately* weaker than the dangerous frontier, so expansion stays the reward). It's a normal owned system: it **produces from turn one**, has a stockpile, ships to the hub, can be automated/defended, and its ownership is **light-gated to rivals** exactly like any claim (`claimed_at` = join time; stockpile owner-only — no leak). Reserved home systems can't be claimed from the pool; `command_center` stays a separate, relocatable field that just *starts* grounded at the home. **Deterministic** (forked per-home seed, independent of the frontier/event RNG) + **persisted** (serde). Client reconciles the rendering: the home is an **owned cyan system + a command-seat pulse + "HOME" label**, with the redundant anchor circle dropped (no more mystery circle); the System view tags it **HOME BASE / your command seat** and marks unassigned ring slots as *reserved*. 60 sim + 24 server tests (owns exactly one home at a sensible spot, produces + ships, modest-not-jackpot, reserved-from-claiming, idempotent reconnect, determinism, light-gating); **verified live** (fresh player owns a selectable, producing home and ships its output turn one). |
| **Map zoom & pan** *(branch `async-automation`)* | ✅ **Complete** | The galaxy map is now navigable: **mouse-wheel zoom toward the cursor** (the world point under the cursor stays put), **left-drag to pan**, **+/⊡/− buttons** (⊡ = fit/reset), and keyboard (`+`/`−`, arrows). Driven entirely through the renderer's single `worldToScreen` transform (`scale`/`cx`/`cy`), so *everything* — systems, ownership rings/labels, hub, ghosts, command center, sensor bubbles, cones, routes, ring guides — follows for free; the world-anchored background redraws on view change, the starfield stays a fixed backdrop. Zoom is **clamped** (min ≈ fit, max ≈ inspect one system) and a resize **preserves** the user's view (re-fits only if untouched). Critically, a **click-vs-drag gate** keeps the existing pointer actions intact: a tap (movement under threshold) runs the click logic exactly as before; exceeding the threshold becomes a pan and **suppresses** the click — no accidental move orders / raids / selections. All hit-testing already used `screenToWorld`, so selection stays correct at any zoom/pan. Client/UX only. Verified live: wheel zooms toward the cursor, drag pans, **system selection (incl. the home system) and ship selection still work under zoom/pan**, drag fires no click, reset re-fits, panels unaffected. |
| **Resource SINK 1 — building COSTS resources** *(branch `async-automation`)* | ✅ **Complete** | Travian-style growth sink (§step1 part 1): **Ore/Alloys/Fuel become build materials, not just goods to sell.** Building a ship or developing a system **deducts a fixed recipe** from the owning system's stockpile, enqueues a **build job**, and **resolves it after a fixed duration** — server-driven, online or off. Entry builds use **Ore alone** (any ore system, incl. your home, builds them), while the advanced **Raider** needs frontier **Alloys + Fuel** (Ore and Alloys rarely co-occur, so it requires shipping materials — the "spread of systems matters" payoff). Recipes: **Convoy = Ore 35 / 12 s**, **Extractor = Ore 60 / 18 s** (a system development that compounds output `×1.5^tier`), **Raider = Alloys 18 + Fuel 12 / 10 s**. A short stockpile **soft-rejects** (no debit, no job — async-fair). The world-level **build queue carries the payer**, so a ship is delivered to whoever paid even if the system flips mid-build (an Extractor is dropped if the system is lost). **Fog-safe**: in-progress build state + extractor tier are **owner-only** in the View (never leak to rivals); a built ship spawns light-gated like any ship. All recipes/durations are tunable consts. Client **Build · develop** panel: each option with commodity icons, recipe cost, build time, and **affordability gating**; an in-progress **"Building X — ETA Ns"**; the production readout shows actual output (`richness × 1.5^tier`) with an **Extractor ×N** tag. 67 sim + 24 server tests; **verified live** (Build Convoy → 35 Ore debited → owner timeline → convoy spawns 12 s later; Extractor → tier up → measured ~1.5× accrual; tier persists across reconnect). |
| **Resource SINK 2 — fleets BURN FUEL to move** *(branch `async-automation`)* | ✅ **Complete** | Travian-style movement/upkeep sink (§step1 part 2): **dispatching a fleet draws Fuel ∝ distance × fleet mass** from the owner's systems — so Fuel becomes a **strategic operating resource** (hold fuel-bearing systems; ship fuel to forward depots). The charge is **atomic**: it draws from the owner's system **nearest the dispatch origin that can cover the full cost** (`(distance, id)` tiebreak → deterministic). **Soft shortfall LIMITS, never destroys** — a fuelless op simply **doesn't dispatch** (the ship/order/goods are never lost), keeping the game **async-fair**: an offline, fuel-poor fleet **idles** rather than breaks. Charged at **MoveShip + CommitRaid** (hold + notify on shortfall), **ShipProduction** (retains Fuel as the reserve, charges per non-fuel convoy, refunds held goods), and **standing-order** dispatch (**exempts Fuel hauls** — else a fuel-starved depot deadlocks — refunds + retries silently). **Exempt**: RecallRaid, patrol/autonomous-defense, market deliveries (never strand a fleet or block defense). The **home system is seeded with a Fuel reserve** on join (turn-one runway; the home produces no fuel). **Fog-safe**: the `FuelShortfall` notice is **owner-only**; `WalletView.fuel_total` sums **only owned systems**. Fuel lives in the existing per-system stockpile → **persists in snapshots for free**. Client **HUD "Fuel"** readout; held-op warnings surface via the existing light-gated check-in timeline. 75 sim + 24 server tests; **verified live** (a haul burned 300→258 Fuel, HUD tracked it; draining the reserve **held** the next moves with three "fleet move held — out of fuel" warnings while the convoy **survived intact**). |
| **Resource icons (Stellar Charters art)** *(branch `async-automation`)* | ✅ **Complete** | The Stellar Charters resource/action icons are bundled into `client/public/icons/` (so Vite serves them at `/icons/*` in **dev and in the `dist/` build** the Rust server ships) and wired into the UI: each commodity now shows its icon in the **Market board**, the **System view** (geology/deposits + owner-only stockpile/production readouts), and the **standing-orders** rule list; the **Claim** button uses the `action-claim` icon. **Credits carry no icon** — they render as the text label **`Cr`** (wallet, prices, claim cost, composer total). Icon→commodity map: **Provisions → `resource-food`** (good match), **Fuel → `resource-fuel`** (dedicated ✓), **Alloys → `resource-alloys`** (dedicated ✓), **Ore → `resource-metals`** (*stand-in*), **Volatiles → `resource-ice`** (*stand-in*). Big source PNGs were downscaled to 96 px (~290 KB for 14 icons). **Remaining gaps to generate later:** dedicated **Ore** and **Volatiles** icons (currently metals/ice stand-ins — the deployed site's `resource-ore.png`/`resource-volatiles.png` are SPA-fallback HTML, not real art); plus Syndicates-specific concepts no Charters icon covers — **convoy/ship, raider, sensor, hub/market, recall, standing-order/automation** (today shown via line-SVG glyphs). Bundled-but-not-yet-wired: action `escort`/`patrol`/`interdict`/`survey` (→ doctrine/scouting) and status `raid-risk`/`distress`/`unrest`/`charter-lapse` (→ threat/attention) — available for a later pass. Client/UX only; verified live in dev + built dist (icons render crisply in Market/System/standing-orders; `Cr` everywhere; no 404s). |
| **Ship details panel (fog-aware)** *(branch `async-automation`)* | ✅ **Complete** | Selecting a ship now opens a **right-docked details card** (the same panel kit/aesthetic as the Star System view; it **shares the dock slot with the rail** — a ship and a system are never both selected, which also fixed the old "selection never cleared" bug). It is a strict **UI layer over `GhostView`** — it shows only what the per-player view reveals. **Information AGE** is the headline ("seen X.Xs ago", re-rendered each View). **Positional certainty is honest to the game's core conceit:** uncertainty is read from `GhostView.uncertainty` (= age × max_speed) for **own AND rival** ships — there is **no FTL tether to your own fleet**, so a *distant own ship is as uncertain as a rival* (a ship at your command center reads "confirmed"; a far one reads "±N su"). ⚠️ This deliberately **diverges from the task prompt**, which asked to show own ships as "confirmed / zero uncertainty" — that contradicts `view.rs`/`GhostView` (own is never a certainty grant), so honesty/fog-safety won. **Own ship:** kind + heading + activity (inferred from your own move/raid/command-signal overlays + route/vel — there is no server order field), and — for a convoy — **cargo + route** (you always know your own), plus a **Recall** action (raiders) and a move hint. **Rival ship:** only what's observable — a convoy's **broadcast route** (light-delayed) and its **cargo ONLY when in sensor range** (else "unknown — out of sensor range", **never leaked**); a raider is a **dark contact** (no route/cargo); **no rival fuel/orders/intent ever**. Rival ships are now **selectable to inspect** (clicking a rival with no own ship selected opens its panel; with an own ship selected it still commits a **raid**). **Fuel indicator moved** off the top navbar into the own-ship panel, framed honestly as the **shared fleet operating reserve** (`wallet.fuel_total`, owner-only) with this ship's burn-rate (mirrors the sim fuel model) — not a per-ship tank (a genuine per-ship fuel mechanic is noted as a future sim deepening). Client/UX only (no protocol/sim change). **Verified live across all four cases:** own convoy (cargo/route/fuel + honest ±uncertainty), own raider (Recall + ±1,780 su, no false "confirmed"), rival convoy out-of-sensor (route shown, **cargo "unknown"**), rival raider (dark contact) — with no leak of rival cargo/fuel/orders. |
| **Custom art set (Stellar Syndicates assets)** *(branch `async-automation`)* | ✅ **Complete** | The cohesive custom art set (manifest in `client/public/art/`) is wired across the game — **celestial sprites, ship sprites, a full UI icon set, and lore illustrations** — all transparent, in the dark-graphite / cyan-teal / red-threat / gold aesthetic. Map sprites downscaled 1024→256 px (lore 1280 px) to keep the bundle ~9 MB / GPU textures small; loaded from `/art/*` (Vite-bundled in **dev AND the production `dist/` the Rust server serves** — verified `200`/correct content-type for every category). **① Celestial bodies:** star systems render the **habitable-planet** sprite, the wormhole hub the **mining-station** sprite (pooled under the data cues, so the value-glow / ownership halo+ring / selection / label / dimmed-unclaimed all still read; hit-testing unchanged → systems stay selectable). *Unused (noted): sun, asteroid_a–d — no galaxy-map home.* **② Ships:** convoy = **cargo_freighter**, raider = **raider_attack_ship**, top-down, **rotated to heading** (atan2(vel)+90°), **tinted by ownership (own cyan / rival red)**, convoy **larger** than the raider (size asymmetry); all per-ship cues preserved (selection ring, uncertainty cone, pulsing raider-threat ring, gold cargo label, staleness fade). *Unused (noted): colony / corvette / scout ships — no such ship kinds yet.* **③ UI icon set** (full-color SVG, supersedes the old Charters borrow; old `/icons/` removed): **Resources** — Fuel→`resource-fuel`, Ore→`resource-metals`, Provisions→`resource-supplies`, Alloys→`resource-industrials` (its purple even matches the alloys map tint); **Volatiles has NO native icon → reuses Fuel with a cold hue-shift** (`.cicon--cold`), the one resource still wanting dedicated art; **Credits stay the text label `Cr`**. **Actions** (claim/build/load-cargo/standing-order/recall/move/raid), **Concepts** (market-exchange on the navbar + action, fleet/convoy on the ship-panel header, command-center/sensor/uncertainty in the legend), **Status** (success/threat/info by severity in the check-in). *Unused (available later): resource-credits, concept-lightspeed-signal, concept-alliance, status-in-transit, action-survey-scout.* **④ Lore:** the **corporate-command-center** scene is the title/join-screen background (darkened wash, card stays readable); the other 5 scenes are bundled for later. Client/UX only — a visual layer over existing data; **fog model intact** (sprite tint comes from the existing `own` flag; rival cargo still sensor-gated; nothing new leaks). tsc + build clean; verified live (own cyan + rival red ships, planet/station bodies with cues, every panel's icons, the lore title screen) and that the **dist build serves all art**. |
| **Buildings step 1 — DEPOT + SHIPYARD + development SLOTS** *(branch `async-automation`)* | ✅ **Complete** | Grows the one-building economy (Extractor only) into a real Travian-style **"what do I build?"** decision: **income (Extractor) vs capacity (Depot) vs military industry (Shipyard), inside a scarce slot budget** that forces systems to SPECIALIZE. **① Development SLOTS:** each system holds `3 + (deposits−1)` slots (cap 5; home = 4) — **derived from public geology** (deterministic, migration-free, tunable consts `DEV_SLOTS_BASE/MAX`); every BUILT development tier consumes one (in-progress upgrade jobs hold theirs; **ships are units — never slot-gated**); a slot-full system **soft-rejects** (no debit, no job, owner-only `BuildRejected/NoSlot` notice). **② DEPOT (storage caps):** every system's stockpile has a TOTAL cap = **500 base + 400/Depot tier** (`Depot = Ore 45 / 15 s`); a full system's production **IDLES at the cap** (accrual stops, resumes when goods ship out — reserves aren't wasted); the home fuel seed (300) fits under the base cap; **over-cap stockpiles are grandfathered** (cap blocks NEW inflow only — nothing is ever destroyed); **inbound-delivery rule:** deliver up to headroom, the SAME convoy carries any excess onward to **sell at the hub** (sub-light, raidable; `TradeEvent::StorageOverflow` — chosen over leave-it-undelivered because an automatic sale can't deadlock or strand cargo). **③ SHIPYARD (industrial geography):** ships build only at a Shipyard system — **Convoy needs tier ≥ 1, Raider ≥ 2** (`required_shipyard_tier`); recipe **Ore 50 + Alloys 10 / 20 s** per tier, so expanding military industry needs FRONTIER alloys shipped in; **home bootstrap:** every home generates with **Shipyard 1 pre-built** (consumes a slot; re-asserted on join for old snapshots) → convoys build turn one, raiders are EARNED. **Fog-safe:** all new per-system detail (depot/shipyard tiers, slots used/total, cap + fill, rejection notices) is **OWNER-ONLY** in the View exactly like `extractor_tier` — rivals see 0/0 (fog test extended). **Async-fair:** every rejection is SOFT (recipe never eaten; production never lost). Client: Build panel gates options ("slots full" / "requires Shipyard 2"), stat strip shows **Stock X / CAP** (fill bar + "storage full — production idling" attention item) + **Slots U/T**, and a developments strip (Extractor ×N · Depot ×N · Shipyard ×N). 85 sim + 24 server tests (slot exhaustion; cap idles + resumes; grandfathering; overflow re-route; convoy@1/raider@2 gating; home bootstrap; serde round-trip); verified live. |
| **Buildings step 2 — DEFENSE PLATFORM + SENSOR ARRAY** *(branch `async-automation`)* | ✅ **Complete** | The military/intel building axis on top of Step 1: two new answers with different VERBS — **SEE and DEFEND** — so a system can specialize as watchtower, fortress, industrial hub, or extraction colony. **① SENSOR ARRAY** (`Ore 40 + Alloys 15 / 18 s`, a dev slot per tier): an owned system projects a **standing sensor bubble for its OWNER** — radius `2200 + 880·(tier−1)` (tier 1 = a ship's bubble; tier 2 outsees any ship; tunables in `build.rs`). **One coverage source of truth** (`World::array_sensor_sources`) feeds every consumer: the View's sensor gate (dark-raider detection + rival-cargo reveal now happen at array range — `view_for_with_arrays`, coverage as per-source `(center, radius)` pairs), picket sensing (a threat beyond the picket's own bubble engages if an owned array covers it; escort ward choice stays proximity-based), and the client's coverage rendering (array bubbles in the same teal idiom). **Fog:** the array's existence/tier is **owner-only** like every tier; what it reveals flows through the existing sensor gate — vision for the owner only, zero new leak (leak-check test). **② DEFENSE PLATFORM** (`Ore 55 + Alloys 20 / 22 s` — the priciest development; fortification is an investment): within a **1300 su protection radius** (~60% of a bubble), a hostile raider making CONTACT with one of the owner's convoys must fight **THROUGH the platform first** — tier = stationary defender units, resolved as **sequential seeded duels on the existing raider-vs-raider table** (unit lost → **platform loses a tier** [damage — the slot frees up; the system is never destroyed]; raider killed/mutual → raid stopped; stand-off → raider **driven off**; defeating every unit fights through to the normal convoy battle). **STANDING defense** — works with the owner offline; the platform "senses" exactly its own radius (the contact is physically inside it — deterministic, fog-clean); nearest covering system engages (`(distance, id)` tiebreak, one platform per contact; convoys of the platform's owner only). **Deterrence surfaces the hard way:** a stopped raid reports through the ORDINARY `RaidResolved` → both sides get standard delayed battle reports; the attacker learns only "destroyed/driven off" — the platform's existence/tier **never leaks in the View** (leak-check test; a future observable "fortified" hint is noted, not built). The defender additionally gets an owner-only, light-delayed `PlatformEngaged` timeline entry (result + tiers lost). Client: Build panel entries, **Sensor ×N · Defense ×N** in the developments strip, array bubbles + a **dashed cyan protection ring** (distinct from sensor teal) on own systems. Tests (89 sim + 25 server): array extends View coverage (same scene dark without it) + picket sensing; platform stops a raid inside the radius (convoy untouched, standard outcome reported), **nothing changes outside it**, damage matches reported tiers lost, deterministic from seed; both tiers owner-only. Verified live. |
| **Buildings step 3 — HABITAT + FUEL REFINERY (the sustain layer)** *(branch `async-automation`)* | ✅ **Complete** | Completes the building economy with STANDING CONSUMPTION and the last dead commodity's job — **every commodity now has a role: Ore/Alloys BUILD, Fuel OPERATES, Provisions SUSTAINS, Volatiles REFINES.** **① HABITAT** (`Ore 45 + Provisions 25 / 20 s`, a slot per tier — the Travian-crop analogue): each FED tier boosts the system's **TOTAL output ×1.25** (deliberately under the Extractor's 1.5; the two **stack multiplicatively** since the Habitat boosts ALL deposits incl. what Extractors multiplied) while consuming **0.15 Provisions/s per tier** from the system's OWN stockpile. **Ordering rule:** upkeep draws FIRST (before accrual), ATOMICALLY per tick (all or nothing — a shortfall never partially eats food). **UNFED = LIMIT, NEVER DESTROY** (the async-fair hard rule — no Travian starvation): a shortfall merely SUSPENDS the boost; nothing destroyed, no tier lost, recovery is automatic the tick food arrives (geology, standing order, or manual haul) — a week-offline player's colony just underperforms, fully intact. Transition-only owner notices (unfed ⇄ fed) + an UNFED attention item. **Balance sanity (real numbers):** home Provisions richness `0.45×[0.85,1.15]` → worst case 0.3825/s vs 2-tier upkeep 0.30/s — **the home feeds two tiers from a standing start** (tested from zero stored food); frontier Habitats need a raidable Provisions **supply line** (standing orders already haul any commodity system→system). **② FUEL REFINERY** (`Ore 50 + Alloys 15 / 20 s`, a slot per tier): converts stockpiled **Volatiles → Fuel** each tick at **0.5 Volatiles/s per tier × 0.8 yield** (slightly lossy so raw Volatiles trade keeps a niche) — runs LAST in the accrual pass (after upkeep + production, so it can refine fresh Volatiles), **idles dry** (soft; attention cue), and **works even at a FULL depot** (the lossy conversion shrinks the total, so the cap never strands it; a guard bounds yield ≥ 1 tunings). Forward fuel production: a refinery near your theater turns a Volatiles supply line into a fuel depot, easing the fuel-∝-distance operating cost — **tested end-to-end** (drained fuel → refine volatiles → a fleet move dispatches with no shortfall hold). **Fog:** habitat/refinery tiers + the FED/UNFED state are **OWNER-ONLY** in the View (rivals see 0/false — a rival never learns you have colonies, let alone whether they starve); leak-check tests. **Persistence:** tiers + fed state ride the snapshot (serde defaults; round-trip tested). Client: Build panel entries; Habitat ×N **FED/UNFED** badge + upkeep line + boost tag (or "Habitat UNFED") in the production readout; Refinery ×N + "converting N/s → fuel" (or idle) line; attention items. 96 sim + 25 server tests; verified live. |
| **SCOUT ship + active intel (the "go look" verb)** *(branch `async-automation`)* | ✅ **Complete** | The game's most on-identity missing verb: **spending resources to KNOW MORE.** **① `ShipKind::Scout`** — the LIGHTEST hull flying (mass 80 → a = 17.5 su/s², max speed 140 < c/2; fuel-∝-mass makes it also the cheapest per trip), **no cargo**, **negligible combat strength**: in ANY engagement it simply dies, **deterministically** (target → destroyed; would-be attacker → destroyed; no roll). **Runs DARK** like a raider (new `ShipKind::broadcasts()` single source of truth drives the View's dark gating + the destroyed-dark-ship latch — a broadcasting spy is useless); inside rival coverage it's a detected contact and **EngageAny pickets hunt scouts** (force-ratio/threat checks still count raiders only). **Sensor bubble — the point:** projects `SCOUT_SENSOR_MULT (1.5) × sensor_range` = 3300 su of **mobile vision** into the owner's shared coverage union (`ShipKind::sensor_mult()`, wired through View coverage, the retarded-frame latch, and the client's coverage draw) — sweeping rival space reveals dark raiders + convoy cargo along its path. Recipe: **Ore 20 + Fuel 8 / 8 s at Shipyard ≥ 1** — the cheap entry unit, home-buildable turn one; losses are acceptable by design. **② INTEL SNAPSHOTS:** a scout within `SCOUT_INTEL_RANGE (1300 su ≈ the platform radius — scouting a defended system is a risk)` of a **RIVAL-owned** system captures `{ defense_tier, shipyard_tier, observed_at, capture-pos }` (deliberately narrow — no stockpiles/habitat state; the raid/siege-relevant prize) into its owner's per-system intel map. **Delivery obeys light:** the snapshot is knowledge ON THE SCOUT at capture — the View + timeline withhold it until that light reaches the owner's command center ("Scout report: X — Defense ×2 · Shipyard ×1"). **It's a SNAPSHOT:** a parked scout refreshes it silently (notice re-fires only on fresh approach / changed tiers — `SCOUT_INTEL_RENOTIFY_S` 60 s anti-spam); out of range it **ages** and never auto-updates — *you know what WAS, not what IS.* **Fog discipline:** the scouted rival learns NOTHING (no "you've been scouted"; a never-detected scout leaves no trace — if caught, it's just a dark contact); intel is the viewer's own map only; leak-checked both directions. Client: `scout_utility_ship.png` (smallest sprite; pip/fade/native-zoom apply), oversized teal bubble, "SCOUT" contact label (no attack alarm), ship-panel sensors note, Build entry, and a **"Scout intel — snapshot"** block (Defense/Shipyard × age, "re-scout to refresh") on rival systems. Tests (99 sim + 28 server): builds turn one + out-accelerates a raider; dies deterministically both directions; scout bubble detects what a ship provably misses; dark outside coverage; snapshot captured/refreshed/re-noticed/ages + non-scouts never gather + scouted side empty; View withholds until light arrives, keeps `observed_at`, owner-only both directions; serde round-trip. |
| **Ship variety: CORVETTE + COLONY SHIP (+ weighted combat)** *(branch `async-automation`)* | ✅ **Complete** | Two crisp, non-overlapping roles + the strength model they need. **① WEIGHTED COMBAT (GDD §26.2 spirit):** battles are weighted-strength contests — per-kind attack/defense weights (**Raider 3/2 · Corvette 1/4 · Convoy 0/1 · Colony 0/1 · Scout 0/0** [dies if engaged] · **platform tier = def 3**), outcome row = f(ratio), anchored to PRESERVE today's outcomes exactly: raider/convoy r=3→(1,0,0)≡old RVC · raider/raider r=1.5 and raider/platform-unit r=1.0→(.35,.35,.12)≡old RVR (both even anchors force a flat band on [1,1.5]; (1.5,3) interpolates; r<1 mirrors); ONE rng draw per battle → the seeded stream is bit-identical (whole prior suite passes untouched). Doctrine force-ratio now compares weighted COMBATANT strength (raiders+corvettes), identical ratios for equal-kind fleets. **② CORVETTE** (`Ore 30 + Alloys 15 / 14 s, Shipyard ≥ 2`; mass 800, 5 su/s², max 80; **BROADCASTS** — a declared escort deters): **cannot raid** (CommitRaid is raider-only now, mirrored in the UI) and defends by **SCREENING**: every friendly corvette within **1300 su** of a raid contact on a civilian ship duels the attacker FIRST (nearest-first, deterministic; corvette losses are real ships, unlike platform tiers; each duel reports via the ordinary RaidResolved) — shadowing a convoy = **escort**, parked at an owned system = **garrison** (screens BEFORE the platform's tiers). Standing defense, owner offline; pickets' autonomous interception stays raider-only by design (a corvette defends by being THERE). **③ COLONY SHIP** (`Ore 60 + Alloys 20 + Provisions 40 / 30 s, Shipyard ≥ 1` — colonists eat; mass 6000 — the heaviest hull, 1.2 su/s², max 40; **BROADCASTS**: your expansion is telegraphed, raidable, escortable — corvette screens + platforms protect it like a convoy): **claiming is PHYSICAL** (GDD §22.1 restored). `ClaimSystem` (instant credit purchase) is **REMOVED** — to claim, build a Colony Ship and SEND it: **on arrival at a still-unclaimed, non-reserved system, ownership transfers and the ship is CONSUMED** (it became the colony; no wreck), `SystemClaimed` light-propagating exactly as before. **THE RACE:** earlier arrival tick wins; same-tick ties break by ship id (deterministic; tested twice-run-equal). **The loser HOLDS** at the spot — intact, redirectable (settles elsewhere when re-sent), ONE owner-only light-delayed `ColonyHeld` notice per hold (`notified_held`, serde). Reserved home-site systems are never settleable. Destroyed in transit = colonists lost, no claim ever lands — expansion has stakes. **MIGRATION:** `Command::ClaimSystem`/`ClientMsg::ClaimSystem` removed (commands aren't persisted — snapshots load fine; old clients' claim messages fail parse harmlessly); `claim_cost` kept on the wire but **deprecated** (charges/gates nothing; a future colony-overhead knob); the client's Claim button/cost display → a "build a Colony Ship and send it here" hint; `scripts/claims_smoke.mjs` is deprecated (it exercises the removed command). Client: both sprites wired (colony 64px — the largest; corvette 48px — between raider and convoy), rival labels ("ESCORT"; "COLONY SHIP" in gold — intel worth acting on), ship-panel role cards, Build entries with gating. Tests (106 sim + 28 server): anchor-preservation; corvette can't raid (no order/fuel); the SAME seeded raid that kills an unescorted convoy is stopped by a screen; garrison screens before PlatformEngaged; Shipyard-2 gating; colony settles on arrival (consumed, no charge, no wreck); race loser holds + one notice + redirects and settles elsewhere; same-tick id tiebreak deterministic; in-transit kill = no claim; reserved homes refuse settlement; determinism test now exercises the settle path. |
| **BATTLES TAKE TIME: config-scaled duration + observable engagements + mid-battle command** *(branch `async-automation`)* | ✅ **Complete** | Battle DURATION is now a config-scaled strategic timescale, not seconds. `Config.battle_target_secs` (playtest **45 s** · production **2700 s**) DERIVES the rate: **`dmg_rate(T) = 0.1435 / T`** (0.1435 = the empirically-measured `duration × rate` constant for equal reference forces grinding to the 50 % retreat threshold — independent of force size). Lopsided fights still end fast (concentration); a **safety valve** (`MAX_BATTLE_MULT 2×`) forces mutual disengage on a no-retreat grind. **Raids stay quick** via a FIXED `RAID_RATE` + a short cap (`RAID_CAP_FRAC 0.15 × T`) — slow battles don't slow raids. Combat is now a **persistent, observable `Engagement` entity** (pooled multi-fleet sides, per-side damage pools) — light-gated in the View (a third observer sees "battle raging — as of N ago" only by their own light) with **weapons-fire reveal** of ALL participants (even dark fleets) at the site. **Battles ANCHOR** (§engagement movement): on contact both sides drop to ~zero velocity — a stationary event that suspends prior missions (pinning a slow hammer while relief travels; survivors resume their course after). Doctrine evaluates **immediately on contact** — a fleet on **Avoid** that gets jumped takes a brief `DISENGAGE_EXPOSURE_SECS` parting-shot scrape then the **speed table** decides escape (a raider outruns corvettes; a colony outruns nothing) — no coast-lock, no fly-through; only fleets that ACCEPT battle stay anchored. Three coarse **light-delayed mid-battle verbs**: **Withdraw** (physical disengage at formation speed — the speed table decides escape; wired to the order-lifecycle echo), **Reinforce** (a friendly fleet arriving joins its side's pool, shifting the ratio), **Change doctrine**. Defender home-field advantage falls out of the physics (shorter command delay near your CC) — intended. Client: pulsing battle marker, "battle raging" digest, Withdraw button on an engaged fleet, doctrine usable mid-fight. Tests: duration ≈ target (both presets), lopsided-faster, raid cap, safety-valve, light-delayed withdraw, reinforce-joins-and-flips, weapons-fire reveal leak-check, persistence mid-battle. |
| **ORDER LIFECYCLE indicator: IN TRANSIT → AWAITING ECHO → CONFIRMED** *(branch `async-automation`)* | ✅ **Complete** | Surfaces where each own order is in its light-delayed round trip. The sim already knows delivery (`apply_time`); it now also computes **`echo_at` = delivered_at + distance(delivery point → command center)/c** (analytic under §14.1 constant velocity) and exposes both, **owner-only**, per pending order (`World::pending_commands`, latest-per-fleet). New owner-only events `OrderDelivered`/`OrderConfirmed` (confirm fires exactly at `echo_at`; a fleet destroyed first drops silently — no phantom confirm) feed the check-in timeline. Server adds `View.pending_orders` (owner-only). Client: fleet-panel status line with **live countdowns** (ticks client-side from the two stamps — no per-second traffic); the MAP now distinguishes the two pending phases with the panel's ◈/◔ vocabulary at the SAME boundaries: **phase 1 IN TRANSIT** (before `delivered_at`) = a **◈ hollow-diamond badge** (the signal motif) + **sparser, dimmer dashes** (3px/6px, α 0.35 — pure intention, the fleet doesn't know yet); **phase 2 AWAITING ECHO** (before `echo_at`) = a **◔ quarter-filled clock badge** + **tighter, brighter dashes** (5px/3px, α 0.55 — executing, unconfirmed); then **SOLID** (α 0.3) + no badge at echo (observed). Same size/position/own-cyan; a second-read step, not a new color. Edge cases: superseding restarts to the latest; near-zero (fleet at the CC) suppressed (the map shares the panel's 1.5s threshold — no sub-second glyph flicker). Confirmation trigger: `now ≥ echo_at`. Verified live on a cross-map order: ◈+sparse → ◔+tight flipping exactly as the transit countdown crossed 0 → solid+no badge at echo, panel and map phases agreeing at every sample. Tests: delivery/echo timestamps match analytic; delivered→confirmed at echo; supersede-latest; destroyed-no-false-confirm; owner-only leak. |
| **SPEED-SIGNATURE DETECTION: throttle + four-factor visibility** *(branch `async-automation`)* | ✅ **Complete** | Replaces binary dark-ship detection with **`distance ≤ sensor_capability × signature`** — ONE shared function (`detection::detected`/`signature`) for both the View's gating and the sim's picket sensing (parity-tested), evaluated from the **retarded** sample velocity (sprint-then-coast caught by its old flare). `signature = size_mult(√-aggregated SIG_SIZE table) × speed_mult(quiet at stealth → 1.0 at full, ratio SPEED_SIG_MAX 2.5) × cloak_mult(STUB 1.0)`; `sensor_capability = range × SENSOR_TECH_MULT(STUB 1.0)`. **Anchor: a single raider at full speed = 1.0**, so the sim's detection is byte-preserved. **Transit throttle** on fleets: `Full` (default) or `Stealth` (× STEALTH_FRACTION 0.5, ~2× trip). Dark fleets only; broadcasters keep the bucket ladder. Client: loud contacts get a flare/plume (distinct from the threat ring); fleet-panel Full/Stealth toggle + rival signature readout. `GhostView.signature`, `SetFleetTransit` command. Tests: anchor exactly 1.0, stubs provable no-ops, √-aggregation ordering, full-vs-stealth same path, retarded sprint-then-coast, View/sim parity, transit persistence. |
| **KINEMATICS: constant per-kind speeds (acceleration removed)** *(branch `async-automation`)* | ✅ **Complete** | Playtest retired flip-and-burn (invisible at async cadence; `t = 2√(d/a)` defeats prediction math). Restores GDD §14.1: **constant-velocity, piecewise-linear movement** at a per-kind `speed()` — Scout 115 · Raider 100 · Corvette 65 · Convoy 40 · Colony 33 (old max-speed ordering; calibrated to an 8000 su convoy trip: old ≈199 s, new 200 s). `movement.rs` `flip_and_burn` → `advance_toward` (constant velocity, stop on arrival); pursuit is now **analytic lead** against a constant-velocity target (`intercept_point`, closed-form). Removed `thrust`/`accel`; fuel-∝-mass, uncertainty=age×speed, dark/broadcast all unchanged. Cargo no longer slows a ship (it costs fuel, not time). Fleet formation speed = min member speed. Tests: travel-time `t=d/v`, analytic intercept correctness, constant-speed cap, lead-pursuit contact; 3 timing-sensitive suite tests re-tuned. |
| **FLEETS Part 3/3: STALE-INTEL battle calculator** *(branch `async-automation`)* | ✅ **Complete** | At raid-commit time, a **projected engagement estimate** computed by running the SAME shared Lanchester attrition (`project_engagement`) forward on the observer's OWN view data — your fleet exact; the target's **exact composition in sensor coverage**, else a **typical warfleet of the bucket midpoint** ("assuming typical hulls"); a platform from your aging **scout snapshot** if one covers it, else unknown. Output: projected per-kind losses both sides **plus the age of every input** ("their composition: 12s old · defenses: scouted 4m ago"). A read-only `EstimateEngagement` query — it MUST call the shared combat fn (no drift) and MUST NOT touch authoritative state. **Leak-checked:** a true 25-ship fleet out of coverage is provably modelled as ≤ its bucket midpoint (23), never the true count. Server computes it from the view filter; a small commit-time client panel renders it. Tests: +3 (exact-in-coverage, bucket-midpoint-out-of-coverage leak check, no-mutation). clippy + tsc clean. |
| **FLEETS Part 2/3: LANCHESTER combat (proportional casualties)** *(branch `async-automation`)* | ✅ **Complete** | Replaces the all-or-nothing seeded outcome table with **deterministic per-tick attrition**: two pooled sides deal `DMG_RATE × attack power` per tick, spread across enemy kinds by `count × hull` share into per-kind **damage pools**; ships die whole when a pool fills a hull (remainder carries). **Hull table** Convoy/Colony 10 · Raider 20 · Corvette 40 · Scout 2 (dies if engaged) · platform tier 30; `DMG_RATE 0.1`, raid skirmish ×0.3. **Concentration proven numerically:** 20 vs 10 → ~18 survivors (√(20²−10²)); 20 vs two sequential 10s → 14. **Retreat** now triggers on fraction-of-own-strength-lost (survivors flee); **mid-battle relief flips outcomes** (tested). **Raid vs battle asymmetry** (skirmish rate + cargo seizure vs decisive full-rate defense-of-place); **platform tiers** attrit into their own pool (ram behavior preserved). Battle **reports** now carry **composition-vs-composition per-kind losses**. One shared pure combat fn (`attrition_tick`/`project_engagement`) — the sim and the Part-3 calculator both call it (no drift). Engagement is stateless except the persisted pools, so a **mid-fight snapshot resumes**. Tests: +14 (concentration proof, proportional two-sided losses, retreat-at-fraction, relief-flips-outcome, raid asymmetry, platform pool↔tier, per-kind report, persistence round-trip mid-engagement). clippy + tsc clean. |
| **FLEETS: multi-ship entities + intel-ladder fog (Part 1/3)** *(branch `async-automation`)* | ✅ **Complete (behavior-preserving)** | The map/sim unit is now a **`Fleet` of N ships (mixed composition)** — GDD §13.1 — replacing the single ship, with a world of fleets-of-one behaving **exactly** as before (all prior tests pass in fleet vocabulary; every persisted ship migrates to a fleet of 1). **FORMATION physics (§14.2):** a fleet moves at its **slowest member's pace** — accel = `min_kind(thrust/hull) × hull/(hull+cargo)`, cruise = `min max_speed`; total mass = `Σ hull×count + cargo`, so fuel-∝-distance×mass is unchanged (a hammer carrying a colony ship crawls). **BROADCAST if ANY member broadcasts** — you can't hide a freighter behind a raider; only all-raider/scout fleets run dark. **The two-tier INTEL LADDER (the key new fog gate):** every visible fleet carries a **`count_class`** — an estimated-size BUCKET (`1 · 2–3 · 4–7 · 8–15 · 16–30 · 31+`, never an exact N, so it can't be inverted); the **exact `composition`** (kinds + counts) is revealed ONLY inside sensor coverage (or for your own fleets), exactly like cargo. You know a hammer is inbound and roughly how big long before you learn what's IN it. **Management v1:** `MergeFleets` / `SplitFleet` at an owned system, build-join-or-new-fleet; **colony-claim consumes ONE colony** and the escort persists; orders (move/intercept/colony/scout) are fleet-level. **Combat is UNCHANGED here** (each fleet fights as its flagship — Part 2 makes it Lanchester-proportional). **Migration:** snapshot entity table `ships`→`fleets` + per-entity `composition` back-fill (`migrate_world_json`), **protocol bumped to v2** (`GhostView` gains `count_class` + `composition`). Client: one sprite per fleet (flagship by precedence colony>convoy>corvette>raider>scout) + a **count badge** (exact Σ own/in-coverage, bucket label outside), fleet panel mirroring the ladder, merge/split controls. Tests (120 sim + 35 server): formation-slowest + mass/fuel sums; composition/count gating leaks BOTH directions; merge/split determinism + soft-rejects; build-join; colony-consumes-one; migration round-trip. See the **Fleets** section below. |
| **Planet art in the System View** *(branch `async-automation`)* | ✅ **Complete** | The System View's procedurally-drawn planet/moon/belt circles are replaced with the generated PLANET ART — a pure presentation swap inside the presentation-only view (generator, deposit→kind mapping, orbits, fog: all untouched). **Assets:** `client/public/art/celestial_sprites/planets/` — one icon per `PlanetKind`, filenames matching the kinds EXACTLY (`terrestrial, desert, ocean, ice, gas_giant, lava, barren` → 1:1, **no kind left on fallback**), plus `moon.png` and `asteroid_belt_chunk.png`; originals (1254px RGB, WHITE background, **no alpha**) backed up to the art source dir and processed in-repo: border **flood-fill background removal** (keys the white surround while preserving white clouds *inside* planet rims), 1px erode + feather, downscaled to **256px real-alpha RGBA** (corner α=0, center α=255; ~14–88 KB each), matching the star/ship treatment. Measured visible extents drive exact sprite scaling (planets fill ~0.79 of canvas, moon 0.31, chunk 0.43) so each sprite renders at precisely the radius its circle used — gas giants stay visibly larger (`radiusForKind` untouched). **Wiring** (`systemview.ts`): textures load lazily (the established `loadArt` idiom); the `KIND_META` tint circle remains the not-yet-loaded fallback (with its sunlit-highlight fakery — the art is already shaded, so overlays on sprites are only the habitable halo + deposit-association pip, which draw on top in either mode); the scene rebuilds once if art lands after first entry, cached thereafter. **Moons** use the moon icon at the same tiny radii; **belts** keep the existing dust-dot ring (fine grit) and add **22 chunk sprites per belt** from an INDEPENDENT seeded stream (`hashId(systemId+"chunks"+radius)`) with jittered angle/radius/rotation/scale — the dots' determinism is untouched and the chunks are stable per system. Selection/labels/hit areas/deposit badges unchanged. **Details panel** shows the selected body's icon as a 96px thumbnail beside the kind/description/deposit block (color swatch kept as the no-art fallback). Manifest updated with the 9 entries. Verified live: per-kind art with correct sizes, moon icons, chunk-dressed belt, panel thumbnail, real alpha over the dark scene; loads in dev + the built dist; tsc/build clean. |
| **Wormhole Hub art (landmark sprite + selection portrait)** *(branch `async-automation`)* | ✅ **Complete** | The game's most important location now reads as a LANDMARK. **Assets** (`client/public/art/`): `wormhole_hub.png` = the transparent MAP SPRITE (verified real alpha: corner α=0, subject fills ~0.93 of canvas; downscaled 1254→512 RGBA, originals in git history + the art source dir) · `wormhole_hub_concept.png` = the CONCEPT PORTRAIT (opaque dark-bg key art, downscaled 1672×941→640×360). **① Map:** the hub's body swaps from the mining-station sprite to the wormhole landmark at a tunable **`HUB_PX = 72`** marker — clearly the largest body on the map (stars top out at 46px), with the gold+violet identity readable at marker size; the mining-station sprite remains the load fallback, and the teal glow + "HUB" label stay. *Sizing path: the max-zoom size hierarchy for BODIES hasn't landed (the two-phase curve covers ships only) — noted in code for the future monumental (~800px) deep-zoom treatment.* **② Selection:** clicking the landmark opens a **hub detail panel** (planet-panel idiom, violet-striped): the concept portrait as the header image, "Wormhole Hub — the neutral trade station at the wormhole to Sol" blurb, and a working **Open Market** button (same action as the navbar/M — the hub IS the market); Esc closes. Hit-tested AFTER ships/rivals, so fleets parked at the hub stay individually selectable/raid-targetable, and before the empty-space move order. *(The optional Market-panel header image was skipped — it cluttered the board.)* Client-only; the hub is public geography (nothing to fog-gate). Verified live: landmark presence at fit zoom, panel + portrait + Open Market working, both images serving in dev and shipping in dist; tsc/build clean. |
| **One engagement = ONE battle icon (suppress engaged fleet markers)** *(branch `async-automation`)* | ✅ **Complete** | A battle now renders as a SINGLE "battle in progress" icon at its anchor — the participants are represented BY the icon, not a cluster of overlapping sprites beside it. **Suppression rule:** while a fleet is engaged in a battle VISIBLE to the viewer, its whole galaxy-map marker is hidden — the formation sprite, heading hint, uncertainty cone, ownership pip, count badge, and echo badge (one `container.visible = false`); it stays pooled so it restores instantly when the fleet breaks away. **Per the observer's light:** `state.battles` is already light-gated (weapons-fire site-reveal), so a distant observer whose retarded view still shows pre-battle fleets sees them converge normally, and the collapse-into-one-icon happens when the battle's light arrives — no new fog rules. **One entity, one icon:** the icon is keyed by the engagement's stable id, so merging reinforcements (same id) stay one icon; two genuinely separate engagements near each other fan out slightly to stay two. **Own-involvement pip:** one cyan diamond on the icon's edge when the viewer has forces in it ("my fight" at a glance), regardless of how many of their fleets are in; no rival pips beyond the existing site-reveal. Screen-space sized (no deep-zoom growth), pulsing, sane click radius. **Selection & panel:** clicking the icon opens the live battle panel — location, observed "raging for" elapsed (per the viewer's light), participants as known to the viewer (own fleets: composition + a **Withdraw** verb each with its echo countdown; rivals: only the site-revealed kind/bucket), plus **Reinforce** guidance and a **Change doctrine** link. Own engaged fleets are reachable here even though their markers are gone — Withdraw is issuable without hunting for a hidden sprite. **View additions (no mechanics change):** `BattleView` now carries the engagement `id`, `started_at`, and the `participants` fleet ids — exactly the set already fed into the weapons-fire reveal (their ghosts are already sent), so no new information leaks; leak covered by the existing weapons-fire reveal test + a new `active_battles` participant-set test. Aftermath markers unchanged. Verified live: a 2-fleet fight's markers collapse to one pulsing icon with the own pip; the panel opens with per-fleet Withdraw + echo countdowns + doctrine/reinforce; a fleet leaving the set restores its marker; two nearby battles show two offset icons. clippy + 46 server / 166 sim tests + tsc + vite build clean. |
| **Contestable territory 2/2: SIEGE → CAPTURE (colony-delivered)** *(branch `async-automation`)* | ✅ **Complete** | A strangled system can now be TAKEN — slowly, telegraphed, colony-delivered. Capture requires, in sequence: **(1) defenses suppressed** (`defense_tier == 0`, ground down through the establishment/relief battles — the platform-pool attrition IS the siege gun) AND no garrison combatant on station; **(2) an unbroken blockade for `SIEGE_DURATION`** (= `SIEGE_DURATION_BATTLE_MULT (8) × battle_target_secs` — one knob scales both: ~6 min at playtest, hours at production; any lift FULLY resets the clock; the clock also resets the moment defenses return or a garrison arrives); **(3) a COLONY SHIP delivered** while (1)+(2) hold — the colony-claim handler is extended so it FLIPS the system (one colony ship consumed as the occupation government). Arrival while conditions don't hold → the existing soft-hold/redirect (never consumed in vain): *sieges strangle; only colonists conquer.* **Flip transfer:** ownership → captor (light-propagates via `claimed_at`); developments at **half tiers rounded down** (a damaged base, freed slots); the **stockpile as plunder** (snapshotted for the report); in-progress builds **dropped** (existing payer rule); the blockade cleared. **HOME PROTECTION (hard, sim-enforced):** a home system can be blockaded but its siege clock NEVER starts (and is reset if forced) — a beaten player always keeps a producing base and their fleets; **no elimination.** **Records:** the flip emits per-participant, light-delayed `SystemCaptured` reports through the retention machinery (captor "you captured X"; old owner "you lost X" itemizing the plunder) + timeline notices + a **capture aftermath marker** (gold flag = gained, red = lost; click → results panel). **ASYNC-FAIRNESS AUDIT:** every stage is standing-defense-first (the platform + garrison fight autonomously, owner online or off), SLOW (a full `SIEGE_DURATION`), TELEGRAPHED (light-delayed under-blockade → under-siege notices + attention items, and a **broadcasting colony ship inbound is the loudest signal on the map**), and NON-ANNIHILATING (fleets survive, home never falls). **A 3-days-offline defender:** their platform + garrison auto-fight every establishment/relief battle; a lone blockader that can't suppress the defenses never starts the siege clock; even fully suppressed, the attacker must hold unbroken for the whole duration AND cross a broadcasting colony ship in — all of which the defender's autonomous defense + the multi-stage delay give a realistic check-in cadence time to break; and if they DO lose a frontier system, they keep their home and fleets and can retake it the same way. **Fog:** siege progress is in the participant-only blockade field (besieger via their fleet, owner light-delayed); capture reports per-participant, light-delayed; plunder revealed only ON the flip; leak-checked. **Persistence:** blockade + siege clock ride the snapshot (round-trip tested). Client: siege badge + live capture countdown (rail + System View), a siege progress bar, capture markers + panel. Tests: capture with half-tier + plunder transfer; each condition individually blocks (defenses up / garrison / clock not ripe / no colony); HOME never flips (all conditions met → refused); clock resets on lift; mid-siege persistence; per-participant capture-report fog. clippy + 46 server / 165 sim tests + tsc + build clean; live-verified client (blockade ring, siege countdown, capture marker + panel with plunder). |
| **Contestable territory 1/2: BLOCKADE (interdiction)** *(branch `async-automation`)* | ✅ **Complete** | Claimed rival systems can now be STRANGLED without being taken. New fleet order **`BlockadeSystem`** (client: select a raider fleet → click a rival system): the fleet must CONTAIN ≥1 raider (corvettes/scouts add strength but can't blockade alone — crisp roles), the target must be rival-owned; fuel-charged and light-delayed via the order-echo lifecycle. The fleet flies to STATION on the system and the standing defense engages it as any hostile contact — the **establishment fight is a normal anchored full-duration battle** (platform pool + garrison combatants as the defender side; Reinforce/Withdraw apply). A blockade holds only if that battle doesn't destroy or repel it (`end_battle` keeps a surviving blockader on station instead of sending it home). While ≥1 hostile blockader is on station: **outbound** dispatches (manual `ShipProduction`, standing orders) HOLD at origin and **inbound** convoys HOLD on a standoff ring (destination re-targeted, nothing destroyed); production keeps accruing, so Habitats strangle via the existing UNFED rule as their supply line is cut — emergent, not scripted. Lifts when the last on-station blockader is destroyed/repelled/withdrawn (full clock reset). **Fog-safe:** the blockade view field is surfaced to the two PARTICIPANTS only — the besieger instantly (their fleet is there), the owner light-delayed from the system; a third party sees the fight via `battles` but never the blockade badge (leak-checked both directions). **Balance:** one labeled placeholder block (`BLOCKADE_STATION_RADIUS`, `BLOCKADE_STANDOFF_RADIUS`, `SIEGE_DURATION_BATTLE_MULT`), playtest-deferred. Client: blockade badges (owner "under blockade" / besieger "blockading"), a pulsing red dashed map ring + ⛔ tag, check-in attention item, the ship button disabled with a banner while blockaded, and the raider panel's blockade hint. Tests: establishment win vs a defended system; holds outbound + inbound; lifts on blockader destruction; command requires a raider + rival target; participant-only light-gated fog. clippy + 45 server / 160 sim tests + tsc + build clean. |
| **Concluded-battle AFTERMATH markers (clickable results, per-participant)** *(branch `async-automation`)* | ✅ **Complete** | When a battle concludes, each PARTICIPANT gets a clickable marker at the site — appearing when THEIR conclusion light arrives. **① Retention (server):** the ReportScheduler now RETAINS the last **`BATTLE_REPORTS_KEPT = 20`** delivered reports per player (id · site · event time · per-player `learned_at` arrival stamp · role · flagship kinds · composition-vs-composition per-kind losses · outcome), re-sent in every View (`View.battle_reports`, owner-only by construction) so markers/results survive reconnects; the transient news toast now shares the same `report_id`. *(Plunder quantities aren't in the conclusion events today — adding them is a sim change; omitted.)* Tests: retained only after the recipient's light arrives with the exact arrival stamp; both sides retain the SAME battle id at DIFFERENT times; a non-participant retains nothing; capped FIFO at the tunable; reads stable across calls (reconnect). **② The marker (client):** screen-space UI like pips (fixed ~22px, never in the deep-zoom ramp), under the ghosts; UNVIEWED = subtle attention pulse, VIEWED = static/dim, DISMISS (in the panel) hides it while the report stays in the log — read/dismissed state persists in localStorage; **`BATTLE_MARKER_TTL_S = 1800`** hides ancient markers; co-located battles fan out; 14px click radius, hit-tested after ships/systems/hub so it never steals a gameplay click. **Click → battle results panel** (planet-panel idiom, ember-striped): outcome verdict in your terms (victory / defeat / mutual destruction / withdrawal), concluded-vs-learned times with the light delay, both sides as you learned them, per-kind losses — also opened by clicking the battle's entry in the reports log (same id). **③ Icons:** the ongoing-battle marker + aftermath marker are plumbed for `battle_in_progress.png` / `battle_aftermath.png` with the established drawn fallbacks active — **the two staged icons were not found on disk** (not in `client/public/art/`, Downloads, or the art source dir); drop them at those exact names and both light up with no code change. **Verified live, end-to-end with a real fight:** the starting raiders of two corps met and mutually destroyed each other → both participants received retained reports for the SAME battles with genuinely different `learned_at` (193.5 s vs 194.6 s per their command-center distances) while a third corp whose convoys fly through the battle region received NOTHING (live leak check); markers rendered/pulsed, panel showed the full result (losses, 0:11 light delay), log-row linkage worked, dismiss hid marker 1, and a reload restored reports from the server + read/dismissed state locally. clippy + 199 tests + tsc + build clean. |
| **Build progress: bars · ticking countdowns · done-at times (Travian-style)** *(branch `async-automation`)* | ✅ **Complete** | Replaces the static "Building X — ETA Ns" text with a full construction-queue presentation. **① Queue rows** (System View management column): per job — bundled icon + name + resulting tier (`Depot ×2`; same-key jobs ahead counted in), a **determinate progress bar** (the depot-cap Bar idiom), a **live M:SS countdown** and the **absolute local done-at** ("done 14:32" — the async-planning detail). All timing derives from `complete_time` (sim-stamp from the view) + the recipe's public `build_secs` (start = complete − total), recomputed from scratch every render — **correct across reconnects/offline by construction** (no client-accumulated time; the echo-countdown pattern). Completion: a brief **✓ resolve row** (~4s, guarded against stale-history flashes), the existing notices unchanged. **② Multiple jobs, really:** the sim always allowed concurrent builds (costs debit up front; pending upgrades count against slots) — the view collapsed them to the soonest and the panel hid itself ("one job at a time" was UI fiction). The view now sends the FULL completion-ordered queue (`SystemStateView.builds`, owner-only exactly like `build` — leak test extended: rival list provably empty + ordering asserted), and the build menu stays open during construction. **③ Hammer-on-the-plot:** while anything builds, an amber **scaffold-and-crane glyph** hangs at the job's anchor body (developments at their own anchor, ship builds at the shipyard anchor), stacking on the same rim arc as the finished markers; cleared on completion via the cached tier-signature rebuild — never per frame. **④ Galaxy rail:** the slimmed rail's attention line becomes the compact `🔧 building: <item> — M:SS` (or `building ×N — next M:SS`). Verified live: Depot + Convoy queued together → two rows with filling bars/ticking countdowns/done-at times, two scaffolds at the primary world, rail line correct, **reload mid-build resumes at the correct fill**, ✓ flash then the built marker appears. clippy + tests + tsc + build clean. |
| **Single-click everywhere (double-click bug fixed)** *(branch `async-automation`)* | ✅ **Complete** | **Diagnosis (cause #1 confirmed):** Views stream every **~100ms** (`BROADCAST_EVERY` 3 ticks @ 30Hz) and every open panel re-rendered per View — so a normal ~100ms press almost always straddled an `innerHTML` rebuild, destroying the pressed button mid-press; the browser retargets the `click` to the old/new targets' common ANCESTOR, where the delegated `closest("[data-*]")` finds nothing → the action silently never fired ("build buttons need a double click"; the unbuildable Scout). Not #2 (no select-then-act rows) and not #3 (map pointer handling is canvas-scoped). **Structural fix — the PRESS GUARD:** delegation on stable panel roots was already the codebase pattern (handles the orphaned-handler half); the guard handles the destroyed-node half: while a press is down inside a panel, that panel's re-renders are **deferred** and flushed just after the click dispatches (`pointerup` → click → `setTimeout(0)`), each panel guarded independently (map pans / other panels defer nothing). Applied to every per-View-rebuilt panel: **system tab · System View management column · ship panel · market (board/composer preview/resting orders) · standing orders · check-in**; the standing-orders ✕ also migrated from per-render listeners to root delegation (the last non-delegated rebuilt control). Doctrine selects were already change-guarded; static controls (navbar, breadcrumb, zoom, composer submit) were never affected. The app's ONLY intentional double-click — galaxy-map dbl-click to enter the System View — is preserved (single click there = select). **Verified live with Views streaming:** a 350ms held press over a build button spanned 9 ticks and the node SURVIVED (previously destroyed ~3×), flushing on release; one realistic click (press → hold across a View → release) built a Scout first time; canvas presses leave panels updating; market row select + side toggle fire on first click. tsc + vite build clean. |
| **System View = the MANAGEMENT HOME (city-screen pattern)** *(branch `async-automation`)* | ✅ **Complete** | The planet-level System View goes from pure scenery to WHERE AN OWNED SYSTEM IS RUN — by RELOCATION + VISUAL ANCHORING, not new gameplay scale. **The hard boundary holds** (guardrail comments extended): buildings stay SYSTEM-level (SYSTEM dev slots, system stockpile, same `BuildShip`/`DevelopSystem` commands); no per-planet entities/claiming/combat/movement. **① Structure markers:** each built development draws a small decorative glyph at a DETERMINISTIC anchor body (mirroring the deposit→body association): Extractor → the richest deposit's body (amber rig) · Refinery → the volatiles body (icy-moon motif) → else gas giant → else outermost (orange tanks + flare) · Habitat → the habitable world → terrestrial/ocean → primary (green dome; warn-tinted when UNFED) · Shipyard → orbital gantry at the PRIMARY (innermost) planet · Depot → orbital warehouse, also primary · Sensor → relay dish at the OUTERMOST planet · Defense → battle-station ring in close star orbit · (Interdictor: slot reserved for when it exists). ×N tier tags; several developments at one body stack around its rim in fixed dev order. Cached like the labels: rebuilt on layout + tier-signature change (build completion), never per frame; clicking a marker selects its anchor body. **② The management column** (`#sysview-manage`, right dock, panel kit): full build/develop menu (costs/ETAs/affordability + shipyard-tier & slot gates via ONE shared row renderer), **SLOTS N/M promoted to the header**, stockpile + depot-cap bar, production readout (fed/unfed), in-progress build ETA, Ship→hub / auto-supply / market actions. **Contextual sugar:** clicking a body offers the developments that ANCHOR there ("icy moon → Refinery") — same system-level build, friendlier entry. Its ONE delegated listener sits on the static panel shell (only the body's innerHTML rewrites), immune to the listener-loss class of bugs. **③ The galaxy rail slims to a SUMMARY:** header/ownership, stats strip, stockpile bar, attention cues (storage full · habitat unfed · building ETA), geology, scout intel, claim guidance (unclaimed), and a prominent **"Open System View — manage ▸"** primary action (double-click / deep-zoom enter paths unchanged); the duplicated build menu/production readout are REMOVED from the rail (one management UI). **Fog leak-checked live:** a rival's System View shows the plain Inspect button, NO management column, ZERO markers/marker hit-targets, and no develop offers on its bodies (tiers are owner-only zeros in the View; the client additionally gates on `mine`). Verified live: shipyard gantry ×1 on the primary from the bootstrap tier, ocean world offering Habitat, Extractor built from inside the view (queue + slots + marker). tsc + vite build clean. |
| **Hub at NATIVE 1254px at max zoom (blur fix)** *(branch `async-automation`)* | ✅ **Complete** | The max-zoom hub was soft because the stored 512px asset was upscaled 1.72× to the fixed 820px target. `wormhole_hub.png` is now the ORIGINAL **1254×1254** processed to real alpha at native resolution (the staged original was raw RGB-on-white; same flood-fill → feather pipeline, NO downscale; raw backed up to the art source dir) — superseding the 512px version at the same path, so no loader changes. The hub's deep-zoom target is no longer a fixed const: it is **the texture's native extent** (`HUB_ART_FILL × tex.width` ≈ 1166px visible), so the sprite-scale math lands at **exactly 1.0 at max zoom** (verified live: `spriteScale === 1`) — pixel-crisp by construction, never upscaled, like the old ship native rule. **Minification:** `autoGenerateMipmaps` enabled on the hub texture source (trilinear minification), so the 1254px source stays clean at the ~72px normal-zoom marker too (verified: exactly 72.0px visible at fit zoom, unchanged). Hierarchy intent preserved — hub ~1166px visible ≫ stars ≤ 413px visible (480px canvas) ≫ fleet markers ≤ ~120px (≈ 2.4–2.9× the biggest star, ~10× a ship). Click cap (90px) + hub-panel click + ramp seamlessness unchanged/verified. clippy + tsc + vite build clean. |
| **MAX-ZOOM size hierarchy (hub ≫ stars ≫ ships)** *(branch `async-automation`)* | ✅ **Complete** | At max zoom the map now reads with a true scale hierarchy instead of ships dwarfing stars. The ships' two-phase deep-zoom ramp is factored into ONE shared curve (`deepZoomPx`: identical below the threshold, smoothstep to a per-class target at max zoom) applied to ships AND bodies: **`HUB_MAX_PX 820`** (the monument at the top — 512px art × 0.93 fill ≈ **1.72× upscale**, reads clean) · **`STAR_MAX_PX 480`** = the icon **CANVAS** target (a uniform **1.875×** upscale of the 256px icons; targeting the canvas, not the visible disk, keeps per-type size differences — visible disks land **96px (white dwarf) … 413px (neutron)** — and avoids blowing small-disk types up 9× into mush) · **`SHIP_MAX_PX 120`** replaces the old native-256px ship target (a ship is now a small machine flying past monumental bodies; sprites stay ≤ native, downscale-crisp). **Normal zoom is pixel-identical** — the curve returns the unchanged base size through the whole normal range, and every overlay keeps its ORIGINAL radius plus only the deep-zoom growth delta (`extra`), so fit-zoom rings/labels are untouched while at max zoom the ownership rings, system label, and HOME tag ride out with the grown rim instead of drowning inside the disk. **Clicking:** body hit circles follow the rendered disk but are **capped at `BODY_HIT_CAP_PX 90`** (hub + stars), ships stay hit-tested FIRST and top out ≤ ~65px — verified live that a fleet parked ON the max-zoom home star is individually selectable dead-on while the star picks everywhere else on its disk. **Draw order** already had bodies under ghosts — verified: fleets render on TOP of the giant hub/star art. tsc + vite build clean; verified live at fit zoom (pixel-identical), the 820px hub monument (rival fleet flying over it), and the grown home star (ring/label on the rim, wing formation + badge above it). |
| **Fleet FORMATION sprites (family × tier)** *(branch `async-automation`)* | ✅ **Complete** | A fleet no longer draws as one flagship + a number — the marker itself shows a FORMATION. **Assets:** 12 sprites (`client/public/art/ship_sprites/fleet_{freighter,raider,corvette,scout}_{wing,squadron,armada}.png`) — mapping from the generation batch verified visually (freighter = bulky industrial haulers · raider = sleek arrowheads · corvette = broad armored gunships · scout = light winged interceptors; armadas show ~7 hulls vs a wing's 3); processed with the established pipeline (border flood-fill white removal → erode + feather → LANCZOS 1254→**512px real-alpha RGBA**, corner α=0; originals in the art source dir), manifest updated. **Selection is VIEWER-KNOWLEDGE, not truth** (§13.1 ladder): family = the flagship's kind (convoy→freighter etc.), tier = the **exact count when known** (own / in-coverage) else the **fog bucket** — `1 → single sprite (unchanged)` · `2–3 → wing` · `4–7 → squadron` · `8+ → armada`, the same breakpoints as the count badge so sprite and badge never disagree; **colony fleets always draw the single colony ship + badge** (no formation art). **No flagship size pop:** per-tier `TIER_SCALE` knobs (1.0) × a **measured per-sprite calibration table** (`FLEET_LEAD_CALIB` = single-sprite subject height ÷ formation lead-ship height, 0.81–1.08) size the formation canvas so the LEAD ship renders at exactly the single sprite's px at every zoom — crossing 3→4 ships swaps escorts in around an unchanged flagship (verified numerically at max zoom: single 211px vs wing lead 210px). Pip / count badge / echo clock anchor to the formation's rendered extent, and the click hit radius covers the whole formation (`fleetHitRadius`, 24px floor kept). Verified live end-to-end: merged a convoy+raider fleet through the real Merge UI → marker swapped to `fleet_freighter_wing` + exact badge "2" at both fit zoom (48px indicator) and deep zoom; tsc + vite build + all 197 workspace tests clean. |
| **Star-type art (varied stars + concept-art panel)** *(branch `async-automation`)* | ✅ **Complete** | Replaces the single sun body (all systems looked identical) with **10 varied star types** — 6 realistic (red-dwarf, yellow, white, blue-giant, red-giant, white-dwarf) + 4 exotic (neutron-star, binary, black-hole, magnetar) — each a **background-removed** transparent map icon + a concept-art portrait (`client/public/art/celestial_sprites/stars/`; icons in `icons/` downscaled 1254→256 px, concepts 640 px → ~4 MB). A **shared `stars.ts`** assigns each system a type as a **deterministic, rarity-weighted function of its id** (FNV-1a hash → `EXOTIC_FRACTION` ≈ 16 % exotic; tunable), so a system is **always the same type across reloads/sessions** and the map + panel agree. Client-only + **flavor only** — it touches no sim/mechanics (deposits/production/ownership/fog unchanged); *future idea (not built): exotic stars could later be special/hazardous systems (a sim change).* **① Map:** each system draws its star-type icon (pooled). The icons share one transparent canvas but the **visible star fills a different area/offset per type**, so the renderer uses each type's manifest **`center` + `visual_diameter`** to **centre the visible star at the system and size that visible disk** (not the canvas) to a consistent on-map diameter (20–46 px by deposit value) — so odd shapes (black-hole disk, neutron jets, binary pair) still read as one crisp, clickable marker. The star carries **no tint**, so **ownership stays unambiguously on the RING** (own cyan / rival red / unclaimed dimmed) — a blue star is never read as "owned", a red star never as "rival". Cues, labels, selection, hit-testing all preserved. **② System view:** selecting any system shows that star's **concept-art banner + type name** (e.g. "Black Hole" + an `exotic` badge) above the existing stats/geology/actions. **Fog-safe:** the star's type/art/name are observable for any system (a star is visible from afar) and derived from the **public** system id — a rival system still shows only its star (holdings/stockpile stay light-gated, "—"). *Note: the map-icon set has 10 types — the earlier `hypergiant` / `anomaly` have no icon here and are dropped; more variety = more icons + a wider table.* tsc + build clean; verified live (8 distinct types incl. exotics across 32 systems; per-type centre/size from the manifest; black-hole concept panel; assets load in dev + the served `dist`). |

**All seven milestones of the build plan are complete** — plus three additive
features layered on the core: the **signals animation** (the order's full round
trip visualized), and the **two-tier information model** (Convention broadcast +
sensor-range detection).

### What M1 delivers (verified)

- **Pure deterministic `sim` core** (`crates/sim`) — no I/O, no async, no DB. Takes
  a `World` + `Command`s, returns the next state + `Event`s. Seeded RNG, fixed
  timestep, fully unit-tested for determinism.
- **Authoritative server** (`crates/server`) — a single Tokio game-loop task owns
  the `World` and the session registry (lock-free by construction), ticking at
  **30 Hz**.
- **Multiplayer session layer from the start** — many concurrent WebSocket
  connections, each mapped to a player identity (a stable hash of the corp name,
  so reconnecting resumes the same corporation), join/leave handling, a
  per-player outbound stream. A player may hold multiple connections; a
  corporation only goes "offline" when its last connection drops.
- **Per-player broadcast** — every connection receives its *own* message stream
  from the authoritative loop (M1: a live tick + identity; from M3 this becomes
  each player's delayed/fogged view).
- **Postgres persistence off the hot path** (`sqlx`) — append-only event log +
  periodic full-world snapshots, written from a dedicated task that the game loop
  never awaits. Migrations in `crates/server/migrations`. **Falls back to an
  in-memory stub if `DATABASE_URL` is unset or unreachable**, so the server runs
  with zero database setup.
- **Pixi.js client** (`client/`) — connects, identifies as a player, and renders
  a galaxy canvas (starfield + the live authoritative tick) with a HUD showing
  corp, id, tick, sim-time, players-online, and link status. Holds no
  authoritative state and runs no game logic.

**M1 checkpoint proven:** two+ clients connect simultaneously, each gets its own
per-player stream and a live tick from the authoritative loop; joins/leaves are
handled (online count rises and falls correctly). See
[`scripts/m1_smoke.mjs`](scripts/m1_smoke.mjs).

### What M2 delivers (verified)

- **Continuous 2D galaxy in the pure core** — a central wormhole hub, seeded
  procedurally-placed star systems (area-uniform), and a ring of home anchors
  assigned to players on join. Galaxy radius scales with player count (§4).
- **Constant-velocity movement (§14.1)** — ships have position + velocity and
  travel at a **constant per-kind speed** straight to the destination, stopping on
  arrival (`t = d/v`). *(Flip-and-burn acceleration was tried and removed after
  playtest — see the KINEMATICS row above.)* Convoy (slow) vs raider (fast) is one
  parameter; all speeds stay below `c`. Unit-tested for `t=d/v`, analytic
  intercept, the constant-speed cap, and determinism.
- **Shared advancing world** — the game loop integrates movement each tick; each
  player gets a `View` of ships + anchors (M2: true positions — explicitly
  temporary until the M3 delay layer). On join a player gets a demo convoy +
  raider that patrol, so the world is visibly alive.
- **Pixi map** — renders the hub, systems (with designations), home anchors
  (own highlighted), and ships as velocity-oriented markers, smoothly
  extrapolated between server updates.

**M2 checkpoint proven:** ships move with flip-and-burn; multiple clients see the
same world advancing with identical positions. See
[`scripts/m2_smoke.mjs`](scripts/m2_smoke.mjs).

### What M3 delivers (verified) — the core

- **Per-player lightspeed view filter** (`crates/server/src/view.rs`, a
  first-class component): keeps every ship's recent true-position history and,
  for each player, reconstructs what the light reaching THEIR command center
  shows — every object at its *retarded* position (where it was when the
  arriving light left it).
- **The fairness guarantee, made exact.** A sample `(t, p)` is observable at a
  command center `cc` iff `t + |p − cc|/c ≤ now`. Because ships move slower than
  `c`, `arrival(t)` is strictly increasing, so the filter shows the unique latest
  observable sample and nothing fresher — provably no leak. Verified by unit
  tests *and* a wire-level smoke test that checks every ghost's staleness equals
  its light-distance, plus an **adversarial multi-agent review** that hunted for
  leaks. That review found two presence leaks (anchor-ownership and a global
  player-count revealed instantly); **both are fixed** — anchor ownership is now
  light-gated, and presence/ops state moved to a separate `/status` meta endpoint
  outside the game view.
- **One fog law for ALL ships (§6):** certainty tracks **proximity to the
  command center, not ownership** — there is no FTL tether to your own fleet.
  Every ship (own or rival) is shown at its stale, light-delayed position with an
  **uncertainty cone** (`age · max_speed` — how far it could have moved since the
  light left) and an age label, fading with staleness. An own ship near the
  command center is fresh and near-certain; the *same* own ship far out is as
  fogged as a rival at that distance. (Own ships under orders also get a hint of
  where they've likely advanced along the commanded course, so the fog reads as
  "proceeding on last orders," not a lost ship.)
- **Command latency / the three clocks (§6):** a move order travels to the ship
  at light speed (scheduled in the pure core), and the player learns the result
  later still via their delayed view. The client shows the estimate from its
  stale sighting — you command on old intel, and the real delay differs.
- **Each player sees a genuinely different delayed galaxy.** Distant things are
  stale; nearer things fresher; rivals are dark until their light arrives.

**M3 checkpoint proven:** two players each see their own delayed/fogged view;
staleness equals light-distance on the wire; commands lag; no information
(positions, presence, or counts) leaks between players' horizons. Own ships obey
the same law — `uncertainty = age · max_speed`, certainty by proximity not
ownership — verified on the wire by
[`scripts/own_fog_check.mjs`](scripts/own_fog_check.mjs). See
[`scripts/m3_smoke.mjs`](scripts/m3_smoke.mjs).

### What M4 delivers (verified) — player-vs-player raiding

- **Intercept-commit (§8):** a player commits a raider to a target; the raider
  pursues autonomously (`movement::intercept_step` solves the lead point) — no
  real-time piloting. The commit is a novel command to a mobile asset, so it
  travels at light speed: the raider begins pursuing only once the order reaches
  it, and it chases the target's *true* position, not the stale ghost the player
  committed on.
- **Randomized resolution in true space:** contact within `CONTACT_RADIUS` rolls
  a **battle** (not an auto-kill) — convoy destroyed, raider destroyed, both
  destroyed, or both survive (driven off). A convoy reaching the hub
  (`HUB_SAFE_RADIUS`) still escapes before contact. **Raiders can now intercept
  rival raiders too** (same commit/contact machinery), with their own even-odds
  battle table. All rolls use the **seeded sim `Rng`** (`crates/sim/src/rng.rs`)
  — one roll per battle, reproducible from seed + commands, no `std` rand.
- **Delayed reports on each player's own clock (§14):** a per-player *event*
  scheduler (`crates/server/src/reports.rs`) holds each battle outcome until its
  light reaches that player's command center, so **attacker and defender learn
  it at different times** — verified on the wire (e.g. attacker 19s stale,
  defender 8s, each equal to its own light-distance).
- **Destruction observed through each player's delayed frame (§6):** a battle
  resolves ONCE in true space with ONE outcome; both players observe that *same*
  fixed result, each delayed by light — never a per-viewer re-roll. A destroyed
  ship does **not** blink out: each player keeps seeing it as a light-delayed
  **ghost flying on old light** until the destruction's light reaches *their*
  command center (`T + |P − CC| / c`), then it vanishes. The view filter
  (`crates/server/src/view.rs`, `mark_destroyed` + the per-viewer gate) enforces
  this, so attacker and defender watch the *same* ship die at *different* times.
  Because a raider is only shown inside the viewer's *sensor coverage*, a
  destroyed raider's detection is latched to its **own retarded frame**
  (`detected_at_retarded_time`): the winner breaking off home can't pull its
  sensor bubble off the kill and make the dead raider blink out before its
  destruction light arrives — without ever revealing a raider the viewer never
  tracked. (Convoys broadcast galaxy-wide, so they were always correct; the
  raider sensor-gated path is the subtle case, covered by four RVR view tests.)
- **Recall can miss the window:** a recall is light-delayed too; if the raider
  has already made contact, you are "commanding into the past" (deterministic
  sim tests cover intercept, successful recall, and recall-too-late).
- **Client:** select your raider, click a rival ghost to raid it, press **R** to
  recall; delayed battle reports surface as a news log phrased per outcome and
  role ("your convoy was destroyed by a rival raider — delayed news, 25s old").

**M4 checkpoint proven:** A raids B's convoy under honest delay; the battle has
ONE seeded outcome both players observe on their own clocks; a destroyed ship
lingers as a ghost per-viewer until its light arrives (attacker and defender
see it vanish at different times); recall can miss. See
[`scripts/m4_smoke.mjs`](scripts/m4_smoke.mjs) and the two-player battle
observer [`scripts/battle_smoke.mjs`](scripts/battle_smoke.mjs) (+ sim battle
tests and `view::tests::destroyed_ship_vanishes_per_viewer_by_light`).

### Signals animation (additive — visualizing the communication delay)

Two traveling signals make the lightspeed delay legible, as **client-side
feedback driven entirely by server-authoritative timing** (the client computes no
delay and never sees true positions):

- **Order round trip** (violet) — the three clocks of §6 made fully legible:
  when you issue any order, the server sends a
  `CommandSignal { ship_id, depart_time, arrive_time, observe_time }` the moment
  it accepts the order. The client renders the whole round trip:
  1. *Comet out* over `[depart, arrive]` — a violet comet crosses from your
     command center to the commanded ship's **live ghost** (endpoint is the ghost
     the renderer already draws, so it meets it and cannot overshoot).
  2. *Order received* — a brief pulse at the ghost when the comet lands.
  3. *Response light home* over `[arrive, observe]` — a faint violet pulse
     travels back from the ship toward your command center, with a status label
     **"RECEIVED · response light ~Xs"** counting down. This fills what used to be
     a dead, unexplained gap: the ship hasn't visibly reacted yet *because the
     light of its maneuver is still on its way home*.
  4. At `observe`, the return light arrives and the ghost's new course becomes
     visible — so the course change is explained (it coincides with the response
     light landing), not mysterious.

  `arrive − depart` and `observe − arrive` each equal the player's *observed*
  one-way light delay to the ship (its ghost's staleness), so nothing reveals the
  ship's true distance — the round trip is the player's honest estimate from their
  delayed view, and the client only interpolates between the server's three times.
- **Inbound result rings** (gold): when a raid report becomes observable (M4's
  per-player delivery already gates this by light), gold rings depart the
  resolution point and travel home to the command center, **revealing the verdict
  on arrival**. This reuses the existing `RaidReport` (`pos` + `age`) — already
  fair, since the player has that data — so no new protocol was needed for it.

The single source of truth is the server's per-player observed stream, so the old
prototype's bugs ("comet overshoots the ghost", "report leaves before you see the
resolution") are structurally impossible. Smoothing/interpolation between
server-provided endpoints and times is the only client-side computation.

**Protocol addition:** `ServerMsg::CommandSignal { ship_id, depart_time,
arrive_time, observe_time }` (server→client, to the issuing player only) in
`crates/server/src/protocol.rs` + `client/src/protocol.ts` — the three clock-times
of the order's round trip. The inbound raid rings needed no addition (they reuse
`RaidReport`'s `pos` + `age`).

### Two-tier information model (broadcast + sensor range)

A second layer of "what each player is allowed to see" sits on top of the
lightspeed delay — and it is enforced **in the view filter**, so it is part of
the fairness guarantee, not a client effect. One law still governs everything:
all information travels at `c`. Nothing here is instant.

- **Tier 1 — broadcast (the Galactic Convention), galaxy-wide, light-delayed.**
  Convoys broadcast identity + position + route, so every convoy (yours and
  rivals') appears as a light-delayed ghost galaxy-wide. **Raiders do not
  broadcast — they are dark.**
- **Tier 2 — sensor range.** Each of a player's assets (every ship + the command
  center) projects a `sensor_range` detection radius; coverage is their union.
  Within coverage you learn more: a convoy's **cargo** is revealed, and a **dark
  raider becomes visible**. Outside coverage, cargo is withheld and a rival
  raider is **omitted from the view payload entirely** — your only warning of an
  approaching raider is the moment it trips your sensors.

**View-filter change & the no-leak choice** (`crates/server/src/view.rs`):
`view_for` now (1) includes all convoys with route, (2) attaches cargo only when
the convoy is within the viewer's coverage, and (3) includes a raider only when
within coverage — otherwise it is *omitted server-side*, never sent-and-hidden.
Detection is computed in the **command center's delayed composite frame**: an
object is "in coverage" when its **delayed ghost** falls within `sensor_range` of
an asset's **delayed ghost** (or the command center). This uses only light that
has arrived, so it never reveals the true position of a dark ship (you still only
see where it *was*), and it matches exactly what the client draws — a detected
raider always appears inside a drawn coverage circle.

**Protocol additions:** `GalaxyInfo.sensor_range`; `GhostView.route` (convoy
broadcast waypoints) and `GhostView.cargo` (present only in range); a `CargoView`
+ `Commodity`. In the sim: a `sensor_range` config constant and an
`Option<Cargo>` on ships (convoys carry demo cargo; raiders carry none).

**Client visualizations:** soft teal **sensor-coverage** bubbles around your
assets; convoy **routes** (waypoints + path, light-delayed); **cargo labels**
shown when known (gold for an in-range rival's manifest — intel) and `cargo ?`
when out of range; a detected rival raider rendered as a **pulsing red "⚠ RAIDER"
threat contact**.

**Verified** (`scripts/sensor_smoke.mjs` + 6 view-filter unit tests): convoys
broadcast galaxy-wide; cargo is present *iff* the convoy is within coverage; a
dark raider well outside coverage is absent from the payload (no leak), and every
visible rival raider is within coverage; browser-confirmed the coverage bubbles,
routes, cargo reveal, and the threat contact appearing as a raider enters range.

### What M5 delivers so far (sub-step 5a — the hub Exchange)

The economic spine of §9, tied to the raiding loop:

- **The hub Exchange** (`crates/sim/src/market.rs`): one shared market, a standing
  price per commodity that **walks with flow** (buys lift, sells depress) and
  **drifts** on a slow seeded random walk so there's always something to trade.
- **Instant execution, lagged price information.** A market order settles *now*
  at the true standing price (correlation is instant), but the **price ticker is
  light-delayed** from the hub (the server's `PriceHistory` sends each player the
  prices as of the light that has reached their command center). So you commit to
  the *true* price, not the stale number you read — verified: the ticker showed
  ≈10.00 while a buy filled at the drifted-true 10.42.
- **Orders carry intent + destination, spawning raidable convoys.** A **buy**
  settles instantly (credits debited) and spawns a delivery convoy **hub → home**
  (price-certain, delivery-risky). A **sell** commits the goods *first* and spawns
  a convoy **home → hub** that clears at the **price-on-arrival** (the §9 buy/sell
  asymmetry — double uncertainty). Both convoys are ordinary `Convoy`s, so they
  are **raidable in transit** (M4); a raided trade convoy's goods are simply lost.
- **Credits + inventory** on each corporation; a **market panel** client UI
  (prices, staleness, your wallet, Buy/Sell — press **M**) and an economy news log.
- *(Nice lightspeed detail: a buy's delivery convoy spawns at the hub, ~16s of
  light from home, so you don't even see your own inbound convoy until its light
  arrives.)*

**Protocol additions:** `ClientMsg::MarketBuy` / `MarketSell`; `View.market`
(lagged `PriceView`s + `staleness`) and `View.wallet` (`credits` + `inventory`);
`ServerMsg::Trade`. Sim: a `Market`, `Corporation.credits`/`inventory`, a
`TradeMission` on ships, and `TradeEvent`s.

**Verified** (`scripts/economy_smoke.mjs` + 3 sim trade tests): lagged ticker;
buy settles instantly and spawns a delivery convoy; sell commits goods to a
hub-bound convoy; delivery/sale resolve on arrival; browser-confirmed the market
panel, trade news, and convoys crossing raidable space.

**Sub-step 5b — limit orders + batch clearing.** Limit orders rest on a per-
commodity book (resources reserved at placement — credits for a buy, goods for a
sell). Every ~20 s a **periodic uniform-price call auction** clears each book:
all trades settle at one price, so reacting fastest confers no edge (the §9 anti-
sniping mechanism). A matched buy settles and spawns a delivery convoy (refunding
any over-reservation); a matched sell is paid; unmatched orders rest to the next
batch. Client: a limit toggle + price in the market panel and a resting-orders
list. Verified by `scripts/limit_smoke.mjs` + 2 sim tests (a crossing pair clears
at the uniform price; non-crossing orders rest).

**Sub-step 5c — equity valuations.** Each corporation's net worth (credits +
goods at market — held, in transit, and reserved in resting orders — plus
buy-order escrow) is recomputed on a **slow cadence** (≈ every 60 s) to keep it
readable, not noisy (§9), and shown in the market panel ("equity"). Verified the
figure ≈ credits + inventory value.

### What M6 delivers (verified) — robustness, persistence, scale

- **Restart restores the galaxy from the latest snapshot (§14).** Snapshots (full
  `World` JSON) are written off the hot path every ~10 s; on startup the server
  loads the most recent one and resumes from it (else generates a fresh galaxy).
  A reconnecting player resolves to the same corporation (the stable name hash),
  now restored with its credits, inventory, ships, resting orders, and market.
  Verified by `scripts/restart_smoke.sh`: a player buys fuel (credits 10000 →
  8023), the world snapshots, the server is **killed and restarted**, and the
  rejoining corp is restored at 8023. *(Restart transient: the per-player view
  history is rebuilt fresh, so the galaxy re-illuminates over ~one light-crossing
  as light propagates from the restored positions.)*
- **Scale to 12 players in one galaxy.** Galaxy radius scales with player count
  (§4); the single authoritative loop builds 12 distinct per-player delayed views
  and keeps up. Verified by `scripts/scale_smoke.mjs` (run with `MAX_PLAYERS=12`):
  12 distinct players each get a live ~10 Hz delayed view and `/status` reports
  12 online — the loop isn't falling behind.
- **Session robustness.** Corporations persist across disconnects and keep
  running on their standing orders (ships patrol, trade convoys continue);
  reconnecting with the same name resumes the corporation; half-open connections
  are reaped by the M1 keepalive + idle timeout.

M5 thus realises the §9 model: instant execution + lagged prices, market AND
limit orders with uniform-price batch clearing, order-spawned **raidable** trade
convoys, the buy/sell asymmetry, and slow valuations. *(Documented
simplifications: limit-order goods settle at the exchange rather than each
spawning a crossing; the sell-news is shown promptly rather than light-delayed;
home is treated as light-distance from the hub rather than a zero-lag coherence
peak — all consistent, additive-friendly choices noted for later refinement.)*

**Verified in-browser:** issuing an order shows the violet comet traveling from
the command center to the ship's ghost (paced by the server's observed delay); a
resolved raid shows gold rings arriving home and the verdict revealed on arrival.
Each player sees these from their own command center / observed frame (the comet
goes only to the issuing player; both signals are built from that player's
command center + ghosts/report).

### What System Claims + Resource Production delivers (verified) — the economic engine

The economy finally has a SOURCE: goods come from systems players develop, not
from nowhere. (Resource model adapted & simplified from Stellar Charters'
*deposits-on-bodies* idea — system-level deposits, no planet/body hierarchy.)

- **Resource deposits with a frontier gradient (§4):** every star system carries
  1–3 **deposits** (`crates/sim/src/galaxy.rs`) — a commodity, a `richness`
  (units/sec), renewable reserves. Generated deterministically from the seed so
  richer/more valuable deposits concentrate toward the rim: the best production
  is out in the dangerous, fog-blind frontier. *Proven on the wire: inner-third
  systems value-rate ≈ 8 vs outer-third ≈ 62 — the frontier ~7× richer.*
- **System claims (credit cost):** `ClaimSystem` is a normal command — the sim
  charges the (value-scaled) `claim_cost` and transfers ownership in true space.
  **Ownership is light-gated** exactly like a home-anchor claim
  (`view::filter_systems`): you see your own claim instantly, a rival learns who
  owns a system only once the claim's light reaches their command center. *Proven
  on the wire: a rival learned a claim 36.5 s later — matching its 36.6 s light
  delay — and never sees the owner's stockpile.*
- **Continuous production (§5.1):** each claimed system accrues `richness·dt` of
  its deposits into a private stockpile every tick — whether or not the owner is
  logged in (it's in the deterministic sim). The owner sees their stockpile
  (predictable from known rates); rivals never do.
- **Production feeds the convoy/raid loop:** `ShipProduction` empties a system's
  whole-unit stockpile into the SAME raidable convoys as M4/M5 — they fly the
  frontier→hub crossing and sell on arrival at the price-on-arrival. So **raiding
  a convoy now destroys real production output.** The loop closes: **claim →
  produce → ship across fogged space → sell → expand**, with raiders preying on
  the shipments.

Server-authoritative & leak-free: static geology (deposits, claim cost) is sent
once; dynamic ownership/stockpile flows through the per-player view filter and
obeys the lightspeed law. Deterministic (seeded generation + accrual); claims,
deposits, and stockpiles are part of the `World` snapshot, so they survive a
restart (M6).

**Verified:** sim tests (frontier-richer determinism, claim charge/ownership,
accrual over time, production → raidable convoy that sells, raiding a production
convoy) + the view light-gating test; the two-player wire smoke
[`scripts/claims_smoke.mjs`](scripts/claims_smoke.mjs) (frontier gradient,
charge, **light-gated ownership**, private stockpile, accrual, shipping); and
in-browser (deposit/claim panel, the richness glow gradient, claiming a frontier
system, live stockpile accrual, shipping a production convoy).

### What Acceleration & Proportional Pursuit delivers (verified) — Expanse-style chases

Ships no longer have a hand-set acceleration; they have **thrust and mass**, and
acceleration is derived (§7).

- **`a = F / m` (`crates/sim/src/ship.rs`):** each `ShipKind` exposes a `thrust`
  (force) and a `hull_mass`; `Ship::accel()` returns `thrust / (hull + cargo)`.
  The convoy hull (4500) is ~22× the raider's (200), so for comparable thrust the
  raider accelerates ~11 su/s² and the convoy ~1.5 — the **nimbleness asymmetry
  emerges from MASS**, not from a per-kind accel constant. **Cargo adds mass**
  (`CARGO_MASS_PER_UNIT`), so a fully-loaded convoy (~0.86 su/s²) accelerates
  noticeably worse than an empty one — your richest shipments are the most
  sluggish. Mass is now a real property other systems can hook into later.
- **Proportional pursuit (`crates/sim/src/movement.rs::pursue_step`):** a raider
  does NOT solve a closed-form acceleration intercept. Each tick it steers toward
  the target's **light-delayed observed position** (`target − target_vel·range/c`,
  a crude retardation that sharpens to the truth as range closes — the pursuit
  loop and the fog model are the same loop) and accelerates within budget, easing
  toward the target's velocity as range shrinks so it **slides into contact
  instead of orbiting** (no donut). Convergence is emergent, like a guided
  missile — cheap and robust. The old `intercept_time`/`intercept_step` solver is
  gone.
- **Approximate intercept estimate (client):** on commit, the client shows a
  CRUDE constant-velocity projection (ignores acceleration + the delayed pursuit,
  so it **drifts**) rendered as a soft, fuzzy, concentric **amber zone in the
  sensor-circle idiom** — "best guess, about here," honest about the player's
  stale, approximate read. It updates as fresher ghosts arrive and clears on
  recall / the result notification.
- **Tuned to be WATCHABLE:** thrust/mass values are deliberately low for the
  current galaxy scale — a full chase plays out over **tens of seconds** (verified
  ~53 s commit-to-contact), the convoy visibly lumbers up to speed while the
  raider darts, and a recall has time to matter. All values are tunable consts.

**Verified:** sim tests (`acceleration_derives_from_thrust_over_mass`,
`raider_runs_down_a_moving_convoy`, `pursuit_runs_down_a_fleeing_target_…`) +
in-browser: instrumented a raider running a fleeing convoy down — raider cruising
120 vs convoy lumbering 33→48, range closing 5900→contact, the raider braking
120→25 into a clean contact (no orbit), the soft drifting intercept estimate on
screen, contact at ~53 s, and the result notification firing. Fog, sensors,
raiding, recall, and the economy all still work.

### What Autonomous Defensive Interception delivers (verified) — defense without presence

Defense must work while you're offline (§5.1, Pillar 1): under lightspeed
command-lag you cannot react in real time, so defense is **standing doctrine your
ships execute autonomously, server-side** — the combat-layer equivalent of offline
production accrual. (First piece of a future configurable defensive-doctrine
system; for now a single hardcoded behavior.)

- **`World::autonomous_defense()` runs every tick for all patrolling raiders**
  (`crates/sim/src/world.rs`), deterministic and server-authoritative. Each picket,
  on its **own local sensing**, adopts the nearest friendly convoy within
  `ASSIGN_RANGE` as its charge and keeps station on it (so a fast escort doesn't
  drift out of sensor range of its ward — the starting raider now escorts its
  convoy's lane instead of roaming).
- **Fog-respecting detection:** it reacts ONLY to hostile raiders inside its OWN
  `sensor_range` (dark raiders beyond it are invisible) that are **on an intercept
  course** toward the charge (moving, heading roughly at it — observable geometry,
  never a peek at the rival's hidden orders). So patrol **positioning** decides what
  it can defend — a picket with no convoy in range, or that can't sense the threat
  in time, fails. `THREAT_MIN_SPEED`, `THREAT_CLOSING_COS`, `ASSIGN_RANGE`,
  `PURSUIT_BREAKOFF_MULT` are all tunable.
- **Reuses everything:** on a threat it sets an ordinary `ShipOrder::Intercept` and
  the existing **proportional pursuit** chases it down; contact resolves through the
  existing **seeded raider-vs-raider battle**; the outcome surfaces through the
  existing **delayed report → notification** (no inbound signal), so the owner —
  even one who was offline — learns it on their own light-clock, asymmetrically. Once
  the quarry is destroyed or flees past breakoff, the picket **resumes patrol**.

**Verified:** sim tests (`patrol_autonomously_intercepts_a_threatening_raider`,
`patrol_ignores_a_threat_beyond_sensor_range`,
`patrol_positioning_decides_whether_it_can_defend` — close engages, far fails &
convoy is lost — and `defender_resumes_patrol_after_the_threat_is_gone`); and the
OFFLINE wire test [`scripts/patrol_defense_smoke.mjs`](scripts/patrol_defense_smoke.mjs):
the defender goes offline, an attacker raids the unattended convoy, the escort
autonomously fights the attacker (raider-vs-raider), and the defender **reconnects
to learn of it as ~20 s-old delayed news**. Movement, fog, sensors, raiding, recall,
economy, and notifications all still work.

---

## Fleets (§13.1 / §14.2)

The map/sim unit is a **`Fleet`** — one or more ships of mixed kinds moving,
fighting, and being observed as a **single entity**. A *fleet-of-one* is the N=1
case and behaves exactly as the old single ship did.

**Kinematics (§14.1 — constant per-kind speeds).** Flip-and-burn acceleration was
retired after playtest (invisible at async cadence; its `t = 2√(d/a)` law defeats
the mental math a prediction game needs). Ships now travel at a **constant
per-kind speed** — Scout 115 · Raider 100 · Corvette 65 · Convoy 40 · Colony 33 —
so travel time is simply `t = d / v`, and retarded-position observation and
intercept are **analytic** (a closed-form lead, not a feedback controller).
Magnitudes are calibrated so a representative galaxy-crossing trip takes about as
long as the old ramped flight did (an 8000 su convoy run: old ≈199 s, new 200 s).

**Formation rule (§14.2 — the slowest member sets the pace).** A fleet's speed is
`min over present kinds (speed)`; total mass is `Σ hull_mass(kind) × count +
cargo`, so **fuel ∝ distance × total mass** as before. A raider "hammer" carrying
a colony ship *lumbers* at the colony's pace, telegraphing itself by physics.
Cargo no longer slows a fleet (constant speed) — it costs FUEL (mass), not time.

**Broadcast vs dark.** A fleet **broadcasts** (Convention, galaxy-wide,
light-delayed) if **any** member kind broadcasts (convoy / corvette / colony) —
you cannot hide a freighter by parking a raider beside it. A fleet of only
raiders and/or scouts runs **dark** (visible only inside a rival's sensor
coverage).

**The two-tier intel ladder (fog gating).** What a rival learns about your fleet
comes in two tiers on top of the lightspeed delay:

| Tier | What | When |
|------|------|------|
| **`count_class`** | an estimated-size **bucket** — `1 · 2–3 · 4–7 · 8–15 · 16–30 · 31+` | **always**, on any visible fleet |
| **`composition`** | the **exact** kinds + counts | only **inside sensor coverage** (or your own fleet) |

Buckets — not ± ranges — so the estimate can't be inverted to an exact N. You
know a hammer is inbound and roughly how big *long before* you learn what's in
it. Dark fleets are omitted entirely outside coverage, so when seen at all they
show full composition (consistent — no half-seen dark fleet). Cargo gating is
unchanged (convoy cargo shows only in coverage).

**Combat (Part 2 — deterministic Lanchester attrition).** Battles are no longer
an all-or-nothing seeded coin-flip. Two pooled sides deal damage **each tick** in
proportion to their weighted **attack** power; the damage spreads across the
enemy's kinds by `count × hull` share and accumulates in per-kind **damage
pools**; when a kind's pool fills a hull, one ship dies and the pool carries the
remainder. You lose *counts*, not coin-flips.

- **Hull table** (`hull = defense_weight × 10`, floored at 2): Convoy 10 ·
  Colony 10 · Raider 20 · Corvette 40 · Scout 2 (a scout "dies if engaged" —
  stripped the instant it's in a battle) · platform tier 30. `DMG_RATE = 0.1`
  per tick (a raider wears a convoy down in ~1 s, grinds a corvette screen over
  ~4 s). **Corvettes soak first** via their high hull share — escort is now
  primarily *composition* (put corvettes in the convoy fleet).
- **Concentration law (proven numerically).** One 20-raider fleet vs a 10 leaves
  **~18 survivors** (Lanchester's √(20²−10²) ≈ 17.3, not the linear 10); the same
  20 beating **two sequential 10s** keeps **14** — concentration beats division,
  exactly the Travian `(loser/winner)^1.5` spirit.
- **Retreat is literal.** Doctrine thresholds now fire on the **fraction of own
  weighted strength lost** (Half = withdraw at 50 % gone); survivors disengage
  and flee home. Re-checked every tick, so **relief merging in mid-battle shifts
  the ratio and can flip the outcome** (tested).
- **Raid vs battle asymmetry.** Cargo raids run at `DMG_RATE × 0.3` (a survivable
  skirmish — the raider seizes the convoy's cargo and breaks off); blockade /
  defense-of-place run at full rate (decisive). A **defense platform** folds into
  the defender as stationary tiers with their own pool (ram attrition preserved).
- **Reports** are the same light-delayed, per-side battle reports — now
  **composition vs composition with losses per kind** ("You lost 2 Raider · They
  lost 3 Corvette"), read by old light.

The engagement is otherwise **stateless**: only the damage pools persist (on the
fleets and the platform), so a mid-battle snapshot/restart resumes the fight. The
whole thing is a single shared pure function (`combat::attrition_tick` /
`project_engagement`) — the authoritative sim and the stale-intel calculator
(Part 3) both call it, so they can never drift.

**The stale-intel battle calculator (Part 3).** When you commit a raid you get a
**projected engagement estimate** — computed by running *that same shared
Lanchester function* forward, fed **only by your own view data**:

- **Your fleet** — exact (you know your own ships, pools and all).
- **The target** — its ghost at the retarded state: **exact composition inside
  your sensor coverage**, otherwise a **typical warfleet of the bucket midpoint**
  ("assuming typical hulls") — provably a function of the *bucket*, never the true
  count (leak-checked: a true 25-ship fleet out of coverage is modelled as ≤ 23).
- **Their defenses** — a platform from your **aging scout snapshot** if one covers
  the target, else marked unknown.

The panel reports projected per-kind losses on both sides **and the age of every
input** ("their composition: 12s old · defenses: scouted 4m ago") — exact
arithmetic, honest about stale inputs. It **calls the shared combat function** (no
reimplementation, no drift) and **never touches authoritative state** (a read-only
`EstimateEngagement` query answered from the view filter).

**Management v1 (compose at an owned system, never in flight).**

- **Build** → new fleet, or **join** a fleet docked at that shipyard (`join`).
- **`MergeFleets { into, from }`** — fold a co-located idle fleet into another.
- **`SplitFleet { fleet_id, counts }`** — detach some ships into a new idle fleet.
- **Colony-claim consumes ONE colony ship** from the arriving fleet's
  composition; the rest of the fleet (escorts, extra colonists) persists and
  parks at the new holding.

All management commands **soft-reject** invalid requests (not the player's, not
idle, not at an owned system, empty/over-draw split). No in-flight detachment in
v1 (deferred).

**Client.** Each fleet renders as **one sprite** — the flagship by precedence —
with a **count badge**: exact Σ for your own fleets and rivals inside coverage,
the **bucket label** ("4–7") for rivals outside it (drawn dimmer, an honest
estimate). A fleet-of-one shows no badge, so it looks exactly like the old single
ship. The fleet panel mirrors the ladder: full composition for own fleets and
rivals in coverage; "est. 4–7 ships — composition unknown" outside.

**Migration & protocol.** The persisted entity table renamed `ships` → `fleets`
and each entity gained `composition` (a `{kind: count}` map) and lost the scalar
`kind`; **`migrate_world_json`** back-fills any pre-fleet snapshot so **every
persisted ship becomes a fleet of one**. The wire **protocol is bumped to v2**
(`GhostView` gains `count_class` + `composition`; the entity is drawn/named by
its flagship). Old clients' extra fields are ignored; a version mismatch is
warned on the client.

---

## Speed-signature detection (§Part 4)

Dark-fleet visibility is no longer binary. **Detected ⇔ `distance ≤
sensor_capability(observer) × signature(target)`** — one shared function
(`detection::detected` / `signature`) used by BOTH the server's View gating and
the sim's picket sensing (parity-tested), evaluated from the **retarded sample's
velocity** (a fleet that sprinted then coasted is caught by its *old* flare).

`signature = size_mult × speed_mult × cloak_mult`:
- **size** — per-kind `SIG_SIZE` (scout 0.5 · raider 1.0 · corvette 2 · convoy 4
  · colony 5) summed over the composition, with range scaling as **√(signal)**: a
  6-raider pack is seen √6 ≈ 2.45× farther than one — louder, but sub-linearly.
- **speed** — quietest (`1/2.5`) at/below the stealth fraction, ramping to **1.0
  at full speed**; the full:stealth loudness ratio is `SPEED_SIG_MAX = 2.5`.
- **cloak** — a research **STUB returning 1.0** (future cloak-tech hook).

`sensor_capability = bubble_range × SENSOR_TECH_MULT`, the second **stub at 1.0**.

**Normalization anchor (migration-gentle):** a **single raider at full speed = 1.0**,
so its detection radius is the plain bubble — today's behavior, byte-for-byte
(the sim's own detection is unchanged; the whole prior suite passes). Scouts
(smaller) run quieter, multi-ship dark packs louder, stealth transit quieter.

**Transit throttle (the choice):** a fleet order carries a transit mode — `Full`
(×1.0, default) or `Stealth` (× `STEALTH_FRACTION` = 0.5 → ~2× trip time). Pursuit
stays Full in v1. A dark strike pack at flank speed is flagged far out; the same
pack creeping at stealth reaches the sensor edge unseen (and slower). Scope: DARK
fleets only — broadcasters stay galaxy-visible through the bucket ladder, own
fleets exact. Client: loud contacts get a steady **flare/plume** (distinct from
the pulsing threat ring); the fleet panel toggles Full/Stealth and reads out a
rival's signature ("running LOUD / quiet").

---

## Architecture (§14 of the design)

```
            ┌──────────────────────────────────────────────────────┐
            │  server (Tokio)                                        │
  client ───┤  ┌────────────┐   intents    ┌──────────────────────┐ │
  (Pixi) ◄──┤  │ ws conn    │ ───────────► │ game loop (single     │ │
   WS       │  │ (axum)     │ ◄─────────── │ owner of World +      │ │
            │  └────────────┘  per-player   │ Sessions; 30 Hz tick) │ │
            │       ▲          stream       └──────────┬───────────┘ │
            │       │                                  │ events,      │
            │       │                                  │ snapshots    │
            │       │                          ┌───────▼───────────┐  │
            │       │                          │ persistence task  │  │
            │       │                          │ (sqlx → Postgres, │  │
            │       │                          │  or no-op stub)   │  │
            │       │                          └───────────────────┘  │
            └───────┼──────────────────────────────────────────────┘
                    │ uses (pure, no I/O)
            ┌───────▼───────┐
            │  sim crate    │  World + step(commands) -> events
            │  (deterministic)
            └───────────────┘
```

The pure core is the determinism guarantee and (later) the bot-balance oracle.
Everything that touches the outside world lives outside it.

---

## Running it

### Prerequisites
- Rust (stable; built with 1.91)
- Node 18+ (for the client; built with Node 24)
- *(optional)* PostgreSQL 16 for durable persistence

### 1. Build & run the server

```bash
# from the repo root
cargo run -p server
```

The server listens on `:8080` (HTTP + WebSocket at `/ws`). With no `DATABASE_URL`
it uses the in-memory persistence stub and prints a warning — that's fine for
playing/testing.

Environment knobs: `PORT` (default 8080), `GALAXY_SEED`, `MAX_PLAYERS` (default 4,
sizes the galaxy), `DATABASE_URL`, `SNAPSHOT_EVERY_TICKS` (default 600 = 20 s),
`RUST_LOG` (e.g. `info`).

### 2. Run the client

**Development (hot reload):**
```bash
cd client
npm install
npm run dev          # serves on http://localhost:5173, connects to ws://localhost:8080/ws
```

**Production (one command):** build the client once and the server serves it:
```bash
cd client && npm install && npm run build && cd ..
cargo run -p server                # open http://localhost:8080
```

### 3. Multiple players

Open the client in two or more browser tabs (or machines, pointing at the same
server). Enter a **different corporation name** in each — each becomes a distinct
player commanding from its own home anchor, with its own delayed view.
Reconnecting with the same name resumes that corporation (its ships, credits,
inventory, and resting orders persist). Size the galaxy for the player count with
`MAX_PLAYERS=12 cargo run -p server`.

## Playing the game

You command a chartered corporation from your **home anchor** — and you never see
the galaxy as it *is*, only as the light that has reached your chair (§6). Every
sighting shows where something *was*; every order crosses space at light speed.

- **Read your delayed map.** Your own ships are cyan **ghosts** — crisp and
  near-certain when close to home, but stale and ringed by an **uncertainty cone**
  when far out (there's no FTL tether to your fleet — certainty comes from being
  near your command center). Rivals are red ghosts the same way. Every ghost shows
  how far it could have moved since the light left and a "Δ Ns" staleness label;
  an own ship under orders also hints where it's likely advanced along its course. Soft **teal bubbles** are your sensor coverage; outside
  them you're blind to raiders. Convoys broadcast galaxy-wide (with their route);
  cargo only shows for convoys inside your sensors. A pulsing red **⚠ RAIDER** is
  your only warning of an attacker that has entered range.
- **Command across the delay.** Click one of your ships to select it, then click
  empty space to **move** it — a violet comet shows your order crossing to the
  ship; then a return pulse + "RECEIVED · response light ~Ns" shows you waiting
  for the light of its maneuver to come home (the ghost only changes course when
  that light lands). The three clocks are always visible.
- **Raid.** Select a raider, click a **rival ghost** to commit an intercept — it
  pursues the rival's *true* position, not the stale ghost you saw. Press **R** to
  recall (it may arrive too late). When a raid resolves, gold **report rings**
  cross home and reveal the verdict on arrival — and the two players learn it on
  *different* clocks.
- **Trade (press M).** The **Hub Exchange** ticker is light-delayed, so you commit
  to the *true* price, not the stale one you read. **Buy** settles now and a
  delivery convoy crosses home (raidable). **Sell** ships goods to the hub first
  and clears at the price-on-arrival (riskier). Or place **limit orders** (tick
  "limit @", set a price) that rest and clear in a periodic uniform-price batch —
  no sniping edge. Your credits, holdings, equity, and resting orders are in the
  panel; credits + equity are also in the top HUD.

The core loop: **command from home through honest lightspeed delay, trade on the
shared Exchange, raid each other's convoys, and learn the outcomes as delayed
news on your own clock.**

### Optional: durable persistence with Postgres

A throwaway, isolated dev cluster (does **not** touch your system Postgres):

```bash
scripts/devdb.sh init                 # creates ./.devdb on port 5433 (trust auth)
export DATABASE_URL="$(scripts/devdb.sh url)"
cargo run -p server                   # now writes events + snapshots to Postgres
# ...
scripts/devdb.sh stop                 # or `nuke` to delete it entirely
```

---

## Tests

```bash
cargo test                            # 33 unit tests: determinism, flip-and-burn
                                      # physics, the lightspeed fairness invariant,
                                      # command latency, raid resolution + recall,
                                      # delayed-report delivery, two-tier sensor model

# end-to-end checkpoint smoke tests (server must be running on :8080):
cargo run -p server &                 # in one shell
node scripts/m1_smoke.mjs             # M1: per-player streams, join/leave (+/status)
node scripts/m2_smoke.mjs             # M2: galaxy + flip-and-burn movement
node scripts/m3_smoke.mjs             # M3: per-player lightspeed views, no leaks (~35s)
node scripts/m4_smoke.mjs             # M4: raid → delayed reports on own clocks (~70s)
node scripts/sensor_smoke.mjs         # broadcast + sensor range: cargo gating, dark
                                      # raiders omitted out of coverage (~35s)
node scripts/economy_smoke.mjs        # M5: lagged ticker, instant buy + delivery
                                      # convoy, sell asymmetry (~25s)
node scripts/limit_smoke.mjs          # M5: limit orders + uniform-price batch (~25s)
node scripts/scale_smoke.mjs 12       # M6: 12 players, loop keeps up (run server with MAX_PLAYERS=12)
bash  scripts/restart_smoke.sh        # M6: kill + restart restores the galaxy (needs the dev DB)
```

The server also exposes `GET /status` (JSON: connection/session meta — kept off
the per-player game view so presence can't leak faster than light) and
`GET /healthz`.

---

## Layout

```
crates/sim/        pure deterministic simulation core (no I/O)
crates/server/     tokio + axum server: game loop, sessions, ws, persistence
  migrations/      sqlx Postgres migrations
client/            Pixi.js + Vite + TypeScript client
scripts/           devdb.sh (local Postgres), m1_smoke.mjs (checkpoint test)
```

## What's next (post-alpha, from the design)

The seven-milestone build is done. Beyond it, GAME_DESIGN sketches: **warp-lane
construction** (player-built public speed-up corridors via the mass-reduction
model, §10), the **conquest / home-assault endgame** and victory condition (§11),
and **depth** — research/tech, coherence as a contestable system, exploration,
the settlement-key economy, the movable forward command center (§6.1) — and only
then **balance** (via the bot simulator + human playtest).

## Notes / known stubs

- **Persistence stub:** without `DATABASE_URL` the event log/snapshots are
  dropped (logged, not stored). The Postgres path is real and verified, and a
  restart **restores the galaxy from the latest snapshot** (M6). The stub exists
  so the game runs without a database. *(Restart transient: the per-player view
  history rebuilds fresh, so the galaxy re-illuminates over ~one light-crossing.
  Command-replay between snapshots — full event-sourcing — is a refinement; the
  snapshot reload alone bounds restart loss to the ~10 s snapshot interval.)*
- **Delayed reports** (raid outcomes) are marked delivered when handed to the
  outbound queue. Reports are rare and the queue is almost never full, but M6
  should make delivery reliable (re-deliver until acknowledged).
- A **destroyed ship's ghost** lingers (frozen, ageing) in a viewer's delayed
  picture until its last light passes the history horizon — this is correct (you
  still see old light), and the delayed *report* tells you the truth; a tidier
  "last-seen, now gone" treatment is polish for later.
- **Balance is deliberately untuned** (per the design): ship speeds, galaxy size,
  `c`, and raid radii are first-pass values chosen for legible delays, not
  balance.
