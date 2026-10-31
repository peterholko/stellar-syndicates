# Asteroid Quarry

Peter (Oct 24 2026): a belt shouldn't need the planet's Mining Complex; it needs
a structure built for zero-g. He then chose **one combined structure: mining,
crew quarters and power**, because it makes more sense for belts in other
systems. He showed two references: a full-grown asteroid mine, and a tier-1
example with one arm, one module, one solar panel and a docking boom.

## The plan (Oct 25 2026, supersedes the sections below where they differ)

**The structure**
- The Asteroid Quarry is the only structure built on an asteroid belt: one per
  belt, mining every deposit on it.
- It needs no workers, power plant or fuel. It pays upkeep in Provisions and
  Machinery, more at each tier. Tiers I–VI, upgraded like any structure.
- On belts it replaces the Mining Complex, Volatile Harvester and Power Plant.
  Gas giants keep the Volatile Harvester. Barges and the Mining Belt Station are
  unchanged.
- Phase 1 covers belts in your own systems. Phase 2, later, covers belts in
  unclaimed systems, as outposts.

**The look**
- The belt view keeps its painted asteroid field. A belt with a Quarry also
  shows one large asteroid in the foreground carrying the Quarry's machinery.
  The machinery grows with the tier; the rock stays the same.
- The same rock goes on every belt type.

**Peter: the art, in ChatGPT**
1. Run "Transparent background, each tier" (below) on each of the five
   `asteroid-mining-tiers-v1` images. Each comes back transparent, without the
   loose rocks or docked ships.
2. Run "The new tier 5" (below) for the tier between Refinery Complex and
   Asteroid Foundry.
3. Save the six as `client/art-src/asteroid-quarry-v2/tier-1.png` to `tier-6.png`:
   1. Prospecting Outpost
   2. Automated Mine
   3. Deep-Core Quarry
   4. Refinery Complex
   5. the new tier
   6. Asteroid Foundry

**Me: the code, phase 1, alongside the art**
1. **Sim:** the Quarry structure. Belts stop offering the three structures it
   replaces.
2. **Bots and the charter:** switch belt mining to the Quarry.
3. **Client:** the Quarry on the belt's build sheet, and the quarry rock in the
   belt view at its tier.
4. **Tests,** and a screenshot of the belt view once the art is in.

**Deploy**
- A protocol bump.
- :8080 needs a reset if its galaxy has belt structures; I ask first.
- Production gets a reset only if someone has built on a belt by then; I check
  and ask.

## As built (phase 1, Oct 25 2026)

Protocol v64. No reset needed: on Oct 25, neither :8080 (267 belts) nor
production (85 belts, 0 players) had anything built on a belt.

**Sim** (`crates/sim`)
- `StructureKind::AsteroidQuarry`, appended last to `ALL`, in the Resource pool.
  It has no research prerequisite. Tier I costs 45 Alloys, 18 Machinery and
  10 Polymers and takes 24 s; higher tiers follow the usual tier curve.
- Ground rule `StructureKind::suits(body)`: a belt takes only the quarry, and the
  quarry stands only on a belt. The build check refuses either mismatch with
  "A belt takes only an Asteroid Quarry" or "An Asteroid Quarry stands on an
  asteroid belt". `site_for` never puts a Mining Complex or Power Plant on a belt.
- A belt with ore has one Resource slot (`Body::resource_slots`), and a bare belt
  has none. Planets keep one slot per deposit, up to 4.
- Upkeep, per tier: 0.03 Provisions/s (1.8 a minute) and 0.005 Machinery/s
  (0.3 a minute), from the system's free stock (`build::QUARRY_UPKEEP_PER_TIER`).
  - `World::supply_quarries` (`world/asteroid_quarries.rs`) charges it each tick,
    before production, all or nothing.
  - Unpaid, the quarry's line reads `no_supplies` and stops, with
    ProductionSuspended and ProductionResumed events; nothing is lost.
