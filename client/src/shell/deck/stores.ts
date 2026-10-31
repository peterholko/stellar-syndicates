// §stores: the stores bar under the top bar and the Stores workbench
// (docs/stores.md). The bar follows your system that is open, else the one
// selected on the map, else home; a switcher pick holds until you navigate or
// select again. Rates are measured from the served stockpiles
// (core/derive/stores.ts). The workbench opens over the map like the
// Construction workbench: the workspace and the camera stay as they are
// (Peter, Oct 28 2026).
import { fmt, fmtBuildDur } from "../../core/derive/format";
import { foundingHomeSystemId } from "../../core/derive/geo";
import { shippableStock } from "../../core/derive/market";
import {
  rateText, stockText, storageState, storeGroups, storeLine, storePins, storesFocus, togglePin, vaultCapacity,
  type StorageState, type StoreLine,
} from "../../core/derive/stores";
import { commodityIcon, label } from "../../icons";
import type { Commodity, EntityId, SystemStateView } from "../../protocol";
import { liveSimTime } from "../../state";
import { renderDeferred, setHtml } from "../dom";
import { sheetFingerprint } from "../signature";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

interface StoresHooks {
  notice(html: string): void;
  /** The bar appeared or went: the map's free area changed. */
  resized(): void;
}

/** Routes that name one of your systems; the bar follows them. */
const SYSTEM_ROUTES = new Set<DeckRoute["name"]>(["system", "world", "build", "shipyard"]);

export class DeckStores {
  /** A system picked in a switcher; it holds until you navigate or select. */
  private picked: EntityId | null = null;
  private pickedFor = "";
  private shown = false;
  /** The route of the last render; the bar's own controls act on it. */
  private route: DeckRoute | null = null;
  private barSignature = "";
  private signature = "";
  /** Whether the Stores workbench is open. */
  private isOpen = false;
  private readonly body: HTMLElement;
  private readonly title: HTMLElement | null;
  /** The bar's own row (the GNN ticker sits after it). */
  private readonly barBody: HTMLElement;

  constructor(
    private readonly bar: HTMLElement,
    /** `#deck-stores-workbench`: centred over the map, not the workspace. */
    private readonly shell: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: StoresHooks,
    signal: AbortSignal,
  ) {
    this.body = shell.querySelector<HTMLElement>("#deck-stores-workbench-body") ?? shell;
    this.title = shell.querySelector<HTMLElement>("#deck-stores-workbench-title");
    this.barBody = bar.querySelector<HTMLElement>("#deck-stores-main") ?? bar;
    for (const surface of [bar, shell]) {
      surface.addEventListener("click", (event) => {
        const button = (event.target as Element).closest<HTMLButtonElement>("button[data-deck-act]");
        if (button) this.handleAction(button, this.route);
      }, { signal });
      surface.addEventListener("change", (event) => {
        if (event.target instanceof HTMLSelectElement) this.handleInput(event.target, this.route);
      }, { signal });
    }
  }

  /** The bar under the top bar; hidden until you hold a system. */
  renderBar(route: DeckRoute | null, force = false): void {
    this.route = route;
    const system = this.focus(route);
    const show = !!system;
    if (show !== this.shown) {
      this.shown = show;
      this.bar.hidden = !show;
      this.bar.closest(".deck")?.classList.toggle("has-stores", show);
      this.hooks.resized();
    }
    if (!system) return;
    const pins = storePins();
    const signature = sheetFingerprint([system.id, system.stockpile, system.storage_used, system.storage_cap,
      system.industry?.reservations, pins, this.owned(), Math.floor(liveSimTime() / 5)]);
    if (!force && signature === this.barSignature) return;
    this.barSignature = signature;
    setHtml(this.barBody, this.barHtml(system, pins));
  }

  /** The Stores workbench, while open. It is never a route. */
  render(route: DeckRoute | null, force = false): void {
    this.route = route;
    if (!this.isOpen) {
      if (!this.shell.hidden) this.shell.hidden = true;
      return;
    }
    const system = this.focus(route);
    const pins = storePins();
    const signature = sheetFingerprint([system?.id, system?.stockpile, system?.storage_used, system?.storage_cap,
      system?.industry?.reservations, system?.blockade?.held_since ?? null, pins, this.owned(), Math.floor(liveSimTime() / 5)]);
    if (!force && signature === this.signature && !this.shell.hidden) return;
    if (renderDeferred(this.body.id, () => this.render(route, true))) return;
    this.signature = signature;
    if (this.title) this.title.textContent = system ? this.systemName(system.id) : "Stores";
    setHtml(this.body, system ? this.popHtml(system, pins) : `<p class="deck-muted">Your systems' goods show here.</p>`);
    this.shell.hidden = false;
  }

