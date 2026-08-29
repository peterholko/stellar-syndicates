import { captainTitle, captainXpFloor, fleetCommandLoad } from "../../core/derive/captains";
import {
  AAA_SERVICE_FEE,
  aaaEstimate,
  coLocatedOwnFleet,
  dockLoadStock,
  dockedAtSystem,
  estimatedFuelForLeg,
  fleetCargoCapacity,
  fleetFuelCapacity,
  FUEL_PER_MASS_DISTANCE,
  guardCapable,
  hauls,
  jumpCapable,
  shipKindLabel,
  shipMass,
  WARP_FACTOR,
} from "../../core/derive/fleet";
import { fmt, fmtEta } from "../../core/derive/format";
import { emplacementLabel, nearestKnownDock, systemName } from "../../core/derive/geo";
import { fitLegal, kitAffordable, kitCostLabel, MODULE_SLOTS, moduleLedgerAt, ownedHaulDestinations } from "../../core/derive/market";
import { orderEtaRange, orderObject, orderPoint } from "../../core/derive/orders";
import type { CoreEvent } from "../../core/events";
import { jumpDepartureKey } from "../../core/session";
import { icon, label, type IconKey } from "../../icons";
import {
  fleetCargoManifest,
  fleetCargoUnits,
  fleetExactCount,
  formatId,
  type Commodity,
  type EngagementEstimate,
  type EngagementPosture,
  type GhostView,
  type ManifestEntryView,
  type ModuleKind,
  type ShipKind,
  type TransitMode,
  type Vec2,
} from "../../protocol";
import { liveSimTime } from "../../state";
import { captainPortrait } from "../art";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

interface FleetHooks {
  go(route: DeckRoute): void;
  notice(html: string): void;
}

type FleetConfirm = "fuel" | "haul-hub" | "haul-system" | "authority" | "flagship";

const REFIT_MODULES: ModuleKind[] = [
  "mass_driver", "torpedo_rack", "point_defense_screen", "reflective_plating", "whipple_armor",
];
const FLAGSHIP_ORDER: ShipKind[] = ["titan", "dreadnought", "battleship", "cruiser", "destroyer", "colony", "convoy", "corvette", "raider", "scout"];
const POSTURES: { key: EngagementPosture; label: string; copy: string }[] = [
  { key: "passive", label: "Passive", copy: "Fight only when engaged." },
  { key: "defensive", label: "Defensive", copy: "Defend guarded assets and stations." },
  { key: "weapons_free", label: "Weapons-free", copy: "Engage rivals detected inside this fleet's sensor bubble." },
];

/** The routed fleet command surface. Everything here is built from the served
 * ghost and the player's own command records: route geometry, fuel, docking,
 * cargo, officers and order phases never consult server truth. */
export class DeckFleetRoutes {
  private signature = "";
  private readonly requestedTransit = new Map<string, TransitMode>();
  private readonly posture = new Map<string, EngagementPosture>();
  private readonly confirms = new Map<string, FleetConfirm>();
  private readonly loadCommodity = new Map<string, Commodity>();
  private readonly loadQuantity = new Map<string, number>();
  private readonly haulDestination = new Map<string, string>();
  private readonly estimates = new Map<string, EngagementEstimate>();
  private readonly estimateAttacker = new Map<string, string>();

  constructor(
    private readonly root: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: FleetHooks,
  ) {}

  render(route: DeckRoute | null, force = false): boolean {
    if (route?.name !== "fleet") return false;
    const id = route.params?.id ?? "";
    const fleet = this.ctx.state.ghosts.find((entry) => entry.id === id);
    const active = document.activeElement;
    if (!force && active instanceof HTMLElement && this.root.contains(active)
      && (active.matches("select, input") || active.isContentEditable)) return true;
    const signature = sheetFingerprint([
      route, Math.floor(liveSimTime()), fleet, this.ctx.state.pendingOrders.get(id),
      this.ctx.state.orders[id], this.ctx.state.raids[id], this.ctx.state.commandSignals,
      this.ctx.state.selectedShipIds, this.ctx.state.wallet, this.ctx.state.market,
      this.ctx.state.systems, this.ctx.state.captains, this.ctx.state.syndicate?.flagship_name,
      this.ctx.state.emplacements, this.ctx.state.jumpDepartures,
      this.requestedTransit.get(id), this.posture.get(id), this.confirms.get(id),
      this.loadCommodity.get(id), this.loadQuantity.get(id), this.haulDestination.get(id),
      [...this.estimates.values()].filter((entry) => entry.target === id || entry.attacker === id),
    ]);
    if (!force && signature === this.signature) return true;
    if (renderDeferred(this.root.id, () => this.render(route, true))) return true;
    this.signature = signature;
    if (route.params?.object === "emplacement" || (!fleet && this.ctx.state.emplacements.some((entry) => entry.id === id))) {
      const emplacement = this.ctx.state.emplacements.find((entry) => entry.id === id);
      setHtml(this.root, emplacement ? this.emplacementReadoutHtml(emplacement) : this.expiredSpecialHtml("Installation report expired"));
      return true;
    }
    if (route.params?.object === "jump-departure" || (!fleet && this.ctx.state.jumpDepartures.some((entry) => jumpDepartureKey(entry) === id))) {
      const departure = this.ctx.state.jumpDepartures.find((entry) => jumpDepartureKey(entry) === id);
      setHtml(this.root, departure ? this.jumpDepartureHtml(id, departure) : this.expiredSpecialHtml("Jump departure report expired"));
      return true;
    }
    if (!fleet) {
      setHtml(this.root, this.contactLostHtml());
      return true;
    }
    this.selectForRoute(fleet);
    setHtml(this.root, this.fleetHtml(fleet));
    return true;
  }

