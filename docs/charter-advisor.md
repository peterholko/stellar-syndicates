# Charter Advisor: design for review

Phase 1 built Nov 9 2026 (uncommitted, protocol v87); see "As built (Phase
1)". Phase 2 waits for Peter's numbers. Peter, Nov 8–9 2026:
- "I want to add an 'Charter Advisor' to help guide the player primarily
  through the tutorial."
- "If it is an Authority Officer that leaves after the tutorial could the
  player hire another Advisor maybe?"
- "Let's have 3 different officers that new players randomly get."

The selected direction is advice plus a small perk, three fixed candidates,
and this doc before building. The copy and the Phase 2 prices remain
proposals for review. The portrait prompts are in
[charter-advisor-portraits.md](charter-advisor-portraits.md).

## Phase 1: the Charter Advisor (the tutorial)

### Who

A Terran Charter Authority officer assigned to every new charter. They leave
when the charter is fulfilled.

| Slug | Name | Manner | Voice |
|---|---|---|---|
| `veteran` | Commander Imara Okafor | Calm, dry, unhurried | Short sentences, understatement |
| `lieutenant` | Lieutenant Daniel Seo | Earnest, precise | By the book; says "we" |
| `frontier` | Major Rurik Halvorsen | Blunt, practical, wry | The shortcut the manual leaves out |

- **Dealt, not chosen.** The server assigns one of three using a stable,
  domain-separated hash of the account ID, galaxy ID and galaxy seed, sent as
  `FoundingView.advisor`. The assignment needs no stored field and survives
  reconnects and server restarts. A new galaxy ID or seed draws again; the
  same officer can recur. A reset preserving all those inputs preserves the
  officer too. Use an independent hash, without changing home-world allocation.
- **Portraits:** generated Nov 9 2026 and wired
  (`client/art-src/charter-advisors-v1/`).

### Where they appear

1. **The guide card.** Their face sits right beside their line, at the top
   of the card, with their name on hover and an accessible name on both
   shells (Peter, Nov 9 2026: no quote marks, the portrait beside the line;
   the step's title moved up beside the chapter, "Landfall 3/8", and the
   shield countdown left the guide).
   Their step line supplies the ordinary explanation under the title. A current blocker, recovery action,
   reward amount or other state-specific `reason` takes precedence; do not
   replace it with generic advice. Keep the title, numbers, hint and button.
   Choose advice for the action actually shown, including navigation and
   staffing substeps. While waiting or handling an interruption, keep the
   current status rather than repeating the chapter step's instruction.
2. **The moments.** Each is the transmission panel: portrait, name and one
   line, as the pirates use.
   - **Introduction:** once the colony has landed.
   - **Chapter openings and charter crates:** the advisor speaks the existing
     text once, as their line, with their portrait. Keep the crate contents
     and acceptance action. No duplicate prose or additional pop-up.
   - **The raid:** when the raider is seen. Preserve the guide's existing
     reveal gates: no pirate or trader advice before that encounter is told.
   - **First blocked step:** the first actionable blocker. Exclude the
     scripted raid and ordinary waiting, so the raid does not also trigger
     this transmission. Keep the actual warning visible on the guide and
     highlight its action after the transmission closes.
   - **Goodbye:** when the charter is fulfilled. In Phase 2 its button opens
     hiring. Until hiring exists, use the Phase 1 goodbye without that offer.
   - Queue through the existing story UI; discard a queued moment if its
     condition is no longer relevant. Remember acknowledgements in client
     state scoped to the account and galaxy, across reloads and shell changes.
     This is once per browser profile, not account-wide acknowledgement sync.
3. **The stall nudge.** An actionable step that sits unstarted for 3 minutes
   of foreground play gets the coach ring on its control, once per step.
   The bubble shows the advisor's face and the advice for the current action.
   Check the primary card's `status`, pending orders and action availability;
   an empty `plan.progress` alone does not mean idle. Conversely, unrelated
   background work must not suppress help with the current action. Pause the
   timer while a story panel is open or the app is hidden, and reset it when
   the action changes. Do not nudge blocked, underway or completed actions.
   Remember delivered nudges with the same account/galaxy client scope.

