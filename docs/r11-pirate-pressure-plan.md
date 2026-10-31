# R11: pirate pressure that grows with the season

**Status: PLAN** (Oct 23 2026). Nothing built yet. Part of the retention work
([retention-r5-r9.md](retention-r5-r9.md) for the earlier steps); independent
of the on-hold endgame ([endgame-keystone.md](endgame-keystone.md)).

## The constraint

Hideout tiers can't go above 3: tiers 4 and 5 already mean the fortified depot
and the regional stronghold (`pirate::DEPOT_TIER`, `STRONGHOLD_TIER`). So the
pressure grows another way:

1. Bigger packs as the weeks pass.
2. Pirate destroyers from week 4 and cruisers from week 6.
3. Raids that take a share of the stock above the floor instead of a flat 60
   units, and hit colonies from week 3.
4. Pirate guards on relics.
5. Larger pirate assaults that a whole syndicate can help defend.

## Today

| Piece | Today | Where |
|---|---|---|
| Hideouts | 3, mid-ring, tiers 1–3 | `pirate.rs`, `World::seed_enclaves` |
| Growth | +1 tier every 5 min while ignored (all at tier 3 about 10 min after the galaxy starts); cleared → 10 min dormant → tier 1 | `World::pirate_ai` |
| Pack | `pack_size(tier)` = tier light hulls of the base's faction (Ashwake torpedo Raiders, Ironclad armoured Corvettes, Rift driver Raiders); fixed at spawn, faction packs never topped up | `spawn_faction_patrol`, `launch_pirate_pack` |
| Base defense | `tier × 2` platform tiers | `base_defense_tiers` |
| Home raids | homes only; one per corporation in flight; 15 min cooldown; warning = light + 90 s; allowance per good = min(10% of stock, stock − 40); hold 2 × battle length (90 s); 1.5 units/s; **60 units total** | `world/pirate_home_raids.rs` |
| Heavy pirates | only the fixed depot (Destroyer + Corvette) and stronghold (Cruiser + Destroyer + 2 Corvettes) | `pirate::formation`, `world/pirate_progression.rs` |

So today pirate pressure peaks ten minutes in and then stays flat for the rest
of the galaxy's life.

## The clock

R11 needs only one number: the **season week**, 1 to 8.

- `World::season_week()` = 1 + ⌊8 × time / season length⌋, capped at 8. The
  season length is `SimConfig::season_weeks × 7 days` (new field, serde default
  8), the knob the R10 calendar was already going to add. A 4-week test season
  runs the same ramp twice as fast.
- `World.time` counts from the galaxy's start, and a season is one galaxy, so
  this is the season's week with no R10 state. When the R10 clock lands it
  takes over the same function.
- It's sim time: at the public server's 4× pacing a week is 42 wall hours.
- Every rule below reads `season_week()`, once per decision, never per tick.
- Precedents: the sim already has `medal_week()` (the weekly medals' clock)
  and Signals that scale by world age (days 0–6 ×1.0, 7–20 ×1.6, 21+ ×2.5; the
  Pirate stash signal unlocks on day 7). R11's heavy hulls at week 4 land as
  Signals reach their top tier, so rewards and danger rise together.

## 1. Bigger packs

Hideout tiers stay 1–3 and keep their 5-minute growth (a fresh hideout is still
weak). The week adds hulls on top:

- **Light hulls** = tier + ⌊(week − 1) / 2⌋: +0 in weeks 1–2, +1 in 3–4, +2 in
  5–6, +3 in 7–8. The faction's own light hull and fit, as today.
- **Base defense** = tier × 2 + ⌊(week − 1) / 3⌋: +1 platform tier from week
  4, +2 from week 7. Assaulting a hideout gets harder alongside.
- **Reinforcement.** A pack that is home and idle is topped up to the current
  composition when it next launches (today only legacy raider packs are).
  Damage is never repaired; only missing hulls are added.
- **Respawn.** A cleared hideout still goes dormant for 10 minutes and returns
  at tier 1, but with the week's extra hulls: clearing buys time, not a reset to
  week 1.
- **More hideouts (decision):** seed one more at the start of week 3 and one at
  week 5, in the same mid-ring band with the same home exclusion, from their own
  keyed RNG stream. Five sources by week 5 spread the pressure over a bigger
  map. Raid frequency per player doesn't rise (the per-corporation cooldown
  still caps it).

