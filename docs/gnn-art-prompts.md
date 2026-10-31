# GNN art: prompts

Art for the season's news stories (`docs/endgame-keystone.md`): fourteen
illustrations for the story pop-ups and two emblems for the news feed. Every
piece replaces a stand-in the game already shows: today the pop-ups are text
under the Authority's seal, and the feed's sources are the words "GNN" and
"Free Signal".

**Status (Oct 29 2026):** all sixteen delivered and wired; see
`client/art-src/gnn-v1/README.md` and `client/art-src/story-events-v1/README.md`.

## How to make them

- **ChatGPT:** one image per chat message. Attach the images listed for that
  piece, in the order given (Image 1 first), then paste its prompt.
- **Illustrations:** ask for the widest landscape size (1536 × 1024). The
  prompts keep every subject inside the band that survives a 16:9 crop.
- **Emblems:** ask for a 1024 × 1024 PNG with a transparent background.
- **Saving:** save each PNG as delivered under its slug: illustrations in
  `client/art-src/story-events-v1/`, emblems in `client/art-src/gnn-v1/`.
  Don't crop or resize them; I'll crop them to 16:9 at 1672 × 941, build the
  derivatives, record the prompts in `prompts.json` and wire each one into its
  pop-up.
- **Already done:** the existing `relics-awaken.png` painting covers "The
  relics stir"; no new art is needed for it.

## The set

Make them in this order: the emblems first (they show everywhere), then
the stories in the order players meet them.

| # | Slug | Shown | Attach |
|---|---|---|---|
| 1 | `gnn-emblem` | the Galactic News Network: the ticker, the News tab and the pop-ups of Authority stories | `terran-charter-authority-seal.png` |
| 2 | `free-signal-emblem` | the rebels' broadcasts in the same feed: exposed false flags, rebel wins, Resolve milestones | nothing |
| 3 | `decree-levies` | The Authority's first decree: a levy on every Hub trade (maturity 25%). | `first-landing.png`, `login-market-horizon-v1.png`, `terran-charter-authority-seal.png` |
| 4 | `decree-inspections` | The second decree: cargo inspections, colony licence fees and a warship registry (maturity 45%). | `first-landing.png`, `authority-arbiter-reference.png`, `login-market-horizon-v1.png` |
| 5 | `decree-martial` | The last decree: garrisons in the Hub district, syndicate dues, enforced requisitions (maturity 65%, or early by emergency decree). | `first-landing.png`, `authority-arbiter-reference.png`, `login-market-horizon-v1.png` |
| 6 | `sabotage-at-the-hub` | Crackdown: 12h of inspections and doubled Hub fees. Some are staged; a Scout at the Hub may find out. | `first-landing.png`, `login-market-horizon-v1.png`, `authority-arbiter-reference.png` |
| 7 | `convoy-massacre` | Crackdown when you are named: a bounty and an enforcement squadron. A Scout at the wreck may prove it false. | `first-landing.png`, `authority-freighter-reference.png`, `authority-arbiter-reference.png` |
| 8 | `quarantine` | Crackdown on the busiest freight destination: Authority freight suspended for 12h. | `first-landing.png`, `free-trader-inbound.png`, `authority-freighter-reference.png` |
| 9 | `relic-seizure` | Crackdown on a relic's holder as it wakes: hand it over or lose standing. | `first-landing.png`, `relics-awaken.png`, `authority-arbiter-reference.png` |
| 10 | `keystone-crackdown` | Crackdown when your Keystone reaches level 3, 6 or 9: enforcement at its builders. | `first-landing.png`, `authority-arbiter-reference.png`, `relics-awaken.png` |
| 11 | `false-flag-exposed` | A Scout's evidence proves the Authority staged it: the crackdown lifts and every colony's Resolve rises. | `first-landing.png`, `pirate-reaver-chroma.png`, `authority-arbiter-reference.png` |
| 12 | `sealed-world-found` | Arriving with a bearing through it finds a Precursor world. Its guardians wake. | `first-landing.png`, `relics-awaken.png`, `pirate-warbarge-chroma.png` |
| 13 | `authority-spy` | After a find: the Authority will know in 2h, then war. | `first-landing.png`, `terran-charter-authority-seal.png` |
| 14 | `authority-war` | The leak lands: no Hub, no Authority freight, bounties on your hulls; the war fleet sails. | `first-landing.png`, `authority-arbiter-reference.png`, `login-market-horizon-v1.png` |
| 15 | `seal-fires` | The Sovereign ending: a Keystone held 48h at level 10 closes every gate. | `first-landing.png`, `login-market-horizon-v1.png`, `relics-awaken.png` |
| 16 | `charter-holds` | The other ending: no Seal fired before the race or the backstop ran out. | `first-landing.png`, `authority-arbiter-reference.png`, `terran-charter-authority-seal.png` |

The attachments are all in the repository:

