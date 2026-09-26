# Founding prologue — version 1

Three original cinematic illustrations generated with the **built-in image_gen
tool**, one scene per image. The exact prompt set and original output paths are
preserved in [prompts.json](prompts.json). No model-version identifier is claimed.
The original 1672 × 941 PNG masters are retained here unchanged.

| Scene | Master | In-game caption |
| --- | --- | --- |
| Beyond the Milky Way | [wormhole-opening-selected.png](wormhole-opening-selected.png) | Humanity has opened a wormhole to another galaxy. Beyond the crossing lies a new frontier of worlds waiting to be explored. |
| Your stellar charter | [stellar-charter.png](stellar-charter.png) | Your corporation has been granted a stellar charter: the right to settle a new planet. Two thousand settlers are ready to begin a new life under your leadership. |
| A world to call home | [first-landing.png](first-landing.png) | Start with a small settlement, not an empire. Develop your colony, turn its resources into trade, and decide what your corporation will become. |

## Delivery

The user-selected wormhole master is an unchanged copy of the supplied
attachment, with decoded pixels verified identical to the smaller, separated
`wormhole-opening-v2.png` composition. It supersedes the enlarged v3 version.
The original, v2 and v3 masters are retained for comparison, with their prompts
in `supersededImages` in the prompt set. Distinct image URLs prevent cached
copies of the rejected composition being reused. The other two scenes are
unchanged. No new image was generated for this final selection.

`client/scripts/build-founding-story-art.mjs` produces WebP variants at 768,
1280 and 1672 pixels wide from each native master. It is included in
`npm run assets:derive`. All nine derivatives live under
`client/public/art/derived/founding-intro/v1/`. No crop, stretching or upscaling
is applied. The client uses responsive image selection and contains the full
illustration on short screens. Native resolution is the upper limit, not a
claim of unlimited detail when zoomed.

The story is shared by desktop and mobile founding flows. It advances manually
with Back / Continue; the third scene leads to the existing profile choice.
Skip story or Escape goes straight to that choice, never to a settled colony.
Captions, descriptive alt text and controls are HTML rather than baked into the
art. No animation, autoplay, extra starting resources or gameplay rules were
added. Existing players do not receive the prologue again.

`npm run test:settlement` checks the three scenes, navigation, reconnection,
explicit landing confirmation and all image variants. An isolated visual
preview is available with `node scripts/test-settlement.mjs --serve`; it never
connects to the live game.
