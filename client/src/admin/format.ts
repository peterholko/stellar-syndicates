// Pure formatting for the operator dashboard. No DOM, no clock of its own:
// callers pass `now`, so the same row reads the same in a test and on screen.

/** "45s", "12m", "3h 05m", "2d 4h". */
export function duration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "—";
  const s = Math.round(seconds);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ${String(m % 60).padStart(2, "0")}m`;
  return `${Math.floor(h / 24)}d ${h % 24}h`;
}

/** "just now", "5m ago", "3h ago", "4d ago"; older reads as a date. */
export function ago(iso: string | null | undefined, now: number): string {
  if (!iso) return "never";
  const then = Date.parse(iso);
  if (!Number.isFinite(then)) return "—";
  const seconds = Math.max(0, (now - then) / 1000);
  if (seconds < 45) return "just now";
  if (seconds < 3600) return `${Math.round(seconds / 60)}m ago`;
  if (seconds < 86_400) return `${Math.floor(seconds / 3600)}h ago`;
  if (seconds < 14 * 86_400) return `${Math.floor(seconds / 86_400)}d ago`;
  return date(iso);
}

/** Local "Oct 5, 14:03". */
export function dateTime(iso: string | null | undefined): string {
  if (!iso) return "—";
  const at = new Date(iso);
  if (!Number.isFinite(at.getTime())) return "—";
  return at.toLocaleString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
}

/** Local "Oct 5, 2026". */
export function date(iso: string | null | undefined): string {
  if (!iso) return "—";
  const at = new Date(iso);
  if (!Number.isFinite(at.getTime())) return "—";
  return at.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
}

export function count(value: number | null | undefined): string {
  return typeof value === "number" && Number.isFinite(value) ? Math.round(value).toLocaleString() : "—";
}

/** "86%"; a share that rounds to nothing but is not zero reads "<1%". */
export function percent(share: number | null | undefined): string {
  if (typeof share !== "number" || !Number.isFinite(share)) return "—";
  if (share > 0 && share < 0.005) return "<1%";
  return `${Math.round(share * 100)}%`;
}

/** Credits as the game's top bar shows them, compacted past a million. */
export function credits(value: number | null | undefined): string {
  if (typeof value !== "number" || !Number.isFinite(value)) return "—";
  const abs = Math.abs(value);
  if (abs >= 1e9) return `${(value / 1e9).toFixed(2)}B`;
  if (abs >= 1e6) return `${(value / 1e6).toFixed(2)}M`;
  return Math.round(value).toLocaleString();
}

/** Population arrives in millions, as the empire panel reads it. */
export function population(millions: number | null | undefined): string {
  if (typeof millions !== "number" || !Number.isFinite(millions)) return "—";
  const people = millions * 1_000_000;
  if (people >= 1_000_000) return `${(people / 1_000_000).toFixed(2)}m`;
  if (people >= 1_000) return `${(people / 1_000).toFixed(1)}k`;
  return Math.round(people).toLocaleString();
}

/** "Chrome 141 · macOS" from a user agent; the raw string stays in a tooltip. */
export function browser(agent: string | null | undefined): string {
  if (!agent) return "—";
  const version = (pattern: RegExp) => agent.match(pattern)?.[1]?.split(".")[0];
  let name = "Browser";
  let v: string | undefined;
  if ((v = version(/Edg(?:e|A|iOS)?\/([\d.]+)/))) name = "Edge";
  else if ((v = version(/OPR\/([\d.]+)/))) name = "Opera";
  else if ((v = version(/Firefox\/([\d.]+)/) ?? version(/FxiOS\/([\d.]+)/))) name = "Firefox";
  else if ((v = version(/Chrome\/([\d.]+)/) ?? version(/CriOS\/([\d.]+)/))) name = "Chrome";
  else if (/Safari\//.test(agent) && (v = version(/Version\/([\d.]+)/))) name = "Safari";
  else if (/^[\w.-]+\/[\d.]+$/.test(agent.trim())) return agent.trim();
  const os = /iPhone|iPod/.test(agent) ? "iPhone"
    : /iPad/.test(agent) ? "iPad"
    : /Android/.test(agent) ? "Android"
    : /Windows/.test(agent) ? "Windows"
    : /Mac OS X|Macintosh/.test(agent) ? "macOS"
    : /CrOS/.test(agent) ? "ChromeOS"
    : /Linux/.test(agent) ? "Linux"
    : null;
  return [v ? `${name} ${v}` : name, os].filter(Boolean).join(" · ");
}

/** First eight characters of a device id: enough to tell browsers apart. */
export function device(id: string | null | undefined): string {
  return id ? id.slice(0, 8) : "—";
}

/** One-line JSON of a recorded command, cut to `max` characters. */
export function detail(value: unknown, max = 120): string {
  if (value === null || value === undefined) return "";
  const text = JSON.stringify(value) ?? "";
  return text.length > max ? `${text.slice(0, max - 1)}…` : text;
}

/** "MoveShip" → "Move ship"; the kind stays exact in its tooltip. */
export function commandName(kind: string): string {
  const words = kind.replace(/([a-z0-9])([A-Z])/g, "$1 $2").toLowerCase();
  return words.charAt(0).toUpperCase() + words.slice(1);
}
