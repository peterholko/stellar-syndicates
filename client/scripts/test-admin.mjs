import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

function compile(path) {
  const exports = {};
  const source = readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
  vm.runInNewContext(ts.transpileModule(source, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, Date, Math, Number, JSON, String });
  return exports;
}

const fmt = compile("admin/format.ts");

// Durations read at a glance: seconds, minutes, hours with padded minutes, days.
assert.equal(fmt.duration(45), "45s");
assert.equal(fmt.duration(12 * 60 + 20), "12m");
assert.equal(fmt.duration(3 * 3600 + 5 * 60), "3h 05m");
assert.equal(fmt.duration(2 * 86_400 + 4 * 3600 + 59), "2d 4h");
assert.equal(fmt.duration(-1), "—");
assert.equal(fmt.duration(Number.NaN), "—");

// "Ago" uses the caller's clock, never its own.
const now = Date.parse("2026-10-06T12:00:00Z");
assert.equal(fmt.ago(null, now), "never");
assert.equal(fmt.ago("2026-10-06T11:59:30Z", now), "just now");
assert.equal(fmt.ago("2026-10-06T11:55:00Z", now), "5m ago");
assert.equal(fmt.ago("2026-10-06T09:00:00Z", now), "3h ago");
assert.equal(fmt.ago("2026-10-02T12:00:00Z", now), "4d ago");
assert.match(fmt.ago("2026-08-01T12:00:00Z", now), /2026/, "a month back reads as a date");
assert.equal(fmt.ago("not a time", now), "—");

// Credits match the top bar until they need compacting; population is millions.
assert.equal(fmt.credits(2500), (2500).toLocaleString());
assert.equal(fmt.credits(1_234_567), "1.23M");
assert.equal(fmt.credits(-2_500_000_000), "-2.50B");
assert.equal(fmt.credits(null), "—");
assert.equal(fmt.population(1.5), "1.50m");
assert.equal(fmt.population(0.0042), "4.2k");
assert.equal(fmt.population(0.0005), "500");

// User agents shrink to browser and platform.
assert.equal(fmt.browser("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36"), "Chrome 141 · macOS");
assert.equal(fmt.browser("Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1"), "Safari 18 · iPhone");
assert.equal(fmt.browser("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36 Edg/140.0.0.0"), "Edge 140 · Windows");
assert.equal(fmt.browser("Mozilla/5.0 (X11; Linux x86_64; rv:143.0) Gecko/20100101 Firefox/143.0"), "Firefox 143 · Linux");
assert.equal(fmt.browser("Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Mobile Safari/537.36"), "Chrome 141 · Android");
assert.equal(fmt.browser("curl/8.7.1"), "curl/8.7.1");
assert.equal(fmt.browser(null), "—");

// Device ids shorten; recorded commands become one bounded line.
assert.equal(fmt.device("0123456789abcdef0123456789abcdef"), "01234567");
assert.equal(fmt.device(null), "—");
assert.equal(fmt.detail({ ship_id: "7" }), '{"ship_id":"7"}');
assert.equal(fmt.detail(null), "");
const long = fmt.detail({ notes: "x".repeat(400) }, 40);
assert.equal(long.length, 40);
assert.ok(long.endsWith("…"));
assert.equal(fmt.commandName("MoveShip"), "Move ship");
assert.equal(fmt.commandName("SetFleetPosture"), "Set fleet posture");
assert.equal(fmt.commandName("Withdraw"), "Withdraw");

// Shares read as whole percentages; tiny but real shares are not rounded away.
assert.equal(fmt.percent(0.857), "86%");
assert.equal(fmt.percent(0.003), "<1%");
assert.equal(fmt.percent(0), "0%");
assert.equal(fmt.percent(null), "—");

// The tutorial order matches the server's: arrival, then tutorial::Step::ALL.
const { STAGE_KEYS, tutorialRank } = compile("admin/tutorial.ts");
const rust = readFileSync(new URL("../../crates/server/src/tutorial.rs", import.meta.url), "utf8");
const variants = rust.match(/pub const ALL: \[Step; \d+\] = \[([^\]]*)\]/)[1].match(/Step::(\w+)/g).map(v => v.slice(6));
const keyArms = rust.slice(rust.indexOf("pub fn key(self)"), rust.indexOf("pub fn from_key"));
const keyOf = Object.fromEntries([...keyArms.matchAll(/Step::(\w+) => "(\w+)"/g)].map(m => [m[1], m[2]]));
assert.deepEqual([...STAGE_KEYS], ["signed_up", "entered", ...variants.map(v => keyOf[v])]);
assert.equal(tutorialRank(null), -1);
assert.equal(tutorialRank({ next: "signed_up" }), 0);
assert.equal(tutorialRank({ next: "privateer" }), 5);
assert.equal(tutorialRank({ next: null }), STAGE_KEYS.length, "a fulfilled charter sorts past every step");

