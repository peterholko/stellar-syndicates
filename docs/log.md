# The Log (Inbox and History)

Built Oct 28 2026 after the Log audit (https://claude.ai/artifact/Gtje7VTPnzUKbT9wnx2esZ),
Peter: "Ok implement your entire recommendations" (P0 + P1 + P2 + the small
fixes; P3 not built). Protocol v71, no galaxy reset.

## What the player sees

Two tabs, the same in both shells.

- **Inbox**: what needs you now, grouped Threats, Supply, Fleets,
  Opportunities. One line a row (title, then the numbers), one button, and a
  `⋯` menu with the other actions, Snooze 1h and Dismiss. The why sits in the
  row's tooltip. Five rows show, then "+N more". Nothing to do reads
  "All clear · <next decision>".
- **History**: what happened, newest first under day headers ("Today · D6",
  "Yesterday · D5", "D3"). One row per event or episode: a raid's warning,
  start and take are one row that shows its latest state. Like lines that land
  within 15 minutes collapse ("×3"). Filters: All, Raids, Combat, Trade, Build,
  Research, Explore, Ops, Fleet, Diplomacy, plus a system filter ("Freya ✕").
  A row opens into its report: every line with its game-clock time, the goods
  table priced at base value with a total, and its ways out (Replay, Defend or
  Show the system, Track the haul, Open fleet, Open operation, History of the
  system). Combat also lists the battle records (Engagements, Open).
- **Away strip**: the server's welcome-back report rides over History until
  closed (desktop: chips; phone: lines plus its next steps).
- A fresh open shows the Inbox when it has rows, else History.
- **Badge**: new Inbox rows plus unread warn-or-worse History news, one per
  episode. Opening a tab reads it; rows stay marked new while you look.
- The desktop has no Command page since Oct 31 2026; the Inbox is the one list
  of what needs you, and Log folds last in the top bar so its count stays in view.

## Server (protocol v71)

`TimelineEntry` keeps `{at_time, severity, text}` and adds optional, serde
default fields (old checkpoints load; old lines have none):

- `id`: per-player sequence (`Timeline.next_id`).
- `kind`: raid, blockade, battle, capture, trade, freight, trader, build,
  production, research, survey, signal, territory, operation, diplomacy,
  syndicate, fleet, refusal, charter, vendetta, medal, admin.
- `group` + `phase`: an episode key and its stage. Keys: `raid:{system}`,
  `blockade:{system}`, `build:{system}:{build_key}`, `landing:{system}`,
  `operation:{id}`. The client ends an episode at a terminal phase (ended,
  lifted, completed, failed, expired, done).
- `refs`: system, fleet, operation, battle record (links).
- `data`: goods (signed, this player's view), ships, credits, count, ids, eta,
  duration (the report's numbers).

One mapper, `log_meta()` in `crates/server/src/timeline.rs`, tags each event
once; push sites stay prose. Other server changes:

- Retention: 300 rows or 7 sim days (`JOURNAL_CAP`, `JOURNAL_MAX_AGE`); when
  full, the oldest info/good line in the older half goes first, so warnings
  outlive routine lines.
- Operation notices that land together merge into one line; "completed" goes
  only to the winner; no bracketed ids; no "Response received" lines.
- Production suspends shorter than 120 s are not news
  (`SUSPEND_GRACE_S`); a reported stop gets its "producing again".
- A rival's claim is news only for non-allies. Treaty labels read
  neutral / non-aggression / war / ceasefire.
- Pirate raid lines: "Pirate raid inbound · X · N raiders · ETA ~M min.",
  "Pirates attacking X — your defenses are fighting them.", "Pirates raiding
  X — they take up to 10% of each good, 60 units in all.", then "Raided X:
  pirates took … Their haul can still be intercepted." or "Pirates left X
  empty-handed." No "LIFTED" for a pirate raid; a player's blockade flushes its
  last plunder minute when lifted.
- A fuel shortfall that still flew reads as a warning, not a held fleet
  (`FuelShortfall.warning`); an industry refusal names what to check
  (`OrderRejectReason::IndustryRefused`).
- The welcome-back tally adds goods lost and credits ("Raid reports ×3: lost
  120 ferrite ore in all.").
- A Timeline message counts as sent only once it is queued
  (`game_loop.rs` `timeline_revs`).

## Client

- `core/derive/history.ts` (pure): `historyRows`, `rowActions`, `logToast`,
  `unreadNewsCount`, `dayLabel`, `goodsLine`, `goodsValue`, `KIND_ICON`,
  `HISTORY_FILTERS`. Lines from before v71 get a kind from their prose
  (`LEGACY`).
- `core/derive/inbox.ts`: `computeInbox` (both shells' old rules merged;
  the founding guide is not an Inbox row), `Go` descriptors each shell maps,
  `latestRaidWarnings`, and the prefs.
  - Charter quiet (§founding R8, restored Oct 28 after Peter saw "53 exotic
    nodes unclaimed" as a new player): until the privateer's bounty is in,
    side rows stay out (`CHARTER_QUIET`: nodes, survey reports, enclaves,
    operations, idle stockpiles, empty systems, invites, idle or loaded
    fleets, storage, refineries, no automation).
  - Exotic nodes: nothing until the charter is complete, then one row for the
    unclaimed nodes within 70,000 su of a system you own (a home's
    neighbourhood ring, `NODE_NEARBY_SU`), nearest first, never the galaxy's
    count.
- Prefs per corporation in browser storage (`ss.log.v1.<player>`):
  dismissed, snoozed (1 h of game time), History's read mark, the Inbox's seen
  keys. Keys that left the Inbox are forgotten only once the View has loaded.
- Shells: `shell/deck/log.ts` (`DeckLogRoutes`) and `shell/mobile/log.ts`
  (`MobileLog`, delegated from `MobileSurfaces`). Desktop routes:
  `log?tab=inbox|history&filter=…&system=…`, applied once per navigation; the
  old `tab=battles` lands on History + Combat. Phone sheet props:
  `{tab, filter, system}`.
- Toasts: `core/session.ts` emits `LogToast` for new raid lines with a group
  and phase; both shells word them with `logToast` (warning → Inbox, the rest
  → History · Raids). The charter milestone toast now matches the server's
  "Charter milestone · {title} · N Cr." line.
- Retired: the desktop "Needs a decision / Arrived reports / Light-delayed
  log" page, the mobile Check-in sheet and its separate rules, the in-memory
  dismissals, `PirateRaidWarning`/`PirateRaidNews`.

Preview: `npm run dev` in `client/`, then
`http://localhost:5173/scripts/log-fixture.html` (add `?mobile` for the phone
sheet, `?tab=history`). Tests: `client/scripts/test-log.mjs` (with
`log-harness.mjs`), `test-battle-archive.mjs`, `test-pirate-raids.mjs`,
`test-story-events.mjs`; server `timeline::tests`.

## Not built (P3)

Search, a per-system history page beyond the filter, daily digests.
