# Signals — version 1

Thirteen paintings for the planned Signals panel: the card art
for each signal type, reused for its report. One painting per type, a single
series in the look of the story-event paintings
([../story-events-v1](../story-events-v1/README.md)). The prompts are in
[prompts.json](prompts.json). All thirteen masters are generated. Exact prompts,
references, selected outputs and verification details are recorded in
[generation-records.json](generation-records.json), with the first painting's
history in [drifting-wreck.generation.json](drifting-wreck.generation.json).

| Master | Signal | Hazard |
| --- | --- | --- |
| [drifting-wreck.png](drifting-wreck.png) | Drifting wreck | Routine |
| [cargo-pod.png](cargo-pod.png) | Cargo pod | Routine |
| [ore-fragment.png](ore-fragment.png) | Ore fragment | Routine |
| [silent-probe.png](silent-probe.png) | Silent probe | Routine |
| [distress-beacon.png](distress-beacon.png) | Distress beacon | Routine or Hazardous |
| [sensor-ghost.png](sensor-ghost.png) | Sensor ghost | Routine |
| [authority-courier.png](authority-courier.png) | Authority courier | Routine |
| [abandoned-outpost.png](abandoned-outpost.png) | Abandoned outpost | Hazardous |
| [pirate-stash.png](pirate-stash.png) | Pirate stash | Hazardous |
| [ghost-ship.png](ghost-ship.png) | Ghost ship | Hazardous |
| [precursor-fragment.png](precursor-fragment.png) | Precursor fragment | Hazardous |
| [exotic-flare.png](exotic-flare.png) | Exotic flare | Hazardous |
| [teos-lost-crate.png](teos-lost-crate.png) | Teo's lost crate | Routine |

## Brief

- Style: `../founding-intro-v1/first-landing.png` established the look in the
  first painting, which then carried the style into the rest of the series, plus a
  subject, livery or colour reference where the prompt names one (the Survey
  Probe, the Authority freighter, the privateer, the corvette, the Long Odds,
  the exploration sprites, a belt chunk, the precursor nebula).
- Series: `drifting-wreck` was generated first and used as the series
  reference for all twelve remaining subjects.
- Framing: approximately 16:9 at native size. Twelve masters are 1672 × 941;
  `precursor-fragment.png` is 1671 × 941. One subject near the centre; every essential detail inside
  the central 60% of the width and the middle 45% of the height, so the
  painting survives the mobile banner crop (about 21:9) and still reads as a
  384-pixel card. Quiet dark space at the edges.
- Each painting has one accent colour (amber, red-orange, violet, gold…) and
  no text, emblems, people, explosions or interface. The hazard shows in the
  panel, not the painting.

## Delivery

Each master is saved here under its slug as an opaque PNG at native size,
without cropping or upscaling. `npm run assets:derive`
(`scripts/build-signal-art.mjs`) writes WebP to
`client/public/art/derived/signals/v1/`: the full frame at 768 and 1280
pixels wide for the detail and report, and the card crop (the central 62% of
the width, 16:9) at 384 and 640. Until a painting lands, its card shows the
type's line icon.
