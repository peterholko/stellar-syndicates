# Playtest galaxy layouts

Galaxy geography is deterministic from the seed, maximum player count, home-ring
distance, and the generator version. These presets preserve the two layouts used
during the August 2026 economy/comms playtest.

## Archived map — wide home ring

- Seed: `12648430` (`0xC0FFEE`)
- Maximum players: `4`
- Galaxy radius: `400000 su`
- Home-ring base radius: `248000 su` (`0.62 × galaxy radius`)

Recreate its static chart with this code revision:

```bash
GALAXY_SEED=12648430 MAX_PLAYERS=4 HOME_RING_SU=248000 scripts/start.sh
```

This preserves the generated map, not the old in-memory corporations or elapsed
simulation state.

## Near-hub map — one buoy hop

- Seed: `12648431` (`0xC0FFEF`)
- Maximum players: `4`
- Galaxy radius: `400000 su`
- Home-ring base radius: `74074.074 su`; ±8% radial jitter puts every home at
  approximately `68148–80000 su`, inside one long-throw buoy

Start it with:

```bash
GALAXY_SEED=12648431 MAX_PLAYERS=4 HOME_RING_SU=74074.074 scripts/start.sh
```
