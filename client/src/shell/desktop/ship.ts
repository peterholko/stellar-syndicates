import { captainTitle, captainXpFloor, fleetCommandLoad, officerFleetName } from "../../core/derive/captains";
import { AAA_SERVICE_FEE, aaaEstimate, coLocatedOwnFleet, dockedAtSystem, dockLoadStock, estimatedFuelForLeg, fleetCargoCapacity, fleetFuelCapacity, FUEL_PER_MASS_DISTANCE, guardCapable, hauls, jumpCapable, shipKindLabel, shipMass, shipRoleLore, WARP_FACTOR } from "../../core/derive/fleet";
import { fmt, fmtDur } from "../../core/derive/format";
import { emplacementLabel, foundingHomeSystemId, HYPERLIMIT_SU, nearestKnownDock, systemName } from "../../core/derive/geo";
import { kitAffordable, kitCostLabel, ownedHaulDestinations } from "../../core/derive/market";
import { clearJumpDepartureSelection, intentSummary, jumpDepartureSelection, orderEtaRange, orderObject, orderPoint } from "../../core/derive/orders";
import { armGuardAiming, armJumpAiming, clearGuardAiming, clearJumpAiming, clearPendingIntent, confirmPendingIntent, intentAiming } from "../../core/intent";
import { jumpDepartureKey } from "../../core/session";
import { badgeChip, chip, icon, type IconKey, label } from "../../icons";
import { type CaptainAttribute, type CaptainRosterView, type Commodity, countClassLabel, type EngagementPosture, type EntityId, fleetCargoManifest, fleetCargoUnits, formatId, type GhostView, type ManifestEntryView, type ShipKind, type Vec2 } from "../../protocol";
import { renderer } from "../../render";
import { liveSimTime, state } from "../../state";
import { confirmAuthorityHostility } from "./faction";
import { net } from "./index";
import { $, badge, bar, commodityIcon, CONTACT_STALE_AGE_S, esc, readout, renderDeferred, setHtml, stat, statStrip, svgIcon } from "./mapchrome";
import { closeRail, openRail } from "./rail";


// --- Ship details panel — a FOG-AWARE master→detail card for the SELECTED ship.
// It shares the right-dock slot with the rail (mutually exclusive: selecting a ship
// closes the rail and clears any system selection; opening the rail deselects the
// ship). Re-renders each View so the information AGE keeps ticking. Strictly a UI
// layer over GhostView — it shows ONLY what the per-player view already reveals, so
// a rival's cargo/route/internal state never leaks. ------------------------------
export let shipPanelBuilt = false;

// The panel is rebuilt with every fresh View, so disclosure state must outlive
// its DOM. Sets make the default collapsed and keep each fleet's choice stable.
export const expandedShipPolicies = new Set<string>();

export const expandedShipManagement = new Set<string>();

export type ShipPanelTab = "orders" | "fleet" | "officer";

export let shipPanelTab: ShipPanelTab = "orders";

export function buildShipPanel(): void {
  if (shipPanelBuilt) return;
  shipPanelBuilt = true;
  // One delegated listener survives the per-View innerHTML rewrites.
  $("ship-panel").addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest("[data-act],[data-ship-tab]");
    if (!b) return;
    const act = (b as HTMLElement).dataset.act;
    const requestedTab = (b as HTMLElement).dataset.shipTab as ShipPanelTab | undefined;
    if (requestedTab && (["orders", "fleet", "officer"] as ShipPanelTab[]).includes(requestedTab)) {
      shipPanelTab = requestedTab;
      updateShipPanel();
    } else if (act === "close") {
      deselectShip();
    } else if (act === "select-order" && state.selectedShipId) {
      const id = Number((b as HTMLElement).dataset.orderId);
      const exists = (state.pendingOrders.get(state.selectedShipId) ?? []).some((p) => p.id === id);
      if (exists) {
        state.selectedOrderId = state.selectedOrderId === id ? null : id;
        renderer.stateVersion++;
        updateShipPanel();
      }
    } else if (act === "dismiss-lost-order") {
      const id = Number((b as HTMLElement).dataset.orderId);
      if (Number.isFinite(id)) net?.send({ type: "DismissLostOrder", order_id: id });
    } else if (act === "jump" && state.selectedShipId) {
      const fleet = state.ghosts.find((g) => g.id === state.selectedShipId && g.own);
      if (fleet && jumpCapable(fleet)) armJumpAiming(fleet);
    } else if (act === "guard" && state.selectedShipId) {
      const fleet = state.ghosts.find((g) => g.id === state.selectedShipId && g.own);
      if (fleet && guardCapable(fleet)) armGuardAiming(fleet);
    } else if (act === "dock" && state.selectedShipId && net) {
      const fleet = state.ghosts.find((g) => g.id === state.selectedShipId && g.own);
      const dock = fleet ? nearestKnownDock(fleet) : null;
      if (!fleet || !dock) return;
      clearPendingIntent();
      net.send({ type: "MoveShip", ship_id: fleet.id, dest: dock.pos });
      readout().innerHTML = `<b>Docking order sent</b> · ${esc(dock.name)} ` +
        `<span class="dim">· ${Math.round(dock.distance).toLocaleString()} su from the served sighting · signal outbound</span>`;
    } else if (act === "fuel-rescue" && state.selectedShipId && net) {
      const fleet = state.ghosts.find((g) => g.id === state.selectedShipId && g.own);
      if (!fleet?.stalled || fleet.rescue_inbound) return;
      const quote = aaaEstimate(fleet);
      const proceed = window.confirm(
        `Call Authority Astral Assistance?\n\n` +
          `Emergency fuel: ~${fmt(quote.fuel)} units\n` +
          `Estimated charge: ~${fmt(quote.cost)} credits\n` +
          `(3× current Fuel price + ${fmt(AAA_SERVICE_FEE)}-credit callout fee)\n\n` +
          `A physical AAA tender will fly from the Wormhole Hub. The dispatch fee is not refunded if the tender is lost.`,
      );
      if (!proceed) return;
      net.send({ type: "RequestFuelRescue", fleet_id: fleet.id });
      readout().innerHTML = `<b>AAA callout requested</b> · awaiting Authority dispatch receipt`;
    } else if (act === "toggle-policy" && state.selectedShipId) {
      if (expandedShipPolicies.has(state.selectedShipId)) expandedShipPolicies.delete(state.selectedShipId);
      else expandedShipPolicies.add(state.selectedShipId);
      updateShipPanel();
    } else if (act === "toggle-management" && state.selectedShipId) {
      if (expandedShipManagement.has(state.selectedShipId)) expandedShipManagement.delete(state.selectedShipId);
      else expandedShipManagement.add(state.selectedShipId);
      updateShipPanel();
    } else if (act === "emplace" && state.selectedShipId) {
      const k = (e.target as HTMLElement).closest(".emplace-btn")?.getAttribute("data-kind") as
        | "deep_space_sensor" | null;
      if (k) {
        // §emplacements: BUILD WHERE THE SHIP IS PARKED — the player flies the
        // Construction Ship to the spot first, then this button raises the
        // structure in place. No siting mode: every refusal the sim would make
        // is mirrored here with its reason, because the server declines in
        // silence (a click that does nothing was the launch bug).
        const g = state.ghosts.find((x) => x.id === state.selectedShipId && x.own);
        const pretty = "Deep Space Sensor";
        if (!g) {
          readout().innerHTML = `<span style="color:var(--warn)">That Construction Ship is gone — reselect one and try again.</span>`;
          return;
        }
        const err = renderer.siteError(k, g.pos, state);
        if (err) {
          readout().innerHTML =
            `<span style="color:var(--warn)">Can't raise a <b>${pretty}</b> here: ${esc(err)} ` +
            `<span class="dim">Move the ship, then build.</span></span>`;
          return;
        }
        if (!kitAffordable(k)) {
          readout().innerHTML =
            `<span style="color:var(--warn)">A <b>${pretty}</b> kit needs ${esc(kitCostLabel(k))} ` +
            `from a single system of yours — none can cover it. Stock up and try again.</span>`;
          return;
        }
        net?.send({ type: "BuildEmplacement", builder: state.selectedShipId, emplacement: k });
        readout().innerHTML =
          `<b>${pretty}</b> ordered — it rises where the ship is parked (signal outbound).`;
        updateShipPanel();
      }
    } else if (act === "split" && state.selectedShipId && net) {
      const kind = (b as HTMLElement).dataset.kind as ShipKind | undefined;
      if (kind) {
        net.send({ type: "SplitFleet", fleet_id: state.selectedShipId, counts: { [kind]: 1 } });
      }
    } else if (act === "merge" && state.selectedShipId && net) {
      const from = (b as HTMLElement).dataset.from;
      if (from) {
        net.send({ type: "MergeFleets", into: state.selectedShipId, from });
      }
    } else if (act === "posture" && state.selectedShipId && net) {
      const posture = (b as HTMLElement).dataset.mode as EngagementPosture | undefined;
      if (posture) {
        postureModes.set(state.selectedShipId, posture);
        net.send({ type: "SetFleetPosture", fleet_id: state.selectedShipId, posture });
        updateShipPanel();
      }
    } else if (act === "train-captain" && net) {
      const attribute = (b as HTMLElement).dataset.attribute as CaptainAttribute | undefined;
      const captainId = Number((b as HTMLElement).dataset.captainId);
      if (attribute && Number.isFinite(captainId)) {
        net.send({ type: "TrainCaptain", captain_id: captainId, attribute });
      }
    } else if (act === "open-officers") {
      openRail("officers");
    } else if (act === "nameflagship" && net) {
      // §ladder B4: christen the syndicate's Titan (empty clears the name).
      const name = window.prompt("Flagship name (≤24 chars — empty to un-name):", state.syndicate?.flagship_name ?? "");
      if (name !== null) net.send({ type: "NameFlagship", name: name.trim() });
    } else if (act === "engage-freight" && state.selectedShipId && net) {
      const turningOn = (b as HTMLElement).dataset.on === "1";
      if (turningOn && !confirmAuthorityHostility("Engage Authority freight arriving at this blockade?")) {
        return;
      }
      net.send({ type: "SetEngageFreight", fleet_id: state.selectedShipId, on: turningOn });
    } else if ((act === "load" || act === "unload" || act === "haul" || act === "haul-system") && state.selectedShipId && net) {
      // §TCA Part 5: dockside logistics. The served BERTH decides which command
      // to send; proximity is not docking (the sim also requires idle + no fight).
      const fleet_id = state.selectedShipId;
      const g = state.ghosts.find((x) => x.id === fleet_id);
      const atHub = g?.docked === "hub";
      const sys = g?.docked
        ? (state.galaxy?.systems ?? []).find((sy) =>
            dockedAtSystem(g, sy.id)
            && state.systems.find((served) => served.id === sy.id)?.owner === state.playerId)
        : undefined;
      const root = (b as HTMLElement).closest("#ship-panel") ?? document;
      if (!g || (!atHub && !sys)) {
        readout().innerHTML = `<span style="color:var(--warn)">That fleet is not berthed yet. Wait for its served Docked report, then unload.</span>`;
        return;
      }
      if (act === "unload") {
        net.send(atHub ? { type: "HubUnload", fleet_id } : { type: "SystemUnload", fleet_id, system: sys!.id });
        readout().innerHTML = `<b>Unload requested</b> · ${atHub ? "Wormhole Hub" : esc(sys!.name)}`;
      } else if (act === "load") {
        const commodity = (root.querySelector(".lg-com") as HTMLSelectElement | null)?.value as Commodity | undefined;
        const units = Math.max(1, Math.floor(Number((root.querySelector(".lg-qty") as HTMLInputElement | null)?.value) || 0));
        if (commodity) {
          net.send(atHub ? { type: "HubLoad", fleet_id, commodity, units } : { type: "SystemLoad", fleet_id, system: sys!.id, commodity, units });
        }
      } else if (act === "haul-system") {
        if (!atHub) return;
        const system = (root.querySelector(".lg-haul-system") as HTMLSelectElement | null)?.value as EntityId | undefined;
        const destination = ownedHaulDestinations().find((candidate) => candidate.id === system);
        if (!destination) return;
        const destinationPos = state.galaxy?.systems.find((candidate) => candidate.id === destination.id)?.pos;
        if (destinationPos && !confirmHaulFuel(g, destinationPos, destination.name)) return;
        net.send({ type: "HaulToSystem", fleet_id, system: destination.id });
        readout().innerHTML = `<b>Return haul ordered</b> · ${esc(destination.name)}`;
      } else {
        const sell = !!(root.querySelector(".lg-sell") as HTMLInputElement | null)?.checked;
        if (state.galaxy && !confirmHaulFuel(g, state.galaxy.hub, "the Market Hub")) return;
        net.send({ type: "HaulToMarketHub", fleet_id, sell_on_arrival: sell });
      }
    }
  });
  $("ship-panel").addEventListener("change", (e) => {
    const select = (e.target as HTMLElement).closest(".lg-haul-system") as HTMLSelectElement | null;
    if (!select || !state.selectedShipId) return;
    haulDestinationByFleet.set(state.selectedShipId, select.value as EntityId);
    const name = select.selectedOptions[0]?.textContent?.trim() || "system";
    const label = select.closest(".sp-line")?.querySelector(".lg-haul-label");
    if (label) label.textContent = `Haul back to ${name}`;
  });
}

