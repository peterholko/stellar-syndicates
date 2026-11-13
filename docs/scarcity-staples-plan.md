# Scarcity and staples: implementation plan

> **Superseded, Nov 11 2026.** Peter rejected the global multiplier: income
> and costs should both climb the tech ladder (basic gear early, Gale and
> Maelstrom drives, dearer guns and bigger hulls later), balanced stage by
> stage from the charter's end to the season's end, across a growing empire.
> K and its charter shields were backed out the same day. **What stays:** the
> Authority's claims on new colonies (step 1.7), the bots buying them, and
> the harness's daily ledgers. The record below is kept for its data.

Nov 10 2026. Step 1 is built (see Status); steps 2–3 are plan only. It turns step 1 of the
[overproduction review](overproduction-fable-review.md) and the regional
staples from the Nov 10 trade debate into exact changes. Wear and tear stays
held back, and so do bots trading on the Exchange.

Three things need Peter's OK: each bot harness run (steps 0, 1 and 2), the
galaxy reset at step 2, and the decisions at the end. **Peter said yes to all
of them on Nov 10** (the runs at steps 0–2, the reset at step 2, and
decisions 1a–5).

## Status, Nov 10 (uncommitted)

**Step 1 is built** with K switched off: `SimConfig.cost_scale` defaults
to 1, today's economy, so :8080 and production are unchanged until K is
picked. The switches are server env `COST_SCALE` (fresh or resumed galaxy)
and harness env `BOT_BALANCE_COST_SCALE`. Where the build differs from the
plan below:
- **K is config, not a constant.** At K 1 the gentle curve stays
  (`TIER_COST_MULT`); above 1, scaled builds climb `STEEP_TIER_COST_MULT`
  `[1, 2, 6, 18, 54, 162]`. Ships, modules, Academy courses and captain
  recruits are builds, so K reaches them through `build::price`.
- **Storage** is `storage_mult` (research × K) on the Warehouses and the
  Vault, not new constants.
- **Wire (v90):** the catalog keeps base costs, plus `Galaxy.cost_scale`,
  `BuildOptionView.unscaled` (the Warehouses), and
  `FoundingView.charter_builds` as lists of catalog keys (`raider`, not
  `ship:raider`). Claims: `CharterView.claims` (held as heard, `mult`,
  `at_war`, `closed`), `ClientMsg::BuyClaim`, and `SystemInfo.reserved`
  for home sites held for new charters (no claim is offered on them).
- **Client:** no `buildPrice`. Welcome prices the catalog once
  (`scaleCatalog`), and each founding update re-prices the charter's builds
  (`priceCharterBuilds`), so every reader of `build_options` sees what the
  sim charges. `structureLevel` climbs from the K price. A charter-priced
  level I shows "· Charter step" (tooltip: the charter's own build, today's
  price, once).
