# Large Freighter reference sheet

Five-view concept reference for the Large Freighter in Stellar Syndicates: **front, back, top, bottom, and side**.

- Final asset: [large-freighter-reference.png](large-freighter-reference.png)
- Resolution: 1774 × 887 pixels
- Generated on: 2026-09-29
- Method: built-in `image_gen`, reference-guided generation with targeted projection and side-module corrections.

## Design

The original twin-keel catamaran layout is preserved: three cargo sections per keel, six sections total, a long open central channel, forward braced crossbeam and command cabin, rear machinery yoke, and four engines in two pairs. Silver and off-white plating, graphite structure, gold fittings, and cyan lights match the game art.

The bottom includes dedicated cargo belly panels, service hatches, cooling vents, and underside treatment for the cabin, keels, crossbeams and engines. The side elevation shows three separate cargo sections; the side-by-side engines overlap in profile.

## References

- Ship identity: [original Large Freighter sprite](../freighters-2026-09-16/large-freighter-alpha.png)
- Presentation and rendering style only: [Small Freighter sheet](../small-freighter-reference-v1/small-freighter-reference.png)

## Prompt sequence

1. [Initial generation](prompt.txt) → [draft-reference.png](draft-reference.png)
2. [Projection correction](projection-correction-prompt.txt) → [projection-pass.png](projection-pass.png)
3. [Side cargo-module correction](side-modules-correction-prompt.txt) → **large-freighter-reference.png**

Original generated files are retained at:

```text
/Users/peter/.codex/generated_images/01a0ee2d-ea60-75d3-b329-15b3aab8b494/exec-b170a936-09f9-462d-835c-8b0e38257e68.png
/Users/peter/.codex/generated_images/01a0ee2d-ea60-75d3-b329-15b3aab8b494/exec-91bf108e-e0e7-4ec6-b3dd-d6d169ea9db8.png
/Users/peter/.codex/generated_images/01a0ee2d-ea60-75d3-b329-15b3aab8b494/exec-bf97c1f7-b2a2-4607-bb4f-ee50bfc92331.png
```

This is a concept-art reference. Front and back elevations are enlarged for detail; reconcile view scales and minor illustrated projection differences during 3D reconstruction. No Blender was used, and no runtime game assets were replaced.
