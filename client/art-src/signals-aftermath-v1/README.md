# Signals: aftermath, version 1

Thirteen paintings for the signal report panel (`signalReportEvent` in
`client/src/story-events.ts`; docs/signals.md, "The report panel"): each is
the same scene as its signal's card painting in
[../signals-v1](../signals-v1/README.md), minutes after a captain's crew has
worked the site and turned for home. Peter (Nov 11 2026) chose these over
reusing the card paintings: "a second painting per type showing the
aftermath rather than the find". The prompts are in
[prompts.json](prompts.json). All thirteen masters are generated and saved as
opaque 1672 × 941 PNGs. Exact submitted prompts, framing revisions, references,
selected native outputs and file verification are in
[generation-records.json](generation-records.json).

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

- References: input image 1 is the signal's own card painting (scene,
  style and series), so each pair reads as before and after; input image 2
  is the player's Interceptor render (`client/public/art/derived/ships/models-v1/previews/raider-768.webp`), the visiting ship.
- Framing: as the card paintings: 1672 x 941, the site near the centre,
  everything essential inside the central 60% of the width and the middle 45%
  of the height, quiet dark edges.
- The visiting ship is small (at most about 15% of the width), never over
  the site, and heading home.
- One painting serves every outcome, so it shows the work done (a cut hatch,
  an opened lid, a planted beacon, scorch marks), not the loot.
- Same rules as the card series: one accent colour, no text, emblems,
  people, explosions, fire or interface.

## Delivery

Save each master here under its slug as an opaque PNG at native size,
without cropping or upscaling. `npm run assets:derive`
(`scripts/build-signal-art.mjs`) writes the full frame as WebP at 768 and
1280 pixels wide to `client/public/art/derived/signals/aftermath-v1/`. Until
a master lands, the report panel shows the signal's card painting.