- **Claims, as built:** an unowned system shows a "Claim" row: "64,000 Cr"
  and Buy, "Held · lapses in 6.5d", "None needed · at war" or "Not sold ·
  charter closed". A Colony Ship sent into an unclaimed well warns "No
  claim: buy one first.", the Faction page lists claims held, and the first
  colony goal adds "Buy the Authority's claim" (its button becomes "Buy
  claim" once the ship is ready). "Unclaimed" now reads "Unowned" wherever it
  meant ownership.
- **One addition:** a found sealed world needs no claim and is never sold
  one. The charter forbids landing there, and a landing is already war
  (`precursor_landed`).
- **Bots** buy the claim before the Colony Ship sails. Their cash and stock
  reserves are not retuned yet; that waits for the step 1 run.
- **Tests:** sim (`build::tests` K prices, `world/scarcity.rs`, the repair
  test, `world/claims.rs`), server (catalog keys, claims heard, `reserved`),
  client `test-scarcity.mjs`. Thirteen colony tests now hold a claim
  (`World::grant_claim`, test-only).

**Step 0 is done** ([scarcity-baseline.md](balance/scarcity-baseline.md)),
on a frozen build of today's economy and smaller than planned: 8 bots, one
seed, 3 days. Twenty bots ran under an hour of game time in 17 minutes of
wall time, so 20 bots × 14 days × 3 seeds is out of reach.
- A median home nets 60,037 Cr an hour on day 2, mostly ore sold to the Hub.
  A Mining Complex I (1,394 Cr) costs 1.4 minutes of it.
- **K = 80**, the only value in the 30–80 band that gives 1.5–3 hours on
  both net cash (1.86 h) and Hub sales (1.54 h).
- The bots stop developing after the charter even at K 1: starving,
  one research programme in three days, no colonies, stalled on drives and
  hull sections they never source. So the colony and level III checks fail
  before K is touched.

**Step 1, first run at K 80** (same 8 bots, seed and 3 days): no-go on
the charter. Only 4 of 8 finished it (at 303, 453, 730 and 1,251 minutes);
the rest never did. Two causes, neither about K's size, both fixed:
- **The survey's probe wasn't a charter build.** Step 23 needs a Survey
  Probe or a Scout, at 80× it stalled six bots on 170–320 Semiconductors. It
  is now entry 18 (last, so the older indexes hold).
- **A charter ship lost mid-charter rebuilt at K.** One bot lost its
  Interceptor and Tiny Freighter at step 23 and sat for three days. Now,
  while the charter runs, a charter ship you have none of, afloat or on the
  slips, rebuilds at today's price (`World::charter_rebuild`). One at a time
  and only during the charter, so it neither stocks a fleet nor rewards
  staying in the tutorial.

With both, a 60-minute check had 7 of 8 bots at step 27 or done.

**Step 1, rerun at K 80** (Nov 11, 02:00): the charter passes, all 8 in
51–207 minutes (K 1: 47–228). After it the bots are income-bound: net cash
about zero, credits under 3,500. Builds drop from 67/40/39 a day to 54/3/13.
The time to afford the next build is 20 minutes on day 1, about 5 hours on
day 2 (3 builds) and 11 hours on day 3, against 1–8 hours. Most of that is
the bots chasing an Interceptor at 80× its parts (80 fission reactors, 80
Zephyr drives, about 236 pulse lasers) while churning about a million
credits a day through the Hub. Per the no-go rule, K alone changes: the one
rerun is at **K 60**, started 02:05.

**Step 1, rerun at K 60: go** ([scarcity-step1.md](balance/scarcity-step1.md)).
The charter in 51–253 minutes for all 8; the next build in 19 minutes, 8.3
hours and 2.1 hours on days 1–3; level III at day 3: 8 (K 1: 5). First
colony and level III at day 14 can't be judged until the bots are fixed.
**K = 60 is the pick.** :8080 gets `COST_SCALE=60` on Peter's go.

## Step 0: a fresh baseline (needs OK)

The Nov 4 numbers are stale: ships got far dearer on Nov 7 and licences
added a credit drain on Nov 8.

**Harness changes (test-only), in `crates/server/src/game_loop/bots/tests.rs`:**
- `BalanceMetrics` gains, per corporation per day:
  - output value (production × base price)
  - Hub sales accepted and refused, in units and credits
  - credits at the day's end
  - time-to-afford the bot's next build (hours until its goods are in stock)
  - minutes a build waited on the storage cap
- `BOT_BALANCE_BOTS` (default 8) so a run can seat 20, the size where the
  Hub's 8 units a second per good starts refusing.

**Run:** release build, 14 days, three seeds, 20 bots, one save directory per
seed so a run can resume:

```bash
BOT_BALANCE_MINUTES=20160 BOT_BALANCE_BOTS=20 BOT_BALANCE_SEEDS=12648430 BOT_BALANCE_SAVE_DIR=/tmp/scarcity-baseline-12648430 cargo test --release -p server multi_seed_bot_balancing_report -- --ignored --nocapture
```

Results go to `docs/balance/scarcity-baseline.md`.

**Pick K:** from day 1 after the charter, a standard Mining Complex I should
take 1.5–3 hours of home income to afford.
**Go:** K lands between 30 and 80. If the surplus starts inside the charter,
the charter gets its own pass first.

## Step 1: costs

### Sim

1. **The multiplier.** `build.rs`: `pub const COST_SCALE: f64 = 50.0` (K from
   step 0) and `pub fn scales_with_k(what: BuildKind) -> bool`:
   - **Not scaled:** Warehouse, Orbital Warehouse, and founding-camp projects
     (`BuildKind::Founding`).
   - **Scaled:** every other structure, the Command Center, power plant and
     station ladders included; every ship; every module; Academy courses and
     captain recruits.
2. **The level curve.** `TIER_COST_MULT` becomes `[1, 2, 6, 18, 54, 162]`:
   level II costs double, then each level three times the last. Build times
   (`TIER_TIME_MULT`) are unchanged. The charter only builds level I.
3. **One price function.** `build::price(what, tier, charter_step: bool) ->
   TierRecipe` is `recipe_for_tier` × K, unless the build is exempt or a
   charter step. `recipe_for_tier` itself stays unscaled, because the
   charter crates are sized from it (`charter_crates.rs`).

   Every caller uses `price`:
   - `World::apply_build` (world.rs, the affordability check and the debit)
   - the server's build catalog (game_loop.rs)
   - the bots' project costs (bots.rs `project_costs`)
   - cancel refunds
4. **Charter steps at today's price.** `founding::CHARTER_BUILDS` lists the
   builds the charter asks for, each priced at today's cost once:

   | # | Build | Step |
   |---|---|---|
   | 1 | Habitat I | 1 |
   | 2 | Power Plant I (Solar Array) | 2 |
   | 3 | Bioharvester I | 3 |
   | 4 | Agroplex I | 4 |
   | 5 | Mining Complex I | 5 |
   | 6 | Smelter I | 6 |
   | 7 | Orbital Defense I (Laser Battery) | 8 |
   | 8 | Shipyard I | 10 |
   | 9 | Interceptor | 11 |
   | 10 | Tiny Freighter | 15 |
   | 11 | Academy I | 19 |
   | 12 | Small Freighter | 21 |
   | 13 | Power Plant I, second world | 22 |
   | 14 | Mining Complex I or Asteroid Quarry I, second world | 22 |
   | 15 | Mining Barge | 24 |
   | 16 | Uranium Mill I | Reach crate |
   | 17 | Vault I | 28 |
   | 18 | Survey Probe or Scout | 23 (added Nov 10, after the first K 80 run) |

   - `FoundingProgram` gains `charter_builds_left` (indexes into the list,
     all of them at founding).
   - In `apply_build`, a level I structure or a ship that matches an entry
     consumes it and pays today's price.
   - Entries never expire, so finishing the charter fast or slow changes
     nothing: nothing a player wants depends on staying in the tutorial
     ([overproduction plan](overproduction-plan.md), Nov 10 note).
5. **Exact refunds.** `BuildJob` gains `charged: Vec<(Commodity, f64)>` and
   `charter_entry: Option<u8>`, both with serde defaults. Cancelling a build
   (construction.rs) refunds exactly what was charged and returns the charter
   entry. Today the refund recomputes from the recipe.
6. **Storage follows K** (`build.rs`):

   | Constant | Today | K=50 |
   |---|---|---|
   | `STORAGE_WAREHOUSE_INITIAL` | 700 | 35,000 |
   | `STORAGE_PER_WAREHOUSE_TIER` | 400 | 20,000 |
   | `STORAGE_PER_ORBITAL_WAREHOUSE_TIER` | 2,000 | 100,000 |
   | `VAULT_UNITS_PER_TIER` | 200 | 10,000 |

   Landing holds (1,000) and the Storehouse (+300) stay, so the charter's
   storage lesson stands. The Warehouse recipe stays at today's price, so a
   corporation fresh out of the charter can build Warehouse I and then hold a
   standard Mining Complex I.
7. **Claims are bought from the Authority** (Peter, Nov 10 2026: "I want
   the player to have to purchase a claim from the Authority for new
   colonies").
   - **The rule:** a Colony Ship can found a colony only on a system its
     corporation holds a claim to. A syndicate at war with the Authority
     (after the Keystone leak) needs none, as with hull licences.
   - **Buying:** one system at a time, as a paid Hub order, the same path as
     a hull licence (`Command::BuyClaim { player_id, system_id }`). The
     credits travel from home, the Authority grants the claim when the order
     reaches the Hub, and the news comes home at light speed.
   - **Price:** the system's claim cost × K × 1.6^n × the standing tariff
     (`tca::tariff_mult`, as for licences).
     - The claim cost is `claim_cost_for`: 600 + 45 × the system's
       value-rate. It's generated today but has never been charged or shown.
     - n counts the systems owned beyond home plus claims already held.
     - At K=50, a home-grade system's first claim is about 40,000 Cr, the
       second about 64,000 and the fourth about 164,000.
   - **One holder at a time:** a second corporation's order for the same
     system is refused at the Hub and its payment returned.
   - **Lapse:** a claim unused for 7 days lapses, unrefunded, and the system
     is for sale again. Held claims raise the price of the next one, so
     hoarding gets dear.
   - **Not sold** at Revoked or Proscribed standing, as for licences.
   - **Code:** `claims.rs` (price and lapse) and `world/claims.rs` (the grant
     on arrival, lapses, and the check where a Colony Ship founds its colony
     in world.rs). `Corporation.claims` maps each held system to `{ bought_at,
     lapses_at }`.
   - The Colony Ship itself is priced like any ship: × K, no escalation.
8. **Specialists:** `SPECIALIST_HIRE_COST` 800 × K (40,000).
9. **Repairs:** `REPAIR_COST_PER_HP` × K, except hulls on the den blast's
   wounded list (the charter's repair step).
10. **Left alone**, as the review says:
    - Hub liquidity, bounties, stipends, contracts, signals and the weekly ladder
    - research baskets, food, requisitions (they follow valuation) and the
      late charter grant
    - the free trader's stock and combat-balance values

    K devalues fixed credit grants on purpose.

### Server (protocol v90)

11. Welcome's build catalog keeps base costs and gains `cost_scale` and a
    `scaled` flag per option.
12. `FoundingView` gains `charter_builds_left` (build keys, e.g. `habitat`,
    `ship:raider`).
13. Claims:
    - `ClientMsg::BuyClaim` and a `ClaimView` in the player's view: claims
      held with their lapse times, and prices for known systems as heard at
      home.
    - Log lines: "Claim granted · {system} · N Cr", "Claim refused ·
      {system} · held by another corporation", "Claim lapsed · {system}".

### Client

14. `core/derive/market.ts`:
    - `TIER_CURVE.cost` becomes `[1, 2, 6, 18, 54, 162]`.
    - A new `buildPrice(option, tier, founding)` returns the costs ×
      `cost_scale`, or today's price with `charter: true` when a charter entry
      is left.
    - Every build card reads it: structures (via `structureLevel`), ships,
      modules and Academy courses. A charter-priced card shows
      "Charter step".
    - Check number formatting for five- and six-figure costs.
15. Claims, on both shells:
    - An unowned system's panel shows "Claim · 40,000 Cr · Buy", or
      "Claimed · lapses in 5d".
    - A Colony Ship sent without a claim warns "No claim: buy one first".
    - The Faction page lists claims held.

### Bots

16. Project costs come from `sim::build::price` with the bot's own charter
    entries. Their cash and stock reserves get retuned from the step 1 run.
17. A bot buys the claim before it sends a Colony Ship.

### Tests

- **Sim:**
  - `price` (K, exemptions, curve)
  - a charter entry consumed once and returned on cancel
  - storage × K
  - claims: no colony without one, one holder at a time, lapse, the price
    climbing with systems and claims held, the standing tariff, refunds on
    refusal, and none needed at war
  - the repair exemption and the specialist price
  - a corporation fresh out of the charter builds Warehouse I and then a
    standard Mining Complex I with no Hub trip
- **Server:** the catalog and view fields.
- **Client:** the curve, `buildPrice` and the "Charter step" label.

### Go/no-go (14-day run, needs OK)

| Check | Target |
|---|---|
| Time to afford the next build, days 1–3 | 1–8 hours |
| Hub sales as a share of output value, from day 7 | 25% or less |
| Bots finish the charter | within 300 minutes |
| First colony | by day 7 |
| Level III structures at day 14 | at least half of today's count |

No-go: change K only and rerun once.

## Step 2: staples (galaxy generation, needs a reset)

### Sim

1. **The staples.** `galaxy.rs`: `enum Staple { Cuprite, Crystalline,
   Volatiles }`. Each covers a chain of ores:

   | Staple | Ores | Makes |
   |---|---|---|
   | Cuprite | Cuprite, Bornite | Conductive Metals |
   | Crystalline | Crystalline, Olivine | Silicates |
   | Volatiles | Volatiles | Polymers, Deuterium |

   `HomeSlot` gains `staple` and `lack`.
2. **Dealing them.** A new `world/home_staples.rs`, run after
   `seed_home_neighbourhoods` and `seed_exploration_stars`, so every system
   near a home exists.
   - Sort the homes by angle around the hub. The staple goes round the three
     in turn, from an offset drawn on a keyed stream (`home-staples-v1`).
   - A home's lack is the staple of the nearest home with a different
     staple, so the home next door has what you lack. That holds in one ring
     or two rows, and wherever the count doesn't divide by three.
3. **The region.** Every system nearer this home than any other home, within
   70k su: the home system, its doorstep stars, its founding targets and its
   neighbourhood ring.
4. **Removing the lack** in every region system:
   - planet and moon deposits of the lacked chain become the staple's ore at
     the same richness
   - belts of the lacked chain become the staple's belt kind (Cuprite →
     Metallic, Crystalline → Stony, Volatiles → Icy)
   - hidden pockets of the lacked chain are removed (e.g. Cuprite and Bornite
     in Stony belts)
   - claim values are recomputed
5. **Boosting the staple.** In the six-system neighbourhood ring, the staple
   sits at double richness in at least two systems. Existing staple deposits
   are doubled. If fewer than two systems have one, the best others get it
   (uninhabitable ones first), replacing their weakest deposit.
6. **The home belt** (`belts::home_belt`) holds the staple at today's
   richness (0.25): Stony Crystalline as now, Metallic Cuprite, or Icy
   Volatiles. Home worlds keep Biomass, Ferrite and the garden's Ferrite, so
   the charter is untouched.
7. **The doorstep ice world:** a home lacking Volatiles keeps the body but
   loses its Volatiles deposit. It only anchors an exploration site.
8. **The frontier is unchanged.** Titanium (frontier 0.45 and out) and
   Rare-metal ore (0.70 and out) stay frontier-only, and outside the regions
   every ore rolls as today. A lack can also be filled by expanding.

### Server and client

9. The home's staple and lack are served with the player's founding view. The
   Faction page and the home system card show "Staple · Cuprite · Lacks ·
   Volatiles", with what each makes in a tooltip.

### Tests

- Every home lacks exactly one chain in its region.
- Its staple sits at double richness in at least two ring systems.
- The nearest home with a different staple holds your lack.
- Home worlds are identical.
- The founding charter still completes: the bot charter end-to-end harness
  (needs OK).

### Reset

`--reset-galaxy` on :8080.

### Go/no-go (14-day run, 20 bots, needs OK)

| Check | Target |
|---|---|
| Every corporation lacks one input | yes |
| The lacked input's use that is bought, not mined, by mid-season | at least 30% |
| Bots stuck | none for more than a day |

No-go: if bots stall on Hub refusals, bring back bots trading on the
Exchange (held back on Nov 10).

## Step 3: wear and tear (held back)

The review's spec, built only if the step 2 run shows demand fading late in
the season:
- Machinery upkeep on works from level III, at 0.0017 × level² per second
  (III 0.015/s, VI 0.061/s), about 8–10% of a mine's output value.
- When Machinery is short, output halves; nothing is destroyed.

## Decisions for Peter

| # | Decision | My pick |
|---|---|---|
| 1 | Claims | Decided Nov 10: bought from the Authority (step 1.7) |
| 1a | A claim lapses after 7 days unused, and one corporation holds a system's claim at a time | Yes to both (Nov 10) |
| 2 | Licences: stay 20% of the ship's (now scaled) price, or keep today's credit prices | 20% of the scaled price, your rule |
| 3 | The Vault scales with storage | Yes, so raids take the same share |
| 4 | Show the region's staple and lack in the UI | Yes, one line and a tooltip |
| 5 | Harness runs at steps 0, 1 and 2, and the reset at step 2 | Yes (Nov 10) |