| File | Path |
|---|---|
| `first-landing.png` | `client/art-src/founding-intro-v1/first-landing.png` |
| `login-market-horizon-v1.png` | `client/art-src/login/login-market-horizon-v1.png` |
| `authority-arbiter-reference.png` | `client/art-src/authority-arbiter-reference-v1/authority-arbiter-reference.png` |
| `authority-freighter-reference.png` | `client/art-src/authority-freighter-reference-v1/authority-freighter-reference.png` |
| `terran-charter-authority-seal.png` | `client/art-src/authority-v2/terran-charter-authority-seal.png` |
| `relics-awaken.png` | `client/art-src/story-events-v1/relics-awaken.png` |
| `free-trader-inbound.png` | `client/art-src/story-events-v1/free-trader-inbound.png` |
| `pirate-reaver-chroma.png` | `client/art-src/npc-ships/pirate-reaver-chroma.png` |
| `pirate-warbarge-chroma.png` | `client/art-src/npc-ships/pirate-warbarge-chroma.png` |

---

## Emblems

### 1 · `gnn-emblem` — GNN emblem

Shown for the Galactic News Network: the ticker, the News tab and the pop-ups of Authority stories. Save as `client/art-src/gnn-v1/gnn-emblem.png`.

Attach:

1. `client/art-src/authority-v2/terran-charter-authority-seal.png`

```text
Use case: logo-brand
Asset type: one small emblem for the Galactic News Network, the Terran Charter Authority's official news broadcast in Stellar Syndicates, a dark premium science-fiction strategy game. It is shown at 16-24 px beside headlines and at 64 px on story pop-ups.
Input image 1: the Authority's EMBLEM, attached as the family reference only: a thin gold ring with a slightly thicker crescent on its right, crossed by three slender vertical gold spires of staggered heights. The news emblem belongs to the same family but is not a copy.
Subject: a circular broadcast mark. A thin gold ring with a bright ivory dot at its centre, like a lens; from the ring's right side, three short concentric gold broadcast arcs radiate outward, growing in size. Behind it, a small round navy-graphite plate with a thin ivory rim. Perfectly symmetrical top to bottom, flat and bold: a few large shapes that still read at 16 px.
Style: crisp flat vector-like emblem with a subtle metallic gold finish; antique gold, ivory and navy-graphite only. Clean edges, no painterly texture, no gradients beyond a slight metal sheen.
Framing: centred on a square 1024 x 1024 canvas, the emblem spanning about 80% of the width.
Background: genuinely transparent PNG with real alpha; no backdrop, no shadow plate, no glow halo.
Constraints: absolutely no text, letters, numbers, initials or words; no watermark, border or frame. Exactly one emblem.
```

### 2 · `free-signal-emblem` — Free Signal emblem

Shown for the rebels' broadcasts in the same feed: exposed false flags, rebel wins, Resolve milestones. Save as `client/art-src/gnn-v1/free-signal-emblem.png`.

Nothing to attach.

```text
Use case: logo-brand
Asset type: one small emblem for the Free Signal, the rebel colonies' pirate-radio broadcast in Stellar Syndicates, a dark premium science-fiction strategy game. It is shown at 16-24 px beside headlines and at 64 px on story pop-ups.
Subject: a hand-made rebel mark. A simple upright antenna mast built from scrap metal, with three cyan broadcast arcs breaking outward from its tip; at the mast's foot, a snapped chain link, its two broken halves falling apart. Set on a rough round disc of dark scrap metal with slightly uneven, hand-cut edges.
Style: bold painted-stencil look, as if sprayed through a cut stencil onto metal: cyan and off-white on dark gunmetal, a little wear and paint chipping. Bold and simple: a few large shapes that still read at 16 px.
Framing: centred on a square 1024 x 1024 canvas, the emblem spanning about 80% of the width.
Background: genuinely transparent PNG with real alpha; no backdrop, no shadow plate, no glow halo.
Constraints: absolutely no text, letters, numbers, initials or words; no skulls, weapons or national flags; no watermark, border or frame. Exactly one emblem.
```

---

## Story illustrations

### 3 · `decree-levies` — Grip II · Levies

The Authority's first decree: a levy on every Hub trade (maturity 25%). Save as `client/art-src/story-events-v1/decree-levies.png`.

Attach:

1. `client/art-src/founding-intro-v1/first-landing.png`
2. `client/art-src/login/login-market-horizon-v1.png`
3. `client/art-src/authority-v2/terran-charter-authority-seal.png`

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Input image 2: the MARKET HUB reference only: a monumental dark industrial trade station framing a luminous cyan wormhole. Use it for the Hub's look; never copy its composition.
Input image 3: the Authority's EMBLEM reference only: a thin gold ring with a slightly thicker crescent on its right, crossed by three slender vertical gold spires of staggered heights. Where the scene shows the emblem, keep it small and simple; never add lettering.
Composition/framing: one opaque wide landscape illustration at the widest landscape size available. It will be cropped to 16:9, so keep every essential subject inside the central band, clear of the top and bottom 10% and the outer 8% at each side. Strong readable focal point. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: The Terran Charter Authority decrees a levy on every sale and purchase at the Market Hub.
Scene/backdrop: Inside the Market Hub's vast trading concourse, a cavernous station hall of charcoal metal and tall glass. Through the window wall behind, the Hub's luminous cyan wormhole turns beyond docked freighters, as in Image 2.
Subject: On a raised gallery, two Authority officers stand beside a newly installed levy gate: a tall ivory-and-silver pillar with a glowing cyan scanning collar and gold trim. A conveyor of sealed cargo crates passes through the gate below; a cyan beam scans each crate and a small gold light marks it. Merchants and dock crews in practical work suits watch from the hall floor, uneasy; one looks down at a thinning stack of credit chips in their palm. A long navy banner hangs from the gallery with the Authority's small gold emblem from Image 3, no lettering.
The Terran Charter Authority's look throughout: ivory ceramic and silver armor over dark navy-graphite, restrained antique-gold trim, brass fittings, sparse cyan running lights; austere, ceremonial and institutional. Its officers wear navy-graphite uniforms with ivory armor plates and a small plain gold pin.
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: Cool institutional light with gold accents; a quiet, resentful moment as commerce takes on a new weight.
Constraints: absolutely no text, letters, numbers, labels, title, watermark, borders, captions or fake interface. No logos or brand marks except the Authority's small plain gold emblem where the scene asks for it. No readable markings, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```

### 4 · `decree-inspections` — Grip III · Inspections

The second decree: cargo inspections, colony licence fees and a warship registry (maturity 45%). Save as `client/art-src/story-events-v1/decree-inspections.png`.

Attach:

1. `client/art-src/founding-intro-v1/first-landing.png`
2. `client/art-src/authority-arbiter-reference-v1/authority-arbiter-reference.png`
3. `client/art-src/login/login-market-horizon-v1.png`

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Input image 2: the Authority's SHIP DESIGN reference only (the Arbiter): ivory ceramic and silver armor over dark navy-graphite structure, one thin circular halo ring around the midship, three slender dorsal spines, a long spinal lance, four engines under white cowls, antique-gold trim. Show it in believable scene perspective, never as a flat reference-sheet view, and never copy the white sheet background or labels.
Input image 3: the MARKET HUB reference only: a monumental dark industrial trade station framing a luminous cyan wormhole. Use it for the Hub's look; never copy its composition.
Composition/framing: one opaque wide landscape illustration at the widest landscape size available. It will be cropped to 16:9, so keep every essential subject inside the central band, clear of the top and bottom 10% and the outer 8% at each side. Strong readable focal point. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: The Authority begins cargo inspections at the Market Hub, with colony licence fees and a registry tax on warships.
Scene/backdrop: A docking berth at the Market Hub: a freighter's open cargo hold under hard white floodlights. Through the berth's mouth, the Hub's cyan wormhole glows in the distance, as in Image 3.
Subject: An Authority inspection team of four, ivory armor over navy-graphite with cyan visor lights, works through a corporation freighter's hold. One sweeps a handheld cyan scanner over opened crates, one levers up a crate lid, one holds a slim dark data slate. The freighter's crew stand aside in worn work suits, arms crossed. Beyond the berth, one Authority Arbiter (Image 2) lies off the station, small with distance and imposing.
The Terran Charter Authority's look throughout: ivory ceramic and silver armor over dark navy-graphite, restrained antique-gold trim, brass fittings, sparse cyan running lights; austere, ceremonial and institutional. Its officers wear navy-graphite uniforms with ivory armor plates and a small plain gold pin.
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: Harsh floodlight and cyan scanner glow; tension; the freedom of the trade lanes ending.
Constraints: absolutely no text, letters, numbers, logos, labels, title, watermark, borders, captions or fake interface. No readable markings, brand marks, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```

### 5 · `decree-martial` — Grip IV · Martial Charter

The last decree: garrisons in the Hub district, syndicate dues, enforced requisitions (maturity 65%, or early by emergency decree). Save as `client/art-src/story-events-v1/decree-martial.png`.

Attach:

1. `client/art-src/founding-intro-v1/first-landing.png`
2. `client/art-src/authority-arbiter-reference-v1/authority-arbiter-reference.png`
3. `client/art-src/login/login-market-horizon-v1.png`

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Input image 2: the Authority's SHIP DESIGN reference only (the Arbiter): ivory ceramic and silver armor over dark navy-graphite structure, one thin circular halo ring around the midship, three slender dorsal spines, a long spinal lance, four engines under white cowls, antique-gold trim. Show it in believable scene perspective, never as a flat reference-sheet view, and never copy the white sheet background or labels.
Input image 3: the MARKET HUB reference only: a monumental dark industrial trade station framing a luminous cyan wormhole. Use it for the Hub's look; never copy its composition.
Composition/framing: one opaque wide landscape illustration at the widest landscape size available. It will be cropped to 16:9, so keep every essential subject inside the central band, clear of the top and bottom 10% and the outer 8% at each side. Strong readable focal point. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: The Authority imposes its Martial Charter: warships garrison the Hub district and enforce its demands.
Scene/backdrop: Open space before the Market Hub (Image 3), its cyan wormhole in the background; the night side of an inhabited planet curves along the bottom edge with faint city lights.
Subject: A formation of five Authority Arbiters (Image 2) holds station in a precise diagonal line across the Hub's approaches, their halo rings catching the wormhole light. Smaller ivory patrol craft move between them. In the foreground, one corporation freighter of dark metal sits halted at a checkpoint, pinned in a cyan scanning beam from the nearest Arbiter.
The Terran Charter Authority's look throughout: ivory ceramic and silver armor over dark navy-graphite, restrained antique-gold trim, brass fittings, sparse cyan running lights; austere, ceremonial and institutional. Its officers wear navy-graphite uniforms with ivory armor plates and a small plain gold pin.
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: Cold, ordered and overwhelming; the Authority's grip closing on the galaxy's trade.
Constraints: absolutely no text, letters, numbers, logos, labels, title, watermark, borders, captions or fake interface. No readable markings, brand marks, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```