export function selectShip(id: string): void {
  clearJumpDepartureSelection();
  clearGuardAiming(true);
  if (state.selectedShipId !== id) {
    clearPendingIntent();
    clearJumpAiming(true);
    state.selectedOrderId = null;
    shipPanelTab = "orders";
  }
  state.selectedShipId = id;
  state.selectedSystemId = null; // a ship and a system are never both selected
  state.selectedEmplacementId = null; // …nor a ship and a structure
  closeRail(); // the ship panel and rail share the right-dock slot
  $("ship-panel").classList.add("is-open");
  buildShipPanel();
  updateShipPanel();
}

// §emplacements: select a standing structure — same right-dock panel as a
// ship, since both answer "what is this thing I clicked".
export function selectEmplacement(id: string): void {
  clearJumpDepartureSelection();
  clearPendingIntent();
  clearJumpAiming(true);
  state.selectedOrderId = null;
  state.selectedEmplacementId = id;
  state.selectedShipId = null;
  state.selectedSystemId = null;
  closeRail();
  $("ship-panel").classList.add("is-open");
  buildShipPanel();
  updateShipPanel();
}

export function selectJumpDeparture(key: string): void {
  deselectShip();
  jumpDepartureSelection.key = key;
  renderer.selectedJumpDepartureKey = key;
  state.selectedSystemId = null;
  closeRail();
  $("ship-panel").classList.add("is-open");
  buildShipPanel();
  updateShipPanel();
}

export function deselectShip(): void {
  clearPendingIntent();
  clearJumpAiming(true);
  clearGuardAiming(true);
  state.selectedOrderId = null;
  state.selectedShipId = null;
  state.selectedEmplacementId = null;
  clearJumpDepartureSelection();
  $("ship-panel").classList.remove("is-open");
}


// --- Deliberate map orders: preview first, transmit only on confirmation. ----
export let intentBarBuilt = false;


export function renderIntentBar(): void {
  const root = $("intent-bar");
  const intent = state.pendingIntent;
  root.classList.toggle("is-open", intent !== null);
  setHtml(root, intent
    ? `<div class="intent-bar__summary">${esc(intentSummary(intent))}</div>` +
      `<div class="intent-bar__actions">` +
      `<button class="act act--primary" data-act="confirm-intent">Confirm <span class="dim">Enter</span></button>` +
      `<button class="act" data-act="cancel-intent">Cancel <span class="dim">Esc</span></button></div>`
    : "");
}


export function buildIntentBar(): void {
  if (intentBarBuilt) return;
  intentBarBuilt = true;
  $("intent-bar").addEventListener("click", (e) => {
    const button = (e.target as HTMLElement).closest("[data-act]") as HTMLElement | null;
    if (button?.dataset.act === "confirm-intent") confirmPendingIntent();
    else if (button?.dataset.act === "cancel-intent") clearPendingIntent();
  });
}


// --- §order-lifecycle: SIGNAL OUTBOUND → PRESUMED DELIVERED → CONFIRMED -------
// Below this, phases collapse to ~instant (a fleet near the command center) —
// suppress the noisy sub-second states.
export const LIFECYCLE_MIN_S = 1.5;


export const fmtCountdown = (secs: number): string => {
  const s = Math.max(0, Math.round(secs));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};


// Shared transient report path: battle alerts, delayed results, and comms
// reacquisition all use the same capped/fading HUD stream.
export function addTransientReport(iconText: string, cls: "good" | "bad", html: string): HTMLDivElement {
  const log = $("reports-log");
  const el = document.createElement("div");
  el.className = `report ${cls}`;
  el.innerHTML = `<span class="ic">${iconText}</span> ${html}`;
  log.prepend(el);
  while (log.children.length > 6) log.removeChild(log.lastChild!);
  setTimeout(() => el.classList.add("fade"), 12000);
  return el;
}


// §battles-take-time: notify ONCE when a battle first becomes visible (light-
// gated by the server). Keyed by a coarse location so it re-fires only for a
// genuinely new battle after an old one ends.
export const seenBattles = new Set<string>();

export function notifyNewBattles(battles: import("../../protocol").BattleView[]): void {
  const nowKeys = new Set<string>();
  for (const b of battles) {
    const key = `${Math.round(b.pos.x / 200)},${Math.round(b.pos.y / 200)}`;
    nowKeys.add(key);
    if (!seenBattles.has(key)) {
      addTransientReport(
        "⚔",
        "bad",
        `<b>Battle raging</b> near (${b.pos.x.toFixed(0)}, ${b.pos.y.toFixed(0)}) <span class="dim">— as of ${fmtCountdown(b.age)} ago${b.own ? " · your fleet is engaged" : ""}</span>`,
      );
    }
  }
  seenBattles.clear();
  for (const k of nowKeys) seenBattles.add(k);
}


export const orderEta = (secs: number): string => `~${Math.max(0, Math.ceil(secs))}s`;


export function ordersZone(g: GhostView): string {
  const queue = state.pendingOrders.get(g.id) ?? [];
  const now = liveSimTime();
  const lifecycleRows = queue.map((p) => {
    if (p.lost) {
      return `<div class="sp-order sp-order--lost" title="The fleet jumped away before this signal reached its old position. It will never arrive; issue a replacement manually.">` +
        `<span class="sp-order__phase" aria-hidden="true">${icon("lost", "sm")}</span>` +
        `<span class="sp-order__copy"><b>${esc(orderObject(p))}</b><small>LOST — the fleet jumped away before the signal arrived</small></span>` +
        `<button class="sp-order__dismiss" data-act="dismiss-lost-order" data-order-id="${p.id}" aria-label="Dismiss lost order">Dismiss</button></div>`;
    }
    const outbound = now < p.arrives_at;
    const selected = state.selectedOrderId === p.id;
    const spoolEnd = p.arrives_at + (state.galaxy?.jump_spool_s ?? 10);
    const spooling = p.kind === "jump" && !outbound && now < spoolEnd;
    const phase = icon(outbound ? "inTransit" : "echo", "sm");
    const responseEstimate = now <= p.response_at
      ? `ETA ${orderEta(p.response_at - now)}`
      : `overdue ${orderEta(now - p.response_at)}`;
    const eta = p.kind === "jump"
      ? outbound
        ? `signal outbound · ETA ${orderEta(p.arrives_at - now)}`
        : spooling
          ? `spooling ~${Math.max(0, Math.ceil(spoolEnd - now))}s (est)`
          : g.jump_presumed
            ? `presumed jumped · report ~${orderEta(g.jump_presumed.report_in)}`
            : `presumed jumped · awaiting light`
      : outbound
        ? `signal outbound · ETA ${orderEta(p.arrives_at - now)}`
        : `awaiting response · ${responseEstimate}`;
    const status = selected && p.kind !== "jump" ? orderEtaRange(p.response_at, now) : eta;
    const tip = p.kind === "jump"
      ? outbound
        ? `SIGNAL OUTBOUND — this jump order should reach the fleet in ${orderEta(p.arrives_at - now)}.`
        : spooling
          ? `SPOOLING — estimated from the served picture; movement or combat can interrupt it.`
          : g.jump_presumed
            ? `PRESUMED JUMPED — departure light proves the jump; destination light is expected in about ${orderEta(g.jump_presumed.report_in)}.`
            : `PRESUMED JUMPED — awaiting the destination light that confirms the discontinuity.`
      : outbound
        ? `SIGNAL OUTBOUND — from the command center's served picture, this ${p.kind} order should reach the fleet in ${orderEta(p.arrives_at - now)}.`
        : `PRESUMED DELIVERED — awaiting compliance light; response ${responseEstimate}.`;
    return `<button class="sp-order${selected ? " is-selected" : ""}${outbound ? "" : " is-presumed"}" data-act="select-order" data-order-id="${p.id}" aria-pressed="${selected}" title="${esc(tip)}">` +
      `<span class="sp-order__phase" aria-hidden="true">${phase}</span>` +
      `<span class="sp-order__copy"><b>${esc(orderObject(p))}</b><small${selected ? ` class="is-estimate"` : ""}>${status}</small></span></button>`;
  }).join("");

  // A pending lifecycle ends when the response reaches command, not when the
  // resulting flight ends. Keep those two ideas separate: the served plan is
  // the current order, and the local destination record survives until the
  // served fleet is seen parked there. If a replacement signal is outbound,
  // this also leaves the old served course visible beside the new lifecycle.
  const localDest = state.orders[g.id];
  const trackingMove = !!localDest || queue.some((p) => p.kind === "move");
  const servedDest = trackingMove && g.path?.length
    ? g.path[g.path.length - 1].pos
    : undefined;
  const currentDest = servedDest ?? localDest;
  const representedByLifecycle = currentDest && queue.some((p) =>
    p.kind === "move" && !!p.dest
      && Math.hypot(p.dest.x - currentDest.x, p.dest.y - currentDest.y) < 1,
  );
  const currentRow = currentDest && !representedByLifecycle
    ? `<div class="sp-order sp-order--current" title="CURRENT ORDER — remains in force until the served fleet arrives at this destination.">` +
      `<span class="sp-order__phase" aria-hidden="true">${icon("move", "sm")}</span>` +
      `<span class="sp-order__copy"><b>${esc(`Move → ${orderPoint(currentDest)}`)}</b><small>current order · under way</small></span></div>`
    : "";
  const rows = lifecycleRows + currentRow;
  if (!rows) return "";
  return shipZone("Orders", rows, "sp-zone--orders");
}


