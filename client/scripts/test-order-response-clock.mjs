// Exercise the actual map badge without a Pixi context or a running galaxy.
// The forecast animates pixels only; pending evidence must never be retired here.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = readFileSync(new URL("../src/render.ts", import.meta.url), "utf8");
const ast = ts.createSourceFile("render.ts", source, ts.ScriptTarget.Latest, true);
const owner = ast.statements.find(n => ts.isClassDeclaration(n) && n.name?.text === "Renderer");
const method = owner.members.find(n => n.name?.getText(ast) === "drawOrderBadge").getText(ast);
const constants = ast.statements.filter(ts.isVariableStatement)
  .flatMap(n => [...n.declarationList.declarations])
  .filter(n => ["clamp01", "COL_OWN", "COL_COMMAND", "COL_DELAYED"].includes(n.name.getText(ast)))
  .map(n => `const ${n.getText(ast)};`).join("\n");
const plain = value => JSON.parse(JSON.stringify(value));

function fixture(text = method) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(`${constants}
    export class Renderer {
      half = 24;
      fleetHitRadius() { return this.half; }
      ${text}
    }`, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports });
  const renderer = new exports.Renderer();
  const ops = [];
  const cone = Object.fromEntries(["poly", "circle", "stroke", "fill", "moveTo", "lineTo", "arc", "roundRect"]
    .map(name => [name, (...args) => { ops.push({ name, args: plain(args) }); return cone; }]));
  const orderText = { text: "", visible: false, get width() { return this.text.length * 6; },
    position: { set(x, y) { this.x = x; this.y = y; } } };
  const sp = { cone, orderText };
  const ghost = { id: "own-interceptor", own: true, age: 90, pos: { x: 20_000, y: 0 } };
  const order = { id: 1, fleet_id: ghost.id, issued_at: 0, arrives_at: 10, response_at: 20, kind: "configure" };
  const state = { selectedOrderId: null, pendingOrders: new Map([[ghost.id, [order]]]) };
  const draw = now => {
    ops.length = 0;
    renderer.drawOrderBadge(ghost, state, sp, now);
    return { text: orderText.visible ? orderText.text : null, ops: plain(ops),
      position: { x: orderText.position.x, y: orderText.position.y } };
  };
  return { renderer, ghost, order, state, draw };
}

const hand = frame => frame.ops.filter(op => op.name === "lineTo").at(-1)?.args;
const rim = frame => frame.ops.find(op => op.name === "arc")?.args;
const near = (a, b) => assert.ok(Math.abs(a - b) < 1e-9, `${a} != ${b}`);

function countdownAndMotion(f) {
  assert.equal(f.draw(9.99).text, null, "no response timer during outbound travel");
  const start = f.draw(10), middle = f.draw(15), nextFrame = f.draw(15 + 1 / 60);
  assert.equal(start.text, "~10s");
  assert.equal(middle.text, "~5s", "use response_at, not the deliberately different ghost.age");
  assert.notDeepEqual(hand(middle), hand(nextFrame), "the clock hand must move every frame");
  assert.deepEqual(f.draw(15), middle, "same sim time means the same pixels, even while paused");
  near(rim(start)[4] - rim(start)[3], 2 * Math.PI);
  near(rim(middle)[4] - rim(middle)[3], Math.PI);
  assert.equal(f.draw(19.99).text, "~1s", "never round a pending estimate down to zero");
}

function remainsPending(f) {
  const atEstimate = f.draw(20), afterEstimate = f.draw(20.5);
  assert.equal(atEstimate.text, "awaiting");
  assert.equal(afterEstimate.text, "awaiting");
  assert.equal(rim(atEstimate), undefined);
  assert.notDeepEqual(hand(atEstimate), hand(afterEstimate), "overdue is still waiting, not a completed clock");
  assert.equal(f.state.pendingOrders.get(f.ghost.id).length, 1, "animation cannot confirm or remove an order");
}

const f = fixture();
countdownAndMotion(f);
remainsPending(f);

// Arrived confirmation removes the lifecycle; pooled text must not remain.
f.state.pendingOrders.clear();
assert.equal(f.draw(21).text, null);
assert.equal(f.draw(21).ops.length, 0);
f.state.pendingOrders.set(f.ghost.id, [f.order]);
f.order.lost = true;
assert.equal(f.draw(15).ops.length, 0);
f.order.lost = false;
f.ghost.own = false;
assert.equal(f.draw(15).ops.length, 0);
f.ghost.own = true;

// Selected lifecycle wins; otherwise preserve the map's latest-order policy.
const newer = { ...f.order, id: 2, arrives_at: 30, response_at: 40 };
f.state.pendingOrders.set(f.ghost.id, [f.order, newer]);
assert.equal(f.draw(15).text, null);
f.state.selectedOrderId = 1;
assert.equal(f.draw(15).text, "~5s");
f.state.selectedOrderId = null;
newer.lost = true;
assert.equal(f.draw(15).text, "~5s");

// Longer waits use the served estimate, never an invented local delay.
f.order.response_at = 50;
assert.equal(f.draw(15).text, "~35s");
f.order.response_at = Infinity;
assert.equal(f.draw(15).text, "awaiting");
f.order.response_at = 11;
assert.equal(f.draw(10).text, null, "preserve suppression of sub-1.5s waits near CC");
f.order.response_at = 20;

// The badge moves clear of a larger hull but never grows with the camera.
const small = f.draw(15);
f.renderer.half = 100;
const large = f.draw(15);
assert.deepEqual(small.ops.filter(op => op.name === "circle").map(op => op.args[2]),
  large.ops.filter(op => op.name === "circle").map(op => op.args[2]));
near(large.position.y - small.position.y, -76);

// Production wiring must tick the badge each draw from the continuous clock.
const drawGhost = owner.members.find(n => n.name?.getText(ast) === "drawGhost").getText(ast);
assert.match(drawGhost, /const liveSim = liveSimTime\(\)/);
assert.match(drawGhost, /this\.drawOrderBadge\(ghost, state, sp, liveSim\)/);

// Regression teeth in isolated copies: static hands and timer-based completion
// both fail, without editing the user's working tree to simulate a rollback.
assert.throws(() => countdownAndMotion(fixture(method.replace(
  /const hand = [^;]+;/, "const hand = top;"))), /clock hand must move/);
assert.throws(() => remainsPending(fixture(method.replace(
  "const remaining = order.response_at - now;",
  "const remaining = order.response_at - now; if (remaining <= 0) return;"))), /awaiting/);
console.log("Order response clock: countdown, smooth hands, draining rim, evidence-only completion, ownership, selection, zoom and regression teeth passed.");