### 6 · `sabotage-at-the-hub` — Sabotage at the Hub

Crackdown: 12h of inspections and doubled Hub fees. Some are staged; a Scout at the Hub may find out. Save as `client/art-src/story-events-v1/sabotage-at-the-hub.png`.

Attach:

1. `client/art-src/founding-intro-v1/first-landing.png`
2. `client/art-src/login/login-market-horizon-v1.png`
3. `client/art-src/authority-arbiter-reference-v1/authority-arbiter-reference.png`

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Input image 2: the MARKET HUB reference only: a monumental dark industrial trade station framing a luminous cyan wormhole. Use it for the Hub's look; never copy its composition.
Input image 3: the Authority's SHIP DESIGN reference only (the Arbiter): ivory ceramic and silver armor over dark navy-graphite structure, one thin circular halo ring around the midship, three slender dorsal spines, a long spinal lance, four engines under white cowls, antique-gold trim. Show it in believable scene perspective, never as a flat reference-sheet view, and never copy the white sheet background or labels.
Composition/framing: one opaque wide landscape illustration at the widest landscape size available. It will be cropped to 16:9, so keep every essential subject inside the central band, clear of the top and bottom 10% and the outer 8% at each side. Strong readable focal point. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: Sabotage at the Market Hub: a blast tears open a cargo berth, insurgents are blamed, and the Authority clamps down.
Scene/backdrop: The Market Hub's outer docking ring (Image 2), one long arm of cargo berths against black space, the cyan wormhole beyond.
Subject: One berth is torn open by a recent explosion: twisted girders, drifting debris, white vapour venting, amber emergency strobes. Authority patrol craft (in the design language of Image 3) swarm around it with searchlight beams; a line of freighters waits, held back at a distance. Small but clear in the foreground debris, one scorched component catches the light: the evidence nobody has looked at yet.
The Terran Charter Authority's look throughout: ivory ceramic and silver armor over dark navy-graphite, restrained antique-gold trim, brass fittings, sparse cyan running lights; austere, ceremonial and institutional. Its officers wear navy-graphite uniforms with ivory armor plates and a small plain gold pin.
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: Emergency amber against cool blue; suspicion and confusion.
Constraints: absolutely no text, letters, numbers, logos, labels, title, watermark, borders, captions or fake interface. No readable markings, brand marks, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```

### 7 · `convoy-massacre` — Convoy massacre

Crackdown when you are named: a bounty and an enforcement squadron. A Scout at the wreck may prove it false. Save as `client/art-src/story-events-v1/convoy-massacre.png`.

Attach:

1. `client/art-src/founding-intro-v1/first-landing.png`
2. `client/art-src/authority-freighter-reference-v1/authority-freighter-reference.png`
3. `client/art-src/authority-arbiter-reference-v1/authority-arbiter-reference.png`

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Input image 2: the Authority FREIGHTER design reference only: a broad institutional carrier with four banks of silver cargo tiles around a central spine and four engines under white cowls. Show it in believable scene perspective, never as a flat reference-sheet view, and never copy the white sheet background or labels.
Input image 3: the Authority's SHIP DESIGN reference only (the Arbiter): ivory ceramic and silver armor over dark navy-graphite structure, one thin circular halo ring around the midship, three slender dorsal spines, a long spinal lance, four engines under white cowls, antique-gold trim. Show it in believable scene perspective, never as a flat reference-sheet view, and never copy the white sheet background or labels.
Composition/framing: one opaque wide landscape illustration at the widest landscape size available. It will be cropped to 16:9, so keep every essential subject inside the central band, clear of the top and bottom 10% and the outer 8% at each side. Strong readable focal point. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: An Authority freight convoy is destroyed in deep space, and the Authority names a rebel corporation as the culprit.
Scene/backdrop: Deep space along a freight lane, a distant pale star, near-black darkness.
Subject: The shattered remains of three Authority freighters (Image 2) drift in a wide debris field: broken hull spines, silver cargo tiles scattered and still glowing faintly at the edges. An Authority Arbiter (Image 3) arrives from the left, floodlights sweeping across the wreckage; a few small recovery drones with cyan lights move among the debris.
The Terran Charter Authority's look throughout: ivory ceramic and silver armor over dark navy-graphite, restrained antique-gold trim, brass fittings, sparse cyan running lights; austere, ceremonial and institutional. Its officers wear navy-graphite uniforms with ivory armor plates and a small plain gold pin.
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: Grim and solemn; cold light on wreckage; a crime someone will be made to answer for.
Constraints: absolutely no text, letters, numbers, logos, labels, title, watermark, borders, captions or fake interface. No readable markings, brand marks, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```

