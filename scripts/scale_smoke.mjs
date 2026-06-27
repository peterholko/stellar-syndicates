// Scale smoke test: N players join one galaxy; each gets its own live delayed
// view and the authoritative loop keeps up (ticks advance for everyone). Run
// against a server started with MAX_PLAYERS=12.
//   node scripts/scale_smoke.mjs [N=12]

const URL = process.env.SERVER_WS || "ws://127.0.0.1:8080/ws";
const STATUS = URL.replace(/^ws/, "http").replace(/\/ws$/, "/status");
const N = Number(process.argv[2] || 12);
const fail = (m) => { console.error("FAIL:", m); process.exit(1); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function client(name) {
  const ws = new WebSocket(URL);
  const got = { welcome: null, ticks: [], errors: [] };
  ws.addEventListener("open", () => ws.send(JSON.stringify({ type: "Join", name })));
  ws.addEventListener("message", (ev) => {
    const m = JSON.parse(ev.data);
    if (m.type === "Welcome") got.welcome = m;
    else if (m.type === "View") got.ticks.push(m.tick);
    else if (m.type === "Error") got.errors.push(m.message);
  });
  return { ws, got };
}

const main = async () => {
  console.log(`connecting ${N} players to ${URL}`);
  const cs = Array.from({ length: N }, (_, i) => client(`Corp ${String.fromCharCode(65 + i)}`));
  await sleep(3000);

  // Every player got a Welcome with their own home, and live ticks.
  const homes = new Set();
  for (let i = 0; i < N; i++) {
    const g = cs[i].got;
    if (!g.welcome) fail(`player ${i} never got Welcome`);
    if (g.errors.length) fail(`player ${i} errors: ${g.errors.join(", ")}`);
    if (g.ticks.length < 3) fail(`player ${i} too few views (${g.ticks.length}) — loop not keeping up`);
    const inc = g.ticks.every((v, k) => k === 0 || v > g.ticks[k - 1]);
    if (!inc) fail(`player ${i} ticks not advancing`);
    homes.add(JSON.stringify(g.welcome.player_id));
  }
  if (homes.size !== N) fail(`expected ${N} distinct players, got ${homes.size}`);
  console.log(`  all ${N} players have distinct identities + live advancing views ✓`);

  // The session layer reports all N online.
  const online = (await (await fetch(STATUS)).json()).online_players;
  if (online !== N) fail(`/status online_players=${online}, expected ${N}`);
  console.log(`  /status reports ${online} online; loop tick = ${cs[0].got.ticks.at(-1)} (advancing) ✓`);

  // Throughput: in the ~3s window each player should have received many views
  // (the 10 Hz broadcast holds up — the loop isn't falling behind at N players).
  const minViews = Math.min(...cs.map((c) => c.got.ticks.length));
  if (minViews < 10) fail(`slowest player only got ${minViews} views in 3s — loop falling behind`);
  console.log(`  slowest player got ${minViews} views in ~3s (≈ ${Math.round(minViews / 3)} Hz) — loop keeps up ✓`);

  for (const c of cs) c.ws.close();
  console.log(`\nPASS — ${N} players in one galaxy, each with their own live delayed view; the loop keeps up.`);
  process.exit(0);
};

main().catch((e) => fail(e.stack || String(e)));
