# Hub district

Status: built Oct 15 2026, uncommitted. Start a fresh galaxy
(`--reset-galaxy`): homes, their doorsteps and the pirate bases are placed
when the galaxy is made.

Peter's call (Oct 15): every new player gets 2–3 nearby stars a few minutes
away, and every home starts about the same distance from the wormhole hub:
125k su, within a 50k su band.

## Homes

`galaxy::generate_home_slots` places them around the hub:

| Players | Layout | Distance from the hub |
| --- | --- | --- |
| 1–12 | One ring | 120–130k su (`HOME_RING_SU` 125k, ±`HOME_RING_JITTER_SU`) |
| 13–20 | Two staggered rows, alternating inner and outer | 100k and 150k su (the edges of the 50k `HOME_BAND_SU`) |
| 21+ | Two rows, crowded | until more hubs exist |

- No two homes are closer than 60k su (`HOME_SPACING_SU`): twice the doorstep's
  reach plus a buffer. A rival Interceptor needs at least 2:17 to arrive.
- One hub holds 20. An odd count in two rows leaves one place empty.
- Jitter is bounded so the spacing survives any roll.
- The server's `HOME_RING_SU` setting still moves the ring.
- **One hub's galaxy:** the server defaults to 20 players (`MAX_PLAYERS`). The
  galaxy radius grows with √players up to four, then holds at 400k su
  (`config::ONE_HUB_RADIUS`), so a full hub's frontier is denser, not farther.
  At √20 it would have been 894k su, with every inner home's neighbours cut
  short by the hub margin. More hubs will revisit this.
- **Frontier stars** keep 25k su from every home (`FRONTIER_HOME_CLEARANCE_SU`):
  homes are placed first, and the frontier redraws around them.

## Doorstep

Three stars per home, 15–20k su out (22k when the doorstep is crowded):
`exploration_stars.rs`, `DOORSTEP_SU`, `DOORSTEP_STARS`.

1. **The den system:** only a rich asteroid belt, where the privateer den
   stands ([founding-charter.md](founding-charter.md)).
2. **An icy stop:** a small ice world with Volatiles.
3. **A prospect:** one or two modest bodies, like the other sparse stars.

| One way, 15–20k su | Full speed | Slow (Uranium) |
| --- | --- | --- |
| Survey Probe | 0:40–0:48 | |
| Interceptor | 0:47–0:57 | |
| Tiny Freighter | 1:54–2:18 | 3:44–4:34 |
| Colony ship | 2:17–2:47 | |
| Mining Barge | 2:30–3:04 | 4:58–6:05 |

## Between the hub and the homes

Twelve small stars (`galaxy::HUB_GAP_STARS`) scattered 20–100k su from the hub
(`HUB_GAP_SU`), at least 12k su from any star and 25k su from any home. They
are ordinary sparse stars: survey targets, never habitable, modest prospects.

## Around the doorstep

- **Neighbourhood:** six systems 28–70k su out, two habitable, as before. Each
  now stands on its own home's side, nearer it than any other home
  (`home_neighbourhoods.rs`).
- **Pirates:** no enclave, depot or stronghold within 70k su of a home
  (`pirate::PIRATE_HOME_EXCLUSION`, was 2,600). That is about the gap the old
  74k su ring left to the pirate band.
- **Special systems:** every galaxy keeps at least one system with two
  habitable worlds. When no star rolls two, one habitable system's largest
  other terrestrial or ocean planet turns Terran (`thin_habitable_worlds`).

## The hub trip

| Home → hub, one way | Tiny Freighter, full | Slow | Hub news delay |
| --- | --- | --- | --- |
| 100k (inner row) | 8:58 | 17:54 | 50 s |
| 125k (one ring) | 11:04 | 22:04 | 62 s |
| 150k (outer row) | 13:08 | 26:14 | 75 s |

- At 150k a slow trip runs past the first-session limit of 25 minutes.
- A loaded Tiny Freighter can't reach the hub at full speed from 100k or more on
  one tank: it needs 61–92 Deuterium against 52.5. Early freight flies slow
  anyway; Fusion Freight fixes it.
- Escorts at full speed buy fuel at the hub for the trip home.

The 74k su trip costs in [fuel-overhaul.md](fuel-overhaul.md) scale with
distance: about ×1.7 at 125k.

## Bots

The bots' charter runs about 7 minutes slower: mining expanded at 29.6
game-minutes, against 22.7 on the old ring. A bot keeps a Hub round trip's fuel
in cash before it spends (`operating_reserve`), and that trip is now 1.7×
longer, so it waits longer for the Interceptor's semiconductors. Players aren't
held to that rule. The end-to-end test allows 36 minutes.
