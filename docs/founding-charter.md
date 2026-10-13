# Founding charter v3

Status: built Oct 8 2026, uncommitted. Protocol v43 (`stellar.msgpack.v43`):
deploy the client and server together, and start a fresh galaxy
(`--reset-galaxy`), because the starting fleet and the local survey stops are
rolled when a galaxy is generated.

This replaces the v2 opening in [local-opening.md](local-opening.md): no
starting Scout, no recovery beacon or cache, and the Growth chapter's choice
between a second Freighter and refining is gone. The rules that page lists for
storage, the Service Berth, shopping caps, refining floors, focus while landing
and the welcome-back report still hold.

## Starting fleet

A founder starts with the 2,000-population home, a Tiny Freighter and an
Interceptor. The home's Landing Command comes with a Solar Array I on its
planet ([power-plants-plan.md](power-plants-plan.md)). §fuel-overhaul (Oct 12
2026, protocol v49, [fuel-overhaul.md](fuel-overhaul.md)): the kit holds 70
Alloys, 27 Machinery, 15 Provisions, 20 Uranium, 10 Deuterium and 1 Warp Core,
and both ships arrive with full stores. The retired cache's 20 Alloys and 4
Machinery moved into it, and the Uranium Mill's 20 Alloys and 8 Machinery
joined them. The mine, the mill, the Service Berth and all four settlement
projects therefore fit the kit in any order, and no project can price the
mine out of reach. The Warp Core goes into the charter's Mining Barge. The Survey Probe and the Mining Barge are built later, at the
founder's own Shipyard, as part of the charter.

## Nine steps in three chapters

| Chapter | Step | Completes when (server facts) |
| --- | --- | --- |
| Landfall | 1. Bring the mine online | The home's site report shows a staffed Mining Complex. |
| | 2. Send your first shipment | A Freighter with Ferrite Ore, guarded by a combat hull, leaves for the Market Hub. It counts when that departure's light reaches command; the sale stays in the transaction ledger. |
| | 3. Defeat the Rogue Privateer | The privateer's defeat is reported and the 4,500 Cr bounty paid. |
| Industry | 4. Build a Shipyard | A Shipyard is built and staffed. |
| | 5. Start research | A programme is running or finished and an Academy is staffed. |
| | 6. Chart a nearby system | A survey report of the assigned stop arrives. |
| Reach | 7. Build a Mining Barge | A barge exists. |
| | 8. Mine the belt | The report that a barge is cutting ore at a belt away from home reaches command. |
| | 9. Bring the ore home | The report that a barge unloaded or delivered ore arrives. |

**Landfall** (`world/opening.rs`, `advance_local_opening`)

- **Mine:** Mining Complex I takes 25 Alloys and 12 Machinery from the
  starting kit. Staffing it is what counts.
- **Privateer:** only a guarded trip summons it: the first escorted
  Freighter to leave its gravity well (beyond the hyperlimit), which is
  normally the shipment. It appears 4,000–9,000 su ahead (at most 60% of the
  leg) and 6,000 su to one side.
- **Lost Freighter:** if the Freighter is lost before the shipment leaves, the
  Authority charters one replacement, as before.

**Industry**

- **Shipyard:** 40 Alloys, 20 Machinery and 15 Electronics. The shipment's
  draft route sells the ore and books part of the kit home. The guide sends
  the rest through the Market on Authority freight.
- **Research:** an Academy costs 25 Alloys, 15 Electronics and 20 Provisions.
  The founding grant funds most of the first programme. The founding also
  pays that programme's goods basket until it finishes, even though the step
  completes once research starts.
- **Survey:**
  - A Survey Probe costs 6 Alloys and 4 Electronics and builds in 10 s; it
    fills its tanks at the dock (§fuel-overhaul: new hulls launch empty).
  - It charts one stop, chosen by `founding_lesson_stop`: the home's local
    survey stop within 34k su, preferring the one with an ore belt.
  - Each home's rocky stop is rolled with one (`local-stop-belt-v1`).
  - If the survey finds no belt within reach, the charter completes here.

**Reach** (`world/founding_belts.rs`)

- **Barge:** a Mining Barge costs 40 Alloys, 20 Machinery, 12 Polymers and
  6 Electronics, needs Shipyard I and builds in 30 s.
- **Mining:** the barge mines the belt the survey charted. With a full hold, it
  can get home from about 34k su, no farther.
- **The haul:** `note_founding_haul` fires when a barge unloads ore at a dock
  (`apply_logistics_unload`), or delivers it on a mission, including to the
  Market warehouse. The step completes when that report's light arrives.
- **Home-system belt:** a belt in the corporation's own system mines straight
  into the stockpile, so it completes the step at once.

## Stipends

| Stage reached | Title | Stipend |
| --- | --- | ---: |
| Shipment step | Mine online | 300 Cr |
| Privateer step | First shipment under way | 500 Cr |
| Research step | Shipyard staffed | 400 Cr |
| Survey step | Research under way | 300 Cr |
| Barge step | Nearby system charted | 500 Cr |
| Mining step | Mining Barge launched | 300 Cr |
| Haul step | Belt mining under way | 300 Cr |
| Complete | Founding charter fulfilled | 800 Cr |

Plus the 4,500 Cr privateer bounty: 7,900 Cr in all (`founding.rs`,
`charter_stipend`).

## After the charter

- **Charter ledger:** opens when the charter is fulfilled, not at the first
  export. Goals already met when it opens are paid. There are now 23
  goals; "Run two Freighters" (400 Cr) carries the old second-Freighter step.
- **Next chapter** (`core/derive/handoff.ts`): two goals, and both are
  navigation only.
  - **Found your first colony:** chart a habitable world, build a Colony Ship,
    claim the system.
  - **Reinforce your fleet:** Shipyard II, then a Corvette.

## The guide

`foundingPlan` (`client/src/core/derive/founding.ts`) shows the chapter's
checklist, what is under way, and one next step. One-click actions send:

- `survey-stop`: SurveySystem to the stop;
- `mine-belt`: MineBelt;
- `barge-home`: MoveShip home;
- `barge-unload`: SystemUnload at home.

The guide never sends the same trip twice while an order is in flight. A
Scout dwelling on an expedition does not count as surveying, and it is never
pulled off. A free Survey Probe is preferred over a Scout.

## Retired stages

Old saves are not migrated: start a fresh galaxy. The retired enum variants
remain only so serialized stages still parse, and each hands on to its
successor:

- `LocalRecovery` and `ExportProduction` go to `PrepareExport`.
- `CompleteExport`, `GrowBusiness` and `BuildSecondFreighter` go to
  `BuildShipyard`.
- `BuildAcademy` goes to `FirstResearch`.
- `BuildScout` goes to `SurveyCandidates`.
- `BuildColony` and `EstablishColony` go to `Complete`.

## Bots, funnel and harness

- **Bots** (`bots/opening.rs`, `belt_lesson`):
  - they mine with a barge;
  - they send it home once it holds 30 or more ore, or is full;
  - they unload it there.
- **Admin funnel** (`crates/server/src/tutorial.rs`): the steps are sign up,
  enter, found, then the nine steps above.
  - Keys: `mine`, `shipment`, `privateer`, `shipyard`, `research`, `survey`,
    `barge`, `belt_mining`, `haul`.
  - The shipment, privateer and haul steps also read their own report stamps.
- **Harness:** `cargo run --release -p sim --example opening_balance` plays
  Industry and Reach from a staffed-mine, Shipyard-stage baseline. The charter
  completes at about 28 game-minutes.