- While paid, it extracts every deposit on its belt, plus the pockets its owner
  has found there, as a fully crewed mine of its tier would (workers = tier,
  staffing 1, food 1). It has no workforce and no power draw.
- Charter step "Expand mining" counts a quarry as the second mine
  (`founding_mine_expanded`).

**Server**
- `assignment_views` serves the quarry's line over every deposit.
- `BuildOption.upkeep` serves `QUARRY_UPKEEP_PER_TIER`.
- The timeline prints the `no_supplies` stop.
- Bots:
  - value a quarry when the home has an ore belt;
  - build one in charter expansion;
  - pick bodies with `suits`, so they never order a mine onto a belt.

**Client**
- **Belt Build sheet:** only the quarry; other structures sit under "Can't build
  here" with a ground reason.
- **Belt view:** the quarry's rock at 28 %, 71 %, 52cqw wide (`QUARRY_ADDRESS`),
  with the tier's machinery. An empty belt shows one site there.
- **Detail card:**
  - status "Automatic", or "Needs Provisions and Machinery";
  - an Upkeep row, e.g. tier III: −5.4 Provisions, −0.9 Machinery a minute;
  - no Workforce row on a belt.
- **Mobile:** the belt's section is "Production"; the quarry's row shows its
  upkeep a minute and has no crew buttons.
- **Guide step 14:** on a belt it reads "Quarry {belt}".
- **Art:**
  - `scripts/build-asteroid-quarry-art.mjs` derives `tier-N-{640,1280,1672}.webp`
    and square menu icons `icon-tier-N-{128,256,512}.webp` into
    `public/art/derived/asteroid-quarry/v2` (36 files, 3.4 MB);
  - it runs from `build-art-derivatives.mjs`.
- **Fixture:** `planet-fixture-belt` (:4182) opens a tier I quarry; add
  `--quarry=N` for tier N, or 0 for the empty site.

**Tests**
- sim:
  - `world::asteroid_quarries` (ground, refusals, paid output, unpaid stop and
    resume);
  - power grids: a gas giant takes a plant, a belt refuses one.
- server: `only_the_quarry_serves_an_upkeep_and_it_is_the_sims`.
- client:
  - `test-planet-panel` (scene, no crew, upkeep, unpaid label);
  - both structure-art tests.

**Not in phase 1:** quarries on belts in other systems (outposts). Barges and
the Mining Belt Station still mine there.

## How it plays (proposed Oct 24, not built)

- **Belt-only, one per deposit.** It works every belt deposit, including the
  Volatiles and Helium-3 ice of icy belts. On belts it replaces three structures:
  the Mining Complex, the Volatile Harvester and the Power Plant. Gas giants keep
  the Volatile Harvester.
- **It crews and powers itself.**
  - It needs no workforce, no power plant and no fuel.
  - It pays upkeep instead, as outposts do today: Provisions for its crew and
    Machinery for wear, rising with tier.
  - Today's outpost rate is 1.8 Provisions and 0.3 Machinery a minute.
- **In your own systems:** you build and upgrade it from the belt's build sheet.
  Its ore goes to the system stockpile.
- **In other systems:**
  - A freighter brings the kit and sets the Quarry up on a charted belt deposit in
    an unclaimed system. It is the belt version of today's extraction outpost
    (`DeployOutpost`, which refuses belts for now).
  - Like an outpost, it claims the system and opens a store there.
  - It upgrades from goods delivered to that store, and freighters haul its ore
    home on a route.
  - The outpost rule stays: no Quarry outpost in a system with a habitable world.
- **Unchanged:** Mining Barges and the Mining Belt Station still mine belts
  without claiming them.
- **Cost of the change:**
  - a new structure kind, a protocol bump and a galaxy reset;
  - the charter's belt branch, the bots' belt logic and the views follow.
- **Phases:**
  1. your own systems;
  2. other systems.

## The look

