# R2 · Every research programme does something (Oct 22 2026)

Peter: "I don't want any research cut, find a way to incorporate into the
gameplay, either add more whatever ships, structures or modules if needed."

A scan found 53 visible programmes whose effect nothing read:

- 51 mod keys or flags with no reader outside `research.rs`;
- Signal Libraries and ECM Emitters, whose bucket keys were declared but unread.

Every one now has a real effect. The guard test
`research::tests::every_visible_effect_has_a_reader` scans the sim and server
sources and fails if a visible programme grants a key or flag that nothing
reads. Each blurb now states exactly what the programme does.

- **Protocol:** v59; v60 with the once-hidden eight.
- **Save compatibility:** no reset needed; every new persisted field has a serde
  default.
- **Tests:**
  - `crates/sim/src/world/research_wiring.rs` (17 tests);
  - the kernel test `research_scales_what_a_side_deals_and_takes`;
  - the view test `intel_research_reshapes_what_a_far_contact_shows`.

## Designations

`SetDesignation { cap, target }` points a researched capability at a target.

- **Targets.**
  - A body is named `Body { system, body }`, since body ids are per system.
  - A system pair is `SystemPair(a, b)`.
  - A fleet is `Fleet(id)`.
- **Rules.**
  - The target must be held: an owned system, both stream ends, or an owned
    fleet.
  - Body and stream designations stay put for 7 days (`DESIGNATION_LOCK_S`),
    unless the target is lost.
  - Mimicry can move freely.
- **Timeline events.** `DesignationSet` / `DesignationRefused`.
- **View.** `ResearchView.designations` serves each one as live/locked.
- **UI.**
  - Crown buttons on the world panel.
  - "⇄ system" buttons under Mass Stream on the system page.
  - "Disguise this fleet" in Fleet details.
  - The mobile sheets have the same controls.

## Economy (Materials, Computation)

| Programme | Effect | Where |
|---|---|---|
| Bulk Storage | +50% Warehouse and Orbital Warehouse capacity, not landing holds | `StarSystem.storage_mult`, refreshed each tick |
| Autoforges | Machine Works draw 20% fewer inputs | `ProductionMods.input_mult` |
| Trace Refining | Mining Complexes on metal ores add 5% Rare Elements | `ProductionMods.traces` |
| Prospecting Charters | A new or captured system's hidden belt pockets are all reported home | `chart_new_holding` |
| Mantle Taps | Poor ground mines as Average; Deep Deposits lose their tier lag | `GeologyMods.floor_average` |
| Crown Vein | One designated world mines one geology band richer | `GeologyMods.crown` |
| Slag Reclamation | Smelters emit +0.10 Silicates per unit | `ProductionMods.byproduct` |
| Flash Forges | Each world builds 2 different structures at once | `update_structure_queues` |
| Crown Project | One designated world: +1 Industrial and +1 Infrastructure site | `Body.crown_project` |
| Survey Protocols | Surveys and prospecting passes run 25% faster | `survey_secs_for`, `prospect_secs` |

Notes:

- **The view mirrors the tick.** The factor chain shown in the view uses the
  same `ProductionMods` as the tick: input multiples, by-products, trace
  output, Civic Rations and geology.
- **The client uses the served values.** Its planning math reads `input_mult`,
  `storage_mult` and `structure_slots`.

## Life

| Programme | Effect |
|---|---|
| Hydroponics, Gene-Tailored Crops | Agroplex output ×1.15 / ×1.25; Gene Crops −20% Biomass |
| Civic Rations | Rationing efficiency 0.85 → 0.92 |
| Heavy Lifters | Colony ships land 2,000 settlers, not 1,000 (whole cohorts) |
| Cryo Berths | Colony ships land with a Habitat I |
| Generation Charters | Colony ships land a specialist and 30 min of Provisions, WellSupplied |
| Xenoacclimation | Your Marginal worlds count as habitable: +1 Infrastructure site, Bioharvesters, Agricultural Export (`Body.acclimated`) |
| Expert Training | Academies raise a specialist to an expert. The course is 80 s and costs 30 Provisions, 20 Semiconductors and 5 Precision Components; the specialist is in school meanwhile. An expert lifts a full line to ×2.25 |
| Broad Curricula | Academies teach 2 courses at once (one without it) |
| Twin Campuses | Specialists housed per Habitat or Terraces tier: 3, not 2 |
| Founders' Institutes | Every course graduates an expert |

New rules that came with the Talent school:

- **Experts.** They are `SpecialistKind::Expert*` variants: trained, never
  hired at Sol.
- **Skill points.** A specialist is worth 3 points and an expert 5
  (`production::skill_points`). Lines with no experts read exactly as before.
- **Housing.** Only `specialist_housing()` specialists work: 2 per Habitat or
  Living Terraces tier. Bots train within that limit.
- **Course line.** Academy courses now run one at a time (two with Broad
  Curricula), queued like structures.
- **UI.** Specialist steppers are in the world panel's line detail, and Academy
  courses (train, raise to expert) are in its Academy detail. Both are on mobile
  too.

## Logistics (Propulsion)

| Programme | Effect |
|---|---|
| Express Charters | Player freighter hulls +25% speed |
| Long Patrol | Warship hulls burn 30% less on cruise |
| Mass Streams | One designated pair of your systems. Your fleets inside its 3,000 su corridor, bound for an end, fly at ×1.4. A held blockade at either end, or an interdiction field, stops it |
| Fleet Tenders | Already unlocked the Fuel Transfer Rig. Its redundant flag was removed; `Cap::FleetTenders` stays in the enum |
| Underway Refit | A rigged tender at rest mends its owner's hulls at rest within 200 su, outside combat. It works at 6 hp/s per rig and costs 1 Alloy per 80 hp |

