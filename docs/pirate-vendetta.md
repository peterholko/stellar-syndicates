# Pirate vendetta

Peter (Oct 25 2026): give the first raid a personality, and end the tutorial
with a vengeful relative who tries to finish the player. Each new player gets a
random pirate family with random names. Portraits:
`client/art-src/pirate-portraits-v1/` (10 masters, 30 expressions; see
`docs/pirate-portraits.md`).

## The story

1. **Step 5, the raid:** the raider is now the nephew, with a name and a face.
   He taunts the colony as he loots it.
2. **Step 10, the kill:** his last words name his uncle.
3. **Step 11, the den:** among the recovered loot is the uncle's message to
   his nephew.
4. **Step 16, the warning:** the uncle swears to come. From here the guide
   counts down his arrival.
5. **New step 19, the vendetta:** the uncle leaves when your step-18 report
   reaches command and arrives about 10 minutes later with his warship and
   escorts. He raids your home like a pirate home raid.
6. **The end:**
   - **Win** (his flagship destroyed): a bounty and his ship's salvage. His
     last words name a brother, the hook for later pirate content.
   - **Loss** (he leaves): he takes a raid's share, up to 60 units and 10% of
     a good, and vows to return.
   - The charter completes either way. A home is never captured.

## Who the player meets

- **Dealt per account and galaxy,** like the home world (`dealt_profile`): one
  of five families, a surname, and first names for the nephew, uncle and
  brother. The art never shows names.
- **Families:** Showboat, Hothead, Collector, Scrapper and Fatalist. Each has a
  nephew and an uncle, and each of those has smug, angry and shocked
  expressions.
- **Names:**
  - Surnames: Vance, Kessler, Marrow, Okafor, Drago, Castellan, Reyes, Holt,
    Varga, Sato, Brandt, Mercer, Calloway, Novak, Quill, Ashby, Rourke, Szabo,
    Moreau, Dunmore.
  - Nephew and brother: Jace, Rook, Dez, Kip, Wren, Corin, Milo, Jory, Nico,
    Dax, Bram, Ezra, Kai, Lazlo, Rafe, Tomas, Ike, Jonah, Vic, Arlo.
  - Uncle: Augustin, Silas, Bartholomew, Cornelius, Mordecai, Ignatius, Lucan,
    Horace, Vasily, Ambrose, Ezekiel, Thaddeus, Casimir, Severin, Barnaby,
    Gideon, Leopold, Ulric, Magnus, Ansel.
- **Elsewhere:** "the Rogue Privateer" becomes the nephew's full name in battle
  notices and the guide, and the privateer den becomes "the <surname> den".

## Lines

`{nephew}`, `{uncle}`, `{brother}`, `{surname}` and `{colony}` (the home
system) are filled in. Each line shows as a transmission: portrait,
expression, name and line.

| Beat | Who, expression |
|---|---|
| raid | nephew, smug |
| kill | nephew, shocked |
| den | uncle, smug |
| warning | uncle, angry |
| arrival | uncle, angry |
| win | uncle, shocked |
| loss | uncle, smug |

### Showboat

- **raid:** "Smile for the feeds, {colony}! {nephew} {surname} thanks you for your donation."
- **kill:** "This wasn't in the script… Uncle {uncle} will finish the show."
- **den:** "{nephew}, my boy. Take what shines, bow to no one, and call home. — {uncle}"
- **warning:** "You cut my nephew's final act short. I'm bringing the encore."
- **arrival:** "Lights up, {colony}. Final performance."
- **win:** "Booed off my own stage… {brother} will want a rematch."
- **loss:** "You've been a lovely audience. Until the sequel."

### Hothead

- **raid:** "Nobody's coming for you. Load it all!"
- **kill:** "No! Uncle {uncle} will break you for this!"
- **den:** "{nephew}. Stop picking fights you can't finish. — {uncle}"
- **warning:** "You killed {nephew}. I'm coming."
- **arrival:** "Here."
- **win:** "Hm. {brother} won't miss."
- **loss:** "Debt's paid. For now."

### Collector