- **The tier-1 design is Peter's example:**
  - one articulated arm, whose bell-shaped sealed cutter head works the rock;
  - a ribbed hose carrying the cuttings to a low cream-white module (processing
    plant and crew quarters);
  - an upright solar panel on the module;
  - a long docking boom.
- **Growth:** tiers add arms, solar panels, a habitat cylinder, radiators, a
  processing deck, silos and ore containers. Tier 6 is the full-grown mine.
- **Power:** solar panels and radiators only, no reactor, since the Quarry burns no
  fuel.
- **No rock in the sprite.** The belt painting supplies the rock, and the Quarry
  stands on one of the hand-picked rock-top sites (`docs/belt-view-art.md`).
- **No ship at the boom.** Ships in the game are real fleets, so building art never
  shows one.

## Prompt

Generate tiers 1 to 6 in order. For each tier, change `N` and the "This tier
specifically" line.

**Attach:**
- **Image 1, every tier:** the tier-1 example. Keep a copy at
  `client/art-src/belt-view/asteroid-quarry-tier1-reference.png`.
- **Image 2, every tier:**
  `client/art-src/planet-surface-structures-v2/buildings/power_plant/orbital-fusion.png`.
- **Image 3, tiers 2–6:** the new tier just below.
- **Image 4, tier 6 only:** the full-grown mine reference. Keep a copy at
  `client/art-src/belt-view/asteroid-quarry-reference.png`.

**Save as**
`client/art-src/planet-surface-structures-v2/buildings/asteroid_quarry/tier-N.png`,
with a `tier-N.prompt.json` beside it in the existing format.

```text
Use case: stylized-concept.
Asset type: ONE standalone structure sprite for the asteroid-belt view of Stellar Syndicates, a space strategy game: an Asteroid Quarry, a self-contained zero-gravity mine that clamps onto an asteroid and carries its own crew quarters and power. Square transparent PNG, 1254 x 1254.
Purpose: the game places this sprite on top of a large rock in a separately painted asteroid field. Draw the quarry ONLY: no asteroid, rock, rubble, dust, cut terraces, pit, crater, terrain, base plate, stars, planet or background. Genuine zero-alpha transparency around the structure; keep a translucent soft contact shadow under its feet.
Input images: Image 1 is the approved design for tier 1: one articulated arm whose bell-shaped sealed cutter head works the rock, a ribbed flexible hose carrying the cuttings to a low module that holds the processing plant and crew quarters, an upright solar panel on its roof, and a long docking boom. Keep its design, proportions and cream-white finish at every tier, but draw no rock and no ship at the boom. Image 2 is the game's approved orbital fusion plant: match its rendering, materials, lighting and finish, not its ring or layout. Image 3, when attached, is this quarry's tier just below: keep everything in it and grow from it. Image 4, attached for tier 6 only, is the reference for the full-grown mine: take its machinery, not its asteroid, ships, drones, people or photographic look.
Zero gravity: everything is held by structure. The quarry rests on short anchor feet clamped at the foot line, its cutter heads are sealed against the rock, and ore moves only through hoses, pipes and sealed containers. No ore heaps, chutes, open conveyors, smoke stacks, sparks, beams, glow haze or flying debris.
Style: near-future frontier engineering. Soft semi-realistic 3D game rendering; cream-white and pale grey modules and arms with amber-yellow trim, graphite steel trusses and hoses, dark blue-grey solar panels, pale radiator panels, restrained warm amber window and work lights, a few cyan docking lights. Large readable forms that still read at 100 px, realistic restrained detail; no ink outlines, cel shading, plastic gloss, extreme contrast or neon. No text, logos, numerals, UI, people, vehicles or spacecraft.
Camera and lighting: elevated front-left three-quarter view, about 40 degrees above the surface the quarry stands on; mild perspective; front and right sides visible. Upper-left sunlight with crisp, soft-edged shadows and a cool neutral fill; the contact shadow falls to the lower right.
Framing/alignment (match the game's other buildings): solid structure width about 94% of the canvas (x about 3-97%), centred at x=50%: the arms on the left, the modules in the middle, the docking boom reaching the right edge. The anchor feet and cutter heads, the lowest solid points, at y about 88%. At least 3% transparent margin above the tallest point. Same footprint and overall width at every tier: new parts grow upward and fill in, so nothing shrinks. Never crop.
Output exactly one structure, no tier lineup/contact sheet.
Subject: ASTEROID QUARRY, the structure only - TIER N of 6.
This tier specifically: [line from the list below].
Tier family progression for context (do NOT draw a lineup): 1: one arm, one module with crew quarters, one solar panel and a docking boom; 2: a second arm, a second solar panel and an ore tank; 3: a crew habitat cylinder, a four-panel solar array and two radiator fins; 4: a raised processing deck, a third arm and ore containers along the boom; 5: tall ore silos, a radiator bank, a beacon mast and more anchor feet; 6: the full-grown mine of Image 4: a monumental processing block, three tall silos and four arms, on the same compact site.
Output: exactly the requested tier alone, genuine transparent PNG.
```

