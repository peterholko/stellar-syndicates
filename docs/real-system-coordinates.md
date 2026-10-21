# Real coordinates inside star systems

Status: design note, third revision Oct 20 2026 after Peter's review. Nothing
built.

Peter (Oct 20 2026): a ship's position didn't line up between the galaxy map
and the System View; asked for "actually have coordinates on the star system
view". Today the sim has no positions inside a system: every fleet that
arrives at a star flies to the star's centre, and both views draw a stylised
layout on top (berths beside worlds, a band outside the orbits).

Peter's review: the worlds keep orbiting ("I really like that small visual
feature"); docked ships are never shown on the galaxy map; the ship icons in
orbit beside their worlds stay; and no ship stops at a star's centre.

## The model

- **Worlds get real orbits.** Each planet orbits its star at a real radius, at
  the angle and speed the System View already animates; a moon orbits its
  planet; a belt is a ring at its rolled radius. Positions are a function of
  the galaxy seed and time, never stored per tick. Scale: one System View unit
  = 860 su, so worlds orbit 190–820 su from the star, inside the 900 su gravity
  well, and the System View's frame reaches about 1,300 su (the defences'
  reach).
- **Every ship at a system is at a world**, never the star's centre:

  | Who | Goes to |
  | --- | --- |
  | Your or an ally's ship, docking | Its berth world (table below) |
  | A visitor (scout, survey, trader) | The main world |
  | A colony ship | The world it will settle |
  | Pirates raiding a colony | The colony's world |
  | Blockaders and sieges | Around the besieged world |
  | Defence posts | Around the defended world |
  | Pirate packs at a base | The base's world |
  | Barges, Mining Belt Stations | A point on the belt's ring |
  | Skimmers | Beside the gas giant |
  | A system with no planets | A point on its belt (the den) |

- **Staying at a world means moving with it.** A ship docked, on station or
  looting at a world follows it each tick: no fuel, no thrust, not counted as
  under way by sensors. After a fight, ships that live at a world fly back to
  it.
- **Berth world by role** (the System View's rule, moved into the sim):

  | Role | Berth world (first match) |
  | --- | --- |
  | Warship | Capital Slipway → Naval Drydock → Shipyard → Garrison → Command Center |
  | Freighter | Orbital Warehouse → Warehouse → Service Berth → Command Center |
  | Transport | Garrison → Command Center → Naval Drydock → Shipyard |
  | Other | Shipyard → Command Center → Warehouse |
  | Free trader | Main world |

  The main world is the most populated, then the Command Center world, then
  the first habitable, then the largest.
- **Arriving and leaving.** A ship drops out of warp at the edge of the
  gravity well and flies in on thrusters to where its world is at that moment;
  leaving, it starts from its world. One change in the movement step covers
  the ~60 places that send a fleet "to a system".

## Rules measured from the world instead of the star

- **Docking:** an idle ship inside the well of an owned or allied system is
  docked (was: within 260 su of the star). The wire's `docked` string keeps
  naming the system, so the client and bots read it as before.
- **Merging, splitting, joining, rejoining after refit:** docked at the same
  system (was: within 80 su of the star).
- **Colony claims:** within 80 su of the world being settled.
- **Landings:** marines at the target world.
- **Blockades:** on station and bombarding around the besieged world; the
  blockade-run zone grows to the well, so freighters bound for outer worlds
  still roll to run it.
- **Pirate assaults, salvage and prize recovery at a base:** at the base's
  world (was: the base star, 220/300 su).
- **Battles:** open where the ships meet, at true positions (a raid on a docked
  fleet happens at its world). Docked warships of the system join a fight
  inside its well (was: within 200 su). A docked Corvette's escort reach is
  measured from the star, so a garrison still covers the whole system.
- **Authority escort contracts:** complete when the freighter docks at its
  destination system.
- **Bots:** their position checks against a star move to the world or the
  well.
- **Unchanged:** the Hub, defence-platform cover (1,300 su from the star),
  scout intel range, the gravity well, detection.

## Server

- `BodyView` carries each world's orbit, for charted systems only. Uncharted
  systems still send no bodies, but rival ships flying to their worlds there
  show where those worlds are (not what they are).
- Protocol v56; the galaxy needs a reset.

## Client

- Worlds, moons and belts from the served orbits; the client's own seeded
  layout goes. Planet sizes, art, light, motion and star effects stay.
- **System View at true scale:** ships at their real positions on the tilted
  plane. Docked ships keep their icons in orbit beside their world and the
  "+N docked" count; the siege cordon is now where the blockaders really are.
  The 260–1,300 su band goes; edge arrows beyond 1,300 su stay.
- **Galaxy close-up at true scale:** the orbit preview grows with the camera;
  ships in flight keep their real distance and bearing, and only the System
  View's tilt blends in over the 40→96× dive. Docked ships stay hidden at every
  zoom, with the ⚓ count under the star. The band warp from Oct 20 goes.
- The handoff starts the System View at the preview's true scale.
- Fix on the way: a ship docked at a blockaded system was meant to stay
  visible, but an id mismatch (`"E29"` against `"29"`) hides it.

## Risks

- Arriving and leaving take a few seconds more on thrusters (up to ~40 s for a
  slow hull crossing to a world on the far side), with fuel to match. Bots'
  timings shift a little.
- A fight stays where it started while its world orbits on.
- Many sim tests park fleets at a star's centre and need their world instead.

## Phases

1. Sim: orbits, berth rule, arrivals and departures, riding a world, docking
   inside the well.
2. Sim: the re-measured rules (merging, colony, landings, blockades, bases,
   battles, contracts), bots, tests; the bot charter test must still pass.
3. Server and client: orbits on the wire, true scale in both views, the
   handoff, the berth bug.
4. Reset :8080 (with the dev Interceptor) and update the docs.
