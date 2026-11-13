# Logistics panel

Status: built Oct 8 2026, uncommitted. Client only, with no protocol change. It
follows the clickable mockup Peter approved.

## Frame

- Logistics opens as a large centred pop-out over the map, like the Market Hub,
  up to 1240 px wide.
- Pop-out routes live in `POPOUTS` in `shell/deck/workspace.ts`. They reserve no
  right rail from the map camera, and the width toggle is hidden.
- While Logistics is open, the founding card shrinks to its header, as it does
  for the build workbench.
- The tabs are Routes, Production, Projects, Harvesting (gas-giant Fuel, moved
  out of Routes) and Standing rules.

## Getting there

- **Top bar:** **Logistics** sits beside Fleets, with the shortcut **T**, which
  is listed in the help overlay.
- **When the top bar runs out of room:** `fitDeckNav` (`shell/deck/markup.ts`)
  folds the least-used destinations into **More** instead of clipping them. It
  runs from a ResizeObserver on the bar and its neighbours.
  - The order is `NAV_FOLD_ORDER`: Faction, Comms, Syndicate, Operations,
    Officers, Research, Log.
  - Fleets, Logistics and Market always stay.
  - More lists exactly the folded destinations plus Rankings.
  - More stays pinned in view, and a folded destination's alert shows as a dot
    on it.
- **A Freighter's route line:** in the deck fleet panel and the mobile fleet
  sheet, **Open in Logistics** opens that Freighter's trip. On the desktop this
  is the route `{ name: "logistics", query: { fleet } }`, and a route preset
  applies each time the route opens.
- **The charter step** "Shipment under way" has the action `shipment-trip`,
  labelled "Follow the trip". It opens the Freighter's trip.
- **Mobile:** the system sheet's Logistics button (formerly "Auto-supply").

## Routes

- **The rail** lists:
  - every cargo fleet, with its state and hold;
  - the escorts, with the Freighter they guard;
  - **+ New route**.
- **The selected Freighter** shows one of three views:
  - **First shipment:** while the charter's Landfall shipment has not been
    dispatched.
  - **Trip status:** while a route run is served.
  - **Route editor:** otherwise, or when the player asks for it.

## First shipment

- **Itinerary:** Freya → Market Hub → Freya. Leg times use the route's own
  cruise. The ore load says when home has too little ore.
- **Return basket:**
  - The basket starts as `openingImportPlan`, and its − / + steppers stay within
    the hold.
  - A coverage line counts the plan's `deficit` (what home lacks for the
    Shipyard) and names what does not fit, with a link to the Exchange.
- **Trip:** freighter, escort (with a warning when none is ready) and spending
  cap.
- **Money:** estimated from the received prices, walking the book
  (`hubQuote`). Goods already in the Hub warehouse load before anything is
  bought.
- **Send shipment:**
  - It stages `SetFreightRoute` + `GuardFleet` behind the standard
    confirmation.
  - While a configure order is pending for the Freighter, the button reads
    "Order sent · confirming".
  - The guide's coach target is `send-shipment`.

## Trip status

From the served run only:

- the current stop and phase, with any hold;
- the ETA and leg progress, from the fleet's position and the route's cruise;
- the remaining units at the current stop;
- a stop list (Done / Now / Next / Later);
- **Edit route**, **Stop route…**, and **Run again…** once the run is complete.

## Editor

- **Settings strip:** freighter, escort, "Buy missing goods at the Hub" with
  its cap, and repeat. Cruise, Fuel kept on arrival and the route name are under
  Advanced.
- **Stops** sit side by side. Each has named cargo rows, a **+ Add** picker
  (a colony's stocked goods come first) and "Aboard after".
- **Blocked loads:** a load that would overfill the hold at a stop that is not
  a shopping Hub visit blocks Assign. Hub shopping only buys what fits.
- **Validation:** `freightRouteReason` is unchanged.
- **After Assign:** the editor closes once the order's pending report has been
  seen and has cleared.

## Editing a trip under way

Peter (Nov 5 2026): "I tried to modify the return items when the freighter
was still traveling to the hub but I couldn't save the changes."

- **Edit route** on a trip under way opens the editor as a revision
  (`Ui.revising`). **Assign route…** sends `SetFreightRoute` with
  `revise: true`; a new route, the first shipment and every bot route send
  it without.
- **In the sim** (`World::assign_freight_route`): a revision with the same
  ports in the same order keeps the run's place (`FreightRun::keeps_place`,
  `FreightRun::revised`). Bound for a stop, it takes that stop's new
  manifests on arrival. Working one (unloading, loading, fuel, escort), each
  transfer still to come moves by what the edit adds or takes off, so units
  already moved never move twice, an unload never past what is aboard, and
  added work sends it back to that step. A changed itinerary, a finished run
  or a plain assignment starts over at stop 1, as before; bots replan that
  way on purpose.
- **In the editor**, "Aboard after" counts from where the trip is
  (`routeResume`, `routeAboardFrom` in `core/derive/industry.ts`), and stops
  already done this trip read "Done this trip". It used to replay the route
  from stop 1 on top of the cargo aboard: the first shipment's 50 ore loaded
  twice read "100 of 50", which disabled **Assign route…**.

## Rendering

The panel reads more than fleets and systems.

- **Re-render inputs:** `industryServedSignature` adds:
  - the charter;
  - the Hub prices;
  - the pending orders;
  - a 2-second clock while a route runs.
- **Where it applies:** both the deck and the mobile sheet signatures include
  it.

## Code

- **Panel:** `client/src/shell/industry.ts`. It is shared with the mobile
  Logistics sheet, and the layout follows container queries.
- **Route helpers:** `client/src/core/derive/industry.ts` (`firstShipmentRoute`,
  `routeLegSeconds`, `routeCruiseSpeed`, `hubQuote`, `routeHubEstimate`,
  `routeAboard`, and `openingImportPlan().deficit`).
- **Styles:** `client/src/styles/industry.css` (`.lx-*`), plus the pop-out
  rules in `deck.css`.
- **Tests:** `client/scripts/test-industrial-expansion.mjs`, and the pop-out
  source checks in `test-market-workbench.mjs` and `test-planet-panel.mjs`.

## Goods picker (Nov 11 2026)

Peter: "the drop down should just be for the item and not include the price,
calculate the price after the player has picked the item", then "can we have
the dropdown styled in the stellar syndicate game style instead of a native
html dropdown?"

- **No native select.** The route editor's Add cargo picker is the game's own
  (`goodsPicker` in `client/src/shell/industry.ts`, `.lx-goodpick` in
  `styles/industry.css`): a button with the picked good's icon and name;
  open, the list unfolds in place under it (a scrolling stop column never
  clips it) with Resources · Parts · Modules tabs and icon rows.
- **Names only, and only goods research has opened** (`knownCommodities`),
  those the port holds first. A report never changes the list, so the game's
  morphing redraw leaves it open, scrolled where it was.
- **The price beside it** (`pickerQuote`), for the units typed: at the Hub,
  "≈ −N cr" when the route buys there (only what the warehouse lacks) or
  "≈ +N cr" when the stop sells, else what the warehouse holds; at a world,
  what it holds.
- A pick or Cancel closes it; on desktop Esc closes it first
  (`closeIndustryMenu` at the top of `escapeOneLayer`).
- The trip panel's money row names what it sells ("Alloys sale", `saleLabel`).
- Preview: `/scripts/logistics-fixture.html` on the Vite dev server
  (`?open=1` opens the list, `?stop=0` picks at the home world).