**"This tier specifically" for each image:**

1. exactly the design of Image 1, without its rock and ship: one articulated arm
   on a turntable at the left, its bell-shaped cutter head lowered to the foot
   line; a ribbed hose from the head to a low cream-white module with amber-lit
   windows (processing plant and crew quarters); one upright solar panel on the
   module's roof; a long docking boom reaching right to an empty berth with a cyan
   light; short anchor feet under the module.
2. keep all of tier 1; add a second arm with its own cutter head and hose in front
   of the first, a second upright solar panel beside the first, and a squat ore
   tank with amber bands beside the module.
3. keep all of tier 2; add a crew habitat cylinder with a row of lit windows,
   joined to the module by a short tube; the solar panels become a four-panel
   array on a short mast; add two tall pale radiator fins at the rear.
4. keep all of tier 3; add a raised processing deck on a truss frame over the
   modules, with a separator drum and two tanks, and a third arm; a line of sealed
   ore containers rides along the docking boom.
5. keep all of tier 4; add two tall ore silos with amber bands, a bank of five
   pale radiator panels and a beacon mast with an amber light; truss legs clamp
   down at more points around the base.
6. keep all of tier 5; grow into the full mine of Image 4 on the same compact
   site: a monumental two-level processing block with three tall silos, a fourth
   arm, a second habitat cylinder, larger solar arrays and the docking boom full
   of ore containers; dense and tall, not a sprawling base.

## Checking a result

- **Size and transparency:** 1254 × 1254 with real alpha.
- **Framing:** solid width about 94% and lowest point at about 88%. This matches
  the Mining Complex tiers (93–94% wide, foot at 88–92%), so the Quarry takes the
  same sites at the same size.
- **Tier 1:** reads as Image 1: one arm, the hose, the module, the solar panel and
  the boom.
- **Nothing extra:** no rock, terrain, pit, ships or people.
- **Growth:** each tier keeps the one below and adds its feature, and nothing
  shrinks.

## Wiring, after the art

- **Art:** `asteroid_quarry` becomes a normal six-tier family in the additions
  manifest. With no ground layer it needs no orbital-look special case.
- **Check:** use the `planet-fixture-belt` launch entry (:4182).

## The rock-included set (Oct 25 2026)

ChatGPT delivered a different set:
- **Files:** `client/art-src/asteroid-mining-tiers-v1/`, five 1672 × 941 tiers:
  Prospecting Outpost, Automated Mine, Deep-Core Quarry, Refinery Complex and
  Asteroid Foundry.
- **Content:** every tier is the same asteroid, camera and upper-left light, with
  the machinery built into the rock, on opaque black.

**Proposed fit: the Quarry is its own rock.**
- A belt with a Quarry shows the quarry asteroid as one large near rock in its
  painted field.
