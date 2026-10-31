# Engagement rules: warships and pirates fight when they meet

Peter (Oct 28 2026): "I sent my interceptor to hunt the pirate and why didn't
they engage into a fight once they got close together, I shouldn't have to
send an attack order, both should have engagement rules."

Before this, a battle opened only when a fleet held an Attack or Intercept
order on another and closed to 80 su. Fleets on a plain move or idle never
fought, and the doctrine's own engagement covered only patrolling pickets
guarding convoys. A hunting Interceptor and the lurking privateer could sit
side by side.

## The rule (`crates/sim/src/world/close_contact.rs`)

Each tick, after the doctrine and weapons-free passes and before battles
resolve, within `CLOSE_ENGAGE_SU` (2,000 su) of each other:

- **Pirates:** an idle pirate warship attacks the nearest corporation warship.
- **Corporations:** a warship that is idle or travelling attacks the nearest
  pirate warship its corporation senses (the same `detection::detected` the
  View uses: command center, its own sensor-carrying fleets, Sensor Arrays),
  unless the corporation's doctrine is **Avoid**.
- Each side gets an ordinary `Attack` order, so the usual contact resolution
  opens one battle for the pair.

Left to their own rules: fleets already fighting; guards, defenders, patrols
and blockades; pirates on any other order (a scripted run such as the
founding raid's flight or its trip home is never diverted). Warships only:
freighters, scouts and other unarmed hulls never start one. Fights between
corporations still need an order or a doctrine. Nothing opens inside the
Market Hub's sovereign zone.

A travelling fleet pulled into a fight stops there afterwards; its move isn't
resumed.

The founding guide's hunt card now reads "Interceptor lying in wait · It
attacks on contact."

Tests: `world::close_contact::tests` (both sides commit, out of range nothing
moves, Avoid holds fire while the pirate still comes, a travelling pirate and
a guard keep their rules).
