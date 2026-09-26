# Local opening and first export

## Player flow

New corporations keep the 2,000-population home, 50-capacity Tiny Freighter and
Interceptor, and receive one Scout. Existing corporations are not migrated or
given replacement starter goods.

The opening guide presents parallel actions:

1. Build and staff the Mining Complex.
2. Investigate a nearby recovery beacon with the Scout. The existing threat
   avoidance rules still apply.
3. Assign the Interceptor to guard the Freighter, recover the cache and deal
   with the local privateer. The Hub voyage no longer triggers this encounter.
4. Bring the finite cache home and fit its Recon Suite to the Scout.
5. Choose return supplies, review the route and spending cap, then confirm the
   guarded ore export. Dispatch is the stopping point for the first session;
   watching the Hub arrival or sale is not required.

The cache contains 20 Alloys, 4 Machinery and one Recon Suite. Investigation
reveals it; a real cargo hull must recover and transport it. Reconnecting or
loading a save does not replenish it. Only its founder can investigate or
recover this tutorial cache.

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

Logistics offers editable one-off home → Hub → home drafts for:

- Transport: Shipyard, then a second Tiny Freighter.
- Refining: Academy, Enrichment research, then Smelter.
- Exploration: materials for restoring a discovered station as an observatory.

Drafts use received home stock and the public build recipes. Each return basket
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

## Exploration and information delay

Fresh galaxies reuse two extra-star slots near each starting home: a rocky
survey prospect with ruins and an icy prospect near a station. The original
systems and total habitable-planet budget remain unchanged. Existing galaxies
are not regenerated or relocated during save loading.

Cache reports, recovery completion, fitting readiness, battle outcomes and
export dispatch respect report travel time. Actual sales remain independently
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