### Copy rule

- A step line is at most 12 words and says why or how. A moment line is at
  most 16 words.
- No line repeats the step's title or numbers, and none is flavour for its own
  sake. Peter cut "Your charter: ore → Alloys → the Market Hub." for exactly
  that.
- A test checks that every step has a line and that every line fits its
  limit, including after pirate-name substitution. Cover state-specific
  advice with the same limits; keep mechanical facts outside the speech when
  they need more space.

### Step lines

All three officers share these ordinary step lines: 29 charter steps plus
the two opening navigation steps, for 31 entries. The guide's pirate names
replace "the privateer" and "the privateer's uncle". These are defaults, not
overrides for the current warning, waiting state or recovery instruction.

| Step | Line |
|---|---|
| Open your home system | Start at home. Your colony ship just landed there. |
| Open your landing world | Open the world you landed on to build there. |
| House the settlers | Housing brings settlers ashore and makes room for the colony. |
| Build a Solar Array | Mines and farms need power. Sunlight is free. |
| Harvest biomass | The crate's Provisions won't last. Food starts with Biomass. |
| Make provisions | Biomass feeds no one. The Agroplex turns it into Provisions. |
| Bring the mine online | Keep ore flowing; the Smelter needs it for every Alloy. |
| Smelt your first Alloys | Alloys build everything ahead, and the Hub pays well. |
| Pirates raid the colony | Let it go. It takes what it came for and leaves. |
| Build a Laser Battery | One gun over the colony. Pirates prefer worlds that don't shoot back. |
| Sell ore to the trader | The Shipyard needs Semiconductors. Ore buys them from the trader. |
| Build a Shipyard | No ships, no reach. The Shipyard builds every hull ahead. |
| Build an Interceptor | One fast warship is enough for a lone privateer. |
| Destroy the privateer | Its bounty funds the colony. Send your Interceptor to find it. |
| Eliminate the privateer den | Its den holds what it took. Clear it and take it back. |
| Repair the Interceptor | Dock it at the Shipyard. Damaged hulls fight badly. |
| Build a Tiny Freighter | The den's loot won't fly itself home. |
| Switch to Full speed | Slow saves fuel, but this run is short. Go Full. |
| Haul back the den's loot | Fetch what's left at the den; repeat until it's empty. |
| Export your Alloys | The Market Hub buys Alloys. This is the charter's first export. |
| Research Freight Frames | Research unlocks hulls. This one gives you the Small Freighter. |
| Answer a signal | Signals are chances near home. Send a captained ship. |
| Build a Small Freighter | Three Tiny holds in one hull: fewer trips to the Hub. |
| Expand mining operations | The charter opens your other worlds. Mine the richest. |
| Chart a nearby system | The survey charts the cleared den's belt for your Mining Barge. |
| Build a Mining Barge | Belts offer deposits beyond home. A barge extracts their ore. |
| Prospect the belt | A prospecting pass reveals hidden ore pockets worth mining. |
| Mine the belt | Give the mining order; prospecting alone fills no holds. |
| Bring the ore home | Unload at home, or sell the ore at the Market Hub. |
| Secure your stores | Warehouses make room. Vaults keep a reserve beyond raiders' reach. |
| Defeat the privateer's uncle | Gather your warships at home; defend the colony together. |

### Moment lines (each officer's own)

