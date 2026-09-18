import { tenderCapable, tenderDemand, tenderFuel, tenderStatus } from "../core/derive/tenders";
import { shipKindLabel } from "../core/derive/fleet";
import type { GhostView } from "../protocol";
import type { ViewState } from "../state";

const esc = (v: string) => v.replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]!));

/** Same compact service form in both shells. The usual press/focus guard owns
 * the select, and its chosen ID is frozen only when the player stages an order. */
export function fuelTransferHtml(g: GhostView, st: ViewState, selected = "", mobile = false): string {
  if (!tenderCapable(g) && !g.fuel_transfer) return "";
  const targets = st.ghosts.filter(t => t.own && !t.tca && t.id !== g.id);
  const chosen = targets.find(t => t.id === selected)?.id ?? targets[0]?.id;
  const waiting = (st.pendingOrders.get(g.id) ?? []).some(p => !p.lost && p.kind === "refuel");
  const available = tenderFuel(g);
  const report = g.fuel_transfer;
  const meter = report?.requested ? `<progress max="${report.requested}" value="${report.spent}" aria-label="Reported fuel transfer"></progress>` : "";
  const targetName = (id: string) => {
    const target = st.ghosts.find(t => t.id === id);
    return `${target ? shipKindLabel(target.kind) : "Fleet"} · ${id}`;
  };
  const progress = report ? `<p>${esc(tenderStatus(g))} · ${esc(targetName(report.target))}${report.requested
    ? `<br>${report.delivered.toFixed(1)} Fuel delivered · ${report.spent}/${report.requested} cargo used` : ""}</p>${meter}` : "";
  const disabled = waiting || !available || !chosen || !tenderCapable(g);
  return `<section class="${mobile ? "m-section" : "deck-section"}" data-fuel-transfer><header><h3>Fleet tender</h3></header>
    <p title="Only Fuel in the hold can be transferred. Your propulsion tank stays separate. Final-can residue is purged (less than 1 Fuel).">Fuel cargo: ${available}</p>${progress}
    <div class="${mobile ? "m-form-row" : "deck-inline-form"}"><select id="tender-target-${esc(g.id)}" data-tender-target aria-label="Fleet to refuel" ${waiting ? "disabled" : ""}>
    ${targets.map(t => `<option value="${esc(t.id)}" ${t.id === chosen ? "selected" : ""}>${esc(targetName(t.id))} · ${tenderDemand(t) === null ? "fuel unknown" : `~${tenderDemand(t)} Fuel needed`}</option>`).join("") || '<option value="">No other owned fleets</option>'}</select>
    <button type="button" ${mobile ? 'data-mobile-act="fleet-refuel"' : 'data-deck-act="fleet-refuel"'} data-id="${esc(g.id)}" ${disabled ? "disabled" : ""}>${waiting ? "Refuel order in flight" : "Refuel fleet"}</button></div>
    <small>Target must hold · stops near combat</small></section>`;
}