  /** Open the workbench on `id` (else the bar's system). */
  open(route: DeckRoute | null, id?: EntityId): void {
    this.isOpen = true;
    this.focus(route);
    if (id && this.owned().some((system) => system.id === id)) this.picked = id;
    this.signature = "";
    this.render(route, true);
  }

  /** Close the workbench. True when it was open, so Esc stops here. */
  close(): boolean {
    if (!this.isOpen) return false;
    this.isOpen = false;
    this.signature = "";
    this.shell.hidden = true;
    return true;
  }

  /** Shortcut I: open on the bar's system, or close. */
  toggle(route: DeckRoute | null): void {
    if (!this.close()) this.open(route);
  }

  handleAction(button: HTMLButtonElement, route: DeckRoute | null): boolean {
    const act = button.dataset.deckAct;
    if (act === "stores-open") {
      this.open(route, button.dataset.system);
      return true;
    }
    if (act === "stores-close") {
      this.close();
      return true;
    }
    if (act === "stores-pin") {
      togglePin(button.dataset.good as Commodity);
      this.refresh(route);
      return true;
    }
    if (act === "stores-ship") {
      const system = this.focus(route);
      if (!system || system.owner !== this.ctx.state.playerId || system.blockade) return true;
      const manifest = shippableStock(system);
      if (!manifest.length) return true;
      this.ctx.send({ type: "ShipProduction", system_id: system.id });
      this.hooks.notice(`<b>Authority pickup requested</b> · ${manifest.map((slot) => `${fmt(slot.units)} ${esc(label(slot.commodity))}`).join(" · ")} → Market Hub.`);
      return true;
    }
    return false;
  }

  handleInput(target: HTMLInputElement | HTMLSelectElement, route: DeckRoute | null): boolean {
    if (!(target instanceof HTMLSelectElement) || !("storesSystem" in target.dataset)) return false;
    this.picked = target.value;
    this.refresh(route);
    return true;
  }

  invalidate(): void {
    this.barSignature = "";
    this.signature = "";
  }

  private refresh(route: DeckRoute | null): void {
    this.invalidate();
    this.renderBar(route, true);
    this.render(route, true);
  }

  private owned(): SystemStateView[] {
    const st = this.ctx.state;
    return st.systems.filter((system) => system.owner === st.playerId && system.owner !== null);
  }

  private focus(route: DeckRoute | null): SystemStateView | undefined {
    const st = this.ctx.state;
    const owned = this.owned();
    const ids = new Set(owned.map((system) => system.id));
    const open = route && SYSTEM_ROUTES.has(route.name) ? route.params?.systemId ?? route.params?.id ?? null : null;
    const context = JSON.stringify([route?.name ?? null, open, st.selectedSystemId]);
    if (context !== this.pickedFor) {
      this.pickedFor = context;
      this.picked = null;
    }
    const id = this.picked && ids.has(this.picked) ? this.picked : storesFocus(ids, open, st.selectedSystemId, foundingHomeSystemId());
    return owned.find((system) => system.id === id);
  }

  private systemName(id: EntityId): string {
    return this.ctx.state.galaxy?.systems.find((system) => system.id === id)?.name ?? id;
  }

  private switcher(system: SystemStateView): string {
    const owned = this.owned();
    if (owned.length < 2) return `<b class="deck-stores__system">${esc(this.systemName(system.id))}</b>`;
    const options = owned.map((entry) => ({ id: entry.id, name: this.systemName(entry.id) }))
      .sort((a, b) => a.name.localeCompare(b.name))
      .map((entry) => `<option value="${esc(entry.id)}"${entry.id === system.id ? " selected" : ""}>${esc(entry.name)}</option>`).join("");
    return `<select class="deck-stores__system" data-stores-system aria-label="System">${options}</select>`;
  }

  private barHtml(system: SystemStateView, pins: readonly Commodity[]): string {
    const goods = pins.map((commodity) => {
      const line = storeLine(system, commodity);
      const rate = rateText(line.rate);
      return `<button type="button" class="deck-stores__good${line.low ? " is-low" : ""}" data-deck-act="stores-open" data-system="${esc(system.id)}" title="${esc(lineTip(line))}">`
        + `${goodIcon(commodity)}<b>${stockText(line.units)}</b>${rate ? `<small class="${line.rate! > 0 ? "is-up" : "is-down"}">${rate}</small>` : ""}</button>`;
    }).join("");
    return `${this.switcher(system)}<div class="deck-stores__goods">${goods}</div>${storageMeter(storageState(system), system.id)}`
      + `<button type="button" class="deck-stores__all" data-deck-act="stores-open" data-system="${esc(system.id)}" title="Every good · I">All goods</button>`;
  }

