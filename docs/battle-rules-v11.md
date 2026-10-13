# Battle rules v11: roles for small ships

Superseded for new battles by v12 (`docs/battle-rules-v12.md`), which keeps
these roles and fires one shot per weapon. Battles opened on rules v11
(`crates/sim/src/tactical/v11.rs`) keep it. Small, fast
hulls now earn their place through jobs big ships can't do (holding a fleet
that tries to leave, slowing what they fight, killing other small hulls)
instead of out-gunning everything per credit. Everything else (v10's
formations, v9's bodies, wreckage and slow turning, v8's weaving, v7's
reach, every other weapon, roll and range) is kept.

## In battle

- **Tackle.** Interceptors hold. A hull that tries to leave the battle (its
  side withdraws, or its fleet's damage withdrawal trips) can't cross the edge
  of the fight while more live enemy interceptors are within 280 units of it
  than it carries Warp Stabilizers. It stops at the edge and keeps getting shot
  until the tacklers die or move off; then it leaves. Interceptors close on
  whatever is leaving, the heaviest hull first, one more of them than each
  hull carries stabilizers, orbiting it at flank speed (their speed is still
  their armour). When a whole side is leaving, every interceptor gives chase.
  A leaving hull now heads straight out along its side's line, as fleet
  withdrawals already did.
- **Stasis Web** (module, 2 fitting points). Slows the nearest enemy hull
  within 240 units to half its top speed. Hit chance reads a target's current
  speed, so a webbed hull is easier to hit and slower to leave. Webs don't
  stack.
- **Warp Stabilizer** (module, 2 points). Each one lets the hull shrug off one
  tackling interceptor.
- **Light batteries.** A destroyer's stock beams (or point-defense beam) fire
  as light batteries: 4 shots a round, each carrying 0.3 of a beam, with a
  speed sensitivity of 1.5 (a stock beam's is 3.6), so they track small, fast
  hulls. Each battery shot picks its own target, strongly favouring small
  hulls, and heavy armour shrugs off part of it: damage falls with
  (2000 / mass)^0.5 above destroyer mass (half against a battleship). Against
  a darting interceptor a battery shot hits about twice as often as a beam.
- **Light guns.** An interceptor's beams and drivers hit destroyer-sized and
  bigger hulls (and platforms) for 0.3 of their weight, a corvette's for 0.5,
  and like batteries they barely scratch heavy armour. Against other small
  hulls and freighters they hit in full, so dogfights and raiding are
  unchanged, and torpedoes are never light.

## On the map

- **Interdiction Field** (module, destroyer only, 3 points, one per hull). The
  fleet's new order "Raise interdiction field" drops a temporary gravity well
  where the fleet is when the order reaches it: 1,500 su in radius, for up
  to 10 minutes, while the destroyer stays inside it and alive
  (the fleet stops to hold station). Every fleet inside moves on thrusters (a
  fleet in warp drops out as it enters, so crossing one is five times slower)
  and no jump can start or end inside it. Raising one burns 20 Fuel, and the
  generator needs 30 minutes before the next. Fields can't be raised inside a
  gravity well or another field. Everyone sees a field once its light arrives
  (it is drawn on the map in your colour or the threat colour), and the
  owner's timeline reports it rising and collapsing.
- **Refusing a fight.** A fleet whose doctrine avoids battle used to get away
  if it was faster than the fastest enemy ship. From v11 only interceptors
  hold it, and only those in fleets at least as fast as it: it gets away
  unless they outnumber the fewest Warp Stabilizers any of its hulls carries.

## Measured effect

The balance matrix (`docs/combat-balance.md`) against the v10 baseline, at
equal cost, 100 fights per pairing in each role (report:
`docs/balance/combat-balance-v11.html`):

- No fleet type dominates any more. The best field score falls from 100%
  (stock-beam interceptors, every size) to 75–79%, and the equilibrium mix
  grows from one type to 3–7: interceptors with beams and with torpedoes,
  armored cruisers, destroyers, torpedo cruisers, and titans with torpedoes or
  drivers.
- Interceptors with beams now lose to beam cruisers, armored cruisers, beam
  battleships, destroyers and escorted battleships (0–6%), and still beat
  fleets that can't hit small hulls: driver and torpedo fits of every size
  (their average against line ships is 58–69%).
- Catch and hold: interceptors hold driver battleships and titans (0–1% get
  away); beam cruisers and escorted battleships kill the tacklers and leave;
  nothing but interceptors holds anything; torpedo interceptors hold and sink
  what they catch.
- Most pairings are still decided 90/10 or worse (75–81%): fights at these
  sizes are close to deterministic, so the next tuning pass should aim at value
  trades as well as win shares.

Tests re-earned: the platform test uses 12 interceptors (six can no longer
dent a platform tier), and the wreckage replay test uses torpedo
interceptors to sink its crippled capitals.

## Test fights

`client/public/dev/battles/` adds "Interceptors hold a retreating battle line"
(`lockdown`: webbing interceptors pin two battleships and two cruisers at the
edge from round 12 and torpedo interceptors sink them there) and "Destroyer
escorts against an interceptor swarm" (`escorts`). Regenerate after any
combat change:

```bash
BATTLE_FIXTURES_WRITE=1 cargo test -p server battle_fixtures
```
