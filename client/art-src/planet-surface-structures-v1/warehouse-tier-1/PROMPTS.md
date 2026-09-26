# Warehouse Tier I — planet-surface sprite draft

Generated with the built-in image-generation tool on 2026-09-26. The tool does not expose its model identifier.

## Status: NOT production-ready

`warehouse-tier-1-draft.png` is the final returned draft, not a usable transparent sprite. Inspection with Sharp confirmed a 1254 × 1254 RGB PNG, three channels and no alpha. The checkerboard is painted into the image. Requested output is 1024 × 1024 RGBA, with zero-opacity surroundings and a semitransparent attached lower-right shadow. All three generation attempts failed to deliver real alpha. Framing, softened pad edge and shadow need rechecking after background cleanup.

This is a separate surface-scene art experiment. No existing icon or game-rendering code was changed. Do not integrate this draft as though it were the finished asset.

## Supplied references

1. Warehouse design: `/var/folders/73/__c45x1d6cz49h74ptlg3r3h0000gn/T/codex-clipboard-dac61680-2f2d-454b-bc4b-008bd892d8ec.png`
2. Planet lighting/rendering: `/var/folders/73/__c45x1d6cz49h74ptlg3r3h0000gn/T/codex-clipboard-9c24006d-2ae9-4cc6-a6de-21979e0aa0ad.png`
3. Pad/shadow composition: `/var/folders/73/__c45x1d6cz49h74ptlg3r3h0000gn/T/codex-clipboard-a813ca33-1012-482c-a911-d30c1e31af80.png`

## Initial prompt

Use case: stylized-concept. Asset type: one standalone building sprite for the PLANET-SURFACE SCENE of a space strategy game, transparent PNG, exactly 1024 x 1024. This is a second art set used only on the surface scene; the existing icon set stays as it is.

STYLE REFERENCES: image 1 is this structure's existing design identity, the Warehouse tier I sprite. Keep its exact building design, silhouette, roof layout, equipment, colours and materials; change only the camera, the rendering style and the ground contact described below. Image 2 is the LIGHTING AND RENDERING REFERENCE: the planet-surface painting this sprite will be composited onto. Match its level of realism, its soft upper-left sunlight, its colour temperature and its gentle contrast. Do not copy any terrain, water or sky from it. Image 3 shows the intended result of compositing: separate buildings on small graded pads with soft contact shadows; use it for the pad and shadow only.

CAMERA: elevated three-quarter view from the front-left with the camera about 55 degrees above the ground plane, clearly steeper than the 35-degree isometric of image 1, so the roof reads larger than the walls. Very mild perspective foreshortening, as if photographed from high altitude. The front and right walls remain visible but shorter.

RENDERING: soft semi-realistic 3D render. Edges are defined by light and material, NOT by ink or line-art outlines: no cel shading, no black contour strokes, no oversharpened highlights. Physically plausible matte steel and painted panels with mild weathering, subtle ambient occlusion in the seams, and restrained coloured service lights that glow softly. A slightly desaturated palette that sits naturally inside a landscape. Large readable forms for a 70 to 130 pixel sprite, not tiny noisy greebles.

GROUND CONTACT: bake a small graded foundation pad: a thin apron of compacted grey-brown gravel and regolith with a low perimeter curb, following the building's footprint with a margin of about one tenth of the building's width. The apron's outer edge is slightly irregular and fades to fully transparent over a few pixels, so it blends into grass, sand, ice or rock alike. No grass, soil colour, vegetation, roads, paths, vehicles or fencing beyond the pad. Bake a soft cast shadow falling to the LOWER RIGHT, from the upper-left sun, attached to the building and semi-transparent in the alpha channel with blurred edges, never a hard-edged black shape, plus a darker contact shadow where the walls meet the pad.

FRAMING: square canvas, pad and building centred, the building about 70% of the canvas width. The shadow may extend into the lower-right margin, but nothing touches the canvas edge. Genuine transparent alpha outside the pad and shadow with zero opacity: no checkerboard, no background colour, no vignette, no glow.