### 8 · `quarantine` — Quarantine

Crackdown on the busiest freight destination: Authority freight suspended for 12h. Save as `client/art-src/story-events-v1/quarantine.png`.

Attach:

1. `client/art-src/founding-intro-v1/first-landing.png`
2. `client/art-src/story-events-v1/free-trader-inbound.png`
3. `client/art-src/authority-freighter-reference-v1/authority-freighter-reference.png`

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Input image 2: the COLONY reference only: the white landed colony ship that serves as the command center, its habitat and farm modules and the dusk highland plain. Use it for the colony's look; never copy its composition.
Input image 3: the Authority FREIGHTER design reference only: a broad institutional carrier with four banks of silver cargo tiles around a central spine and four engines under white cowls. Show it in believable scene perspective, never as a flat reference-sheet view, and never copy the white sheet background or labels.
Composition/framing: one opaque wide landscape illustration at the widest landscape size available. It will be cropped to 16:9, so keep every essential subject inside the central band, clear of the top and bottom 10% and the outer 8% at each side. Strong readable focal point. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: The Authority quarantines a busy colony after disease spreads along its trade route; its freight stops for 12 hours.
Scene/backdrop: A colony at dusk, as in Image 2: the white landed colony ship as its command center, habitat modules around it, the highland plain, seen from a low ridge.
Subject: Tall amber-and-white quarantine beacons ring the settlement, pulsing warning light. At a barrier checkpoint in the foreground, two Authority medics in sealed ivory hazard suits with cyan visors stand guard; behind the barrier, colonists in breathing masks wait. Overhead, one Authority freighter (Image 3) hangs motionless, held off from landing.
The Terran Charter Authority's look throughout: ivory ceramic and silver armor over dark navy-graphite, restrained antique-gold trim, brass fittings, sparse cyan running lights; austere, ceremonial and institutional. Its officers wear navy-graphite uniforms with ivory armor plates and a small plain gold pin.
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: Eerie amber pulses in blue dusk; fear and frustration, no violence.
Constraints: absolutely no text, letters, numbers, logos, labels, title, watermark, borders, captions or fake interface. No readable markings, brand marks, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```

### 9 · `relic-seizure` — Relic seizure

Crackdown on a relic's holder as it wakes: hand it over or lose standing. Save as `client/art-src/story-events-v1/relic-seizure.png`.

Attach:

1. `client/art-src/founding-intro-v1/first-landing.png`
2. `client/art-src/story-events-v1/relics-awaken.png`
3. `client/art-src/authority-arbiter-reference-v1/authority-arbiter-reference.png`

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Input image 2: the RELIC reference only: its tall elongated violet crystal and cold violet light. Use it for the relic and for every violet Precursor light; never copy its composition or its pirate ships.
Input image 3: the Authority's SHIP DESIGN reference only (the Arbiter): ivory ceramic and silver armor over dark navy-graphite structure, one thin circular halo ring around the midship, three slender dorsal spines, a long spinal lance, four engines under white cowls, antique-gold trim. Show it in believable scene perspective, never as a flat reference-sheet view, and never copy the white sheet background or labels.
Composition/framing: one opaque wide landscape illustration at the widest landscape size available. It will be cropped to 16:9, so keep every essential subject inside the central band, clear of the top and bottom 10% and the outer 8% at each side. Strong readable focal point. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: As the relics wake, the Authority comes to seize one from the colony that holds it.
Scene/backdrop: A colony's vault dug beneath its command center: a rough rock chamber braced with charcoal metal ribs, a few hard white work lamps.
Subject: On a reinforced cradle stands a tall elongated violet crystal relic, glowing coldly as in Image 2. Four Authority troopers, ivory armor over navy-graphite in the design language of Image 3, surround it with a heavy silver-and-gold containment frame, ready to lift it. An Authority officer holds out a slim data slate to the colony's director, a weathered woman in a practical work coat who stands between them and the relic and does not move.
The Terran Charter Authority's look throughout: ivory ceramic and silver armor over dark navy-graphite, restrained antique-gold trim, brass fittings, sparse cyan running lights; austere, ceremonial and institutional. Its officers wear navy-graphite uniforms with ivory armor plates and a small plain gold pin.
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: Violet relic glow against cold white lamps; defiance.
Constraints: absolutely no text, letters, numbers, logos, labels, title, watermark, borders, captions or fake interface. No readable markings, brand marks, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```

