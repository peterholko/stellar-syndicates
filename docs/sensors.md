# Sensors

Status: built Oct 16 2026 (needs a fresh galaxy: the Command Center range is
stored in the save).

Two ways to know a ship is there:

- **Transponders.** Declared hulls announce themselves (Corvettes and every
  warship above them, colony ships, transports, Builders, Authority ships).
  They are heard out to 200,000 su from your receivers: your Command Center,
  Sensor Arrays and deep-space sensors. Past that the signal is lost in the
  galaxy's noise.
- **Sensors.** Everything else, pirates included, is found by what its core
  puts out. Small, still or dampened ships have to be close.

Nothing is detected past 200,000 su, however loud (`detection::NOISE_HORIZON_SU`).
Battles follow the same horizon; your own battles always show.

## Detection

```
seen ⇔ distance ≤ min(sensor range × √(core output), 200,000 su)
```

A sensor's range is the distance at which it sees one Raider at full
throttle (output 1.0). Four times the output is seen at twice the distance.

**Core output** = hull and cargo mass × rated speed × state, in Raiders.
It is the drive's own per-second work (`fuel::fuel_tick`), read off the
drive, never the tank: pirates and the Authority burn no fuel in the sim,
and fuel research or a captain's thrift must not double as stealth.

| State | Output | Seen at (× range) |
|---|---|---|
| Full throttle (warp or thrusters, the same burn per second) | 100% | 1.0 |
| Working in place: mining, prospecting, surveying, building, demolishing, an anchored belt depot | 50% | 0.71 |
| Slow warp (Uranium: half speed, a quarter of the energy per distance) | 12.5% | 0.35 |
| Stealth (Slow warp's output, dampened ×0.6) | 7.5% | 0.27 |
| Holding, core on standby | 5% | 0.22 |

A survey sweep keeps its extra ×1.5 on top. Nebulas and a Veil node scale
the result as before.

Full-throttle output by hull (examples): Survey Probe 0.24, Scout 0.46,
Raider 1, Corvette 2.6, Tiny Freighter 3 (empty), Destroyer 5.5, Cruiser 9,
Titan 37.

## Ranges

| Source | Range |
|---|---|
| Command Center | 6,000 su |
| Sensor Array | I 20,000 su, +10,000 a tier (II 30k … VI 70k) |
| Deep-space sensor | 12,000 su |
| Interceptor (Raider) picket | the Command Center's 6,000 su (9,000 with a Recon Suite) |
| Scout with a Recon Suite | 3,000 su |
| Freighter traffic sensor | 1,500 su |
| Scout contact sweep (exploration sites) | 30,000 su (unchanged) |

What the numbers mean around a home (100,000 su from the Hub):

- The Command Center sees a holding Raider only inside 1,340 su, so nothing
  hides in your own 1,300 su near zone, and a Raider moving in at 6,000 su.
- An Array I covers the three doorstep stars (15,000 su) for anything under
  way. It sees the den's three holding Raiders only within 7,700 su.
- Array VI first reaches rival homes (60–63k) and pirate space (70k+).

Sensor Arrays and deep-space sensors also find exploration contacts.

## The founding charter

- §charter-v5: the privateer raids the colony as the mine comes online. It is
  a tracked contact from the moment the colony first sees it (`raid_seen_at`,
  by the shared detection rule; `view::tracked_contacts`): it carries its loot
  to its den, far below what home sensors can see. The battle estimate and
  the bots get the same track.
- §den-retreat (Nov 12 2026): it docks with its den's garrison, and the den
  is known as the hunt (step 12) begins, since the colony saw where he went;
  no sensor reveals a pirate base. Without a den it lurks just outside the
  colony's sensors.
- The lesson's fallback belt is picked within 80,000 su of home (the old
  bubble, now a plain distance).

## Where it lives

- `crates/sim/src/detection.rs`: the rule, output states, horizon.
- `Fleet::core_state`, `core_output`, `signature`, `detection_signature`
  (`ship.rs`): every sim sensor (pickets, defenders, hunters, research
  observation, pirates) reads these.
- `crates/server/src/view.rs`: each served sample records its fleet's core
  output, so the View judges it in the retarded frame; `heard` gates
  transponders.

## Still open

- Ship sensor modules: today's ship bubbles are fractions of the Command
  Center's 6,000 su base, not designed numbers.
- Whether sensor contacts should stay unidentified until something of yours is
  close, or a probe looks.
- Finding the den with a Survey Probe instead of the privateer's log.
- A "Run silent" order (transponder off, core down).
- A balance run for bots (exploration and threat sensing are now short-ranged).
