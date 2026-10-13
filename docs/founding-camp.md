# Founding camps and settlement choice

## Entry flow

New human corporations get a shared desktop/mobile, four-scene illustrated
prologue before the existing world-profile choice and settlement confirmation:

1. **Beyond the Milky Way:** humanity opens a wormhole to another galaxy.
2. **Your stellar charter:** a corporation receives the right to settle a planet.
3. **A world to call home:** two thousand settlers begin a small frontier colony.
4. **Stay vigilant:** pirates and privateers from other charters threaten frontier trade; protect your ships and your new home.

Each scene has a new original illustration, responsive 768/1280/1672px WebP
variants, separate readable captions and descriptive alternative text. Native
masters and generation prompts are preserved in
`client/art-src/founding-intro-v1/`. The art is contained without cropping on
desktop and phone screens; no story text is baked into the paintings.

Slides advance manually with Back / Continue; Skip story and Escape go to the
profile choice, not straight into the simulation. Selection and back navigation
are local drafts. Reading can continue offline, but landing confirmation
requires a connection. Reconnection retains the current slide or profile draft.

The authenticated account's corporation is created only after **Confirm
settlement**. Reading the introduction does not consume that corporation's
supplies or founding protection. Confirmation is idempotent across repeated
clicks and reconnects; an accepted, pending landing command also survives a
checkpoint. The client closes the introduction on its received colony report,
not an optimistic timer.

## One home, three profiles

These are alternative profiles for the allocated home world, not three free
colonies. Whole-system ownership and galaxy geography are unchanged. No extra
habitable planets are generated. All profiles retain the same starter package:
2,000 people, 2,000 credits, Tiny Freighter and Interceptor, plus 70 Alloys,
27 Machinery, 15 Provisions, 20 Uranium, 10 Deuterium and 1 Warp Core (charter
v3 folded the retired recovery cache's 20 Alloys and 4 Machinery into the kit;
the fuel overhaul added the Uranium Mill's goods and the barge's Warp Core).

The four existing starting structures are gathered on the settled world:
Habitat, Warehouse, Bioharvester and Agroplex. Ferrite extraction is available
there, but the player must still build and staff the Mining Complex. No
Shipyard is granted. Camp accommodation initially holds 3,000 people and its
Warehouse holds 250 goods.

The first settlement is always a habitable world: the home's garden, the
habitable world with a Biomass deposit for the Bioharvester, so every camp can
feed itself. Every generated home has one. The home's Ferrite deposits move to
the camp world, and its free resource slot stays open for the Mining Complex. A
seven-seed, six-join test covers this for every starting slot, including one
that previously starved. Galaxies from before this change are not supported;
regenerate them with `--reset-galaxy`.

### Food reserve

While the founding programme runs, and afterwards until the storehouse is
expanded, a camp keeps a food reserve instead of filling its 250-unit Warehouse
with food: the larger of ten minutes of demand and 40 units of Provisions and
Biomass together. At the reserve the Bioharvester pauses (and an Agroplex with
nothing to process pauses with it); harvesting resumes below 75% of the
reserve. The pause is quiet, with no timeline entry, and the planet panel shows
it as **Food reserve full · paused**. The camp stays well supplied, and the
freed space is left for ore and returning cargo.

Completing the free prospecting project reveals the chosen opportunity:

| Profile | Local opportunity |
| --- | --- |
| Mineral Highlands | A finite 3,000-unit Cuprite seam, richness 0.25 |
| Verdant Lowlands | 25% more Biomass deposit output |
| Broad Coastal Plains | One additional infrastructure slot |

All retain access to farming, mining and later industry; profiles are not
research branches or permanent corporation classes. Their comparative value
still needs player testing rather than a claim of proven balance.

The landing cards and settled planet share the same scenery: Mineral Highlands
uses **Highland Plateau**, Verdant Lowlands uses **Temperate Basin**, and Broad
Coastal Plains uses **Archipelago World**. Highlands now also has a living
terrestrial globe in System View, rather than a barren one.

Ordinary habitable land worlds receive stable visual variety from Highland,
Savanna, Volcanic Highlands, Boreal and Temperate landscapes; habitable ocean
worlds use Archipelago. This is client-side art selection from public identity,
not extra planet types or undisclosed resource hints. Existing saves need no
migration. Each panorama has 768/1280/1672px WebP versions and fixed terrain
anchors for structure overlays. Nonhabitable scenery and gas-giant globes stay
unchanged. See `client/art-src/planet-view/habitable-v1/README.md`.

## Optional first-session projects

| Project | Goods | Work time | Result |
| --- | --- | --- | --- |
| Prospect the home world | None | 60 seconds | Profile opportunity; prerequisite for surface workings |
| Open surface workings | 8 Alloys, 2 Machinery | 120 seconds | 50% more Ferrite deposit output; requires a built mine |
| Expand the storehouse | 7 Alloys, 1 Machinery | 90 seconds | Camp Warehouse capacity rises from 250 to 700 |
| Build permanent quarters | 5 Alloys, 1 Machinery | 120 seconds | Base accommodation rises from 3,000 to 6,000 |

Projects appear in the planet panel and the parallel founding goals. They are
not additional mandatory export milestones. They use unreserved local goods
and share the ordinary one-active-structure-per-planet FIFO construction queue.
Costs are charged once at accepted enqueue, not again when a queued job starts.
Order delivery, build progress, completion notices and resulting capacity
changes follow the normal command/report light delays. Research-gated structure
tiers remain gated; these projects do not grant Tier II structures.

All four projects plus the mine and Service Berth fit within the initial kit
(50 Alloys and 19 Machinery), in any order. Project costs do not create an early
requirement to import materials before the opening can be completed.

Storage does not increase loading speed. Housing does not grant people or
workers: provisions, jobs and real migrant arrivals still govern growth.
Ordinary research capacity multipliers continue to apply to migration;
the body's exposed `population_capacity` field is its base accommodation.

## Compatibility and rollout

Camp data defaults to absent for old saves. Existing corporations keep their
capacity and do not see the introduction or receive duplicate starter grants.
The existing bot bootstrap remains unchanged; no balancing campaign was run
for this work. Client and server use protocol version 40 and must be deployed
together. No live restart, galaxy reset or deployment is included.

Tests cover explicit creation, duplicate confirmation, saved pending commands,
old colonies, population/world budgets, project affordability and reservations,
one-time effects, FIFO construction, command/report delays, client confirmation,
and binary protocol round trips. Desktop and phone previews use isolated data,
not the live galaxy. Longer first-session engagement remains a playtest goal.