// §offensive-orders Part 2: the player's chosen engagement POSTURE per own fleet
// (optimistic — echoes SetFleetPosture; falls back to the View's owner-only value).
export const postureModes = new Map<string, EngagementPosture>();

export const POSTURE_META: { key: EngagementPosture; label: string; hint: string }[] = [
  { key: "passive", label: "Passive", hint: "Fight only if engaged — take no autonomous offensive action (default)." },
  { key: "defensive", label: "Defensive", hint: "Defend a guarded asset / station (picket behaviour); no proactive hunting." },
  { key: "weapons_free", label: "Weapons-free", hint: "Auto-attack any rival that enters this fleet's OWN sensor bubble — on its own local detection, no command-center round trip. A lone freighter is raided, anything armed is destroyed; still gated by your corp doctrine's odds." },
];


// The POSTURE control — standing per-fleet aggression, for a strike-capable fleet
// (a raider aboard). Composes with the corp doctrine (which decides the odds).
export function postureSection(g: GhostView): string {
  if (!g.composition?.some((c) => c.kind === "raider")) return ""; // needs strike capability
  const cur = postureModes.get(g.id) ?? g.posture ?? "passive";
  const btn = (m: EngagementPosture, label: string, hint: string) =>
    `<button class="act${cur === m ? " is-on" : ""}" data-act="posture" data-mode="${m}" title="${esc(hint)}">${esc(label)}</button>`;
  // Short labels; each posture's full description is its button tooltip (§UX-diet).
  return `<div class="sp-sec">${icon("posture", "sm")} Posture</div><div class="sp-line">${POSTURE_META.map((p) => btn(p.key, p.label, p.hint)).join(" ")}</div>`;
}


export function jumpSection(g: GhostView): string {
  if (!jumpCapable(g) || !state.galaxy) return "";
  const armed = intentAiming.jump === g.id;
  const range = Math.round(state.galaxy.jump_range).toLocaleString();
  return `<div class="sp-line"><button class="act${armed ? " is-on" : ""}" data-act="jump" ` +
    `title="Choose a served-picture destination within ${range} su. Both ends must be clear of gravity wells; the sim validates the true fleet when the delayed order arrives.">` +
    `${icon("jump", "md")} Set jump destination · J</button></div>` +
    `<div class="sp-line dim" title="Both ends must be clear of gravity wells.">Range ${range} su</div>`;
}


export function guardSection(g: GhostView): string {
  if (!guardCapable(g)) return "";
  const armed = intentAiming.guard === g.id;
  const target = g.guard_target
    ? state.ghosts.find((candidate) => candidate.id === g.guard_target && candidate.own)
    : undefined;
  const assignment = g.guard_target
    ? `<div class="sp-line action-line">${icon("fleet", "md")}<span>Guarding <b>${esc(target ? `${shipKindLabel(target.kind)} fleet` : "assigned fleet")}</b>` +
      `</span></div>`
    : `<div class="sp-line dim">No fleet assigned.</div>`;
  return assignment +
    `<div class="sp-line"><button class="act${armed ? " is-on" : ""}" data-act="guard" ` +
    `title="Choose another one of your fleet markers. The order is light-delayed; defensive reactions are local once it arrives.">` +
    `${icon("fleet", "md")} ${g.guard_target ? "Reassign guard" : "Guard a fleet"}</button></div>`;
}


// Flagship precedence (drawn/named order) — also the composition display order.
export const FLAGSHIP_ORDER: ShipKind[] = ["titan", "dreadnought", "battleship", "cruiser", "destroyer", "colony", "convoy", "corvette", "raider", "scout"];


// The COMPOSITION section of the fleet panel — mirrors the §13.1 intel ladder:
/// §upkeep: an UNSUPPLIED fleet is immobilized — it declines new movement and
/// offensive orders until Provisions reach it. Owner-only, and loud: without this
/// line a refused order just looks like the click did nothing.
export function supplyLine(g: GhostView): string {
  if (!g.own || g.supplied !== false) return "";
  return `<div class="sp-line"><span class="negative" title="This fleet has run out of Provisions. It keeps its guns and its current order and loses nothing — but it will not set out again until food reaches a system near it. Ship Provisions toward it, or bring it home.">` +
    `${icon("warning", "sm")} <b>out of supply</b> — immobilized until fed</span></div>`;
}


/// §roster: how beaten up the formation is. Rides the composition gate, so a
/// rival's condition shows only from inside sensor coverage — finding a wounded
/// fleet is real intel. An AGGREGATE: the per-hull roster never leaves the sim.
export function damageLine(g: GhostView): string {
  const d = g.damage ?? 0;
  if (d <= 0.001) return "";
  const pct = Math.round(d * 100);
  const tone = d > 0.5 ? "negative" : d > 0.2 ? "warn" : "dim";
  const word = d > 0.5 ? "crippled" : d > 0.2 ? "mauled" : "scratched";
  const who = g.own ? "Dock at an Ordnance Foundry to repair." : "A wounded formation is the best target on the map.";
  return `<div class="sp-line"><span class="${tone}" title="${esc(pct + "% of this formation's hull is gone. Damage persists between battles until a foundry services it. " + who)}">` +
    `hull <b>${100 - pct}%</b> — ${word}</span></div>`;
}


// full composition for own fleets and rivals inside sensor coverage; a bucket-only
// estimate ("est. 4–7 ships — composition unknown") outside coverage.
export function compositionSection(g: GhostView): string {
  if (g.composition && g.composition.length) {
    const items = [...g.composition]
      .sort((a, b) => FLAGSHIP_ORDER.indexOf(a.kind) - FLAGSHIP_ORDER.indexOf(b.kind))
      .map((c) => `${esc(shipKindLabel(c.kind))} <b>×${c.count}</b>`)
      .join(" · ");
    const total = g.composition.reduce((a, c) => a + c.count, 0);
    // §ladder B4: the OWNER's Titan row carries the christened flagship name —
    // plus the christening button (any member; empty un-names).
    let flagship = "";
    if (g.own && g.composition.some((c) => c.kind === "titan" && c.count > 0)) {
      const name = state.syndicate?.flagship_name;
      flagship = `<div class="sp-line">${icon("fleet", "sm")} flagship: <b>${name ? esc(name) : "<span class=\"dim\">unnamed</span>"}</b> ` +
        `<button class="act" style="width:auto;padding:2px 7px;font-size:11px" data-act="nameflagship" title="Christen your syndicate's Titan — the name shows on your fleet and in participant battle records.">${name ? "rename" : "name it"}…</button></div>`;
    }
    return `<div class="sp-sec">Composition</div><div class="sp-line">${items} <span class="dim">(${total} ship${total > 1 ? "s" : ""})</span></div>${flagship}${damageLine(g)}${supplyLine(g)}${splitControls(g)}`;
  }
  return `<div class="sp-sec">Composition</div><div class="sp-line dim">${icon("unknown", "sm", "Composition unknown — this fleet is out of your sensor range, so you have only the size estimate, never the exact makeup.")} est. <b>${countClassLabel(g.count_class)}</b> ships</div>`;
}


// Split controls belong to the fleet payload: they change the composition being
// described, rather than the standing policy or the merge-only management fold.
export function splitControls(g: GhostView): string {
  if (!g.own) return "";
  const comp = g.composition ?? [];
  const total = comp.reduce((a, c) => a + c.count, 0);
  if (total < 2) return "";
  const buttons = [...comp]
    .sort((a, b) => FLAGSHIP_ORDER.indexOf(a.kind) - FLAGSHIP_ORDER.indexOf(b.kind))
    .filter((c) => c.count >= 1)
    .map((c) => `<button class="act" data-act="split" data-kind="${c.kind}" title="Detach one ${esc(shipKindLabel(c.kind))} into a new fleet (at an owned system)">Split 1 ${esc(shipKindLabel(c.kind))}</button>`)
    .join("");
  return `<div class="sp-split">${buttons}</div>`;
}


// Fleet management now owns only MERGE. It is revealed behind a persistent
// module-state fold; split stays beside the composition it changes.
export function fleetManagementSection(g: GhostView): string {
  const merge = coLocatedOwnFleet(g);
  if (!merge) return "";
  const officer = g.captain ?? merge.captain;
  const twoOfficers = !!g.captain && !!merge.captain;
  const mergedLoad = fleetCommandLoad(g) + fleetCommandLoad(merge);
  const overAuthority = !!officer && mergedLoad > officer.command_capacity;
  const authority = officer
    ? ` Combined command load ${mergedLoad}/${officer.command_capacity} for ${captainTitle(officer.title)} ${officer.name}.`
    : " Neither formation carries your Flag Officer, so no Captain bonuses apply.";
  const reason = twoOfficers
    ? "Both formations have an assigned officer. Return one officer to reserve before merging."
    : overAuthority
    ? `Requires ${mergedLoad} command points; this officer has ${officer!.command_capacity}. Gain rank or merge a smaller formation.`
    : `Merge the co-located fleet into this one — works only at one of your owned systems (idle).${authority}`;
  const blocked = twoOfficers || overAuthority;
  const label = twoOfficers ? "Reserve one officer to merge" : overAuthority ? "Rank too low to merge" : "Merge co-located fleet";
  return `<button class="act" data-act="merge" data-from="${merge.id}" ${blocked ? "disabled" : ""} title="${esc(reason)}">${icon("fleet", "sm")} ${label}</button>`;
}


