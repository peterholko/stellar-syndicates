# Rocky planet colony concept

Generated with the built-in image_gen tool. This is a generated concept using the game's structure artwork as references, not a direct pixel-for-pixel sprite composite or a change to the live game.

The generated preview shows nine separate facilities. The prompt requested ten; this remains a visual layout study rather than an exact colony inventory.

Sources:
- Approved empty-pad planet concept: /Users/peter/.codex/generated_images/01a07fc6-daf8-7bb0-997a-3ce6703aa795/exec-67fa1d9d-32c5-40a9-84e9-32107473da98.png
- Game structure reference sheet: client/art-src/ui-icons/structures-preview.png
- Individual game sprites: client/public/art/ui_icons/structures/

Layout context checked in the code:
- crates/sim/src/build.rs defines 24 structure types.
- crates/sim/src/body.rs allocates resource, industrial, and infrastructure slots by body properties and population.
- One distinct built structure consumes one slot; upgrading its tier does not add another footprint.

## Exact generation prompt

Use case: compositing
Asset type: landscape planet-view concept for Stellar Syndicates.
Input images:
Image 1 is the EDIT TARGET: the rocky partial-planet landscape with seven empty construction pads.
Image 2 is the SOURCE ART: a labeled contact sheet of the actual existing game structure sprites. Its black background and labels are not part of any building.

Primary request: Place buildings from the game's source-art sheet onto the planet surface, showing a colony with MORE THAN SEVEN buildings and enough open land to expand. Use TEN individual structures from Image 2, one of each of the following exact designs:
- mining_complex: rocky industrial excavation facility.
- volatile_harvester: purple-lit vertical extraction towers.
- smelter: orange-glowing arched furnace.
- electronics_fabricator: low square facility with blue circuitry and cyan center.
- chemical_works: compact purple-lit tanks and pipes.
- fuel_refinery: steel pipework with gold trim and tall narrow towers.
- machine_works: low steel and orange factory with an open mechanical workshop.
- armaments_complex: blocky steel military factory with orange accents.
- habitat: round green-gold glazed central dome surrounded by small cylindrical towers.
- sensor_array: large tilted silver satellite dish on an industrial base.

Faithfully preserve the game art's distinctive silhouettes, roof layouts, recognizable small details and materials from Image 2. These must be the provided game's building designs, not generic replacements. Integrate them naturally into Image 1 as separate readable structures with small contact shadows and coherent scale; do not turn the source-art sheet itself into a billboard or put its labels into the world.

Change the seven oversized pads into a FLEXIBLE COLONY LAYOUT: smaller individual foundations for the ten selected buildings, distributed in three loose staggered depth bands across the broad terrain. Grow the settlement to use more of the available surface; do not cram ten buildings onto seven giant pads. Each building is separate, completely visible, comfortably spaced and large enough to recognize, with no overlapping silhouettes. Buildings nearer the horizon are a little smaller. Cluster the industrial facilities loosely toward the quarry and the sides, with the habitat and communications in the middle. Add a restrained branching service-road network appropriate to the new positions. Leave some flat empty terrain between clusters for future construction. This should feel like a developed playable colony, with enough room for many structures, rather than an illustration of seven fixed construction slots.

Preserve the approved partial-planet composition: broad curved rocky surface filling the lower part of the image, cropped by the bottom and sides, dark starfield above the curved horizon, upper-left sun and cool atmospheric rim. Preserve the wide landscape aspect ratio, rocky gray-brown palette, mountains, visible quarry and cinematic background quality. The buildings retain the crisp, slightly stylized industrial strategy-game artwork of Image 2. No interface, text, labels, numbers, borders, watermarks, giant new planets, people, or added spaceships.

