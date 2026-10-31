// §notifications (R8): the player's opt-in alerts. Settings live with the
// account (same-origin API, like sign-in); push goes to this browser through
// the service worker once the player allows it.

export interface NotifyPrefs {
  kinds: string[];
  email: string | null;
  email_digest: boolean;
  /** The server's VAPID key, for `pushManager.subscribe`. */
  public_key: string;
  /** Whether the server sends pushes and emails, or only logs them. */
  push_live: boolean;
  email_live: boolean;
  /** Browsers this account has subscribed. */
  devices: number;
}

export const ALERT_KINDS: readonly [string, string][] = [
  ["siege", "Colony under siege"],
  ["raid", "Pirate raid"],
  ["research", "Research done"],
  ["warship", "Warship launched"],
  ["signal", "Signal waiting"],
  ["board", "Fresh board"],
  ["medal", "Weekly medals"],
];

async function api(path: string, body?: object): Promise<Response> {
  const response = await fetch(`/api/notifications${path}`, {
    method: body === undefined ? "GET" : "POST",
    credentials: "same-origin",
    redirect: "error",
    cache: "no-store",
    headers: { "Content-Type": "application/json", "X-Stellar-Client": "1" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) {
    const data = await response.json().catch(() => null);
    throw new Error(data?.error ?? "Notifications are unavailable right now.");
  }
  return response;
}

/** base64url → bytes, for the VAPID key. */
function keyBytes(base64url: string): Uint8Array<ArrayBuffer> {
  const padded = base64url.replace(/-/g, "+").replace(/_/g, "/").padEnd(Math.ceil(base64url.length / 4) * 4, "=");
  const raw = atob(padded);
  const out = new Uint8Array(new ArrayBuffer(raw.length));
  for (let i = 0; i < raw.length; i++) out[i] = raw.charCodeAt(i);
  return out;
}

const pushSupported = (): boolean => "serviceWorker" in navigator && "PushManager" in window && "Notification" in window;

async function registration(): Promise<ServiceWorkerRegistration> {
  return navigator.serviceWorker.register("/sw.js");
}

export const notifications = {
  pushSupported,

  async get(): Promise<NotifyPrefs> {
    return (await api("")).json();
  },

  async save(kinds: string[], email: string | null, email_digest: boolean): Promise<void> {
    await api("", { kinds, email, email_digest });
  },

  /** This browser's subscription, if it has one. */
  async device(): Promise<PushSubscription | null> {
    if (!pushSupported()) return null;
    const reg = await navigator.serviceWorker.getRegistration("/sw.js");
    return reg ? reg.pushManager.getSubscription() : null;
  },

  /** Ask permission, subscribe this browser, and register it with the server. */
  async subscribe(publicKey: string): Promise<void> {
    if (!pushSupported()) throw new Error("This browser can't receive push alerts.");
    if (await Notification.requestPermission() !== "granted") throw new Error("Alerts are blocked in this browser's settings.");
    const reg = await registration();
    const sub = await reg.pushManager.getSubscription()
      ?? await reg.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: keyBytes(publicKey) });
    const json = sub.toJSON();
    await api("/push", { endpoint: sub.endpoint, p256dh: json.keys?.p256dh ?? "", auth: json.keys?.auth ?? "" });
  },

  async unsubscribe(): Promise<void> {
    const sub = await notifications.device();
    if (!sub) return;
    await api("/push/remove", { endpoint: sub.endpoint }).catch(() => undefined);
    await sub.unsubscribe();
  },
};
