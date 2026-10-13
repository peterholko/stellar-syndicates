# Bulk Freighter reference sheet

Five-view colored concept reference for the Bulk Freighter in Stellar Syndicates: **front, back, top, bottom and side**.

- Final asset: [bulk-freighter-reference.png](bulk-freighter-reference.png)
- Resolution: 1774 × 887 pixels
- Generated on: 2026-09-29
- Method: built-in `image_gen`, guided by the original ship artwork, followed by targeted projection and side-cargo corrections.

## Design

The original long cargo-train silhouette, exposed central lattice spine, tiny pointed command prow and separate stern reactor/drive housing are retained. Off-white and silver armor, graphite trusses, amber clamps and cyan glazing/lights follow the existing freighter artwork.

The top and bottom views establish eight longitudinal cargo stations: four pairs of large transverse mineral silos alternating with four pairs of shorter cargo cages, for sixteen cargo modules in total. They attach to the central spine through short lattice arms, with open gaps between modules. The original written brief called for five paired banks; this sheet follows the four repeating silo-and-cage groups visible in the delivered game sprite.

The underside has distinct silo service hatches, cage inspection panels, reinforced spine details and reactor maintenance covers. The rear has eight exhaust apertures in one horizontal row at a common height. These engines overlap as one nozzle profile in the side elevation. The front shows the command pod end-on between the nearest pair of silos.

This is an illustrated modeling reference. Use the top and bottom views to establish cargo count and longitudinal placement. The end and side drawings guide thickness and appearance; reconcile illustrated scale, side-bay spacing and small projection differences during 3D reconstruction. No surveyed dimensions are implied.

## References

- Ship identity: [original Bulk Freighter sprite](../freighters-2026-09-16/bulk-freighter-alpha.png)
- Presentation and rendering style: [Heavy Freighter reference](../heavy-freighter-reference-v1/heavy-freighter-reference.png)

## Prompt sequence

1. [Initial generation prompt](prompt.txt) → [draft sheet](draft-reference.png)
2. [Front and side projection correction](projection-correction-prompt.txt) → [projection draft](projection-corrected-draft.png)
3. [Side cargo correction](side-cargo-correction-prompt.txt) → [final reference](bulk-freighter-reference.png)

[Artifact manifest](artifact-manifest.json) records dimensions, checksum, input assets and original generation paths. This project addition is limited to the reference folder.
