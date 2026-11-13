// §client-updates (Nov 11 2026, docs/client-updates.md): an open tab learns
// that a newer client is out. Each build stamps its pages' `ss-build` meta and
// serves the newest stamp, with its wire protocol, in /version.json
// (vite.config.ts `buildStamp`). The tab checks when it comes back into view,
// every five minutes, and when the server closes it for an old protocol. Out
// of view it reloads itself; in view a pill offers the reload ("Update
// required" once the protocol changed: this tab can no longer reach the server).
import "./styles/update.css";
import { PROTOCOL_VERSION } from "./wire.mjs";

export const CHECK_INTERVAL_MS = 5 * 60_000;
/** A burst of focus changes checks once. */
export const MIN_CHECK_GAP_MS = 30_000;
const RELOADED_FOR = "ss-reloaded-for";

export interface LatestBuild { build: string; protocol?: number }
export type UpdateNeed = "none" | "offer" | "required";

/** What this tab should do about the newest build: nothing (the same build,
 * or the Vite dev server's "dev"), offer it, or require it (another protocol). */
export function updateNeed(current: string, latest: LatestBuild | null, protocol: number = PROTOCOL_VERSION): UpdateNeed {
  if (current === "dev" || !latest || typeof latest.build !== "string" || !latest.build || latest.build === current) return "none";
  return typeof latest.protocol === "number" && latest.protocol !== protocol ? "required" : "offer";
}

/** The page around the checks; the browser's own unless a test passes one. */
export interface UpdateEnv {
  build: string;
  latest: () => Promise<LatestBuild | null>;
  hidden: () => boolean;
  reload: () => void;
  now: () => number;
  /** The build this tab last reloaded itself for (this session). */
  remembered: () => string | null;
  remember: (build: string) => void;
  show: (need: Exclude<UpdateNeed, "none">, reload: () => void) => void;
}

export interface ClientUpdates {
  /** Ask for the newest build; `force` skips the burst guard. */
  check: (force?: boolean) => Promise<void>;
  /** The tab went out of view: a known update loads now. */
  hidden: () => void;
}

export function clientUpdates(env: UpdateEnv): ClientUpdates {
  let last = -Infinity;
  let pending: LatestBuild | null = null;
  const reload = (latest: LatestBuild) => {
    env.remember(latest.build);
    env.reload();
  };
  // Out of view nothing is lost. Once per build, so a page that still serves
  // the old stamp (a cache mid-deploy) never reloads in a loop.
  const reloadQuietly = (latest: LatestBuild): boolean => {
    if (env.remembered() === latest.build) return false;
    reload(latest);
    return true;
  };
  return {
    check: async (force = false) => {
      if (env.build === "dev") return;
      const now = env.now();
      if (!force && now - last < MIN_CHECK_GAP_MS) return;
      last = now;
      let latest: LatestBuild | null;
      try {
        latest = await env.latest();
      } catch {
        return; // offline: the next check tries again
      }
      const need = updateNeed(env.build, latest);
      if (need === "none" || !latest) return;
      const found = latest;
      pending = found;
      if (env.hidden() && reloadQuietly(found)) return;
      env.show(need, () => reload(found));
    },
    hidden: () => {
      if (pending) reloadQuietly(pending);
    },
  };
}

let pill: HTMLElement | null = null;

/** The pill over either shell; one per page, its words and button replaced. */
export function showUpdatePill(need: Exclude<UpdateNeed, "none">, reload: () => void): void {
  pill ??= document.body.appendChild(document.createElement("div"));
  pill.className = `ss-update${need === "required" ? " is-required" : ""}`;
  pill.setAttribute("role", "status");
  pill.innerHTML = `<span>${need === "required" ? "Update required" : "New version"}</span><button type="button">Reload</button>`;
  pill.querySelector("button")?.addEventListener("click", reload);
}

function browserEnv(): UpdateEnv {
  return {
    build: document.querySelector<HTMLMetaElement>('meta[name="ss-build"]')?.content || "dev",
    latest: async () => {
      const response = await fetch("/version.json", { cache: "no-store" });
      return response.ok ? await response.json() as LatestBuild : null;
    },
    hidden: () => document.visibilityState === "hidden",
    reload: () => location.reload(),
    now: () => Date.now(),
    remembered: () => {
      try { return sessionStorage.getItem(RELOADED_FOR); } catch { return null; }
    },
    remember: (build) => {
      try { sessionStorage.setItem(RELOADED_FOR, build); } catch { /* storage blocked: reload anyway */ }
    },
    show: showUpdatePill,
  };
}

/** Starts the checks in this page: on coming back into view and every five
 * minutes. Returns the check, for the socket's protocol close. */
export function installClientUpdates(): (force?: boolean) => Promise<void> {
  const updates = clientUpdates(browserEnv());
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") void updates.check();
    else updates.hidden();
  });
  window.setInterval(() => void updates.check(), CHECK_INTERVAL_MS);
  return updates.check;
}
