# Chapter art: prompts

Paintings for the charter's chapter openings (Peter, Oct 29 2026: "event art
for each chapter of the tutorial"). Landfall already opens with the
prologue's landing, the Raid with the raid painting and the Vendetta with the
uncle's arrival; these three open Pirates, Expansion and Reach.

**Status:** delivered and integrated Oct 30 2026. Bands: Pirates row 64,
Expansion row 64, Reach row 0 (records in
`client/art-src/story-events-v1/README.md`).

## How to make them

- **ChatGPT:** one image per chat message. Attach the images listed for that
  piece, in the order given (Image 1 first), then paste its prompt.
- **Size:** ask for the widest landscape size (1536 × 1024). The prompts keep
  every subject inside the band that survives a 16:9 crop.
- **Saving:** save each PNG as delivered under its slug in
  `client/art-src/story-events-v1/`. Don't crop or resize it; I'll frame its
  16:9 band, build the derivatives, record the prompt in `prompts.json` and
  the pop-up picks it up.

## The set

| # | Slug | Chapter | Pop-up | Attach |
|---|---|---|---|---|
| 1 | `chapter-pirates` | Pirates | Strike back | `first-landing.png`, `free-trader-inbound.png`, `interceptor-reference.png` |
| 2 | `chapter-expansion` | Expansion | Lanes open | `first-landing.png`, `free-trader-inbound.png`, `tiny-freighter.png` |
| 3 | `chapter-reach` | Reach | Into the belt | `first-landing.png`, `mining-barge-reference.png`, `stony-belt-v1.png` |

The attachments are all in the repository:

| File | Path |
|---|---|
| `first-landing.png` | `client/art-src/founding-intro-v1/first-landing.png` |
| `free-trader-inbound.png` | `client/art-src/story-events-v1/free-trader-inbound.png` |
| `interceptor-reference.png` | `client/art-src/interceptor-reference-v1/interceptor-reference.png` |
| `tiny-freighter.png` | `client/art-src/freighters-2026-09-16/tiny-freighter.png` |
| `mining-barge-reference.png` | `client/art-src/mining-barge-reference-v1/mining-barge-reference.png` |
| `stony-belt-v1.png` | `client/art-src/belt-view/stony-belt-v1.png` |

---

## 1 · `chapter-pirates` — Strike back

Opens the Pirates chapter (step 9), when the Shipyard is staffed: "The Shipyard stands. {Nephew} is still out there." Save as `client/art-src/story-events-v1/chapter-pirates.png`.

Attach:

1. `client/art-src/founding-intro-v1/first-landing.png`
2. `client/art-src/story-events-v1/free-trader-inbound.png`
3. `client/art-src/interceptor-reference-v1/interceptor-reference.png`

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Input image 2: the COLONY reference only: the white landed colony ship that serves as the command center, its habitat and farm modules and the dusk highland plain. Use it for the colony's look; never copy its composition or its ship.
Input image 3: the colony's INTERCEPTOR design reference only: a slim white swept-wing fighter with twin engines and blue running lights, in top, front and side views. Show it in believable scene perspective, never as a flat reference-sheet view, and never copy the white sheet background or labels.
Composition/framing: one opaque wide landscape illustration at the widest landscape size available. It will be cropped to 16:9, so keep every essential subject inside the central band, clear of the top and bottom 10% and the outer 8% at each side. Strong readable focal point. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: The colony arms itself: its new Shipyard starts the first warship to hunt the privateer who raided it.
Scene/backdrop: The colony at dusk, as in Image 2: the white landed colony ship that serves as its command center, habitat and farm modules, the highland plain. Beside it now stands a new open-air Shipyard: a cradle of charcoal-metal gantries and cranes under hard white floodlights.
Subject: In the Shipyard's cradle, the colony's first Interceptor (Image 3) nears completion, seen in believable three-quarter perspective: a slim white swept-wing fighter with twin engines and blue running lights, a few hull panels still open, welding sparks falling from the gantry arms. A handful of colonists in work suits watch from the yard's edge. High in the darkening sky, far beyond the colony, a tiny orange engine trail: the privateer, still out there.
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: Resolve; floodlight white and welding sparks against blue dusk; the colony arming itself.
Constraints: absolutely no text, letters, numbers, logos, labels, title, watermark, borders, captions or fake interface. No readable markings, brand marks, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```

## 2 · `chapter-expansion` — Lanes open

Opens the Expansion chapter (step 12), when the den has fallen: "The den is gone. The lanes to the Market Hub are clear." Save as `client/art-src/story-events-v1/chapter-expansion.png`.

Attach:

1. `client/art-src/founding-intro-v1/first-landing.png`
2. `client/art-src/story-events-v1/free-trader-inbound.png`
3. `client/art-src/freighters-2026-09-16/tiny-freighter.png`

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Input image 2: the COLONY reference only: the white landed colony ship that serves as the command center, its habitat and farm modules and the dusk highland plain. Use it for the colony's look; never copy its composition or its ship.
Input image 3: the colony's TINY FREIGHTER design reference only: white cargo blocks with yellow edge trim behind a small command nose, and two rear engines. Show it in believable scene perspective, never as a flat top-down sprite.
Composition/framing: one opaque wide landscape illustration at the widest landscape size available. It will be cropped to 16:9, so keep every essential subject inside the central band, clear of the top and bottom 10% and the outer 8% at each side. Strong readable focal point. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: With the pirate den destroyed, the colony's first freighter carries its ore toward the distant Market Hub.
Scene/backdrop: The same colony at sunrise, as in Image 2, grown since the landing: a mine's headframe and ore conveyors, a field of solar arrays, the Shipyard's gantries, warm lights in the habitats.
Subject: The colony's Tiny Freighter (Image 3), in believable perspective, lifts off from a landing pad loaded with sealed ore containers, its two engines glowing as it climbs into the morning sky. Dock crew and colonists watch from beside stacked ore. High in the pale sky, a faint cyan glint marks the far-off Market Hub.
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: Hopeful dawn; warm sunrise on the colony and the climbing freighter; the lanes open at last.
Constraints: absolutely no text, letters, numbers, logos, labels, title, watermark, borders, captions or fake interface. No readable markings, brand marks, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```

