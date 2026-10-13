// Development-only synthetic evidence, passed through the production theater.
// Does not open a socket, send orders or modify any saved game.
import { THEATER_EPILOGUE_ROUNDS, theaterAttach, theaterClose, theaterDebug, theaterResetCamera, theaterSetTime, theaterStep } from "../src/battletheater";
import { SHIP_MODELS } from "../src/ship-models.generated";
import type { BattleRecordView, KeyframeView, RecordCount, ShipKind } from "../src/protocol";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id)! as T;
const stage = $("stage"), scene = $<HTMLSelectElement>("scene"), kind = $<HTMLSelectElement>("kind");
const kinds = Object.keys(SHIP_MODELS) as ShipKind[];
for (const value of kinds) kind.add(new Option(SHIP_MODELS[value].title, value));
kind.value = "cruiser";
const combat: ShipKind[] = ["raider", "corvette", "destroyer", "cruiser", "battleship", "dreadnought", "titan"];
let time = 0, playing = true, closed = false, previous = performance.now(), infoAt = 0;
/** A real engine fight (client/public/dev/battles), when one is selected. */
let engine: BattleRecordView | null = null;

function makeRecord(): BattleRecordView {
  const inspect = scene.value === "single";
  const types = scene.value === "all" ? kinds : inspect ? [kind.value as ShipKind] : combat;
  const n = scene.value === "stress" ? 40 : types.length;
  const ships: KeyframeView["ships"] = [];
  for (let side = 0; side < 2; side++) for (let i = 0; i < n; i++) {
    const rows = n > 18 ? 10 : n > 7 ? 9 : n;
    ships.push({ cid: side * 100 + i + 1, side, kind: types[i % types.length], hp: 1,
      x: (side ? 1 : -1) * ((inspect ? 200 : 380) + Math.floor(i / rows) * 170),
      y: (i % rows - (rows - 1) / 2) * (n > 18 ? 150 : n > 7 ? 185 : 250) });
  }
  const counts = (ss: KeyframeView["ships"], side: number): RecordCount[] => types.flatMap(type => {
    const exact = ss.filter(s => s.side === side && s.kind === type).length;
    return exact ? [{ kind: type, exact, class: exact === 1 ? "one" : "eight_to_fifteen" }] : [];
  });
  const deathTarget = ships.filter(s => s.side === 1).at(-1)!;
  const rec: BattleRecordView = { id: `battle-model-fixture-${scene.value}-${kind.value}`, pos: { x: 0, y: 0 }, system: null,
    started_at: 0, raid: false, fidelity: "participant", own_side: 0, light_frontier_tick: 4, outcome: "both_survive",
    sides: [0, 1].map(side => ({ corp: side ? "foe" : "own", posture: "engage_any", platform_tiers: 0,
      initial: counts(ships, side), flagship_name: side ? "Vesper" : "Aurora", loadouts: combat.map(type => ({ kind: type, n: 1,
        modules: type === "destroyer" ? ["torpedo_rack"] : type === "dreadnought" ? ["point_defense_screen"] : [] })) })) as BattleRecordView["sides"],
    rounds: [] };
  for (let round = 0; round < 5; round++) {
    const current = ships.filter(s => !(round >= 2 && s.cid === deathTarget.cid)).map(s => ({ ...s,
      x: s.x + (s.side ? -1 : 1) * round * (inspect ? 5 : 22),
      y: s.y + Math.sin(s.cid! + round * .3) * (inspect ? 0 : 18), hp: round > 0 ? 1 - round * .11 : 1 }));
    const previousShips = rec.rounds[round - 1]?.frame?.ships ?? ships;
    const gunfire: NonNullable<KeyframeView["gunfire"]> = [];
    if (round > 0 && !inspect) for (const s of previousShips) {
      if (!combat.includes(s.kind)) continue;
      const foes = previousShips.filter(t => t.side !== s.side);
      const target = foes[(s.cid! + round) % foes.length];
      if (target) gunfire.push({ side: s.side, from: s.cid!, to: target.cid!, weapon: s.cid! % 3 === 0 ? "driver" : "beam",
        damage: s.cid! % 5 === 0 ? 0 : 40 });
    }
    rec.rounds.push({ tick: round, counts: [counts(current, 0), counts(current, 1)], kills: [[], []], notes: [],
      dealt: [10000, 10000], frame: { ships: current, gunfire,
        deaths: round === 2 ? [{ step: 2, side: deathTarget.side, kind: deathTarget.kind, x: deathTarget.x - 22, y: deathTarget.y }] : [],
        torpedoes: inspect ? [] : [0, 1].map(side => ({ side, x: side ? 380 : -380, y: -220, n: Math.max(0, 8 - round * 2) })) } });
  }
  return rec;
}
let record = makeRecord();
/** Replay windows in the current record (a window runs keyframe → keyframe),
 * plus the epilogue an engine fight plays after its last record. */