  handleAction(button: HTMLButtonElement, route: DeckRoute | null): boolean {
    if (route?.name !== "fleet") return false;
    if (button.dataset.deckAct === "fleet-special-center") {
      const id = route.params?.id ?? "";
      const position = route.params?.object === "emplacement"
        ? this.ctx.state.emplacements.find((entry) => entry.id === id)?.pos
        : this.ctx.state.jumpDepartures.find((entry) => jumpDepartureKey(entry) === id)?.pos;
      if (position) this.ctx.renderer.centerOnWorld(position);
      return true;
    }
    const fleet = this.ctx.state.ghosts.find((entry) => entry.id === route.params?.id);
    if (!fleet) return true;
    const action = button.dataset.deckAct;
    if (!action?.startsWith("fleet-")) return false;
    const command = action.slice(6);
    if (command === "select-order") {
      const id = Number(button.dataset.order);
      this.ctx.state.selectedOrderId = this.ctx.state.selectedOrderId === id ? null : id;
    } else if (command === "dismiss-order") {
      const id = Number(button.dataset.order);
      if (Number.isFinite(id)) this.ctx.send({ type: "DismissLostOrder", order_id: id });
    } else if (command === "jump" && fleet.own) {
      this.ctx.intent.armJumpAiming(fleet);
    } else if (command === "guard" && fleet.own) {
      this.ctx.intent.armGuardAiming(fleet);
    } else if (command === "hold" && fleet.own) {
      this.ctx.send({ type: "HoldFleet", ship_id: fleet.id });
      this.hooks.notice("<b>Hold order sent</b> · the served course remains visible until stopping light returns.");
    } else if (command === "recall" && fleet.own) {
      this.ctx.send({ type: "RecallRaid", raider_id: fleet.id });
      this.hooks.notice("<b>Recall sent</b> · pursuit continues until the signal reaches the fleet.");
    } else if (command === "transit" && fleet.own) {
      const mode = button.dataset.mode as TransitMode;
      if (mode === "full" || mode === "stealth") {
        this.requestedTransit.set(fleet.id, mode);
        this.ctx.send({ type: "SetFleetTransit", fleet_id: fleet.id, mode });
      }
    } else if (command === "dock" && fleet.own) {
      const dock = nearestKnownDock(fleet);
      if (dock) {
        this.ctx.send({ type: "MoveShip", ship_id: fleet.id, dest: dock.pos });
        this.hooks.notice(`<b>Docking order sent</b> · ${esc(dock.name)} · ${fmt(dock.distance)} su.`);
      }
    } else if (command === "split" && fleet.own) {
      const kind = button.dataset.kind as ShipKind;
      if (kind) this.ctx.send({ type: "SplitFleet", fleet_id: fleet.id, counts: { [kind]: 1 } });
    } else if (command === "merge" && fleet.own && button.dataset.from) {
      this.ctx.send({ type: "MergeFleets", into: fleet.id, from: button.dataset.from });
    } else if (command === "posture" && fleet.own) {
      const value = button.dataset.posture as EngagementPosture;
      if (POSTURES.some((entry) => entry.key === value)) {
        this.posture.set(fleet.id, value);
        this.ctx.send({ type: "SetFleetPosture", fleet_id: fleet.id, posture: value });
      }
    } else if (command === "authority" && fleet.own) {
      const on = button.dataset.on === "1";
      if (on) this.setConfirm(fleet.id, "authority");
      else this.ctx.send({ type: "SetEngageFreight", fleet_id: fleet.id, on: false });
    } else if (command === "authority-confirm" && fleet.own) {
      this.ctx.send({ type: "SetEngageFreight", fleet_id: fleet.id, on: true });
      this.clearConfirm(fleet.id);
    } else if (command === "confirm-cancel") {
      this.clearConfirm(fleet.id);
    } else if (command === "refit" && fleet.own) {
      this.refit(button, fleet);
    } else if (command === "emplace" && fleet.own) {
      const reason = this.ctx.renderer.siteError("deep_space_sensor", fleet.pos, this.ctx.state);
      if (reason) this.hooks.notice(`<span class="deck-command-status__error"><b>Cannot build here</b> · ${esc(reason)}</span>`);
      else if (!kitAffordable("deep_space_sensor")) this.hooks.notice(`<span class="deck-command-status__error"><b>Kit unavailable</b> · ${esc(kitCostLabel("deep_space_sensor"))}</span>`);
      else this.ctx.send({ type: "BuildEmplacement", builder: fleet.id, emplacement: "deep_space_sensor" });
    } else if (command === "fuel-rescue" && fleet.own) {
      this.setConfirm(fleet.id, "fuel");
    } else if (command === "fuel-confirm" && fleet.own) {
      this.ctx.send({ type: "RequestFuelRescue", fleet_id: fleet.id });
      this.clearConfirm(fleet.id);
    } else if (command === "unload" && fleet.own) {
      this.unload(fleet);
    } else if (command === "load" && fleet.own) {
      this.load(fleet);
    } else if (command === "haul-hub" && fleet.own) {
      this.haulHub(fleet, false);
    } else if (command === "haul-hub-confirm" && fleet.own) {
      this.haulHub(fleet, true);
    } else if (command === "haul-system" && fleet.own) {
      this.haulSystem(fleet, false);
    } else if (command === "haul-system-confirm" && fleet.own) {
      this.haulSystem(fleet, true);
    } else if (command === "officers") {
      this.hooks.go({ name: "officers" });
    } else if (command === "train" && fleet.captain) {
      const attribute = button.dataset.attribute as "command" | "navigation" | "fieldcraft" | "logistics";
      if (attribute) this.ctx.send({ type: "TrainCaptain", captain_id: fleet.captain.id, attribute });
    } else if (command === "flagship") {
      this.setConfirm(fleet.id, "flagship");
    } else if (command === "flagship-save") {
      const input = this.root.querySelector<HTMLInputElement>("[data-fleet-flagship-name]");
      this.ctx.send({ type: "NameFlagship", name: input?.value.trim() ?? "" });
      this.clearConfirm(fleet.id);
    } else if (command === "estimate" && !fleet.own) {
      const select = this.root.querySelector<HTMLSelectElement>("[data-fleet-estimate-attacker]");
      const attacker = select?.value;
      if (attacker) {
        this.estimateAttacker.set(fleet.id, attacker);
        this.ctx.send({ type: "EstimateEngagement", attacker, target: fleet.id });
        this.hooks.notice("<b>Projection requested</b> · computed from the served combat picture.");
      }
    }
    this.signature = "";
    this.render(route, true);
    return true;
  }

  handleInput(input: HTMLInputElement | HTMLSelectElement, route: DeckRoute | null): boolean {
    if (route?.name !== "fleet") return false;
    const id = route.params?.id;
    if (!id) return true;
    if (input.matches("[data-fleet-load-commodity]")) this.loadCommodity.set(id, input.value as Commodity);
    else if (input.matches("[data-fleet-load-quantity]")) this.loadQuantity.set(id, Math.max(1, Math.floor(Number(input.value) || 1)));
    else if (input.matches("[data-fleet-haul-destination]")) this.haulDestination.set(id, input.value);
    else if (input.matches("[data-fleet-estimate-attacker]")) this.estimateAttacker.set(id, input.value);
    else if (input instanceof HTMLSelectElement && input.matches("[data-refit-target]")) {
      const option = input.selectedOptions[0];
      const row = input.closest<HTMLElement>("[data-refit-row]");
      const count = row?.querySelector<HTMLInputElement>("[data-refit-count]");
      if (count) {
        count.max = option?.dataset.max ?? "1";
        count.value = String(Math.min(Number(count.value) || 1, Number(count.max) || 1));
      }
    } else return false;
    return true;
  }

  /** Returns true only when an open fleet route absorbed an estimate into its
   * detail section. The shell uses that bit to suppress the duplicate toast. */
  onCoreEvent(event: CoreEvent, route: DeckRoute | null): boolean {
    if (event.kind !== "EstimateReady") return false;
    this.estimates.set(this.estimateKey(event.estimate.attacker, event.estimate.target), event.estimate);
    const open = route?.name === "fleet" && route.params?.id === event.estimate.target;
    if (open) {
      this.signature = "";
      this.render(route, true);
    }
    return open;
  }

  invalidate(): void { this.signature = ""; }