The raid lines are cynical and insulting by request (Peter, Nov 9 2026:
"something cynical & insulting to the player like 'Typical pirates preying
on the weak and disorganized'"). Okafor's is Peter's line. The introductions
too ("some snarky, cynical, quasi insulting remark"); the pop-up names the
officer, so the lines don't.

| Moment | Okafor | Seo | Halvorsen |
|---|---|---|---|
| Introduction | Another founder. I've buried a dozen colonies; try not to be the thirteenth. | Regulations require an advisor for every charter. Even the hopeless ones. | They give me the founders nobody expects to last. So. Here we are. |
| The raid | Typical pirates, preying on the weak and disorganized. | By the book, raiders hit the least prepared colony in range. Congratulations. | Pirates always find the easiest mark in the sector. Can't say I blame them. |
| The raider is gone | Gone, with your engines. Smelt that ore and earn them back. Slowly, I expect. | Incident logged. Per procedure, smelt ore, sell Alloys and buy it back. Even you can. | Better start smelting that ore to buy back what you lost. You call yourself an entrepreneur?! |
| First blocked | Read the warning under the task. Fix that first. | We have a blocker. Check the warning, then the highlighted control. | Start with the warning. That's what's holding us up. |
| Goodbye (Phase 1) | The charter's fulfilled. The colony stands. What you build next is yours. | Charter complete. I'm reassigned, but your next objectives are ready. Keep us proud. | The colony's standing. My work here is done. Keep it that way. |
| Goodbye (hiring available) | The charter's fulfilled. If you want counsel for what's next, hire an advisor. | Charter complete. I'm reassigned. We can find you an advisor for your next objectives. | You made it. Hire someone sharp for what's next. |
| Freighter lost (Nov 11 2026) | Odd. Pirates rob freighters; they don't destroy the haul. You've upset some criminal lowlife. | Irregular. Pirates destroying a freighter and its haul? We suspect you've upset a criminal lowlife. | A pirate destroying a freighter and its valuable haul? I suspect you've upset a criminal lowlife... |

**Freighter lost** (Peter, Nov 11 2026: "When the player loses their Tiny
Freighter, the Authority Advisor should point out something like 'It is odd
that a pirate would destroy a freighter and its valuable haul. I suspect you
have upset a criminal lowlife...'"). Halvorsen's is Peter's line within the
16-word limit. The first time the corporation's freighter hulls afloat drop
while an officer is on duty (`AdvisorDeskHooks.freighters`, counted from
every view: views carry every own fleet, and a merge keeps its hulls; there
is no scrap order), the panel "Freighter lost" follows the pop-up on screen
by 2 s (`FREIGHTER_LOST_GAP_MS`), typically the uncle's strike transmission
(docs/pirate-vendetta.md, "The first strike"). Once per account and galaxy.
Fixture: `story-event-fixture.html?story=advisor-freighter-lost&advisor=2`.

**Story cards in their own words** (`Advisor.cards`, `cardLine`; at most 18
words with the names filled in). Peter, Nov 9 2026:
- The Laser Battery: "The recommendation to build a Laser Battery should
  come from the Authority Advisor and make it snarky, cynical and quasi
  insulting to the player. The idea is to instill some hate for the
  Authority in the player early."
- The free trader: "let's have the Authority Advisor announce the arrival of
  the Free Trader … Add some snarky remark about how sleezy free traders but
  they might save you in this situation."
- Time to Strike Back: "Add more snarkiness, cynicism and insulting remarks
  to this panel" (it said "The Shipyard stands. {Nephew} is still out
  there.").
- The privateer's contract: "The Authority Advisor gets a transmission from
  the Authority that there just happens to be a contract out to eliminate
  the Pirate" (docs/founding-charter.md, The privateer's contract). Its one
  button opens Contracts.

"{trader}" is the trader's ship ("Ines Varga's Clean Slate"), "{privateer}"
the nephew by name, capitalised at a sentence start.

| Card | Okafor | Seo | Halvorsen |
|---|---|---|---|
| Build a Laser Battery | The Authority protects its investments. You aren't one yet. Shoot back next time. | Per regulation, colony defense is the founder's expense. We've already drafted your next raid report. | Authority patrols don't come out this far. Not for you, anyway. Get a gun up. |
| A free trader | {trader} has docked with semiconductors. Free traders are thieves with permits. Today, you need one. | {trader} has docked. Unlicensed, overpriced and carrying semiconductors. Regrettably, we need them. | {trader} docked. Free traders would sell their own mothers. Lucky you: they also sell semiconductors. |
| Time to Strike Back | A Shipyard. Finally. {privateer} is still out there, laughing at you. Build something that shoots. | Shipyard operational, behind schedule. Regulations permit you to hunt {privateer} yourself. The Authority certainly won't. | Took you long enough. {privateer} is out there selling your engines. Go take them back. |
| Authority contract (the hunt) | Authority contract on {privateer}. They waited until you had a warship. Smart. Unlike you. | Coincidentally, the Authority has just funded a contract on {privateer}. Sign before they notice you might succeed. | What luck. The Authority found money for {privateer}'s head the moment you stopped being useless. |

Each keeps its painting; without an advisor on duty the card keeps its
plain line. The contract card has no painting: the advisor speaks large, the
terms (title, reward) under the line, and its one button is Open Contracts;
without an advisor the Authority's seal says the title.

### Build (Phase 1)

1. **Server.** `FoundingView.advisor` (0–2), dealt as above, plus a protocol
   bump.
2. **Client, shared.**
   - `core/derive/advisor.ts` holds the cast, the lines and the moment
     triggers.
   - Story events gain the advisor as a speaker.
   - The coach bubble can take a face.
3. **Client, shells.** The guide card's header portrait and line on desktop
   (`founding-guide.ts`) and mobile (`surfaces.ts`), the moments and the stall
   nudge.
4. **Tests.** All 29 charter steps and two navigation steps have copy within
   its limit; aliases and action substeps resolve without hiding warnings.
   Assignment is stable for the same account/galaxy inputs and does not alter
   home allocation. Moments do not replay in the same browser profile or
   duplicate the raid. Nudges wait 3 foreground minutes, skip pending and
   underway actions, and survive reloads without repeating. Hidden encounters
   stay hidden. The advisor leaves at the charter's end, with hiring copy and
   a hiring button only when that feature is available.

No reset is needed, because the deal is computed, not stored.

## As built (Phase 1, Nov 9 2026)

Uncommitted. Protocol v87. :8080 restarted on the same galaxy: no reset, since
the deal is computed.

- **Server.**
  - `game_loop::charter_advisor(galaxy, seed, player)` deals the officer. It
    hashes `charter-advisor:{instance}:{seed}:{player}` with 64-bit FNV-1a and
    finishes with SplitMix64.
  - `FoundingView.advisor` carries it (0–2).
  - One test: a stable draw, about a third each, all nine pairings with the
    home world, and a fresh draw for a new instance or seed.
- **Art.**
  - `buildAdvisorPortraitArt` (build-founding-story-art.mjs) cuts two sizes into
    `public/art/derived/advisors/v1/`:
    - `<slug>-320.webp`: the trader/pirate crop, for pop-ups.
    - `<slug>-face-128.webp`: a tighter crop (`ADVISOR_FACE_CROP`), for the
      guide card and the nudge at 28–40 px.
- **The rules, `core/derive/advisor.ts`.**
  - It holds the cast, the moment lines and the 31 step lines.
  - Five substep lines cover actions other than the step's own: staffing,
    the trader's parts, assigning a captain, bringing a ship to its captain,
    and, before the trader is told, "Find semiconductors".
  - `adviceKey` picks the line for the action shown. Aliases resolve through
    `stepStage`.
  - `stepAdvice` returns nothing on a blocked card, a wait (`status`), an
    interruption (`quiet`) or a live `reason`.
- **The guide plan, `founding.ts`.** A step's ordinary why moved from
  `reason` to a new `why` field. The advisor's line replaces it. Without an
  advisor, the why shows as before.
  - **`why`:** "Light the colony.", the Laser Battery, Smelter, Shipyard,
    Return to Home, Full speed, signal, Small Freighter, Warehouse and Vault
    lines, and the den belt's line at the survey.
  - **`reason`:** rewards, counts, places and warnings stay there.
  - **`quiet`:** "The Smelter is running" (it keeps the raid secret),
    "Shipment waiting" and the uncle's end.
- **Both shells.**
  - The line goes where the why was, the face right beside it (name on
    hover and as alt text). The step's title sits in the header beside the
    chapter; the shield countdown is gone from the guide for now. No quote marks, here
    or in the advisor's pop-ups (Peter, Nov 9 2026). The face shows only
    when the advisor speaks.
  - The primary button is `data-coach="founding-next"`. When the card is
    minimized, its expand button is.
- **The moments, `advisor-desk.ts`.** One `AdvisorDesk` in boot.ts, shared by
  both shells. Moments go through the story queue, the officer in gold.
  Until Nov 9 2026 a "TERRAN CHARTER AUTHORITY" seal band topped them;
  Peter: "remove the Terran Charter Authority title at the top of these
  panels, doesn't make sense". They take the plain header now, like every
  story pop-up (the crates too). Shown moments and delivered nudges are kept in `localStorage`,
  under `ss_advisor_v1:[instance, player]`.
  - **Introduction:** once the landing card has closed. Title "Assigned to
    your charter".
  - **Raid:** on the desktop, inside the raid's warning (Peter, Nov 9 2026:
    "add the advisor's portrait much larger integrated into the Pirate
    Warning with some text"). The raid camera's tracking window ("RAIDER
    DETECTED · ON COURSE FOR …") gains a 176 px column on its left: the face
    (up to 156 px, framed in gold), the name in gold and the raid line
    (`TrackingVoice`, trackinginset.ts; render.ts preloads the face). It
    counts as said, and no pop-up follows (`raidVoiced`). Without that
    window (the phone, a small map, a page loaded mid-raid), the pop-up
    comes 2 s after the sighting instead, at step 6 or 7, until the raid's
    report is told. Title "Raider sighted".
  - **The raider is gone** (Peter, Nov 9 2026: "when the Pirate Leaves, pop
    up a large Charter Advisor panel saying some snarky, cynical and
    insulting towards the player"): when the colony loses the privateer
    after its raid (`FoundingView.raid_lost`, v88), up to the trader's
    step, 2 s after the last pop-up. Halvorsen's is Peter's line, cut to 16
    words. Title "The raider is gone".
  - **The Laser Battery's card** (Nov 9 2026): the painted "Build a Laser
    Battery" pop-up, with the advisor's face
    beside their `battery` line (it keeps its painting, so it isn't the
    large panel). It waits for the watch to end and comes 2 s after the
    raider-gone panel (`AdvisorDesk.leftOwed`, boot.ts `offerLaserBattery`).
    Fixture: `story-event-fixture.html?story=advisor-laser-battery&advisor=0|1|2`.
  - **Large panel:** every pop-up where the advisor speaks alone (no
    painting, no crate) is large: a 220 px face beside the line in 22 px
    type (`is-advisor`).
  - **First blocker:** the first actionable one only. Not the raid, not a
    wait, and not a strike or the colony's fall (the uncle's transmissions
    tell those). When it closes, the guide's action is ringed with the
    warning. Title "Needs attention".
  - **Goodbye:** only after a charter they introduced, 3 s after the last
    pop-up. Title "Farewell". No hiring offer yet.
  - **Chapters and crates:** the advisor speaks their existing text as the
    line, and the text isn't repeated. The crate's goods and Accept stay.
- **The stall nudge.**
  - 3 minutes on an unstarted action triggers it, once per step and substep.
  - The clock pauses while the app is hidden, offline or behind a pop-up,
    and while the guide's control is out of sight or disabled.
  - A new action, a new fleet order or a click on the guide's button resets
    it.
  - The bubble sits beside the guide card, with the face. It goes above the
    control where there's no room.
- **Tests.**
  - New: `scripts/test-advisor.mjs` (`npm run test:advisor`).
  - Added: the advisor's line on the real charter plan in test-progression.
  - Results: sim untouched, server 280, client 66 plus tsc.
- **Fixtures.**
  - `scripts/guide-fixture.html` shows the guide card in five situations,
    plus the nudge.
  - `story-event-fixture.html?story=advisor-intro|advisor-raid|advisor-blocked|advisor-goodbye|advisor-chapter-pirates|advisor-crate-vendetta&advisor=0|1|2`.
  - `scripts/raid-warning-fixture.html?advisor=0|1|2` shows the raid's
    window with the advisor's column (Pixi; capture it with headless Chrome
    and `&frames=120` when the pane is hidden).
- **Not checked:**
  - A live playthrough. It needs a guest sign-in, which needs Peter's OK.
  - The mobile card visually. Its markup is checked by tests.

## Phase 2: hired advisors (after the charter)

### Who

Three fixed candidates on the Officers page, one per field. Their portraits
come later and are made the same way.

| Field | Name | Watches | Perk |
|---|---|---|---|
| Industrialist | Mara Voss | Idle factories, short inputs, full storage, the next build | +5% output in the home system |
| Trader | Felix Abara | Hub prices, freight, the contract board | −10% Authority freight fees |
| Strategist | Ines Kade | Raids, pirate dens, weak defenses | +25% sensor range at home: raiders are seen sooner |

### What they do

- **Keep the guide card going after the charter.** Their portrait, then one
  next goal at a time in their field (beside the daily board and medals,
  `postVictoryHandoff`), then the stall nudge for it.
- **Speak when something is wrong in their field,** once per problem: a factory
  idle for 10 minutes, storage at 95%, or a raid sighted.

### Cost and rules

These are the original proposed rates, not approved balance numbers.

- **Hiring fee:** 2% of the corporation's valuation, at least 2,000 Cr. It is a
  paid order from the Command Center.
- **Wage:** 0.5% of valuation a day, at least 500 Cr, taken from the treasury
  each sim day.
- **Unpaid:** an advisor whose wage can't be paid quits, with a line.
- **One at a time.** Firing is free; hiring again costs the fee.
- **Valuation-scaled,** so the fee bites at every size: the credit sink.

### Balance and billing review before Phase 2

- At a fixed valuation of **1,000,000 Cr**, hiring costs **20,000 Cr** and
  wages cost **5,000 Cr per day**: **55,000 Cr** for hiring plus seven paid
  days. These are costs against valuation, not a percentage of daily income.
- The Trader's 10% discount needs **50,000 Cr in eligible daily freight
  fees** to cover that 5,000 Cr wage through the perk alone. This excludes
  the hiring fee and any value from advice. Measure actual freight spending
  before treating the three perks as equally valuable.
- For playtesting, compare the proposed wage with **0.1% of valuation per
  day, minimum 100 Cr**, keeping the hiring fee unchanged. This is a cheaper
  comparison to evaluate, not a replacement already agreed to.
- Define a day as **86,400 simulation seconds**. Proposed billing: the first
  wage is due one full day after hiring, then every full day of employment;
  no immediate bill at the next galaxy-wide midnight. Show the current daily
  wage and next due time before hiring and while employed. Specify which
  valuation snapshot each charge uses and persist billing progress so reloads
  cannot double-charge or skip a payment.
- Define the Industrialist's eligible production and whether 5% means extra
  output per input or faster processing with greater input consumption. Keep
  power, staffing and storage limits explicit. The Trader discounts freight
  fees, not cargo prices; specify stacking with Authority standing. The
  Strategist affects home detection only, preserving report travel time and
  unknown-contact rules.

### Build (Phase 2)

1. **Sim.**
   - `Corporation.advisor: Option<HiredAdvisor { field, hired_at }>`.
   - `Command::HireAdvisor` (paid) and `DismissAdvisor`.
   - The daily wage, with quitting when unpaid.
   - The three perks: a production multiplier at home, a factor on the
     freight fee, a factor on the home sensor range.
   - Tests.
2. **Server.** `View.advisor`, Log lines, and a protocol bump.
3. **Client.** The Officers page hiring section on both shells, the
   post-charter guide card, and the field alerts.
4. **Art.** Three portraits, prompts written when Phase 1's land.
5. **Bots:** they don't hire, in version 1.

## For Peter

1. Are the three names and manners right?
2. Edit any step or moment line you don't like. Each one is short on purpose.
3. Phase 2 numbers: a 2% fee, a 0.5% daily wage, and the three perks?
