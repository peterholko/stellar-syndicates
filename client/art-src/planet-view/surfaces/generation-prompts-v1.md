# Empty planet surface backgrounds — v1

Generated with the built-in image_gen tool, using one separate image-edit call per world. These are empty environment assets for the planet-view concept; buildings, roads, construction pads, and other artificial features are absent. Gas giants were excluded as requested. The previously explored rocky/barren world is outside this batch.

All five images were visually reviewed for empty natural terrain, matching camera framing, planetary curvature, and open areas suitable for placing separate building sprites. The ocean variant uses natural islands, and the lava variant uses cooled basalt plateaus.

Composition reference: `/Users/peter/.codex/generated_images/01a07fc6-daf8-7bb0-997a-3ce6703aa795/exec-67fa1d9d-32c5-40a9-84e9-32107473da98.png`.

The game's visual planet types were checked in `client/src/systemview.ts`; the physical body classes are in `crates/sim/src/body.rs`.

## Terrestrial world

Output: `terrestrial-surface-v1.png`.

Biome reference: `client/public/art/celestial_sprites/planets/terrestrial.png`.

Exact prompt:

```text
Use case: precise-object-edit
Asset type: empty planet-surface background for the space strategy game Stellar Syndicates.
Input images: Image 1 is the EDIT TARGET and approved composition template, a partial rocky planet with empty construction pads. Image 2 is the existing game's small planet sprite, used ONLY as a reference for this world's biome and overall color family, never its full-globe framing.

Primary request: Create a matching EMPTY surface variant for the specified solid-world type. Keep Image 1's camera and visual format but transform all terrain to the specified biome.

Composition invariants: Wide landscape, approximately 16:9, matching Image 1's 1672 x 941 framing. High oblique three-quarter aerial view looking down over a broad convex planetary cap. The planet is cropped beyond the bottom and both side edges. The curved horizon reaches about one-fifth of the image height at the middle and falls toward the sides, with quiet black starfield above. Preserve a recognizable, strong planetary arc; this is not a full small globe, a floating island or a ground-level landscape. Maintain the same camera elevation, lens, horizon placement, upper-left raking sunlight, sparse starfield and realistic detailed cinematic game-environment style as the approved image.

Remove EVERY human-made feature from the template: all seven construction pads, their borders and lights, all roads, all connecting lines, and the terraced artificial quarry. Replace these completely with natural terrain in the new biome. No structures, foundations, pavement, sockets, building plots, vehicles, spaceships, lights or ruins. Empty means a pristine natural background with broad clear areas on which separate game-building sprites can later be placed. Do not bake in any fixed number of locations. Keep foreground and midground textures readable but not cluttered, with ample smooth continuous areas.

Output constraints: A single full-frame image of ONLY this biome. No collage, split view, comparison, border, UI, text, labels, numbers, logos or watermark. No other planets in the sky. No gas giant.

World-specific terrain:
A temperate terrestrial world with mixed natural terrain: broad gently rolling green grasslands and low mossy stone plains across the foreground and middle ground, scattered small forests mostly near the edges and distant foothills, distant blue-green mountains and one subtle winding river well away from the main open foreground. A few wisps of cloud near the distant horizon, never covering the foreground. The broad empty grassy plateaus must be the dominant near-surface feature, creating ample natural space for many future buildings. Natural greens, warm earth, gray stone, a thin cool-blue atmospheric rim. Lush but grounded and restrained, no giant alien plants. The land still follows the unmistakable large planetary curvature.
```

## Desert world

Output: `desert-surface-v1.png`.

Biome reference: `client/public/art/celestial_sprites/planets/desert.png`.

Exact prompt:

```text
Use case: precise-object-edit
Asset type: empty planet-surface background for the space strategy game Stellar Syndicates.
Input images: Image 1 is the EDIT TARGET and approved composition template, a partial rocky planet with empty construction pads. Image 2 is the existing game's small planet sprite, used ONLY as a reference for this world's biome and overall color family, never its full-globe framing.

Primary request: Create a matching EMPTY surface variant for the specified solid-world type. Keep Image 1's camera and visual format but transform all terrain to the specified biome.

Composition invariants: Wide landscape, approximately 16:9, matching Image 1's 1672 x 941 framing. High oblique three-quarter aerial view looking down over a broad convex planetary cap. The planet is cropped beyond the bottom and both side edges. The curved horizon reaches about one-fifth of the image height at the middle and falls toward the sides, with quiet black starfield above. Preserve a recognizable, strong planetary arc; this is not a full small globe, a floating island or a ground-level landscape. Maintain the same camera elevation, lens, horizon placement, upper-left raking sunlight, sparse starfield and realistic detailed cinematic game-environment style as the approved image.

Remove EVERY human-made feature from the template: all seven construction pads, their borders and lights, all roads, all connecting lines, and the terraced artificial quarry. Replace these completely with natural terrain in the new biome. No structures, foundations, pavement, sockets, building plots, vehicles, spaceships, lights or ruins. Empty means a pristine natural background with broad clear areas on which separate game-building sprites can later be placed. Do not bake in any fixed number of locations. Keep foreground and midground textures readable but not cluttered, with ample smooth continuous areas.

Output constraints: A single full-frame image of ONLY this biome. No collage, split view, comparison, border, UI, text, labels, numbers, logos or watermark. No other planets in the sky. No gas giant.

World-specific terrain:
A parched desert world: expansive pale ochre hardpan plains and broad level sandstone shelves dominate the foreground and middle ground, with subtle wind-swept sand textures. Graceful tawny dunes lie toward the sides and further back; eroded mesas, low red-brown ridges and long dry washes provide depth without filling the buildable foreground. Sparse small rocks. Keep plenty of level empty natural ground rather than making the whole surface steep dunes. Warm sand, amber, buff, muted rust; a very restrained dusty atmospheric rim. No oasis, vegetation, bones, ruins or artificial tracks.
```

## Ocean world

Output: `ocean-surface-v1.png`.

Biome reference: `client/public/art/celestial_sprites/planets/ocean.png`.

Exact prompt:

```text
Use case: precise-object-edit
Asset type: empty planet-surface background for the space strategy game Stellar Syndicates.
Input images: Image 1 is the EDIT TARGET and approved composition template, a partial rocky planet with empty construction pads. Image 2 is the existing game's small planet sprite, used ONLY as a reference for this world's biome and overall color family, never its full-globe framing.

Primary request: Create a matching EMPTY surface variant for the specified solid-world type. Keep Image 1's camera and visual format but transform all terrain to the specified biome.

Composition invariants: Wide landscape, approximately 16:9, matching Image 1's 1672 x 941 framing. High oblique three-quarter aerial view looking down over a broad convex planetary cap. The planet is cropped beyond the bottom and both side edges. The curved horizon reaches about one-fifth of the image height at the middle and falls toward the sides, with quiet black starfield above. Preserve a recognizable, strong planetary arc; this is not a full small globe, a floating island or a ground-level landscape. Maintain the same camera elevation, lens, horizon placement, upper-left raking sunlight, sparse starfield and realistic detailed cinematic game-environment style as the approved image.

Remove EVERY human-made feature from the template: all seven construction pads, their borders and lights, all roads, all connecting lines, and the terraced artificial quarry. Replace these completely with natural terrain in the new biome. No structures, foundations, pavement, sockets, building plots, vehicles, spaceships, lights or ruins. Empty means a pristine natural background with broad clear areas on which separate game-building sprites can later be placed. Do not bake in any fixed number of locations. Keep foreground and midground textures readable but not cluttered, with ample smooth continuous areas.

Output constraints: A single full-frame image of ONLY this biome. No collage, split view, comparison, border, UI, text, labels, numbers, logos or watermark. No other planets in the sky. No gas giant.

World-specific terrain:
A blue ocean-dominated world seen from the same high oblique near-surface orbital viewpoint. Vast deep-blue and teal seas occupy most of the visible curved planetary surface, stretching all the way to the distant curved horizon. In the foreground and middle ground, a small natural archipelago of broad low rocky islands and substantial coastal shelves provides generous empty land for future buildings. Two large foreground islands or peninsulas with gently undulating, nearly level gray-tan stone interiors and sparse low coastal greenery; a few smaller islands recede toward the horizon. Ocean must remain the dominant planetary feature. The islands have irregular natural shorelines, shallow turquoise lagoons, and restrained white surf. Do not make lots of tiny circular stepping-stone islands, artificial islands, platforms or building pads. Clear atmosphere with a subtle blue rim and sparse low distant clouds, leaving all foreground islands readable.
```

## Ice world

Output: `ice-surface-v1.png`.

Biome reference: `client/public/art/celestial_sprites/planets/ice.png`.

Exact prompt:

```text
Use case: precise-object-edit
Asset type: empty planet-surface background for the space strategy game Stellar Syndicates.
Input images: Image 1 is the EDIT TARGET and approved composition template, a partial rocky planet with empty construction pads. Image 2 is the existing game's small planet sprite, used ONLY as a reference for this world's biome and overall color family, never its full-globe framing.

Primary request: Create a matching EMPTY surface variant for the specified solid-world type. Keep Image 1's camera and visual format but transform all terrain to the specified biome.

Composition invariants: Wide landscape, approximately 16:9, matching Image 1's 1672 x 941 framing. High oblique three-quarter aerial view looking down over a broad convex planetary cap. The planet is cropped beyond the bottom and both side edges. The curved horizon reaches about one-fifth of the image height at the middle and falls toward the sides, with quiet black starfield above. Preserve a recognizable, strong planetary arc; this is not a full small globe, a floating island or a ground-level landscape. Maintain the same camera elevation, lens, horizon placement, upper-left raking sunlight, sparse starfield and realistic detailed cinematic game-environment style as the approved image.

Remove EVERY human-made feature from the template: all seven construction pads, their borders and lights, all roads, all connecting lines, and the terraced artificial quarry. Replace these completely with natural terrain in the new biome. No structures, foundations, pavement, sockets, building plots, vehicles, spaceships, lights or ruins. Empty means a pristine natural background with broad clear areas on which separate game-building sprites can later be placed. Do not bake in any fixed number of locations. Keep foreground and midground textures readable but not cluttered, with ample smooth continuous areas.

Output constraints: A single full-frame image of ONLY this biome. No collage, split view, comparison, border, UI, text, labels, numbers, logos or watermark. No other planets in the sky. No gas giant.

World-specific terrain:
A frozen world: immense pale blue-white ice plains and wind-smoothed snowfields dominate the foreground and middle ground, with broad gently undulating level areas available for future buildings. Translucent blue glacial ridges, weathered ice boulders and low frost-covered mountains mostly toward the sides and distance. A few narrow deep-blue crevasses run near the edges, leaving large contiguous central and foreground fields intact and clear. Restrained crystalline surface textures and soft snow drifts, believable subdued frost, no huge jagged spikes crowding the scene. Cold white, slate blue and pale cyan; thin icy-blue rim at the curved horizon. No blizzard, aurora, animals, tracks or liquid-water ocean.
```

## Lava world

Output: `lava-surface-v1.png`.

Biome reference: `client/public/art/celestial_sprites/planets/lava.png`.

Exact prompt:

```text
Use case: precise-object-edit
Asset type: empty planet-surface background for the space strategy game Stellar Syndicates.
Input images: Image 1 is the EDIT TARGET and approved composition template, a partial rocky planet with empty construction pads. Image 2 is the existing game's small planet sprite, used ONLY as a reference for this world's biome and overall color family, never its full-globe framing.

Primary request: Create a matching EMPTY surface variant for the specified solid-world type. Keep Image 1's camera and visual format but transform all terrain to the specified biome.

Composition invariants: Wide landscape, approximately 16:9, matching Image 1's 1672 x 941 framing. High oblique three-quarter aerial view looking down over a broad convex planetary cap. The planet is cropped beyond the bottom and both side edges. The curved horizon reaches about one-fifth of the image height at the middle and falls toward the sides, with quiet black starfield above. Preserve a recognizable, strong planetary arc; this is not a full small globe, a floating island or a ground-level landscape. Maintain the same camera elevation, lens, horizon placement, upper-left raking sunlight, sparse starfield and realistic detailed cinematic game-environment style as the approved image.

Remove EVERY human-made feature from the template: all seven construction pads, their borders and lights, all roads, all connecting lines, and the terraced artificial quarry. Replace these completely with natural terrain in the new biome. No structures, foundations, pavement, sockets, building plots, vehicles, spaceships, lights or ruins. Empty means a pristine natural background with broad clear areas on which separate game-building sprites can later be placed. Do not bake in any fixed number of locations. Keep foreground and midground textures readable but not cluttered, with ample smooth continuous areas.

Output constraints: A single full-frame image of ONLY this biome. No collage, split view, comparison, border, UI, text, labels, numbers, logos or watermark. No other planets in the sky. No gas giant.

World-specific terrain:
A volcanic world with predominantly SOLID cooled basalt crust: broad charcoal-gray and muted dark brown rock plateaus dominate the foreground and middle ground and provide generous stable-looking empty areas for future buildings. Natural glowing orange-red magma fissures and a few narrow molten channels thread along the edges and between the broad cooled plateaus, becoming a fine glowing network in the distance. Low volcanic ridges and distant subdued cones, a little thin distant haze, no foreground smoke or huge erupting plume. Rough basalt detail but clear readable open areas. The surface is not one unbroken lake of lava. Dark graphite rock, ember orange, restrained sulfur ochre, very subtle warm atmospheric rim. Maintain sufficient sunlight and fill light that the solid ground is clearly visible.
```