// Heading arrow + speed, computed in SCREEN space so it matches the map exactly.
export function headingCell(g: GhostView): string {
  const sp = Math.hypot(g.vel.x, g.vel.y);
  if (sp < 0.5) return stat("Heading", `<span class="dim">stationary</span>`);
  const p0 = renderer.worldToScreen(g.pos);
  const p1 = renderer.worldToScreen({ x: g.pos.x + g.vel.x, y: g.pos.y + g.vel.y });
  const deg = (Math.atan2(p1.y - p0.y, p1.x - p0.x) * 180) / Math.PI;
  return stat("Heading", `<span class="sp-arrow" aria-hidden="true" style="transform:rotate(${deg.toFixed(0)}deg)">➤</span> ${sp.toFixed(0)} su/s`);
}


// The drive row is evaluated in the sighting's own retarded frame. Warp's
// short spool/drop transitions read as impulse until cruise is established.
export function regimeCell(g: GhostView): string {
  const sp = g.speed ?? Math.hypot(g.vel.x, g.vel.y);
  const d = g.drive ?? "thrusters";
  const cruising = typeof d === "object" && "cruising" in d ? d.cruising : null;
  const spooling = typeof d === "object" && "spooling" in d ? d.spooling : null;
  const dropping = typeof d === "object" && "dropping" in d ? d.dropping : null;
  if (g.jump_presumed) {
    const report = Math.max(0, Math.ceil(g.jump_presumed.report_in));
    const tip = `The departure report proves the jump occurred, but no report emitted at the destination has arrived yet. Expected in about ${report}s.`;
    return stat("Drive", `<span style="color:var(--warn)" title="${esc(tip)}">Jump complete · awaiting report</span>`);
  }
  if (g.jump_spool) {
    const reported = Math.max(0, g.jump_spool.remaining);
    const age = Math.max(0, g.age);
    if (g.jump_spool.waiting_for_fuel) {
      const tip = `Delayed jump telemetry (${age.toFixed(1)}s old): the spool completed but the fleet was waiting for fuel when this report left.`;
      return stat("Drive", `<span style="color:var(--warn)" title="${esc(tip)}">Jump Drive Ready · awaiting fuel</span>`);
    }
    const seconds = Math.max(1, Math.ceil(reported));
    const tip = `Delayed jump telemetry (${age.toFixed(1)}s old): ${reported.toFixed(1)}s remained when this report left. The map snaps when departure light reaches command.`;
    return stat("Drive", `<span style="color:var(--warn)" title="${esc(tip)}">Jump Drive Spooling · ${seconds}s</span>`);
  }
  if (g.stalled) {
    const tip = g.rescue_inbound
      ? "Fuel exhausted. Authority Astral Assistance has dispatched a physical tender; this fleet's held order will resume after fuel transfer."
      : "Fuel exhausted. The fleet is holding its current order and will resume when refuelled.";
    return stat("Drive", `<span style="color:var(--warn)" title="${esc(tip)}">Fuel exhausted · holding</span>`);
  }
  if (cruising === "warp") {
    const tip = "Warp drive: five times impulse, and it flies anywhere — no lane needed, no heading to hold.";
    return stat("Drive", `<span title="${esc(tip)}">Warp</span>`);
  }
  // Everything else is IMPULSE — parked, crawling, mid-warp-transition, or
  // pinned inside a gravity well. The tooltip says which.
  if (sp < 0.5) {
    return stat("Drive", `<span class="dim" title="Holding station — impulse engines idle.">Impulse</span>`);
  }
  // §course-change: WHY impulse, when it is the well's doing. Computed from
  // the ghost's own retarded position against the (static) star chart, so the
  // reason shown can never disagree with the delayed state beside it — no wire
  // field, no staleness to manage.
  if (state.galaxy) {
    const well = state.galaxy.systems.find(
      (sys) => Math.hypot(sys.pos.x - g.pos.x, sys.pos.y - g.pos.y) <= HYPERLIMIT_SU,
    );
    if (well) {
      const tip = `Inside ${well.name}'s gravity well: no drive can light within ${HYPERLIMIT_SU} su of a star. The ship crawls clear on impulse, then the warp drive spools up.`;
      return stat("Drive", `<span class="dim" title="${esc(tip)}">Impulse · in ${esc(well.name)}'s gravity well</span>`);
    }
  }
  const tip = spooling
    ? `Impulse while the warp drive spins up — ${spooling.left.toFixed(1)}s to go.`
    : dropping
      ? `Impulse while the warp drive shuts down — ${dropping.left.toFixed(1)}s to go.`
      : "Impulse engines only — running with the warp drive shut off.";
  return stat("Drive", `<span class="dim" title="${esc(tip)}">Impulse</span>`);
}


// Inferred activity for an OWN ship — there is NO server order field, so this reads
// purely from the client's own overlays (raids/orders/command signals/route/vel).
export function ownActivity(g: GhostView): string {
  const a = (key: IconKey, label: string, tip: string) => `${icon(key, "sm", tip)} <b>${label}</b>`;
  if (state.commandSignals.some((s) => s.shipId === g.id)) return a("delivered", "signal outbound", "Your command is still crossing space to this fleet.");
  if (g.jump_presumed) return a("delay", "presumed at jump destination", "Awaiting the first report emitted at the new location.");
  if (g.jump_spool) return a("move", g.jump_spool.waiting_for_fuel ? "jump ready · awaiting fuel" : "jump drive spooling", "Delayed telemetry from this fleet's jump drive.");
  if (g.rescue_inbound) return a("fuel", "AAA rescue active", "An Authority Astral Assistance tender is physically en route from the Wormhole Hub.");
  if (g.stalled) return a("fuel", "out of fuel · holding", "The fleet keeps its current order and resumes when emergency fuel reaches it.");
  if (g.job) {
    const pct = Math.round(g.job.progress * 100);
    return g.job.kind === "demolishing"
      ? a("raid", `demolishing ${pct}%`, "Tearing down a rival structure — committed until the work ends.")
      : a("build", `constructing ${pct}%`, "Raising a structure where it stands — committed until the work ends.");
  }
  if (state.raids[g.id]) return a("raid", "raiding", "Pursuing a rival contact. Press R to recall (break off).");
  if (state.orders[g.id]) return a("move", "en route", "Proceeding on your last move order.");
  if (g.route && g.route.length) return a("convoy", "hauling", "En route along its trade route.");
  if (Math.hypot(g.vel.x, g.vel.y) < 0.5) return `<span class="dim">holding station</span>`;
  return a("move", "under way", "Under way.");
}


export function shipZone(title: string, body: string, modifier = ""): string {
  if (!body) return "";
  return `<section class="sp-zone${modifier ? ` ${modifier}` : ""}"><div class="sp-zone__title">${esc(title)}</div>${body}</section>`;
}


export type UxTabOption<T extends string> = readonly [key: T, label: string, iconKey?: IconKey];


/// One compact navigation grammar for the System, Planet, and Ship panels.
/// Panels render only the active task surface; rules and inactive controls do
/// not remain in the scroll merely because they exist somewhere in the game.
export function uxTabBar<T extends string>(
  tabs: readonly UxTabOption<T>[],
  active: T,
  dataKey: string,
): string {
  return `<div class="ux-tabs" role="tablist">${tabs.map(([key, label, iconKey]) =>
    `<button type="button" role="tab" aria-selected="${key === active}" class="ux-tab${key === active ? " is-active" : ""}" ` +
    `data-${dataKey}="${key}">${iconKey ? icon(iconKey, "sm", label) : ""}<span>${esc(label)}</span></button>`,
  ).join("")}</div>`;
}


export function jobProgress(g: GhostView): string {
  // Jump spool stays separate from JobView: it is delayed fleet telemetry shown
  // in the Drive row, while this channel describes committed construction work.
  if (!g.job) return "";
  const pct = Math.max(0, Math.min(100, g.job.progress * 100));
  const wrecking = g.job.kind === "demolishing";
  const verb = wrecking ? "Demolishing" : "Constructing";
  return `<div class="sp-job"><div class="sp-job__meta"><span>${verb}</span><b>${pct.toFixed(0)}%</b></div>${bar(pct, wrecking ? "is-negative" : "")}</div>`;
}


export function fuelSection(g: GhostView): string {
  const fuel = Math.max(0, g.fuel ?? 0);
  const capacity = fleetFuelCapacity(g);
  const logisticsMult = g.captain
    ? 1 - Math.min(0.10, Math.max(0, g.captain.attributes.logistics - 1) * 0.02)
    : 1;
  const rate = FUEL_PER_MASS_DISTANCE * 1000 * shipMass(g) * logisticsMult;
  const dest = state.orders[g.id] ?? g.path?.at(-1)?.pos;
  let primary = g.fuel == null ? "Tank report unavailable" : `Tank ${fmt(fuel)} / ${fmt(capacity)}`;
  let secondary = `burn ~${(rate / WARP_FACTOR).toFixed(1)}/1k su in warp`;
  if (dest) {
    const cost = estimatedFuelForLeg(g, dest);
    secondary = `current leg needs ~${fmt(cost)}`;
  }
  const pct = capacity > 0 ? Math.max(0, Math.min(100, fuel / capacity * 100)) : 0;
  const tank = g.fuel == null
    ? chip("fuel", "delayed telemetry", "The server has not served a tank reading for this fleet yet.", "sm")
    : chip("fuel", `${pct.toFixed(0)}%`, `Carried fuel at this fleet's latest served sighting (${g.age.toFixed(1)}s information delay).`, "sm");
  return `<div class="sp-sec">${icon("fuel", "sm")} Fuel</div><div class="sp-fuel">` +
    `<span class="sp-fuel__burn" title="Tank level and trip estimate come from this fleet's latest information-delayed report."><b>${primary}</b><small>${secondary}</small></span>` +
    `${tank}</div>`;
}


