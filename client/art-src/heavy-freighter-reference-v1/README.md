# Heavy Freighter reference sheet

Five-view colored concept reference for the Heavy Freighter in Stellar Syndicates: **front, back, top, bottom and side**.

- Final asset: [heavy-freighter-reference.png](heavy-freighter-reference.png)
- Resolution: 1774 × 887 pixels
- Generated on: 2026-09-29
- Method: built-in `image_gen`, guided by the existing ship artwork, followed by a targeted front-elevation correction.

## Design

The existing broad armored cargo body, four major hatch plates in a two-by-two array, continuous central service spine, projecting command prow, folded flank cranes and narrower stern machinery block are retained. Off-white and silver armor, graphite structure, amber fittings and cyan glazing/lights follow the original game art.

The underside has its own cargo belly panels, reinforced central keel, vents, maintenance hatches and crane-mount details. The front shows the command prow end-on; the rear shows eight exhaust apertures in two rows of four. The engine columns overlap in side view, leaving two vertically stacked nozzle profiles.

The legacy text brief in `freighters-2026-09-16/prompts.json` called for six thrusters, while the delivered Heavy Freighter sprite visibly contains eight apertures. This sheet follows the actual sprite and interprets those as four paired engine stacks. The ship's gameplay capacity and other game data are unchanged.

## References

- Ship identity: [original Heavy Freighter sprite](../freighters-2026-09-16/heavy-freighter-alpha.png)
- Presentation and rendering style: [Large Freighter sheet](../large-freighter-reference-v1/large-freighter-reference.png)

## Prompt sequence

1. [Initial generation prompt](prompt.txt) → [draft-reference.png](draft-reference.png)
2. [Front projection correction prompt](front-projection-correction-prompt.txt) → [final reference](heavy-freighter-reference.png)

[Artifact manifest](artifact-manifest.json) records dimensions, checksum, input assets and original generation paths.

This is an illustrated modeling reference. View scales and small projection differences should be reconciled during 3D reconstruction; no surveyed dimensions are implied. The project addition is limited to this reference folder.
