const LIVE_NUMBER_FIELDS = new Set([
  "age",
  "current",
  "eta_secs",
  "fuel",
  "fuel_total",
  "inbound_migrants",
  "left",
  "population",
  "population_upkeep",
  "progress",
  "remaining",
  "reserves",
  "signature",
  "speed",
  "staleness",
  "storage_used",
  "suppression",
  "units",
  "valuation",
]);

/** Preserve discrete served state while a shell's whole-second heartbeat
 * refreshes values that advance every View. Route geometry never belongs in an
 * HTML-surface fingerprint; velocity is reduced to the status boundary it draws. */
export function sheetFingerprint(slice: unknown): string {
  return JSON.stringify(slice, (key, value: unknown) => {
    if (key === "path" || key === "route" || key === "pos") return undefined;
    if (key === "vel" && value && typeof value === "object") {
      const vector = value as { x?: number; y?: number };
      return Math.hypot(vector.x ?? 0, vector.y ?? 0) >= 0.5;
    }
    if (LIVE_NUMBER_FIELDS.has(key) && typeof value === "number") return undefined;
    return value;
  }) ?? "undefined";
}
