// Exercise the real Net class with a deterministic socket/clock harness. No
// browser, production server, or timer sleeps; TS is compiled only in memory.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import ts from "typescript";
import { decodeMessage, encodeMessage, PROTOCOL_CLOSE_CODE, SUBPROTOCOL } from "../src/wire.mjs";

const sockets = [];
const timers = new Map();
let timerId = 0;
let removedVisibility = false;
class Socket {
  static CONNECTING = 0; static OPEN = 1; static CLOSING = 2; static CLOSED = 3;
  readyState = Socket.CONNECTING;
  sent = [];
  constructor(url, protocols) { this.url = url; this.protocols = protocols; sockets.push(this); }
  open() { this.readyState = Socket.OPEN; this.onopen?.(); }
  send(bytes) { assert.ok(bytes instanceof Uint8Array, "outbound application frame is binary"); this.sent.push(bytes); }
  receive(bytes) { this.onmessage?.({ data: bytes }); }
  close(code = 1000) { this.readyState = Socket.CLOSED; this.onclose?.({ code }); }
}
globalThis.WebSocket = Socket;
globalThis.location = new URL("http://localhost:8080/");
globalThis.document = {
  visibilityState: "visible", addEventListener() {},
  removeEventListener(type) { if (type === "visibilitychange") removedVisibility = true; },
};
globalThis.window = {
  setTimeout(callback, delay) { const id = ++timerId; timers.set(id, { callback, delay }); return id; },
  clearTimeout(id) { timers.delete(id); },
};
const source = readFileSync(new URL("../src/net.ts", import.meta.url), "utf8");
const compiled = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext } }).outputText
  .replaceAll('"./wire.mjs"', JSON.stringify(new URL("../src/wire.mjs", import.meta.url).href));
const { Net } = await import(`data:text/javascript;base64,${Buffer.from(compiled).toString("base64")}`);
const received = [];
let replaced = 0;
let closes = 0;
let net;
net = new Net({
  onOpen: () => net.join("Binary Corp"), onMessage: (message) => received.push(message),
  onClose: () => closes++, onSessionReplaced: () => replaced++, onError() {},
});
net.setViewHz(5);
net.connect();
const first = sockets.at(-1);
assert.equal(first.protocols, SUBPROTOCOL);
assert.equal(first.binaryType, "arraybuffer");
first.open();
assert.deepEqual(decodeMessage(first.sent[0]), { type: "Join", name: "Binary Corp", view_hz: 5 });
net.send({ type: "HoldFleet", ship_id: "18446744073709551615" });
assert.deepEqual(decodeMessage(first.sent[1]), { type: "HoldFleet", ship_id: "18446744073709551615" });
for (const type of ["View", "BattleRecords", "OrderConfirmed"]) first.receive(encodeMessage({ type }).buffer);
assert.deepEqual(received.map((message) => message.type), ["View", "BattleRecords", "OrderConfirmed"]);

first.close(1006);
assert.equal(closes, 1);
assert.equal(timers.size, 1);
const [id, retry] = timers.entries().next().value;
assert.ok(retry.delay >= 400 && retry.delay <= 600);
timers.delete(id); retry.callback();
const second = sockets.at(-1);
second.open();
assert.deepEqual(decodeMessage(second.sent[0]), decodeMessage(first.sent[0]));
const count = received.length;
first.receive(encodeMessage({ type: "Error", message: "stale socket" }).buffer);
assert.equal(received.length, count);
second.close(4001);
assert.equal(replaced, 1);
assert.equal(timers.size, 0, "session replacement must not reconnect");

net.connect();
const third = sockets.at(-1);
third.open();
const invalid = encodeMessage({ type: "View" }); invalid[3]--;
const warn = console.warn; console.warn = () => {};
try { third.receive(invalid.buffer); } finally { console.warn = warn; }
assert.equal(third.readyState, Socket.CLOSED);
assert.equal(received.length, count + 1, "one clear protocol error, not duplicate toasts");
assert.match(received.at(-1).message, /Reload/);
assert.equal(timers.size, 0, "incompatible protocol must not reconnect in a loop");

net.connect();
const fourth = sockets.at(-1);
fourth.open();
net.send({ type: "MoveShip", ship_id: "1", dest: { x: Infinity, y: 0 } });
assert.equal(fourth.sent.length, 1, "non-finite intent was not sent");
assert.match(received.at(-1).message, /invalid number/);
fourth.close(PROTOCOL_CLOSE_CODE);
assert.equal(timers.size, 0);
net.disconnect();
assert.ok(removedVisibility);

// Authentication is checked before EVERY connection, including retries. An
// expired/revoked session must return to sign-in, not fight a newer browser.
const flush = () => new Promise(resolve => setImmediate(resolve));
const beforeAuthSockets = sockets.length;
const authHandlers = { onOpen() {}, onMessage() {}, onClose() {}, onError() {}, onSessionReplaced() { replaced++; } };
const denied = new Net({ ...authHandlers, beforeConnect: async () => false });
denied.connect();
await flush();
assert.equal(sockets.length, beforeAuthSockets, "unauthenticated client never opens a game socket");
assert.equal(timers.size, 0);
let allowPending;
const pending = new Net({ ...authHandlers, beforeConnect: () => new Promise(resolve => { allowPending = resolve; }) });
pending.connect(); pending.connect();
pending.disconnect(); allowPending(true);
await flush();
assert.equal(sockets.length, beforeAuthSockets, "late session check cannot reconnect after logout");
const authenticated = new Net({ ...authHandlers, beforeConnect: async () => true });
authenticated.connect();
await flush();
const signedInSocket = sockets.at(-1);
signedInSocket.open(); signedInSocket.close(4003);
assert.equal(timers.size, 0, "revoked cookie must not retry the socket");
authenticated.disconnect(); denied.disconnect();

globalThis.location = new URL("https://stellar.example/?server=wss://evil.example/ws");
const pinnedOrigin = new Net(authHandlers);
assert.equal(pinnedOrigin.url, "wss://stellar.example/ws", "URL overrides cannot redirect authenticated traffic");
pinnedOrigin.disconnect();
globalThis.location = new URL("http://localhost:5173/");
const development = new Net(authHandlers);
assert.equal(development.url, "ws://localhost:5173/ws", "Vite keeps account cookies same-origin");
development.disconnect();
console.log("PASS: binary Net transport, ordered dispatch, reconnect/rejoin, stale sockets, session replacement, visible protocol errors, invalid-order refusal.");
