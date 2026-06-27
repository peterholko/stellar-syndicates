// Join as an attacker, wait for a rival convoy's (light-delayed) ghost to appear,
// then commit a raid on it. Used to drive the convoy-destruction sync test.
const URL = process.env.SERVER_WS || "ws://127.0.0.1:8090/ws";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const ws = new WebSocket(URL);
let welcome = null; const views = []; const reports = [];
ws.addEventListener("open", () => ws.send(JSON.stringify({ type: "Join", name: "Raider Co" })));
ws.addEventListener("message", (ev) => {
  const m = JSON.parse(ev.data);
  if (m.type === "Welcome") welcome = m;
  else if (m.type === "View") views.push(m);
  else if (m.type === "Report") reports.push(m.report);
});
const main = async () => {
  await sleep(2000);
  console.log("attacker joined; waiting for a rival convoy's light to arrive...");
  let convoyId = null;
  const deadline = Date.now() + 130000;
  while (Date.now() < deadline && reports.length === 0) {
    const v = views.at(-1);
    const raider = v && v.ghosts.find((g) => g.own && g.kind === "raider");
    const convoy = v && v.ghosts.find((g) => !g.own && g.kind === "convoy");
    if (convoy && !convoyId) { convoyId = convoy.id; console.log(`saw rival convoy ${convoyId} (age ${convoy.age.toFixed(1)}s) — committing raid`); }
    if (raider && convoyId) ws.send(JSON.stringify({ type: "CommitRaid", raider_id: raider.id, target_id: convoyId }));
    await sleep(1200);
  }
  const rep = reports[0];
  console.log("RESULT:", rep ? `${rep.outcome} target=${rep.target_ship} age=${rep.age.toFixed(1)}s` : "TIMEOUT (no rival convoy seen / no contact)");
  process.exit(0);
};
main();