- **raid:** "Your semiconductors and armaments, itemised. Your cooperation is noted, {colony}."
- **kill:** "An… unbudgeted loss. Uncle {uncle} will audit you."
- **den:** "Nephew. Your ledger is in arrears. Collect, then report. — {uncle} {surname}"
- **warning:** "You have closed an account of mine. I am coming to settle it."
- **arrival:** "Your balance is due, {colony}."
- **win:** "A miscalculation… {brother} keeps the books now."
- **loss:** "Paid in full. Do not fall behind again."

### Scrapper

- **raid:** "Ooh, shiny! Don't mind me, {colony}, just tidying up."
- **kill:** "Ha… I'm scrap. Uncle {uncle} will strip you for parts."
- **den:** "Bring me the good bits, {nephew}. Leave the bones. — Uncle {uncle}"
- **warning:** "You broke my favourite nephew. I'm coming to take you apart."
- **arrival:** "Every bolt in {colony} is mine."
- **win:** "Heh… good salvage, me. {brother} will want the pieces back."
- **loss:** "Good haul. I'll be back for the rest."

### Fatalist

- **raid:** "The void gives. The void takes. Today it takes from you."
- **kill:** "So this is my hour… Uncle {uncle} will choose yours."
- **den:** "{nephew}. Fear nothing, keep nothing. Return when the void allows. — {uncle}"
- **warning:** "You hastened {nephew}'s end. The void has named yours."
- **arrival:** "It is time, {colony}."
- **win:** "Not my hour after all… {brother} will carry it."
- **loss:** "Not today. The void is patient. So am I."

## The fight

- **His force:** a pirate warship plus Raider escorts, arriving from the
  direction of his nephew's cleared den. It uses the home-raid code, so he gets a real inbound flight,
  a short contestable blockade and capped loot.
- **Sizing:** two Interceptors with captains win and one alone loses. The
  combat balance harness sets the exact force before release.
- **The lesson:** the charter never teaches home defence. From step 16 the
  guide adds "Build a second Interceptor" and "Assign a captain" beside the
  barge steps.
- **Its goods (Oct 29 2026):** the kit's fourth Warp Core and its Alloys; its
  12 semiconductors and 15 Armaments come from the trader's last call, which
  the bigger privateer bounty (6,400 Cr) pays for (docs/founding-charter.md,
  "The trader's last call").
- **Bots skip it:** for a bot, step 19 completes at once.

## Art

- **The raid painting stays.** Its crew stay anonymous, and the nephew's
  portrait and line sit in the card under it. In a wide shot his face would be
  a few pixels, so per-family repaints wouldn't show him.
- **One new painting for the finale,** shared by all five families. It opens
  step 19.
  - Attach: `client/art-src/story-events-v1/pirate-raid.png`,
    `client/art-src/pirate-corsair-reference-v1/pirate-corsair-reference.png`
    and `client/public/art/derived/ships/models-v1/previews/raider-768.webp`.
  - Save it as `client/art-src/story-events-v1/pirate-vendetta.png`.

```
Use case: illustration
Asset type: one cinematic story-event illustration for Stellar Syndicates, a space colony strategy game, opaque 1672 x 941 pixels. It opens the tutorial's final battle: a pirate warlord comes to avenge his nephew.
Input image 1: LOCATION and STYLE reference, the game's earlier painting of a pirate raid on this colony. Keep the same colony and landscape: the white colony ship, the domed habitats, the solar fields, the mountains and the large moon in the sky. Match its realistic cinematic painting style and finish. It is now night.
Input image 2: the pirate CORSAIR design: long dagger nose, swept crescent wings, three aft engines, dark gunmetal hull with rust-orange patch plates. This is the warlord's flagship, far larger than the raider in Image 1. Never include the white reference sheet, its views or its labels.
Input image 3: the colony's INTERCEPTOR design: white and pale grey hull, twin engines, dark wingtips with cyan lights. These are the defenders.
Composition: the corsair drops out of the night sky over the colony, upper center-right, huge and menacing, engines glowing orange, with three smaller pirate raiders in the same rust-orange and gunmetal livery fanned out around it. Lower left, two interceptors lift off from the colony's pad and climb toward it, their engine trails curving upward. Below, the colony is on alert: amber warning beacons, floodlights sweeping the sky, colonists hurrying to shelter. Keep the corsair, the raiders and the interceptors large and readable in a small pop-up, never tiny distant dots.
Lighting/mood: night; cold blue moonlight, warm amber colony lights, the corsair's orange engines and red running lights; tense, the moment before the fight.
Constraints: no weapons fire, explosions or debris yet; no text, captions, numbers, logos, interface, watermark or border. Exactly one image.
```

