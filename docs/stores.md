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
  tooltip says when storage fills. Full means within a unit of the cap, as
  the guide counts it: food eaten keeps a full store a fraction under its
  cap, which read amber "999/1,000" before Nov 3 2026. A full meter's label
  reads "Storage full".
- **Opening Stores:** "All goods", any good, the meter or a system's Production
  tab (Open Stores) opens the workbench. **I** opens and closes it; Esc or ✕
  closes it.

## Stores workbench

- **Layout:** a central workbench over the map (`#deck-stores-workbench`, the
  Construction workbench's frame), 980 px wide (760 until Nov 5 2026, widened
  for Made, Used and Net) with 32 px goods icons and 40 px rows (Peter, Oct 28
  2026: "a little larger"), as tall as its content
  but never below the Construction workbench's edge above the
  command/tutorial stack; past that the list scrolls. It is not a route (Peter, Oct 28 2026: opening Stores used to replace the
  planet panel and zoom the map out). The workspace and the camera stay.
- **Header:** the switcher, the storage meter and when storage fills.
- **Groups:** Raw, Refined, Advanced and Fuel, plus Other for an unknown good.
  - A good is listed when you hold it, have pinned it, or it is moving.
  - A header row names the columns. Each row shows amount, held, **Made**,
    **Used** and **Net** a minute (Peter, Nov 5 2026: "clearly 'input/output'
    ... and then 'net' for each resource"), time until empty and a pin. A
    dash means nothing moves; the headers' tooltips say what counts.
- **Ship goods to Market** (one click shipped every non-fuel unit to the Hub)
  was removed Nov 5 2026, desktop and mobile; the Market and Logistics move
  goods. The Production tab keeps its Stores line with an Open Stores button.

## Rates are served (Nov 5 2026)

The sim tallies each system's flows as its economy runs: what production lines
make (mines, harvesters, factories, colony projects, outposts) and what is used
(factory inputs, the settlers' food, power plant fuel, fleet, garrison and
quarry upkeep, repairs and research). Freight, trades, builds, raids and
refuelling are transfers, not flows. Each tick folds into a rate smoothed over
20 s (`StarSystem::fold_flows`); the site report carries it at a 0.1 a minute
grain, so a steady economy makes no extra reports. The view serves it
owner-only as `SystemStateView.flows` (made and used a minute), and a good's
rate is made less used.

## The measured fallback

A system that reports no flows keeps the old measured rate.
`noteStockSamples` (called from the session's View handler) keeps one sample
per owned system every 2 sim seconds.

- **Measurement:** a rate is the steady change over the last 300 sim seconds.
- **Transfers left out:** a single step of 3 units or more, over 4 times the
  median speed, is a transfer: a freighter unloading, a build starting or a sale.
- **Warm-up:** a rate needs 60 s of steady history before it shows.
- **Resets:** the history starts over on a new corporation or when time runs
  backwards.
- **Units:** rates and times are sim time, like the rest of the UI.

The measured rate came first; the served flows replaced it on Nov 5 2026.

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