### 10 · `keystone-crackdown` — Keystone crackdown

Crackdown when your Keystone reaches level 3, 6 or 9: enforcement at its builders. Save as `client/art-src/story-events-v1/keystone-crackdown.png`.

Attach:

1. `client/art-src/founding-intro-v1/first-landing.png`
2. `client/art-src/authority-arbiter-reference-v1/authority-arbiter-reference.png`
3. `client/art-src/story-events-v1/relics-awaken.png`

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Input image 2: the Authority's SHIP DESIGN reference only (the Arbiter): ivory ceramic and silver armor over dark navy-graphite structure, one thin circular halo ring around the midship, three slender dorsal spines, a long spinal lance, four engines under white cowls, antique-gold trim. Show it in believable scene perspective, never as a flat reference-sheet view, and never copy the white sheet background or labels.
Input image 3: the RELIC reference only: its tall elongated violet crystal and cold violet light. Use it for the relic and for every violet Precursor light; never copy its composition or its pirate ships.
Composition/framing: one opaque wide landscape illustration at the widest landscape size available. It will be cropped to 16:9, so keep every essential subject inside the central band, clear of the top and bottom 10% and the outer 8% at each side. Strong readable focal point. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: The Authority's fleet strikes at a rising Keystone on a sealed Precursor world.
Scene/backdrop: Low orbit over a dark Precursor world, its black stone-like continents veined with faint violet light (the violet of Image 3).
Subject: Far below on the surface, a colossal stepped black monolith, the Keystone, rises inside construction scaffolding, a violet crystal heart glowing at its apex. In orbit, two Authority Arbiters (Image 2) fire long cyan lance beams toward the surface. Rising from the planet to meet them, a handful of small dark corporation warships with blazing amber engines.
The Terran Charter Authority's look throughout: ivory ceramic and silver armor over dark navy-graphite, restrained antique-gold trim, brass fittings, sparse cyan running lights; austere, ceremonial and institutional. Its officers wear navy-graphite uniforms with ivory armor plates and a small plain gold pin.
Precursor architecture: impossibly old, seamless black stone-like metal with a faint basalt grain, monolithic stepped geometry, no ornament and no glyphs, inlaid with thin channels of the same cold violet light as the relic crystal.
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: Epic and dangerous; violet against cyan; the race for the gates.
Constraints: absolutely no text, letters, numbers, logos, labels, title, watermark, borders, captions or fake interface. No readable markings, brand marks, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```

### 11 · `false-flag-exposed` — False flag exposed

A Scout's evidence proves the Authority staged it: the crackdown lifts and every colony's Resolve rises. Save as `client/art-src/story-events-v1/false-flag-exposed.png`.

Attach:

1. `client/art-src/founding-intro-v1/first-landing.png`
2. `client/art-src/npc-ships/pirate-reaver-chroma.png`
3. `client/art-src/authority-arbiter-reference-v1/authority-arbiter-reference.png`

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Input image 2: the PIRATE SHIP design reference only: blackened, rust-patched hull with orange stripes and amber engines. Show it in believable scene perspective, never as a flat top-down sprite, and never copy the green background.
Input image 3: the Authority's SHIP DESIGN reference only (the Arbiter): ivory ceramic and silver armor over dark navy-graphite structure, one thin circular halo ring around the midship, three slender dorsal spines, a long spinal lance, four engines under white cowls, antique-gold trim. Show it in believable scene perspective, never as a flat reference-sheet view, and never copy the white sheet background or labels.
Composition/framing: one opaque wide landscape illustration at the widest landscape size available. It will be cropped to 16:9, so keep every essential subject inside the central band, clear of the top and bottom 10% and the outer 8% at each side. Strong readable focal point. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: A scout pilot finds proof that the Authority staged an attack it blamed on rebels.
Scene/backdrop: The cramped cockpit of a small scout ship drifting through a debris field; through the wide canopy, a wrecked warship tumbles slowly.
Subject: The wreck in the canopy wears blackened, rust-patched, orange-striped pirate paint (the pirate style of Image 2), but where a blast scraped the paint away, clean ivory ceramic armor and antique-gold trim show beneath: Authority construction, in the design language of Image 3. The scout pilot, a young man in a worn flight suit, leans toward the canopy in the glow of cyan instruments, recording it with a handheld scanner.
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: A quiet revelation; cyan cockpit light and the gold glint of the lie.
Constraints: absolutely no text, letters, numbers, logos, labels, title, watermark, borders, captions or fake interface. No readable markings, brand marks, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```

### 12 · `sealed-world-found` — A sealed world

Arriving with a bearing through it finds a Precursor world. Its guardians wake. Save as `client/art-src/story-events-v1/sealed-world-found.png`.

