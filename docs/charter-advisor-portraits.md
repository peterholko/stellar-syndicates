# Charter Advisor portraits

Peter (Nov 9 2026): "Give me a prompt to generate the charter authority
officer, let's have 3 different officers that new players randomly get." The
design is in [charter-advisor.md](charter-advisor.md).

Generated November 9, 2026 with the built-in image generator:
[Okafor](../client/art-src/charter-advisors-v1/veteran.png),
[Seo](../client/art-src/charter-advisors-v1/lieutenant.png), and
[Halvorsen](../client/art-src/charter-advisors-v1/frontier.png).
All three masters are opaque 1024 × 1024 PNGs. See the
[crop comparison](../client/art-src/charter-advisors-v1/preview.png) and
[exact generation record](../client/art-src/charter-advisors-v1/generation.json).
Derived and wired Nov 9 2026: `public/art/derived/advisors/v1/<slug>-320.webp`
(the shared face crop, for pop-ups) and `<slug>-face-128.webp` (a tighter
head crop, for the guide card and the nudge).

- **The cast:** three Terran Charter Authority officers. Each new player is
  dealt one for the charter.

  | Slug | Name | Manner |
  | --- | --- | --- |
  | `veteran` | Commander Imara Okafor | Calm, dry, unhurried |
  | `lieutenant` | Lieutenant Daniel Seo | Earnest, precise, by the book |
  | `frontier` | Major Rurik Halvorsen | Blunt, practical, wry |

- **Names** are the game's, never painted.
- **Order:** generate `veteran` first. Then attach her finished portrait to the
  other two, so all three wear the same uniform in the same room.

## Delivery

- **Save as:** `client/art-src/charter-advisors-v1/<slug>.png`. Square,
  1024 × 1024 asked, at least 512.
- **Attach for every portrait:**
  1. The style reference, `client/public/art/captains/astrid_nystrom_senior.png`.
  2. The Authority reference,
     `client/art-src/authority-aesthetic-study-v1/wormhole-market-crackdown.png`.
  3. The insignia, `client/art-src/authority-v2/terran-charter-authority-seal.png`.
- **Also attach for `lieutenant` and `frontier`:** the finished `veteran.png`.
- **Framing** matches the pirate portraits, so the same face crop works.

## veteran

```
Use case: character-portrait
Asset type: one square character portrait for Stellar Syndicates, a persistent science-fiction colony and trading game. She is a new player's Charter Advisor: the Terran Charter Authority officer assigned to guide a newly landed colony through its charter, shown small (48 to 112 pixels) on the tutorial card and beside her transmissions.
Input image 1: STYLE reference only, one of the game's approved captain portraits. Match its realistic painterly digital portrait style, head-and-shoulders framing, soft directional light and restrained detail. Do not copy that person, that uniform or the bridge.
Input image 2: AUTHORITY reference, the game's painting of Terran Charter Authority personnel and architecture: fitted ivory ceramic over dark navy uniforms, antique-gold insignia, brass details, tactile ivory mineral walls and narrow amber light slots. Take only these materials, colors and light for an officer's service uniform and her surroundings; do not copy any person, pose, weapon or the scene.
Input image 3: INSIGNIA reference, the Authority emblem: a thin orbital ring crossed by three slender vertical spires of staggered heights. Show it only as a small antique-gold pin at her collar, simplified.
Composition/framing: one opaque square portrait, exactly 1024 x 1024 pixels. Head and shoulders; the face a little above center, turned three-quarters toward the viewer, the eyes about 38% from the top. Keep the whole head inside the central 80% so the portrait survives a rounded-square crop. It must read at 48 pixels: a strong silhouette and clear light-dark separation between the face and an uncluttered background.
Uniform: a high-collared dark navy Authority service tunic; a fitted ivory ceramic mantle over both shoulders with a thin brass edge; the small antique-gold ring-and-three-spire pin at the collar; two thin antique-gold bars on the mantle's edge for her seniority.
Subject: a senior Authority officer in her late fifties who has seen a dozen colonies founded and several fail: calm, dry and unhurried. Deep brown skin, close-cropped silver hair, fine lines at the eyes, a small old scar through her left eyebrow, small gold stud earrings. Expression: a steady, measuring gaze with the faint hint of an approving smile, as if this colony might just be one that stands.
Background: an Authority liaison alcove in the colony's new Command Center, softly out of focus: tactile ivory mineral wall panels with brass trim, one narrow amber light slot, the edge of a navy banner.
Style/medium: high-end realistic digital painting matching the captain portraits; grounded materials (pressed wool, polished ceramic, brass, skin texture), believable anatomy. Not a cartoon, not a photograph, not a poster.
Lighting/mood: warm amber key light from the light slot, a cool pale rim on the far side; reassuring, quietly formidable.
Constraints: absolutely no text, letters, numbers, labels, logos with writing, watermark, borders, frames or interface. No helmet, no weapons. Exactly one person. Deliver exactly one finished portrait.
```

## lieutenant

