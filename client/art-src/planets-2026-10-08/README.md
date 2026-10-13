# Planet sprites added Oct 2026

| Look | Master | Used for |
| --- | --- | --- |
| Marginal world | `marginal-soft-edges-v1.png` | terrestrial worlds that colonists can't settle (Marginal or Hostile air) |

The master is a cleanup edit of `drafts/marginal.png`. That first pass had
stair-stepped edges between surface regions. Every alpha value is unchanged.
The two `*.prompt.json` files record the prompts and source paths.

Marginal ocean worlds use the ice sprite until their own art exists.

`client/scripts/build-planet-derivatives.mjs` derives the sizes from these
masters, together with the 2026-09-08 set. The matching ground painting is
`../planet-view/surfaces/marginal-surface-v1.png`.