  private popHtml(system: SystemStateView, pins: readonly Commodity[]): string {
    const storage = storageState(system);
    const rows = storeGroups(system, pins).map((group) => `<section class="deck-stores-group"><h3>${esc(group.title)}</h3>`
      + group.lines.map((line) => rowHtml(line, pins.includes(line.commodity))).join("") + `</section>`).join("");
    const manifest = shippableStock(system);
    const units = manifest.reduce((sum, slot) => sum + slot.units, 0);
    const blocked = (system.blockade?.held_since ?? null) !== null;
    const vault = vaultCapacity(system);
    // §player-styles: shown once raids have taken something today.
    const raided = this.ctx.state.raidLosses;
    return `<section class="deck-stores-pop"><header class="deck-stores-pop__head">${this.switcher(system)}`
      + (vault ? `<span class="deck-stores-pop__vault" title="Raiders can't reach it. Dearest goods first."><small>Vault</small><b>${fmt(vault)}</b></span>` : "")
      + (raided && raided.taken > 0 ? `<span class="deck-stores-pop__raided" title="Raids take at most a quarter of your storage a day."><small>Raided today</small><b>${fmt(raided.taken)}${raided.cap !== null ? ` / ${fmt(raided.cap)}` : ""}</b></span>` : "")
      + `<span class="deck-stores-pop__storage${storage.full ? " is-full" : storage.warn ? " is-warn" : ""}"><small>Storage</small>${meter(storage)}`
      + `<b>${fmt(storage.used)} / ${fmt(storage.cap)}</b>${storage.full ? "<small>Full</small>" : storage.fullIn !== null ? `<small>full in ${fmtBuildDur(storage.fullIn)}</small>` : ""}</span></header>`
      + (rows || `<p class="deck-muted">Empty.</p>`)
      + `<div class="deck-section__action"><button type="button" class="is-primary" data-deck-act="stores-ship" ${blocked || !manifest.length ? "disabled" : ""}>Ship ${units ? fmt(units) : "no"} goods to Market</button>`
      + `<span>Fuels stay in reserve.</span></div></section>`;
  }
}

function rowHtml(line: StoreLine, pinned: boolean): string {
  const rate = rateText(line.rate);
  return `<div class="deck-stores-row${line.low ? " is-low" : ""}">${goodIcon(line.commodity)}`
    + `<span class="deck-stores-row__name">${esc(label(line.commodity))}</span>`
    + `<b>${fmt(line.units)}</b>`
    + `<small class="deck-stores-row__held"${line.safe > 0 ? ` title="Out of raiders' reach"` : ""}>${[line.held > 0 ? `${fmt(line.held)} held` : "", line.safe > 0 ? `${fmt(line.safe)} safe` : ""].filter(Boolean).join(" · ")}</small>`
    + `<span class="deck-stores-row__rate${rate ? (line.rate! > 0 ? " is-up" : " is-down") : ""}">${rate ? `${rate}/min` : ""}</span>`
    + `<span class="deck-stores-row__eta">${line.emptyIn !== null ? `empty ${fmtBuildDur(line.emptyIn)}` : ""}</span>`
    + `<button type="button" class="deck-stores-row__pin" data-deck-act="stores-pin" data-good="${line.commodity}" aria-pressed="${pinned}" title="${pinned ? "Unpin from bar" : "Pin to bar"}">${pinned ? "★" : "☆"}</button></div>`;
}

function storageMeter(storage: StorageState, systemId: EntityId): string {
  const tip = `Storage ${fmt(storage.used)} / ${fmt(storage.cap)}${storage.full ? " · full, production stops" : storage.fullIn !== null ? ` · full in ${fmtBuildDur(storage.fullIn)}` : ""}`;
  return `<button type="button" class="deck-stores__storage${storage.full ? " is-full" : storage.warn ? " is-warn" : ""}" data-deck-act="stores-open" data-system="${esc(systemId)}" title="${esc(tip)}">`
    + `<small>Storage</small>${meter(storage)}<b>${stockText(storage.used)}/${stockText(storage.cap)}</b></button>`;
}

function meter(storage: StorageState): string {
  return `<span class="deck-stores__meter"><i style="width:${Math.min(100, Math.round(storage.fill * 100))}%"></i></span>`;
}

function lineTip(line: StoreLine): string {
  const rate = rateText(line.rate);
  return [`${label(line.commodity)} ${fmt(line.units)}`, line.held > 0 ? `${fmt(line.held)} held` : "", rate ? `${rate}/min` : "",
    line.emptyIn !== null ? `empty in ${fmtBuildDur(line.emptyIn)}` : ""].filter(Boolean).join(" · ");
}

/** The good's icon without its own tooltip, so the button's shows. */
function goodIcon(commodity: Commodity): string {
  return commodityIcon(commodity).replace(/ title="[^"]*"/, "");
}

function esc(value: string): string {
  return value.replace(/[&<>"']/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[char]!);
}
