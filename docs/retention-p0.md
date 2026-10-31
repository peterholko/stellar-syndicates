# Retention P0: tier curve, earned standing, async clocks, leave (Oct 22 2026)

The first fixes from the Oct 22 "After the Charter" review
(https://claude.ai/artifact/UomnmWZB4SkjDD3a12wRsH). R2 (every research
programme does something) is written up in docs/research-wiring-r2.md.
Protocol v58 for R1/R3/R4, v59 with R2. No galaxy reset is needed: every new
persisted field has a serde default.

## R1 · Tiers cost more and take longer

Tiers I–II are unchanged. From tier III, an upgrade's cost and build time
climb a curve (`TIER_COST_MULT` / `TIER_TIME_MULT` in `crates/sim/src/build.rs`):

| Tier | Cost | Time | 20 s base becomes |
|------|------|------|-------------------|
| III  | ×1.6   | ×20    | ~7 min |
| IV   | ×2.56  | ×180   | 1 h |
| V    | ×4.10  | ×900   | 5 h |
| VI   | ×6.55  | ×2,700 | 15 h |

- `recipe_for_tier` returns an owned, already-scaled `TierRecipe`, so every
  caller (charge, refund, queue estimate, bots) uses the same numbers.
  Cancelling refunds what the order actually cost.
- Exempt: Command Center, power plants and orbital stations. Each already has
  its own explicit level recipe.
- The client mirrors the curve in `structureLevel` (`TIER_CURVE`,
  `TIER_CURVE_EXEMPT` in `client/src/core/derive/market.ts`).
  `test-planet-panel` checks parity with the server.

## R3 · Rewards credits can't buy

Standing runs to 200. Full standing is 100, and buying standing back stops
there. Above it are four grades, which can only be earned
(`crates/sim/src/tca.rs`):

| Grade | From | Freight fee | Lift per departure | Board |
|-------|------|-------------|--------------------|-------|
| Chartered    | 100 | ×1.00 | 400 | 4 |
| Trusted      | 125 | ×0.85 | 500 | 4 |
| Preferred    | 150 | ×0.70 | 600 | 5 |
| Commissioned | 175 | ×0.50 | 800 | 5 |

- **Allowance.** Past full standing, a corporation earns at most 8 standing per
  sim day from all sources together (`Corporation::earn_standing`). Below full,
  standing earned is uncapped.
- **Sources.** Board contracts (their own standing), the board's daily bonus
  (+3 standing and a 5% research dossier), Signals, and charter-ledger goals
  (+3 standing plus a dossier sized to the goal). The board and ledger no longer
  pay credits.
- **Dossiers.** `grant_dossier` advances the active programme by a fraction of
  its time.
- **Research grants** are the credit sink: 1,000 Cr × tier buys one research
  hour on the active programme, up to 3 a day. The Fund button sits under the
  active programme, on both shells.
- **Freight lift.** The per-owner shipment cap is applied in `load_shipments`.

## R4 · Async territory clocks

The playtest-speed clocks are now hours, set in `SimConfig` with serde defaults:

| Clock | Was | Now |
|-------|-----|-----|
| `siege_capture_secs` | minutes | 6 h |
| `war_notice_secs`    | 2 h const | 2 h (config) |
| `reprisal_secs`      | 12 h const | 12 h (config) |

- The station repair clock stays battle-scaled
  (`station_repair_clock_secs`, `REPAIR_CLOCK_BATTLE_MULT`).
- `test_world()` keeps short clocks so siege tests stay fast.

## R4 · Leave (vacation mode)

`crates/sim/src/leave.rs`. This follows Travian's vacation mode.

- **Length.** 2–15 days, then a 7-day cooldown. Leave can't start mid-battle or
  under a blockade.
- **Orders are frozen.** `World::apply` refuses every order from a corporation
  on leave (`OrderRejectReason::OnLeave`), except `SetLeave` and bookkeeping:
  dismissing lost orders and signals, and annotating exploration
  (`Command::allowed_on_leave`).
- **Shielded both ways.** It can't attack and can't be attacked.
  `allow_pvp_order`, `diplomacy_protects`, `blockade_allowed` and the
  pirate-raid filters all count leave.
- **Automation keeps running.** Production, freight routes and standing rules
  continue, so an absence costs progress but never the colony.
- **UI.** The Profile dialog has a Leave section with a live countdown to the
  earliest return and to the next allowed start (`client/src/shell/account.ts`).

## Tests

- **Sim:**
  - `tiers_past_two_cost_more_take_longer_and_refund_what_they_cost`
  - `async_clocks_default_to_hours`
  - `leave_runs_two_to_fifteen_days_then_cools_down_a_week`
  - the leave, earned-standing and research-grant world tests
  - the grades test in `tca.rs`
- **Client:** `test-planet-panel` (tier parity), `test-medals` (board line),
  `test-accounts` (Leave section).
- **Fixture:** `client/scripts/standing-fixture.html` (Faction button) previews
  the grade section.
