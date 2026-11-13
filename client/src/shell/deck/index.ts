import "../../styles/deck.css";
import { coach, CONTRACT_COACH } from "../coach";
import { commsBadge } from "../../core/derive/social";
import { bindAccountForm } from "../account";

import { bindFleetNet, guardCapable as guardCapableForKey, jumpCapable as jumpCapableForKey, shipKindLabel } from "../../core/derive/fleet";
import { bindMarketDerive, spendableMarketCredits } from "../../core/derive/market";
import type { CoreEvent } from "../../core/events";
import { formatId } from "../../protocol";
import { battleNotice, reportNotice } from "../../core/derive/battlenotice";
import { installPressGuard } from "../dom";
import { tickLiveProgress } from "../liveprogress";
import { sheetFingerprint } from "../signature";
import type { CoreContext, Rect, Shell } from "../types";
import { installDeckDebug } from "./debug";
import { DeckEmpireRoutes } from "./empire";
import { type SelectTarget } from "../../core/mapclick";
import { DeckMapInteraction } from "./map";
import { fitDeckNav, mountDeckMarkup, syncNavMoreBadge } from "./markup";
import { DeckNewsTicker } from "./news";
import { NAME_LAYERS, loadNameLayers, saveNameLayers, type NameLayers } from "../../core/map-layers";
import { homeStepSystem, noteHomeOpened, noteHomeWorldOpened } from "../../core/derive/founding";
import { colonyName, pirateNames } from "../../core/derive/vendetta";
import { foundingHomeSystemId, systemName } from "../../core/derive/geo";
import { logToast } from "../../core/derive/history";
import { DECK_ROUTES, DeckRouter, type DeckCrumb, type DeckRoute, type DeckRouteName } from "./router";
import { DeckCommandStrip } from "./strip";
import { DeckFoundingGuide } from "./founding-guide";
import { deckTradeNotice, DeckMarketRoutes } from "./market";
import { DeckMerchantRoutes } from "./merchant";
import { DeckStores } from "./stores";
import { DeckShipyardRoutes } from "./shipyard";
import { DeckSignalsRoutes } from "./signals";
import { openSignals } from "../../core/derive/signals";
import { DeckPolicyRoutes } from "./policy";
import { closeIndustryMenu } from "../industry";
import { DeckFleetRoutes } from "./fleet";
import { DeckLogRoutes } from "./log";
import { bindResearchNet } from "../../core/derive/research";
import { gameClock, informationDelay } from "../../core/derive/format";
import { traderOf } from "../../core/derive/trader";
import { isProjectKey, isStructureJob, needsWorkforce, slotLine, worldSlots } from "../../core/derive/worldbuild";
import { icon, label as buildLabel } from "../../icons";
import { liveSimTime } from "../../state";
import { setHtml } from "../dom";
import { DeckBottomBand } from "./bottom-band";
import { DeckRosterRoutes } from "./roster";
import { DeckStrategicRoutes } from "./strategic";
import { DeckTheaters } from "./theaters";
import { DeckToasts } from "./toasts";
import { DeckWorkspace } from "./workspace";
import { DeckWelcome } from "./welcome";

const byId = <T extends HTMLElement = HTMLElement>(id: string): T => document.getElementById(id) as T;

class DeckShell implements Shell {
  private root: HTMLElement | null = null;
  private ctx: CoreContext | null = null;
  private abort: AbortController | null = null;
  private router: DeckRouter | null = null;
  private workspace: DeckWorkspace | null = null;
  private map: DeckMapInteraction | null = null;
  private toasts: DeckToasts | null = null;
  private strip: DeckCommandStrip | null = null;
  private empire: DeckEmpireRoutes | null = null;
  private guide: DeckFoundingGuide | null = null;
  private welcome: DeckWelcome | null = null;
  private market: DeckMarketRoutes | null = null;
  private merchant: DeckMerchantRoutes | null = null;
  private stores: DeckStores | null = null;
  private news: DeckNewsTicker | null = null;
  private shipyard: DeckShipyardRoutes | null = null;
  private signals: DeckSignalsRoutes | null = null;
  private policy: DeckPolicyRoutes | null = null;
  private fleet: DeckFleetRoutes | null = null;
  private log: DeckLogRoutes | null = null;
  private roster: DeckRosterRoutes | null = null;
  private strategic: DeckStrategicRoutes | null = null;
  private theaters: DeckTheaters | null = null;
  private bottomBand: DeckBottomBand | null = null;
  private removeDebug: (() => void) | null = null;
  private chromeSignature = "";
  /** §construction: the top bar's every-world construction menu is open. */
  private constructionMenu = false;
  private zoomSignature = "";
  private activeCrumbs: readonly DeckCrumb[] = [];