## 2. Destroyers from week 4, cruisers from week 6

| Week | Tier 3 light hulls | Destroyers (Reaver) | Cruisers (Warbarge) |
|---|---|---|---|
| 1–2 | 3 | – | – |
| 3 | 4 | – | – |
| 4–5 | 4–5 | 1 (tier ≥ 2) | – |
| 6 | 5 | 1 | 1 (tier 3) |
| 7–8 | 6 | 2 (tier 3) | 1 |

- **Fits by faction**, authored and published like the depot's, never generated
  against a player's fleet: Ashwake torpedo Destroyer; Rift mass-driver
  Destroyer; Ironclad armoured Destroyer; one armoured Cruiser fit for all
  three. Every fit passes `Loadout::validate` (test).
- **No engine work:** the hulls are the existing Destroyer and Cruiser, which
  pirates already field at the depot and stronghold.
- **Bulletins:** at the start of weeks 3, 4 and 6, every corporation's timeline
  gets one Authority line: "Raids now reach colonies", "Reavers sighted · relics
  guarded", "Warbarges sighted".
- **Map art:** a pirate fleet whose largest hull is a Destroyer draws as your
  Reaver, a Cruiser as your Warbarge. Both are already keyed and derived
  (`public/art/derived/season/v1/pirate-{reaver,warbarge}-*.png`, catalog
  `SEASON_ART` in `season-art.generated.ts`), but nothing imports them yet.
  Today `fleetMarker` draws every pirate fleet as one of three raider
  silhouettes whatever its hulls; a pirate Destroyer or Cruiser would also lose
  the pulsing threat ring (drawn only for `raider`) and its label (only Raider
  and Corvette have one). The change: the two sprites, the ring for all pirate
  warships, and "⚠ REAVER" / "⚠ WARBARGE" labels. The depot and stronghold
  garrisons get them too. `test-ship-art.mjs` pins the pirate art list and
  needs the two new files.
- **Battle theater:** pirate Destroyers and Cruisers already render with the
  player Destroyer and Cruiser sprites and 3D models, labelled "Destroyer" and
  "Cruiser", with no pirate livery. Labelling them Reaver and Warbarge is a
  one-line change; pirate GLBs stay optional art.

## 3. Raids by share, and colonies from week 3

**Haul.** A raid takes the least of three limits, with no flat cap:

- **Share above the floor**, per good: min(share × stock, stock − 40), with
  share = 10% + 2.5% per week after the first (27.5% in week 8).
- **What the pack can carry:** 20 units per light hull, 60 per destroyer, 150
  per cruiser.
- **What it can load while it holds orbit:** 0.5 units/s per light hull, 1.5
  per destroyer, 3 per cruiser, over the existing hold (2 × battle length,
  90 s).

A week-1 tier-3 pack (3 light hulls) carries 60 and loads 1.5/s: exactly
today's raid. The week-8 maximum is 6 + 2 + 1 hulls: 390 units.

**Targets.** From week 3, a corporation's colonies join its home as targets:

- Any owned system with a good above the floor, outside the Hub's sovereign
  zone, not blockaded, not founded or captured in the last 24 h (new-colony
  grace), with the owner not under founder protection or on leave.
- Still one raid per corporation in flight and the 15-minute cooldown: more
  colonies spread the raids, they don't multiply them.
- Within a corporation, raiders prefer a system with no defense platform, then
  the biggest haul, then the nearest. Defending a colony now deters raids, not
  just wins them.

