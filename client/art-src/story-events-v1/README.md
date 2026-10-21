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