NO text, tier numbers, logos, UI, borders, sky, stars, horizon, terrain beyond the pad, people, ships or ink outlines. Keep the same compact site footprint across the family's upgrades; grow capability through recognizable equipment and vertical complexity, not a huge spread of buildings.

Subject: WAREHOUSE TIER I. Modest starter ground warehouse. ONE low curved-rib steel storage hall, one front loading door with amber safety stripes, one short delivery ramp and two neat freight crates. Simple cream and graphite walls, small cyan doorway light. No extra towers or second hall; recognizably the beginning of a cargo depot family.
Family color/material identity: Cream steel, dark ribs, amber loading stripes and cyan service lights.

Deliver only the single isolated building-and-pad sprite, not a scene, sheet, comparison or mockup. Preserve the design identity from image 1; images 2 and 3 contribute only the lighting/rendering and pad/shadow references respectively.

## Camera and transparency refinement prompt

Use case: stylized-concept. Refine this game building into one production-ready transparent PNG sprite, exactly 1024 x 1024 pixels.

REFERENCE ROLES: Image 1 is the current sprite candidate to correct. Image 2 is the original Warehouse Tier I design identity: preserve its one curved-rib cream-steel hall, graphite ribs, exact roof vent/ridge equipment, front loading door, amber safety stripes, short ramp, cyan service lights and two exterior freight crates. Image 3 is the lighting and gentle semi-realistic rendering reference ONLY. Image 4 is the compact foundation-pad and soft contact-shadow reference ONLY. Do not include any landscape or other buildings.

CRITICAL CORRECTIONS:
1. Image 1 has an erroneous checkerboard PAINTED INTO THE IMAGE. Remove it completely. Deliver an actual RGBA PNG with zero-opacity alpha outside the building/pad/shadow, and semitransparent alpha for the soft shadow. A transparency preview pattern is NOT acceptable. No checkerboard of any size, no grey or white background, no background color, no backdrop. The empty space must be genuinely transparent.
2. Raise the camera to about 55 degrees ABOVE THE GROUND PLANE, noticeably steeper than the current candidate. This is a roof-dominant, elevated three-quarter front-left viewpoint with very mild perspective, a distant high-altitude camera. Keep the reference azimuth: loading door toward lower-left, long right wall visible. The roof should occupy substantially more projected area, and the visible front and right walls should be shorter due to viewpoint. Do not redesign or physically squash the warehouse.
3. Keep the building around 70% of the square canvas width. Center the building and small pad, preserve a clear transparent border on all four sides; do not let the shadow or gravel touch the canvas edges.

RENDERING: soft semi-realistic 3D, mildly weathered matte cream steel and graphite panels; soft upper-left sunlight and gentle contrast matching image 3. Material-defined edges, NO black ink outlines, no cel shading, no oversharpened highlights, no metallic sparkle, no noisy greebles. Slightly desaturated colors. Cyan and amber service lights are small and restrained. Keep large readable forms at 70 to 130 screen pixels.

PAD: only a small thin graded grey-brown gravel/regolith apron with a low perimeter curb, following the warehouse footprint with a margin around one tenth of the building width. The outer edge is irregular and fades to zero alpha over a few pixels. No grass, vegetation, roads, paths, vehicles, fences or additional terrain. Attached soft cast shadow goes LOWER RIGHT from upper-left sunlight, with genuinely semitransparent alpha and blurred edges; darker contact shadow at wall/pad junction. Do not add detached floating shadows or a black hard-edged silhouette.

Keep ONE storage hall, no towers, second hall or extra structures. No text, tier numbers, logos, UI, borders, sky, stars, horizon, people, ships, glow or vignette. Retain all other design identity. Output just the single isolated building-and-pad sprite, not a scene, mockup or sheet.

## Final focused transparency prompt

Remove the background from this warehouse sprite. Return the building, its compact gravel foundation, and its soft lower-right cast shadow as a transparent-background PNG cutout with a real alpha channel. All empty space outside the sprite must have zero opacity; the soft cast shadow must have partial opacity. Preserve the warehouse, colors, elevated camera, pad, and full silhouette exactly. Do not add anything. No background at all. Deliver a square 1024 × 1024 PNG with the building occupying about 70% of the width and clear transparent padding.
