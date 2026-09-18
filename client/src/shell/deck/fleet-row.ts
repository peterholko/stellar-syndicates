import { shipKindLabel } from "../../core/derive/fleet";
import { fmt, informationDelay } from "../../core/derive/format";
import { icon, type IconKey } from "../../icons";
import { countClassLabel, fleetCargoUnits, fleetExactCount, type GhostView, type ShipKind } from "../../protocol";

interface FleetRowOptions {
  openAction: "roster-open" | "system-fleet-open";
  status: string;
  location?: string;
  flagshipName?: string | null;
  controls?: boolean;
  grouped?: boolean;
}

/** Shared desktop roster presentation, built only from the supplied served
 * ghost. Callers retain their own membership filters and delegated actions. */
export function fleetListRow(fleet: GhostView, options: FleetRowOptions): string {
  const exact = fleetExactCount(fleet);
  const composition = (fleet.composition ?? []).filter((stack) => stack.count > 0)
    .map((stack) => `${stack.count}× ${shipKindLabel(stack.kind)}`).join(" · ");
  const cargo = fleetCargoUnits(fleet);
  const summary = [
    exact === null ? `estimated ${countClassLabel(fleet.count_class)} ships` : `${exact} ship${exact === 1 ? "" : "s"}`,
    composition,
    cargo > 0 ? `${fmt(cargo)} cargo` : "",
  ].filter(Boolean).join(" · ");
  const name = (fleet.kind === "titan" && options.flagshipName?.trim()) || `${shipKindLabel(fleet.kind)} fleet`;
  const location = options.location ? `<small class="deck-fleet-list-row__location">${esc(options.location)}</small>` : "";
  const controls = options.controls
    ? `<div class="deck-fleet-list-row__actions"><button type="button" data-deck-act="roster-group" data-fleet="${esc(fleet.id)}" aria-pressed="${!!options.grouped}" aria-label="${options.grouped ? "Ungroup" : "Group"} ${esc(name)}">${options.grouped ? "✓ Grouped" : "+ Group"}</button><button type="button" data-deck-act="roster-center" data-fleet="${esc(fleet.id)}" aria-label="Center ${esc(name)} on the map">Center</button></div>`
    : "";
  return `<article class="deck-fleet-list-row${options.grouped ? " is-grouped" : ""}" data-fleet-row="${esc(fleet.id)}"><button type="button" class="deck-fleet-list-row__open" data-deck-act="${options.openAction}" data-fleet="${esc(fleet.id)}" aria-label="Open ${esc(name)}"><span class="deck-fleet-list-row__art">${icon(fleetListIcon(fleet), "md")}</span><span class="deck-fleet-list-row__identity"><b>${esc(name)}</b><small>${esc(summary)}</small>${location}</span><span class="deck-fleet-list-row__meta"><b${fleet.docked ? ' class="is-docked"' : ""}>${esc(options.status)}</b><small class="deck-stale-value">${esc(informationDelay(fleet.age))}</small></span></button>${controls}</article>`;
}

function fleetListIcon(fleet: GhostView): IconKey {
  // A single hull gets its ship art; a formation keeps the multi-ship emblem.
  if (fleetExactCount(fleet) !== 1) return "fleet";
  const kind = fleet.composition?.find((stack) => stack.count === 1)?.kind ?? fleet.kind;
  const icons: Partial<Record<ShipKind, IconKey>> = {
    scout: "scout", raider: "raider", corvette: "corvette", convoy: "convoy", colony: "colony",
    tiny_freighter: "tiny_freighter", small_freighter: "small_freighter", large_freighter: "large_freighter", heavy_freighter: "heavy_freighter", bulk_freighter: "bulk_freighter",
    builder: "builder", transport: "transport",
    destroyer: "destroyer", cruiser: "cruiser", battleship: "battleship",
    dreadnought: "dreadnought", titan: "titan", freighter: "authorityFreighter",
  };
  return icons[kind] ?? "fleet";
}

const esc = (value: string): string => value.replace(/[&<>"']/g, (char) => ({
  "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
})[char]!);
