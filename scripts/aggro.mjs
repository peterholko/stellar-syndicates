// Connect as a player, wait for light to reveal a rival convoy (broadcast),
// then commit this player's raider to intercept it and hold the connection.
// Used to demo the "detected raider" threat contact from the victim's side.
//   node scripts/aggro.mjs "Bravo Mining" 150
const URL = process.env.SERVER_WS || "ws://127.0.0.1:8080/ws";
const name = process.argv[2] || "Bravo Mining";
const holdS = Number(process.argv[3] || 150);
const ws = new WebSocket(URL);
let last = null;
ws.addEventListener("open", () => ws.send(JSON.stringify({ type: "Join", name })));
ws.addEventListener("message", (ev) => {
  const m = JSON.parse(ev.data);
  if (m.type === "View") last = m;
});
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
(async () => {
  console.log(`[${name}] joined; waiting for light to reveal a rival convoy…`);
  await sleep(33000);
  const raider = last && last.ghosts.find((g) => g.own && g.kind === "raider");
  const target = last && last.ghosts.find((g) => !g.own && g.kind === "convoy");
  if (raider && target) {
    ws.send(JSON.stringify({ type: "CommitRaid", raider_id: raider.id, target_id: target.id }));
    console.log(`[${name}] raider ${raider.id} committed to hunt rival convoy ${target.id}`);
  } else {
    console.log(`[${name}] could not find raider/target (raider=${!!raider}, target=${!!target})`);
  }
  await sleep(holdS * 1000);
  ws.close();
  process.exit(0);
})();
