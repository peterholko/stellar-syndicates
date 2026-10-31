# Story events — version 1

Illustrations for the charter's event pop-ups (`client/src/story-events.ts`),
in the format of the founding prologue
([../founding-intro-v1](../founding-intro-v1/README.md)): one 1672 × 941 PNG
master per scene, generated with the built-in image_gen tool. The prompts are
in [prompts.json](prompts.json).

| Scene | Master | Pop-up |
| --- | --- | --- |
| Pirate raid | `pirate-raid.png` | Shown when the report of the privateer's raid on the colony reaches command (charter step 5): it took every semiconductor and the Armaments. |
| A free trader | `free-trader-inbound.png` | Shown when the server reports the trader docked at home, no sooner than 15 seconds after the raid pop-up closes; it may carry semiconductors. |

## GNN stories (2026-10-29)

Fourteen paintings for the season's news stories, briefed in
[docs/gnn-art-prompts.md](../../../docs/gnn-art-prompts.md) and generated at
1536 × 1024 (3:2). The masters stay as delivered; `generation-manifest.json` in
[../gnn-v1](../gnn-v1/README.md) records each prompt, its references and its
SHA-256. The build cuts each one's 16:9 band (1536 × 864, its offset in
`STORY_EVENT_BAND`) and sizes it to the 1672 × 941 frame.

| Pop-up | Masters |
| --- | --- |
| Decrees (`decreeEvent`) | `decree-levies.png`, `decree-inspections.png`, `decree-martial.png` |
| Crackdowns (`crackdownEvent`) | `sabotage-at-the-hub.png`, `convoy-massacre.png`, `quarantine.png`, `relic-seizure.png`, `keystone-crackdown.png` |
| False flag exposed (`exposedEvent`) | `false-flag-exposed.png` |
| The relic journey (`journeyEvent`) | `sealed-world-found.png`, `authority-spy.png`, `authority-war.png` |
| Endings (`endingEvent`) | `seal-fires.png`, `charter-holds.png` |

"The relics stir" (`relicsEvent`) shows `relics-awaken.png`.

## Chapter openings (2026-10-30)

Three paintings for the charter's chapter openings (`chapterEvent`), briefed in
[docs/chapter-art-prompts.md](../../../docs/chapter-art-prompts.md), generated
at 1536 × 1024 and kept as delivered. `chapter-generation-manifest.json`
records each prompt, its references, its revisions and its SHA-256. The build
cuts them like the GNN paintings.

| Chapter | Master | Opens on | Band |
| --- | --- | --- | --- |
| Pirates, "Strike back" | `chapter-pirates.png` | Shipyard staffed | row 64: the privateer's trail clear of the top |
| Expansion, "Lanes open" | `chapter-expansion.png` | Privateer den eliminated | row 64: the Hub's glint clear of the top |
| Reach, "Into the belt" | `chapter-reach.png` | Nearby system charted | row 0: the star and the den's wreck whole |

Two were edited once: the privateer's trail moved down into the safe band, and
a full space station in the Expansion sky became the Hub's distant glint.

Retired with charter v5 (2026-10-16), kept for history and no longer shown:
`counterfeit-semiconductors.png` (the kit's fake semiconductors) and
`pirates-on-the-lane.png` (the freighter crew sighting the privateer on the Hub
lane). Their briefs stay in `prompts.json`, marked `retired`.

## Pirate raid brief

The brief is the `pirate-raid` entry in `prompts.json`. The original references: the
first-landing illustration for style, the free-trader scene for the colony
itself (same landed ship, habitat and farm modules, unfinished solar array, dusk
plain), and `../privateer-raider-reference-v1/privateer-raider-reference.png` for
the ship. The revised privateer is landed beside the colony ship. Two armed
pirates carry a sealed crate up its loading ramp, a third receives it inside the
hold, and a rifle-bearing guard watches the unarmed colonists. Settlers watch
from the rocky foreground and near the habitats; one shields their eyes and
another pulls a companion back. One pirate ship, no trader, no weapons fire,
no text. The white colony ship remains the settlement's stationary command center.
The earlier cable-lift scene is preserved in `revisions/pirate-raid-v1-cable-lift.png`.

## Delivery

Save the master here under its slug, then run `npm run assets:derive`.
`buildStoryEventArt` (in `client/scripts/build-founding-story-art.mjs`) writes
WebP variants at 768, 1280 and 1672 pixels wide to
`client/public/art/derived/story-events/v1/`. No crop, stretching or upscaling.
The original three masters were generated on 2026-10-15 and the raid painting
on 2026-10-16 with the built-in image_gen tool, natively at 1672 × 941, and copied
without resizing or cropping. The raid was revised to show armed pirates physically
loading stolen cargo at the user's request. Exact generation prompts and reference
paths are in `prompts.json`; the original briefs are retained
alongside them. `generation-manifest.json` records the native files and SHA-256 hashes.

The first-landing illustration sets the visual style for all scenes. The trader
scene also uses its shipped top-view sprite for the hull and the counterfeit
painting for dusk/colony continuity. Its four gold side fittings remain docking
collars, with two rear engines, matching the approved model.

The retired Hub-lane scene uses the counterfeit painting for the cockpit's lighting language
and the approved Privateer Raider reference sheet for its distant ship: patched
orange/yellow armor, short wings and three engines. The first version accidentally
used the player Interceptor (the catalogue's `raider` key); it is preserved under
`revisions/pirates-on-the-lane-v1-interceptor.png`. The current master faces the
Privateer Raider toward the freighter, much farther away as a tiny ship in the
canopy, with its bow and guns aimed at the crew and its rear engine apertures
hidden. The previous closer front view is preserved as
`revisions/pirates-on-the-lane-v3-privateer-front-close.png`. The rear view is preserved as
`revisions/pirates-on-the-lane-v2-privateer-rear.png`. The cockpit composition is
retained. Its physical sensor display
contains abstract range rings and one contact, with no text or overlaid interface.

The existing pop-ups select these paintings automatically. Interim images remain
available only as load-error fallbacks.
