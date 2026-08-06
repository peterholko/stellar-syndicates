# Playtest galaxy layouts

Galaxy geography is deterministic from the seed, maximum player count, home-ring
distance, and the generator version. These presets preserve the two layouts used
during the August 2026 economy/comms playtest.

`§jump-v1` breaks snapshot compatibility: regenerate these presets rather than resuming a
lane-era world. Their star systems, homes, and Market Hub chart positions remain unchanged
for a given seed; only the removed lane/relay layer is absent.

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

## Near-hub map — preserved lane-era spacing

- Seed: `12648431` (`0xC0FFEF`)
- Maximum players: `4`
- Galaxy radius: `400000 su`
- Home-ring base radius: `74074.074 su`; ±8% radial jitter puts every home at
  approximately `68148–80000 su`. The distance is retained even though buoys are gone.

Start it with:

```bash
GALAXY_SEED=12648431 MAX_PLAYERS=4 HOME_RING_SU=74074.074 scripts/start.sh
```

## Jump-v1 pacing

All news and commands now travel directly at 2,000 su/s (`c × WARP_FACTOR`). On a four-player
chart, hub-to-rim light takes about 200 s. A nominal home-to-hub order takes about 37 s. Jumps
move hulls, not information, so neither figure is shortened by a relocation.