  private fleetHtml(g: GhostView): string {
    const informationDelay = g.jump_presumed?.information_delay ?? g.age;
    const title = fleetTitle(g);
    const role = g.own ? "Your fleet" : g.tca ? "Terran Charter Authority" : g.pirate ? "Pirate contact" : "Rival contact";
    const stats = [
      stat("Information delay", `${informationDelay.toFixed(1)}s`, informationDelay >= 8 ? "warn" : ""),
      stat("Drive", driveLabel(g)),
      stat("Speed", `${Math.round(Math.hypot(g.vel.x, g.vel.y)).toLocaleString()} su/s`),
      stat("Position", `${fmt(g.pos.x)} · ${fmt(g.pos.y)}`),
    ].join("");
    return `<section class="deck-page deck-fleet"><header class="deck-page__lead"><span>${esc(role)} · served picture</span><h2>${icon(g.kind === "convoy" || g.kind === "freighter" ? "convoy" : "fleet", "md")} ${esc(title)}</h2><p>${g.own ? "Commands and internal telemetry follow this report's information delay." : "Only information carried by arrived light is shown."}</p></header><div class="deck-stat-grid deck-fleet__stats">${stats}</div>${g.own ? this.ownHtml(g) : this.rivalHtml(g)}</section>`;
  }

  private ownHtml(g: GhostView): string {
    const orders = this.ordersHtml(g);
    const command = this.commandHtml(g);
    const payload = this.compositionHtml(g) + this.refitHtml(g) + (hauls(g) ? this.cargoHtml(g) + this.logisticsHtml(g) : "") + this.fuelHtml(g);
    return `<div class="deck-fleet-now"><span>Now</span><b>${this.activity(g)}</b></div>${this.jobHtml(g)}${orders}${command}<section class="deck-section"><header><div><h3>Fleet</h3><p>Served composition, fitting and carried stores.</p></div></header>${payload}${this.policyHtml(g)}${this.mergeHtml(g)}</section>${this.captainHtml(g)}`;
  }

  private ordersHtml(g: GhostView): string {
    const now = liveSimTime();
    const queue = this.ctx.state.pendingOrders.get(g.id) ?? [];
    const rows = queue.map((order) => {
      if (order.lost) return `<article class="deck-fleet-order is-lost"><span>${icon("lost", "sm")}</span><div><b>${esc(orderObject(order))}</b><small>Lost before delivery · issue a replacement</small></div><button type="button" data-deck-act="fleet-dismiss-order" data-order="${order.id}">Dismiss</button></article>`;
      const outbound = now < order.arrives_at;
      const selected = this.ctx.state.selectedOrderId === order.id;
      const spoolEnd = order.arrives_at + (this.ctx.state.galaxy?.jump_spool_s ?? 10);
      const spooling = order.kind === "jump" && !outbound && now < spoolEnd;
      const detail = outbound
        ? `signal outbound · ${shortEta(order.arrives_at - now)}`
        : order.kind === "jump"
          ? spooling ? `jump drive spooling · ${Math.ceil(spoolEnd - now)}s` : g.jump_presumed ? `presumed jumped · report ~${shortEta(g.jump_presumed.report_in)}` : "presumed jumped · awaiting light"
          : selected ? orderEtaRange(order.response_at, now) : now <= order.response_at ? `awaiting response · ${shortEta(order.response_at - now)}` : "response overdue · unconfirmed";
      return `<button type="button" class="deck-fleet-order${selected ? " is-selected" : ""}" data-deck-act="fleet-select-order" data-order="${order.id}" aria-pressed="${selected}"><span>${icon(outbound ? "inTransit" : "echo", "sm")}</span><span><b>${esc(orderObject(order))}</b><small>${esc(detail)}</small></span></button>`;
    }).join("");
    const local = this.ctx.state.orders[g.id];
    const served = (local || queue.some((entry) => entry.kind === "move")) && g.path?.length ? g.path.at(-1)?.pos : undefined;
    const current = served ?? local;
    const represented = current && queue.some((entry) => entry.kind === "move" && entry.dest && Math.hypot(entry.dest.x - current.x, entry.dest.y - current.y) < 1);
    const currentRow = current && !represented
      ? `<article class="deck-fleet-order is-current"><span>${icon("move", "sm")}</span><div><b>${esc(`Move → ${orderPoint(current)}`)}</b><small>Current order · remains until observed arrival</small></div></article>` : "";
    return rows || currentRow ? `<section class="deck-section"><header><div><h3>Orders</h3><p>Outbound → presumed delivered → confirmed by returned light.</p></div></header><div class="deck-fleet-orders">${rows}${currentRow}</div></section>` : "";
  }

  private commandHtml(g: GhostView): string {
    const items: string[] = [];
    const queue = this.ctx.state.pendingOrders.get(g.id) ?? [];
    const hasCourse = !!this.ctx.state.orders[g.id] || !!g.path?.length || Math.hypot(g.vel.x, g.vel.y) >= .5 || queue.some((order) => order.kind !== "hold");
    if (hasCourse && !g.docked) items.push(commandButton("fleet-hold", "Hold position", "Cancel the current course when this signal arrives.", "is-danger"));
    if (this.ctx.state.raids[g.id]) items.push(commandButton("fleet-recall", "Recall raid", "Break pursuit and return toward home.", "is-danger"));
    if (!g.docked) {
      const requested = this.requestedTransit.get(g.id);
      items.push(`<div class="deck-command-block"><b>Transit</b><div class="deck-segment"><button type="button" data-deck-act="fleet-transit" data-mode="full" aria-pressed="${requested === "full"}">Full speed</button><button type="button" data-deck-act="fleet-transit" data-mode="stealth" aria-pressed="${requested === "stealth"}">Stealth</button></div><small>Stealth trades speed for a smaller detection signature.</small></div>`);
    }
    const dock = nearestKnownDock(g);
    if (g.docked) items.push(`<div class="deck-command-block"><b>Docked</b><small>${esc(g.docked === "hub" ? "Wormhole Hub" : systemName(normalizeDock(g.docked)))}</small></div>`);
    else if (dock) items.push(commandButton("fleet-dock", "Initiate docking", `${dock.name} · ${fmt(dock.distance)} su`, "is-primary"));
    if (guardCapable(g)) items.push(commandButton("fleet-guard", g.guard_target ? "Reassign guard" : "Guard a fleet", g.guard_target ? `Currently guarding ${shipKindLabel(this.ctx.state.ghosts.find((entry) => entry.id === g.guard_target)?.kind ?? "convoy")}` : "Choose a friendly fleet on the map."));
    if (jumpCapable(g)) items.push(commandButton("fleet-jump", "Set jump destination", `Range ${fmt(this.ctx.state.galaxy?.jump_range ?? 0)} su · choose on map.`, "is-primary"));
    if (g.kind === "builder") items.push(this.emplaceHtml(g));
    if (g.stalled || g.rescue_inbound) items.push(this.rescueHtml(g));
    return items.length ? `<section class="deck-section"><header><div><h3>Commands</h3><p>Immediate fleet verbs remain information-delayed.</p></div></header><div class="deck-command-grid">${items.join("")}</div></section>` : "";
  }

