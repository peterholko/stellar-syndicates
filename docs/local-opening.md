# Local opening and first export

> **Superseded in part (Oct 8 2026):** the founding charter is now v3, in
> [founding-charter.md](founding-charter.md). There is no starting Scout,
> recovery beacon or cache, and the Growth chapter became Industry and Reach.
> The storage, Service Berth, shopping-cap, refining-floor, focus and
> welcome-back rules below still hold.

## Player flow

New corporations keep the 2,000-population home, the 50-capacity Tiny Freighter
and the Interceptor. The v3 **Landfall** chapter has three steps: bring the mine
online, send the first guarded shipment, and defeat the Rogue Privateer. The
guide card lists what is already under way and gives one next step with a
one-sentence reason. When a step cannot proceed, it says why in amber: the
Freighter was lost, or home storage is full. The Log's founding directive, the
mobile card and the Log entry all run that same step. After a guide link opens
a screen, a pulsing ring points at the exact control to use.

### The scripted privateer

Guard targets may be docked fleets; the command strip lists your fleets as
buttons while aiming a guard order.

Only a guarded trip summons the scripted Rogue Privateer: the first escorted
Freighter to leave its gravity well, normally the first shipment. It appears 4,000–9,000 su ahead along the route (at most 60% of the
leg) and 6,000 su to one side, inside ordinary sensors. An escorted Freighter
keeps working through it; its autonomous threat retreat ignores the scripted
privateer. Unescorted hulls still turn back from real threats, and the guide
says so ("Your Freighter turned back … send it again with your Interceptor as
escort") once that report arrives.

If the starter Freighter is destroyed during Landfall, before the export
leaves, and no other Freighter exists or is being built, the Terran Charter
Authority charters one replacement Tiny Freighter at home, once per
corporation. An unengaged privateer withdraws, and the lesson resets for the
next guarded trip.

### Storage on the return leg

A route that reaches its full home port first loads that stop's pickups (for
example, the ore it is about to export) and then unloads into the space it
freed. If there is still no room, it holds with "Storage full at Home
(used/cap); unloading waits for space", and the guide turns that into **Make
room at home**.

## Service Berth

The new one-tier Warehouse attachment costs 5 Alloys and 3 Machinery and takes
120 seconds to construct. It does not consume an additional structure slot.
It supports utility refits on Scouts, Interceptors, Tiny Freighters and Small
Freighters, not shipbuilding, module manufacture, combat fitting or repairs.

A staffed Shipyard remains preferred. Otherwise an unstaffed Service Berth can
temporarily borrow one generic Mining Complex worker. The mining assignment
is restored on completion or cancellation, unless a later manual posting has
superseded the loan. It never borrows food-production crews or creates workers.

## First return trip and later investments

Logistics offers one editable home → Hub → home draft, **Shipyard materials**:
sell the ore and bring part of the Shipyard kit home. The rest is booked on
Authority freight from the Market. Drafts use received home stock and the public build recipes. Each return basket
contains 10 Fuel and at most 40 units of project materials. A larger project
kit is explicitly shown as requiring further deliveries. Choosing a draft
sends no orders; assigning it uses the normal fleet confirmation, including
the guard order.

The default cargo-purchase cap is 3,000 credits per Hub visit, including market
fees, and is editable. Goods are sold only after physical unloading. Purchases
then use actual prices, liquidity and cash at arrival, retaining cash for local
tank service. Unaffordable or unavailable wishlist items are skipped so the
ship can return with a partial load. Normal fuel, escort, storage and supply
constraints still apply; tank-service charges are separate from the cargo cap.
Old routes without a shopping budget retain their warehouse-only behavior.

A non-repeating three-stop return to the first port requires no automation
research. Recurring routes with more than two stops still require Autonomous
Freight. Larger freighter research gates are unchanged.

An optional Smelter policy leaves 50 units of each ore and 20 Fuel untouched by
refining. These floors are in addition to project reservations. Freighters may
still export that ore, and ships may refuel. The production planner deducts the
received floors from usable input stock.

## Charter stipends, ledger and founder shield

The Authority pays a small one-time stipend when each milestone is first
reached. Each payment is announced on the timeline ("Charter milestone · … ·
the Authority paid N Cr.") and as a toast on both shells. The v3 stipends are
listed in [founding-charter.md](founding-charter.md#stipends). The 4,500 Cr
privateer bounty is announced the same way. Replacement hulls are announced
separately.

Once the charter is fulfilled, the **Charter ledger** opens on Command (and on
the mobile Operations sheet): 23 tiered goals, three visible at a time and each
paid once, in any order. "Join or found a syndicate" leads, with any waiting
invitations. Later goals cover an Exchange limit order, storage, population,
refining, a second Freighter, hull count, contracts, research, a second system
and Tier II structures. Rewards stay at 2,000 Cr or less. A corporation that
opens the ledger long after its charter has goals it already met recorded
without payment, so old saves do not receive a windfall.

Founder protection now lasts until 24 hours after the programme completes,
never less than 24 hours from its start, and never more than 48. An unfinished
programme keeps the 48-hour ceiling. The guide shows it in hours and minutes,
with what it covers. The 30-minute minimum only governs when a founder may end
it early by attacking another corporation.

## Focus while landing

Until the first guarded export leaves home, the follow-up board offers one
contract at a time, starting with Salvage, and each such offer stays open for
24 hours instead of expiring within the session. The Authority's two rescue
contracts (Hold the recovery zone, Protect the evacuation) also wait until the
export has left. The desktop Log keeps the
founding directive and real threats on top and folds other decisions (node
broadcasts, distant hostiles that are not closing, contract offers, survey
opportunities, idle-stock and storage notices) into one "Also happening" line.
**Show all** restores the full inbox.

Battle notices say who won from the reader's side ("Victory: Your Interceptor
destroyed the Rogue Privateer", "Loss: …"), and refit starts name the ship and
modules in the log.

## Welcome-back report

On return after at least ten minutes away, both shells show one charter report
per session. It lists arrived sales and purchases from the transaction ledger,
milestone payments, and how many reports need attention. It then gives the
guide's under-way lines and next step, with a button that runs that step.
Nothing is reported before its receipt light has arrived. Push notifications
are not part of this change.

## Exploration and information delay

Fresh galaxies reuse two extra-star slots near each starting home: a rocky
survey prospect with ruins and an icy prospect near a station. The original
systems and total habitable-planet budget remain unchanged. Existing galaxies
are not regenerated or relocated during save loading.

Mine, shipment, battle, survey, belt-mining and haul reports respect report
travel time. Actual sales remain independently
recorded in the transaction ledger; the guide does not fabricate an early sale
to advance. Commands continue through ordinary delay and confirmation paths.
Bots use received observations to follow the local opening as well.

## Verification and rollout

Tests cover finite/private recovery, save/reconnect behavior, old-save defaults,
report delay, local encounter triggers, dispatch handoff, berth slots and crew
restoration, refining floors, guarded route confirmation, capped shopping,
partial returns, fuel cash protection and nearby-star/habitability invariants.
The ordinary simulation and server unit suites, relevant client fixtures,
TypeScript checks and a temporary production bundle have been checked.

The subsequent [founding-camp flow](founding-camp.md) adds explicit settlement
confirmation, three home-world profiles and optional local developments.
The client/server protocol is version 40 and must be deployed together. No live
server restart or galaxy reset is part of this change. The 35–45-minute opening
is a pacing target, not a measured playtest result; travel and combat pacing
still need a real-time fresh-founder playthrough.