- It sits at a fixed spot per painting, with the rock the same size at every
  tier. The belt's six rock-top sites go.
- Drill pits, terraces and the ring only read cut into rock, which a rockless
  sprite on a painted rock can't do.
- **One rock per belt.** The plan above allows a Quarry per deposit, up to four,
  but four copies of one asteroid would look pasted. Either:
  - show one rock at the tier of the belt's highest Quarry; or
  - make the Quarry one per belt, working all its deposits.
- **Recommended (Oct 25):**
  - One Quarry per belt, working every deposit; one upgrade path, and in other
    systems one belt is one claim.
  - The stony rock on every belt type until an icy belt shows it looks wrong.
    Icy variants would then be six recolour edits.
  - The docked ships removed in the transparency edit: the game draws the real
    ones.

**The art needs two things first:**
- **Transparency:** the black can't be keyed out. The rock's shadow side (7–35
  in its darkest channel) is as dark as the space behind it (2–14), so a key
  takes the underside with it. The prompt for each tier is below.
- **A sixth tier:** the game's tiers run I–VI. A new tier between Refinery
  Complex and Asteroid Foundry keeps the Foundry at VI.

**Other limits:**
- The rock is the same grey-brown stone in every tier, so it won't match icy,
  metallic or shattered belts. Accept that, or paint per-type rocks at 4× the
  art.
- The docked ships at the booms are part of the painting, and players would take
  them for their own. The transparency edit removes them.

**Fallback with no new art:** show the tier painting full-frame as the belt's
view once it has a Quarry. It is exactly the belt view's 1672 × 941 ground
frame, but the belt's field is lost.

### Transparent background, each tier

```
Use case: precise-object-edit
Input image 1 is the EDIT TARGET. Keep everything else in it exactly as it is: the asteroid, all machinery, panels and booms, the lighting, colors, camera, framing, size and position.
Change only two things. Remove the black space so the output is a PNG with a genuinely transparent background (zero alpha) everywhere outside the asteroid and its machinery, including the gaps between ring segments, trusses and panels; remove the small loose rocks floating around it. And remove every docked ship or craft at the ends of the docking booms, leaving the empty docking clamps.
Keep the asteroid's dark shadow side fully opaque and as dark as it is now. Add no rim light, glow, outline, halo, drop shadow or new background.
Output: one 1672 x 941 transparent PNG. No text, logos or borders.
```

If transparency isn't possible: a flat pure green #00FF00 background instead,
with no green light or reflections on the asteroid or machinery.

### The new tier 5 (the Asteroid Foundry becomes tier 6)

Attach tier 4 (Refinery Complex) as Image 1 and the Asteroid Foundry as Image 2.

```
Use case: stylized-concept
Input image 1 is tier 4, the Refinery Complex. Input image 2 is the Asteroid Foundry, which becomes tier 6. Both come from the same set: the same asteroid, camera, lighting, materials and rendering.
Create tier 5, the step between them: keep everything in Image 1 at the same size and position, and grow about halfway toward Image 2. Add the first arc of the industrial ring along the near side of the asteroid (about a third of the ring, carried on trusses anchored to the rock), two more refinery towers, a deeper excavation terrace and two more solar arrays. Leave the closed ring and the central domed hub for tier 6.
Keep the same asteroid shape, camera, upper-left sunlight, cream-white machinery with amber work lights and graphite panels. Everything is attached to rock or trusses; ore moves in enclosed conveyors; no smoke or haze. No ships or craft docked at the booms: leave the docking clamps empty.
Composition: 1672 x 941, the asteroid the same size and position as in Image 1, with all machinery inside the frame and margin on every side. Transparent background (zero alpha) around the asteroid and machinery; keep its shadow side opaque; add no glow, outline or rim.
No text, captions, numbers, logos, interface, watermark or border. One image, not a lineup.
```
