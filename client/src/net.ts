// WebSocket connection to the authoritative server. Pure transport: it sends
// the player's intents and surfaces the per-player message stream. No game
// logic lives here.

import type { ClientMsg, ServerMsg } from "./protocol";
import { decodeMessage, encodeMessage, PROTOCOL_CLOSE_CODE, SUBPROTOCOL } from "./wire.mjs";

const RECONNECT_BASE_MS = 500;
const RECONNECT_MAX_MS = 30_000;
const RECONNECT_JITTER = 0.2;
const SESSION_REPLACED_CLOSE_CODE = 4001;
const AUTH_REQUIRED_CLOSE_CODE = 4003;

export type ViewHz = 5 | 10;

export interface NetHandlers {
  beforeConnect?: () => Promise<boolean>;
  onOpen: () => void;
  onMessage: (msg: ServerMsg) => void;
  onClose: () => void;
  onSessionReplaced: () => void;
  onError: (e: Event) => void;
}

// Resolve the server WebSocket URL. Works whether the page is served by Vite
// (dev, through Vite's proxy) or by Rust. Account cookies are same-origin;
// ?server= must never redirect a signed-in client to an unrelated host.
function resolveServerUrl(): string {
  const override = new URLSearchParams(location.search).get("server");
  const proto = location.protocol === "https:" ? "wss" : "ws";
  const host = location.hostname;
  const port = location.port;
  const authority = port ? `${host}:${port}` : host;
  const url = `${proto}://${authority}/ws`;
  return override === url ? override : url;
}

export class Net {
  private ws: WebSocket | null = null;
  private reconnectTimer: number | null = null;
  private reconnectAttempt = 0;
  private stopped = true;
  private opening = false;
  private generation = 0;
  private viewHz: ViewHz = 10;
  readonly url: string;

  constructor(private handlers: NetHandlers) {
    this.url = resolveServerUrl();
    document.addEventListener("visibilitychange", this.handleVisibilityChange);
  }

  connect(): void {
    document.addEventListener("visibilitychange", this.handleVisibilityChange);
    this.stopped = false;
    this.clearReconnect();
    this.open();
  }

  disconnect(): void {
    this.stopped = true;
    this.generation += 1;
    this.opening = false;
    this.clearReconnect();
    document.removeEventListener("visibilitychange", this.handleVisibilityChange);
    const ws = this.ws;
    this.ws = null;
    if (ws && ws.readyState < WebSocket.CLOSING) ws.close();
  }

  private open(): void {
    if (this.stopped || this.opening || (this.ws && this.ws.readyState < WebSocket.CLOSING)) return;
    if (!this.handlers.beforeConnect) { this.openSocket(); return; }
    this.opening = true;
    const generation = this.generation;
    void this.handlers.beforeConnect().then(allowed => {
      if (generation !== this.generation || this.stopped) return;
      if (allowed) this.openSocket();
      else {
        this.stopped = true;
        this.clearReconnect();
        this.handlers.onSessionReplaced();
      }
    }).catch(() => {
      if (generation !== this.generation || this.stopped) return;
      this.handlers.onError(new Event("error"));
      this.scheduleReconnect();
    }).finally(() => {
      if (generation === this.generation) this.opening = false;
    });
  }

  private openSocket(): void {
    if (this.stopped || (this.ws && this.ws.readyState < WebSocket.CLOSING)) return;
    const ws = new WebSocket(this.url, SUBPROTOCOL);
    // Synchronous decoding preserves message order; Blob.arrayBuffer() per
    // frame would introduce an asynchronous race between Views and increments.
    ws.binaryType = "arraybuffer";
    this.ws = ws;
    ws.onopen = () => {
      if (this.ws !== ws) return;
      this.reconnectAttempt = 0;
      this.handlers.onOpen();
    };
    ws.onmessage = (ev) => {
      if (this.ws !== ws || this.stopped) return;
      let message: ServerMsg;
      try {
        message = decodeMessage(ev.data);
      } catch (e) {
        // A skipped reliable battle/order increment cannot be repaired by the
        // next View. Stop this incompatible stream visibly, never quietly drop.
        console.warn("invalid binary server frame:", e);
        this.stopped = true;
        this.clearReconnect();
        this.handlers.onMessage({ type: "Error", message: "Network protocol error. Reload the game." });
        ws.close(PROTOCOL_CLOSE_CODE, "invalid binary server frame");
        return;
      }
      this.handlers.onMessage(message);
    };
    ws.onclose = (event) => {
      if (this.ws !== ws) return;
      this.ws = null;
      if (event.code === PROTOCOL_CLOSE_CODE) {
        const alreadyReported = this.stopped;
        this.stopped = true;
        this.clearReconnect();
        if (!alreadyReported) this.handlers.onMessage({ type: "Error", message: "Network protocol changed. Reload the game." });
      }
      if (event.code === SESSION_REPLACED_CLOSE_CODE || event.code === AUTH_REQUIRED_CLOSE_CODE) {
        // This corporation permits one live client. A replacement is a
        // deliberate sign-out, not a network failure: retrying would kick the
        // newer browser and make the two tabs fight forever.
        this.stopped = true;
        this.clearReconnect();
        this.handlers.onSessionReplaced();
        return;
      }
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
      try {
        this.ws.send(encodeMessage(msg));
      } catch (error) {
        this.handlers.onMessage({ type: "Error", message: error instanceof Error ? error.message : "Order could not be sent." });
      }
    }
  }

  setViewHz(hz: ViewHz): void {
    this.viewHz = hz;
  }

  join(name: string): void {
    this.send({ type: "Join", name, view_hz: this.viewHz });
  }

  get connected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }
}
