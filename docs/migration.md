# Migration

Built Nov 1 2026 after Peter asked when migrants arrive and how a player gets
more ("Yes fix it"). Protocol v82, no galaxy reset.

## How settlers come

- Population grows only when a liner lands. A liner is an Authority Freighter
  carrying one 1,000-person cohort (`MIGRANT_COHORT_PEOPLE`). It is a real
  hull: it can be intercepted, and a blockade stops new ones.
- External settlers come from the Wormhole Hub. A home 125,000 su out is
  about 14 minutes away (32 su/s, warp ×5, thrusters inside the well).
- One allocation per corporation every `MIGRATION_BASE_INTERVAL_S` (360 s)
  × the policy (Priority 0.75) / (world appeal × Med Bays × the syndicate
  Migration bonus × Boom Charters below half housing). A Terran home is
  about 5 minutes. At most 3 liners in flight per corporation.
- The pick is the best-scoring world (policy weight, appeal, free housing).
- Liners land only if housing still has room; otherwise one turns back.

## The gates (`migration::candidate`)

In order; the Population tab names the first that holds:

| Hold | Shown | Meaning |
| --- | --- | --- |
| `closed` | Closed | The policy is Closed. |
| `unsettled` | Uninhabited | Nobody lives here; a colony ship founds it. |
| `blockaded` | Blockaded | A held blockade on the system. |
| `food` | Needs food | The system is below Well Supplied. |
| `housing` | Housing full | No room for a cohort once inbound liners land. |
| `crew_free` | Crew idle | Managed: a crew in the system is not posted. |
| `staffed` | Posts staffed | Managed: crews on hand and inbound fill every post. |

Managed counts crews already aboard liners bound for the system, so one
empty post calls one liner (before Nov 1 it could call three).

`World::migration_pick` and `depart_migrant_liners` use the same gates.

## Player levers

- Policy per world: Closed, Managed (default: fill empty posts), Open
  (whenever housing and food allow), Priority (picked first, 25% faster).
- Posts: every new crewed building or level is an empty post for Managed.
- Housing: Habitat levels, Living Terraces, Dense Housing (+25%).
- Speed: Med Bays (+10%), Boom Charters (+20% under half full), the
  syndicate Migration bonus, world appeal (Gaia 1.5, Terran 1.15, Marginal
  0.85, Hostile 0.55).
- Relocate 1,000 between own colonies; colony ships land 1,000 (2,000 with
  Heavy Lifters).

## What the player sees

- Population tab, both shells (`core/derive/migration.ts`):
  - **Housing** "7,000 / 12,500" (with Dense Housing).
  - **Immigration**, one line: "Liner lands in 9m", "2 liners · first lands
    in 2m", a hold above, "Next liner in 4m" (this world is next), or "Queued"
    (another world is next). The why is the tooltip.
- `BodyView.migration` (`MigrationView`, owner only): `housing`, `liners`,
  `lands_at` (whole seconds), `hold`, `next_at`. Built in the game loop from
  the arrived report of the system, the corporation's research, and the Hub
  timetable (`World::migrant_lands_at`, `World::migration_due_at`).
- Log (kind production, system link): "+1,000 people · Nergal III",
  "Relocated 1,000 · Nergal III", and a warning "Liner turned back · Nergal
  III · no housing for 1,000". Like lines within 15 minutes collapse (×N).

## Tests

Sim: `managed_counts_inbound_crews_so_one_empty_post_calls_one_liner`,
`a_liner_lands_on_its_timetable_and_logs_the_landing` (a full Hub-to-home
trip landed at the timetable's 827.3 s). Server:
`the_home_world_serves_its_housing_and_liner_status`,
`timeline::tests::migrant_landings_and_turnbacks_are_the_owners_news`.
Client: `client/scripts/test-migration.mjs`.