The client mirrors the freighter speed and warship burn
(`core/derive/fleet.ts`), and the bots mirror the burn too.

## Combat (Weapons, Hulls)

The effects ride `tactical::SideMods` as new fields. An unset field (0) reads as
1, so an unresearched side fights bit-for-bit as before. The pinned v16
fingerprints are unchanged, and older recordings replay. No new rules version
was needed. `World::battle_research` fills the fields.

| Programme | Effect |
|---|---|
| Fire Control, Magnetic Accelerators, Penetrator Slugs, Warhead Yields | Beam / driver / torpedo damage ×1.10 (drivers ×1.265 with both) |
| Fire Discipline | ×1.10 damage in the opening window |
| Ablative Refits, Spall Liners | Reflective blunts 60% (was 50%), Whipple 70% (was 60%) |
| Reinforced Frames, Compartmentalization, Citadel Compartments | Armed hulls take ÷1.10, ÷1.10, ÷1.20 damage |
| Hardened Anchorage | Hulls fighting within 1,300 su of their own system take ×0.85 |
| Fleet Escorts | Freighters take ×0.75 while a Corvette of theirs fights |
| Wolfpack Doctrine | +15% Raider damage per further Raider fleet on the side, to ×2 |
| Slip Anchors | Withdrawing Raiders take ×0.5; all-Raider fleets slip away in half the exposure time |
| Prize Holds | Blockade plunder strips 50% faster, and a warship carries 90 units, not 60 |
| Rapid Rearm | Yard repairs ×1.25 |
| Modular Berths | Refits take half the time |

## Intel (Computation)

| Programme | Effect |
|---|---|
| Signal Libraries / ECM Emitters | The size estimate reads one step finer or coarser (`GhostView.count_range`). Under ECM the true bucket never ships |
| Counter-Intelligence | Rival Signal Libraries, Traffic and Wake Analysis read nothing on your fleets |
| Traffic Analysis | Unrevealed freighters show `manifest_class`: goods, personnel, modules or empty |
| Wake Analysis | Seen rivals show `wake`: fuel in quarters and a 16-point bearing they came from |
| Signature Mimicry | The designated fleet reads as a Convoy, one class smaller, its makeup hidden outside plain coverage, until it fights |
| Standing Watch | A lost contact stays as a dead-reckoned track for 10 min (`GhostView.tracked`; the map label reads TRACK) |
| Battle Archives | Third-party records are now `archive` fidelity, with bucketed per-round losses; the theater shows the tally. Plain bucket observers no longer receive kill buckets |
| Panopticon Catalog | Every owned system and fleet catalogs the battles in its sensor range |
| Survey Corps | A Scout idle in an uncharted system's well surveys it by itself |

## Gates

- **`SU_PER_LY` 40 → 2,000.** A home lies about 62 ly from the Hub, so
  Propulsion II (200 ly) takes a few round trips.
- **Life II gate.** 5 M → 0.1 M migrants (100,000).
- **Growth III gate.** 20 M → 0.3 M total population.
- **Display.** Population gates now show thousands.

## The eight once-hidden programmes (built Oct 22 2026, v60)

These were hidden on Oct 9 because their gameplay didn't exist. They now have
content, and nothing in the catalog is hidden (119 programmes, all
researchable).

| Programme | Effect | Where |
|---|---|---|
| Beneficiation | Mineral ground mines at 85% of Average at worst (Ultra-poor 50% and Poor 75% lift to 85%) | `GeologyMods.beneficiation`, `BENEFICIATION_FLOOR` |
| Predictive Plots | Moving rival contacts carry `GhostView.projected`: their sighting carried forward by velocity over the light's age. The map draws a dashed line and a ring | view.rs pass 2; `render.ts drawProjections` |
| Gravimetric Survey | Every 30 s, each system within 30,000 su of one you hold charts itself. The reading comes from the nearest held system and heads home by light with the usual survey notice | `World::gravimetric_survey` |
| Lance Array | A new module (`lance_array`). It fits destroyers up, costs 3 fitting points, deals ×1.7 beam damage a gun, and Reflective Plating's blunt applies twice to it. Made at an Armaments Complex from 6 Precision Components, 10 Armaments and 8 Alloys; making it needs the research, fitting a found crate never does | module.rs, `v5::offense`, `v12::guns`/`volley` |
| Overpressure Warheads | A torpedo kill splashes 10% of its damage into the nearest live hull of the victim's stack (same side, kind and fit) | `SideMods.splash`, `TacticalState::splash_target` |
| Salvage Rigs | Each engagement tallies the modules its lost hulls carried. A side still on the field at the end hauls a quarter of the enemy's (rounded) aboard its first fleet as crates | `Engagement.wrecked_modules`, `WrecksSalvaged` |
| Boarding Parties | When a raid strips a convoy, the freighters lost in the final blow become the raider's prize fleet at 25% hull and fuel | `World::take_prize`, `PrizeTaken` |
| Orbital Habitats | Habitats on every world you hold house at least as on a Marginal world (environment ×1.0 floor), and each gas giant you hold gains a habitat site | `Body.orbital_habitat` |

Tests:

- `boarding_parties_take_a_prize_and_salvage_rigs_haul_modules`
- `reflective_plating_blunts_a_lance_array_twice_over`
- `a_torpedo_kill_splashes_into_the_victims_stack`
- `predictive_plots_project_a_moving_contact_to_now`
- three more in `research_wiring.rs`

Two older tests that used these programmes as examples of hidden ones now use
retired ids instead.
