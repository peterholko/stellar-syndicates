# Founding charter v4: a bare landing, counterfeit semiconductors

Status: built Oct 14 2026 with every recommendation, uncommitted; protocol
v51, needs `--reset-galaxy`. As built: [founding-charter.md](founding-charter.md).
The opening order is Peter's (Oct 14). The free trader is the phase 1 merchant
([traveling-merchant.md](traveling-merchant.md)), moved to a new cue.

**Where the build differs from this design:**
- **Holds:** 1,000, not 900. The kit came to about 860 units with the fuel.
- **Kit:** 410 Alloys and 163 Machinery, not 390 and 155: the Uranium Mill's
  20 Alloys and 8 Machinery stay in it, as in v3. Uranium went from 20 to 120
  (the 20 home seed plus 100 in the kit), because new hulls launch empty and a
  Tiny Freighter's tanks take 52.5.
- **Stipends:** retuned across the 17 steps, 4,400 Cr plus the 4,500 Cr bounty
  ([founding-charter.md](founding-charter.md#stipends)).
- **The trader:** arrives from 2,500 su, so it docks about 30 seconds after
  the reveal, and carries 100 semiconductors, enough for every charter build.
- **The reveal** also fires if the fakes are booked out on Authority freight.
- **Oct 15, the Pirates chapter reworked (Peter's call):** step 11 is
  "Destroy the privateer" (the reserve captain, then a one-click attack; the
  kill promotes the captain), and a new step 12, "Eliminate the privateer
  den", sends the Interceptor against a pirate base in a nearby system that
  holds only a rich belt. That belt is then the survey's and the barge's.
  Eighteen steps, protocol v52 ([founding-charter.md](founding-charter.md#eighteen-steps-in-five-chapters)).

## Peter's order

1. The colony ship lands on the habitable planet and becomes the Command
   Center. Nothing else stands anywhere: no Warehouse, farms, Habitat, power
   plant, mine or ships.
2. The player builds a Bioharvester,
3. a Habitat,
4. an Agroplex for Provisions,
5. and a Mining Complex, on the home planet.
6. To export ore they need Freighters, so a Shipyard. The Shipyard needs more
   power than the Command Center's reactor makes, so they order a Solar Array,
   and find out the kit's semiconductors are fakes.
- **No planet has free power** (Peter, Oct 14). Every powered building needs a
  power source: a Command Center's reactor or a power plant.
7. A free trader shows up; they buy real semiconductors.
8. They build the Shipyard,
9. then a Tiny Freighter.
10. They try to export ore. Pirates attack, and the Freighter retreats to
    orbit at once.
11. They're told to build an Interceptor to fight off the pirates.
12. Later in the tutorial, "Expand mining operations" opens the system's
    second ore deposit, on Planet III. Players don't build there from the
    start (Peter, Oct 14).

## The Landing Command (the colony ship)

- **Storage:** its holds are the colony's storage. Recommended: 800, enough
  for the kit. No Warehouse at landing.
  - A Warehouse becomes a later build for when storage fills.
  - The founding camp's Storehouse project expands the Landing Command's
    holds (+300) instead of lifting a Warehouse's 250 cap.
  - The Service Berth needs a Warehouse on its world, so it waits for one.
- **Power:** the free 3 MW every planet gets today is removed (`BASE_MW`
  goes). The Command Center's reactor is the landing's only power: 1 MW, on
  its own planet.
  - That runs a Mining Complex I (1 MW).
  - The Shipyard (another 1 MW) needs the Solar Array (8 MW).
  - Farms, the Habitat and storage draw no power.
- **Ore on the home planet:** today the home system holds exactly two
  deposits from `generate_home_deposits` (Biomass and one Ferrite Ore), plus
  the belt's ore.
  - Two is a holdover from the old Provisions-and-Ore home, kept modest so
    nearby systems are worth expanding to. The ore landed on Planet III,
    which is why the v3 guide built the first mine there. Planet III never
    started with one.
  - Nothing stops more: a body takes up to four resource sites.
  - **Built Oct 14 (ahead of the rest of v4):** a basic Ferrite Ore deposit
    on the home planet, at 0.85× base richness, the middle of the home ore
    range, with no extra roll. The home planet now has two resource sites
    (Biomass and ore). The guide's first mine goes there while it has a free
    site (`bestMineBody`). It needs `--reset-galaxy` to reach new homes.
  - Planet III keeps its own Ferrite Ore for step 13, "Expand mining
    operations". Until then the charter's land grant covers only the home
    planet (see **Expanding the mine**).
  - The belt keeps its ore.
- **Housing:** the settlers live aboard until the Habitat stands. Habitat I
  houses 3,000 (6,000 after the camp's Permanent Quarters), and migrants come
  only once it's built.

## The charter, step by step

| Chapter | Step | What you do | Completes when |
| --- | --- | --- | --- |
| Landfall | 1. Harvest biomass | Build and staff a Bioharvester | It's staffed |
| | 2. House the settlers | Build a Habitat | It's built |
| | 3. Make provisions | Build and staff an Agroplex | It's staffed |
| | 4. Bring the mine online | Mining Complex I on the home planet, powered by the reactor; staff it | It's staffed (as v3) |
| Counterfeit | 5. Power the Shipyard | Order a Solar Array → **the reveal** | The reveal |
| | 6. Buy semiconductors | The trader docks; buy, then build the Solar Array | The Solar Array is built |
| | 7. Build a Shipyard | Build and staff it | It's staffed |
| Pirates | 8. Build a Freighter | A Tiny Freighter (12 s) | It launches |
| | 9. Export your ore | Send ore to the Hub → **ambush** → the Freighter flees home | The retreat is reported |
| | 10. Build an Interceptor | Shipyard II, then an Interceptor | It launches |
| | 11. Defeat the privateer | Send the shipment again under escort; the Interceptor fights on its own | The defeat is reported; 4,500 Cr bounty |
| Expansion | 12. Start research | Academy; pick a programme | As v3 |
| | 13. Expand mining operations | Solar Array, then a Mining Complex on Planet III; staff it | The new mine is staffed |
| | 14. Chart a nearby system | Survey Probe | As v3 |
| Reach | 15. Build a Mining Barge | | As v3 |
| | 16. Mine the belt | | As v3 |
| | 17. Bring the ore home | | As v3 |

**The food chain** (steps 1–3)
- The Bioharvester harvests Biomass from the home planet's deposit.
- The Agroplex turns it into Provisions (1 Biomass → 1.2), which the settlers
  eat: about 5 a minute at 2,000.
- Out of food, both work at half rate, so hunger is recoverable.
- With the Habitat between them, the food seed rises from 15 Provisions
  (about 3 minutes) to about 40.

**The reveal** (step 5)
- **Trigger:** the first build order that uses semiconductors. The Solar
  Array costs 8, so that's it.
- **Effects:**
  - the order is refused and nothing is spent;
  - the kit's semiconductors are condemned and removed;
  - a story card names the culprit: the Authority's contracted outfitter, the
    Calder Combine (name open);
  - the free trader drops out of warp about 30 seconds later with crude but
    working semiconductors, on its phase 1 terms (Hub +50%; buys Ferrite Ore
    at half price).

**The ambush** (step 9)
- The first export leaves unescorted.
- As it clears the gravity well, the Rogue Privateer appears ahead.
- The Freighter turns for home at once: a scripted retreat with no losses, the
  ore still aboard.
- The privateer breaks off and lurks on the Hub lane.
- The guide: "Pirates on the Hub lane · build an Interceptor".

**The escort** (steps 10–11)
- Interceptors keep today's Shipyard II gate.
  - Step 10 upgrades the Shipyard (40 Alloys, 20 Machinery, 15
    semiconductors; 20 s).
  - Then it builds the Interceptor (20 Alloys, 12 semiconductors, 15
    Armaments, a Warp Core; 10 s).
- Step 11 is today's privateer fight: an escorted shipment draws it, the
  Interceptor engages on its own, and the bounty lands with the report.

**Expanding the mine** (step 13)
- **The land grant:** during the charter, building is limited to the home
  planet until step 13. In the story, the charter's land grant covers the
  landing world, and the Authority opens the rest of the system with the
  expansion.
  - Recommended: a hard rule. A build anywhere else is refused with that
    reason, and the planet view shows the other worlds locked.
  - Bots follow it too.
- **The step:** a Solar Array on Planet III first, since it's a separate grid
  with no free power, then a Mining Complex beside it. Staff it from the
  migrants the Habitat has drawn.
- **It teaches** that every planet needs its own power.
- **Costs:**
  - Solar Array: 30 Alloys, 12 Machinery, 8 semiconductors;
  - Mining Complex: 25 Alloys, 12 Machinery.

**Semiconductors replace Electronics**
- **Display only:** "Semiconductors" everywhere Electronics shows today, and
  the Electronics Fabricator becomes the Semiconductor Fab.
- **Code unchanged:** the internal slug stays `electronics`.
- **"Crude"** is story text only; they're ordinary Semiconductors.

## Kit and money

The kit, in the Landing Command's holds, covers every charter build except
semiconductors:

| Good | v3 kit | v4 kit | For |
| --- | ---: | ---: | --- |
| Alloys | 70 | 390 | Bioharvester 25, Habitat 30, Agroplex 30, mine 25, Solar 30, Shipyard 40, Freighter 20, Shipyard II 40, Interceptor 20, Academy 25, Planet III Solar 30 and mine 25, Probe 6, Barge 40 |
| Machinery | 27 | 155 | Bioharvester 12, Habitat 8, Agroplex 15, mine 12, Solar 12, Shipyard 20, Freighter 8, Shipyard II 20, Planet III Solar 12 and mine 12, Barge 20 |
| Polymers | 0 | 40 | Habitat 20, Freighter 8, Barge 12 |
| Armaments | 0 | 15 | Interceptor |
| Warp Cores | 1 | 3 | Freighter, Interceptor, Barge |
| Provisions | 15 | 60 | the food seed 40, Academy 20 |
| Semiconductors | 0 | 40 (fake) | the reveal |

**Storage:** about 730 units with the fuel seeds, so the holds go to 900.
The first four builds draw about 180 within two minutes, and the reveal frees
40.

**Semiconductors from the trader** at about 51 Cr each:
- before the bounty: Solar 8, Shipyard 15, Shipyard II 15, Interceptor 12,
  for about 2,550 Cr;
- after it: Academy 15, Planet III Solar 8, Probe 4, Barge 6, for about
  1,680 Cr.

**Cash:**
- The 2,000 Cr start plus stipends must cover the 2,550 before the bounty.
  Stipends retune across the new steps.
- Selling ore to the trader is the fallback.
- The 4,500 Cr bounty funds the rest.

## Expected first session (estimates)

| ≈ Time | Step |
| --- | --- |
| 2 min | Food chain, Habitat and mine done |
| 2.5 min | Solar Array ordered → reveal |
| 3 min | Trader docks; semiconductors bought |
| 4.5 min | Solar Array and Shipyard built, staffed |
| 5 min | Tiny Freighter launched |
| 6–7 min | Export ambushed; Freighter back in orbit |
| 8 min | Shipyard II and Interceptor |
| 11 min | Privateer defeated, bounty paid |
| 12 min | Research started |
| 13.5 min | Planet III mine online |
| 17.5 min | Nearby system charted |
| 18.5 min | Mining Barge launched |
| 30–33 min | Belt ore home: charter fulfilled |

## What changes in code

- **Sim:**
  - the home bootstrap lands only the Command Center, with storage and a
    1 MW reactor;
  - no free power on any planet (`BASE_MW` removed);
  - a second home Ferrite Ore deposit, on the habitable planet;
  - no starting fleets;
  - the new kit and food seed;
  - the reveal (a founding flag, a refused build, an event, the trader call);
  - the unescorted ambush and its scripted retreat;
  - 17 charter steps with retuned stipends;
  - the charter land grant (builds limited to the home planet until step 13);
  - the camp's Storehouse project;
  - the lost-Freighter replacement retired.
- **Server:** protocol v51, timeline lines, the admin funnel keys.
- **Client:**
  - the guide's 17 steps;
  - other worlds shown locked until the expansion;
  - the story card;
  - the "Semiconductors" label and the building name;
  - storage and power shown on the Landing Command;
  - the welcome text (the colony ship becomes the Command Center; the
    outfitter).
- **Bots:** the same opening. Their merchant buying covers semiconductor
  shortfalls; the opening script needs the new order and the escort.
- **Tests:** founding, opening, settlement, power, storage, merchant, bots,
  guide, wire and funnel.
- **Everywhere else:** every colony and outlying planet now needs a power
  source.
  - A colony's own Command Center gives its planet 1 MW.
  - A mine or factory on any other planet needs a plant there first.
  - Bots and the balancing harness need a check.
- **Galaxy:** needs `--reset-galaxy`.

## Decisions (recommendation first)

1. **Home ore:** decided and built. A basic deposit on the home planet;
   Planet III keeps its own.
2. **The holds:** 900 storage, fitting the kit; or 500 with a leaner kit,
   buying the later chapters' parts as they come.
3. **The Interceptor's gate:** Shipyard II, as today; or allow Interceptors at
   Shipyard I.
4. **Armaments:** in the kit; or bought from the trader.
5. **The retreat:** scripted, with no losses; or a real chase, with risk.
6. **After the privateer:** keep research → expand mining → survey → Barge →
   belt → haul in the charter (about 30–33 minutes); or end the charter after
   "Expand mining operations" (about 15 minutes) and move the survey and Barge
   to the next chapter.
7. **The food seed:** 40 Provisions; or 15.
8. **The land grant:** a hard rule until step 13; or soft, where only the
   guide and Planet III's lack of power hold players back.
9. **Unchanged from the first draft:**
   - "Semiconductors" is display only;
   - "crude" is flavour only;
   - the supplier is the Calder Combine.
