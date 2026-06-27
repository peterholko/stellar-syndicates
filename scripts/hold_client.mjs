// Connect a player and hold the connection open for N seconds, so the galaxy
// has a second fleet during manual/visual testing. Usage:
//   node scripts/hold_client.mjs "Corp Name" 60
const URL = process.env.SERVER_WS || "ws://127.0.0.1:8080/ws";
const name = process.argv[2] || "Holder Co";
const secs = Number(process.argv[3] || 60);
const ws = new WebSocket(URL);
ws.addEventListener("open", () => ws.send(JSON.stringify({ type: "Join", name })));
ws.addEventListener("error", (e) => console.error("ws error", e.message || e));
console.log(`holding "${name}" for ${secs}s on ${URL}`);
setTimeout(() => { ws.close(); process.exit(0); }, secs * 1000);
