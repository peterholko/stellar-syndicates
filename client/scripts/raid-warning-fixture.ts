// Isolated preview of the raid's tracking window (the "pirate warning") with
// the Charter Advisor's column (§charter-advisor): the real TrackingInset over
// an empty scene. No socket, no game.
import { Application, Container } from "pixi.js";
import { TrackingInset } from "../src/trackinginset";
import { ADVISORS } from "../src/core/derive/advisor";

const params = new URLSearchParams(location.search);
const officer = ADVISORS[Math.max(0, Math.min(2, Number(params.get("advisor") ?? 0)))];
const voice = params.get("voice") === "0" ? null : { portrait: officer.portrait, name: officer.name, line: officer.moments.raid };
// The game preloads the face before the raid (render.ts); so does this page.
const inset = new TrackingInset();
if (voice) inset.preloadFace(voice.portrait);
const app = new Application();
await app.init({ resizeTo: window, background: 0x05070d, antialias: true, autoStart: false });
document.body.append(app.canvas);
app.stage.addChild(inset.root);
const scene = new Container();
const camera = { trackCamera() {} };
const target = { x: 0, y: 0, px: 36, lx: 0, ly: 0, title: "RAIDER", color: 0xe08a2c };
// A timer, not the ticker: it keeps drawing in a background tab; `?frames=`
// stops it, so a headless capture can settle.
const frames = Number(params.get("frames") ?? Infinity);
let drawn = 0;
const timer = setInterval(() => {
  const area = { x: 0, y: 40, w: window.innerWidth, h: window.innerHeight - 40 };
  if (!inset.prepare(camera, "p1", target, area, "center", voice)) return;
  inset.render(app.renderer, scene, target, "RAIDER DETECTED · ON COURSE FOR FREYA");
  app.render();
  if (++drawn >= frames) clearInterval(timer);
}, 100);