## Build (me)

1. **Server:** deal the family and names, and send them in `FoundingView`.
2. **Art:** derive the 40 portraits to 256-pixel WebPs with Teo's face crop.
3. **Client:** the transmission card. Beats 1–3 go into today's charter, and
   names replace "Rogue Privateer" and "privateer den".
4. **Sim:** the `Vendetta` stage (step 19), the uncle's raid, win and loss, the
   reward and completion. Bots skip it.
5. **Guide:** a sixth chapter, Vendetta, kept hidden until the warning, with a
   countdown and the step-16 preparation lines.
6. **Checks:** sim tests for every path, client tests, harness sizing, and
   screenshots of each beat.

## Release

- A protocol bump.
- No reset: the new stage follows Haul Ore, and every new field has a default.
- Corporations past the charter never see the uncle; anyone still in it meets
  him at the end.

## As built (Oct 25 2026)

**Sim** (`crates/sim/src/world/founding_vendetta.rs`, `founding.rs`):
- **The family:** `PirateFamily::of(seed, corp, join tick)` deals it. It is
  never saved, so a new galaxy deals afresh. `World::pirate_family` reads it.
- **The step:** `FoundingStage::Vendetta` is appended to the enum and is the
  19th entry in `STEPS`. Haul Ore, or a survey that charts no belt, hands on to
  it; bots and the older arcs go straight to Complete.
- **The oath** (`swear_vendetta`): sworn as a step from 16 on begins, or as
  step 19 begins for corporations already past 16.
- **The pack:** a Corvette flagship and two Raiders
  (`VENDETTA_FLAGSHIP`, `VENDETTA_ESCORTS`, the `vendetta_pirate` fleet flag).
  - Raised at 1.25 sensor ranges on the den's bearing. It sets out
    `VENDETTA_LEAD_S` (8 min) after the step begins, never at a founder on
    leave.
  - The flight in reuses the founding raid's `FoundingRaid` phases.
- **Over the colony:** he attacks the strongest warship within 1.25 sensor
  ranges of home, and the rest at rest in the well join the fight.
  - Unopposed, he loads a raid's share: a tenth of each good above the 40-unit
    floor, 60 units at most, for at most `VENDETTA_HOLD_S` (2 min).
  - If a fight ends with his flagship alive and a defender still near home, he
    breaks off, so there is at most one fight.
- **The end:**
  - The flagship's death is a win: on its report, `VENDETTA_BOUNTY` (6,000 Cr)
    and `VENDETTA_SALVAGE` (60 Alloys, 20 Machinery, 15 Armaments, as storage
    allows).
  - Reaching the edge of the sensors is a loss.
  - The step completes when the report reaches command and his last ship is
    gone.
- **Sizing:** `VENDETTA_DAMAGE_MULT` 0.3, with no captains aboard.
  - Swept over six galaxy seeds: one Interceptor lost 6 of 6, two won 5 of 6,
    three won 6 of 6 (at ×1.0 even three lost).
  - Seed 7's two-Interceptor loss is the battle's fortune: both fought.
- **Events:** each turn is an `EventPayload::Vendetta { beat }` (kill, den,
  sworn, arrival, won, lost), on its light home.

**Server:**
- `timeline.rs` writes one "Vendetta · …" line per turn. The battle line and
  the den line use the family's names.
- `FoundingView.pirates` and `.vendetta`, protocol v63.
- The arrival raises a Raid alert.
- The tutorial funnel's last step is `vendetta`.

**Client:**
- `core/derive/vendetta.ts`: the lines, faces and names.
- `story-events.ts`: the transmission card; the arrival is painted
  (`pirate-vendetta`).
- `session.ts` turns the server's lines into story pop-ups.
- The guide's step 19:
  - before he sets out: a countdown, and "Build a second Interceptor", then
    "Hold your warships at home";
  - once he is here: "Attack" sends every warship near home;
  - from step 16: the oath's line and the second-Interceptor card.
- `withPirateNames` puts the names into the guide's privateer copy.
- Art: `buildPirateCrewArt` derives 40 faces to `art/derived/pirates/v1`.
- Checks: `client/scripts/test-vendetta.mjs`; the fixture is
  `scripts/story-event-fixture.html?beat=arrival&family=scrapper` on the Vite
  dev server.