const windows = () => Math.max(1, record.rounds.length - 1) + (engine ? THEATER_EPILOGUE_ROUNDS : 0);
function syncScrubber(): void {
  $<HTMLInputElement>("time").max = String(windows() - 0.001);
}
// Engine fights recorded on the current battle rules by the server's
// battle_fixtures test: listed from their index, loaded on demand.
void fetch("/dev/battles/index.json").then((r) => (r.ok ? r.json() : []), () => []).then((index: { name: string; title: string }[]) => {
  if (!index.length) return;
  const group = document.createElement("optgroup");
  group.label = "Real engine fights";
  for (const fight of index) group.append(new Option(fight.title, `engine:${fight.name}`));
  scene.append(group);
});
function attach(): void {
  if (closed) return;
  theaterAttach(stage, record, null, { width: stage.clientWidth, height: stage.clientHeight });
}
function changeScene(): void {
  const name = scene.value.startsWith("engine:") ? scene.value.slice(7) : null;
  const ready = name
    ? fetch(`/dev/battles/${name}.json`).then((r) => r.json()).then((fight: BattleRecordView) => ({ ...fight, id: `engine-${name}` }))
    : Promise.resolve(null);
  void ready.then((fight) => {
    engine = fight;
    theaterClose(); record = engine ?? makeRecord(); time = 0; closed = false; $("close").textContent = "Close viewer";
    syncScrubber(); attach(); theaterResetCamera();
  });
}
scene.addEventListener("change", changeScene);
kind.addEventListener("change", () => { scene.value = "single"; changeScene(); });
$("play").onclick = () => { playing = !playing; $("play").textContent = playing ? "Pause" : "Play"; };
$("reset").onclick = theaterResetCamera;
$("phone").onclick = () => { stage.classList.toggle("phone"); attach(); };
$("zoom").onclick = () => {
  const canvas = stage.querySelector("canvas"), r = canvas?.getBoundingClientRect();
  if (canvas && r) canvas.dispatchEvent(new WheelEvent("wheel", { deltaY: -300, clientX: r.x + r.width / 2,
    clientY: r.y + r.height / 2, cancelable: true }));
};
$("close").onclick = () => {
  closed = !closed; $("close").textContent = closed ? "Reopen viewer" : "Close viewer";
  if (closed) theaterClose(); else attach();
};
$<HTMLInputElement>("time").oninput = event => { playing = false; $("play").textContent = "Play"; time = +(event.target as HTMLInputElement).value; };
$("death").onclick = () => {
  playing = false; $("play").textContent = "Play";
  // The first window that ends in a recorded death.
  const round = Math.max(0, record.rounds.findIndex((r) => (r.frame?.deaths.length ?? 0) > 0) - 1);
  theaterSetTime(round, 0, false); theaterStep();
  const death = (theaterDebug()?.deaths as { t: number }[] | undefined)?.[0];
  time = round + (death?.t ?? .5) + .12;
};
new ResizeObserver(attach).observe(stage);
window.addEventListener("pagehide", theaterClose);
attach();
function tick(now: number): void {
  // Engine fights play at about the real battle pace; the samples slower.
  const rate = engine ? 1.5 : 1 / 2.5;
  if (playing && !closed && !document.hidden) time = (time + Math.min(.05, (now - previous) / 1000) * rate) % windows();
  previous = now;
  if (!closed) theaterSetTime(Math.floor(time), time % 1, false);
  $<HTMLInputElement>("time").value = String(time); $("clock").textContent = `${time.toFixed(2)} / ${windows()}`;
  if (now - infoAt > 500) { $("stats").textContent = JSON.stringify(theaterDebug()?.models ?? "Viewer closed", null, 2); infoAt = now; }
  requestAnimationFrame(tick);
}
requestAnimationFrame(tick);