export function captainSection(g: GhostView): string {
  const c = g.captain;
  if (!c) {
    if (!g.own) return "";
    const load = fleetCommandLoad(g);
    return `<section class="sp-zone"><div class="sp-zone__title">Command authority</div>` +
      `<div class="sp-note">No officer assigned · formation load <b>${load}</b>. Captain bonuses are inactive.</div>` +
      `<button class="act" data-act="open-officers">Open Officer Corps</button></section>`;
  }
  const floor = captainXpFloor(c.level);
  const span = Math.max(1, c.next_level_xp - floor);
  const progress = c.level >= 10 ? 100 : Math.max(0, Math.min(100, ((c.xp - floor) / span) * 100));
  const homeSystem = state.commandCenter && state.galaxy?.systems.find(
    (system) => Math.hypot(system.pos.x - state.commandCenter!.x, system.pos.y - state.commandCenter!.y) < 1,
  )?.id;
  const canTrain = c.unspent > 0 && !!homeSystem && g.docked === homeSystem;
  const stat = (attribute: CaptainAttribute, label: string, effect: string): string => {
    const value = c.attributes[attribute];
    const add = c.unspent > 0
      ? `<button class="captain-stat__add" data-act="train-captain" data-captain-id="${c.id}" data-attribute="${attribute}" ${canTrain ? "" : "disabled"} title="${esc(canTrain ? `Train ${label} by one point.` : "Captain training is available only while berthed at your home command center.")}">+</button>`
      : "";
    return `<div class="captain-stat"><span>${esc(label)}</span><b>${value}</b>${add}<small>${esc(effect)}</small></div>`;
  };
  const a = c.attributes;
  const stats =
    stat("command", "Command", `+${Math.min(10, Math.max(0, a.command - 1) * 2)}% battle damage`) +
    stat("navigation", "Navigation", `−${Math.min(10, Math.max(0, a.navigation - 1) * 2)}% jump spool`) +
    stat("fieldcraft", "Fieldcraft", `−${Math.min(15, Math.max(0, a.fieldcraft - 1) * 3)}% survey dwell`) +
    stat("logistics", "Logistics", `−${Math.min(10, Math.max(0, a.logistics - 1) * 2)}% fuel burn`);
  const points = c.unspent > 0
    ? `<div class="captain-points">${c.unspent} training point${c.unspent === 1 ? "" : "s"}${canTrain ? " available" : " · return to home CC"}</div>`
    : "";
  const commandLoad = fleetCommandLoad(g);
  const commandFree = c.command_capacity - commandLoad;
  const commandTip = "Command points price coordination burden by hull: support craft 1, Interceptor 2, Corvette/Colony/Transport 4, Destroyer 8, Cruiser 16, Battleship 32, Dreadnought 64, Titan 128. Rank increases capacity.";
  const commandLine = commandFree >= 0
    ? `<div class="captain-xp__copy" title="${esc(commandTip)}">Command authority <b>${commandLoad} / ${c.command_capacity}</b> · ${commandFree} free</div>`
    : `<div class="captain-xp__copy warn" title="${esc(commandTip)}">Over command authority <b>${commandLoad} / ${c.command_capacity}</b> · bonuses suspended until split or promoted</div>`;
  const titledName = `${captainTitle(c.title)} ${c.name}`;
  return `<section class="sp-zone sp-captain"><img class="captain-portrait" src="/art/captains/${c.portrait}_${c.portrait_age}.png" alt="Portrait of ${esc(titledName)}" />` +
    `<div class="captain-card"><div class="sp-zone__title">Officer</div>` +
    `<div class="captain-name"><b>${esc(titledName)}</b><span>Level ${c.level}</span></div>` +
    commandLine +
    `<div class="captain-xp"><span style="width:${progress.toFixed(1)}%"></span></div>` +
    `<div class="captain-xp__copy">${c.level >= 10 ? "maximum level" : `${c.xp.toLocaleString()} / ${c.next_level_xp.toLocaleString()} XP`} · ${g.age.toFixed(1)}s old</div>` +
    `<div class="captain-stats">${stats}</div>${points}<button class="act" data-act="open-officers">Manage assignment</button></div></section>`;
}


export function officerCard(entry: CaptainRosterView, home: string | null): string {
  const report = entry.report;
  const assigned = entry.assigned_fleet
    ? state.ghosts.find((ghost) => ghost.id === entry.assigned_fleet && ghost.own)
    : undefined;
  const recovering = entry.recovering_until !== null && entry.recovering_until > state.simTime;
  const killed = entry.loss_fate === "killed";
  // Officers are physical: assignment and reserve changes require the officer
  // and formation to share any owned dock. Academy training remains a separate
  // home-command-center action below.
  const station = entry.assigned_fleet === null
    ? entry.stationed_system
    : assigned && state.systems.find((system) =>
        system.owner === state.playerId && dockedAtSystem(assigned, system.id))?.id;
  const local = !recovering && !killed && !!station && (
    entry.assigned_fleet === null
      || (!!assigned && Math.hypot(assigned.vel.x, assigned.vel.y) < 0.5)
  );
  const canTrain = local && station === home;
  const occupied = new Set(state.captains.flatMap((captain) => captain.assigned_fleet ? [captain.assigned_fleet] : []));
  const eligible = report && station
    ? state.ghosts.filter((fleet) =>
        fleet.own
        && dockedAtSystem(fleet, station)
        && Math.hypot(fleet.vel.x, fleet.vel.y) < 0.5
        && (!occupied.has(fleet.id) || fleet.id === entry.assigned_fleet)
        && fleetCommandLoad(fleet) <= report.command_capacity
        && fleet.id !== entry.assigned_fleet)
    : [];
  const status = killed
    ? "Killed in action"
    : recovering
    ? `${entry.loss_fate === "captured" ? "Captured · repatriation" : entry.loss_fate === "injured" ? "Injured · recovery" : "Rescue underway"} · ${fmtDur(entry.recovering_until! - state.simTime)}`
    : entry.assigned_fleet
      ? assigned ? `Assigned · ${officerFleetName(assigned)}` : "Assigned · report in transit"
      : entry.stationed_system ? `Reserve · ${systemName(entry.stationed_system)}` : "Reserve";
  const tone = killed ? "negative" : recovering ? "warn" : entry.assigned_fleet ? "accent" : "positive";
  const portraitAge = report?.portrait_age ?? "young";
  const titled = report ? `${captainTitle(report.title)} ${entry.name}` : entry.name;
  const floor = report ? captainXpFloor(report.level) : 0;
  const span = report ? Math.max(1, report.next_level_xp - floor) : 1;
  const progress = report ? (report.level >= 10 ? 100 : Math.max(0, Math.min(100, ((report.xp - floor) / span) * 100))) : 0;
  const xp = report
    ? `${report.level >= 10 ? "maximum level" : `${report.xp.toLocaleString()} / ${report.next_level_xp.toLocaleString()} XP`} · authority ${report.command_capacity}`
    : "Personnel light has not reached command";
  const stats = report
    ? `Cmd ${report.attributes.command} · Nav ${report.attributes.navigation} · Field ${report.attributes.fieldcraft} · Log ${report.attributes.logistics}`
    : "Attributes pending";
  const train = report && report.unspent > 0
    ? `<div class="officer-card__train">${(["command", "navigation", "fieldcraft", "logistics"] as CaptainAttribute[]).map((attribute) =>
        `<button data-officer-act="train" data-captain-id="${entry.id}" data-attribute="${attribute}"${canTrain ? "" : " disabled"} title="${esc(canTrain ? `Train ${attribute} by one point.` : "Captain training is available only at the home command center.")}">+ ${attribute}</button>`).join("")}</div>`
    : "";
  let actions = "";
  if (!recovering && report && local) {
    const options = eligible.map((fleet) =>
      `<option value="${esc(fleet.id)}">${esc(officerFleetName(fleet))} · load ${fleetCommandLoad(fleet)}/${report.command_capacity}</option>`).join("");
    if (options) {
      actions += `<div class="officer-card__actions"><select id="officer-assign-${entry.id}">${options}</select>` +
        `<button class="act" data-officer-act="assign" data-captain-id="${entry.id}">${entry.assigned_fleet ? "Transfer" : "Assign"}</button></div>`;
    }
    if (entry.assigned_fleet) {
      actions += `<button class="act" data-officer-act="reserve" data-captain-id="${entry.id}">Return to reserve</button>`;
    }
  }
  if (assigned) {
    actions += `<button class="act" data-officer-act="select-fleet" data-fleet="${esc(assigned.id)}">Select formation</button>`;
  }
  return `<article class="officer-card"><img class="officer-card__portrait" src="/art/captains/${entry.portrait}_${portraitAge}.png" alt="Portrait of ${esc(titled)}" />` +
    `<div><div class="officer-card__name"><b>${esc(titled)}</b>${badge(tone, status)}</div>` +
    `<div class="officer-card__meta">${esc(stats)}</div><div class="officer-card__bar"><span style="width:${progress.toFixed(1)}%"></span></div>` +
    `<div class="officer-card__xp">${esc(xp)}</div>${train}${actions}</div></article>`;
}


export function updateOfficersPanel(): void {
  const root = $("tab-officers");
  const home = foundingHomeSystemId();
  const system = home ? state.systems.find((entry) => entry.id === home) : undefined;
  const academyTier = system?.structures?.academy ?? 0;
  const pending = system?.builds?.filter((job) => job.key === "officer_commission").length ?? 0;
  const living = state.captains.filter((captain) => captain.loss_fate !== "killed").length;
  const memorials = state.captains.length - living;
  const used = living + pending;
  const canRecruit = !!home && academyTier > 0 && used < state.captainCapacity;
  const roster = state.captains.map((captain) => officerCard(captain, home)).join("");
  setHtml(root,
    `<div class="panel-title"><div><div class="eyebrow">personnel · physical command</div><h2>Officer Corps</h2></div></div>` +
    `<div class="officer-summary"><span>${living} active · ${pending} commissioning${memorials ? ` · ${memorials} memorial` : ""}</span><b>${used} / ${state.captainCapacity} berths</b></div>` +
    `<button class="act act--primary" data-officer-act="recruit"${canRecruit ? "" : " disabled"}>Commission Lieutenant · Academy ${academyTier || "required"}</button>` +
    `<div class="mhint">60 s · 40 Provisions · 20 Electronics · 10 Machinery. Each home Academy tier adds one berth beyond the founding commission.</div>` +
    (roster || `<div class="mhint">No officer reports available.</div>`) +
    `<div class="officer-doctrine"><b>Career XP</b> · combat 60–85 · survey 35 · delivery 20 · jump 12. Junior officers efficiently cover several light formations; senior ranks unlock one concentrated capital command. A formation loss can mean rescue, injury, capture, or death; the outcome reaches the roster only with the casualty report.</div>`);
}


export function authorityFreightSection(g: GhostView): string {
  if (g.engage_freight === null || g.engage_freight === undefined) return "";
  const on = g.engage_freight;
  return `<div class="sp-sec">${icon("authorityFreighter", "sm")} Authority freight</div>` +
    `<div class="sp-line"><button class="act${on ? " is-on" : ""}" data-act="engage-freight" data-on="${on ? "0" : "1"}" ` +
    `title="While blockading, also engage Terran Charter Authority freighters arriving here. OFF: they land and unload through your blockade — a small leak, self-limiting because the Authority already refuses NEW bookings to a blockaded system. ON: an arriving freighter becomes an ordinary hostile contact.">` +
    `${on ? "Engaging" : "Ignoring"} Authority freight arriving here</button></div>`;
}


