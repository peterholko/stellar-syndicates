# Habitable planet surfaces v1

Six approved, empty landscape panoramas. The PNG masters are 1672 × 941;
`prompts.json` preserves the generation prompts and original source paths.

| Scenery | Use |
| --- | --- |
| Highland Plateau | Mineral Highlands founding choice; habitable land worlds |
| Temperate Basin | Verdant Lowlands founding choice; habitable land worlds |
| Archipelago World | Coastal Plains founding choice; habitable ocean worlds |
| Savanna World | Habitable land worlds |
| Volcanic Highlands | Habitable land worlds |
| Boreal World | Habitable land worlds |

`client/scripts/build-planet-workbench-art.mjs` derives 768, 1280 and native
1672px-wide WebP images directly from these masters (never enlarging a thumbnail),
as part of `npm run assets:derive`. Runtime files live in
`client/public/art/derived/planet-workbench/habitable-v1/`.

The founding screen and planetary management scene share `planet-surfaces.ts`.
Existing saves select scenery deterministically from public system/body identity;
arrived founding profiles keep the image chosen during settlement. No resource,
habitability, capacity, gameplay timer or simulation rule depends on this art.
Uninhabitable worlds retain their previous landscapes and gas giants their globes.

Buildings remain separate served-state overlays. Each painting has a fixed
permutation of the existing 24 terrain addresses, keeping the opening colony on
land. Addresses stay attached to the same 16:9 plane on resize, upgrade and reload;
changing the background does not reset saved site preferences or grant new slots.
No cities, structures, text or UI are baked into the paintings.

Validation: `npm run test:planet-surfaces`, `npm run test:planet-panel`,
`npm run test:settlement`.
