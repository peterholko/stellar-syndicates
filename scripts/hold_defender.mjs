import { gameSocket, encodeMessage, decodeMessage } from "./game-socket.mjs";

const URL = process.env.SERVER_WS || "ws://127.0.0.1:8090/ws";
const ws = gameSocket(URL);
ws.addEventListener("open", () => ws.send(encodeMessage({ type: "Join", name: process.env.NAME || "Convoy Corp" })));
ws.addEventListener("message", (ev) => { const m = decodeMessage(ev.data); if (m.type === "Welcome") console.log("defender online:", m.player_id); });
setInterval(() => { try { ws.send(encodeMessage({ type: "Ping" })); } catch {} }, 15000);