export function standingPolicyZone(g: GhostView): string {
  const hasPosture = !!g.composition?.some((c) => c.kind === "raider");
  const hasFreight = g.engage_freight !== null && g.engage_freight !== undefined;
  const garrison = garrisonSection(g);
  if (!hasPosture && !hasFreight && !garrison) return "";
  const summary: string[] = [];
  if (hasPosture) {
    const cur = postureModes.get(g.id) ?? g.posture ?? "passive";
    summary.push(`Posture ${POSTURE_META.find((p) => p.key === cur)?.label ?? "Passive"}`);
  }
  if (hasFreight) summary.push(`Freight ${g.engage_freight ? "engaging" : "ignoring"}`);
  if (g.garrison_host) summary.push(`Garrison ${g.garrison_fed === false ? "unfed" : "fed"}`);
  const open = expandedShipPolicies.has(g.id);
  const controls = postureSection(g) + authorityFreightSection(g);
  return `<section class="sp-zone sp-zone--fold"><button class="sp-fold" data-act="toggle-policy" aria-expanded="${open}">` +
    `<span><span class="sp-zone__title">Standing policy</span><span class="sp-fold__summary">${esc(summary.join(" · "))}</span></span>` +
    `<span class="sp-fold__chev" aria-hidden="true">${open ? "▾" : "▸"}</span></button>${garrison}` +
    (open && controls ? `<div class="sp-fold__body">${controls}</div>` : "") + `</section>`;
}


export function managementZone(g: GhostView): string {
  const controls = fleetManagementSection(g);
  if (!controls) return "";
  const open = expandedShipManagement.has(g.id);
  return `<section class="sp-zone sp-zone--fold"><button class="sp-fold" data-act="toggle-management" aria-expanded="${open}">` +
    `<span><span class="sp-zone__title">Fleet management</span><span class="sp-fold__summary">Merge co-located fleets</span></span>` +
    `<span class="sp-fold__chev" aria-hidden="true">${open ? "▾" : "▸"}</span></button>` +
    (open ? `<div class="sp-fold__body">${controls}</div>` : "") + `</section>`;
}


// OWN ship: NOW first, then orders, payload, and contextual verbs. The jump
// drive is deliberately exposed here because it is an explicit two-step map
// command, not a standing policy toggle.
export function ownBody(g: GhostView): string {
  const payload: string[] = [compositionSection(g)];
  if (hauls(g)) {
    const manifest = fleetCargoManifest(g);
    const used = fleetCargoUnits(g);
    const capacity = fleetCargoCapacity(g);
    const free = Math.max(0, capacity - used);
    const full = capacity > 0 && used >= capacity;
    const cargo = manifest.length
      ? manifest.map((stack) => `<div class="sp-cargo">${commodityIcon(stack.commodity, "md")} <b>${fmt(stack.units)}</b> ${esc(label(stack.commodity))}</div>`).join("")
      : `<span class="dim">empty hold</span>`;
    payload.push(
      `<div class="sp-sec">${icon("cargo", "sm")} Cargo hold · ${fmt(used)} / ${fmt(capacity)} units</div>` +
      `<div class="storage-row">${bar(capacity > 0 ? (used / capacity) * 100 : 0, full ? "is-warn" : "")}` +
      `<span class="storage-warn">${full ? badge("warn", "hold full") : `${fmt(free)} units free`}</span></div>${cargo}`,
    );
    if (g.route && g.route.length) {
      const d = g.route[g.route.length - 1];
      payload.push(`<div class="sp-sec">${icon("freightRoute", "sm")} Route</div><div class="sp-line" title="The waypoints this freighter will fly; the last is its destination.">${g.route.length} leg${g.route.length > 1 ? "s" : ""} → (${d.x.toFixed(0)}, ${d.y.toFixed(0)})</div>`);
    }
    payload.push(logisticsSection(g));
  }
  payload.push(fuelSection(g));
  const commands = dockingSection(g) +
    fuelRescueSection(g) +
    (guardCapable(g) ? shipZone("Escort", guardSection(g), "sp-zone--guard") : "") +
    (jumpCapable(g) ? shipZone("Jump drive", jumpSection(g), "sp-zone--jump") : "") +
    (g.kind === "builder" ? shipZone("Construct", emplaceSection(g), "sp-zone--construct") : "");

  const tabs: readonly UxTabOption<ShipPanelTab>[] = [
    ["orders", "Orders", "move"],
    ["fleet", "Fleet", "fleet"],
    ["officer", "Officer", "commandCenter"],
  ];
  const orders = ordersZone(g) +
    (commands ? `<div class="sp-command-group"><div class="sp-command-group__title">Commands</div>${commands}</div>` :
      `<div class="sp-empty">No contextual commands available.</div>`);
  const fleet = shipZone("Fleet", payload.join("")) + standingPolicyZone(g) + managementZone(g);
  const officer = captainSection(g) || `<div class="sp-empty">No officer assigned.</div>`;
  const active = shipPanelTab === "orders" ? orders : shipPanelTab === "fleet" ? fleet : officer;

  // Activity is the one fact that matters in every task. Everything else lives
  // in one of three stable surfaces rather than one ever-growing vertical sheet.
  return `<div class="sp-current"><span class="sp-current__label">Now</span><span class="sp-current__activity">${ownActivity(g)}</span></div>` +
    jobProgress(g) + uxTabBar(tabs, shipPanelTab, "ship-tab") +
    `<div class="sp-tab-body">${active}</div>`;
}


export function dockingSection(g: GhostView): string {
  if (g.docked === "hub") {
    return shipZone("Docking", `<div class="sp-line action-line" title="A movement order undocks this fleet automatically.">${icon("dock", "md")}<span>Docked at <b>Wormhole Hub</b></span></div>`);
  }
  const dockedSystem = g.docked
    ? state.galaxy?.systems.find((system) => dockedAtSystem(g, system.id))
    : undefined;
  if (dockedSystem) {
    return shipZone("Docking", `<div class="sp-line action-line" title="A movement order undocks this fleet automatically.">${icon("dock", "md")}<span>Docked at <b>${esc(dockedSystem.name)}</b></span></div>`);
  }
  const target = nearestKnownDock(g);
  if (!target) return "";
  const distance = Math.round(target.distance).toLocaleString();
  return shipZone(
    "Docking",
    `<div class="sp-line"><button class="act act--primary" data-act="dock" ` +
      `title="Send this fleet to the nearest known valid berth. The order and the fleet's arrival both remain information-delayed.">` +
      `${icon("dock", "md")} Initiate Docking</button></div>` +
      `<div class="sp-line dim">${esc(target.name)} · ${distance} su</div>`,
  );
}


export function fuelRescueSection(g: GhostView): string {
  if (!g.own || (!g.stalled && !g.rescue_inbound)) return "";
  if (g.rescue_inbound) {
    return shipZone(
      "Authority Astral Assistance",
      `<div class="sp-line action-line">${icon("fuel", "md")}<span><b>AAA rescue active</b><br><span class="dim">Physical tender dispatched from the Wormhole Hub</span></span></div>`,
      "sp-zone--rescue",
    );
  }
  const quote = aaaEstimate(g);
  const affordable = (state.wallet?.credits ?? 0) + 1e-6 >= quote.cost;
  const price = state.market?.prices.find((entry) => entry.commodity === "fuel")?.price;
  const detail = price == null
    ? `~${fmt(quote.cost)} Cr · 3× Fuel price + ${fmt(AAA_SERVICE_FEE)} callout`
    : `~${fmt(quote.cost)} Cr · ${price.toFixed(1)} × 3 per Fuel + ${fmt(AAA_SERVICE_FEE)} callout`;
  return shipZone(
    "Out of fuel",
    `<div class="sp-line"><button class="act act--primary" data-act="fuel-rescue"${affordable ? "" : " disabled"} ` +
      `title="AAA dispatches a physical rescue tender from the Wormhole Hub. Payment is charged at dispatch and is non-refundable if the tender is lost.">` +
      `${icon("fuel", "md")} Call AAA rescue · ~${fmt(quote.cost)} Cr</button></div>` +
      `<div class="sp-line ${affordable ? "dim" : "warn"}">${affordable ? detail : `Need ~${fmt(quote.cost)} Cr · treasury ${fmt(state.wallet?.credits ?? 0)} Cr`}</div>`,
    "sp-zone--rescue",
  );
}


// §syndicates Part 3: if this OWN fleet is stationed as an ally GARRISON, show its
// host + supply state (fed = the host is covering its Provisions upkeep; UNFED =
// its defense is suspended until fed — nothing destroyed).
export function garrisonSection(g: GhostView): string {
  if (!g.own || !g.garrison_host) return "";
  const hostName = systemName(g.garrison_host);
  const fed = g.garrison_fed !== false;
  const tip = fed
    ? `Stationed at ally ${hostName}, joining its defense per your doctrine. The host is feeding this garrison its Provisions upkeep.`
    : `Stationed at ally ${hostName}, but the host is OUT of Provisions — this garrison's defense is SUSPENDED until fed (nothing is destroyed).`;
  return `<div class="sp-policy-note">${icon("garrison", "sm")} ` +
    `${badgeChip("garrison", `${hostName} · ${fed ? "fed" : "UNFED"}`, fed ? "positive" : "negative", tip)}</div>`;
}


