# Retention P1: talk, return, settle together (Oct 22 2026)

R5–R9 from the Oct 22 "After the Charter" review
(https://claude.ai/artifact/UomnmWZB4SkjDD3a12wRsH). Protocol v61. No galaxy
reset is needed: every new persisted field has a serde default. Accounts
database migration `0006_notifications.sql` runs at startup.

## R5 · Syndicate channel and corporation mail

`crates/server/src/social.rs`, in the galaxy checkpoint (`ReportingCheckpoint.social`).

- One channel per syndicate, last 200 lines. Mail between corporations by
  name: 100 kept per inbox and per sent folder.
- Limits: a line is 400 characters, a subject 80, a mail 2,000; 8 sends a
  minute per corporation. Text is trimmed of control characters.
- Mute hides a corporation's lines and mail from you. Report (⚑) logs a
  warning and keeps the last 500 reports for the operators.
- Presence: members online now, else "seen 3h ago" (wall clock, stamped on
  disconnect).
- Delivered as `Sections.social`, rebuilt only when its signature changes.
- Light lag is flavour only ("Authority relay · no light lag").
- Client: Comms page (desktop route `comms`, mobile sheet) with Channel and
  Mail tabs, and a badge for unread mail plus unseen lines
  (`core/derive/social.ts`).

## R9 · Welcome-back report, built on the server

`crates/server/src/welcome.rs`, sent as `Timeline.welcome` with the connect
digest after 10 minutes or more away.

- Each timeline line carries a topic (siege, raid, battle, research, trade,
  launch, built, arrival, signal, board, syndicate, medal, charter). While a
  corporation is away its lines are tallied by topic, so a long absence isn't
  cut to the journal's last 40 lines. The tally clears while it's online.
- Sales and purchases are summed from the receipt history.
- Waiting: open Signals not being answered, the day's board contracts, idle
  Academies (no programme), idle yards, project points a manager can spend,
  unread mail.
- Next: up to three actions. A siege or raid whose latest word is still bad
  comes first, then Signals, research, board, yards, points, mail.
- New sim event `ShipLaunched` (a finished hull, not a split) gives launches
  a Log line.
- Client: the desktop card and the mobile Log's inbox show it; each line and
  action leads to its page. While the charter runs, the guide's step leads
  and two server actions follow. Without a server report the old client-side
  report still shows.

## R7 · Settle near friends

- Invite links: Syndicate page › Invite a friend. Each holds the free home
  slot nearest your home for 72 h (`HOME_RESERVATION_S`); three open at once.
  Other newcomers pass a held slot by.
- `?invite=CODE` is kept in the browser through sign-in. The landing dialog
  redeems it ("Invited by Harbor · home near Vega · joins Pact"). Landing
  takes the held slot and joins the inviter's syndicate if it has a seat.
  Redeeming the link is the accept. Codes are single-use.
- Seats: only active human corporations count, for the cap and for members.
  Active means an order in the last 7 days, or being on leave. Bots
  (`PlayerId::is_bot`) never count. The cap is a third of the active human
  corporations, at least 2.

## R6 · Syndicate projects and bonuses

`crates/sim/src/world/syndicate_projects.rs`; numbers in `crates/sim/src/syndicate.rs`.

- A weekly ladder of five rungs replaces the one shared operation
  (`CreateSyndicateOperation` is now ignored; one already running finishes).
- Rungs ask for 150 / 400 / 900 / 1,800 / 3,200 units of goods and earn
  1 / 2 / 3 / 4 / 5 project points. The goods turn with the week (Alloys,
  Semiconductors, Machinery, Polymers, Provisions).
- Members give from their own worlds' free stock, by choice. Nothing is
  levied, and leaving costs the bonuses, never goods.
- Points buy lasting bonuses, levels I–V for 3 / 6 / 10 / 15 / 21 points
  (founder, officers, quartermasters spend them):

  | Bonus | Per level | At V |
  |-------|-----------|------|
  | Freight: player freighter holds | +6% | +30% |
  | Research: Academy output (inputs unchanged) | +4% | +20% |
  | Migration: settlers arriving | +8% | +40% |

- New members wait for the higher levels: I at once, II after 24 h, III 48 h,
  IV 72 h, V 96 h of membership.
- At each medal week's close the three syndicates that earned the most points
  take gold, silver and bronze, and every ladder restarts. Points and levels
  stay.
- Rankings: a Syndicates table (members' combined valuation, this week's
  points, medals), published light-delayed from the hub like the corporations'.
- Freight reaches the client as a per-fleet multiplier in the sim's fleet
  research cache. Holds round down per hull kind on both sides.

## R8 · Opt-in notifications

`crates/server/src/notify.rs`; client `src/notifications.ts`, `public/sw.js`,
and Profile › Alerts.

- Kinds: colony under siege, pirate raid, research done, warship launched,
  Signal waiting, fresh board, weekly medals. Everything is off until the
  player turns a kind on. A siege or raid alerts only the side under attack.
- Only for players who aren't connected. The game loop buffers alerts and the
  notifier collects them every 5 s. Limits: one push per kind per 15 min,
  24 a day per account.
- Push: Profile › "Push on this device" subscribes the browser. The VAPID key
  pair is made once and kept in the accounts database (`server_keys`), so a
  galaxy reset doesn't break subscriptions. Dead subscriptions (404/410) are
  dropped.
- Email digest: the welcome-back report as text, at most once per 20 h, after
  12 h away, to an address the player gives.
- Delivery is log-only until the operator turns it on:
  - Push: `WEB_PUSH=on` and `VAPID_SUBJECT=mailto:…` or `https://…`.
  - Email: `EMAIL_DIGEST_URL=https://…` (JSON POST of
    `{from, to, subject, text}`), optional `EMAIL_DIGEST_TOKEN` (sent as a
    bearer token) and `EMAIL_DIGEST_FROM`.
  - Profile shows "server push off" or "email off" until then.
