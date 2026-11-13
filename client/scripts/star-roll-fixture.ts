// §star-roll: every system star as the System View draws it, turning about
// its north–south axis (src/starroll.ts). No server; the real art and shader.
import { Application, Assets, Text, type Texture } from "pixi.js";
import { RollingStar, starPeriod } from "../src/starroll";
import { starArtwork, starArtLevel } from "../src/starart";
import { STAR_TYPES } from "../src/stars";

const params = new URLSearchParams(location.search);
const size = Number(params.get("size") ?? 220);
const speed = Number(params.get("speed") ?? 1);
let fixedT = params.has("t") ? Number(params.get("t")) : null;
const turnDeg = params.has("turn") ? Number(params.get("turn")) : null;
const only = params.get("only")?.split(",");
const slugs = STAR_TYPES.map((t) => t.slug).filter((slug) => !only || only.includes(slug));
const cols = Math.min(slugs.length, Number(params.get("cols") ?? 5));
const rows = Math.ceil(slugs.length / cols);

const app = new Application();
await app.init({ width: cols * size, height: rows * (size + 22), background: "#05070d", antialias: true,
  resolution: Math.min(devicePixelRatio || 1, 2), autoDensity: true });
document.getElementById("stage")!.appendChild(app.canvas);

const stars: { star: RollingStar; slug: string }[] = [];
for (const [i, slug] of slugs.entries()) {
  const art = starArtwork("system", slug);
  const level = starArtLevel(art, size * 0.8, app.renderer.resolution);
  const texture = await Assets.load<Texture>(level.url);
  texture.source.autoGenerateMipmaps = true;
  texture.source.scaleMode = "linear";
  const star = new RollingStar(texture, slug);
  star.anchor.set(art.anchor[0], art.anchor[1]);
  star.scale.set((size * 0.8) / (art.visualRatio * texture.width));
  star.position.set((i % cols) * size + size / 2, Math.floor(i / cols) * (size + 22) + size / 2);
  app.stage.addChild(star);
  const label = new Text({ text: slug, style: { fill: 0x8aa0b8, fontSize: 12, fontFamily: "system-ui" } });
  label.anchor.set(0.5, 0);
  label.position.set(star.position.x, star.position.y + size / 2 + 2);
  app.stage.addChild(label);
  stars.push({ star, slug });
}

const start = performance.now();
const pose = () => {
  const t = fixedT ?? ((performance.now() - start) / 1000) * speed;
  for (const { star, slug } of stars) {
    const period = starPeriod(slug);
    star.roll(turnDeg !== null && period > 0 ? (turnDeg / 360) * period : t);
  }
};
pose();
app.ticker.add(pose);
(window as unknown as { __stars: typeof stars; __pose: typeof pose }).__stars = stars;
// Frame capture: hold the clock at `t` (s) and draw it now.
(window as unknown as { __at: (t: number) => void }).__at = (t: number) => {
  fixedT = t;
  pose();
  app.render();
};
(window as unknown as { __ready: boolean }).__ready = true;