// RIVAL ship: ONLY what's observable. A convoy broadcasts its route (light-delayed)
// and reveals cargo ONLY when inside your sensor coverage (cargo present). A raider
// runs dark. Never any order/intent/fuel/internal state.
export function rivalBody(g: GhostView): string {
  const parts: string[] = [];
  parts.push(compositionSection(g));
  if (g.kind === "convoy") {
    if (g.route && g.route.length) {
      const d = g.route[g.route.length - 1];
      parts.push(`<div class="sp-sec">Route</div><div class="sp-line" title="A freighter broadcasts its route under the Convention — light-delayed, like everything you see.">${g.route.length} leg${g.route.length > 1 ? "s" : ""} → (${d.x.toFixed(0)}, ${d.y.toFixed(0)}) <span class="dim">(broadcast)</span></div>`);
    }
    // Cargo ONLY when in sensor range (cargo present). NEVER shown otherwise.
    const manifest = fleetCargoManifest(g);
    parts.push(`<div class="sp-sec">${icon("cargo", "sm")} Cargo</div>` + (manifest.length
      ? manifest.map((stack) => `<div class="sp-line">${chip(stack.commodity as IconKey, `${fmt(stack.units)} ${esc(label(stack.commodity))}`, "Cargo — visible because this freighter is inside your sensor coverage.")}</div>`).join("")
      : `<div class="sp-line dim">${icon("unknown", "sm", "Cargo unknown — this freighter is out of your sensor range. It is revealed only inside your coverage.")} unknown</div>`));
  } else if (g.kind === "freighter") {
    if (g.rescue_service) {
      parts.push(
        `<div class="sp-sec">${icon("fuel", "sm")} Authority Astral Assistance</div>` +
        `<div class="sp-line"><b>AAA Rescue Tender</b><span class="dim">Emergency fuel service operating from the Wormhole Hub.</span></div>`,
      );
    } else if (g.migrant) {
      const cohort = state.galaxy?.migrant_cohort_people ?? 1_000;
      parts.push(
        `<div class="sp-sec">${icon("population", "sm")} Civilian passengers</div>` +
        `<div class="sp-line"><b>${cohort.toLocaleString()} migrants</b><span class="dim">One complete workforce cohort, travelling physically from the Wormhole Hub.</span></div>`,
      );
    } else {
    // §TCA: an Authority freighter BROADCASTS — it is a scheduled common carrier,
    // not a dark contact. Its MANIFEST is the two-tier surface the server already
    // fog-gates: your own lots always, everyone else's only from inside sensor
    // range (`revealed`). Rendering it is the whole point of shipping it.
    const mine = (g.manifest ?? []).filter((m) => m.mine);
    const theirs = (g.manifest ?? []).filter((m) => !m.mine);
    const row = (m: ManifestEntryView): string =>
      `<div class="sp-line">${chip(m.commodity as IconKey, `${fmt(m.units)} ${esc(label(m.commodity))}`, m.mine ? "Your lot — always legible to you, wherever this hull is." : "A rival's lot — legible because this freighter is inside your sensor coverage.")} <span class="dim">${m.direction === "outbound" ? "→ system" : "→ hub"}</span></div>`;
    parts.push(`<div class="sp-sec">${icon("cargo", "sm")} Manifest</div>`);
    if (mine.length) parts.push(mine.map(row).join(""));
    if (theirs.length) {
      parts.push(theirs.map(row).join(""));
    } else if (g.revealed) {
      if (!mine.length) parts.push(`<div class="sp-line dim">Riding empty.</div>`);
    } else {
      parts.push(`<div class="sp-line dim" title="Other corporations' lots are legible only from inside your sensor coverage. Yours are always legible.">${icon("unknown", "sm")} other lots unknown — out of sensor range</div>`);
    }
    }
    parts.push(
      `<div class="sp-sec">${icon("blockade", "sm")} Sanctuary</div>` +
      `<div class="sp-line dim" title="Destroying or intercepting an Authority hull is CITED — it costs charter standing, which prices your freight and your Exchange access. It is never forbidden, only expensive.">Attacking this is priced, not forbidden — it will be cited.</div>`,
    );
  } else {
    const tip = g.kind === "scout"
      ? "A scout runs silent — someone is LOOKING at your space. No cargo, no weapons. You see it only because it is within your sensor range right now."
      : "A raider runs silent — no route or cargo is observable. You see it only because it is within your sensor range right now.";
    parts.push(`<div class="sp-sec">${icon("stealth", "sm")} Dark contact</div><div class="sp-line dim" title="${esc(tip)}">${g.kind === "scout" ? "scout" : "raider"} — in sensor range</div>`);
    // §Part 4: how LOUD it is (signature) — a big pack at flank speed flares far out.
    if (g.signature != null) {
      const loud = g.signature >= 1.6 ? "running LOUD — flank speed and/or a big pack (flares far out)"
        : g.signature <= 0.6 ? "running quiet — creeping or small (you caught it close)"
        : "a moderate signature";
      parts.push(`<div class="sp-line">${chip("delay", `${g.signature.toFixed(2)}×`, `Detection signature — how LOUD this contact is: ${loud}.`)}</div>`);
    }
  }
  return shipZone("Payload", parts.join(""));
}


export function updateJumpDeparturePanel(root: HTMLElement, key: string): void {
  const departure = state.jumpDepartures.find((d) => jumpDepartureKey(d) === key);
  if (!departure) {
    // The marker's two-minute local retention expired. It is a transient clue,
    // not a permanent intelligence ledger, so its detail selection expires too.
    deselectShip();
    return;
  }
  const now = liveSimTime();
  const jumpedAgo = fmtCountdown(Math.max(0, now - departure.departed_at));
  const learnedAgo = fmtCountdown(Math.max(0, now - departure.learned_at));
  const delay = Math.max(0, departure.learned_at - departure.departed_at);
  const ownerName = departure.owner_name?.trim()
    || (departure.owner === state.playerId ? state.name : `Corporation ${formatId(departure.owner)}`);
  const mine = departure.owner === state.playerId;
  const ownership = mine ? badge("accent", "yours") : badge("negative", "rival");
  const title = `${shipKindLabel(departure.kind)} fleet`;
  const head =
    `<div class="sp-head"><div class="panel-title"><div><div class="eyebrow">jump departure · delayed light</div>` +
    `<h2>${svgIcon("concept-fleet", "md")} ${esc(title)}</h2></div><div class="panel-title__right">${ownership}</div></div>` +
    `<button class="sp-close" data-act="close" title="Deselect (Esc)" aria-label="Close">✕</button></div>`;
  const identity = statStrip([
    stat("Corporation", `<b>${esc(ownerName)}</b><br><span class="dim">${esc(formatId(departure.owner))}</span>`),
    stat("Fleet", `<b>${esc(departure.fleet)}</b>`),
    stat("Jumped", `<b>${esc(jumpedAgo)} ago</b><br><span class="dim">T+${departure.departed_at.toFixed(1)}s</span>`),
    stat("Report delay", `<b>${delay.toFixed(1)}s</b><br><span class="dim">received ${esc(learnedAgo)} ago</span>`),
    stat("Origin", `<b>${fmt(departure.pos.x)} · ${fmt(departure.pos.y)}</b>`),
  ], "sp-status-strip");
  const body =
    `${identity}<section class="sp-zone"><div class="sp-zone__title">Observed event</div>` +
    `<div class="sp-line"><b>${esc(ownerName)}</b>'s ${esc(title)} jumped away from this position.</div>` +
    `<div class="sp-line dim">The split chevrons are a historical departure report. They do not reveal the destination or claim the fleet is still here.</div></section>`;
  setHtml(root, head + `<div class="sp-body">${body}</div>`);
}


export function updateShipPanel(): void {
  if (renderDeferred("ship-panel", updateShipPanel)) return; // §single-click
  // §emplacements: the same dock shows a selected STRUCTURE. Handled first —
  // the two selections are mutually exclusive, so whichever is set owns the
  // panel this tick.
  if (state.selectedEmplacementId) {
    updateEmplacementPanel();
    return;
  }
  const root = $("ship-panel");
  if (jumpDepartureSelection.key) {
    updateJumpDeparturePanel(root, jumpDepartureSelection.key);
    return;
  }
  if (!state.selectedShipId) return;
  const g = state.ghosts.find((x) => x.id === state.selectedShipId);
  // §perf/wedge: while the player is working the dockside load controls — the
  // native <select> popup open, or typing a quantity — DON'T rebuild the whole
  // panel. A 10 Hz rebuild wipes the typed qty and wedges the <select> (the
  // Deliver-dropdown bug family). Reconcile the stock list in place while the
  // quantity owns focus, but freeze the native option list while the select
  // itself is open; mutating an open list makes browsers reset it to item one.
  const ae = document.activeElement;
  if (ae instanceof HTMLElement && root.contains(ae) && (ae.classList.contains("lg-com") || ae.classList.contains("lg-qty"))) {
    if (g?.own) syncDockLoadControls(root, g);
    return;
  }
  if (!g) {
    // No longer observable (passed beyond your sensors/light, or — a rival —
    // destroyed). Honest: we can't show what we can't see.
    root.innerHTML =
      `<div class="sp-head"><div class="panel-title"><div><div class="eyebrow">contact</div><h2>Contact lost</h2></div></div>` +
      `<button class="sp-close" data-act="close" title="Close" aria-label="Close">✕</button></div>` +
      `<div class="sp-body"><div class="sp-note" title="It has passed beyond your sensors and the last light to reach you — nothing more is observable.">Passed beyond your sensors.</div></div>`;
    return;
  }
  const own = g.own;
  // §TCA: an Authority hull is named for what it IS — a scheduled common carrier,
  // or an enforcement warship. Both fly the same flag; only one is a threat, and
  // both are neutral rather than "rival".
  const eyebrow = own
    ? "your fleet"
    : g.tca
      ? "Terran Charter Authority"
      : g.pirate
        ? "pirate contact"
        : g.kind === "raider"
          ? "dark contact"
          : "rival contact";
  const title = g.tca
    ? g.kind === "freighter"
      ? g.rescue_service ? "AAA Rescue Tender" : g.migrant ? "Authority Migrant Liner" : "Authority Freighter"
      : "Authority Enforcement"
    : g.pirate && g.kind === "raider"
      ? "Pirate Raider"
      : shipKindLabel(g.kind);
  const ownTag = own ? badge("accent", "yours") : g.tca ? badge("neutral", "neutral") : badge("negative", "rival");
  const informationDelay = g.jump_presumed?.information_delay ?? g.age;
  const stale = informationDelay >= CONTACT_STALE_AGE_S;
  const panelGhost = g;
  const roleLore = own ? shipRoleLore(g) : "";

  const head =
    `<div class="sp-head"><div class="panel-title"><div><div class="eyebrow">${esc(eyebrow)}</div>` +
    `<h2${roleLore ? ` title="${esc(roleLore)}"` : ""}>${svgIcon(g.kind === "convoy" || g.kind === "freighter" ? "concept-convoy" : "concept-fleet", "md")} ${esc(title)}${roleLore ? ` <span class="sp-role-info" aria-label="Role information" title="${esc(roleLore)}">ⓘ</span>` : ""}</h2></div><div class="panel-title__right">${ownTag}</div></div>` +
    `<button class="sp-close" data-act="close" title="Deselect (Esc)" aria-label="Deselect">✕</button></div>`;

  // Information delay is the headline stat. For a presumed jump this is the
  // delay at the authored destination, not the age of its departure proof.
  const ageCell = `<div class="stat sp-age ${stale ? "is-stale" : ""}"><dt>Information Delay</dt><dd>${informationDelay.toFixed(1)}s</dd></div>`;
  const strip = statStrip(
    [ageCell, regimeCell(panelGhost), headingCell(panelGhost)],
    "sp-status-strip",
  );
  // Preserve an in-progress dockside load selection/qty across the rebuild (the
  // fresh <input> would otherwise snap back to its default 50, the fresh <select>
  // to its first option) — the panel still rebuilds ~10 Hz to keep the age live.
  const prevQty = (root.querySelector(".lg-qty") as HTMLInputElement | null)?.value;
  const prevCom = (root.querySelector(".lg-com") as HTMLSelectElement | null)?.value;
  setHtml(root, head + `<div class="sp-body">${strip}${own ? ownBody(g) : rivalBody(g)}</div>`);
  if (prevQty !== undefined) {
    const q = root.querySelector(".lg-qty") as HTMLInputElement | null;
    if (q) q.value = prevQty;
  }
  if (prevCom) {
    const c = root.querySelector(".lg-com") as HTMLSelectElement | null;
    if (c && [...c.options].some((o) => o.value === prevCom)) c.value = prevCom;
  }
}


