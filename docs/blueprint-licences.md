# Blueprint licences

Built Nov 8 2026 (uncommitted, protocol v86); see "As built". Peter (Nov 8 2026): "Research is basically unlocking the skill to
build it but the Blueprint is the license to build from the Authority, once
the player declares independence from the Authority they no longer need
licenses."

This is EVE's split. A skill decides what you may build, a blueprint gives
you the right to build it, and no blueprint gets around a missing skill.

## The rule

- **Research is the skill.** It unlocks a hull as it does today. Nothing gets
  around it.
- **A licence is the Authority's permission** to build a hull class. A
  chartered corporation needs both for every hull built at an Orbital
  Shipyard or a Capital Slipway.
- **Independence ends licences, and independence is the endgame.** A
  syndicate at war with the Authority needs none (below).

## Licences

Peter (Nov 8 2026): every warship from the Destroyer up, and the freighters
from the Large Freighter up. That is every hull built at an Orbital Shipyard
or a Capital Slipway. Nothing built at the ground Shipyard needs a licence:
not the Interceptor, the Corvette, the Small and Medium Freighters or the
Belt Station, although research still unlocks some of them.

| Hull | Research | Yard | Licence at full standing |
|---|---|---|---|
| Large Freighter | Express Charters (III, 24 h) | Orbital Shipyard I | 16,200 |
| Destroyer | Destroyer Hull (IV, 72 h) | Orbital Shipyard I | 10,000 |
| Heavy Freighter | Bulk Charters (IV, 72 h) | Orbital Shipyard II | ≈ 49,000 |
| Cruiser | Cruiser Hull (V, 168 h) | Orbital Shipyard II | 51,000 |
| Bulk Freighter | Autonomous Freight (V, 168 h) | Orbital Shipyard III | ≈ 147,000 |
| Battleship | Battleship (VI, 240 h) | Orbital Shipyard III | 258,000 |
| Dreadnought | Dreadnought (VII, 336 h) | Capital Slipway I | Commissioned only: 1.3M |
| Titan | Titan (VIII, 480 h) | Capital Slipway II | Commissioned only: 13M |

- **Price.** Peter (Nov 8 2026): "roughly 20% of the cost of the ship and
  then it can be reduced if your standing increases or if your standing
  decreases the cost can increase."
  - The price is 20% of the hull's kit value at base prices
    (`LICENCE_SHARE`), × the Authority's multiplier at your standing.
  - The multiplier is the freight tariff's (`tca::tariff_mult`), so the
    Authority prices freight and licences alike.
  - The price follows the hull ladder and any later rescale of recipes.

  | Standing | Multiplier | Destroyer |
  |---|---|---|
  | Commissioned (175) | ×0.50 | 5,000 |
  | Preferred (150) | ×0.70 | 7,000 |
  | Trusted (125) | ×0.85 | 8,500 |
  | Full (100) | ×1 | 10,000 |
  | Suspended (60) | ×2 | 20,000 |
  | Just above Revoked (21) | ≈ ×3 | ≈ 30,000 |
  | Revoked or Proscribed | not sold | — |
- **Capitals:** the Authority sells the Dreadnought and Titan licences only
  at Commissioned.
- **Bought once** with credits from the treasury, by a paid order to the
  Authority at the Hub. Like any Hub order, it travels at light speed.
- **Permanent and registered to the corporation.** There are no copies and no
  resale, and nothing is hauled or raided.
- **Offered once the research is done.** The Shipyard shows the hull with
  "Licence · 10,000 Cr" and a Buy button. Nothing that research hasn't
  unlocked shows anywhere (§research-locked-hidden).
- **Checked when a keel is laid.** Hulls already built or queued are never
  affected.
- **Suspended while the charter is Revoked or Proscribed.** The Exchange is
  closed then anyway. Good standing restores them.

## Independence is the endgame

Peter (Nov 8 2026): "No independence is the end game remember?" There is no
declaration to make mid-season. Breaking from the Authority is the Keystone
race (docs/endgame-keystone.md).

- **Licences end with the Authority's declaration of war.** Finding a
  Precursor world leaks to the Authority within 2 h. It then declares war on
  the finder's syndicate, as the syndicate stands at that moment. From then on
  its members need no licences: they build any researched hull, capitals
  included, whatever their standing. That is how the rebels raise a fleet for
  the 48-hour stand.