  private compositionHtml(g: GhostView): string {
    const stacks = [...(g.composition ?? [])].filter((entry) => entry.count > 0).sort((a, b) => FLAGSHIP_ORDER.indexOf(a.kind) - FLAGSHIP_ORDER.indexOf(b.kind));
    if (!stacks.length) return `<div class="deck-muted">Composition report unavailable.</div>`;
    const rows = stacks.map((entry) => `<div class="deck-fleet-stack"><span>${icon("fleet", "sm")}<b>${esc(shipKindLabel(entry.kind))}</b></span><em>×${entry.count}</em></div>`).join("");
    const total = stacks.reduce((sum, entry) => sum + entry.count, 0);
    const split = total > 1 ? `<div class="deck-fleet-split">${stacks.map((entry) => `<button type="button" data-deck-act="fleet-split" data-kind="${entry.kind}">Split 1 ${esc(shipKindLabel(entry.kind))}</button>`).join("")}</div>` : "";
    const damage = (g.damage ?? 0) > .001 ? `<div class="deck-alert ${(g.damage ?? 0) > .5 ? "deck-alert--bad" : ""}"><b>Hull integrity ${Math.round((1 - (g.damage ?? 0)) * 100)}%</b><span>Damage persists until foundry service.</span></div>` : "";
    const supply = g.supplied === false ? `<div class="deck-alert deck-alert--bad"><b>Out of supply</b><span>Immobilized until Provisions arrive.</span></div>` : "";
    const flagship = stacks.some((entry) => entry.kind === "titan") ? this.flagshipHtml(g) : "";
    return `<div class="deck-subhead"><b>Composition</b><span>${total} ship${total === 1 ? "" : "s"}</span></div><div class="deck-fleet-stacks">${rows}</div>${flagship}${damage}${supply}${split}`;
  }

  private flagshipHtml(g: GhostView): string {
    const current = this.ctx.state.syndicate?.flagship_name?.trim() ?? "";
    if (this.confirms.get(g.id) === "flagship") return `<div class="deck-inline-confirm"><b>Christen flagship</b><input data-fleet-flagship-name maxlength="32" value="${escAttr(current)}" placeholder="Flagship name"><div><button type="button" data-deck-act="fleet-flagship-save" class="is-primary">Save</button><button type="button" data-deck-act="fleet-confirm-cancel">Cancel</button></div></div>`;
    return `<div class="deck-fleet-flagship"><span>Flagship <b>${esc(current || "unnamed")}</b></span><button type="button" data-deck-act="fleet-flagship">${current ? "Rename" : "Name"}</button></div>`;
  }

  private fuelHtml(g: GhostView): string {
    const fuel = Math.max(0, g.fuel ?? 0);
    const capacity = fleetFuelCapacity(g);
    const pct = capacity > 0 ? fuel / capacity * 100 : 0;
    const dest = this.ctx.state.orders[g.id] ?? g.path?.at(-1)?.pos;
    const burn = FUEL_PER_MASS_DISTANCE * 1000 * shipMass(g) / WARP_FACTOR;
    const detail = dest ? `Current leg needs ~${fmt(estimatedFuelForLeg(g, dest))}` : `Warp burn ~${burn.toFixed(1)} / 1k su`;
    return `<div class="deck-subhead"><b>${icon("fuel", "sm")} Fuel</b><span>${g.fuel == null ? "report unavailable" : `${fmt(fuel)} / ${fmt(capacity)}`}</span></div><div class="deck-meter"><i style="width:${Math.max(0, Math.min(100, pct)).toFixed(1)}%"></i></div><p class="deck-muted">${esc(detail)} · delayed ${g.age.toFixed(1)}s.</p>`;
  }

  private captainHtml(g: GhostView): string {
    const captain = g.captain;
    if (!captain) return `<section class="deck-section"><header><div><h3>Officer</h3><p>No officer assigned · formation load ${fleetCommandLoad(g)}.</p></div><button type="button" data-deck-act="fleet-officers">Open Officer Corps</button></header></section>`;
    const floor = captainXpFloor(captain.level);
    const span = Math.max(1, captain.next_level_xp - floor);
    const progress = captain.level >= 10 ? 100 : Math.max(0, Math.min(100, (captain.xp - floor) / span * 100));
    const home = this.ctx.state.commandCenter && this.ctx.state.galaxy?.systems.find((system) => Math.hypot(system.pos.x - this.ctx.state.commandCenter!.x, system.pos.y - this.ctx.state.commandCenter!.y) < 1)?.id;
    const canTrain = captain.unspent > 0 && !!home && dockedAtSystem(g, home);
    const attributes = (["command", "navigation", "fieldcraft", "logistics"] as const).map((attribute) => `<div><span>${label(attribute)}</span><b>${captain.attributes[attribute]}</b>${captain.unspent > 0 ? `<button type="button" data-deck-act="fleet-train" data-attribute="${attribute}" ${canTrain ? "" : "disabled"}>+</button>` : ""}</div>`).join("");
    const titled = `${captainTitle(captain.title)} ${captain.name}`;
    return `<section class="deck-section deck-captain"><header><div><h3>Officer</h3><p>Physical assignment · ${g.age.toFixed(1)}s delayed.</p></div><button type="button" data-deck-act="fleet-officers">Manage</button></header><div class="deck-captain__body">${captainPortrait(captain.portrait, captain.portrait_age, `Portrait of ${titled}`, "deck-captain__portrait", false)}<div><b>${esc(titled)}</b><small>Level ${captain.level} · authority ${fleetCommandLoad(g)} / ${captain.command_capacity}</small><div class="deck-meter"><i style="width:${progress.toFixed(1)}%"></i></div><small>${captain.level >= 10 ? "Maximum level" : `${captain.xp.toLocaleString()} / ${captain.next_level_xp.toLocaleString()} XP`}</small><div class="deck-captain__stats">${attributes}</div></div></div></section>`;
  }

  private refitHtml(g: GhostView): string {
    const capable = (g.composition ?? []).some((stack) => stack.count > 0 && (MODULE_SLOTS[stack.kind] ?? 0) > 0);
    if (!capable) return "";
    const dock = g.docked && g.docked !== "hub" ? this.ctx.state.systems.find((system) => dockedAtSystem(g, system.id)) : undefined;
    const foundry = !!dock && (dock.owner === this.ctx.state.playerId || dock.ally) && (dock.structures.ordnance_foundry ?? 0) > 0;
    if (!foundry) return `<div class="deck-subhead"><b>Refit</b><span>Requires an owned/allied Ordnance Foundry.</span></div>`;
    const ledger = moduleLedgerAt(dock!.id);
    const stacks = refitStacks(g);
    const busy = Math.hypot(g.vel.x, g.vel.y) >= .5 || !!g.path?.length || (this.ctx.state.pendingOrders.get(g.id)?.length ?? 0) > 0;
    const rows = stacks.map((stack) => {
      const targets = refitTargets(stack.kind, stack.modules, ledger, this.ctx.state.syndicate?.fits ?? []);
      const options = targets.map((target) => {
        const max = maxRefitCount(stack.modules, target, ledger, stack.n);
        return `<option value="${escAttr(fitKey(target))}" data-max="${max}">${esc(fitName(target))} · up to ${max}</option>`;
      }).join("");
      const initial = targets.length ? maxRefitCount(stack.modules, targets[0], ledger, stack.n) : 1;
      return `<div class="deck-refit-row" data-refit-row data-ship="${stack.kind}" data-from="${escAttr(fitKey(stack.modules))}"><span><b>${stack.n}× ${esc(shipKindLabel(stack.kind))}</b><small>${esc(fitName(stack.modules))}</small></span>${options ? `<select data-refit-target>${options}</select><input data-refit-count type="number" min="1" max="${initial}" value="1"><button type="button" data-deck-act="fleet-refit" ${busy ? "disabled" : ""}>Refit</button>` : `<em>No stocked legal change</em>`}</div>`;
    }).join("");
    const stock = REFIT_MODULES.filter((module) => (ledger[module] ?? 0) > 0).map((module) => `${label(module)} ${ledger[module]}`).join(" · ") || "ledger empty";
    return `<div class="deck-subhead"><b>Refit · ${esc(systemName(dock!.id))}</b><span>${esc(stock)}</span></div><div class="deck-refit-list">${rows}</div>${busy ? `<p class="deck-warning">Fleet must be idle with no command in flight.</p>` : ""}`;
  }