// §emplacements: what each standing structure IS, in the player's terms —
// the same sentence the build button promises, so a structure explains itself
// when clicked months after it was placed.
export const EMPLACEMENT_BLURB: Record<string, string> = {
  deep_space_sensor:
    "A stationary picket. Watches its bubble like a ship's sensors and reports home at warp speed.",
};


// §emplacements: the SELECTED STRUCTURE panel. Structures are stationary and
// yours, so there is no fog story to tell here — no Seen age, no uncertainty:
// the position IS where you put it.
export function updateEmplacementPanel(): void {
  const root = $("ship-panel");
  const e = state.emplacements.find((x) => x.id === state.selectedEmplacementId);
  if (!e) {
    // Destroyed, or a fresh View no longer lists it.
    setHtml(root, `<div class="sp-body"><div class="sp-line dim">That structure is no longer there.</div></div>`);
    return;
  }
  const name = emplacementLabel(e.kind);
  const mine = e.own !== false;
  const head =
    `<div class="sp-head"><div class="panel-title"><div><div class="eyebrow">${mine ? "Your structure" : "Rival structure"}</div>` +
    `<h2>${name}</h2></div><div class="panel-title__right">${badge(mine ? "accent" : "warn", mine ? "STANDING" : "HOSTILE")}</div></div>` +
    `<button class="sp-close" data-act="close" title="Deselect (Esc)" aria-label="Deselect">✕</button></div>`;
  const stats = statStrip([
    `<div class="stat"><dt>Position</dt><dd>${fmt(e.pos.x)} · ${fmt(e.pos.y)}</dd></div>`,
    `<div class="stat" title="Everything inside this radius is watched from here."><dt>Watches</dt><dd>${fmt(e.sensor_range)} su</dd></div>`,
  ], "sp-status-strip sp-status-strip--emplacement");
  const body =
    `<div class="sp-line dim">${esc(EMPLACEMENT_BLURB[e.kind] ?? "")}</div>` +
    (mine
      ? ""
      : // A rival's: say how to be rid of it. The verb lives on the map, in the
        // same grammar as raiding, so the panel teaches rather than adds a button.
        `<div class="sp-line dim">Seen from inside your sensor coverage. Select an <b>armed fleet</b>, ` +
        `then click this structure to send it to tear the structure down — it must hold station there to finish.</div>`);
  setHtml(root, head + `<div class="sp-body">${stats}${body}</div>`);
}


// §emplacements: the CONSTRUCTION SHIP's build section — BUILD-HERE
// buttons on the actor. The ship builds where it is parked: fly it to the
// spot with an ordinary move order, then press the button. The status line
// says what the current spot allows, so a refusal is never a surprise.
export function emplaceSection(g: GhostView): string {
  const kinds: [string, string, string][] = [
    ["deep_space_sensor", "Deep Space Sensor", "A stationary picket. Watches like a ship's sensors and reports home at warp. Stands anywhere."],
  ];
  // Busy = MID-CONSTRUCT (`build_progress`, own-only from the wire), an order signal
  // still in its lifecycle (pendingOrders — server-managed, so it EXPIRES),
  // or visibly moving. Not `state.orders`: that record used to persist after
  // arrival, which kept these buttons disabled forever once the ship had
  // moved anywhere — and a disabled button swallows clicks silently.
  const busy = !!g.job || state.pendingOrders.has(g.id) || Math.hypot(g.vel.x, g.vel.y) >= 0.5;
  // The spot's verdict, computed where the ship stands (it is parked when the
  // buttons are live, so the light-delayed position IS the position).
  const openOk = !renderer.siteError("deep_space_sensor", g.pos, state);
  const note = busy
    ? `<div class="sp-line dim">${g.job ? "Committed to the current job; new construction unlocks when it finishes." : "Under way — it builds where it stops, once idle."}</div>`
    : `<div class="sp-line dim">Builds at this spot. ${
        openOk
          ? "Open space — the sensor can stand here."
          : "Too close to another structure — move on a little."
      }</div>`;
  const btns = kinds
    .map(
      ([k, name, tip]) =>
        `<button class="emplace-btn" data-act="emplace" data-kind="${k}" title="${esc(`${tip} Kit: ${kitCostLabel(k)}, charged from one of your systems.`)}"${busy ? " disabled" : ""}>${name}</button>`,
    )
    .join(" ");
  return note + `<div class="sp-line">${btns}</div>`;
}


export const haulDestinationByFleet = new Map<EntityId, EntityId>();


export function dockLoadOptions(g: GhostView): string {
  return dockLoadStock(g)
    .filter(([, units]) => units > 0)
    .map(([commodity, units]) => `<option value="${esc(commodity)}">${esc(commodity)} (${units})</option>`)
    .join("");
}


/// The player may knowingly risk a marginal leg, but never discovers the tank
/// limit only after departure. Both numbers are from the latest served fleet
/// picture, so the confirmation is explicitly an estimate rather than truth.
export function confirmHaulFuel(g: GhostView, dest: Vec2, destinationName: string): boolean {
  if (g.fuel == null) return true; // rolling-server compatibility
  const needed = estimatedFuelForLeg(g, dest);
  if (g.fuel + 1e-6 >= needed) return true;
  return window.confirm(
    `Fuel warning\n\nLatest tank report: ${fmt(g.fuel)} Fuel.\n` +
      `Estimated need to ${destinationName}: ~${fmt(needed)} Fuel.\n\n` +
      `The Freighter may run dry and hold its current order. Emergency AAA service costs 3× the Fuel market price plus a ${fmt(AAA_SERVICE_FEE)}-credit callout fee.\n\nDepart anyway?`,
  );
}


// Keep the resource picker live without replacing the native control. An OPEN
// native select is the exception: even morphing only its option children at View
// cadence resets the popup's highlighted choice to item one in some browsers.
// Freeze it while focused, then catch up on the first View after blur. Outside
// that interaction window, preserve the chosen commodity whenever it still
// exists; a genuinely depleted selection naturally falls to the next good.
export function syncDockLoadControls(root: HTMLElement, g: GhostView): void {
  const select = root.querySelector(".lg-com") as HTMLSelectElement | null;
  if (!select) return;
  if (document.activeElement === select) return;
  const selected = select.value;
  const options = dockLoadOptions(g);
  setHtml(select, options);
  if (selected && [...select.options].some((option) => option.value === selected)) {
    select.value = selected;
  }
  const free = Math.max(0, fleetCargoCapacity(g) - fleetCargoUnits(g));
  const unavailable = select.options.length === 0 || free === 0;
  select.disabled = unavailable;
  const quantity = root.querySelector(".lg-qty") as HTMLInputElement | null;
  if (quantity) {
    quantity.max = String(Math.max(1, free));
    quantity.disabled = unavailable;
  }
  const load = root.querySelector('[data-act="load"]') as HTMLButtonElement | null;
  if (load) load.disabled = unavailable;
}


export function logisticsSection(g: GhostView): string {
  const atHub = g.docked === "hub";
  const sys = g.docked
    ? (state.galaxy?.systems ?? []).find((sy) =>
        dockedAtSystem(g, sy.id)
        && state.systems.find((served) => served.id === sy.id)?.owner === state.playerId)
    : undefined;
  if (!atHub && !sys) {
    return `<div class="sp-line dim" title="Cargo controls unlock after the fleet stops at a berth, leaves any engagement, and the Docked report arrives.">${icon("cargo", "sm")} Not docked</div>`;
  }
  const where = atHub ? "the hub" : esc(sys!.name);
  const rows: string[] = [`<div class="sp-sec">${icon("manifest", "sm")} Logistics · ${where}</div>`];
  const manifest = fleetCargoManifest(g);
  const free = Math.max(0, fleetCargoCapacity(g) - fleetCargoUnits(g));
  if (manifest.length) {
    const summary = manifest.map((stack) => `${fmt(stack.units)} ${esc(label(stack.commodity))}`).join(" · ");
    rows.push(`<div class="sp-line"><button class="act" data-act="unload" title="Put every commodity stack ashore at ${esc(where)}.">${icon("unload", "md")} Unload all · ${summary}</button></div>`);
  }
  // Load: pick a commodity + amount from whatever the dock actually holds.
  const options = dockLoadOptions(g);
  if (options && free > 0) {
    rows.push(
      `<div class="sp-line"><select class="lg-com" data-act="noop">` +
      options +
      `</select> <input class="lg-qty" type="number" min="1" max="${free}" value="${Math.min(50, free)}" style="width:5.5em" /> ` +
      `<button class="act" data-act="load" title="Add cargo from ${esc(where)}. This fleet has ${fmt(free)} units of hold space remaining.">${icon("manifest", "sm")} Load · ${fmt(free)} free</button></div>`,
    );
  } else if (options) {
    rows.push(`<div class="sp-line">${badge("warn", "hold full")} Unload cargo to make room.</div>`);
  }
  if (manifest.length && !atHub) {
    rows.push(
      `<div class="sp-line"><button class="act act--primary" data-act="haul" title="Send this loaded hull to the Market Hub. It deposits into your Market Warehouse on arrival — and, if you tick sell, clears on that tick's quantity-aware curve. The fleet SURVIVES and goes idle there.">${icon("freightRoute", "md")} Haul to the hub</button> ` +
      `<label class="lim" title="Sell the lot at the Exchange the moment it lands."><input type="checkbox" class="lg-sell" /> sell on arrival</label></div>`,
    );
  }
  if (manifest.length && atHub) {
    const destinations = ownedHaulDestinations();
    if (destinations.length) {
      const remembered = haulDestinationByFleet.get(g.id);
      const selected = destinations.some((destination) => destination.id === remembered)
        ? remembered!
        : destinations[0].id;
      haulDestinationByFleet.set(g.id, selected);
      const destination = destinations.find((candidate) => candidate.id === selected)!;
      const options = destinations.map((candidate) =>
        `<option value="${esc(candidate.id)}"${candidate.id === selected ? " selected" : ""}>${esc(candidate.name)}</option>`).join("");
      rows.push(
        `<div class="sp-line"><select class="lg-haul-system" aria-label="Return-haul destination">${options}</select> ` +
        `<button class="act act--primary" data-act="haul-system" title="Send this loaded freighter to the selected owned system. Its full mixed manifest unloads there on arrival.">${icon("freightRoute", "md")} <span class="lg-haul-label">Haul back to ${esc(destination.name)}</span></button></div>`,
      );
    }
  }
  return rows.join("");
}

