# Galaxy map glyph sheets

Four matched sheets generated sequentially with the built-in image generation tool. Each PNG is 1536 × 1024 with six symbols in a three-column, two-row layout. The first sheet served as the style reference for later sheets. A targeted image-tool edit corrected the delivery crate to a flat front view.

Files and reading order:

- `01-exploration-sites.png`: derelict ship, ring station, asteroid cluster, spatial anomaly, precursor artifact, unidentified contact.
- `02-contracts-part-1.png`: bounty, patrol, salvage, escort, combat objective, enforcement.
- `03-contracts-part-2.png`: strategic control, regional mandate, megaproject, survey, delivery, map pin.
- `04-contract-issuers.png`: trade market, survey office, salvage office, regional council, syndicate, Authority.

`generation-prompts.json` records every generation prompt, the delivery correction, source paths and the built-in mode. `image-validation.json` records dimensions and exact-palette checks.

The images are generated source art. The generator introduced off-white and near-black pixels despite the binary-palette prompt. Exact two-color cleanup and normalization to approximately 332 pixels per 512-pixel cell are pending explicit authorization for Python image editing. No game assets or runtime code were replaced.
