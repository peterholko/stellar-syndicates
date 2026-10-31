# Post-charter loops: contract board, first finds, weekly medals

> Oct 20 2026: the first finds moved into Signals, as its scripted first
> ten ([signals.md](signals.md)); identifying exploration sites pays no
> first-find rewards any more.

Status: built Oct 19 2026, uncommitted. Protocol v54 (`stellar.msgpack.v54`):
deploy the client and server together. No galaxy reset needed; every new
saved field is `#[serde(default)]`.

Peter asked for these after comparing the founding charter with Travian (the
tutorial hands over to daily quests, the first adventures, and the weekly top
10). His choices (Oct 19 2026): top 3 medals, not 10; a new corporation card;
the physical route for first-find rewards (goods never appear at home); a
board that opens when the charter is fulfilled and replaces the old 15-minute
Survey and Market offers. He decided against Travian Kingdoms' governor and
king hierarchy: syndicates stay flat.

## The contract board (Travian's daily quests, without the trap)

- **Who:** a corporation whose founding charter is fulfilled (or one without
  a charter). Posted from `refresh_operations` (every 30 s) by
  `post_contract_board`, once per board day (`BOARD_DAY_S`, 24 sim hours: a
  real day at the dev server's 1× pacing).
- **What:** 4 contracts (`BOARD_CONTRACTS`): up to two surveys of the nearest
  systems it hasn't surveyed (400 Cr, 2 standing each), and deliveries of
  goods its home holds at least 30 of: a third of the stock, rounded to 10,
  30–150 units, sold or delivered at the Market Hub (8 Cr a unit, 1
  standing). The four most plentiful goods take turns by a keyed daily order.
  If goods run short, more surveys.
- **Arrival:** the offers come from the hub at light speed; one timeline
  line, "Contract board · 4 new contracts.", replaces the per-offer lines.
- **Lapse:** untaken offers lapse at the day's end, with no expiry lines. An
  accepted one keeps a full day (`BOARD_DAY_S`) whenever it was accepted.
- **Rewards wait:** payouts are made as each completion report reaches
  command, as for every operation, and never vanish. No captain XP or
  research insight: those could be lost (no assigned fleet, no research).
- **Bonus:** the third contract of a day's board that pays out pays +3
  standing and a 5% research dossier (`BOARD_BONUS_*`), once per day. Since
  Oct 22 2026 it pays no credits, and standing past 100 counts against the
  daily allowance (docs/retention-p0.md). Counted per corporation (`ContractBoard.done`), since
  the operations table is pruned at 512.
- **Retired:** the per-30-second private Survey Office and Market offers.
- **Client:** a "Contract board" section at the top of Operations (both
  shells): "2/3 for +1,000 Cr · new in 19h 59m" and today's contracts, open
  ones first; they're kept out of the other lists. Long deadlines now read in
  days and hours on the deck (they used to read "1440m 0s").
- **Wire:** `OperationView.board_day`, `View.board` (`ContractBoardView`).
- **Bots** don't take operations; they get offers like anyone.

## First finds (Travian's first adventures)

Retired Oct 20 2026: the four first finds became the first four of the
Signals' scripted first ten ([signals.md](signals.md)), with the same rewards.
They used to pay on the first four exploration sites a corporation identified.

## Weekly medals (Travian's weekly top 10, as a top 3)

- Each medal week (`rankings::MEDAL_WEEK_S`, 7 sim days, counted from tick 0;
  downtime stretches it) ends on a ledger close. The top three gains of the
  week in each of eight categories win gold, silver and bronze
  (`award_weekly_medals`). Recovery has no medal.
- Gains are the week's own: valuation growth, cargo hauled, profit, captured,
  protected, tiers built, intel; battle efficiency on the week's own
  engagements, at least three. Only a gain above zero places; ties go to the
  lowest id.
- Each corporation's week starts at its first close (a new founder, or an
  older save) and again after each award (`MedalCase`).
- The public rankings row carries each corporation's tally (`medals`); the
  owner gets "Week 3 · Gold medal · Trade throughput." with the published
  table's light delay, and a toast.
- **Corporation card:** click a rankings row (deck and mobile), your
  corporation name in the deck's top bar, or "Corporation card" in the mobile
  status panel. It shows titles, medals won by category ("None yet."), and
  the place in each category of the published table ("—" for a value of
  zero or a provisional one). Preview: `/scripts/standing-fixture.html` on
  the vite dev server.
- Bots are corporations and compete.

## Also changed

- The trader's docking line is now "A free trader docked at {home}." (it no
  longer claims Teo leaves when the charter is fulfilled).

## Tests

- Sim: `the_contract_board_posts_each_day_and_pays_its_bonus_once`,
  `weekly_medals_go_to_the_weeks_top_three`, and the medal helpers in
  `rankings.rs`.
- Client: `test-medals.mjs` (medal totals, the week clock, the card, the
  board line).

## Parked: the Colony Charter

Peter's idea (Oct 19 2026): buy a charter from the Authority for each new
system, a large credit price rising with each charter held, gated by a
Command Center II per extra charter and good standing; the Colony Ship needs
an unused charter and uses it on landing; losing a system frees its charter;
conquest needs a free charter too (Travian's rule for culture points and
expansion slots). The unused per-system `claim_cost` would go. Not built; the
price needs income data from a bot run.