  private cargoHtml(g: GhostView): string {
    const manifest = fleetCargoManifest(g);
    const used = fleetCargoUnits(g);
    const capacity = fleetCargoCapacity(g);
    return `<div class="deck-subhead"><b>${icon("cargo", "sm")} Cargo</b><span>${fmt(used)} / ${fmt(capacity)} units</span></div><div class="deck-fleet-cargo">${manifest.length ? manifest.map((entry) => `<span>${icon(entry.commodity as IconKey, "sm")}<b>${fmt(entry.units)}</b> ${esc(label(entry.commodity))}</span>`).join("") : `<span class="deck-muted">Empty hold</span>`}</div>`;
  }

  private logisticsHtml(g: GhostView): string {
    const system = g.docked && g.docked !== "hub" ? this.ctx.state.systems.find((entry) => entry.owner === this.ctx.state.playerId && dockedAtSystem(g, entry.id)) : undefined;
    if (g.docked !== "hub" && !system) return `<p class="deck-muted">Cargo controls unlock after a valid docking report arrives.</p>`;
    const stocks = dockLoadStock(g).filter(([, units]) => units > 0);
    const selected = stocks.some(([commodity]) => commodity === this.loadCommodity.get(g.id)) ? this.loadCommodity.get(g.id)! : stocks[0]?.[0];
    if (selected) this.loadCommodity.set(g.id, selected);
    const free = Math.max(0, fleetCargoCapacity(g) - fleetCargoUnits(g));
    const qty = Math.max(1, Math.min(free || 1, this.loadQuantity.get(g.id) ?? Math.min(50, free || 1)));
    this.loadQuantity.set(g.id, qty);
    const manifest = fleetCargoManifest(g);
    const load = stocks.length && free > 0 ? `<div class="deck-inline-form"><select data-fleet-load-commodity>${stocks.map(([commodity, units]) => `<option value="${commodity}" ${commodity === selected ? "selected" : ""}>${esc(label(commodity))} (${fmt(units)})</option>`).join("")}</select><input data-fleet-load-quantity type="number" min="1" max="${free}" value="${qty}"><button type="button" data-deck-act="fleet-load">Load</button></div>` : "";
    const unload = manifest.length ? `<button type="button" data-deck-act="fleet-unload">Unload all</button>` : "";
    const haul = manifest.length && system ? `<button type="button" class="is-primary" data-deck-act="fleet-haul-hub">Haul to Market Hub</button><label class="deck-check"><input type="checkbox" data-fleet-sell> Sell on arrival</label>${this.confirmHtml(g, "haul-hub")}` : "";
    const destinations = g.docked === "hub" && manifest.length ? ownedHaulDestinations() : [];
    const remembered = this.haulDestination.get(g.id);
    const destination = destinations.find((entry) => entry.id === remembered) ?? destinations[0];
    if (destination) this.haulDestination.set(g.id, destination.id);
    const returnHaul = destination ? `<div class="deck-inline-form"><select data-fleet-haul-destination>${destinations.map((entry) => `<option value="${entry.id}" ${entry.id === destination.id ? "selected" : ""}>${esc(entry.name)}</option>`).join("")}</select><button type="button" class="is-primary" data-deck-act="fleet-haul-system">Haul to system</button></div>${this.confirmHtml(g, "haul-system")}` : "";
    return `<div class="deck-subhead"><b>Dockside logistics</b><span>${g.docked === "hub" ? "Market Warehouse" : esc(systemName(system!.id))}</span></div><div class="deck-logistics">${load}${unload}${haul}${returnHaul}</div>`;
  }

  private policyHtml(g: GhostView): string {
    const rows: string[] = [];
    if ((g.composition ?? []).some((entry) => entry.kind === "raider")) {
      const current = this.posture.get(g.id) ?? g.posture ?? "passive";
      rows.push(`<div class="deck-command-block"><b>Engagement posture</b><div class="deck-segment">${POSTURES.map((entry) => `<button type="button" data-deck-act="fleet-posture" data-posture="${entry.key}" aria-pressed="${current === entry.key}">${entry.label}</button>`).join("")}</div><small>${esc(POSTURES.find((entry) => entry.key === current)?.copy ?? "")}</small></div>`);
    }
    if (g.engage_freight !== null && g.engage_freight !== undefined) {
      rows.push(`<div class="deck-command-block"><b>Authority freight</b><button type="button" data-deck-act="fleet-authority" data-on="${g.engage_freight ? "0" : "1"}" aria-pressed="${g.engage_freight}">${g.engage_freight ? "Engaging" : "Ignoring"} arriving Authority freighters</button>${this.confirmHtml(g, "authority")}</div>`);
    }
    return rows.length ? `<div class="deck-subhead"><b>Standing policy</b><span>Local autonomous behavior</span></div><div class="deck-command-grid">${rows.join("")}</div>` : "";
  }

  private mergeHtml(g: GhostView): string {
    const other = coLocatedOwnFleet(g);
    if (!other) return "";
    const officer = g.captain ?? other.captain;
    const two = !!g.captain && !!other.captain;
    const load = fleetCommandLoad(g) + fleetCommandLoad(other);
    const over = !!officer && load > officer.command_capacity;
    return `<div class="deck-subhead"><b>Fleet management</b><span>Co-located formation available</span></div><button type="button" data-deck-act="fleet-merge" data-from="${escAttr(other.id)}" ${two || over ? "disabled" : ""}>${two ? "Reserve one officer to merge" : over ? `Requires ${load} command points` : `Merge ${shipKindLabel(other.kind)} fleet into this fleet`}</button>`;
  }

  private rivalHtml(g: GhostView): string {
    const parts: string[] = [this.rivalComposition(g), this.rivalPayload(g), this.estimateHtml(g)];
    return `<section class="deck-section"><header><div><h3>Observed contact</h3><p>Fog removes internal state, intent, fuel and unarrived cargo reports.</p></div></header>${parts.join("")}</section>`;
  }

  private rivalComposition(g: GhostView): string {
    if (!g.composition?.length) return `<div class="deck-alert"><b>Estimated ${esc(label(g.count_class))} formation</b><span>Exact composition remains outside sensor coverage.</span></div>`;
    return `<div class="deck-subhead"><b>Composition</b><span>${fleetExactCount(g)} ships</span></div><div class="deck-fleet-stacks">${g.composition.map((entry) => `<div class="deck-fleet-stack"><span>${icon("fleet", "sm")}<b>${esc(shipKindLabel(entry.kind))}</b></span><em>×${entry.count}</em></div>`).join("")}</div>`;
  }