**Unchanged:** the warning (light from the base to the command center + 90 s),
the rule that a raid must beat the standing defense before it can steal, the
counter-raid offer, no capture, bombardment or population loss.

## 4. Pirate guards on relics

Relics are the exotic-node systems (neutron stars, binaries, black holes,
magnetars; about 16% of systems). The system's owner holds the node and gets
Deep Scan, Relay Anchor or Veil within 1,800 su. A corporation runs at most one
node (`NODES_PER_CORP`), and a node costs 4 Provisions and 2 Deuterium a
second from its own system's stock, or it goes unfed. Each awakened node already
carries a public Strategic Control mission (own it for 180 s); guards make that
mission a real fight, so its reward needs a look.

- **From week 4**, every node system that is unowned and outside the home
  neighbourhoods gets a fixed pirate garrison, built from the depot machinery:
  a permanent site with a Destroyer, a Corvette and 3 platform tiers.
- **Fixed-site rules apply:** no growth, raids, repair or reinforcement; it
  stays cleared once beaten; dark until scouted; a colony ship can't land while
  it holds; clearing it pays plunder, a relic-grade gear roll (R14) and the
  Authority bounty, and opens the system for settlement.
- **First movers keep their prize:** a node system already owned at week 4 gets
  no guard.
- **With R10:** R10's calendar wakes the relics at week 4. The guards spawn at
  the same moment, so the relic race opens with every relic guarded. If R10
  isn't built yet, the guards still spawn at week 4 on today's (already awake)
  nodes.

## 5. Syndicate assaults

The co-op event, after OGame's Project Orion and Travian's Natars, and only for
syndicates: a solo corporation faces raids alone, a syndicate also faces
assaults it can defend together.

- **Cadence:** from week 3, one assault per syndicate per week; two from week 6.
  Scheduled from a keyed RNG stream, so a seed always produces the same
  assaults.
- **Target:** one member's home or colony, picked among the members' systems with
  the most stock above the floor, skipping protected, on-leave and new
  colonies.
- **Force:** twice the nearest active hideout's current pack, plus a Destroyer
  from week 4 and a Cruiser from week 6. It spawns at that hideout and flies a
  real course, so it can be intercepted on the way.
- **Warning:** 20 sim minutes plus light. Every member gets it, each when the
  pirates' broadcast reaches their own command center: a timeline notice, a
  line in the syndicate channel and a push/email alert (new `Assault` kind, on
  by default).
- **Defense:** the target's standing defense plus any member's warships waiting
  at the target. (Decision: today an ally's fleet joins a defense only if it is
  `garrison_fed`; assaults may drop that requirement.)
- **Repelled** (destroyed or withdrawn empty-handed): every member whose ships
  fought gets a share of an Authority bounty, earned standing and captain
  experience, and the syndicate scores points on its weekly project ladder (R6).
- **Not repelled:** it holds twice as long as a raid and hauls up to its
  capacity. No structure damage, capture or population loss, as for raids.

## Server and client

- **Events:** `PirateAssaultInbound` (target, member, ships, departure) and
  `PirateAssaultEnded` (repelled or hauled, by whom); the raid events gain the
  target kind (home or colony) and the pack's composition.
- **Notices, terse.** Today's warning always says raiders: "Pirate raid
  incoming: {system} — {ships} raider(s). Assemble defenses or intercept the
  approach." With R11:
  - "Pirates inbound · Freya II · 5 Raiders + Reaver"
  - "Pirate assault · Kepler (Orion & Sons) · 18m · 11 hulls"
- **Who hears it:** today every raid notice, alert and counter-raid mission
  goes to the raided owner only, and the syndicate channel carries only what
  players type. Assault notices use the existing per-member pattern (as the
  syndicate rung, bonus and medal news do, `timeline.rs`); a system line in the
  channel needs a small server-side "system message" path in `social.rs`.
