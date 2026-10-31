# GNN emblems — version 1

The news feed's two sources (`docs/gnn-art-prompts.md`), 1254 × 1254 PNGs with
transparency:

| Emblem | Master | Shown |
| --- | --- | --- |
| Galactic News Network | `gnn-emblem.png` | the ticker, News rows and the header of Authority stories |
| Free Signal | `free-signal-emblem.png` | the rebels' items, the false-flag and Seal pop-ups |

`buildNewsEmblems` (`client/scripts/build-art-derivatives.mjs`) trims each to
the emblem and writes lossless WebP at 48 and 128 px to
`client/public/art/derived/gnn-v1/`. `generation-manifest.json` records these
and the fourteen story paintings (`../story-events-v1`): prompts, references,
the generator's files and SHA-256 hashes.