// Every player-supplied string reaches the page as text, never as markup.
const page = readFileSync(new URL("../src/admin/main.ts", import.meta.url), "utf8");
assert.doesNotMatch(page, /innerHTML|outerHTML|insertAdjacentHTML/, "the dashboard never parses strings as HTML");
assert.match(page, /"X-Stellar-Client": "1"/, "admin reads carry the same-origin marker");

console.log("admin dashboard ok");

// --serve: the real page against canned API answers. Never contacts a game
// server or an account database. /__mode/<code> switches to an error answer.
if (process.argv.includes("--serve")) {
  const { createServer } = await import("node:http");
  const { resolve, sep } = await import("node:path");
  const { fileURLToPath } = await import("node:url");
  const { build } = await import("vite");
  const clientRoot = fileURLToPath(new URL("../", import.meta.url));
  const result = await build({ root: clientRoot, configFile: false, publicDir: false, logLevel: "error",
    build: { write: false, minify: false, rollupOptions: { input: resolve(clientRoot, "admin.html") } } });
  const output = (Array.isArray(result) ? result[0] : result).output;
  const files = new Map(output.map(file => [`/${file.fileName}`, file.type === "chunk" ? file.code : file.source]));

  const t = Date.now();
  const at = seconds => new Date(t - seconds * 1000).toISOString();
  let seed = 7;
  const rand = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
  const agents = [
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36",
    "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36 Edg/140.0.0.0",
    "Mozilla/5.0 (X11; Linux x86_64; rv:143.0) Gecko/20100101 Firefox/143.0",
  ];
  const names = ["Aurora Freight", "Kestrel Mining", "Vesper Combine", "Halcyon Drift", "Obsidian Works", "Meridian Trust",
    "Venture 3F9A2C11D0B4", "Corvid Salvage", "Lumen Syndicate", "Tidewater Holdings", "Venture 81C0E5AA9F27", "Ashfall Ventures",
    "Venture 5D02BB7E1C93", "Venture A7713F0C42E8"];
  const stageLabels = {
    signed_up: ["Sign up", "Arrival"], entered: ["Enter the game", "Arrival"], founded: ["Found a colony", "Founding"],
    mine: ["Bring the mine online", "Landfall"], shipment: ["Send your first shipment", "Landfall"],
    privateer: ["Defeat the Rogue Privateer", "Landfall"], shipyard: ["Build a Shipyard", "Industry"],
    research: ["Start research", "Industry"], survey: ["Chart a nearby system", "Industry"],
    barge: ["Build a Mining Barge", "Reach"], belt_mining: ["Mine the belt", "Reach"], haul: ["Bring the ore home", "Reach"],
  };
  // The stage each sample player is on (12 = fulfilled): a cluster stuck on
  // the privateer fight, two who never entered, one who never founded.
  const nextStage = [9, 12, 5, 6, 12, 8, 3, 5, 10, 5, 2, 4, 1, 1];
  const operatorIndex = 1;
  const stageTimes = (registered, index) => STAGE_KEYS.map((_, k) => k < index ? new Date(Date.parse(registered) + k * (12 + k * 3) * 60_000).toISOString() : null);
  const players = names.map((corporation, i) => {
    const guest = corporation.startsWith("Venture");
    const online = [0, 2, 5].includes(i);
    const device = (i === 9 ? 3 : i).toString(16).padStart(2, "0").repeat(16);
    const sessions = nextStage[i] < 2 ? 0 : 1 + Math.floor(rand() * 40);
    const founded = nextStage[i] > 2;
    const registered = at(3600 * (2 + i * 37));
    const lastSeen = online ? at(0) : nextStage[i] < 2 ? null : at(60 * (5 + i * i * 40));
    const times = stageTimes(registered, nextStage[i]);
    const next = STAGE_KEYS[nextStage[i]] ?? null;
    const active = online || t - Math.max(Date.parse(lastSeen ?? registered), Date.parse(registered)) <= 24 * 3_600_000;
    return {
      account_id: `00000000-0000-4000-8000-${String(i + 1).padStart(12, "0")}`,
      login: guest ? null : `${corporation.split(" ")[0].toLowerCase()}@example.com`,
      corporation, guest, disabled: i === 11,
      registered_at: registered, last_login_at: at(3600 * (1 + i * 3)), logins: 1 + Math.floor(rand() * 9),
      last_ip: `203.0.113.${10 + (i === 9 ? 3 : i)}`, last_device: device, last_user_agent: agents[i % agents.length],
      sessions, online_seconds: sessions * (900 + rand() * 7200), active_seconds: sessions * 300 * Math.ceil(rand() * 6),
      actions: Math.floor(sessions * rand() * 120), last_seen_at: lastSeen,
      online: online ? { since: at(60 * (7 + i * 11)), ip: `203.0.113.${10 + i}`, device_id: device, actions: 12 + i, last_action_at: at(30) } : null,
      devices: 1 + (i % 3 === 0 ? 1 : 0), shared_accounts: i === 3 || i === 9 ? 1 : 0,
      game: founded ? { name: corporation, bot: false, home: ["Veles", "Freya", "Kharon", "Tessaly"][i % 4],
        systems: 1 + Math.floor(rand() * 6), worlds: 1 + Math.floor(rand() * 9), population: rand() * 4,
        fleets: 1 + Math.floor(rand() * 8), ships: 2 + Math.floor(rand() * 60), credits: rand() * 2_400_000, valuation: rand() * 9_000_000 } : null,
      tutorial: { next, next_label: next ? stageLabels[next][0] : null, done: Math.max(0, Math.min(nextStage[i], 12) - 2), total: 10,
        since: next ? times.filter(Boolean).at(-1) : null },
      stageTimes: times,
      operator: i === operatorIndex, in_funnel: i !== operatorIndex, active,
    };
  });
  const funnel = () => {
    const members = players.filter(p => p.in_funnel);
    return {
      galaxy_id: "fixture-galaxy", galaxy_started_at: at(86_400 * 30), players: members.length, quiet_after_hours: 24,
      stages: STAGE_KEYS.map((key, k) => {
        const waits = members.filter(p => k > 0 && p.stageTimes[k] && p.stageTimes[k - 1])
          .map(p => (Date.parse(p.stageTimes[k]) - Date.parse(p.stageTimes[k - 1])) / 1000).sort((a, b) => a - b);
        const median = waits.length ? (waits.length % 2 ? waits[(waits.length - 1) / 2] : (waits[waits.length / 2 - 1] + waits[waits.length / 2]) / 2) : null;
        const waiting = active => members.filter(p => p.tutorial.next === key && p.active === active).length;
        return { key, label: stageLabels[key][0], chapter: stageLabels[key][1], reached: members.filter(p => p.stageTimes[k]).length,
          median_seconds: median, waiting_active: waiting(true), waiting_quiet: waiting(false) };
      }),
    };
  };
  const wire = player => { const { stageTimes, ...rest } = player; return rest; };
  const overview = () => ({
    generated_at: at(0),
    server: { started_at: at(3600 * 5 + 120), online_players: 3, connections: 3, bot_players: 2, bot_commands: 4412,
      corporations: 13, galaxy_systems: 412, tick: 1_284_220, sim_time: 128_422, holdings: true },
    totals: { accounts: players.length, registered: players.filter(p => !p.guest).length, guests: players.filter(p => p.guest).length,
      founded: players.filter(p => p.game).length, online: 3, new_24h: 1, active_24h: 6, active_7d: 10 },
    funnel: funnel(),
    players: players.map(wire),
    unlinked: [
      { name: "Bot Corporation 1", bot: true, home: "Nyx", systems: 4, worlds: 6, population: 2.4, fleets: 7, ships: 31, credits: 820_000, valuation: 4_100_000 },
      { name: "Bot Corporation 2", bot: true, home: "Orrin", systems: 2, worlds: 3, population: 0.9, fleets: 3, ships: 12, credits: 210_000, valuation: 1_300_000 },
    ],
  });
  const kinds = ["MoveShip", "BuildShip", "SetAssignment", "DevelopSystem", "MarketSell", "SetStandingOrder", "JumpShip", "HaulToSystem", "SurveySystem", "SetFleetPosture"];
  const detailFor = player => ({
    generated_at: at(0),
    player: { ...wire(player), tutorial: { ...player.tutorial,
      steps: STAGE_KEYS.map((key, k) => ({ key, label: stageLabels[key][0], chapter: stageLabels[key][1], at: player.stageTimes[k] })) } },
    logins: Array.from({ length: Math.min(player.logins, 12) }, (_, i) => ({ kind: i === player.logins - 1 ? (player.guest ? "guest" : "register") : "login",
      at: at(86_400 * i * 1.7 + 3600), ip: `203.0.113.${10 + (i % 3)}`, user_agent: agents[i % 2], language: "en-US,en;q=0.9", device_id: player.last_device })),
    sessions: Array.from({ length: Math.min(player.sessions, 20) }, (_, i) => ({ connected_at: at(3600 * (i * 9 + 1)),
      disconnected_at: i === 0 && player.online ? null : at(3600 * (i * 9 + 1) - 1800 - i * 400), last_seen_at: at(3600 * i * 9),
      ip: `203.0.113.${10 + (i % 3)}`, user_agent: agents[i % 2], device_id: player.last_device, actions: Math.floor(rand() * 90), active_seconds: 300 * Math.floor(rand() * 7) })),
    days: Array.from({ length: 30 }, (_, i) => ({ day: new Date(t - (29 - i) * 86_400_000).toISOString().slice(0, 10), sessions: 1 + (i % 3),
      online_seconds: rand() < 0.3 ? 0 : rand() * 14_400, active_seconds: rand() * 3600, actions: Math.floor(rand() * 200) })).filter(day => day.online_seconds > 0),
    commands: kinds.map((kind, i) => ({ kind, count: Math.floor(400 / (i + 1)), last_at: at(60 * (i + 1)) })),
    recent: Array.from({ length: 40 }, (_, i) => ({ at: at(45 * i), kind: kinds[i % kinds.length],
      detail: i % 4 === 0 ? { ship_id: String(1200 + i), target: { x: 1834.5 + i, y: -220.25 } } : i % 4 === 1 ? { system_id: "88", body_id: 2, structure: "mining_complex", workers: 3 } : { fleet_id: String(900 + i) } })),
    shared: player.shared_accounts ? [
      { account_id: players[player.corporation === "Halcyon Drift" ? 9 : 3].account_id, login: "tidewater@example.com", corporation: players[player.corporation === "Halcyon Drift" ? 9 : 3].corporation, via: "device", value: player.last_device },
      { account_id: players[9].account_id, login: "tidewater@example.com", corporation: "Tidewater Holdings", via: "ip", value: player.last_ip },
    ] : [],
  });

  let mode = null;
  const publicRoot = resolve(clientRoot, "public");
  const json = (response, status, body) => { response.writeHead(status, { "Content-Type": "application/json", "Cache-Control": "no-store" }); response.end(JSON.stringify(body)); };
  const server = createServer((request, response) => {
    const url = new URL(request.url, "http://localhost");
    if (url.pathname.startsWith("/__mode/")) { mode = url.pathname.slice(8) || null; if (mode === "ok") mode = null; response.writeHead(302, { Location: "/" }).end(); return; }
    if (url.pathname.startsWith("/api/admin/")) {
      if (request.headers["x-stellar-client"] !== "1") return json(response, 401, { code: "sign_in", error: "Sign in to the game in this browser first." });
      if (mode) return json(response, { sign_in: 401, not_operator: 403, off: 404 }[mode] ?? 503, { code: mode, error: {
        sign_in: "Sign in to the game in this browser first.", not_operator: "This account is not an operator. Add its login to ADMIN_LOGINS.",
        off: "The admin dashboard is off on this server. Set ADMIN_LOGINS to enable it." }[mode] ?? "The account database is unavailable. Try again shortly." });
      if (url.pathname === "/api/admin/overview") return json(response, 200, overview());
      const player = players.find(p => url.pathname === `/api/admin/players/${p.account_id}`);
      return player ? json(response, 200, detailFor(player)) : json(response, 404, { code: "no_account", error: "No such account." });
    }
    const path = url.pathname === "/" ? "/admin.html" : url.pathname;
    if (files.has(path)) {
      response.setHeader("Content-Type", path.endsWith(".html") ? "text/html; charset=utf-8" : path.endsWith(".css") ? "text/css" : "text/javascript");
      response.end(files.get(path));
      return;
    }
    if (url.pathname.startsWith("/art/")) {
      const file = resolve(publicRoot, `.${decodeURIComponent(url.pathname)}`);
      if (!file.startsWith(publicRoot + sep)) { response.writeHead(403).end(); return; }
      try { response.end(readFileSync(file)); } catch { response.writeHead(404).end(); }
      return;
    }
    response.writeHead(404).end();
  });
  const port = Number(process.argv.find(arg => arg.startsWith("--port="))?.slice("--port=".length)) || 0;
  server.listen(port, "127.0.0.1", () => console.log(`Admin fixture: http://127.0.0.1:${server.address().port}/`));
}