- **Not at the Seal.** The Seal ends the season, so dropping licences then
  would change nothing.
- **Everyone else needs licences all season,** loyalists included. Their
  standing makes licences cheap, and Commissioned opens the capitals.

## The charter

Unchanged. The charter builds nothing from an Orbital Shipyard, so it never
meets a licence.

## Bots and client

- Bots stay loyal, as in the season rules. They buy a licence before laying a
  keel that needs one.
- **Shipyard:** the licence row with its Buy button. At war, it reads "No
  licence · at war".
- **Faction page:** the licences held. Both shells.
- A protocol bump. No reset: a corporation's licences start empty.

## Build order

1. **Sim, licences:** `Corporation.licences`, `Command::BuyLicence` (paid),
   the gate in `BuildShip`, the prices and the suspension, with tests.
2. **Sim, the war exemption:** members of the syndicate at war with the
   Authority skip the licence check, with a test.
3. **Server:** views, Log lines and the bot rule.
4. **Client:** Shipyard and Faction page on both shells, with tests.

## Go/no-go

- Sim, server and client tests pass.
- The bot charter run is unchanged (8/8 by 300 min). Running the harness needs
  Peter's OK.
- A licence is affordable when its research completes, measured on the same
  run.

## For Peter

Settled (Nov 8):
- **Which hulls:** those built at an Orbital Shipyard or a Capital Slipway.
- **Price:** 20% of the hull, scaled by standing.
- **Capitals:** licences sold only at Commissioned.
- **Independence:** the endgame. Licences end for a syndicate when the
  Authority declares war on it.

Left out unless wanted: a registry fee on every hull built, rising with the
grip (5% at III, 10% at IV).

## As built (Nov 8 2026)

The protocol is v86. :8080 restarted on the same galaxy, with no reset.
Nothing is committed.

- **Sim, `licence.rs`.** It holds the rules: which hulls need a licence
  (`needs_licence`: built at the Naval Drydock or Capital Slipway),
  `commissioned_only`, `licensed_hulls` (cheapest first), `price`
  (`LICENCE_SHARE` 0.2 × `combat_balance::hull_value` × `tca::tariff_mult`),
  the shared `offer` rule, `shown_price` and `LicenceRefusal`.
- **Sim, the purchase.**
  - `Corporation.licences` maps each hull to when it was sold.
  - `Command::BuyLicence` is a paid Hub order. It carries the price plus the
    10% payment margin, and the change travels home.
  - `world/licences.rs` holds `licensed`, `licence_offer`, `hull_researched`
    and `buy_licence`.
- **Sim, the build check.** `BuildRejectReason::NeedsLicence` follows the
  research check in the build path. `LicenceBought` and `LicenceRefused` are
  owner-only events placed at the Hub.
- **Server.**
  - `ClientMsg::BuyLicence` shows the outbound chevron to the Hub.
  - `CharterView.licences` comes from `LicenceView::heard`. A licence counts as
    held once its receipt's light reaches home, and prices use the standing
    heard.
  - Log lines go under the Charter kind (bought) and the Refusal kind.
- **Bots.**
  - They plan only hulls they are licensed for (`hull_ready`).
  - They buy the cheapest researched licence once the treasury covers it twice
    over, with one order per hull per round trip to the Hub.
  - At war they buy none.
- **Client.**
  - `shipOption` checks the licence (`licenceFor`, `licenceMissing`,
    `licenceForSale`, `licenceNote` in `core/derive/market.ts`).
  - The Shipyard shows a Licence row with a Buy button, on desktop and mobile.
  - The Faction page lists the licences on both shells. Hulls that research
    hasn't unlocked show nowhere.
- **Tests.**
  - New: 8 sim tests (`licence`, `world::licences`), 3 server tests (the
    view's receipt delay, the bot's purchase, the bot's hull choice) and the
    licence block in `client/scripts/test-industry.mjs`.
  - Five older sim tests that lay licensed keels now grant licences.
  - Results: sim 1,227 pass (the one failure, in captain caps, predates
    this), server 278, client 65 plus tsc.
- **Not run:** the go/no-go bot charter harness and the affordability check.
  Both need Peter's OK to run the harness.