## The campaign (Oct 30 2026)

Peter: "The uncle should harass the player's expanded mining ops and conclude
in some big battle", "I would hope the player could be online for the final
large battle" — but not "strictly for online players because the players will
use that fact to exploit and avoid the battle".

**The turns** (charter steps from the 21-step charter):
1. **Step 16, the oath:** he swears once mining spreads to a second world (it
   was step 17, the Reach chapter).
2. **First strike, the second mine:**
   - About 3 minutes after the oath (`STRIKE_MINE_DELAY_S`), one Raider sets
     out from beyond the colony's sensors for the second mine's world.
   - Over it, that world's mines stand idle. It loads up to 30 of the world's
     ore from home stock (`STRIKE_LOOT`, never what the Vault keeps), and
     leaves after 2 minutes unopposed (`STRIKE_HOLD_S`).
   - The guide's card: "Raider closing on / over Freya III · Attack" (every
     warship near home).
3. **Second strike, the barge:**
   - 45 seconds after the barge starts cutting the den's belt
     (`STRIKE_BARGE_DELAY_S`), two Raiders jump it from 5,000 su out.
   - Mining stops while they fight. The card: "Raiders on the barge · Send
     Interceptors". A lost barge gets the existing "Replace the Mining Barge".
4. **Each strike** pays 500 Cr (`STRIKE_BOUNTY`) when all its raiders die, on
   the report. Raiders that get away ride with him at the end
   (`strike_survivors`).
5. **Step 21, the big battle:**
   - His flagship (a Corvette) and four Raiders (`VENDETTA_ESCORTS`, was two),
     plus the strike survivors, all at `VENDETTA_DAMAGE_MULT` 0.28 (was 0.3).
   - Swept over six galaxy seeds with no captains aboard: two Interceptors
     win 2 of 6, three win 5 of 6, four win 6 of 6.
   - From step 16 the guide asks for a second, then a third Interceptor.
6. **Online, if possible:**
   - The server keeps `World::online` (not saved; after a restart nobody is
     online until they reconnect).
   - His 8-minute countdown runs only while the founder is online, and he sets
     out 24 hours after the step begins regardless (`VENDETTA_DEADLINE_S`).
     Once he sets out he never waits.
   - The strikes do the same with a 1-hour deadline (`STRIKE_DEADLINE_S`).
   - The step-21 card's hover says so. Formal leave still defers him, within
     its own limits.
7. **Lines:** a transmission per family for each strike (`mine_strike` smug,
   `barge_strike` angry; client `core/derive/vendetta.ts`). The server's log
   lines are "Vendetta · {uncle}'s raider is over {world}." and "Vendetta ·
   {uncle}'s raiders are on your Mining Barge."; both raise a Raid alert.

**Supplies for the third Interceptor:** the kit's Warp Cores go from 4 to 5. The
trader's stock adds 12 Semiconductors and 15 Armaments (127 and 45). The
privateer bounty goes from 6,400 to 8,300 Cr, so the last call still has
room to spare.

**Wire:** protocol v77. `VendettaView.deadline_at` and `.strikes` (kind, system,
body, fleet once in sight, over on its light, done). No reset is needed: every
new field has a default.

**Known:** the painting shows four escorts, which now matches the game.

## The finale and the fall (Oct 30 2026, §gun-platform)

