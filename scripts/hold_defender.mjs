const URL = process.env.SERVER_WS || "ws://127.0.0.1:8090/ws";
const ws = new WebSocket(URL);
ws.addEventListener("open", () => ws.send(JSON.stringify({ type: "Join", name: process.env.NAME || "Convoy Corp" })));
ws.addEventListener("message", (ev) => { const m = JSON.parse(ev.data); if (m.type === "Welcome") console.log("defender online:", m.player_id); });
setInterval(() => { try { ws.send(JSON.stringify({ type: "Ping" })); } catch {} }, 15000);
