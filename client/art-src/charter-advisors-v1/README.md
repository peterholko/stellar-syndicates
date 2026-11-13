# Charter Advisor portraits — version 1

Three opaque 1024 × 1024 PNG masters generated on November 9, 2026 with the
built-in image generator from the [portrait prompts](../../../docs/charter-advisor-portraits.md).

| Officer | Master | Manner |
| --- | --- | --- |
| Commander Imara Okafor | [veteran.png](veteran.png) | Calm, dry, unhurried |
| Lieutenant Daniel Seo | [lieutenant.png](lieutenant.png) | Earnest, precise, by the book |
| Major Rurik Halvorsen | [frontier.png](frontier.png) | Blunt, practical, wry |

[Compare the portraits and face crops](preview.png). The lower row shows
each portrait at 48, 72 and 112 pixels using the existing trader/pirate face
crop: 75% of the master, starting 14.3% from the left and 11.3% from the top.

Okafor was generated first from the approved captain style, Authority scene
and seal references. Her finished portrait then supplied the uniform and
room reference for the other two officers. Targeted framing edits added
headroom and aligned the faces for the common crop.

[generation.json](generation.json) records every generation and correction
prompt, reference paths and hashes, selected source paths, and final hashes.
The generator's 1254 × 1254 results were resized to the requested 1024 × 1024
without changing their square composition. Original generator outputs remain
in the built-in output directory; two intermediate exported compositions are
kept in `revisions/`.

**Derived and wired (Nov 9 2026).** `buildAdvisorPortraitArt` in
`client/scripts/build-founding-story-art.mjs` writes
`public/art/derived/advisors/v1/<slug>-320.webp` (this crop) and
`<slug>-face-128.webp` (a tighter head crop: 48% of the master from 25.5% left,
12% top) for the guide card and the stall nudge.
