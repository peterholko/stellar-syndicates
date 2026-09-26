# Warehouse landscape previews

Generated with the built-in image-generation tool on 2026-09-26. These are illustrative scene previews, not production sprites or pixel-exact compositing tests. They are not referenced by the game client.

- `warehouse-planet-preview.png`: surface-scene warehouse on the central plain.
- `warehouse-comparison-preview.png`: original warehouse on the left, newer surface-scene version on the right, aligned at the same ground level.

The standalone sprite's transparency limitation remains documented in [PROMPTS.md](PROMPTS.md). These opaque landscape previews do not resolve it.

## Single-warehouse preview prompt

Use case: compositing / stylized-concept. Make a faithful game-scene preview by placing the warehouse from Image 2 into the landscape in Image 1.

Image 1 is the BASE IMAGE and edit target: preserve the entire wide 1672:941 composition, all terrain, forests, river bends, lake, mountains, horizon, sky, and its existing soft daylight. Do not crop or zoom.
Image 2 is the BUILDING INSERT REFERENCE, not a background: extract only its single Warehouse Tier I building, its compact foundation apron, and attached soft shadow. Its grey checkerboard is a baked-in unwanted background and must be removed completely. No rectangle or checker pattern should appear in the finished scene.

Place ONE warehouse on the flat grassy central plain, centered at roughly 50% of the image width and 59% of its height. Make it a SMALL in-game building: the building should occupy about 7–8% of the total scene width (approximately 120–130 pixels wide when the landscape is 1672 pixels wide), with ample empty grassland around it. This is not a close-up or a hero-sized building.

Preserve the reference warehouse's exact recognizable design: one low curved-rib cream steel hall with graphite ribs and central roof ridge vents, one front loading doorway and short ramp with amber safety stripes, muted cyan service lights, and two neat exterior freight crates. Preserve the roof-dominant elevated three-quarter view; entrance faces lower-left and the long right wall remains visible. No additional halls, towers, vehicles, or equipment. Keep its compact grey-brown gravel foundation pad and low curb, blending the thin slightly irregular apron edge into the grass. Match the landscape's soft upper-left sunlight, gentle contrast, desaturated material finish, darker contact shadow, and a subtle attached cast shadow to the lower right. It must look planted on the landscape, not floating or surrounded by a white halo.

The only change to Image 1 is this single modest warehouse and its immediate pad/contact shadow. No roads, paths, other buildings, people, labels, text, UI, borders, arrows, inset closeups, checkerboard, or added glow. Output a normal opaque landscape composite in the base image's wide aspect ratio.

## Comparison preview prompt

Use case: compositing.
Asset type: preview comparison for a space-strategy planet-surface scene.

Image 1 is the existing planetary composite and EDIT TARGET. Keep its landscape, framing, lighting, and the warehouse already standing in the central plain unchanged.
Image 2 is the PREVIOUS / ORIGINAL warehouse sprite to INSERT. Add this exact earlier warehouse to Image 1 beside the existing one. Do not replace the current warehouse.

Put the original warehouse to the LEFT of the existing warehouse, on the open grassland, with approximately one building-width of clear grass between them. IMPORTANT: the two warehouses must be at the SAME VERTICAL LEVEL in the image, their ground-contact baselines aligned on the same horizontal line (approximately y=590 in the 1672x941 scene). Neither building should appear above, behind, below or in front of the other. Keep the existing warehouse at its current position; center the added left warehouse at about 32% of image width. Match the apparent building width and ground footprint of the existing warehouse, around 145–155 pixels wide in the full scene, excluding soft shadow.

For a genuine comparison, preserve Image 2's original lower-angle camera, proportions, crisp rendering, taller visible walls, curved cream-steel ribbed roof, graphite frame, roof vents, amber loading stripes, cyan doorway light, ramp and freight crates. Do NOT redesign it into another copy of the newer warehouse or change its viewing angle. Extract only the warehouse and its accessories: the black/transparent surround must not appear. Add only a restrained contact shadow and soft lower-right cast shadow to seat it on the grass. Do not add a large platform or extra buildings.

Final image must have exactly TWO distinct warehouse versions, side by side at matching scale and aligned vertical ground level. Preserve the original existing warehouse completely. All sky, mountains, water, trees and surrounding grassland stay unchanged. Wide full-frame composition with no crop or zoom. No labels, text, UI, arrows, split panels, checkerboard or black background rectangles.