  private rivalPayload(g: GhostView): string {
    if (g.kind === "convoy") {
      const manifest = fleetCargoManifest(g);
      return `<div class="deck-subhead"><b>Convention broadcast</b><span>${g.route?.length ? `${g.route.length} route legs` : "route unavailable"}</span></div><div class="deck-fleet-cargo">${manifest.length ? manifest.map((entry) => `<span>${icon(entry.commodity as IconKey, "sm")}<b>${fmt(entry.units)}</b> ${esc(label(entry.commodity))}</span>`).join("") : `<span class="deck-muted">Cargo unknown outside sensor coverage.</span>`}</div>`;
    }
    if (g.kind === "freighter") {
      const mine = (g.manifest ?? []).filter((entry) => entry.mine);
      const theirs = (g.manifest ?? []).filter((entry) => !entry.mine);
      const rows = [...mine, ...theirs].map((entry: ManifestEntryView) => `<span>${icon(entry.commodity as IconKey, "sm")}<b>${fmt(entry.units)}</b> ${esc(label(entry.commodity))} <small>${entry.direction === "outbound" ? "→ system" : "→ hub"}</small></span>`).join("");
      return `<div class="deck-subhead"><b>Authority manifest</b><span>${g.revealed ? "sensor-resolved" : "your lots only"}</span></div><div class="deck-fleet-cargo">${rows || `<span class="deck-muted">Other lots unknown.</span>`}</div><div class="deck-alert"><b>Authority sanctuary</b><span>Attack is possible, but cited and priced through standing.</span></div>`;
    }
    return `<div class="deck-alert"><b>${g.kind === "scout" ? "Silent scout" : "Dark combat contact"}</b><span>Visible only because this report was detected inside sensor range.${g.signature == null ? "" : ` Signature ${g.signature.toFixed(2)}×.`}</span></div>`;
  }

  private estimateHtml(target: GhostView): string {
    if (target.tca) return "";
    const attackers = this.ctx.state.ghosts.filter((fleet) => fleet.own && (fleet.kind === "raider" || fleet.composition?.some((entry) => entry.kind === "raider" && entry.count > 0)));
    if (!attackers.length) return "";
    const selected = attackers.some((entry) => entry.id === this.estimateAttacker.get(target.id)) ? this.estimateAttacker.get(target.id)! : attackers[0].id;
    this.estimateAttacker.set(target.id, selected);
    const estimate = this.estimates.get(this.estimateKey(selected, target.id)) ?? [...this.estimates.values()].find((entry) => entry.target === target.id);
    return `<div class="deck-subhead"><b>Engagement projection</b><span>Served inputs only</span></div><div class="deck-inline-form"><select data-fleet-estimate-attacker>${attackers.map((entry) => `<option value="${entry.id}" ${entry.id === selected ? "selected" : ""}>${esc(shipKindLabel(entry.kind))} fleet</option>`).join("")}</select><button type="button" data-deck-act="fleet-estimate">Estimate</button></div>${estimate ? renderEstimate(estimate) : `<p class="deck-muted">The projection uses stale composition and defense reports; it is never combat truth.</p>`}`;
  }

  private emplaceHtml(g: GhostView): string {
    const busy = !!g.job || this.ctx.state.pendingOrders.has(g.id) || Math.hypot(g.vel.x, g.vel.y) >= .5;
    const error = this.ctx.renderer.siteError("deep_space_sensor", g.pos, this.ctx.state);
    return `<div class="deck-command-block"><b>Construct</b><button type="button" data-deck-act="fleet-emplace" ${busy || !!error ? "disabled" : ""}>Build Deep Space Sensor</button><small>${busy ? "Builder must be idle." : error ? esc(error) : `Builds here · kit ${esc(kitCostLabel("deep_space_sensor"))}.`}</small></div>`;
  }

  private rescueHtml(g: GhostView): string {
    if (g.rescue_inbound) return `<div class="deck-command-block"><b>AAA rescue active</b><small>Physical tender dispatched from the Market Hub.</small></div>`;
    const quote = aaaEstimate(g);
    const affordable = (this.ctx.state.wallet?.credits ?? 0) + 1e-6 >= quote.cost;
    return `<div class="deck-command-block"><b>Out of fuel</b><button type="button" data-deck-act="fleet-fuel-rescue" ${affordable ? "" : "disabled"}>Call AAA · ~${fmt(quote.cost)} Cr</button><small>3× market Fuel price + ${fmt(AAA_SERVICE_FEE)} Cr callout.</small>${this.confirmHtml(g, "fuel")}</div>`;
  }

  private confirmHtml(g: GhostView, kind: FleetConfirm): string {
    if (this.confirms.get(g.id) !== kind) return "";
    if (kind === "fuel") {
      const quote = aaaEstimate(g);
      return `<div class="deck-inline-confirm"><b>Dispatch physical rescue tender?</b><span>Charge ~${fmt(quote.cost)} Cr now. The callout is non-refundable if the tender is lost.</span><div><button type="button" class="is-primary" data-deck-act="fleet-fuel-confirm">Dispatch</button><button type="button" data-deck-act="fleet-confirm-cancel">Cancel</button></div></div>`;
    }
    if (kind === "authority") return `<div class="deck-inline-confirm"><b>Engage Authority freight?</b><span>Each intercepted Authority hull can trigger citations and higher market costs.</span><div><button type="button" class="is-danger" data-deck-act="fleet-authority-confirm">Enable</button><button type="button" data-deck-act="fleet-confirm-cancel">Cancel</button></div></div>`;
    const destination = kind === "haul-hub" ? this.ctx.state.galaxy?.hub : ownedHaulDestinations().find((entry) => entry.id === this.haulDestination.get(g.id))?.id;
    const pos = typeof destination === "string" ? this.ctx.state.galaxy?.systems.find((entry) => entry.id === destination)?.pos : destination;
    const needed = pos ? estimatedFuelForLeg(g, pos) : 0;
    return `<div class="deck-inline-confirm"><b>Fuel estimate is short</b><span>Latest tank ${fmt(g.fuel ?? 0)} · estimated leg ${fmt(needed)} Fuel. The fleet may run dry and hold.</span><div><button type="button" class="is-danger" data-deck-act="fleet-${kind}-confirm">Depart anyway</button><button type="button" data-deck-act="fleet-confirm-cancel">Cancel</button></div></div>`;
  }

  private jobHtml(g: GhostView): string {
    if (!g.job) return "";
    const pct = Math.max(0, Math.min(100, g.job.progress * 100));
    return `<div class="deck-job"><span>${g.job.kind === "demolishing" ? "Demolishing" : "Constructing"}</span><div class="deck-meter"><i style="width:${pct.toFixed(1)}%"></i></div><b>${pct.toFixed(0)}%</b></div>`;
  }

  private activity(g: GhostView): string {
    if (this.ctx.state.commandSignals.some((entry) => entry.shipId === g.id)) return "Signal outbound";
    if (g.jump_presumed) return "Presumed at jump destination";
    if (g.jump_spool) return g.jump_spool.waiting_for_fuel ? "Jump ready · awaiting fuel" : "Jump drive spooling";
    if (g.rescue_inbound) return "AAA rescue active";
    if (g.stalled) return "Out of fuel · holding";
    if (g.job) return `${g.job.kind === "demolishing" ? "Demolishing" : "Constructing"} ${Math.round(g.job.progress * 100)}%`;
    if (this.ctx.state.raids[g.id]) return "Raiding";
    if (this.ctx.state.orders[g.id]) return "En route";
    if (g.route?.length) return "Hauling";
    return Math.hypot(g.vel.x, g.vel.y) < .5 ? "Holding station" : "Under way";
  }

