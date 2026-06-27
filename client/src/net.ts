// WebSocket connection to the authoritative server. Pure transport: it sends
// the player's intents and surfaces the per-player message stream. No game
// logic lives here.

import type { ClientMsg, ServerMsg } from "./protocol";

export interface NetHandlers {
  onOpen: () => void;
  onMessage: (msg: ServerMsg) => void;
  onClose: () => void;
  onError: (e: Event) => void;
}

// Resolve the server WebSocket URL. Works whether the page is served by Vite
// (dev, port 5173) or by the Rust server itself (prod, port 8080). Override
// with `?server=ws://host:port/ws`.
function resolveServerUrl(): string {
  const override = new URLSearchParams(location.search).get("server");
  if (override) return override;
  const proto = location.protocol === "https:" ? "wss" : "ws";
  // In dev the page is on 5173 but the game server is on 8080; if we're already
  // served from the game server, location.port is 8080 and this still resolves.
  const host = location.hostname;
  const port = location.port === "5173" || location.port === "" ? "8080" : location.port;
  return `${proto}://${host}:${port}/ws`;
}

export class Net {
  private ws: WebSocket | null = null;
  readonly url: string;

  constructor(private handlers: NetHandlers) {
    this.url = resolveServerUrl();
  }

  connect(): void {
    const ws = new WebSocket(this.url);
    this.ws = ws;
    ws.onopen = () => this.handlers.onOpen();
    ws.onmessage = (ev) => {
      try {
        this.handlers.onMessage(JSON.parse(ev.data) as ServerMsg);
      } catch (e) {
        // Surface protocol violations instead of swallowing them — silent drops
        // make client/server contract bugs near-impossible to diagnose.
        console.warn("dropping unparseable server frame:", e, ev.data);
      }
    };
    ws.onclose = () => this.handlers.onClose();
    ws.onerror = (e) => this.handlers.onError(e);
  }

  send(msg: ClientMsg): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg));
    }
  }

  get connected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }
}