Peter, the same day: stolen ore "doesn't seem like an equivalent revenge
action to losing a nephew"; the pirates should avoid the home planet now that
step 6 builds a Gun Platform ([founding-charter.md](founding-charter.md), "The
Gun Platform"); the finale stays at the home world; "keep his forces static,
if the player hasn't built enough defenses, game over and they can restart".
The charter now has 22 steps: the oath is at step 17 and the finale step 22.
Since Oct 31 (§charter-crates) "Secure your stores" (a Warehouse and a Vault)
comes before the finale, so it is step 23 of 23.

**The first strike** (`founding_vendetta.rs`, `launch_strike`). It is decided
when the raider sets out:
- **A freighter, if one is out.** It takes a freighter of the founder's in
  open space within 30,000 su of home (`STRIKE_FREIGHTER_SU`), outside every
  gravity well and not docked. The raider sets out 5,000 su ahead of it and
  attacks to destroy it.
  - The freighter is safe once it docks or reaches a well. The raider then
    breaks off.
  - Card: "Raider hunting your freighter · Attack". Log: "Vendetta · {uncle}'s
    raider is hunting your {freighter}." (a transmission and a Raid alert).
- **Else, the second world.** Over it and unopposed, the raider wrecks one
  structure every 30 s (`STRIKE_WRECK_S`): the Mining Complex (or a belt's
  Asteroid Quarry), then the Solar Array (`STRIKE_WRECKS`). Then it leaves.
  - Work under way on a wrecked structure goes with it.
  - Any fight ends the wrecking, as before.
  - Nothing is stolen and the world's mines no longer idle; they're simply
    gone.
  - Log: "Vendetta · {uncle}'s raider wrecked the {structure} on {world}."
    (a plain log line, no transmission).
  - Card: "Raider over Freya III · Mining Complex wrecked". Afterwards the
    guide offers "Rebuild Freya III's mine" (a Solar Array first if that's
    gone too) until it stands again; it doesn't block the step.
- **Unchanged:** the barge strike, the 500 Cr bounty, and survivors riding
  with him.

**Home is off limits** to the strikes. Ordinary pirate packs also avoid any
system with a firing station now, as they always did legacy platforms.

**The finale at home, under the Gun Platform.** His force never changes: the
Corvette and four Raiders plus strike survivors, at ×0.28. The platform's
gun joins any fight in the home's well. With no warship near home, he
assaults the platform itself (`open_vendetta_assault`, once). It ends three
ways:
- **Won:** his flagship dies. The bounty and salvage are paid and the charter
  completes, as before.
- **Repelled:** a fight ends with a warship near home or the platform still
  firing. He breaks off, and the charter completes without a bounty. Log:
  "{uncle} got away." (or "got away with …").
- **Fallen:** he finishes loading his raid share with nothing left fighting
  for the colony. Game over: `FoundingVendetta.fallen`, beat `Fallen`, log
  "Vendetta · {uncle} sacked {colony}." (a transmission and a Raid alert).
  - The charter stops at step 23.
  - The sim refuses every order from the corporation except
    `RestartCorporation` (`World::fallen_refuses`), including orders already
    on their way.

Measured over six galaxies, with the platform at home and no captains aboard:

| At home | Won | Fell |
| --- | ---: | ---: |
| Gun Platform alone | 0 | 6 |
| + 1 Interceptor | 3 | 3 |
| + 2 Interceptors | 5 | 1 |
| + 3 Interceptors (the guide's ask) | 6 | 0 |

The first Gun Platform (1,200 hull) repelled him alone in 5 of 6 and killed
him in the sixth, so the colony could never fall. At 300 hull it's worth
about one Interceptor. No fight ended "repelled" in the sweep; it stays for
a fight that ends with both sides standing.

**The restart** (`world/refounding.rs`). On the game-over screen (desktop
overlay; the guide card "{colony} has fallen · Restart" on both shells) the
founder sends `ClientMsg::RestartCorporation`. The server adds the account's
dealt home world. The sim then:
- Ends the corporation's fights where they stand.
- Removes its fleets, the pirates and traders its charter called up (the
  privateer, his den's garrison, the uncle and his strike raiders, the free
  trader), and TCA expeditions hunting it.
- Clears its build, refit and market orders, freight, survey and pocket
  reports, private contracts (their staged actors leave with them), its place
  in shared ones, its diplomacy and treaties, and its season ledgers.
- Wipes every world it held. The colonies are let go; the home and the den's
  system are reset, and their belts hide their pockets whole again.
- Founds it again on the same home slot (`admit_corporation`, shared with
  `AddPlayer`, then the founding camp as on first join). It keeps its name,
  syndicate and medals. The same account, player id and seat, so the
  journal's history stays.
- The den rises again with the next privateer.

**Lines:** `freighter_strike` (smug) and `fallen` (smug) for each family, and
the `mine_strike` lines now speak of wrecking.

**Wire:** protocol v78. `VendettaView.fallen`, `StrikeView.target` and
`.wrecked`, strike kind `freighter`, `ClientMsg::RestartCorporation`. Needs
`--reset-galaxy` (the station ladder was renumbered).