  async mount(root: HTMLElement, ctx: CoreContext): Promise<void> {
    this.root = root;
    this.ctx = ctx;
    this.abort = new AbortController();
    mountDeckMarkup(root);
    ctx.renderer.setDeckSaliency(true);
    // §tracking: a selected fleet in the System View gets a zoomed camera.
    ctx.renderer.setTrackingInset(true);
    installPressGuard();
    const signal = this.abort.signal;
    this.router = new DeckRouter((route, stack) => this.routeChanged(route, stack), signal);
    this.workspace = new DeckWorkspace(byId("deck-workspace"), ctx.renderer, signal);
    this.map = new DeckMapInteraction(ctx, {
      enteredSystem: (system) => this.router?.go({ name: "system", params: { id: system.id, systemLabel: system.name } }),
      enteredBattle: (id) => this.theaters?.enterBattle(id),
      watchSystemBattle: (id, systemId) => {
        const running = this.ctx?.state.battles.some((battle) => battle.id === id) ?? false;
        this.router?.go({ name: "battle", params: { id, label: running ? "Ongoing battle" : "Battle replay", systemId } });
        this.theaters?.openBattle(id);
      },
      returnedToGalaxy: () => {
        if (this.router?.current?.name === "system" || this.router?.current?.name === "world" || this.router?.current?.name === "build") {
          this.router.back();
        }
      },
      openTarget: (target) => this.openMapTarget(target),
      diveOnSelect: (id) => homeStepSystem(ctx.state, foundingHomeSystemId()) === id,
      notice: (html) => this.setStatus(html),
      clearNotice: () => this.strip?.clearStatus(),
    }, signal);
    // A world route climbs the zoom ladder like the map does, so Back lands on its system.
    this.toasts = new DeckToasts(byId("deck-toast-lane"), (route) => route.name === "world"
      ? this.openWorldPanel(route.params?.systemId ?? "", Number(route.params?.bodyId), route.query)
      : this.router?.go(route), signal);
    this.strip = new DeckCommandStrip(byId("deck-command-strip"), ctx, {
      notice: (html) => this.setStatus(html),
    }, signal);
    this.bottomBand = new DeckBottomBand(
      byId("deck-bottom-band"),
      byId("deck-command-strip"),
      byId("deck-founding"),
      byId("deck-zoom"),
      () => this.workspace?.publishCameraRect(true),
      signal,
    );
    this.empire = new DeckEmpireRoutes(byId("deck-workspace-body"), byId("deck-build-workbench-body"), byId("deck-planet-stage-body"), ctx, {
      go: (route) => this.router?.go(route),
      replace: (route) => this.router?.replace(route),
      openGroundViewer: (id) => this.theaters?.openGround(id),
      openWorldBuild: (systemId, bodyId, query) => this.openWorldBuild(systemId, bodyId, query),
      underlay: () => this.popoutWorld(),
      notice: (html) => this.setStatus(html),
      toast: (title, message, tone, destination) => this.toasts?.push({ title, message, tone, destination }),
    });
    this.log = new DeckLogRoutes(byId("deck-workspace-body"), ctx, {
      go: (route) => this.router?.go(route),
      focusSystem: (id) => this.focusInboxSystem(id),
      focusFleet: (id) => this.focusInboxFleet(id),
      notice: (text) => this.setStatus(escapeHtml(text)),
    });
    this.welcome = new DeckWelcome(byId("deck-welcome"), ctx, {
      runFounding: () => this.guide?.runFoundingPrimary(),
      openLog: () => this.router?.go({ name: "log" }),
      go: (route) => this.router?.go(route),
    }, signal);
    this.guide = new DeckFoundingGuide(byId("deck-founding"), ctx, {
      go: (route) => this.router?.go(route),
      openWorld: (systemId, bodyId, structure) => {
        if (structure) this.empire?.focusWorldStructure(structure);
        this.openWorldPanel(systemId, bodyId);
      },
      openWorldBuild: (systemId, bodyId, query, mode) => this.openWorldBuild(systemId, bodyId, query, mode),
      focusFleet: (id) => this.map?.focusFleet(id),
      openFleetDetails: (id) => this.fleet?.openDetails(id),
      flyIntoSystem: (id) => this.flyIntoSystem(id),
      followShipment: (id) => {
        const home = foundingHomeSystemId();
        if (!home) return;
        this.focusShipment(id, home);
        this.ctx?.renderer.trackShipment(id, home);
      },
    });
    this.market = new DeckMarketRoutes(byId("deck-workspace-body"), ctx, {
      go: (route) => this.router?.go(route),
      notice: (html) => this.setStatus(html),
    });
    this.stores = new DeckStores(byId("deck-stores"), byId("deck-stores-workbench"), ctx, {
      notice: (html) => this.setStatus(html),
      resized: () => this.workspace?.publishCameraRect(),
    }, signal);
    this.news = new DeckNewsTicker(byId("deck-news") as HTMLButtonElement, ctx,
      () => this.router?.go({ name: "log", query: { tab: "news" } }), signal);
    this.merchant = new DeckMerchantRoutes(byId("deck-workspace-body"), ctx, {
      go: (route) => this.router?.go(route),
      notice: (html) => this.setStatus(html),
    });
    this.shipyard = new DeckShipyardRoutes(byId("deck-workspace-body"), ctx, {
      go: (route) => this.router?.go(route),
    });
    this.signals = new DeckSignalsRoutes(byId("deck-workspace-body"), ctx, {
      go: (route) => this.router?.go(route),
      notice: (html) => this.setStatus(html),
    });
    this.policy = new DeckPolicyRoutes(byId("deck-workspace-body"), ctx, {
      go: (route) => this.router?.go(route),
      notice: (html) => this.setStatus(html),
    });
    this.fleet = new DeckFleetRoutes(byId("deck-workspace-body"), ctx, {
      go: (route) => this.router?.go(route),
      notice: (html) => this.setStatus(html),
    }, byId("deck-fleet-workbench-body"));
    this.roster = new DeckRosterRoutes(byId("deck-workspace-body"), ctx, {
      go: (route) => this.router?.go(route),
      openWorld: (systemId, bodyId) => this.openWorldPanel(systemId, bodyId),
      notice: (html) => this.setStatus(html),
    });
    this.strategic = new DeckStrategicRoutes(byId("deck-workspace-body"), ctx, {
      go: (route) => this.router?.go(route),
      replace: (route) => this.router?.replace(route),
      openWorld: (systemId, bodyId) => this.openWorldPanel(systemId, bodyId),
      back: () => this.router?.back(),
      notice: (html) => this.setStatus(html),
      openBattleViewer: (id) => this.theaters?.openBattle(id),
      selectFleets: (ids) => {
        if (!ids.length) return;
        this.map?.focusFleet(ids[0]);
        for (const id of ids) ctx.state.selectedShipIds.add(id);
        ctx.renderer.stateVersion++;
      },
    });
    this.theaters = new DeckTheaters(
      byId("deck-battle-theater"),
      byId("deck-battle-theater-card"),
      byId("deck-ground-theater"),
      byId("deck-ground-theater-card"),
      ctx,
      {
        go: (route) => this.router?.go(route),
        openDoctrine: () => this.router?.go({ name: "doctrine" }),
        notice: (html) => this.setStatus(html),
        battleSelectionChanged: () => this.syncBattleSelection(),
      },
      signal,
    );
    bindFleetNet(() => ctx.net);
    bindResearchNet(() => ctx.net);
    bindMarketDerive(() => ctx.net, () => this.empire?.composedFit ?? [], () => this.empire?.composedReactor ?? "fission",
      () => this.empire?.composedDrive ?? "zephyr");
    this.removeDebug = installDeckDebug(
      ctx,
      (id) => this.theaters?.openBattle(id),
      (id) => this.theaters?.openGround(id),
      (id) => this.theaters?.enterBattle(id),
    );
    bindAccountForm("deck", ctx, signal);
    byId("deck-nav").addEventListener("click", (event) => {
      const action = (event.target as Element).closest<HTMLButtonElement>("[data-deck-act]");
      if (action?.dataset.deckAct === "nav-more") {
        this.setNavOverflow(byId("deck-nav-overflow").hasAttribute("hidden"));
        return;
      }
      const button = action?.closest<HTMLButtonElement>("[data-deck-act=route]");
      const route = button?.dataset.route as DeckRouteName | undefined;
      if (route && route in DECK_ROUTES) this.openRoute(route);
    }, { signal });
    byId("deck-nav-overflow").addEventListener("click", (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>("[data-deck-act=route]");
      const route = button?.dataset.route as DeckRouteName | undefined;
      if (!route || !(route in DECK_ROUTES)) return;
      this.setNavOverflow(false);
      this.openRoute(route);
    }, { signal });
    // §medals: your corporation's card.
    byId("deck-identity").addEventListener("click", () => {
      const me = ctx.state.playerId;
      if (me !== null) this.router?.go({ name: "rankings", query: { corp: String(me) } });
    }, { signal });
    byId("deck-merchant-chip").addEventListener("click", (event) => {
      if ((event.target as Element).closest("[data-deck-act=merchant-chip]")) this.router?.go({ name: "merchant" });
    }, { signal });
    byId("deck-signals-chip").addEventListener("click", (event) => {
      if ((event.target as Element).closest("[data-deck-act=signals-chip]")) this.router?.go({ name: "signals" });
    }, { signal });
    byId("deck-construction-chip").addEventListener("click", (event) => {
      if ((event.target as Element).closest("[data-deck-act=construction-chip]")) this.setConstructionMenu(!this.constructionMenu);
    }, { signal });
    byId("deck-construction-menu").addEventListener("click", (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>("[data-deck-act=construction-open]");
      if (!button) return;
      this.setConstructionMenu(false);
      const systemId = button.dataset.system ?? "", bodyId = Number(button.dataset.body);
      if (button.dataset.build) this.openWorldBuild(systemId, bodyId, {});
      else this.openWorldPanel(systemId, bodyId);
    }, { signal });
    window.addEventListener("pointerdown", (event) => {
      const target = event.target;
      if (!(target instanceof Node)) return;
      if (!byId("deck-construction-chip").contains(target) && !byId("deck-construction-menu").contains(target)) this.setConstructionMenu(false);
      if (!byId("deck-layers").contains(target) && !byId("deck-zoom").contains(target)) this.setLayersOpen(false);
      if (byId("deck-nav").contains(target) || byId("deck-nav-overflow").contains(target)) return;
      this.setNavOverflow(false);
    }, { signal });
    window.addEventListener("resize", () => this.setNavOverflow(false), { signal });
    // The top bar refits whenever its row or a neighbour (credits, the
    // construction chip) changes size.
    const navFit = new ResizeObserver(() => fitDeckNav());
    for (const id of ["deck-topbar", "deck-identity", "deck-stats", "deck-construction-chip", "deck-merchant-chip", "deck-signals-chip", "deck-profile-open"]) navFit.observe(byId(id));
    signal.addEventListener("abort", () => navFit.disconnect());
    byId("deck-workspace").addEventListener("click", (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>("[data-deck-act]");
      if (!button) return;
      if (this.roster?.handleAction(button, this.router?.current ?? null)) return;
      if (this.strategic?.handleAction(button, this.router?.current ?? null)) return;
      if (this.fleet?.handleAction(button, this.router?.current ?? null)) return;
      if (this.log?.handleAction(button, this.router?.current ?? null)) return;
      if (this.empire?.handleAction(button, this.router?.current ?? null)) return;
      if (this.market?.handleAction(button, this.router?.current ?? null)) return;
      if (this.stores?.handleAction(button, this.router?.current ?? null)) return;
      if (this.merchant?.handleAction(button, this.router?.current ?? null)) return;
      if (this.shipyard?.handleAction(button, this.router?.current ?? null)) return;
      if (this.signals?.handleAction(button, this.router?.current ?? null)) return;
      if (this.policy?.handleAction(button, this.router?.current ?? null)) return;
      if (button.dataset.deckAct === "back") this.router?.back();
      else if (button.dataset.deckAct === "close") this.closeWorkspace();
      else if (button.dataset.deckAct === "width") this.workspace?.toggleWidth();
      else if (button.dataset.deckAct === "breadcrumb") {
        const crumb = this.activeCrumbs[Number(button.dataset.crumbIndex)];
        if (crumb?.route) this.router?.go(crumb.route);
        else if (crumb) this.router?.close();
      }
    }, { signal });
    const workspaceInput = (event: Event) => {
      const target = event.target;
      if (target instanceof HTMLInputElement || target instanceof HTMLSelectElement) {
        // Native selects and checkboxes emit both input and change. Route each
        // control through one event only so a doctrine selection sends once.
        if (event.type === "input" && (target instanceof HTMLSelectElement || target.type === "checkbox")) return;
        if (event.type === "change" && target instanceof HTMLInputElement && target.type !== "checkbox") return;
        if (this.fleet?.handleInput(target, this.router?.current ?? null)) return;
        if (this.market?.handleInput(target, this.router?.current ?? null)) return;
        if (this.stores?.handleInput(target, this.router?.current ?? null)) return;
        if (this.merchant?.handleInput(target, this.router?.current ?? null)) return;
        if (this.strategic?.handleInput(target, this.router?.current ?? null)) return;
        this.policy?.handleInput(target, this.router?.current ?? null);
      }
    };
    byId("deck-workspace").addEventListener("input", workspaceInput, { signal });
    byId("deck-workspace").addEventListener("change", workspaceInput, { signal });
    // The fleet details workbench carries the same fleet controls (refits,
    // posture, tending), so it routes through the same handlers.
    byId("deck-fleet-workbench").addEventListener("input", workspaceInput, { signal });
    byId("deck-fleet-workbench").addEventListener("change", workspaceInput, { signal });
    byId("deck-fleet-workbench").addEventListener("click", (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>("[data-deck-act]");
      if (button) this.fleet?.handleAction(button, this.router?.current ?? null);
    }, { signal });
    byId("deck-fleet-workbench").addEventListener("keydown", (event) => {
      const input = event.target;
      if (event.key !== "Enter" || !(input instanceof HTMLInputElement)) return;
      const action = input.dataset.deckEnter;
      if (!action) return;
      const scope = input.closest<HTMLElement>(".deck-inline-confirm, .deck-inline-form") ?? byId("deck-fleet-workbench");
      const submit = scope.querySelector<HTMLButtonElement>(`[data-deck-act="${action}"]`);
      if (!submit || submit.disabled) return;
      event.preventDefault();
      submit.click();
    }, { signal });
    byId("deck-workspace").addEventListener("keydown", (event) => {
      const input = event.target;
      if (event.key !== "Enter" || !(input instanceof HTMLInputElement)) return;
      const action = input.dataset.deckEnter;
      if (!action) return;
      const scope = input.closest<HTMLElement>(".deck-inline-confirm, .deck-inline-form") ?? byId("deck-workspace");
      const submit = scope.querySelector<HTMLButtonElement>(`[data-deck-act="${action}"]`);
      if (!submit || submit.disabled) return;
      event.preventDefault();
      submit.click();
    }, { signal });
    byId("deck-build-workbench").addEventListener("click", (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>("[data-deck-act]");
      if (button) this.empire?.handleAction(button, this.router?.current ?? null);
    }, { signal });
    byId("deck-build-workbench").addEventListener("keydown", (event) => {
      const input = event.target;
      if (event.key !== "Enter" || !(input instanceof HTMLInputElement)) return;
      const action = input.dataset.deckEnter;
      if (!action) return;
      const scope = input.closest<HTMLElement>(".deck-inline-confirm, .deck-inline-form") ?? byId("deck-build-workbench");
      const submit = scope.querySelector<HTMLButtonElement>(`[data-deck-act="${action}"]`);
      if (!submit || submit.disabled) return;
      event.preventDefault();
      submit.click();
    }, { signal });
    byId("deck-planet-stage").addEventListener("click", (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>("[data-deck-act]");
      if (button) this.empire?.handleAction(button, this.router?.current ?? null);
    }, { signal });
    // The planet is the deepest rung of the zoom ladder: wheeling out over its
    // scene climbs back to the orrery, as the orrery's own scrub-out does.
    let stageWheel = 0;
    byId("deck-planet-stage").addEventListener("wheel", (event) => {
      if (this.router?.current?.name !== "world") return;
      stageWheel = event.deltaY < 0 ? 0 : stageWheel + event.deltaY;
      if (stageWheel < 240) return;
      stageWheel = 0;
      this.router.back();
    }, { passive: true, signal });
    byId("deck-founding").addEventListener("click", (event) => {
      const button = (event.target as Element).closest<HTMLButtonElement>("[data-deck-act]");
      if (button) this.guide?.handleFoundingAction(button);
    }, { signal });
    byId("deck-zoom").addEventListener("click", (event) => {
      const action = (event.target as Element).closest<HTMLButtonElement>("[data-deck-act]")?.dataset.deckAct;
      if (action === "zoom-in") this.zoomIn();
      else if (action === "zoom-out") this.zoomOut();
      else if (action === "zoom-fit") this.zoomFit();
      else if (action === "layers") this.setLayersOpen(byId("deck-layers").hasAttribute("hidden"));
      else if (action === "help") this.setHelpOpen(true);
    }, { signal });
    // §map-layers: which systems' names the map shows.
    const layers = loadNameLayers();
    byId("deck-layers").innerHTML = `<small>Names</small>${NAME_LAYERS.map(({ id, label }) =>
      `<label><input type="checkbox" data-name-layer="${id}"${layers[id] ? " checked" : ""}> ${label}</label>`).join("")}`;
    byId("deck-layers").addEventListener("change", () => {
      const next = { ...layers } as NameLayers;
      for (const box of byId("deck-layers").querySelectorAll<HTMLInputElement>("input[data-name-layer]")) {
        next[box.dataset.nameLayer as keyof NameLayers] = box.checked;
      }
      saveNameLayers(next);
      ctx.renderer.setNameLayers(next);
    }, { signal });
    // The deck's own Esc skips inputs, and focus sits on a checkbox here.
    byId("deck-layers").addEventListener("keydown", (event) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      this.setLayersOpen(false);
      document.querySelector<HTMLElement>('#deck-zoom [data-deck-act="layers"]')?.focus();
    }, { signal });
    byId("deck-overlays").addEventListener("click", (event) => {
      const action = (event.target as Element).closest<HTMLButtonElement>("[data-deck-act]")?.dataset.deckAct;
      if (action === "close-help") this.setHelpOpen(false);
      // §gun-platform: the sim refuses it unless the colony fell.
      if (action === "restart-corporation") ctx.send({ type: "RestartCorporation" });
    }, { signal });
    window.addEventListener("keydown", (event) => this.keyDown(event), { signal });
    this.syncSessionVisibility();
    this.renderChrome(true);
    if (ctx.state.playerId === null) requestAnimationFrame(() => byId<HTMLInputElement>("deck-login")?.focus());
  }

  /** §raid-focus (Oct 26 2026): out of a planet or system view to the galaxy
   * map, the raider selected with its panel open, and one camera move framing
   * it with the home it closes on (§raid-frame, Oct 27), taken once the panel
   * has narrowed the map. A battle theater keeps the player. */
  focusRaid(id: string, homeId: string): void {
    const renderer = this.ctx?.renderer;
    const raider = this.ctx?.state.ghosts.find((entry) => entry.id === id);
    const home = this.ctx?.state.galaxy?.systems.find((entry) => entry.id === homeId);
    if (!renderer || !this.map || !raider || !home || renderer.viewMode.type === "battle") return;
    if (renderer.viewMode.type === "system") {
      renderer.exitSystemView();
      renderer.setSystemDynamic([], [], true);
    }
    this.map.focusFleet(id);
    renderer.frameRaid(raider.pos, home.pos);
  }

  /** §privateer-contract: Contracts, the charter's contract on top, its
   * Accept ringed. */
  openContract(): void {
    this.router?.go({ name: "operations" });
    coach("contract-accept", CONTRACT_COACH);
  }

  /** §export-follow (Peter, Nov 2 2026): the first shipment's departure takes
   * the player to the freighter the way a raid's sighting does: out to the
   * galaxy map, it selected, one camera move framing it with home; boot.ts
   * puts the tracking window on it until it warps at the hyperlimit. */
  focusShipment(id: string, homeId: string): void {
    const renderer = this.ctx?.renderer;
    const freighter = this.ctx?.state.ghosts.find((entry) => entry.id === id);
    const home = this.ctx?.state.galaxy?.systems.find((entry) => entry.id === homeId);
    if (!renderer || !this.map || !freighter || !home || renderer.viewMode.type === "battle") return;
    if (renderer.viewMode.type === "system") {
      renderer.exitSystemView();
      renderer.setSystemDynamic([], [], true);
    }
    this.map.focusFleet(id);
    renderer.frameRaid(freighter.pos, home.pos);
  }

  onCore(events: CoreEvent[]): void {
    this.empire?.onCore(events, this.router?.current ?? null);
    this.market?.onCore(events, this.router?.current ?? null);
    this.log?.onCore(events, this.router?.current ?? null);
    for (const event of events) {
      const fleetAbsorbed = this.fleet?.onCoreEvent(event, this.router?.current ?? null) ?? false;
      if (event.kind === "Welcomed") {
        this.syncSessionVisibility();
      } else if (event.kind === "SessionReplaced") {
        byId("deck-join-error").textContent = "Session ended. Please sign in again.";
        byId<HTMLButtonElement>("deck-join-button").disabled = false;
        this.syncSessionVisibility();
      } else if (event.kind === "JoinRejected") {
        byId("deck-join-error").textContent = event.message;
        byId<HTMLButtonElement>("deck-join-button").disabled = false;
      } else if (event.kind === "TransportError" && this.ctx?.state.playerId === null) {
        byId("deck-join-error").textContent = `Could not reach server at ${event.url}.`;
        byId<HTMLButtonElement>("deck-join-button").disabled = false;
      } else if (event.kind === "ProtocolMismatch") {
        console.warn(`protocol mismatch: server v${event.server}, client expects v${event.client}`);
      }
      if (event.kind === "IntentChanged" && event.readout !== undefined) {
        if (event.readout) this.setStatus(event.readout);
        else this.strip?.clearStatus();
      } else if (event.kind === "OrderConfirmed") {
        const fleet = this.ctx?.state.ghosts.find((entry) => entry.id === event.shipId);
        this.setStatus(`<b>Order received</b> · ${escapeHtml(humanize(event.orderKind))}${fleet ? ` · ${escapeHtml(shipKindLabel(fleet.kind))}` : ""}`);
      } else if (event.kind === "ServerError" || event.kind === "CommandRejected") {
        this.setStatus(`<span class="deck-command-status__error"><b>Command refused</b> · ${escapeHtml(event.message)}</span>`);
      }
      if (!fleetAbsorbed) this.toastFor(event);
    }
    this.syncSessionVisibility();
    this.renderChrome(true);
    this.strip?.render(true);
  }

  onViewTick(): void {
    this.map?.tick();
    const mode = this.ctx?.renderer.viewMode;
    // §home-step: the guide's first step is done once home's System View is open.
    if (this.ctx && mode?.type === "system" && !this.ctx.state.homeOpened
      && noteHomeOpened(this.ctx.state, mode.systemId, foundingHomeSystemId())) this.guide?.invalidate();
    // §home-world-step: the second once one of its worlds is open.
    const route = this.router?.current;
    if (this.ctx && route?.name === "world" && !this.ctx.state.homeWorldOpened
      && noteHomeWorldOpened(this.ctx.state, route.params?.systemId ?? null, foundingHomeSystemId())) this.guide?.invalidate();
    this.strip?.render();
    this.renderChrome();
    this.stores?.renderBar(this.router?.current ?? null);
    this.news?.render();
    this.renderZoom();
    this.empire?.render(this.router?.current ?? null);
    this.guide?.render();
    this.welcome?.render();
    this.market?.render(this.router?.current ?? null);
    this.merchant?.render(this.router?.current ?? null);
    this.stores?.render(this.router?.current ?? null);
    this.shipyard?.render(this.router?.current ?? null);
    this.signals?.render(this.router?.current ?? null);
    this.policy?.render(this.router?.current ?? null);
    this.fleet?.render(this.router?.current ?? null);
    this.roster?.render(this.router?.current ?? null);
    this.log?.render(this.router?.current ?? null);
    this.strategic?.render(this.router?.current ?? null);
    this.theaters?.onViewTick();
    this.syncBattleSelection();
    // §live-progress: construction bars glide between their panels' redraws.
    tickLiveProgress(document, liveSimTime());
  }

  framePolicy() {
    // Only opaque Deck overlays may rest the galaxy ticker. Workspace, chrome
    // and the planet stage (the orrery keeps running beneath it, ready for
    // Back) leave it live; theaters and session overlays pause it.
    const join = document.getElementById("deck-join");
    const help = document.getElementById("deck-help");
    return { maxFps: 0, renderGalaxy: !this.overlayOpen(join, help) };
  }

  cameraRect(): Rect {
    return this.workspace?.cameraRect() ?? { x: 0, y: 0, w: window.innerWidth, h: window.innerHeight };
  }

  teardown(): void {
    this.abort?.abort();
    this.bottomBand?.teardown();
    this.ctx?.renderer.setDeckSaliency(false);
    this.ctx?.renderer.setTrackingInset(false);
    this.removeDebug?.();
    this.router?.teardown();
    this.workspace?.teardown();
    this.map?.teardown();
    this.toasts?.teardown();
    this.strip?.clear();
    this.empire?.teardown();
    this.guide?.teardown();
    this.welcome?.teardown();
    this.market?.invalidate();
    this.merchant?.invalidate();
    this.stores?.invalidate();
    this.shipyard?.invalidate();
    this.signals?.invalidate();
    this.policy?.invalidate();
    this.fleet?.invalidate();
    this.roster?.invalidate();
    this.log?.invalidate();
    this.strategic?.invalidate();
    this.theaters?.teardown();
    if (this.ctx) this.ctx.renderer.selectedBattleId = null;
    this.router = null;
    this.workspace = null;
    this.map = null;
    this.toasts = null;
    this.strip = null;
    this.empire = null;
    this.guide = null;
    this.welcome = null;
    this.market = null;
    this.merchant = null;
    this.stores = null;
    this.news = null;
    this.shipyard = null;
    this.signals = null;
    this.policy = null;
    this.fleet = null;
    this.roster = null;
    this.log = null;
    this.strategic = null;
    this.theaters = null;
    this.bottomBand = null;
    bindFleetNet(() => null);
    bindResearchNet(() => null);
    bindMarketDerive(() => null, () => []);
    this.removeDebug = null;
    this.abort = null;
    this.root?.replaceChildren();
    this.root = null;
    this.ctx = null;
    this.chromeSignature = "";
    this.zoomSignature = "";
    this.activeCrumbs = [];
  }

  private openRoute(name: DeckRouteName): void {
    this.setNavOverflow(false);
    this.router?.go({ name });
  }

  private setStatus(html: string): void {
    this.strip?.setStatus(html);
  }

  private keyDown(event: KeyboardEvent): void {
    if (!this.ctx || document.getElementById("deck-profile")?.hasAttribute("open")) return;
    if (this.editableTarget(event.target)) return;
    const key = event.key;
    const overlayOpen = this.overlayOpen();
    if (key === "Enter" && this.ctx.state.pendingIntent && !overlayOpen) {
      event.preventDefault();
      if (event.repeat || event.ctrlKey || event.metaKey || event.altKey) return;
      this.ctx.intent.confirmPendingIntent();
      this.strip?.render(true);
      return;
    }
    if (key === "Escape") {
      if (this.escapeOneLayer()) event.preventDefault();
      return;
    }
    if (overlayOpen) return;
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    const routeKeys: Partial<Record<string, DeckRouteName>> = {
      m: "market", v: "fleets", t: "logistics", r: "research", p: "officers",
      u: "operations", y: "syndicate", c: "faction", k: "rankings", l: "log",
    };
    const route = routeKeys[key.toLowerCase()];
    if (route) {
      event.preventDefault();
      this.openRoute(route);
      return;
    }
    if (key.toLowerCase() === "i") {
      event.preventDefault();
      this.stores?.toggle(this.router?.current ?? null);
      return;
    }
    if (key.toLowerCase() === "s") {
      event.preventDefault();
      this.openSelectedSystem();
      return;
    }
    const fleet = this.ctx.state.selectedShipId
      ? this.ctx.state.ghosts.find((entry) => entry.id === this.ctx!.state.selectedShipId && entry.own)
      : undefined;
    if (key.toLowerCase() === "j" && fleet && jumpCapableForKey(fleet)) {
      event.preventDefault();
      this.ctx.intent.armJumpAiming(fleet);
      this.strip?.render(true);
    } else if (key.toLowerCase() === "g" && fleet && guardCapableForKey(fleet)) {
      event.preventDefault();
      this.ctx.intent.armGuardAiming(fleet);
      this.strip?.render(true);
    } else if (key === "[") {
      event.preventDefault();
      this.map?.cycleFleet(-1);
      this.strip?.render(true);
    } else if (key === "]") {
      event.preventDefault();
      this.map?.cycleFleet(1);
      this.strip?.render(true);
    } else if (key === "+" || key === "=") {
      event.preventDefault();
      this.zoomIn();
    } else if (key === "-" || key === "_") {
      event.preventDefault();
      this.zoomOut();
    } else if (key === "?") {
      event.preventDefault();
      this.setHelpOpen(byId("deck-help").hasAttribute("hidden"));
    }
  }

  private openSelectedSystem(): void {
    if (!this.ctx || !this.router) return;
    const mode = this.ctx.renderer.viewMode;
    const id = this.ctx.state.selectedSystemId ?? (mode.type === "system" ? mode.systemId : null);
    const system = id ? this.ctx.state.galaxy?.systems.find((entry) => entry.id === id) : undefined;
    if (!system) {
      this.setStatus("<b>No system selected</b> · click a star, then press S.");
      return;
    }
    this.router.go({ name: "system", params: { id: system.id, systemLabel: system.name } });
  }

  /** The zoom cluster and +/- keys follow the ladder: on a planet, zooming out
   * climbs to the orrery, and zooming in or fitting has nowhere deeper to go. */
  private zoomIn(): void {
    if (this.router?.current?.name === "world") return;
    this.map?.zoomIn();
  }

  private zoomOut(): void {
    if (this.router?.current?.name === "world") {
      this.router.back();
      return;
    }
    this.map?.zoomOut();
  }

  private zoomFit(): void {
    if (this.router?.current?.name === "world") return;
    this.map?.fit();
  }

  private escapeOneLayer(): boolean {
    if (!this.ctx) return false;
    if (this.ctx.state.pendingIntent) {
      this.ctx.intent.clearPendingIntent();
    } else if (this.ctx.intent.intentAiming.jump) {
      this.ctx.intent.clearJumpAiming();
    } else if (this.ctx.intent.intentAiming.guard) {
      this.ctx.intent.clearGuardAiming();
    } else if (closeIndustryMenu(this.ctx.state)) {
      // §route-picker: the route editor's open goods list, before its panel.
      this.policy?.render(this.router?.current ?? null, true);
    } else if (this.constructionMenu) {
      this.setConstructionMenu(false);
    } else if (!byId("deck-nav-overflow").hidden) {
      this.setNavOverflow(false);
    } else if (document.getElementById("deck-layers")?.hasAttribute("hidden") === false) {
      this.setLayersOpen(false);
    } else if (this.stores?.close()) {
      // Stores sits over the other central workbenches.
    } else if (this.empire?.closeBuildWorkbench(this.router?.current ?? null)) {
      // The central construction workbench is the topmost non-modal layer.
    } else if (this.fleet?.closeDetails()) {
      // So is the fleet details workbench, which shares its place.
    } else if (this.ctx.renderer.isSystemScrubbing() || this.ctx.renderer.isFlying()) {
      this.ctx.renderer.cancelSystemScrub();
    } else if (this.theaters?.closeTop()) {
      // Focused replay/landing overlays own Esc before workspace navigation.
    } else if (!byId("deck-help").hidden) {
      this.setHelpOpen(false);
    } else if (this.ctx.renderer.viewMode.type === "battle") {
      this.ctx.renderer.exitBattleView();
      if (this.router?.current) this.router.back();
    } else if (this.popoutWorld()) {
      // §merchant: the trader's stall (or the Shipyard view) closes back to
      // the planet beneath it.
      this.router!.back();
    } else if (this.router?.current?.name === "world") {
      // The planet is the deepest rung: Esc climbs to the orrery and its
      // system workspace before the orrery itself is left.
      this.router.back();
    } else if (this.ctx.renderer.viewMode.type === "system") {
      // First Esc leaves the semantic orrery but preserves its workspace. A
      // second Esc walks the router, matching the visible layer stack.
      this.ctx.renderer.exitSystemView();
      this.ctx.renderer.setSystemDynamic([], [], true);
    } else if (this.router?.current) {
      this.router.back();
    } else if (this.ctx.state.selectedShipId || this.ctx.state.selectedShipIds.size) {
      this.ctx.intent.clearPendingIntent(true);
      this.ctx.intent.clearJumpAiming(true);
      this.ctx.intent.clearGuardAiming(true);
      this.ctx.state.selectedShipId = null;
      this.ctx.state.selectedShipIds.clear();
      this.ctx.state.selectedOrderId = null;
      this.ctx.renderer.stateVersion++;
    } else {
      return false;
    }
    this.strip?.render(true);
    return true;
  }

  private editableTarget(target: EventTarget | null): boolean {
    return target instanceof HTMLInputElement
      || target instanceof HTMLSelectElement
      || target instanceof HTMLTextAreaElement
      || (target instanceof HTMLElement && target.isContentEditable);
  }

  private overlayOpen(
    join = document.getElementById("deck-join"),
    help = document.getElementById("deck-help"),
  ): boolean {
    return (join !== null && !join.hidden)
      || (help !== null && !help.hidden)
      || (this.theaters?.isOpen ?? false);
  }

  private openMapTarget(target: SelectTarget): void {
    if (!this.ctx || !this.router) return;
    const state = this.ctx.state;
    // A target picked inside the System View keeps the orrery beneath its
    // workspace: the route remembers which star it was opened from.
    const mode = this.ctx.renderer.viewMode;
    const fromSystem: Record<string, string> = mode.type === "system" ? { systemId: mode.systemId } : {};
    switch (target.type) {
      case "fleet": {
        const fleet = state.ghosts.find((entry) => entry.id === target.id);
        this.router.go({ name: "fleet", params: { id: target.id, fleetLabel: fleet ? shipKindLabel(fleet.kind) : "Fleet", ...fromSystem } });
        break;
      }
      case "landing":
        this.theaters?.openGround(target.id);
        break;
      case "system": {
        const system = state.galaxy?.systems.find((entry) => entry.id === target.id);
        this.router.go({ name: "system", params: { id: target.id, systemLabel: system?.name ?? "System" } });
        break;
      }
      case "hub":
        this.router.go({ name: "market", query: { inspect: "map" } });
        break;
      case "exploration":
        state.selectedExplorationSiteId = target.id;
        this.ctx.renderer.stateVersion++;
        this.router.go({ name: "operations", query: { site: target.id } });
        break;
      case "operation":
        this.router.go({ name: "operations", query: { contract: target.id } });
        break;
      case "signal":
        this.router.go({ name: "signals", query: { signal: target.id } });
        break;
      case "ongoingBattle": {
        // One marker family: the id is a running engagement OR a concluded record.
        const running = state.battles.some((battle) => battle.id === target.id);
        this.router.go({ name: "battle", params: { id: target.id, label: running ? "Ongoing battle" : "Battle replay", ...fromSystem } });
        break;
      }
      case "aftermath":
        this.router.go({ name: "battle", params: { id: String(target.id), report: "battle", label: "Battle report" } });
        break;
      case "capture":
        this.router.go({ name: "battle", params: { id: String(target.id), report: "capture", label: "Capture report" } });
        break;
      case "systemBody": {
        const systemId = this.ctx.renderer.viewMode.type === "system" ? this.ctx.renderer.viewMode.systemId : state.selectedSystemId ?? "";
        this.openWorldPanel(systemId, Number(target.detail.id));
        break;
      }
      case "emplacement":
        this.router.go({ name: "fleet", params: { id: target.id, object: "emplacement", fleetLabel: "Installation" } });
        break;
      case "jumpDeparture":
        this.ctx.renderer.selectedJumpDepartureKey = target.key;
        this.router.go({ name: "fleet", params: { id: target.key, object: "jump-departure", fleetLabel: "Jump departure" } });
        break;
      case "anchor": {
        // Your own Command Center opens home; a rival's is its readout only.
        const home = target.own ? foundingHomeSystemId() : null;
        if (home) this.flyIntoSystem(home);
        break;
      }
      case "clearSystemBody":
        if (this.router.current?.name === "world") this.router.back();
        break;
    }
  }

  private openWorldPanel(systemId: string, bodyId: number, query?: Record<string, string>): void {
    if (!this.ctx || !this.router || !systemId || !Number.isFinite(bodyId)) return;
    const system = this.ctx.state.galaxy?.systems.find((entry) => entry.id === systemId);
    const body = this.ctx.state.systems.find((entry) => entry.id === systemId)?.bodies.find((entry) => entry.id === bodyId);
    if (!system || !body) return;
    // A world sits beneath its system on the ladder: unless the current
    // workspace already belongs to this system, Back must land on the system.
    const current = this.router.current;
    const sameSystem = (current?.name === "system" || current?.name === "build" || current?.name === "world")
      && (current.params?.systemId ?? current.params?.id) === systemId;
    if (!sameSystem) this.router.go({ name: "system", params: { id: system.id, systemLabel: system.name } });
    this.router.go({ name: "world", params: { systemId: system.id, systemLabel: system.name, bodyId: String(body.id), worldLabel: body.name },
      ...(query ? { query } : {}) });
  }

  /** §construction: every developed world's one work slot, from the top bar:
   * how many build now and how many sit idle, and a way to each. */
  private renderConstructionChip(): void {
    if (!this.ctx) return;
    const state = this.ctx.state;
    const chip = byId("deck-construction-chip");
    const menu = byId("deck-construction-menu");
    const slots = worldSlots(state.systems, state.playerId, state.galaxy?.build_options ?? [], liveSimTime());
    chip.hidden = !slots.length;
    if (!slots.length) this.constructionMenu = false;
    const building = slots.filter((slot) => slot.job).length;
    const idle = slots.length - building;
    setHtml(chip, `<button type="button" class="deck-construction-chip__button" data-deck-act="construction-chip" aria-expanded="${this.constructionMenu}" aria-haspopup="menu" aria-controls="deck-construction-menu" title="Construction across your worlds">${icon("queue", "sm")}<span><b>${building}</b> building · <b class="${idle ? "is-idle" : ""}">${idle}</b> idle</span></button>`);
    menu.hidden = !this.constructionMenu;
    if (!this.constructionMenu) return;
    const rows = slots.map((slot) => {
      const system = state.galaxy?.systems.find((entry) => entry.id === slot.systemId);
      const job = slot.job;
      const line = slotLine(job, slot.waiting);
      return `<div class="deck-construction-menu__row${job ? "" : " is-idle"}"><span><b>${escapeHtml(slot.body.name)}</b><small>${escapeHtml(system?.name ?? "")} · ${escapeHtml(line)}</small></span><button type="button" role="menuitem" data-deck-act="construction-open" data-system="${escapeHtml(slot.systemId)}" data-body="${slot.body.id}" data-build="${job ? "" : "1"}">${job ? "Open" : "Build"}</button></div>`;
    }).join("");
    setHtml(menu, `<p>One structure builds at a time on each world.</p>${rows}`);
    const anchor = chip.getBoundingClientRect();
    menu.style.left = `${Math.max(8, Math.min(anchor.left, window.innerWidth - menu.offsetWidth - 8))}px`;
  }

  /** §signals: "Signals · 3" while any are open or a report waits. */
  private renderSignalsChip(): void {
    if (!this.ctx) return;
    const view = this.ctx.state.signals;
    const chip = byId("deck-signals-chip");
    const open = openSignals(view);
    const reports = view?.signals.filter((s) => s.report).length ?? 0;
    chip.hidden = !view || (!open && !reports);
    if (chip.hidden) return;
    const label = reports ? `Signals · ${open} · report in` : `Signals · ${open}`;
    setHtml(chip, `<button type="button" class="deck-construction-chip__button${reports ? " has-report" : ""}" data-deck-act="signals-chip" title="Signals near home">${icon("sensor", "sm")}<span><b>${label}</b></span></button>`);
  }

  /** §merchant: shown while the free trader is at your home. */
  private renderMerchantChip(): void {
    if (!this.ctx) return;
    const merchant = this.ctx.state.merchant;
    const chip = byId("deck-merchant-chip");
    // The trader is news only once its pop-up has told it docked.
    chip.hidden = !merchant || merchant.phase !== "docked" || !this.ctx.state.storyTold.has("free_trader");
    if (!merchant || chip.hidden) return;
    // §den-contract: docked for the whole charter, so no countdown.
    const trader = traderOf(this.ctx.state.founding);
    setHtml(chip, `<button type="button" class="deck-construction-chip__button" data-deck-act="merchant-chip" title="${escapeHtml(trader.name)} of the ${escapeHtml(trader.ship)}, at your home">${icon("market", "sm")}<span><b>Free trader</b></span></button>`);
  }

  private setConstructionMenu(open: boolean): void {
    if (this.constructionMenu === open) return;
    this.constructionMenu = open;
    this.renderConstructionChip();
  }

  /// §planet-build: a structure builder opened from anywhere (the tutorial, a
  /// handoff, a link, the system's Build tab) lands on its world with the
  /// builder over the planet, exactly as if the player had gone there and
  /// pressed Build. A `build` route asking for it becomes its system, so Back
  /// climbs world → system like any other world.
  private openWorldBuild(systemId: string, bodyId: number, query: Record<string, string>, mode: "structures" | "ships" = "structures"): void {
    if (!this.ctx || !this.router) return;
    const system = this.ctx.state.galaxy?.systems.find((entry) => entry.id === systemId);
    const body = this.ctx.state.systems.find((entry) => entry.id === systemId)?.bodies.find((entry) => entry.id === bodyId);
    if (!system || !body) return;
    const systemRoute: DeckRoute = { name: "system", params: { id: system.id, systemLabel: system.name } };
    const current = this.router.current;
    const sameSystem = (current?.name === "system" || current?.name === "world")
      && (current.params?.systemId ?? current.params?.id) === systemId;
    const world: DeckRoute = { name: "world", params: { systemId: system.id, systemLabel: system.name, bodyId: String(body.id), worldLabel: body.name },
      query: { ...query, build: mode } };
    // Already on this world: open its builder in place rather than stacking a twin.
    if (current?.name === "world" && sameSystem && current.params?.bodyId === String(body.id)) {
      this.router.replace(world);
      return;
    }
    if (current?.name === "build") this.router.replace(systemRoute);
    else if (!sameSystem) this.router.go(systemRoute);
    this.router.go(world);
  }

  private routeChanged(route: DeckRoute | null, stack: readonly DeckRoute[]): void {
    this.syncBattleSelection(route);
    if (this.ctx && route?.name !== "operations" && this.ctx.state.selectedExplorationSiteId) {
      this.ctx.state.selectedExplorationSiteId = null;
      this.ctx.renderer.stateVersion++;
    }
    const viewMode = this.ctx?.renderer.viewMode;
    // A fleet, battle or shipyard opened from inside the orrery keeps it beneath.
    const routeSystem = route?.name === "shipyard" ? route.params?.id : route?.params?.systemId;
    // §merchant (Peter, Nov 4 2026): so does the trader's stall opened from a
    // world, over that world's planet, at the same zoom; Nov 9, the Shipyard
    // view too.
    const overWorld = !!this.popoutWorld(route);
    const fromThisSystem = ((route?.name === "fleet" || route?.name === "battle" || route?.name === "shipyard")
      && viewMode?.type === "system" && routeSystem === viewMode.systemId) || overWorld;
    const semanticRoute = route?.name === "system" || route?.name === "world" || route?.name === "build" || fromThisSystem;
    if (!semanticRoute && this.ctx?.renderer.viewMode.type === "system" && !this.ctx.renderer.isSystemScrubbing()) {
      this.ctx.renderer.exitSystemView();
      this.ctx.renderer.setSystemDynamic([], [], true);
    }
    if (!route || !this.workspace || !this.router) {
      this.activeCrumbs = [];
      // No route: the planet stage and any construction child go with the workspace.
      this.empire?.render(null);
      this.workspace?.close();
      this.renderActiveNav(null);
      return;
    }
    if (semanticRoute && this.ctx && !fromThisSystem) {
      const systemId = route.params?.systemId ?? route.params?.id;
      const dynamic = this.ctx.state.systems.find((entry) => entry.id === systemId);
      if (systemId) {
        this.ctx.state.selectedSystemId = systemId;
        this.ctx.renderer.setSystemDynamic(
          dynamic?.bodies ?? [],
          (dynamic?.builds ?? []).map((build) => ({ key: build.key, body_id: build.body_id })),
          dynamic?.habitat_fed ?? true,
        );
      }
      if (route.name === "world") {
        // The planet rung implies the system rung beneath it: enter the
        // orrery now so Back lands there rather than on the galaxy.
        const system = this.ctx.state.galaxy?.systems.find((entry) => entry.id === systemId);
        const mode = this.ctx.renderer.viewMode;
        if (system && mode.type === "galaxy" && !this.ctx.renderer.isSystemScrubbing()) {
          this.ctx.renderer.enterSystemView(system, dynamic?.bodies ?? []);
        } else if (mode.type === "system" && mode.systemId !== systemId) {
          this.ctx.renderer.exitSystemView();
        }
        if (route.params?.bodyId) {
          // The world is the System View's one selection: no fleet keeps a ring beside it.
          this.ctx.intent.deselectFleets();
          this.ctx.renderer.pulseSystemBody(route.params.bodyId);
        }
      } else if (route.name === "system") {
        // Back from a world (or a zoomed look): the orrery eases out to its fit.
        this.ctx.renderer.systemResetZoom();
      }
    }
    this.activeCrumbs = this.router.breadcrumbs(route);
    this.workspace.show(route, this.activeCrumbs, stack.length > 1);
    const focus = overWorld ? null : this.routeFocus(route);
    if (focus) requestAnimationFrame(() => this.ctx?.renderer.ensureWorldVisible(focus));
    this.guide?.render(true);
    const empireHandled = this.empire?.render(route, true) ?? false;
    const marketHandled = (this.market?.render(route, true) ?? false) || (this.merchant?.render(route, true) ?? false)
      || (this.shipyard?.render(route, true) ?? false)
      || (this.signals?.render(route, true) ?? false);
    this.stores?.render(route, true);
    this.stores?.renderBar(route);
    const policyHandled = this.policy?.render(route, true) ?? false;
    const fleetHandled = this.fleet?.render(route, true) ?? false;
    const rosterHandled = this.roster?.render(route, true) ?? false;
    const logHandled = this.log?.render(route, true) ?? false;
    const strategicHandled = this.strategic?.render(route, true) ?? false;
    if (!empireHandled && !marketHandled && !policyHandled && !fleetHandled && !rosterHandled && !logHandled && !strategicHandled) this.renderPlaceholder(route);
    this.renderActiveNav(route.name);
  }

  private syncBattleSelection(route = this.router?.current ?? null): void {
    if (!this.ctx) return;
    // A viewer can open directly from the log without replacing its route.
    // On close, selection returns to the underlying battle panel, if any.
    const panelId = route?.name !== "battle" || route.params?.report === "capture"
      ? null
      : route.params?.report === "battle"
        ? this.ctx.state.battleReports.find((report) => String(report.id) === route.params?.id)?.battle_id ?? null
        : route.params?.id ?? null;
    this.ctx.renderer.selectedBattleId = this.theaters?.activeBattleId ?? panelId;
  }

  private routeFocus(route: DeckRoute): { x: number; y: number } | null {
    if (!this.ctx) return null;
    if (route.name === "market") return this.ctx.state.galaxy?.hub ?? null;
    if (route.name === "merchant") {
      const home = this.ctx.state.merchant?.system;
      return this.ctx.state.galaxy?.systems.find((system) => system.id === home)?.pos ?? null;
    }
    if (route.name === "fleet" && route.params?.id) {
      return this.ctx.state.ghosts.find((fleet) => fleet.id === route.params!.id)?.pos ?? null;
    }
    if (route.name === "system" || route.name === "world" || route.name === "build" || route.name === "shipyard") {
      const id = route.params?.systemId ?? route.params?.id;
      return this.ctx.state.galaxy?.systems.find((system) => system.id === id)?.pos ?? null;
    }
    if (route.name === "battle" && route.params?.id) {
      if (route.params.report === "battle") return this.ctx.state.battleReports.find((report) => String(report.id) === route.params!.id)?.pos ?? null;
      if (route.params.report === "capture") return this.ctx.state.captureReports.find((report) => String(report.id) === route.params!.id)?.pos ?? null;
      return this.ctx.state.battles.find((battle) => battle.id === route.params!.id)?.pos
        ?? this.ctx.state.battleRecords.find((record) => record.id === route.params!.id)?.pos
        ?? null;
    }
    return null;
  }

  private renderPlaceholder(route: DeckRoute): void {
    const body = byId("deck-workspace-body");
    body.replaceChildren();
    const placeholder = document.createElement("div");
    placeholder.className = "deck-placeholder";
    const eyebrow = document.createElement("span");
    eyebrow.textContent = "Workspace unavailable";
    const title = document.createElement("b");
    title.textContent = DECK_ROUTES[route.name].title;
    const copy = document.createElement("p");
    copy.textContent = "This report is not available in the current served picture.";
    placeholder.append(eyebrow, title, copy);
    body.append(placeholder);
  }

  private renderActiveNav(name: DeckRouteName | null): void {
    for (const button of document.querySelectorAll<HTMLButtonElement>("#deck-nav [data-route], #deck-nav-overflow [data-route]")) {
      if (button.dataset.route === name) button.setAttribute("aria-current", "page");
      else button.removeAttribute("aria-current");
    }
  }

  private setHelpOpen(open: boolean): void {
    byId("deck-help").hidden = !open;
  }

  /** §merchant (Peter, Nov 4 2026): the world the trader's stall was opened
   * from; the stall floats over its planet. Nov 9 2026 ("should not zoom out
   * of the planetary view"): the world's own Shipyard view does too. */
  private popoutWorld(route = this.router?.current ?? null): DeckRoute | null {
    const beneath = this.router?.beneath ?? null;
    if (beneath?.name !== "world") return null;
    if (route?.name === "merchant") return beneath;
    return route?.name === "shipyard" && route.params?.id === beneath.params?.systemId ? beneath : null;
  }

  /** ✕ closes the workspace, but a planet climbs one rung: to its system's
   * orrery (Peter, Oct 21 2026), never out to the galaxy. */
  private closeWorkspace(): void {
    // §merchant: the trader's stall (or the Shipyard view) over a planet
    // closes back to it.
    if (this.popoutWorld()) {
      this.router?.back();
      return;
    }
    const route = this.router?.current;
    const systemId = route?.name === "world" ? route.params?.systemId : undefined;
    const system = systemId ? this.ctx?.state.galaxy?.systems.find((entry) => entry.id === systemId) : undefined;
    if (system) this.router?.climbTo({ name: "system", params: { id: system.id, systemLabel: system.name } });
    else this.router?.close();
  }

  /** §home-step: open a system by flying into it, its panel opening as for a
   * click; straight in when the map can't fly (another System View is open). */
  private flyIntoSystem(id: string): void {
    const system = this.ctx?.state.galaxy?.systems.find((entry) => entry.id === id);
    if (!this.ctx || !system) return;
    this.ctx.state.selectedSystemId = id;
    if (this.map?.flyInto(id)) this.router?.go({ name: "system", params: { id, systemLabel: system.name } });
    else this.map?.enterSystem(system);
  }

  private setLayersOpen(open: boolean): void {
    byId("deck-layers").hidden = !open;
    document.querySelector('#deck-zoom [data-deck-act="layers"]')?.setAttribute("aria-expanded", String(open));
  }

  private setNavOverflow(open: boolean): void {
    byId("deck-nav-overflow").hidden = !open;
    byId("deck-nav-more").setAttribute("aria-expanded", String(open));
  }

  private renderZoom(): void {
    if (!this.ctx) return;
    const mode = this.ctx.renderer.viewMode.type;
    const zoom = this.ctx.renderer.zoomFactor();
    const planet = this.router?.current?.name === "world";
    const text = planet ? "PLANET" : mode === "system" ? "SYSTEM" : mode === "battle" ? "BATTLE" : zoom < 10 ? `${zoom.toFixed(1)}×` : `${Math.round(zoom)}×`;
    if (text === this.zoomSignature) return;
    this.zoomSignature = text;
    const level = byId("deck-zoom-level");
    level.textContent = text;
    level.setAttribute("aria-label", !planet && mode === "galaxy" ? `${zoom.toFixed(2)}× galaxy magnification relative to fit` : `${text.toLowerCase()} semantic view`);
  }

  private toastFor(event: CoreEvent): void {
    if (!this.toasts) return;
    if (event.kind === "OrderConfirmed") {
      this.toasts.push({
        title: "Order received",
        message: humanize(event.orderKind),
        tone: "good",
        destination: { name: "fleet", params: { id: event.shipId } },
      });
    } else if (event.kind === "FleetDocked") {
      const fleet = this.ctx?.state.ghosts.find((entry) => entry.id === event.fleetId);
      const system = event.berth === "hub" ? null : this.ctx?.state.galaxy?.systems.find((entry) => entry.id === event.berth);
      this.toasts.push({
        title: "Fleet docked",
        message: `${fleet ? shipKindLabel(fleet.kind) : "Fleet"} · ${event.berth === "hub" ? "Market Hub" : system?.name ?? "system berth"}`,
        tone: "good",
        destination: event.berth === "hub"
          ? { name: "market", query: { tab: "warehouse" } }
          : { name: "system", params: { id: event.berth, systemLabel: system?.name ?? "System" }, query: { tab: "fleets" } },
      });
    } else if (event.kind === "FleetArrived") {
      const fleet = this.ctx?.state.ghosts.find((entry) => entry.id === event.fleetId);
      this.toasts.push({
        title: "Fleet arrived",
        message: fleet ? shipKindLabel(fleet.kind) : "Destination reached",
        tone: "good",
        destination: { name: "fleet", params: { id: event.fleetId } },
      });
    } else if (event.kind === "BuildCompleted") {
      const system = this.ctx?.state.galaxy?.systems.find((entry) => entry.id === event.systemId);
      const report = this.ctx?.state.systems.find((entry) => entry.id === event.systemId);
      const body = event.bodyId === undefined ? undefined : report?.bodies.find((entry) => entry.id === event.bodyId);
      // §construction: a finished building opens on its world, selected; one
      // that still needs a crew says so and points at its workforce.
      const structure = !!(system && body && isStructureJob(event.buildKey) && !isProjectKey(event.buildKey));
      const staff = structure && !!report && needsWorkforce(report, body!.id, event.buildKey);
      this.toasts.push({
        title: "Construction complete",
        message: `${buildLabel(event.buildKey)} · ${body?.name ?? system?.name ?? "colony"}${staff ? " · assign workforce to start it" : ""}`,
        tone: "good",
        destination: structure
          ? { name: "world", params: { systemId: system!.id, systemLabel: system!.name, bodyId: String(body!.id), worldLabel: body!.name },
            query: { structure: event.buildKey, ...(staff ? { staff: "1" } : {}) } }
          : { name: "system", params: { id: event.systemId, systemLabel: system?.name ?? "System" }, query: { tab: "build" } },
      });
    } else if (event.kind === "StructureStaffed") {
      const system = this.ctx?.state.galaxy?.systems.find((entry) => entry.id === event.systemId);
      this.toasts.push({
        title: "Production staffed",
        message: `${event.title} · ${system?.name ?? "colony"}`,
        tone: "good",
        destination: { name: "system", params: { id: event.systemId, systemLabel: system?.name ?? "System" }, query: { tab: "production" } },
      });
    } else if (event.kind === "ResearchCompleted") {
      this.toasts.push({
        title: "Research complete",
        message: event.programmeName,
        tone: "good",
        destination: { name: "research" },
      });
    } else if (event.kind === "CommandRejected") {
      this.toasts.push({ title: "Command refused", message: event.message, tone: "bad", destination: { name: "log" } });
    } else if (event.kind === "LogToast") {
      // §log: a raid's warning lands in the Inbox, its turns in History.
      const toast = logToast(event.entry, systemName, liveSimTime());
      if (toast) this.toasts.push({ ...toast, durationMs: 20_000, destination: event.entry.phase === "inbound"
        ? { name: "log", query: { tab: "inbox" } } : { name: "log", query: { tab: "history", filter: "raids" } } });
    } else if (event.kind === "ReportArrived") {
      // §founding R7: a win and a loss never share a title or a tone.
      const notice = reportNotice(event.report, this.ctx?.state.galaxy?.pirate_id, this.ctx?.state.founding?.privateer ?? this.ctx?.state.lastFoundingPrivateer,
        this.ctx?.state.founding?.pirates?.nephew);
      this.toasts.push({
        title: notice.title,
        message: `${notice.text} · ${informationDelay(event.report.age)}`,
        tone: notice.tone,
        destination: { name: "log" },
      });
    } else if (event.kind === "BattleConcluded") {
      const record = this.ctx?.state.battleRecords.find((candidate) => candidate.id === event.recordId);
      const notice = battleNotice(event.outcome, record);
      this.toasts.push({
        title: notice.title,
        message: notice.text,
        tone: notice.tone,
        destination: { name: "battle", params: { id: event.recordId } },
      });
    } else if (event.kind === "SignalNews") {
      this.toasts.push({
        title: event.title,
        message: event.message,
        tone: event.tone,
        destination: { name: "signals" },
        durationMs: event.tone === "good" ? 16_000 : 10_000,
      });
    } else if (event.kind === "CharterMilestone") {
      this.toasts.push({
        title: event.title,
        message: event.message,
        tone: event.tone ?? "good",
        destination: { name: "operations" },
        durationMs: event.tone === "bad" ? 24_000 : 16_000,
      });
    } else if (event.kind === "EstimateReady") {
      const win = event.estimate.win_pct == null ? "Projection ready" : `${Math.round(event.estimate.win_pct)}% projected win chance`;
      this.toasts.push({
        title: "Engagement estimate",
        message: win,
        destination: { name: "fleet", params: { id: event.estimate.target } },
        durationMs: 15_000,
      });
    } else if (event.kind === "TradeSettled") {
      this.toasts.push(deckTradeNotice(event.trade));
    } else if (event.kind === "ServerError") {
      this.toasts.push({ title: "Command refused", message: event.message, tone: "bad", destination: { name: "log" } });
    }
  }

  private syncSessionVisibility(): void {
    if (!this.ctx) return;
    const ready = this.ctx.state.playerId !== null;
    byId("deck-join").hidden = ready;
    byId("deck-topbar").hidden = !ready;
    if (!ready) {
      this.workspace?.close();
      const connecting = this.ctx.state.link === "connecting" || this.ctx.state.link === "reconnecting";
      byId<HTMLButtonElement>("deck-join-button").disabled = byId("deck-join-form").dataset.busy === "true" || (connecting && !!this.ctx.state.name);
    }
  }

  private renderChrome(force = false): void {
    if (!this.ctx) return;
    const state = this.ctx.state;
    const counts = this.navBadgeCounts();
    const signature = sheetFingerprint([
      state.name, state.playerId, state.link, state.tick, state.simTime, state.pacingScale,
      state.wallet, state.credits, counts,
    ]);
    this.renderFallen();
    if (!force && signature === this.chromeSignature) return;
    this.chromeSignature = signature;
    this.renderConstructionChip();
    this.renderMerchantChip();
    this.renderSignalsChip();
    byId("deck-corp").textContent = state.name || "—";
    byId("deck-corp-id").textContent = state.playerId !== null ? formatId(state.playerId) : "—";
    byId("deck-credits").textContent = state.credits !== null ? Math.round(spendableMarketCredits()).toLocaleString() : "—";
    byId("deck-equity").textContent = state.wallet ? Math.round(state.wallet.valuation).toLocaleString() : "—";
    byId("deck-tick").textContent = state.link === "online" ? gameClock(state.simTime) : "—";
    const link = byId("deck-link");
    link.textContent = state.link === "online" ? "● online" : state.link === "reconnecting" ? "reconnecting…" : state.link === "connecting" ? "connecting…" : "offline";
    link.classList.toggle("is-online", state.link === "online");
    for (const [route, count] of Object.entries(counts)) {
      const badge = byId(`deck-badge-${route}`);
      badge.textContent = count > 99 ? "99+" : String(count);
      badge.hidden = count <= 0;
    }
    syncNavMoreBadge();
  }


  /** §gun-platform (Peter, Oct 30 2026: "game over and they can restart"):
   * the uncle sacked the colony; a restart is all that is left. */
  private renderFallen(): void {
    const st = this.ctx!.state;
    const fallen = !!st.founding?.vendetta?.fallen;
    byId("deck-fallen").hidden = !fallen;
    if (!fallen) return;
    byId("deck-fallen-title").textContent = `${colonyName(st)} has fallen`;
    byId("deck-fallen-line").textContent = `${pirateNames(st.founding).uncle} sacked the colony.`;
  }

  private navBadgeCounts(): Record<string, number> {
    const state = this.ctx!.state;
    const ownBattleIds = new Set(state.battles.flatMap((battle) => battle.participants));
    const logDecisions = (state.founding && state.founding.stage !== "complete" ? 1 : 0)
      + (state.diplomacy?.incoming.length ?? 0)
      + state.syndicateInvites.length
      + state.operations.filter((operation) => operation.state === "offered" && !operation.joined).slice(0, 3).length
      + state.battles.filter((battle) => battle.own).length;
    return {
      fleets: state.ghosts.filter((fleet) => fleet.own && (fleet.stalled || fleet.rescue_inbound || ownBattleIds.has(fleet.id))).length,
      market: (state.wallet?.orders.length ?? 0) + (state.freight?.shipments.length ?? 0),
      research: state.research?.stalled ? 1 : 0,
      officers: state.captains.filter((captain) => !captain.assigned_fleet || (captain.report?.unspent ?? 0) > 0).length,
      operations: state.operations.filter((operation) => operation.state === "offered" || (operation.state === "active" && !operation.joined)).length,
      syndicate: state.syndicateInvites.length,
      comms: commsBadge(state.social, state.playerId),
      faction: (state.charter && state.charter.status !== "good_standing" ? 1 : 0) + (state.diplomacy?.incoming.length ?? 0),
      log: this.log?.badgeCount(this.router?.current ?? null) ?? logDecisions,
    };
  }

  private focusInboxSystem(id: string): void {
    if (!this.ctx) return;
    const system = this.ctx.state.galaxy?.systems.find((entry) => entry.id === id);
    if (!system) return;
    this.ctx.state.selectedSystemId = id;
    this.ctx.renderer.centerOnWorld(system.pos);
    this.ctx.renderer.pingWorld(system.pos);
    this.ctx.renderer.stateVersion++;
    this.router?.go({ name: "system", params: { id, systemLabel: system.name } });
  }

  private focusInboxFleet(id: string): void {
    if (!this.ctx) return;
    const fleet = this.ctx.state.ghosts.find((entry) => entry.id === id);
    if (!fleet) return;
    this.map?.focusFleet(id);
    this.ctx.renderer.pingWorld(fleet.pos);
  }
}

export const createShell = (): Shell => new DeckShell();

function humanize(value: string): string {
  return value.replace(/_/g, " ").replace(/([a-z])([A-Z])/g, "$1 $2").replace(/^./, (letter) => letter.toUpperCase());
}

function escapeHtml(value: string): string {
  return value.replace(/[&<>"]/g, (character) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;",
  })[character]!);
}
