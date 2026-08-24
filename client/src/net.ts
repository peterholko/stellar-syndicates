// WebSocket connection to the authoritative server. Pure transport: it sends
// the player's intents and surfaces the per-player message stream. No game
// logic lives here.

import type { ClientMsg, ServerMsg } from "./protocol";

const RECONNECT_BASE_MS = 500;
const RECONNECT_MAX_MS = 30_000;
const RECONNECT_JITTER = 0.2;

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
  private reconnectTimer: number | null = null;
  private reconnectAttempt = 0;
  private stopped = false;
  readonly url: string;

  constructor(private handlers: NetHandlers) {
    this.url = resolveServerUrl();
    document.addEventListener("visibilitychange", this.handleVisibilityChange);
  }

  connect(): void {
    this.stopped = false;
    this.clearReconnect();
    this.open();
  }

  disconnect(): void {
    this.stopped = true;
    this.clearReconnect();
    document.removeEventListener("visibilitychange", this.handleVisibilityChange);
    const ws = this.ws;
    this.ws = null;
    if (ws && ws.readyState < WebSocket.CLOSING) ws.close();
  }

  private open(): void {
    if (this.stopped || (this.ws && this.ws.readyState < WebSocket.CLOSING)) return;
    const ws = new WebSocket(this.url);
    this.ws = ws;
    ws.onopen = () => {
      if (this.ws !== ws) return;
      this.reconnectAttempt = 0;
      this.handlers.onOpen();
    };
    ws.onmessage = (ev) => {
      if (this.ws !== ws) return;
      try {
        this.handlers.onMessage(JSON.parse(ev.data) as ServerMsg);
      } catch (e) {
        // Surface protocol violations instead of swallowing them — silent drops
        // make client/server contract bugs near-impossible to diagnose.
        console.warn("dropping unparseable server frame:", e, ev.data);
      }
    };
    ws.onclose = () => {
      if (this.ws !== ws) return;
      this.ws = null;
      this.handlers.onClose();
      this.scheduleReconnect();
    };
    ws.onerror = (e) => {
      if (this.ws !== ws) return;
      this.handlers.onError(e);
      // Browsers report connection failures through both error and close, but
      // only close owns the retry so one failed socket cannot schedule twice.
      try { ws.close(); } catch { /* close will follow or the next wake retries */ }
    };
  }

  private scheduleReconnect(): void {
    if (this.stopped || this.reconnectTimer !== null) return;
    const base = Math.min(RECONNECT_MAX_MS, RECONNECT_BASE_MS * 2 ** this.reconnectAttempt);
    const jitter = 1 - RECONNECT_JITTER + Math.random() * RECONNECT_JITTER * 2;
    const delay = Math.min(RECONNECT_MAX_MS, Math.round(base * jitter));
    this.reconnectAttempt += 1;
    this.reconnectTimer = window.setTimeout(() => {
      this.reconnectTimer = null;
      this.open();
    }, delay);
  }

  private clearReconnect(): void {
    if (this.reconnectTimer === null) return;
    window.clearTimeout(this.reconnectTimer);
    this.reconnectTimer = null;
  }

  private handleVisibilityChange = (): void => {
    if (document.visibilityState !== "visible" || this.stopped || this.connected) return;
    this.clearReconnect();
    this.open();
  };

  send(msg: ClientMsg): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg));
    }
  }

  get connected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }
}