Attach:

1. `client/art-src/founding-intro-v1/first-landing.png`
2. `client/art-src/story-events-v1/relics-awaken.png`
3. `client/art-src/npc-ships/pirate-warbarge-chroma.png`

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Input image 2: the RELIC reference only: its tall elongated violet crystal and cold violet light. Use it for the relic and for every violet Precursor light; never copy its composition or its pirate ships.
Input image 3: the PIRATE WARSHIP design reference only: blackened, rust-patched hull with orange stripes and amber engines. Show it in believable scene perspective, never as a flat top-down sprite, and never copy the green background.
Composition/framing: one opaque wide landscape illustration at the widest landscape size available. It will be cropped to 16:9, so keep every essential subject inside the central band, clear of the top and bottom 10% and the outer 8% at each side. Strong readable focal point. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: Following its relics' bearings, a corporation finds a sealed Precursor world whose Keystone could close every wormhole gate.
Scene/backdrop: A barren grey world under a pale sun, a vast crater seen from its rim.
Subject: In the crater floor, an immense ancient Precursor structure lies exposed for the first time: stepped black terraces descending into the ground around a dark circular seal, thin channels of violet light (as in Image 2) waking along its lines. At the crater's rim, a small corporation scout ship hovers, its floodlight a thin beam across the ruin. From the crater's far shadows, three pirate warships (the style of Image 3) rise with amber engines: the world's guardians, roused.
Precursor architecture: impossibly old, seamless black stone-like metal with a faint basalt grain, monolithic stepped geometry, no ornament and no glyphs, inlaid with thin channels of the same cold violet light as the relic crystal.
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: Awe and dread; violet light rising; a secret too big to keep.
Constraints: absolutely no text, letters, numbers, logos, labels, title, watermark, borders, captions or fake interface. No readable markings, brand marks, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```

### 13 · `authority-spy` — An Authority spy

After a find: the Authority will know in 2h, then war. Save as `client/art-src/story-events-v1/authority-spy.png`.

Attach:

1. `client/art-src/founding-intro-v1/first-landing.png`
2. `client/art-src/authority-v2/terran-charter-authority-seal.png`

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Input image 2: the Authority's EMBLEM reference only: a thin gold ring with a slightly thicker crescent on its right, crossed by three slender vertical gold spires of staggered heights. Where the scene shows the emblem, keep it small and simple; never add lettering.
Composition/framing: one opaque wide landscape illustration at the widest landscape size available. It will be cropped to 16:9, so keep every essential subject inside the central band, clear of the top and bottom 10% and the outer 8% at each side. Strong readable focal point. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: A spy inside the syndicate sends word of the discovery to the Authority.
Scene/backdrop: A dim corporate command room at night: charcoal metal and glass, a few amber console lights. Through the window, the colony's lights and, high in the sky, a faint cyan glint of the distant Hub wormhole.
Subject: A figure in a syndicate officer's long coat, face half in shadow, slides a small glowing data crystal into a private console; a narrow cyan beam rises from a hidden transmitter. On the wall behind, a dim holographic star map marks one world in violet (no text). Inside the figure's coat lapel, a small gold pin in the shape of the Authority's emblem (Image 2) glints.
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: Secretive and cold; a betrayal in progress.
Constraints: absolutely no text, letters, numbers, labels, title, watermark, borders, captions or fake interface. No logos or brand marks except the Authority's small plain gold emblem where the scene asks for it. No readable markings, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```

### 14 · `authority-war` — War with the Authority

The leak lands: no Hub, no Authority freight, bounties on your hulls; the war fleet sails. Save as `client/art-src/story-events-v1/authority-war.png`.

Attach:

