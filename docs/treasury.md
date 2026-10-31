# Treasury

Peter, Oct 27 2026: "I swear I had enough credits ... or the credits in the
top menu aren't updating to the true number of credits." Credits were kept
at the Market Hub, and a home is 125,000 su from it, so the Hub's account
report reached home about a minute late (signals travel 2,000 su/s). The top
bar showed money already spent, and the free trader refused deals that
looked affordable. Decision (Peter: "Yes go"): the treasury sits at home.

## Rules

- **The treasury is at the command center** (`Corporation::credits`). The
  wallet (`WalletView.credits`, protocol v68) is the treasury, always current.
- **Spending at home is instant:** the free trader, research grants.
- **An order to the Hub (or a colony) takes its payment as it leaves.** The
  sim wraps it in `Command::Paid` (`World::payment_for`, `take_payment`):
  - market buys: units × your price cap, plus the charter penalty;
  - limit buys: units × limit;
  - modules: price + 10%;
  - specialists and reinstatement: their fixed cost;
  - a fuel rescue: its cost + 10%;
  - freight bookings: the fee + 10%.
- **If the treasury can't cover an order, it's refused at home, at once**
  (the event's origin is the command center): `TradeEvent::Rejected`,
  `FuelRescueRejected`, or `PaymentRefused` for modules and specialists.
  Those two used to vanish silently at the Hub.
- **At the receiver an order spends only what it carried** (`apply_paid`).
  The change travels home.
- **Money from the Hub travels home at lightspeed** (`wire_home`,
  `Corporation::wires`) and lands with its receipt (`settle_wires`, right
  after the clock advances each step). That covers sales, fills,
  cancellations, voided orders, trade-run sales, module sell-backs and an
  order's change.
- **Automatic buys at the Hub have no click to pay from.** A route's shopping
  and fuel, bunkerage and depot rules use money still on its way home first,
  netted out of what the Hub sends (`at_hub`, newest wire first), then the
  treasury. A trade run's sale pays for its return cargo and fuel.
- **Paid at home as before:** stipends, bounties, contract rewards, signal
  finds.
- **Still delayed:** prices, the Hub warehouse, resting orders, standing,
  equity. Those really are at the Hub.

## What changed for players

- The top bar is the treasury (client `state.credits`, set from every view
  even before the Hub's first report; `state.wallet` stays null until then).
  A new corporation sees its 2,000 Cr at once instead of "—" for a minute.
  The "~" mark and "N reserved" are gone (desktop and mobile), and so are the
  Market's "Reserved in flight" and mobile's "Account report" and "Reserved
  by orders" rows.
- Goods in a sell order stay off the warehouse count until the receipt
  (the hold used to lapse after one light delay, while the receipt takes two).
- Order notices say what left: "Buy order sent · 1,320 Cr paid."
- The trader stall checks against the treasury, even before the first Hub
  report arrives.

## Checked live (:8080, Oct 27 2026)

- A new guest bought 50 Deuterium. The top bar went from 2,000 to 1,203 on
  the click ("Buy order sent · 797 Cr paid.").
- About 2.5 minutes later the receipt ("Bought 50 Deuterium @ 13.49") and the
  123 Cr change landed together: 1,326.

## Bots

- They read the treasury (`Observation.credits`).
- A hold lasts until the order has paid (the next tick).
- A route's shopping budget is held for one round trip of light, as before.

## Dropped from the plan

- Per-rule budgets for depot rules: automatic buys net out of money in
  flight instead, so there's no field to fill in.

## Known leaks (accepted)

- A payment for something the market prices (modules, freight, a rescue) is
  sized from the live price. The drop in the treasury hints at that price
  about a minute early, within 10%.
- An automatic Hub buy beyond the money in flight shows in the treasury
  before its news arrives.

## Code

- Sim:
  - `crates/sim/src/world/treasury.rs` (with tests);
  - `world.rs`: `apply`, `administration_receiver`, the Hub income sites;
  - `world/industrial_expansion.rs`: route shopping;
  - `command.rs`: `Paid`;
  - `event.rs`: `PaymentRefused`.
- Server:
  - `game_loop.rs`: the wallet; `MarketAccountSample` no longer carries credits;
  - `game_loop/bots*`: credits and holds;
  - `timeline.rs`: the refusal line.
- Client:
  - `core/derive/market.ts`: `spendableMarketCredits` is the treasury; holds are goods only;
  - `shell/deck/index.ts`, `shell/deck/market.ts`, `shell/deck/merchant.ts`;
  - `shell/mobile/*`.