## 3 · `chapter-reach` — Into the belt

Opens the Reach chapter (step 16), when the survey of the den's system arrives: "The survey is in: the den's old belt is rich in ore." Save as `client/art-src/story-events-v1/chapter-reach.png`.

Attach:

1. `client/art-src/founding-intro-v1/first-landing.png`
2. `client/art-src/mining-barge-reference-v1/mining-barge-reference.png`
3. `client/art-src/belt-view/stony-belt-v1.png`

```text
Use case: stylized-concept
Asset type: one cinematic illustration for an in-game event pop-up in Stellar Syndicates, a persistent science-fiction colony and trading game.
Input image 1: STYLE REFERENCE ONLY, the approved opening-slideshow first-landing illustration. Match its realistic cinematic science-fiction painting, grounded materials, restrained detail and frontier world. Create the new scene below, not a copy of that composition.
Input image 2: the colony's MINING BARGE design reference only: a heavy white-and-yellow hull with a central ore hold and two forward cutter arms ending in toothed cutting heads, in top, bottom, front, back and side views. Show it in believable scene perspective, never as a flat reference-sheet view, and never copy the white sheet background or labels.
Input image 3: the ASTEROID BELT reference only: its pitted grey-brown boulders and scattered rubble. Use it for the belt's rocks and their scale; never copy its composition or camera.
Composition/framing: one opaque wide landscape illustration at the widest landscape size available. It will be cropped to 16:9, so keep every essential subject inside the central band, clear of the top and bottom 10% and the outer 8% at each side. Strong readable focal point. Full frame, no collage. The game adds its title and caption outside this image.
Primary request: The colony reaches past its own sky: its first Mining Barge works the rich asteroid belt where the pirate den once stood.
Scene/backdrop: A quiet asteroid belt in a neighbouring system, its rocks as in Image 3: drifting boulders of pitted stone and rubble, a small distant star throwing long cold light across them. Far off among the rocks drifts the dark, broken wreck of the pirate den.
Subject: The colony's Mining Barge (Image 2), in believable three-quarter perspective, holds beside a large ore-rich asteroid; its two forward cutter arms bite into the rock with bright cutting beams, and glittering ore fragments drift toward its open hold.
Style/medium: high-end realistic digital science-fiction concept painting, grounded industrial technology and believable materials, refined atmosphere and restrained detail. Charcoal metal, cool blue-white technology, subtle amber practical lights, natural planet greens and blues. Not a poster or user interface.
Lighting/mood: Quiet ambition; cold distant starlight and the warm glow of cutting; the colony's reach widening.
Constraints: absolutely no text, letters, numbers, logos, labels, title, watermark, borders, captions or fake interface. No readable markings, brand marks, holographic text, exaggerated cartoon expressions, alien characters, or franchise designs. Deliver exactly one finished image.
```