1. `client/art-src/founding-intro-v1/first-landing.png`
2. `client/art-src/authority-arbiter-reference-v1/authority-arbiter-reference.png`
3. `client/art-src/login/login-market-horizon-v1.png`

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Input image 2: the Authority's SHIP DESIGN reference only (the Arbiter): ivory ceramic and silver armor over dark navy-graphite structure, one thin circular halo ring around the midship, three slender dorsal spines, a long spinal lance, four engines under white cowls, antique-gold trim. Show it in believable scene perspective, never as a flat reference-sheet view, and never copy the white sheet background or labels.
Input image 3: the MARKET HUB reference only: a monumental dark industrial trade station framing a luminous cyan wormhole. Use it for the Hub's look; never copy its composition.
Composition/framing: one opaque wide landscape illustration at the widest landscape size available. It will be cropped to 16:9, so keep every essential subject inside the central band, clear of the top and bottom 10% and the outer 8% at each side. Strong readable focal point. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: The Authority declares permanent war, and its war fleet sails from the Market Hub against the rebels.
Scene/backdrop: The Market Hub (Image 3), its cyan wormhole flaring brighter than ever.
Subject: A vast Authority war fleet pours out of the wormhole toward the viewer: rank upon rank of Arbiters (Image 2) with halo rings and long spinal lances, escorted by smaller ivory craft. In the foreground, one lone dark corporation freighter turns away hard, its engines flaring amber.
The Terran Charter Authority's look throughout: ivory ceramic and silver armor over dark navy-graphite, restrained antique-gold trim, brass fittings, sparse cyan running lights; austere, ceremonial and institutional. Its officers wear navy-graphite uniforms with ivory armor plates and a small plain gold pin.
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: Overwhelming and ominous; cyan wormhole glare on gold-trimmed hulls.
Constraints: absolutely no text, letters, numbers, logos, labels, title, watermark, borders, captions or fake interface. No readable markings, brand marks, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```

### 15 · `seal-fires` — The Seal fires

The Sovereign ending: a Keystone held 48h at level 10 closes every gate. Save as `client/art-src/story-events-v1/seal-fires.png`.

Attach:

1. `client/art-src/founding-intro-v1/first-landing.png`
2. `client/art-src/login/login-market-horizon-v1.png`
3. `client/art-src/story-events-v1/relics-awaken.png`

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Input image 2: the MARKET HUB reference only: a monumental dark industrial trade station framing a luminous cyan wormhole. Use it for the Hub's look; never copy its composition.
Input image 3: the RELIC reference only: its tall elongated violet crystal and cold violet light. Use it for the relic and for every violet Precursor light; never copy its composition or its pirate ships.
Composition/framing: one opaque wide landscape illustration at the widest landscape size available. It will be cropped to 16:9, so keep every essential subject inside the central band, clear of the top and bottom 10% and the outer 8% at each side. Strong readable focal point. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: The Keystone's Seal fires: every wormhole gate closes and the Authority is cut off from the galaxy.
Scene/backdrop: The Market Hub (Image 2) seen from far away across open space.
Subject: The Hub's cyan wormhole collapses inward into a brilliant violet point as an expanding ring of violet light (the violet of Image 3) sweeps across space from a distant world; the Hub's station lights fail section by section and the Authority ships around it stand frozen in silhouette. In the foreground, a few small dark rebel warships watch, their hulls lit violet.
Precursor architecture: impossibly old, seamless black stone-like metal with a faint basalt grain, monolithic stepped geometry, no ornament and no glyphs, inlaid with thin channels of the same cold violet light as the relic crystal.
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: Awe and finality; violet triumph over cyan.
Constraints: absolutely no text, letters, numbers, logos, labels, title, watermark, borders, captions or fake interface. No readable markings, brand marks, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```

### 16 · `charter-holds` — The Charter holds

The other ending: no Seal fired before the race or the backstop ran out. Save as `client/art-src/story-events-v1/charter-holds.png`.

Attach:

1. `client/art-src/founding-intro-v1/first-landing.png`
2. `client/art-src/authority-arbiter-reference-v1/authority-arbiter-reference.png`
3. `client/art-src/authority-v2/terran-charter-authority-seal.png`

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Input image 2: the Authority's SHIP DESIGN reference only (the Arbiter): ivory ceramic and silver armor over dark navy-graphite structure, one thin circular halo ring around the midship, three slender dorsal spines, a long spinal lance, four engines under white cowls, antique-gold trim. Show it in believable scene perspective, never as a flat reference-sheet view, and never copy the white sheet background or labels.
Input image 3: the Authority's EMBLEM reference only: a thin gold ring with a slightly thicker crescent on its right, crossed by three slender vertical gold spires of staggered heights. Where the scene shows the emblem, keep it small and simple; never add lettering.
Composition/framing: one opaque wide landscape illustration at the widest landscape size available. It will be cropped to 16:9, so keep every essential subject inside the central band, clear of the top and bottom 10% and the outer 8% at each side. Strong readable focal point. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: No Seal fired; the Authority keeps the galaxy, and the Charter holds.
Scene/backdrop: The sealed Precursor world, quiet now, its violet light dimmed to embers; a pale dawn on the horizon.
Subject: On the summit of the dark, unfinished Keystone, a stepped black monolith, Authority troopers plant a tall navy banner bearing the Authority's small gold emblem (Image 3), no lettering. An Authority Arbiter (Image 2) hangs above in the sky; far beyond, the steady cyan glint of the Hub's wormhole, still open.
The Terran Charter Authority's look throughout: ivory ceramic and silver armor over dark navy-graphite, restrained antique-gold trim, brass fittings, sparse cyan running lights; austere, ceremonial and institutional. Its officers wear navy-graphite uniforms with ivory armor plates and a small plain gold pin.
Precursor architecture: impossibly old, seamless black stone-like metal with a faint basalt grain, monolithic stepped geometry, no ornament and no glyphs, inlaid with thin channels of the same cold violet light as the relic crystal.
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: Cold order restored; ivory and gold over black stone; the gates stay open.
Constraints: absolutely no text, letters, numbers, labels, title, watermark, borders, captions or fake interface. No logos or brand marks except the Authority's small plain gold emblem where the scene asks for it. No readable markings, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```