```
Use case: character-portrait
Asset type: one square character portrait for Stellar Syndicates, a persistent science-fiction colony and trading game. He is a new player's Charter Advisor: the Terran Charter Authority officer assigned to guide a newly landed colony through its charter, shown small (48 to 112 pixels) on the tutorial card and beside his transmissions.
Input image 1: STYLE reference only, one of the game's approved captain portraits. Match its realistic painterly digital portrait style, head-and-shoulders framing, soft directional light and restrained detail. Do not copy that person, that uniform or the bridge.
Input image 2: AUTHORITY reference, the game's painting of Terran Charter Authority personnel and architecture: fitted ivory ceramic over dark navy uniforms, antique-gold insignia, brass details, tactile ivory mineral walls and narrow amber light slots. Take only these materials, colors and light; do not copy any person, pose, weapon or the scene.
Input image 3: INSIGNIA reference, the Authority emblem: a thin orbital ring crossed by three slender vertical spires of staggered heights. Show it only as a small antique-gold pin at his collar, simplified.
Input image 4: UNIFORM MATCH reference, the first Charter Advisor's finished portrait. Match its uniform, mantle, collar pin, background alcove, framing and rendering exactly; paint a different person.
Composition/framing: one opaque square portrait, exactly 1024 x 1024 pixels. Head and shoulders plus one hand; the face a little above center, turned three-quarters toward the viewer, the eyes about 38% from the top. Keep the whole head and the hand inside the central 80% so the portrait survives a rounded-square crop. It must read at 48 pixels: a strong silhouette and clear light-dark separation between the face and an uncluttered background.
Uniform: identical to input image 4: a high-collared dark navy Authority service tunic, the fitted ivory ceramic mantle with its thin brass edge, the antique-gold ring-and-three-spire collar pin; one thin bar on the mantle's edge for his junior rank. Immaculate and freshly pressed.
Subject: a young Authority officer in his late twenties on his first founding assignment: earnest, precise and quietly excited. East Asian features, neat side-parted black hair, clean-shaven, bright attentive eyes. One hand holds a slim brass-framed data slate at chest height, its face turned away from the viewer. Expression: an eager, open half-smile, leaning very slightly forward, ready to walk you through the procedure.
Background: the same Authority liaison alcove as input image 4, softly out of focus, with a faint pale-blue glow from a holographic system map below the frame.
Style/medium: high-end realistic digital painting matching the captain portraits; grounded materials (pressed wool, polished ceramic, brass, skin texture), believable anatomy. Not a cartoon, not a photograph, not a poster.
Lighting/mood: soft pale-blue fill from the map below, warm amber key from the light slot; bright, helpful and by the book.
Constraints: absolutely no text, letters, numbers, labels, logos with writing, watermark, borders, frames or interface; nothing readable on the slate. No helmet, no weapons. Exactly one person. Deliver exactly one finished portrait.
```

## frontier

```
Use case: character-portrait
Asset type: one square character portrait for Stellar Syndicates, a persistent science-fiction colony and trading game. He is a new player's Charter Advisor: the Terran Charter Authority officer assigned to guide a newly landed colony through its charter, shown small (48 to 112 pixels) on the tutorial card and beside his transmissions.
Input image 1: STYLE reference only, one of the game's approved captain portraits. Match its realistic painterly digital portrait style, head-and-shoulders framing, soft directional light and restrained detail. Do not copy that person, that uniform or the bridge.
Input image 2: AUTHORITY reference, the game's painting of Terran Charter Authority personnel and architecture: fitted ivory ceramic over dark navy uniforms, antique-gold insignia, brass details, tactile ivory mineral walls and narrow amber light slots. Take only these materials, colors and light; do not copy any person, pose, weapon or the scene.
Input image 3: INSIGNIA reference, the Authority emblem: a thin orbital ring crossed by three slender vertical spires of staggered heights. Show it only as a small antique-gold pin at his collar, simplified.
Input image 4: UNIFORM MATCH reference, the first Charter Advisor's finished portrait. Match its uniform, mantle, collar pin, background alcove, framing and rendering; paint a different person, and wear the uniform as described below.
Composition/framing: one opaque square portrait, exactly 1024 x 1024 pixels. Head and shoulders; the face a little above center, turned three-quarters toward the viewer, the eyes about 38% from the top. Keep the whole head inside the central 80% so the portrait survives a rounded-square crop. It must read at 48 pixels: a strong silhouette and clear light-dark separation between the face and an uncluttered background.
Uniform: the same Authority service uniform as input image 4, worn hard: the high collar unhooked at the throat, the ivory ceramic mantle scuffed and chipped along its brass edge, the antique-gold collar pin dulled; two thin bars on the mantle's edge.
Subject: a weathered Authority officer in his mid-forties who has spent his career founding colonies on the frontier: blunt, practical and wry. Fair skin reddened by sun and wind, a close-cropped ginger beard going grey, short hair, deep crow's feet, a faded scar along his jaw. Expression: a lopsided, knowing smirk with one eyebrow slightly raised, as if about to tell you the shortcut the manual leaves out.
Background: the same Authority liaison alcove as input image 4, softly out of focus, a little more lived-in: a scuffed console edge and a dented brass fitting.
Style/medium: high-end realistic digital painting matching the captain portraits; grounded materials (worn wool, chipped ceramic, tarnished brass, weathered skin), believable anatomy. Not a cartoon, not a photograph, not a poster.
Lighting/mood: low warm amber console light from below-left, a cool dusk rim from behind; trustworthy and a little irreverent.
Constraints: absolutely no text, letters, numbers, labels, logos with writing, watermark, borders, frames or interface. No helmet, no weapons. Exactly one person. Deliver exactly one finished portrait.
```
