// §client-updates (docs/client-updates.md): an open tab learns of a newer
// client build, reloads itself out of view, and otherwise offers the reload
// (required once the protocol changed). Against a fake page; no game socket.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
const exports = {};
vm.runInNewContext(ts.transpileModule(read("src/client-update.ts"), { compilerOptions: {
  target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
} }).outputText, { exports, require: (name) => name === "./wire.mjs" ? { PROTOCOL_VERSION: 91 } : {} });
const { updateNeed, clientUpdates, MIN_CHECK_GAP_MS } = exports;

// What to do about the newest build.
assert.equal(updateNeed("abc", { build: "abc", protocol: 91 }, 91), "none", "the same build");
assert.equal(updateNeed("dev", { build: "abc", protocol: 91 }, 91), "none", "the dev server never asks");
assert.equal(updateNeed("abc", null, 91), "none", "no answer");
assert.equal(updateNeed("abc", { build: "" }, 91), "none", "no stamp");
assert.equal(updateNeed("abc", { build: 7 }, 91), "none", "not a stamp");
assert.equal(updateNeed("abc", { build: "def", protocol: 91 }, 91), "offer", "a newer client, same protocol");
assert.equal(updateNeed("abc", { build: "def" }, 91), "offer", "no protocol named: an offer");
assert.equal(updateNeed("abc", { build: "def", protocol: 92 }, 91), "required", "another protocol: this tab can't play on");

// A fake page around the checks.
function page({ build = "abc", latest = { build: "def", protocol: 91 }, hidden = false } = {}) {
  const log = { fetched: 0, reloads: 0, shown: [], remembered: null, clock: 1_000_000 };
  const env = {
    build,
    latest: async () => {
      log.fetched += 1;
      if (latest instanceof Error) throw latest;
      return latest;
    },
    hidden: () => log.hidden,
    reload: () => { log.reloads += 1; },
    now: () => log.clock,
    remembered: () => log.remembered,
    remember: (b) => { log.remembered = b; },
    show: (need, reload) => log.shown.push({ need, reload }),
  };
  log.hidden = hidden;
  return { log, updates: clientUpdates(env) };
}

{
  // In view: the pill, no reload; its button reloads into the new build.
  const { log, updates } = page();
  await updates.check();
  assert.equal(log.reloads, 0, "never pulled out from under the player");
  assert.deepEqual(log.shown.map((s) => s.need), ["offer"]);
  log.shown[0].reload();
  assert.deepEqual([log.reloads, log.remembered], [1, "def"], "Reload remembers the build it loads");
}
{
  // Found in view, then the tab goes out of view: it loads then.
  const { log, updates } = page();
  await updates.check();
  log.hidden = true;
  updates.hidden();
  assert.equal(log.reloads, 1, "out of view nothing is lost");
  updates.hidden();
  assert.equal(log.reloads, 1, "once per build");
}
{
  // Out of view at the check: it reloads at once, without the pill.
  const { log, updates } = page({ hidden: true });
  await updates.check();
  assert.deepEqual([log.reloads, log.shown.length, log.remembered], [1, 0, "def"]);
}
{
  // A page that still serves the old stamp after that reload never loops:
  // the pill instead.
  const { log, updates } = page({ hidden: true });
  log.remembered = "def";
  await updates.check();
  assert.deepEqual([log.reloads, log.shown.map((s) => s.need)], [0, ["offer"]]);
}
{
  // Another protocol: required.
  const { log, updates } = page({ latest: { build: "def", protocol: 92 } });
  await updates.check();
  assert.deepEqual(log.shown.map((s) => s.need), ["required"]);
}
{
  // A burst of focus changes asks once; the socket's protocol close forces it.
  const { log, updates } = page({ latest: { build: "abc", protocol: 91 } });
  await updates.check();
  await updates.check();
  assert.equal(log.fetched, 1, "within the gap: one request");
  log.clock += MIN_CHECK_GAP_MS;
  await updates.check();
  assert.equal(log.fetched, 2, "after the gap: another");
  await updates.check(true);
  assert.equal(log.fetched, 3, "forced");
  assert.deepEqual([log.reloads, log.shown.length], [0, 0], "the same build: nothing");
}
{
  // Offline, or the dev server: nothing.
  const offline = page({ latest: new Error("offline") });
  await offline.updates.check();
  assert.deepEqual([offline.log.reloads, offline.log.shown.length], [0, 0]);
  const dev = page({ build: "dev" });
  await dev.updates.check(true);
  assert.equal(dev.log.fetched, 0, "the dev server never asks");
}

// Wiring: the stamp, the checks, the socket's close and the cache rule.
assert.match(read("index.html"), /<meta name="ss-build" content="dev" \/>/, "the stamp's placeholder");
assert.match(read("vite.config.ts"), /plugins: \[buildStamp\(\)\]/);
assert.match(read("vite.config.ts"), /writeFileSync\(resolve\(outDir, "version\.json"\), `\$\{JSON\.stringify\(\{ build, protocol: PROTOCOL_VERSION \}\)\}\\n`\)/);
assert.match(read("public/_headers"), /\/version\.json\n  Cache-Control: no-store/, "never cached");
const boot = read("src/boot.ts");
assert.match(boot, /const checkForUpdate = installClientUpdates\(\);/);
assert.match(boot, /onProtocolChanged: \(\) => void checkForUpdate\(true\),/);
assert.match(read("src/net.ts"), /if \(event\.code === PROTOCOL_CLOSE_CODE\) \{[\s\S]*?this\.handlers\.onProtocolChanged\?\.\(\);\n\s*\}/,
  "the server's protocol close checks at once");
assert.match(read("src/client-update.ts"), /fetch\("\/version\.json", \{ cache: "no-store" \}\)/);
// The server names an old tab's own protocol so its close reaches it.
assert.match(readFileSync(new URL("../../crates/server/src/ws.rs", import.meta.url), "utf8"),
  /ws\.protocols\(\[accepted_protocol\(&headers\)\]\)/);
// A build made with the stamp agrees with itself.
if (existsSync(new URL("../dist/version.json", import.meta.url))) {
  const served = JSON.parse(read("dist/version.json"));
  assert.match(served.build, /^[0-9a-f]{12}$/);
  assert.equal(served.protocol, Number(/export const PROTOCOL_VERSION = (\d+);/.exec(read("src/wire.mjs"))[1]));
  assert.match(read("dist/index.html"), new RegExp(`<meta name="ss-build" content="${served.build}" />`), "the page's stamp is the served one");
}
console.log("PASS: client updates: offer in view, reload out of view (once per build), required on a new protocol, burst guard, the socket's close, the stamp and its cache rule.");