  private load(g: GhostView): void {
    const commodity = this.loadCommodity.get(g.id);
    if (!commodity) return;
    const free = Math.max(0, fleetCargoCapacity(g) - fleetCargoUnits(g));
    const available = dockLoadStock(g).find(([entry]) => entry === commodity)?.[1] ?? 0;
    const units = Math.max(1, Math.min(free, available, this.loadQuantity.get(g.id) ?? 1));
    if (g.docked === "hub") this.ctx.send({ type: "HubLoad", fleet_id: g.id, commodity, units });
    else {
      const system = this.ctx.state.systems.find((entry) => entry.owner === this.ctx.state.playerId && dockedAtSystem(g, entry.id));
      if (system) this.ctx.send({ type: "SystemLoad", fleet_id: g.id, system: system.id, commodity, units });
    }
  }

  private unload(g: GhostView): void {
    if (g.docked === "hub") this.ctx.send({ type: "HubUnload", fleet_id: g.id });
    else {
      const system = this.ctx.state.systems.find((entry) => entry.owner === this.ctx.state.playerId && dockedAtSystem(g, entry.id));
      if (system) this.ctx.send({ type: "SystemUnload", fleet_id: g.id, system: system.id });
    }
  }

  private haulHub(g: GhostView, confirmed: boolean): void {
    const pos = this.ctx.state.galaxy?.hub;
    if (!pos) return;
    if (!confirmed && this.needsFuelConfirm(g, pos)) { this.setConfirm(g.id, "haul-hub"); return; }
    const sell = this.root.querySelector<HTMLInputElement>("[data-fleet-sell]")?.checked ?? false;
    this.ctx.send({ type: "HaulToMarketHub", fleet_id: g.id, sell_on_arrival: sell });
    this.clearConfirm(g.id);
  }

  private haulSystem(g: GhostView, confirmed: boolean): void {
    const id = this.haulDestination.get(g.id);
    const system = this.ctx.state.galaxy?.systems.find((entry) => entry.id === id);
    if (!id || !system) return;
    if (!confirmed && this.needsFuelConfirm(g, system.pos)) { this.setConfirm(g.id, "haul-system"); return; }
    this.ctx.send({ type: "HaulToSystem", fleet_id: g.id, system: id });
    this.clearConfirm(g.id);
  }

  private refit(button: HTMLButtonElement, g: GhostView): void {
    const row = button.closest<HTMLElement>("[data-refit-row]");
    const ship = row?.dataset.ship as ShipKind | undefined;
    const from = parseModules(row?.dataset.from);
    const target = row?.querySelector<HTMLSelectElement>("[data-refit-target]");
    const to = parseModules(target?.value);
    const count = row?.querySelector<HTMLInputElement>("[data-refit-count]");
    const n = Math.max(1, Math.min(Number(count?.max) || 1, Math.floor(Number(count?.value) || 1)));
    if (ship && fitLegal(ship, to)) this.ctx.send({ type: "RefitShips", fleet_id: g.id, ship, from, to, n });
  }

  private needsFuelConfirm(g: GhostView, dest: Vec2): boolean {
    return g.fuel != null && g.fuel + 1e-6 < estimatedFuelForLeg(g, dest);
  }

  private setConfirm(id: string, kind: FleetConfirm): void { this.confirms.set(id, kind); this.signature = ""; }
  private clearConfirm(id: string): void { this.confirms.delete(id); this.signature = ""; }
  private estimateKey(attacker: string, target: string): string { return `${attacker}:${target}`; }

  private selectForRoute(g: GhostView): void {
    this.ctx.state.selectedShipId = g.id;
    this.ctx.state.selectedEmplacementId = null;
    this.ctx.renderer.selectedJumpDepartureKey = null;
    if (g.own && this.ctx.state.selectedShipIds.size <= 1) this.ctx.state.selectedShipIds = new Set([g.id]);
  }

  private contactLostHtml(): string {
    return `<section class="deck-page"><header class="deck-page__lead"><span>Contact</span><h2>Contact lost</h2><p>The fleet or transient report is no longer present in the served picture.</p></header></section>`;
  }

  private emplacementReadoutHtml(emplacement: (typeof this.ctx.state.emplacements)[number]): string {
    this.ctx.state.selectedShipId = null;
    this.ctx.state.selectedShipIds.clear();
    this.ctx.state.selectedEmplacementId = emplacement.id;
    this.ctx.renderer.selectedJumpDepartureKey = null;
    const mine = emplacement.own !== false;
    return `<section class="deck-page deck-installation"><header class="deck-page__lead"><span>${mine ? "Your structure" : "Rival structure"} · served map object</span><h2>${icon("sensor", "md")} ${esc(emplacementLabel(emplacement.kind))}</h2><p>${mine ? "Stationary sensor emplacement." : "Detected inside your sensor coverage; no private internal state is exposed."}</p></header><div class="deck-stat-grid"><dl class="deck-stat"><dt>Position</dt><dd>${fmt(emplacement.pos.x)} · ${fmt(emplacement.pos.y)}</dd></dl><dl class="deck-stat"><dt>Sensor radius</dt><dd>${fmt(emplacement.sensor_range)} su</dd></dl><dl class="deck-stat"><dt>Status</dt><dd>${mine ? "Standing" : "Hostile"}</dd></dl></div><section class="deck-section"><header><div><h3>Open-space picket</h3><p>Watches every detectable object inside its served radius.</p></div><button type="button" data-deck-act="fleet-special-center">Center map</button></header>${mine ? `<div class="deck-alert"><b>Owned installation</b><span>Its sensor picture and delayed reporting are already included in command.</span></div>` : `<div class="deck-alert deck-alert--bad"><b>Demolition path</b><span>Select an armed fleet, then click this installation on the map. The fleet must reach it and hold station through the job.</span></div>`}</section></section>`;
  }

  private jumpDepartureHtml(key: string, departure: (typeof this.ctx.state.jumpDepartures)[number]): string {
    this.ctx.state.selectedShipId = null;
    this.ctx.state.selectedShipIds.clear();
    this.ctx.state.selectedEmplacementId = null;
    this.ctx.renderer.selectedJumpDepartureKey = key;
    const now = liveSimTime();
    const mine = departure.owner === this.ctx.state.playerId;
    const owner = departure.owner_name?.trim() || (mine ? this.ctx.state.name : `Corporation ${formatId(departure.owner)}`);
    const delay = Math.max(0, departure.learned_at - departure.departed_at);
    return `<section class="deck-page deck-jump-report"><header class="deck-page__lead"><span>Jump departure · delayed light</span><h2>${icon("jump", "md")} ${esc(shipKindLabel(departure.kind))} fleet</h2><p>A historical departure report, not a live position claim. No destination is disclosed.</p></header><div class="deck-stat-grid"><dl class="deck-stat"><dt>Corporation</dt><dd>${esc(owner)}</dd></dl><dl class="deck-stat"><dt>Jumped</dt><dd>${esc(shortEta(Math.max(0, now - departure.departed_at)))} ago</dd></dl><dl class="deck-stat"><dt>Report delay</dt><dd>${delay.toFixed(1)}s</dd></dl><dl class="deck-stat"><dt>Origin</dt><dd>${fmt(departure.pos.x)} · ${fmt(departure.pos.y)}</dd></dl></div><section class="deck-section"><header><div><h3>Observed event</h3><p>${esc(owner)}'s ${esc(shipKindLabel(departure.kind))} fleet jumped away from this point.</p></div><button type="button" data-deck-act="fleet-special-center">Center map</button></header><div class="deck-alert"><b>Transient clue</b><span>The split chevrons fade from the local event ledger. Their expiry removes this readout too.</span></div></section></section>`;
  }

