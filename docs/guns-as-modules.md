# Guns as modules — design for review

Phase 2 of [ship-parts-modules-plan.md](ship-parts-modules-plan.md). Not
built. Peter (Nov 8 2026): "I don't like the built-in guns, let's have all
guns have to be modules"; "Corvette needs more hard points the interceptor
can't have 6 vs 2"; "Let's start the interceptor with 4 mount points"; mount
points are a part like Hull Sections.

## Mount points

Every warship has **mount points**, part of its hull like the frame. They are
the **Weapon Mounts** good (Armaments, "mounts, launch tubes, magazines, fire
control"): one Weapon Mount in the hull's recipe for a light mount point,
four for a heavy one (the Pulse Laser : Beam Battery ratio). +2 a rung, +4
into the Titan; from the Cruiser up, the new ones are heavy.

| Hull | Light | Heavy | Mount points | Weapon Mounts in the recipe | Full beam firepower (today) |
|---|---|---|---|---|---|
| Interceptor | 4 | – | 4 | 4 | 2.0 (3.0) |
| Corvette | 6 | – | 6 | 6 | 3.0 (1.0) |
| Destroyer | 8 | – | 8 | 8 | 4.0 (2.5) |
| Cruiser | 8 | 2 | 10 | 16 | 8.0 (4.5) |
| Battleship | 8 | 4 | 12 | 24 | 12.0 (8.0) |
| Dreadnought | 8 | 6 | 14 | 32 | 16.0 (15.0) |
| Titan | 8 | 10 | 18 | 48 | 24.0 (24.0) |

Freighters, the Scout, the barge, the Combat Freighter and the civil hulls
have none.

## Guns

- **Every gun is a module on a mount point.** Pulse Laser (light beam, 0.5)
  and Beam Battery (heavy beam, 2.0) leave the hull recipes and become
  modules; the Torpedo Rack, Mass Driver, Lance Array, Prismatic Lance and
  Point-Defense Screen move from the module slots onto mount points.
- **Sizes:** each gun comes light and heavy, heavy = four lights in
  firepower and price; a heavy mount takes a heavy gun. The Lance Array is
  heavy only.
- **Firepower is the fitted guns:** each gun's size firepower × its type's
  multiplier (torpedo 1.6, driver 1.3, lance 1.7, prismatic 1.45; Point
  Defense 0.5 and its screen). An empty mount fires nothing; an unarmed
  warship cannot attack and counts as a transport for targeting.
- **One weapon type per hull,** as today, so each weapon keeps its one
  counter and the battle engine one damage type a ship. Mixed fits (8 Beam
  Batteries and 4 Torpedo Racks) can come later.
- Today's duplicate stacking (two Torpedo Racks fire at 3.2× the hull's
  weight) is replaced by filling the mounts (a full rack fit is 1.6× its
  beams); the balance pass sets the prices.
- **Munitions** follow the guns: a rack's 40 Torpedoes and a driver's 60
  Slugs a gun, a Beam Crystal a beam gun.

## Slots and fitting points

Weapons leave them; they stay for armour, control and utilities, with the
budgets re-tuned in the balance pass (they were sized to hold weapons too).

## NPC fleets

Pirates, dens, Authority patrols and scripted spawns carry a fixed default
fit, their mount points full of the gun they fire today. The privateer and
the den are Interceptors (4 Pulse Lasers): their fights with the charter's
Interceptor take about 1.5× as long and end the same way.

## Charter

- The Landfall crate carries the Interceptor's 4 Pulse Lasers (6 today) as
  modules; the Shipyard's default fit fills the mounts from stock.
- The first raid takes every Weapon Mount in stock, so the Interceptor's 4
  mount points come back from the trader (it stocks Weapon Mounts).

## Battle rules

A new version, v18, with v17 frozen so archived battles replay exactly: a
ship's attack is its fitted guns, its damage type theirs. Fingerprints
re-pinned; `combat_balance` re-run at the default fits.

## Prices

Hulls lose their guns (Pulse Laser 110, Beam Battery 520 each) and gain
their Weapon Mounts (56 each); guns are priced as modules (recipe × 1.2, the
Weapon Mount now in the hull).

## Bots and client

- Bots build warships with the default fit and buy or make guns like any
  module.
- Shipyard: a mount points row and the fit picker filling it; Pulse Laser
  and Beam Battery move to the Modules tab.

## Go/no-go

Sim, server and client tests; `combat_balance` targets within their bands
at the default fits; the charter's privateer and den fights within 1.5× of
today's length; the bot charter run.

## For Peter

1. Mixed guns on one hull: later, as above?
2. Station hardpoints already take modules over a built-in beam: keep the
   built-in beam (stations are structures, not hulls)?
3. Light and heavy versions of every gun, as above?
