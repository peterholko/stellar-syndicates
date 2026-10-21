# Traveling merchant: phase 2

Status: **parked** Oct 14 2026. Peter chose to stay with phase 1 for now.
Nothing built. Phase 1 (the founding visit) is in
[traveling-merchant.md](traveling-merchant.md).

**In short:** after the charter, the player hails a free trader to any colony,
once every 6 hours. It arrives in about 5 minutes and trades for 45. Now and
then it brings pirates in behind it.

## Hailed, not scheduled

The phase 1 sketch had it visit on a timetable: every ~6 hours, for 30
minutes. That fails on timing:
- A player on for an hour a day would catch about one visit in six.
- The one they catch may be at the wrong colony.

A hail fixes both. The player is online when it comes, and it comes where it's
needed. The 6-hour wait and the markup keep it a last resort ("the player
should only rely on it when needed", Peter, Oct 13).

## A visit

| Step | What happens |
| --- | --- |
| Hail | Pick a colony. The hail travels at light speed from your command center, like any order to that system. The next hail is 6 hours away. |
| Arrival | 5 minutes after the hail lands, the trader drops out of warp 5,000 su out on the Hub side and docks about a minute later, as in phase 1. |
| Trade | 45 minutes at the stall. Trades travel at light speed like other orders to that system. |
| Leaving | When its time is up, or when you dismiss it. |

- **First hail:** ready when the charter is fulfilled, as the founding trader
  leaves.
- **Refusals:** it won't come to a blockaded system, or to a corporation
  whose charter is suspended. These are the same refusals Authority freight
  uses. A refused hail costs nothing.
- **Losing it:** if it's destroyed (see the chase), the hail is refunded and
  ready again at once.

## The stall

| Line | Stock | Price |
| --- | --- | --- |
| Staples: Alloys, Machinery, Electronics, Polymers, Provisions, Deuterium | Phase 1 amounts: 120 / 50 / 60 / 30 / 40 / 30 | Hub mid +50–100%, rolled per good per visit |
| One special per visit | see below | see below |
| It buys: ores and staples | up to 3,000 Cr a visit | 50% of Hub mid |

**Specials** (equal odds):
- **Grey-market core:** 2 Warp Cores or 1 Fusion Core at Hub +100%. The
  Authority sells cores only at the Hub, so this is the trader's edge.
- **Rare material:** 30 Iridium, Helium-3 or Exotic Matter at +50–100%.
- **Bargain:** one staple at Hub +15%, with double stock.

**Rules**
- Prices are fixed at the hail: the Hub mids at that moment, as in phase 1.
- Each row shows its own markup ("+72%"), so a good roll is visible.
- No arbitrage: it never sells below Hub +15% and never pays more than half
  the Hub mid.
- Bought goods land in that colony's stockpile, limited by its storage.

**What it's worth:** at a colony, 40 Electronics cost about 1,450 Cr by
Authority freight (Hub mid 34 plus the 6% fee), against 2,040–2,720 Cr from
the trader. That's 600–1,300 Cr to have them in about 6 minutes instead of a
freight trip from the Hub.

## The chase

- **When:** about 1 hail in 4, never the first. It only rolls when you have a
  combat hull or a defense station at that colony, so it never lands on a
  player who can't answer it.
- **What:** the trader arrives with a small pirate patrol (one or two
  raiders) about 4,000 su behind it. It runs for the star while the pirates
  go for it.
- **The contract:** "Protect the free trader", a private combat objective
  like the pirate campaign's: drive off or destroy the pursuers within about
  10 minutes.
- **Reward:** 1,000 Cr, plus this visit's staples at +25% instead of
  +50–100%. The usual captain XP and Authority standing come with it.
- **Failure:** the trader is destroyed, the visit is lost, and the hail is
  refunded.
- **How pirates can target it:** the pirate campaign's "Protect the
  evacuation" contract already lets pirates hunt one specific
  Authority-protected liner (its `evacuees` exception). The chase reuses that
  exception.

Correction to Oct 14: the chase doesn't need the trader to become its own
faction.

## Identity

- **On screen:** an independent trader. This needs the client fixes
  proposed Oct 14 (its own colour, gold suggested; "Independent trader" in
  its panel; no Authority boxes) and its own hull
  ([free-trader-art-prompt.md](free-trader-art-prompt.md)).
- **In the sim:** still Authority-licensed. Pirates leave it alone outside a
  chase, and attacking it draws citations.

## Bots

- **When they hail:** a project is blocked, its shortfall would wait more than
  about 15 minutes for freight, and spare cash covers the markup. They buy
  only the shortfall, as in phase 1.
- **Chases:** they apply to bots too. Bot defenses respond as usual.

## Interface

- **Top-bar chip** (exists):
  - "Hail trader" when a hail is ready;
  - "Trader · 4m" while it's on its way;
  - "Trader at Kepler · 38m" while it's docked;
  - hidden while the hail recharges, with the wait in its tooltip.
- **Hail:** from the chip, or "Hail trader" on a colony's panel. It shows the
  arrival time and the 6-hour wait.
- **Stall** (the existing pop-out and sheet):
  - the colony's name in the header;
  - each row's markup;
  - the special at the top;
  - a Sell section with what's left of the 3,000 Cr;
  - a Dismiss button.
- **Chase:** the contract appears with the other combat contracts, plus a map
  alert on the trader.

## Build sketch

- **Sim:**
  - `MerchantVisit` gains `kind` (founding / hailed), a markup per good, the
    special, the sell budget, and the chase contract's id.
  - The corporation gets `merchant_ready_at`.
  - Commands: `MerchantHail { system }` and `MerchantDismiss`.
  - Events: hailed, hail refused, lost.
  - `CombatObjective::ProtectTrader`, plus the evacuee-style targeting
    exception.
- **Server:**
  - `MerchantView` gains the system, per-row markups, the special, the sell
    budget and the ready time.
  - Timeline lines.
  - Protocol v51. No reset: new fields default.
- **Client:** the chip states, hailing, the stall changes and mobile parity.
- **Tests:**
  - the cooldown and refunds, and each refusal;
  - markups stay inside their bands, and the no-arbitrage invariant holds;
  - chase odds and eligibility, and the refund on loss;
  - the bots' hail rule.

## Left out

- A named captain with a portrait.
- Unprompted "passing trader" offers.
- Better prices for loyal customers.
- Several traders, or one shared between corporations.

## Decisions (recommendation first)

1. **Visits:** hailed, once every 6 hours; or on a timetable.
2. **Where:** any colony you pick; or only colonies far from the Hub.
3. **Arrival:** 5 minutes after the hail lands; or 10.
4. **Stay:** 45 minutes; or 30.
5. **Prices:** a +50–100% roll per good; or one roll per visit.
6. **Special:** one per visit; or none.
7. **Buying:** ores and staples at half the Hub mid, up to 3,000 Cr a visit;
   or Ferrite Ore only, as in phase 1.
8. **Chase:** about 1 hail in 4 when you can defend, for 1,000 Cr plus +25%
   prices; or no chases.
9. **Owner:** Authority-licensed under an independent identity; or its own
   faction.
