# Stores: the system's goods at a glance

Peter (Oct 24 2026): the stockpile was "kinda buried under the production tab".
The fix follows Travian's resource bar: a stores bar always visible under the
top bar, and a Stores workbench with every good.

## Stores bar (desktop)

- **Placement:** a 32 px row under the top bar (`#deck-stores`). It shows only
  when you hold a system.
- **Effect on layout:** while it's shown, `.deck.has-stores` makes
  `--deck-topbar-height` the top row plus the bar. Panels, workbenches, toasts and
  the planet stage all follow. The top bar's own row is `--deck-topbar-row`.
- **Which system:**
  - your system that is open (system, world, build or shipyard route);
  - else the system selected on the map;
  - else home.
- **Switcher:** a picker appears when you hold two or more systems. A pick holds
  until you navigate or select again.
- **Goods:** each shows an icon, an amount and a per-minute rate (green or red).
  - The defaults are Provisions, Ferrite Ore, Alloys, Machinery,
    Semiconductors, Uranium and Deuterium. You pin or unpin goods in the
    workbench, saved per browser in `stellar.stores.pins.v1`.
  - A good turns amber when it empties within 30 minutes.
  - The tooltip gives the amount, the amount held for projects, the rate and
    when it empties.
- **Storage meter:** amber at 90% full, red when full (production stops). Its
  tooltip says when storage fills.
- **Opening Stores:** "All goods", any good, the meter or a system's Production
  tab (Open Stores) opens the workbench. **I** opens and closes it; Esc or ✕
  closes it.

## Stores workbench

- **Layout:** a central workbench over the map (`#deck-stores-workbench`, the
  Construction workbench's frame), 760 px wide with 32 px goods icons and
  40 px rows (Peter, Oct 28 2026: "a little larger"), as tall as its content
  but never below the Construction workbench's edge above the
  command/tutorial stack; past that the list scrolls. It is not a route (Peter, Oct 28 2026: opening Stores used to replace the
  planet panel and zoom the map out). The workspace and the camera stay.
- **Header:** the switcher, the storage meter and when storage fills.
- **Groups:** Raw, Refined, Advanced and Fuel, plus Other for an unknown good.
  - A good is listed when you hold it, have pinned it, or it is moving.
  - Each row shows amount, held, rate per minute, time until empty and a pin.
- **Ship goods to Market:** moved here from the Production tab. Reserve fuels
  stay behind (`shippableStock` in `core/derive/market.ts`). The Production
  tab now has a Stores line with an Open Stores button.

## Rates are measured

The server sends stock, not flows. `noteStockSamples` (called from the
session's View handler) keeps one sample per owned system every 2 sim seconds.

- **Measurement:** a rate is the steady change over the last 300 sim seconds.
- **Transfers left out:** a single step of 3 units or more, over 4 times the
  median speed, is a transfer: a freighter unloading, a build starting or a sale.
- **Warm-up:** a rate needs 60 s of steady history before it shows.
- **Resets:** the history starts over on a new corporation or when time runs
  backwards.
- **Units:** rates and times are sim time, like the rest of the UI.

A served net rate per good would be exact, but needs a protocol field. The
measured rate was chosen first.

## Code

- `client/src/core/derive/stores.ts`: groups, pins, samples, rates, storage,
  focus and formats.
- `client/src/shell/deck/stores.ts`: `DeckStores` draws the bar and the
  workbench (`open`, `close`, `toggle`). It is wired in `shell/deck/index.ts`
  (render on view ticks and routes, actions, inputs, shortcut I, Esc).
- `client/scripts/test-stores.mjs`: tests.
- `client/scripts/stores-fixture.html`: a preview on the Vite dev server. Add
  `?deck` to see it inside the real Deck markup.

## Not yet

- **Mobile:** the mobile shell still lists the stockpile as text in the system
  sheet and the system page.
- **All systems:** an "All systems" table, with goods as rows and systems as
  columns.
