// Bootstrap: wire the join screen → WebSocket → view state → HUD + Pixi render.

import { Net } from "./net";
import { Renderer } from "./render";
import { initialState, type LinkStatus, type ViewState } from "./state";
import { formatId } from "./protocol";

const state: ViewState = initialState();

// --- DOM handles -----------------------------------------------------------
const $ = (id: string) => document.getElementById(id)!;
const joinScreen = $("join");
const joinBtn = $("join-btn") as HTMLButtonElement;
const nameInput = $("name") as HTMLInputElement;
const joinErr = $("join-err");
const hud = $("hud");

function setHud(): void {
  $("hud-name").textContent = state.name || "—";
  $("hud-id").textContent = state.playerId !== null ? formatId(state.playerId) : "—";
  $("hud-tick").textContent = state.link === "online" ? state.tick.toLocaleString() : "—";
  $("hud-time").textContent =
    state.link === "online" ? `${state.simTime.toFixed(1)}s` : "—";
  $("hud-online").textContent = state.link === "online" ? String(state.playersOnline) : "—";
  $("hud-ships").textContent = state.link === "online" ? String(state.ships.length) : "—";
  const link = $("hud-link");
  const labels: Record<LinkStatus, string> = {
    connecting: "connecting…",
    online: "● online",
    offline: "✕ disconnected",
  };
  link.textContent = labels[state.link];
  link.className = "v " + (state.link === "online" ? "accent" : "warn");
}

// --- Renderer --------------------------------------------------------------
const renderer = new Renderer();
let rendererReady = false;

async function startRenderer(): Promise<void> {
  if (rendererReady) return;
  await renderer.init($("app"));
  rendererReady = true;
  const frame = () => {
    renderer.update(state);
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
}

// --- Networking ------------------------------------------------------------
let net: Net | null = null;

function join(): void {
  const name = nameInput.value.trim();
  if (!name) {
    joinErr.textContent = "Enter a corporation name.";
    return;
  }
  joinErr.textContent = "";
  joinBtn.disabled = true;
  state.name = name;
  state.link = "connecting";

  net = new Net({
    onOpen: () => {
      net!.send({ type: "Join", name });
    },
    onMessage: (msg) => {
      switch (msg.type) {
        case "Welcome":
          state.playerId = msg.player_id;
          state.name = msg.name;
          state.tickHz = msg.tick_hz;
          state.tick = msg.tick;
          state.simTime = msg.sim_time;
          state.galaxy = msg.galaxy;
          state.link = "online";
          // Swap from the join screen to the galaxy view.
          joinScreen.style.display = "none";
          hud.style.display = "flex";
          void startRenderer();
          break;
        case "View":
          state.tick = msg.tick;
          state.simTime = msg.sim_time;
          state.playersOnline = msg.players_online;
          state.anchors = msg.anchors;
          state.ships = msg.ships;
          state.lastViewWallMs = performance.now();
          state.link = "online";
          break;
        case "Error":
          joinErr.textContent = msg.message;
          break;
      }
      setHud();
    },
    onClose: () => {
      state.link = "offline";
      joinBtn.disabled = false;
      setHud();
    },
    onError: () => {
      state.link = "offline";
      joinErr.textContent = `Could not reach server at ${net?.url ?? ""}.`;
      joinBtn.disabled = false;
      setHud();
    },
  });
  net.connect();
}

joinBtn.addEventListener("click", join);
nameInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") join();
});
nameInput.focus();
setHud();