  private expiredSpecialHtml(title: string): string {
    return `<section class="deck-page"><header class="deck-page__lead"><span>Transient map report</span><h2>${esc(title)}</h2><p>This clue is no longer present in the served map picture. Back returns to the fleet roster.</p></header></section>`;
  }
}

function renderEstimate(estimate: EngagementEstimate): string {
  const losses = estimate.own_loss_bands?.filter((entry) => entry.hi > 0).map((entry) => `${entry.lo === entry.hi ? entry.lo : `${entry.lo}–${entry.hi}`} ${shipKindLabel(entry.kind)}`).join(", ")
    || estimate.own_losses.filter((entry) => entry.count > 0).map((entry) => `${entry.count} ${shipKindLabel(entry.kind)}`).join(", ") || "none";
  const verdict = estimate.win_pct == null ? "Projected engagement" : `${Math.round(estimate.win_pct)}% ${estimate.win_pct >= 55 ? "favorable" : estimate.win_pct >= 45 ? "even" : "unfavorable"}`;
  const picture = estimate.target_known ? "exact reported composition" : `typical hulls for a ${label(estimate.target_count_class)} contact`;
  return `<article class="deck-estimate"><small>Arrived projection</small><b>${esc(verdict)}</b><span>Expected losses ${esc(losses)} · ${esc(picture)} · composition ${estimate.composition_age.toFixed(0)}s old.</span></article>`;
}

function refitStacks(g: GhostView): { kind: ShipKind; modules: ModuleKind[]; n: number }[] {
  const out: { kind: ShipKind; modules: ModuleKind[]; n: number }[] = [];
  for (const composition of g.composition ?? []) {
    if ((MODULE_SLOTS[composition.kind] ?? 0) <= 0 || composition.count <= 0) continue;
    const fitted = (g.loadouts ?? []).filter((entry) => entry.kind === composition.kind);
    const stock = Math.max(0, composition.count - fitted.reduce((sum, entry) => sum + entry.n, 0));
    if (stock) out.push({ kind: composition.kind, modules: [], n: stock });
    for (const entry of fitted) if (entry.n > 0) out.push({ kind: entry.kind, modules: entry.modules, n: entry.n });
  }
  return out;
}

function refitTargets(kind: ShipKind, from: ModuleKind[], ledger: Record<string, number>, fits: { kind: ShipKind; modules: ModuleKind[] }[]): ModuleKind[][] {
  const candidates: ModuleKind[][] = [[]];
  for (const module of REFIT_MODULES) candidates.push([module]);
  for (let i = 0; i < REFIT_MODULES.length; i++) for (let j = i; j < REFIT_MODULES.length; j++) candidates.push([REFIT_MODULES[i], REFIT_MODULES[j]]);
  for (const fit of fits) if (fit.kind === kind) candidates.push([...fit.modules]);
  const unique = new Map<string, ModuleKind[]>();
  for (const candidate of candidates) {
    if (fitLegal(kind, candidate) && fitKey(candidate) !== fitKey(from) && ledgerCovers(from, candidate, ledger)) unique.set(fitKey(candidate), candidate);
  }
  return [...unique.values()];
}

function ledgerCovers(from: ModuleKind[], to: ModuleKind[], ledger: Record<string, number>): boolean {
  const before = moduleCounts(from);
  const after = moduleCounts(to);
  return REFIT_MODULES.every((module) => Math.max(0, (after.get(module) ?? 0) - (before.get(module) ?? 0)) <= (ledger[module] ?? 0));
}

function maxRefitCount(from: ModuleKind[], to: ModuleKind[], ledger: Record<string, number>, available: number): number {
  const before = moduleCounts(from);
  const after = moduleCounts(to);
  let max = available;
  for (const module of REFIT_MODULES) {
    const added = Math.max(0, (after.get(module) ?? 0) - (before.get(module) ?? 0));
    if (added > 0) max = Math.min(max, Math.floor((ledger[module] ?? 0) / added));
  }
  return Math.max(0, max);
}

function moduleCounts(modules: ModuleKind[]): Map<ModuleKind, number> {
  const out = new Map<ModuleKind, number>();
  for (const module of modules) out.set(module, (out.get(module) ?? 0) + 1);
  return out;
}

function parseModules(value: string | undefined): ModuleKind[] {
  return value ? value.split(",").filter((entry): entry is ModuleKind => REFIT_MODULES.includes(entry as ModuleKind)) : [];
}
function fitKey(modules: ModuleKind[]): string { return [...modules].sort().join(","); }
function fitName(modules: ModuleKind[]): string { return modules.length ? modules.map((entry) => label(entry)).join(" + ") : "Stock"; }
function normalizeDock(value: string): string { return value.startsWith("E") ? value.slice(1) : value; }

function driveLabel(g: GhostView): string {
  if (g.jump_presumed) return "Jump complete · awaiting report";
  if (g.jump_spool) return g.jump_spool.waiting_for_fuel ? "Jump ready · awaiting fuel" : `Jump spooling · ${Math.ceil(g.jump_spool.remaining)}s`;
  if (g.stalled) return "Fuel exhausted · holding";
  const drive = g.drive;
  if (typeof drive === "object" && "cruising" in drive && drive.cruising === "warp") return "Warp";
  return "Impulse";
}

function fleetTitle(g: GhostView): string {
  if (g.tca && g.kind === "freighter") return g.rescue_service ? "AAA Rescue Tender" : g.migrant ? "Authority Migrant Liner" : "Authority Freighter";
  if (g.tca) return "Authority Enforcement";
  if (g.pirate && g.kind === "raider") return "Pirate Raider";
  return `${shipKindLabel(g.kind)} fleet`;
}

function commandButton(action: string, title: string, copy: string, modifier = ""): string {
  return `<div class="deck-command-block"><button type="button" class="${modifier}" data-deck-act="${action}">${esc(title)}</button><small>${esc(copy)}</small></div>`;
}

function stat(name: string, value: string, tone = ""): string {
  return `<dl class="deck-stat${tone ? ` is-${tone}` : ""}"><dt>${esc(name)}</dt><dd>${esc(value)}</dd></dl>`;
}

function shortEta(seconds: number): string {
  if (seconds < 90) return `~${Math.max(1, Math.ceil(seconds))}s`;
  return fmtEta(seconds);
}

const esc = (value: string): string => value.replace(/[&<>"']/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[char]!);
const escAttr = esc;