- **Alerts:** today only the raid warning alerts (`AlertKind::Raid`, "Pirate
  raid"), offline players only, at most one per kind per 15 minutes. `Raid`
  also covers colony raids; new `Assault` for every member.
- **Admin dashboard:** a pirate line per week (raids, units taken, assaults
  repelled/lost) to see whether the ramp is too steep.
- **Protocol:** one version bump for the new events and fields.
- **Saves:** new fields take serde defaults; no galaxy reset needed. A galaxy
  older than week 3 simply picks up the ramp.

## Bots

Bots must keep up with the ramp, or the harness turns into a pirate
massacre. Today (`crates/server/src/game_loop/bots/`):

- **They never see a raid warning.** Bots read reports, ghosts and battles,
  never events or notices, so `PirateRaidInbound` is invisible to them. They
  react only to hostile ghosts within 25,000 su (`threatened`, a yes/no test).
- **Defense is sized by fixed numbers**, not by the threat: one Raider escort
  per freighter, `DefendSystem` on owned sites without a defender, one home
  guard kept out of campaigns, and a navy of about (ports + √(spendable /
  20,000)) × military priority. They raise defense platforms' value when their
  home is threatened.
- **Attacks are strength-checked**: a sortie needs about 1.6× the visible enemy
  strength. For a hideout they take the larger of the visible ghosts and the
  `pirate::formation(tier)` template. That template would understate the week's
  packs.
- **No syndicate behaviour:** bots issue no ally or syndicate commands.
- **No relics:** bots don't seek exotic nodes.

R11 changes for bots:

1. One composition function (`pirate::pack(tier, week, faction)`) used by the
   sim and by the bots' site estimate, so bots size hideout attacks against the
   real week's pack.
2. Give bots the raid warning they have earned by light: a bot whose home or
   colony is warned recalls its home guard and nearest idle warships to
   `DefendSystem` there, and holds freight departures from that system until the
   raid ends.
3. From week 3, value a defense platform at any colony holding goods above the
   floor, not just at a threatened home.
4. Syndicate bots answer an assault warning on a member with idle warships that
   can arrive before the pirates do, then return home.
5. Harness: losses and units stolen per bot per week, so the ramp's steepness is
   measured, not guessed. You decide when it runs.

## Tests

- **Sim unit tests, one per rule:**
  - pack composition by tier and week;
  - top-up without repair;
  - respawn strength;
  - haul limits (share, capacity, load rate) and week-1 parity with today;
  - colony targeting from week 3 with the grace and the undefended preference;
  - guards spawning only on unowned, non-neighbourhood node systems at week 4;
  - assault scheduling, warning light and ally defense;
  - reward split.
- **Determinism:** keyed RNG streams only; the existing pirate streams
  unchanged.
- **Saves:** a save/load round trip at every phase.
- **No-leak tests:** warnings arrive by light, never early.
- **Client:** marker art for the heavy hulls; notice text.
- **Balance:** a bot harness run through eight weeks. You decide when.

## Phases

| Phase | Scope | Size |
|---|---|---|
| A | Season week, bigger packs, heavy hulls, reinforcement, bulletins, Reaver/Warbarge markers | 1 session |
| B | Raid haul by share/capacity/rate, colonies from week 3, new-colony grace, notices | 1 session |
| C | Relic guards at week 4 | ½ session |
| D | Syndicate assaults: schedule, warning, defense, rewards, alerts | 1–2 sessions |
| E | Bots and the harness tuning pass | your call |

## Decisions for you

1. The clock: week = galaxy age (8 weeks, scaled by `season_weeks`), ahead of R10?
2. Two extra hideouts at weeks 3 and 5?
3. The haul curve: 10% → 27.5% share, 20/60/150 capacity?
4. Colonies from week 3 with a 24 h grace, undefended systems first?
5. Relic guards on every unowned node system, or only some?
6. Assault cadence (1, then 2 per week), the 20-minute warning, and allies
   defending without garrison supply?
7. What repelling an assault pays.
