// Cross-language protocol test; builds in-memory Rust fixtures, never connects
// to the user's running game or database. Run: npm --prefix client run test:wire
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { performance } from "node:perf_hooks";
import { decodeMessage, encodeMessage, MAX_CLIENT_FRAME, PROTOCOL_VERSION, SUBPROTOCOL } from "../src/wire.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
function rustTest(name, extraEnv = {}) {
  return execFileSync("cargo", ["test", "-p", "server", name, "--", "--exact", "--nocapture"], {
    cwd: root, env: { ...process.env, ...extraEnv }, encoding: "utf8", maxBuffer: 32 * 1024 * 1024,
  });
}
assert.equal(SUBPROTOCOL, `stellar.msgpack.v${PROTOCOL_VERSION}`);
const output = rustTest("wire::tests::real_server_messages_preserve_the_served_picture_and_measure_bytes", { STELLAR_WIRE_FIXTURES: "1" });
const fixtures = JSON.parse(output.split("\n").find((line) => line.startsWith("WIRE_FIXTURES:")).slice("WIRE_FIXTURES:".length));
assert.ok(fixtures.server.length > 20);

for (const fixture of fixtures.server) {
  const bytes = new Uint8Array(fixture.bytes);
  assert.deepEqual(decodeMessage(bytes.buffer), fixture.message);
  // Buffer/typed-array subviews must honor their byte offset and length.
  const padded = new Uint8Array(bytes.length + 13);
  padded.set(bytes, 7);
  assert.deepEqual(decodeMessage(padded.subarray(7, 7 + bytes.length)), fixture.message);
}
// Reordering independent packets and dropping a View must not require decoder
// history. Reliable updates retain the exact optional/absent field semantics.
for (const fixture of fixtures.server.toReversed().filter((_, i) => i % 3)) {
  assert.deepEqual(decodeMessage(new Uint8Array(fixture.bytes)), fixture.message);
}

const small = encodeMessage({ type: "Join", name: "名前 🚀", view_hz: undefined });
assert.deepEqual(decodeMessage(small), { type: "Join", name: "名前 🚀" });
for (let n = 0; n < small.length; n++) assert.throws(() => decodeMessage(small.subarray(0, n)));
const wrong = small.slice(); wrong[3] -= 1;
assert.throws(() => decodeMessage(wrong), /protocol/);
assert.throws(() => decodeMessage(JSON.stringify({ type: "View" })), /protocol/);
assert.throws(() => decodeMessage(new Uint8Array([...small, 0xc0])));
assert.throws(() => encodeMessage({ type: "Join", name: "x".repeat(MAX_CLIENT_FRAME) }), /large/);
for (const bad of [NaN, Infinity, -Infinity]) {
  assert.throws(() => encodeMessage({ type: "MoveShip", ship_id: "1", dest: { x: bad, y: 0 } }), /number/);
}
// A bad packet must not poison the reused decoder.
assert.deepEqual(decodeMessage(small), { type: "Join", name: "名前 🚀" });

const dir = mkdtempSync(join(tmpdir(), "stellar-wire-"));
try {
  const path = join(dir, "client-frames.json");
  writeFileSync(path, JSON.stringify(fixtures.client.map((message) => ({ message, bytes: [...encodeMessage(message)] }))));
  rustTest("wire::tests::javascript_encoded_orders_decode_in_rust", { STELLAR_WIRE_CLIENT_FIXTURES: path });
} finally {
  rmSync(dir, { recursive: true }); // only this test's freshly created temp dir
}

// Timings are informational, never flaky wall-time assertions. Include UTF-8
// decoding in the JSON baseline, because WebSocket binary data begins as bytes.
for (const kind of ["View", "BattleRecords"]) {
  const fixture = fixtures.server.find((f) => f.message.type === kind);
  const bytes = new Uint8Array(fixture.bytes);
  const jsonBytes = new TextEncoder().encode(JSON.stringify(fixture.message));
  const utf8 = new TextDecoder();
  for (let i = 0; i < 30; i++) { decodeMessage(bytes); JSON.parse(utf8.decode(jsonBytes)); }
  const runs = 200;
  let started = performance.now();
  for (let i = 0; i < runs; i++) decodeMessage(bytes);
  const binaryMs = (performance.now() - started) / runs;
  started = performance.now();
  for (let i = 0; i < runs; i++) JSON.parse(utf8.decode(jsonBytes));
  const jsonMs = (performance.now() - started) / runs;
  console.log(`${kind}: ${fixture.json_bytes} JSON bytes → ${bytes.length} binary bytes; decode ${binaryMs.toFixed(3)} ms binary / ${jsonMs.toFixed(3)} ms JSON`);
}
console.log(`PASS: ${fixtures.server.length} Rust→JS messages, ${fixtures.client.length} JS→Rust orders, framing/precision/omission/error tests.`);
