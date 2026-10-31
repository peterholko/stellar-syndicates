# Exotic stars and homes

Peter (Oct 25 2026), after a new production player landed at Alphard: "started
in a system with deep scan / pulsar star system, is that correct, that seems
wrong?" Then, on extending the rule to founding targets: "ok".

## What was wrong

- **Exotic came from the id alone.** A system's star came from an FNV-1a roll of
  its id. One id in six is exotic: pulsar, binary, black hole or magnetar. The
  sim seeded a node at every exotic id, homes included.
- **Alphard was unlucky.** Five homes take ids 33–37, and 37 rolls a pulsar.
  Alphard was the last free home slot, so the next player to join got it.
- **The node ate the starting stores.** The node woke at 180 s. Once a player
  held it, it took 4 Provisions and 2 Deuterium a second from home whenever both
  were in stock.
  - The new player's 10 starting Deuterium and about 20 Provisions went in the
    first 5 seconds.
  - After that it would have eaten any Deuterium they brought home.
- **The map labelled it.** Holding an awake node puts the violet ring and
  "◈ DEEP SCAN" over the home.
- **Founding targets too.** The same galaxy put Bifrost's charter garden (44)
  on a binary star with a Deep Scan node.

## Why a node is no gift to a new player

- **The bonus:**
  - It is tactical only, and only one node per corporation counts.
  - It works within 1,800 su of the star. Planets orbit within about 820 su, and
    the nearest other star is at least 12,400 su away, so it is that one
    system.
  - Deep Scan gives exact counts for fleets already visible there; a parked
    ship or a Sensor Array gives the same. Relay Anchor halves the light delay
    for orders there. Veil halves how far your dark fleets there can be seen.
- **The upkeep:**
  - 4 Provisions and 2 Deuterium a second, about 64 Cr a second at base
    prices.
  - The Provisions alone are 17× what a new 6,000-settler colony eats.
- **For tuning later:** the region predates the 125,000 su home ring, and every
  node wakes 180 s in.

## The rule

A home and its two founding targets (the charter's garden and mineral world)
are never exotic, whatever their ids roll.

- **Sim:** `World::exotic_bonus(id)` is `node::node_bonus_for(id)`, except that
  a home slot's system and its `founding_opportunities` get `None`.
  - Both node seeders use it: `seed_nodes`, and the exploration stars in
    `world/exploration_stars.rs`.
  - The targets are fixed at generation and never change, so neither do the
    stars.
- **Wire (v62):** `SystemInfo.exotic` carries `exotic_bonus(id).is_some()` in
  Welcome and GalaxyUpdate.
- **Client:** `chartStars` (`stars.ts`) keeps the served flags. `starTypeFor`
  uses the flag to choose between the exotic and ordinary pools.
  - The id's roll still picks the star within its pool, so a node's star matches
    its bonus.
  - A system outside the served chart (fixtures, demos) keeps the roll.
- **Homes minted past the slot pool** go into `home_slots`, so they are ordinary
  too.
- **Dens can still be exotic.** A magnetar privateer den fits, and nobody settles
  there during the charter.
- **No old-galaxy code.** Galaxies generated before this keep their starter
  nodes. Reset them.

## Production

Production was reset on Oct 25 2026 with this build (release
`522b4a1-wip-20261025-homes`). The previous saves are in the reset's
`archive-<uuid>` folder.

## Checks

- `world::tests::homes_and_founding_targets_are_never_exotic`: production's
  galaxy, seed 0xC0FFEE with five homes. There, home 37 rolls a pulsar and
  target 44 a binary star. It checks two things:
  - no home or target gets a node;
  - every other exotic roll keeps its node.
- `client/scripts/test-star-rendering.mjs`: the served flag overrides the roll,
  and a new chart replaces the old.
