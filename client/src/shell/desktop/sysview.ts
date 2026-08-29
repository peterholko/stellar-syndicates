import { affinityLine } from "../../core/derive/captains";
import { constructionStock, dockedFreighterStock, sendCrew } from "../../core/derive/fleet";
import { doneAtLocal, fmt, fmtBuildDur } from "../../core/derive/format";
import { pushSystemDynamic, viewedSystemId } from "../../core/derive/geo";
import { bodyPoolUsage, type BuildOpt, buildOption, dispatchBuildKey, fitLegal, FITTING_POINTS, hullResearched, MODULE_SLOTS, moduleLedgerAt, type Pool, POOL_LABEL, POOL_OF, poolUsage, type PoolUse, SHIP_YARD, type ShipOpt, shipOption, shipyardBoost, slipsFor, type StructOpt, structOption, YARD_TITLE } from "../../core/derive/market";
import { SHIP_STATS, siegeProgress } from "../../core/derive/orders";
import { badgeChip, icon, type IconKey, type IconSize, label } from "../../icons";
import { type AssignmentView, type BodyView, type BuildState, type Commodity, type Deposit, type MigrationPolicy, type ModuleKind, type ShipKind, type SystemInfo, type SystemStateView } from "../../protocol";
import { renderer } from "../../render";
import { liveSimTime, state } from "../../state";
import { type SystemBodyDetail } from "../../systemview";
import { net } from "./index";
import { $, badge, bar, COLONY_ROLE_ICON, commodityIcon, ENVIRONMENT_ICON, esc, FEATURE_ICON, fmtPopulation, GEOLOGY_ICON, readout, renderDeferred, setHtml, stat, statStrip, svgIcon } from "./mapchrome";
import { berthLine, closeRail, groundLine, openRail, updateStandingPanel } from "./rail";
import { ROMAN } from "./research";
import { fmtCountdown, uxTabBar, type UxTabOption } from "./ship";


// --- System View (semantic-zoom LOD) — ENTER/EXIT + planet details -----------
// A PRESENTATION-ONLY level-of-detail: the schematic star-system view. It shows
// public geography + the SAME light-gated ownership as the galaxy map, and adds
// NO gameplay (no per-planet claim/build/defend, no intra-system ships/combat).
// All state lives in the renderer (viewMode); this layer only wires the UX.
export const hex6 = (n: number) => "#" + (n >>> 0).toString(16).padStart(6, "0").slice(-6);


export function showBreadcrumb(name: string): void {
  $("bc-system").textContent = name;
  $("breadcrumb").classList.add("is-open");
}

export function showSystemUi(sys: SystemInfo): void {
  document.body.classList.add("is-system-view");
  state.selectedSystemId = sys.id; // keep the galaxy selection in sync (rail shows it)
  showBreadcrumb(sys.name);
  closePlanetPanel();
  closeRail(); // the management column takes the right dock inside the view
  // §management-home: feed the scene's structure markers + open the management
  // column (owned systems only — both no-op into scenery for rival/unclaimed).
  pushSystemDynamic(sys.id);
  updateSysviewManage();
  const mine = state.systems.find((s) => s.id === sys.id)?.owner === state.playerId;
  readout().innerHTML = mine
    ? `<b>${esc(sys.name)}</b> · <span class="dim">Select a world to manage it.</span>`
    : `<b>${esc(sys.name)}</b> · <span class="dim">Select a world to inspect it.</span>`;
}

export function hideSystemUi(): void {
  document.body.classList.remove("is-system-view");
  $("breadcrumb").classList.remove("is-open");
  closePlanetPanel();
  closeSysviewManage();
  renderer.setSystemDynamic([], [], true);
}

export function enterSystem(sys: SystemInfo): void {
  renderer.enterSystemView(sys, state.systems.find((s) => s.id === sys.id)?.bodies ?? []);
  showSystemUi(sys);
}

export function exitSystem(): void {
  if (renderer.viewMode.type !== "system") return;
  renderer.exitSystemView();
  hideSystemUi();
}


// --- §management-home: the System View is where an OWNED system is RUN --------
// (the city-screen pattern). The management column + the structure markers are a
// RELOCATION of the rail's system-level management, not new gameplay scale:
// every command here is the same system-level BuildShip/DevelopSystem/… the rail
// sent, buildings consume SYSTEM dev slots, and the markers are decorative
// anchors. Rival/unclaimed system views stay pure scenery (tiers are owner-only
// in the View — a rival's dyn carries 0s — and we ALSO gate on `mine` here).
export type SystemManageTab = "overview" | "worlds" | "production" | "construction";

export let systemManageTab: SystemManageTab = "worlds";

export let sysviewManageBuilt = false;

/// Per-View refresh while inside the System View: feed the scene's markers (a
/// cached no-op unless a build completed) and re-render the management column.
export function updateSysviewDynamic(): void {
  const sid = viewedSystemId();
  if (!sid) return;
  pushSystemDynamic(sid);
  updateSysviewManage();
  // §body-management: the open body panel is live — worker counts, queue bars,
  // and afford states track the Views (single-click-guarded like every panel).
  refreshOpenBodyPanel();
  // §build-panel: its rows/costs/queued-note track the same Views.
  refreshBuildPanel();
}

export function buildSysviewManage(): void {
  if (sysviewManageBuilt) return;
  sysviewManageBuilt = true;
  $("svm-close").addEventListener("click", exitSystem);
  // ONE delegated listener on the static panel shell (only #svm-body's innerHTML
  // is ever rewritten), so build clicks can never lose their handler.
  // §body-management: the summary is PURE DATA — its only clickables are
  // NAVIGATION chips (data-body) that open a body's panel; every command verb
  // (build / worker assignment / ship / auto-supply) lives on the body panels now.
  $("sysview-manage").addEventListener("click", (e) => {
    const tabButton = (e.target as HTMLElement).closest("[data-svm-tab]") as HTMLElement | null;
    const requestedTab = tabButton?.dataset.svmTab as SystemManageTab | undefined;
    if (requestedTab && (["overview", "worlds", "production", "construction"] as SystemManageTab[]).includes(requestedTab)) {
      systemManageTab = requestedTab;
      lastSysviewManageSig = "";
      updateSysviewManage();
      return;
    }
    const el = (e.target as HTMLElement).closest("[data-body]") as HTMLElement | null;
    if (el?.dataset.body) openBodyPanelById(el.dataset.body);
  });
}

/// §body-management: chip → the body's panel, with a sprite pulse so the eye
/// lands on the right dot ("HERE is the thing you tapped").
export function openBodyPanelById(bodyId: string): void {
  const d = renderer.systemBodyDetail(bodyId);
  if (!d) return;
  renderer.pulseSystemBody(bodyId);
  openPlanetPanel(d);
}

export function closeSysviewManage(): void {
  $("sysview-manage").classList.remove("is-open");
}

export let lastSysviewManageSig = "";

export function updateSysviewManage(): void {
  if (renderDeferred("sysview-manage", updateSysviewManage)) return; // §single-click
  const sid = viewedSystemId();
  const sys = sid && state.galaxy ? state.galaxy.systems.find((s) => s.id === sid) : undefined;
  const dyn = sid ? state.systems.find((s) => s.id === sid) : undefined;
  const mine = !!dyn && dyn.owner !== null && dyn.owner === state.playerId;
  const panel = $("sysview-manage");
  if (!sid || !sys || !mine) {
    // Rival/unclaimed system view: pure scenery — no management column at all.
    panel.classList.remove("is-open");
    return;
  }
  buildSysviewManage();
  panel.classList.add("is-open");
  $("svm-title").textContent = `${sys.name} System`;
  // SLOTS — the system's defining constraint, promoted to the header.
  const sUsed = dyn.slots_used ?? 0;
  const sTotal = dyn.slots_total ?? 0;
  const slotsEl = $("svm-slots");
  setHtml(slotsEl, `${icon("slots", "sm")} SLOTS ${sUsed}/${sTotal}`);
  slotsEl.classList.toggle("is-warn", sTotal > 0 && sUsed >= sTotal);

  // §perf: the svm-body sections (stockpile/build/workforce/blockade) are built
  // and re-parsed on every View at 10 Hz. Skip when the owner-only dynamic slice
  // is unchanged; a 1 s simTime heartbeat keeps build/siege ETAs ticking at their
  // whole-second cadence. (Header title/slots above stay live every call.)
  const svmSig = JSON.stringify([
    sid,
    systemManageTab,
    dyn,
    [...dockedFreighterStock(sid).entries()],
    Math.floor(state.simTime),
  ]);
  if (svmSig === lastSysviewManageSig && $("svm-body").innerHTML) return;
  lastSysviewManageSig = svmSig;

  // Stockpile + storage cap (the "ship it or it idles" pressure).
  const cap = dyn.storage_cap ?? 0;
  const used = dyn.storage_used ?? 0;
  const storageFull = cap > 0 && used >= cap;
  const storageBar = cap > 0
    ? `<div class="deps-head">${icon("storage", "sm", "Stockpile")} Stockpile Capacity ${fmt(used)} / ${fmt(cap)}</div>` +
      `<div class="storage-row">${bar(Math.min(100, (used / cap) * 100), storageFull ? "is-warn" : "")}` +
      (storageFull ? ` ${badgeChip("storage", "full", "warn", "Storage full — production idles at the cap. Ship goods out or build an Orbital Warehouse to raise it (reserves aren't wasted; accrual resumes when goods ship).")}` : "") +
      `</div>`
    : "";
  // §body-management: COLONY VITALS — population, food rung, workforce, and the
  // Provisions upkeep (§system-reorg: the upkeep moved up here from the
  // production readout; the population eats provisions_per_million_per_s · pop).
  const wf = dyn.workforce;
  const foodState = label(dyn.food_state ?? "well_supplied");
  const popM = dyn.population ?? 0;
  const upkeepRate = dyn.population_upkeep ?? (state.galaxy?.provisions_per_million_per_s ?? 0.06) * popM;
  const vitalCells = [
    stat("Population", `${icon("population", "sm")} ${fmtPopulation(popM)}`),
    stat("Food", `${icon("food", "sm")} ${esc(foodState)}`, dyn.habitat_fed ? "" : "is-warn"),
    stat("Workforce", `${icon("workforce", "sm")} ${wf ? `${Math.min(wf.posted, wf.units)}/${wf.posted}` : "—"}`, wf && wf.posted > wf.units ? "is-warn" : ""),
  ];
  if (popM > 0)
    vitalCells.push(stat("Upkeep", `${icon("upkeep", "sm")} −${upkeepRate.toFixed(2)} ${commodityIcon("provisions", "sm")}/s`, dyn.habitat_fed ? "" : "is-warn"));
  const vitals = popM > 0 || wf ? statStrip(vitalCells) : "";
  // §body-management: the three SLOT POOLS — a system fact, so it reads here
  // (the per-pool gating itself lives with the build rows on the body panels).
  const pools = poolUsage(dyn);
  const poolStrip = `<div class="mhint action-line" title="Slots are PER BODY — one per distinct structure on its body; deepening a built tier never needs a slot. Totals here sum the roster.">${icon("slots", "sm")}<span>` +
    (["resource", "industrial", "infrastructure"] as const)
      .map((k) => `${k} ${pools[k].used}/${pools[k].total}`)
      .join(" · ") + `</span></div>`;
  // §system-reorg: the ROSTER — one row per body (public geography). The row is a
  // NAVIGATION button (opens that body's panel to build/staff/ship) followed by
  // the planet's CONTRIBUTION: the net output of its staffed lines, per commodity
  // (+x/s <icon>). Buildings no longer list here — they live on the body panel.
  const bodies = dyn.bodies ?? [];
  const outByBody = new Map<number, Map<Commodity, number>>();
  for (const a of dyn.assignments ?? []) {
    let m = outByBody.get(a.body_id);
    if (!m) { m = new Map(); outByBody.set(a.body_id, m); }
    for (const [c, r] of a.outputs) if (r > 0.001) m.set(c, (m.get(c) ?? 0) + r);
  }
  const contribFor = (b: BodyView): string => {
    const m = outByBody.get(b.id);
    if (!m || !m.size) return `<span class="dim">undeveloped</span>`;
    return [...m.entries()]
      .sort((x, y) => y[1] - x[1])
      .map(([c, r]) => `<span class="dev-contrib" title="${esc(b.name)} contributes +${r.toFixed(2)} ${esc(label(c))}/s to the colony">+${r.toFixed(2)}/s ${commodityIcon(c, "sm")}</span>`)
      .join(" ");
  };
  const devs = bodies.length
      ? bodies.map((b) => {
        const pop = b.population > 0 ? ` <span class="dim">${fmtPopulation(b.population)}</span>` : "";
        return `<div class="devs-row"><button class="dev act" data-body="${b.id}" title="Open ${esc(b.name)} — build, staff, ship from its panel">${esc(b.name)}</button>${pop} ${bodyProfileTags(b)} ${contribFor(b)}</div>`;
      }).join("")
    : `<div class="mhint">No bodies rostered yet.</div>`;
  // §contestable-territory Part 1: a blockade STRANGLES logistics — outbound
  // dispatches hold at origin, so the ship button is disabled while blockaded
  // (production still accrues into the stockpile). A prominent banner explains it.
  const blockaded = !!dyn.blockade;
  const siege = siegeProgress(dyn);
  const siegeTip = "Defenses suppressed, the siege clock is running. Break the blockade or rebuild a Defense Platform to reset it — at full siege, rival MARINES landing in strength take this system. (Your home can be blockaded but never falls.)";
  const siegeLine = siege
    ? `<div class="deps-head" style="margin-top:6px">${badgeChip("siege", siege.ripe ? "SIEGE CRITICAL — capture imminent" : `siege — falls in ${fmtCountdown(siege.left)}`, "negative", siegeTip)}</div>` +
      `<div class="storage-row">${bar(siege.pct, "is-warn")}</div>`
    : "";
  const blockadeBanner = blockaded
    ? `<div style="margin:6px 0">${badgeChip("blockade", "under blockade", "negative", "A rival fleet holds station — freighters are held in & out (production still accrues). Break the blockade (relief, or a new Defense Platform tier) to resume shipping.")}</div>${siegeLine}`
    : "";
  // §body-management: NO action buttons here — shipping/auto-supply live on
  // the warehouse station panel, builds/worker assignments on their anchor bodies.
  // §syndicates Part 3: the ally GARRISON you're hosting here — the coalition
  // shield you feed (its Provisions upkeep draws from THIS system).
  const gShips = dyn.ally_garrison_ships ?? 0;
  const garrisonHost = gShips > 0
    ? `<div class="deps-head" style="margin-top:6px">${icon("garrison", "sm")} Ally garrison: <b>${gShips}</b> ship${gShips > 1 ? "s" : ""} ` +
      (dyn.ally_garrison_fed
        ? badgeChip("garrison", "fed", "positive", "You're hosting an allied coalition shield here — its Provisions upkeep is covered from this system, and it joins your defense.")
        : badgeChip("garrison", "UNFED", "warn", "The allied garrison here is UNFED — this system is out of Provisions to cover its upkeep, so its defense is suspended. Ship Provisions here to restore it.")) +
      `</div>`
    : "";
  $("svm-eyebrow").textContent = blockaded ? "UNDER BLOCKADE" : "";
  const queue = buildQueueRows(sid, dyn, { nav: true });
  const tabs: readonly UxTabOption<SystemManageTab>[] = [
    ["overview", "Overview", "home"],
    ["worlds", "Worlds", "planetHabitable"],
    ["production", "Production", "storage"],
    ["construction", "Build", "queue"],
  ];
  const active = systemManageTab === "overview"
    ? vitals + poolStrip + groundLine(dyn) + garrisonHost
    : systemManageTab === "worlds"
      ? devs
      : systemManageTab === "production"
        ? storageBar + productionReadout(dyn) + converterBanner(dyn)
        : berthLine(sid) + queue;
  const alert = blockadeBanner ? `<div class="ux-alert ux-alert--danger">${blockadeBanner}</div>` : "";
  setHtml($("svm-body"), alert + uxTabBar(tabs, systemManageTab, "svm-tab") +
    `<div class="ux-tab-body">${active || `<div class="sp-empty">Nothing to show here.</div>`}</div>`);
}


export type PlanetPanelTab = "economy" | "population" | "infrastructure";

export let planetPanelTab: PlanetPanelTab = "economy";

export let planetPanelBuilt = false;

export function buildPlanetPanel(): void {
  if (planetPanelBuilt) return;
  planetPanelBuilt = true;
  // §body-management: the body panel is THE action surface — every branch
  // sends the SAME system-level command the old summary buttons sent (the
  // anchor is a lens, not an address; nothing here is per-planet on the wire).
  $("planet-panel").addEventListener("click", (e) => {
    if ((e.target as HTMLElement).closest("[data-act='close']")) { closePlanetPanel(); return; }
    const tabButton = (e.target as HTMLElement).closest("[data-planet-tab]") as HTMLElement | null;
    const requestedTab = tabButton?.dataset.planetTab as PlanetPanelTab | undefined;
    if (requestedTab && (["economy", "population", "infrastructure"] as PlanetPanelTab[]).includes(requestedTab)) {
      planetPanelTab = requestedTab;
      refreshOpenBodyPanel();
      return;
    }
    const el = (e.target as HTMLElement).closest("[data-build],[data-crew],[data-action],[data-fit],[data-migration],[data-relocate]") as HTMLElement | null;
    const sid = viewedSystemId();
    if (!el || !sid || !net) return;
    if (el.dataset.fit) {
      // §modules Part B4: toggle a module into the composed fit (max 2 slots).
      const m = el.dataset.fit as ModuleKind;
      const i = pendingFit.indexOf(m);
      if (i >= 0) pendingFit.splice(i, 1);
      else if (pendingFit.length < 2) pendingFit.push(m);
      refreshOpenBodyPanel();
      return;
    }
    if (el.dataset.migration && openBodyDetail) {
      net.send({
        type: "SetMigrationPolicy",
        system_id: sid,
        body_id: Number(openBodyDetail.id),
        policy: el.dataset.migration as MigrationPolicy,
      });
      refreshOpenBodyPanel();
      return;
    }
    if (el.dataset.relocate && openBodyDetail) {
      const select = $("planet-panel").querySelector(".migration-dest") as HTMLSelectElement | null;
      const [toSystem, toBody] = select?.value.split("|") ?? [];
      if (toSystem && toBody !== undefined) {
        net.send({
          type: "RelocateMigrants",
          from_system: sid,
          from_body: Number(openBodyDetail.id),
          to_system: toSystem,
          to_body: Number(toBody),
        });
      }
      return;
    }
    if (el.dataset.crew) {
      sendCrew(sid, el.dataset.crew);
      refreshOpenBodyPanel();
      updateSysviewManage();
      return;
    }
    if (el.dataset.build) {
      dispatchBuildKey(el.dataset.build, sid, openBodyDetail ? Number(openBodyDetail.id) : undefined);
      refreshOpenBodyPanel(); // the queue row appears on the next View push
      updateSysviewManage();
      return;
    }
    switch (el.dataset.action) {
      case "open-builder":
        // §build-panel: open the dedicated builder for THIS body (a sibling panel
        // to the right — nothing is sent until "Queue build").
        if (openBodyDetail) openBuildPanel(openBodyDetail.id);
        break;
      case "open-shipyard":
        // §build-ship-panel: open the ship builder for this shipyard body (its
        // sibling — opening it closes the structure builder, and vice-versa).
        if (openBodyDetail) openShipPanel(openBodyDetail.id);
        break;
      case "standing":
        // Logistics is a corp-wide galaxy concern — a deliberate context switch.
        exitSystem();
        openRail("logistics");
        updateStandingPanel();
        {
          const sel = $("so-source") as HTMLSelectElement;
          if ([...sel.options].some((o) => o.value === sid)) sel.value = sid;
        }
        break;
    }
  });
}

/// §body-management: the OPEN body panel (its visual detail), for live
/// re-renders as Views land — worker counts, queue bars, afford states.
export let openBodyDetail: SystemBodyDetail | null = null;

export function refreshOpenBodyPanel(): void {
  if (!openBodyDetail || !$("planet-panel").classList.contains("is-open")) return;
  if (renderDeferred("planet-panel", refreshOpenBodyPanel)) return; // §single-click
  openPlanetPanel(openBodyDetail);
}

export function closePlanetPanel(): void {
  openBodyDetail = null;
  closeBuildPanel(); // the builder is a child of the planet context
  $("planet-panel").classList.remove("is-open");
}

/// §body-management: a section header for the body panel.
export const ppSec = (title: string, tip = "", iconKey?: IconKey): string =>
  `<div class="sp-sec panel-subhead"${tip ? ` title="${esc(tip)}"` : ""}>${iconKey ? icon(iconKey, "sm", title) : ""}${esc(title)}</div>`;


export function bodyProfileTags(b: BodyView): string {
  const planet = icon(ENVIRONMENT_ICON[b.environment], "sm", label(b.environment));
  const geology = b.geology && GEOLOGY_ICON[b.geology]
    ? icon(GEOLOGY_ICON[b.geology]!, "sm", `${label(b.geology)} minerals`)
    : "";
  const feature = b.special && FEATURE_ICON[b.special]
    ? icon(FEATURE_ICON[b.special]!, "sm", label(b.special))
    : "";
  const known = b.geology
    ? ` · ${geology}<b>${esc(label(b.geology))}</b> minerals${b.special ? ` · ${feature}<b>${esc(label(b.special))}</b>` : ""}`
    : " · geology unsurveyed";
  return `<span class="dim body-profile-tags">${planet}${esc(label(b.size))} · ${esc(label(b.environment))}${known}</span>`;
}


export function bodyProfileReadout(b: BodyView): string {
  const planet = icon(ENVIRONMENT_ICON[b.environment], "sm", label(b.environment));
  const geologyKey = b.geology ? GEOLOGY_ICON[b.geology] : undefined;
  const geo = b.geology == null
    ? `<span class="pp-pool" title="Survey this system to learn the mineral grade and any rare feature.">Minerals: unsurveyed</span>`
    : `<span class="pp-pool" title="Mineral deposits on this body extract at this natural grade before research and staffing.">${geologyKey ? icon(geologyKey, "sm", `${label(b.geology)} minerals`) : ""}Minerals: ${esc(label(b.geology))} ×${(b.mineral_extraction_mult ?? 1).toFixed(2)}</span>`;
  const featureKey = b.special ? FEATURE_ICON[b.special] : undefined;
  const feature = b.special
    ? `<div class="pp-note pp-feature">${featureKey ? icon(featureKey, "md", label(b.special)) : ""}<span><b>${esc(label(b.special))}</b> — ${esc(b.special_effect ?? "Rare planetary feature")}</span></div>`
    : b.geology == null
      ? ""
      : `<div class="mhint">No rare planetary feature detected.</div>`;
  return ppSec("Planetary profile", "Size and environment are public astronomy. Mineral grade, deposits and rare features require a survey.", ENVIRONMENT_ICON[b.environment]) +
    `<div class="pp-pools"><span class="pp-pool">${planet}${esc(label(b.size))}</span><span class="pp-pool">${esc(label(b.environment))}</span>${geo}</div>` +
    `<div class="pp-pools"><span class="pp-pool" title="Natural Habitat capacity before research">Habitat cap ×${b.habitat_capacity_mult.toFixed(2)}</span>` +
    `<span class="pp-pool" title="How strongly this environment attracts civilian settlers before research and policy">Settlement appeal ×${b.population_growth_mult.toFixed(2)}</span>` +
    `<span class="pp-pool" title="Per-capita Provisions use">Provisions ×${b.provisions_mult.toFixed(2)}</span>` +
    `<span class="pp-pool" title="Structure construction time on this body">Construction ×${b.construction_time_mult.toFixed(2)}</span></div>${feature}`;
}


export function openPlanetPanel(d: SystemBodyDetail): void {
  buildPlanetPanel();
  if (openBodyDetail?.id !== d.id) planetPanelTab = "economy";
  // §build-panel: retargeting to a DIFFERENT body closes a stale builder (it
  // pointed at the old body); a live refresh of the SAME body keeps it open.
  if (buildTargetBodyId && buildTargetBodyId !== d.id) closeBuildPanel();
  openBodyDetail = d;
  // §bodies: THE WIRE BODY — deposits/structures/population are ITS OWN now.
  const sid = viewedSystemId();
  const dyn = sid ? state.systems.find((s) => s.id === sid) : undefined;
  const body = dyn?.bodies?.find((b) => String(b.id) === d.id);
  const planetKey = body ? ENVIRONMENT_ICON[body.environment] : d.habitable ? "planetHabitable" : "planetUninhabitable";
  const eyebrow = d.isMoon ? "natural satellite" : d.habitable ? "habitable world" : "planet";
  const habitable = d.habitable ? " " + badge("positive", "habitable") : "";
  const head =
    `<div class="pp-head"><div class="panel-title"><div><div class="eyebrow">${esc(eyebrow)}</div>` +
    `<h2>${icon(planetKey, "md", eyebrow)} ${esc(d.name)}</h2></div></div>` +
    `<button class="pp-close" data-act="close" title="Close" aria-label="Close">✕</button></div>`;
  // The body's art as the panel thumbnail (mirrors the star concept banner in
  // the System tab); the color swatch stays as the no-art fallback.
  const thumb = d.icon
    ? `<img class="pp-thumb" src="${d.icon}" alt="" />`
    : "";
  const kindLine = `<div class="pp-kindrow">${thumb}<div><span class="pp-swatch" style="background:${hex6(d.kindColor)}"></span>${esc(d.kindLabel)}${habitable}</div></div>`;
  const profile = body ? bodyProfileReadout(body) : "";
  const bodyRoles = (dyn?.opportunities ?? []).filter((o) => String(o.body_id) === d.id);
  const roles = bodyRoles.length
    ? ppSec("Colony roles", "Surveyed combinations scored against the dependable home baseline; investment multipliers come later.") +
      bodyRoles.map((o) => {
        const tone = o.tier === "jackpot" ? "positive" : o.tier === "exceptional" ? "accent" : "neutral";
        return `<div class="sp-line colony-role" title="${esc(o.reason)}">${icon(COLONY_ROLE_ICON[o.role], "md", o.title)}${badge(tone, o.tier)} <b>${esc(o.title)}</b> <span class="positive">×${o.score.toFixed(2)}</span></div>`;
      }).join("")
    : "";
  // 1. GEOLOGY — this body's deposits (survey-gated on the wire: null =
  // unsurveyed, [] = surveyed and barren).
  const deps = body
    ? body.deposits == null
      ? `<div class="pp-note" style="border:0;padding:0;margin-top:10px">Geology unsurveyed — a survey reveals what lies here, and on which body.</div>`
      : body.deposits.length
        ? ppSec("Geology", "Surveyed deposits on this body.", body.geology && GEOLOGY_ICON[body.geology] ? GEOLOGY_ICON[body.geology] : undefined) + body.deposits.map(depositRow).join("")
        : `<div class="pp-note" style="border:0;padding:0;margin-top:10px">No deposits on this body.</div>`
    : d.deposits.length
      ? ppSec("Geology") + d.deposits.map(depositRow).join("")
      : `<div class="pp-note" style="border:0;padding:0;margin-top:10px">No deposits on this body.</div>`;
  // §bodies: THE ACTION SURFACE — owner's own system only (fog law: rivals get
  // geology + flavor, nothing else, ever).
  let economy = "";
  let population = "";
  let infrastructure = "";
  const mine = !!dyn && dyn.owner !== null && dyn.owner === state.playerId;
  if (mine && sid && dyn && body) {
    const tiers = body.structures ?? {};
    const blockaded = !!dyn.blockade;
    const blockChip = blockaded
      ? `<div style="margin-top:8px">${badgeChip("blockade", "under blockade", "negative", "A rival fleet holds station — freighters are held in & out. Production and construction continue; shipping resumes when the blockade breaks.")}</div>`
      : "";

    // Population is event-driven: every post-founding cohort is reserved, then
    // credited only when its physical Authority liner unloads here.
    const habitatTier = tiers["habitat"] ?? 0;
    const capM = habitatTier
      * (state.galaxy?.pop_cap_per_habitat_tier ?? 0.025)
      * body.habitat_capacity_mult;
    const inbound = body.inbound_migrants ?? 0;
    const migrationPolicy = body.migration_policy ?? "managed";
    const policyCopy: Record<MigrationPolicy, string> = {
      closed: "No new migrant liners will be allocated.",
      managed: "Accept settlers only while posted jobs exceed the system workforce.",
      open: "Accept settlers while food and Habitat capacity permit.",
      priority: "Prefer this body and shorten its allocation interval.",
    };
    const policies = (["closed", "managed", "open", "priority"] as MigrationPolicy[])
      .map((policy) => `<button class="act${migrationPolicy === policy ? " is-on" : ""}" data-migration="${policy}" title="${esc(policyCopy[policy])}">${esc(label(policy))}</button>`)
      .join("");
    const inboundLine = inbound > 0
      ? `<div class="sp-line positive">${icon("population", "sm")} <b>${inbound.toLocaleString()} inbound</b> aboard Authority migrant ${inbound === (state.galaxy?.migrant_cohort_people ?? 1_000) ? "liner" : "liners"}</div>`
      : `<div class="mhint">No migrant liner is currently allocated here.</div>`;
    const cohortPeople = state.galaxy?.migrant_cohort_people ?? 1_000;
    const cohortM = cohortPeople / 1_000_000;
    const relocationOptions = state.systems
      .filter((system) => system.owner === state.playerId && !system.blockade && system.habitat_fed)
      .flatMap((system) => (system.bodies ?? [])
        .filter((candidate) => {
          if (system.id === sid && candidate.id === body.id) return false;
          if (candidate.population <= 0) return false;
          const capacity = (candidate.structures?.habitat ?? 0)
            * (state.galaxy?.pop_cap_per_habitat_tier ?? 0.025)
            * candidate.habitat_capacity_mult;
          return capacity - candidate.population - (candidate.inbound_migrants ?? 0) / 1_000_000 + 1e-12 >= cohortM;
        })
        .map((candidate) => {
          const systemName = state.galaxy?.systems.find((known) => known.id === system.id)?.name ?? system.id;
          return `<option value="${esc(system.id)}|${candidate.id}">${esc(systemName)} · ${esc(candidate.name)}</option>`;
        }));
    const canRelocate = body.population + 1e-12 >= cohortM * 2 && relocationOptions.length > 0;
    const relocate = `<div class="sp-line"><span><b>Internal relocation</b><small>Move one ${cohortPeople.toLocaleString()}-person cohort on a physical liner. One founding cohort must remain.</small></span></div>` +
      (relocationOptions.length
        ? `<div class="sp-actions"><select class="lg-com migration-dest" ${canRelocate ? "" : "disabled"}>${relocationOptions.join("")}</select><button class="act" data-relocate="1" ${canRelocate ? "" : "disabled"}>Relocate cohort</button></div>`
        : `<div class="mhint">No other owned, supplied body currently has room for a complete cohort.</div>`);
    const migrationSec = ppSec("Population & migration", "Population changes on physical arrivals. One liner carries one 1,000-person workforce cohort; closing a policy does not recall a ship already under way.", "population") +
      `<div class="pp-pools"><span class="pp-pool">Population ${fmtPopulation(body.population)}</span><span class="pp-pool">Habitat ${fmtPopulation(capM)}</span></div>` +
      inboundLine +
      `<div class="sp-line"><span><b>Immigration policy</b><small>${esc(policyCopy[migrationPolicy])}</small></span></div>` +
      `<div class="sp-actions">${policies}</div>${relocate}`;

    // 2. BUILT HERE — structures ON this body, with status chips.
    const builtKeys = Object.keys(tiers).filter((k) => (tiers[k] ?? 0) > 0);
    const lineOf = (slug: string) => dyn.assignments?.find((a) => a.body_id === body.id && a.structure === slug);
    const built = builtKeys.length
      ? ppSec("Built here") + `<div class="devs-row">` + builtKeys.map((k) => {
          const line = lineOf(k);
          const status = line?.suspended
            ? ` ${badgeChip("unfed", esc(label(line.suspended)), "warn", SUSPEND_HINT[line.suspended] ?? "suspended — nothing is lost")}`
            : !line && PRODUCER_SLUGS.has(k) ? ` ${badge("warn", "unstaffed")}` : "";
          return `<span class="dev" title="${esc(label(k))} ×${tiers[k]}">${esc(label(k))} <b>×${tiers[k]}</b>${status}</span>`;
        }).join(`<span class="dev-sep">·</span>`) + `</div>`
      : "";

    // 3. PRODUCTION LINES — this body's lines, worker assignment controls (SetAssignment
    // now carries the body).
    const lines = assignmentLines(dyn, true, body);
    const linesSec = lines ? ppSec("Production lines", "output = richness/rate × tier × staffing × skill × food — hover a row for its chain") + lines : "";

    // 4. BUILD — the per-body slot pools at a glance + ONE button into the
    // dedicated build panel (the per-structure grid moved there wholesale, so the
    // geology/built-here above stay readable while you choose what to build).
    const pools = bodyPoolUsage(body, dyn);
    const structOpts = ((state.galaxy?.build_options ?? []) as BuildOpt[]).filter((o) => !SHIP_KEYS.has(o.key) && !!POOL_OF[o.key]);
    // Openable if there's anything to DO here — a foundable structure (free slot +
    // deposit) or an existing tier to deepen (goods aside; the panel shows afford).
    const anyOpenable = structOpts.some((o) => {
      const st = structOption(o, dyn, body, pools);
      return st.tierUp || (!st.poolFull && !st.noDeposit);
    });
    const poolStrip = `<div class="pp-pools">` + (["resource", "industrial", "infrastructure"] as const)
      .map((k) => `<span class="pp-pool${pools[k].used >= pools[k].total ? " is-full" : ""}" title="${POOL_LABEL[k]} slots used / total on this body — founding a new structure needs a free slot; deepening a tier never does.">${POOL_LABEL[k]} ${pools[k].used}/${pools[k].total}</span>`)
      .join("") + `</div>`;
    const buildSec = ppSec("Build", "The at-a-glance slot pools — the reason to open the builder. Founding a NEW structure claims one of this body's pool slots; tier-ups deepen in place.") +
      poolStrip +
      `<button class="act pp-build-open" data-action="open-builder" ${anyOpenable ? "" : "disabled"} title="${anyOpenable ? "Open the build panel — pick a structure, read its recipe & effect, then queue it." : "Nothing buildable here — every slot pool is full and there's nothing to deepen. Grow this body's population, or build on another body."}">${icon("build", "sm")} Build Structure</button>`;

    // Per-body construction queue (ship jobs render under the yard below).
    const bodyQueue = buildQueueRows(sid, dyn, { filter: (j) => j.body_id === body.id && !SHIP_KEYS.has(j.key), seenKey: `${sid}#b${body.id}` });

    // 5. SHIPYARD on this body: SHIP CONSTRUCTION — the orbital yard's menu + queue.
    // §modules Part B4: the FIT PICKER rides above the warship buttons (fits the
    // next warship built here from what's in the module ledger).
    let yardSec = "";
    if ((tiers["shipyard"] ?? 0) > 0) {
      const shipOpts = (state.galaxy?.build_options ?? []).filter((o) => SHIP_KEYS.has(o.key));
      const shipQueue = buildQueueRows(sid, dyn, { filter: (j) => SHIP_KEYS.has(j.key), seenKey: `${sid}#yard` });
      // The per-hull rows moved into the dedicated ship builder; the planet panel
      // keeps the fit composer + the yard's queue (watch it here, choose there).
      // §yards: the whole family, each with its own slipway count — a tier-N yard
      // holds N hulls on the stocks, and the yards count their slips separately.
      const yardChips = (["shipyard", "naval_drydock", "capital_slipway", "ordnance_foundry"] as const)
        .map((k) => [k, dyn.structures?.[k] ?? 0] as const)
        .filter(([k, t]) => t > 0 || k === "shipyard")
        .map(([k, t]) => {
          const busy = (dyn.builds ?? []).filter((j) => SHIP_YARD[j.key]?.yard === k).length;
          const slips = k === "ordnance_foundry" ? "" : ` · ${busy}/${slipsFor(t)} slips`;
          const tip = k === "ordnance_foundry"
            ? "Outfitting and maintenance — refits install here, and damaged hulls docked here are repaired. Repair requires assigned workers, like any production line."
            : `A tier-${t} yard holds ${slipsFor(t)} hull${slipsFor(t) === 1 ? "" : "s"} on the stocks at once.`;
          // §roster: a foundry with NO worker assigned services nothing — and says so.
          // It isn't a converter, so the idle-converter banner never covers it.
          const unstaffed = k === "ordnance_foundry"
            && t > 0
            && !(dyn.assignments ?? []).some((a) => a.structure === "ordnance_foundry" && a.workers > 0)
            ? ` <span class="warn" title="An unstaffed foundry installs no refits and repairs nothing. Assign a worker from this body's production lines.">· unstaffed</span>`
            : "";
          return `<span class="pp-pool" title="${esc(tip)}">${icon("shipyard", "sm")} ${esc(YARD_TITLE[k])} ${romanTier(t)}${slips}${unstaffed}</span>`;
        }).join("");
      yardSec = shipOpts.length
        ? ppSec("Orbital yards — ship construction", "Light hulls build at the Shipyard, the line of battle at a Naval Drydock, super-capitals at a Capital Slipway. Each yard's tier is its slipway count.") +
          `<div class="pp-yardline">${yardChips}` +
          `<button class="act pp-build-open" data-action="open-shipyard" title="Open the ship builder — pick a hull, set a quantity, read its stats & recipe, then queue it.">${icon("shipyard", "sm")} Build Ship</button></div>` +
          fitPicker(dyn) +
          shipQueue
        : "";
    }

    // 5b. ARMAMENTS COMPLEX on this body: MODULE MANUFACTURE + the system ledger
    // (§modules Part B3). Modules pool in the ledger and fit ships at build/refit.
    let modulesSec = "";
    if ((tiers["armaments_complex"] ?? 0) > 0) {
      modulesSec = ppSec("Armaments — module manufacture", "Modules are manufactured here into the system's module ledger, then fitted to warships at build (the yard's fit picker) or by refitting a docked fleet.") +
        moduleForge(dyn);
    }

    // 6. ORBITAL WAREHOUSE on this body: LOGISTICS. The warehouse is CAPACITY and
    // nothing else — it is NOT a permit to ship (dispatch is system-scoped and
    // needs no structure), so no per-body shipping button lives here, and it buys
    // no favour from the Authority (freight terms are uniform).
    let warehouseSec = "";
    if ((tiers["orbital_warehouse"] ?? 0) > 0) {
      warehouseSec = ppSec("Orbital Warehouse — logistics", "Raises how much this system can hold. The stockpile it guards is system-wide, and shipping out needs no structure at all.") +
        `<div><button class="act" data-action="standing" title="Set a standing logistics rule that auto-dispatches freighters from here (online or off).">${icon("doctrine", "sm")} Auto-supply</button></div>`;
    }

    // The action surface leads: slot pressure + Build first, its active queue
    // immediately below, then the built/production reference detail. Players
    // can act without reading through the full colony sheet first.
    population = blockChip + migrationSec;
    economy = built + linesSec;
    infrastructure = buildSec + bodyQueue + yardSec + modulesSec + warehouseSec;
  }

  const summary = `<div class="pp-summary">${kindLine}</div>`;
  const tabs: readonly UxTabOption<PlanetPanelTab>[] = [
    ["economy", "Economy", "storage"],
    ["population", "Population", "population"],
    ["infrastructure", "Build", "build"],
  ];
  const activeManagement = planetPanelTab === "economy"
    ? economy || `<div class="sp-empty">No production on this world.</div>`
    : planetPanelTab === "population"
      ? population || `<div class="sp-empty">No colony population here.</div>`
      : infrastructure || `<div class="sp-empty">No infrastructure here.</div>`;
  const management = mine
    ? uxTabBar(tabs, planetPanelTab, "planet-tab") + `<div class="ux-tab-body">${activeManagement}</div>`
    : "";
  const worldData = `<details class="ux-details"${mine ? "" : " open"}><summary>${icon(planetKey, "sm")} World data</summary>` +
    `<div class="ux-details__body">${profile + roles + deps}</div></details>`;
  setHtml($("planet-panel"), head + `<div class="pp-body">${summary}${management}${worldData}</div>`);
  $("planet-panel").classList.add("is-open");
}


export function depositRow(d: Deposit): string {
  const pct = Math.min(100, d.richness * 40);
  const reserves = d.reserves === null
    ? `<span class="tone-up">renewable</span>`
    : d.reserves < 50 ? `<span class="is-warn">${fmt(d.reserves)} left</span>`
      : `${fmt(d.reserves)} left`;
  return `<div class="dep-row"><span class="dep-ico">${commodityIcon(d.resource, "md")}</span>` +
    `<span class="dep-name">${label(d.resource)}</span>${bar(pct)}` +
    `<span class="dep-r">~${d.richness.toFixed(2)}/s · ${reserves}</span></div>`;
}


// Owner-only production readout: per-resource stockpile + the deposit yield as its
// flow (the protocol carries no separate per-tick flow). Gated behind ownership.
// Per-Extractor-tier output multiplier — MIRRORS the sim's `EXTRACTOR_RICHNESS_MULT`
// (crates/sim/src/build.rs). Production compounds as `richness · MULT^tier`, so the
// readout shows the ACTUAL current output, not the intrinsic geology (which the
// Geology section above shows unmodified).
export const EXTRACTOR_RICHNESS_MULT = 1.5;


export function constructionStockTotal(stockpiled: number, freight: number): string {
  const total = stockpiled + freight;
  if (freight <= 0) return fmt(total);
  return `<span title="${fmt(stockpiled)} stockpiled + ${fmt(freight)} aboard docked Freighters">${fmt(total)} <span class="dim">(+${fmt(freight)})</span></span>`;
}


export function productionReadout(dyn: SystemStateView | undefined): string {
  if (!dyn) return "";
  const { stockpile: stockOf, freighters, available } = constructionStock(dyn);
  const tier = dyn?.extractor_tier ?? 0;
  const mult = Math.pow(EXTRACTOR_RICHNESS_MULT, tier);
  const rateOf = new Map<Commodity, number>();
  // §explore: the readout is owner-only, and an owner always knows their own
  // geology (dyn.deposits present) — read from the light-gated view.
  for (const d of dyn?.deposits ?? []) rateOf.set(d.resource, (rateOf.get(d.resource) ?? 0) + d.richness * mult);
  const all = new Set<Commodity>([...available.keys(), ...rateOf.keys()] as Commodity[]);
  const rows = [...all].filter((c) => (available.get(c) ?? 0) >= 1 || (rateOf.get(c) ?? 0) > 0.01);
  if (!rows.length) return "";
  // (The Fuel Refinery — like every converter — now reports its real idle reason
  // via `converterBanner`, which reads the server-computed status. The old
  // volatiles>0 heuristic here wrongly claimed "staffed line" without checking
  // assigned workers, so it was dropped in favour of the accurate banner.)
  return rows.map((c) => {
      const rt = rateOf.get(c) ?? 0;
      const rate = rt > 0.01 ? `<span class="sp-rate">+${rt.toFixed(2)}/s</span>` : `<span class="sp-none">—</span>`;
      // Fuel here is the system's operating/movement RESERVE — not the tradeable HQ
      // inventory the Exchange buys/sells. Tag it so a market sell "not moving" this
      // number reads as expected, not a bug.
      const nameCell = c === "fuel"
        ? `<span class="sp-name" title="This system's operating/movement reserve — ships spend it to move. NOT the hub warehouse the Exchange buys/sells against, so a market Buy/Sell never changes it.">${label(c)} <span class="dim" style="font-size:9px">· reserve</span></span>`
        : `<span class="sp-name">${label(c)}</span>`;
      return `<div class="sys-prod"><span class="dep-ico">${commodityIcon(c, "md")}</span>` +
        `${nameCell}<span class="sp-stock">${constructionStockTotal(stockOf.get(c) ?? 0, freighters.get(c) ?? 0)}</span>${rate}</div>`;
    }).join("");
}


// §economy Part 6: the per-line PRODUCTION ROWS — one row per line with the
// server-resolved factor chain (shown math: throughput × staffing × skill ×
// food), worker assignment controls (data-crew), suspension causes. Rendered on the BODY
// panels now (§system-reorg dropped the system-screen workforce block).
export const PRODUCER_SLUGS = new Set([
  "mining_complex", "volatile_harvester", "bioharvester", "smelter",
  "electronics_fabricator", "chemical_works", "fuel_refinery", "agroplex",
  "machine_works", "armaments_complex", "shipyard", "academy",
]);

export const SUSPEND_HINT: Record<string, string> = {
  no_food: "out of Provisions — ship food",
  no_inputs: "input basket dry — ship raws in or staff extraction",
  storage_full: "storage full — ship goods out or build an Orbital Warehouse",
  needs_crew: "built but idle — assign a worker from its body panel",
};


// §economy: an at-a-glance banner for BUILT converters producing nothing, with
// the reason (needs worker / no inputs / no food / storage full). "" when all run.
export function converterBanner(dyn: SystemStateView | undefined): string {
  const idle = (dyn?.converters ?? []).filter((c) => c.status !== "running");
  if (!idle.length) return "";
  const rows = idle.map((c) => {
    const tone = c.status === "no_food" ? "negative" : "warn";
    const word = c.status === "needs_crew" ? "needs worker"
      : c.status === "no_inputs" ? "no inputs"
      : c.status === "no_food" ? "no food"
      : c.status === "storage_full" ? "storage full" : c.status;
    return `<div class="conv-idle">${badgeChip("unfed", esc(word), tone, SUSPEND_HINT[c.status] ?? "idle")} <b>${esc(c.title)}</b></div>`;
  }).join("");
  return `<div class="deps-head">Idle converters</div>${rows}`;
}

// §body-management: the production-line rows, shared between the read-only
// summary/rail digest (withControls=false — pure data) and the BODY PANELS
// (withControls=true — the ONLY place worker assignment controls render; the SetAssignment
// sends are byte-identical, just relocated). `slugFilter` scopes a body panel
// to the structures anchored there.
export function assignmentLines(dyn: SystemStateView | undefined, withControls: boolean, forBody?: BodyView): string {
  const nameOf = new Map((dyn?.bodies ?? []).map((b) => [b.id, b.name] as const));
  const lines = (dyn?.assignments ?? []).filter((a) => !forBody || a.body_id === forBody.id);
  // §bodies: a line is keyed (body, structure) — idle detection must match.
  const postedAt = new Set((dyn?.assignments ?? []).map((a) => `${a.body_id}:${a.structure}`));
  const rowFor = (a: AssignmentView): string => {
    const chain = `×${a.throughput.toFixed(1)} tier · ×${a.staffing.toFixed(2)} staffing · ×${a.skill.toFixed(2)} skill · ×${a.food.toFixed(2)} food` +
      (Math.abs(a.site - 1) > 0.001 ? ` · ×${a.site.toFixed(2)} planet` : "");
    const out = a.outputs.filter(([, r]) => r > 0.001).map(([c, r]) => `+${r.toFixed(2)} ${esc(label(c))}/s`).join(" ");
    const activity = out || (a.structure === "academy" ? "research" : "—");
    const spec = Object.entries(a.specialists).map(([k, n]) => `${n as number}× ${esc(label(k))}`).join(", ");
    const susp = a.suspended
      ? ` ${badgeChip("unfed", esc(label(a.suspended)), "warn", SUSPEND_HINT[a.suspended] ?? "suspended — nothing is lost")}`
      : "";
    const controls = withControls
      ? `<button class="act" data-crew="${a.body_id}:${a.structure}:${a.workers + 1}" title="Assign another worker">Assign</button>` +
        `<button class="act" data-crew="${a.body_id}:${a.structure}:${Math.max(0, a.workers - 1)}" title="Unassign one worker">Unassign</button>`
      : "";
    // The body lives in the hover title — the roster table already maps
    // what's where, and the row grid is tuned for short names.
    return `<div class="sys-prod sys-prod--flow" title="${esc(a.title)} ×${a.tier} on ${esc(nameOf.get(a.body_id) ?? "—")} — ${chain}${spec ? ` · specialists: ${spec}` : ""}">` +
      `<span class="sp-name">${esc(a.title)} ×${a.tier}</span>` +
      `<span class="sp-stock">${a.workers}👷${spec ? ` +${Object.values(a.specialists).reduce((s: number, n) => s + (n as number), 0)}🎓` : ""}</span>` +
      `<span class="sp-rate">${activity}</span>${susp}${controls}` +
      `</div>`;
  };
  const idle = (forBody ? [forBody] : dyn?.bodies ?? [])
    .flatMap((b) => Object.entries(b.structures ?? {})
      .filter(([slug, t]) => t > 0 && PRODUCER_SLUGS.has(slug) && !postedAt.has(`${b.id}:${slug}`))
      .map(([slug, t]) =>
        `<div class="sys-prod sys-prod--flow dev--none" title="built but UNSTAFFED — it produces nothing until a worker is assigned${withControls ? "" : " (assign one from its body\u2019s panel)"}">` +
        `<span class="sp-name">${esc(label(slug))} ×${t}</span><span class="sp-none">unstaffed</span>` +
        (withControls ? `<button class="act" data-crew="${b.id}:${slug}:1" title="Assign one worker">Assign worker</button>` : "") +
        `</div>`))
    .join("");
  return lines.map(rowFor).join("") + idle;
}


// Build / develop panel (§step1 growth + structure sinks) for an OWNED system:
// each buildable option with its recipe cost + afford state (costs draw from THIS
// system's stockpile), plus any in-progress build with an ETA. Fog-safe — only
// rendered for systems you own (the View only sends build state to the owner).
// Ship build keys — units, not developments: they never consume a development
// slot (mirrors the sim's slot rule in world.rs apply_build).
export const SHIP_KEYS = new Set(["convoy", "raider", "corvette", "colony", "scout", "destroyer", "cruiser", "battleship", "dreadnought", "titan"]);

// --- §build-progress: the construction QUEUE (Travian-style) -----------------
// Rows derive ENTIRELY from the job timestamps the view already carries:
// `complete_time` (sim-time) from the server + the recipe's `build_secs` from
// the public build options give start = complete − total, so the bar fill and
// the countdown recompute from scratch every render — correct across reconnects
// and offline gaps by construction (no client-accumulated time, same pattern as
// the order-echo countdowns; no per-second traffic).
export const BUILD_ICON: Record<string, string> = {
  convoy: "concept-convoy", raider: "action-attack-raid", corvette: "concept-fleet",
  colony: "action-claim-system", scout: "action-survey-scout",
  extractor: "resource-metals", orbital_warehouse: "action-load-cargo", shipyard: "action-build",
  naval_drydock: "action-build", capital_slipway: "action-build", ordnance_foundry: "action-build",
  sensor_array: "concept-sensor-range", defense_platform: "status-warning-threat",
  habitat: "resource-supplies", refinery: "resource-fuel",
  // §ground: the garrison and the hull it musters.
  garrison: "status-warning-threat", transport: "action-claim-system",
};

export const buildLabel = (key: string): string => buildOption(key)?.label ?? key;

// Brief ✓ resolve when a watched job leaves the queue (the completion notice /
// digest entry are unchanged — this is just the row's exit animation). The
// last-seen stamp keeps a long-closed panel from "flashing" stale history.
export const buildQueueSeen = new Map<string, { keys: string[]; at: number }>();

export const buildDoneFlash = new Map<string, { label: string; until: number }[]>();

export function buildQueueRows(
  sid: string,
  dyn: SystemStateView | undefined,
  opts?: {
    /// §bodies: rows NAVIGATE to their build site (data-body from the job's
    /// own body_id — never a command).
    nav?: boolean;
    /// Scope the rows (e.g. the shipyard panel shows only ship jobs).
    filter?: (j: BuildState) => boolean;
    /// Independent completion-flash bookkeeping per surface (the summary and a
    /// body panel may both render queues for one system).
    seenKey?: string;
  },
): string {
  const jobs = (dyn?.builds ?? []).filter((j) => !opts?.filter || opts.filter(j));
  const now = liveSimTime();
  const seenAs = opts?.seenKey ?? sid;
  // Diff vs the previous render to catch completions (only if seen recently).
  const prev = buildQueueSeen.get(seenAs);
  const keys = jobs.map((j) => j.key);
  if (prev && performance.now() - prev.at < 2000) {
    const remaining = [...keys];
    for (const k of prev.keys) {
      const i = remaining.indexOf(k);
      if (i >= 0) remaining.splice(i, 1);
      else {
        const flashes = buildDoneFlash.get(seenAs) ?? [];
        flashes.push({ label: buildLabel(k), until: performance.now() + 4000 });
        buildDoneFlash.set(seenAs, flashes);
      }
    }
  }
  buildQueueSeen.set(seenAs, { keys, at: performance.now() });
  const flashes = (buildDoneFlash.get(seenAs) ?? []).filter((f) => f.until > performance.now());
  buildDoneFlash.set(seenAs, flashes);
  if (!jobs.length && !flashes.length) return "";

  // Resulting tier per development job: the SITE BODY's current tier + 1 +
  // same-site jobs ahead (§bodies: tiers live on bodies).
  const bodyTier = (bid: number, slug: string): number =>
    (dyn?.bodies?.find((b) => b.id === bid)?.structures ?? {})[slug] ?? 0;
  const aheadCount: Record<string, number> = {};
  const rows = jobs.map((j) => {
    const total = buildOption(j.key)?.build_secs ?? 0;
    const start = j.complete_time - total;
    const pct = total > 0 ? Math.max(0, Math.min(100, ((now - start) / total) * 100)) : 0;
    const left = Math.max(0, j.complete_time - now);
    const isDev = !SHIP_KEYS.has(j.key);
    const site = `${j.body_id}:${j.key}`;
    const ahead = aheadCount[site] ?? 0;
    aheadCount[site] = ahead + 1;
    const name = isDev ? `${buildLabel(j.key)} ×${bodyTier(j.body_id, j.key) + 1 + ahead}` : buildLabel(j.key);
    // §bodies: a queue row NAVIGATES to its site body when the caller asks.
    const bodyId = opts?.nav ? String(j.body_id) : null;
    const nav = bodyId ? ` data-body="${bodyId}" style="cursor:pointer" title="under construction — click to open its body panel"` : "";
    return `<div class="bq-row"${nav}><span class="bq-ic">${svgIcon(BUILD_ICON[j.key] ?? "action-build", "sm")}</span>` +
      `<div class="bq-main"><div class="bq-head"><b>${esc(name)}</b>` +
      `<span class="bq-eta">${fmtCountdown(left)} · done ${doneAtLocal(j.complete_time)}</span></div>` +
      `${bar(pct)}</div></div>`;
  }).join("");
  const doneRows = flashes.map((f) =>
    `<div class="bq-row bq-done"><span class="bq-ic tone-up">✓</span><div class="bq-main"><b>${esc(f.label)}</b> <span class="dim">complete</span></div></div>`).join("");
  return `<div class="deps-head" style="margin-top:8px">${icon("queue", "sm")} Under construction</div>` +
    `<div class="bq-list">${rows}${doneRows}</div>`;
}


// --- §modules Part B: the module catalog + client UI state -------------------
// The 5 modules in a fixed order (mirrors sim MODULE_KINDS); labels + a compact
// glyph for chips/ledger; and per-hull slot counts (mirrors ShipKind::module_slots).
export const MODULE_ALL: ModuleKind[] = ["mass_driver", "torpedo_rack", "point_defense_screen", "reflective_plating", "whipple_armor"];

export const MODULE_LABEL: Record<ModuleKind, string> = {
  mass_driver: "Mass Driver", torpedo_rack: "Torpedo Rack", point_defense_screen: "Point-Defense",
  reflective_plating: "Reflective Plating", whipple_armor: "Whipple Armor",
};

export const MODULE_ICON: Record<ModuleKind, IconKey> = {
  mass_driver: "moduleMassDriver",
  torpedo_rack: "moduleTorpedoRack",
  point_defense_screen: "modulePointDefense",
  reflective_plating: "moduleReflectivePlating",
  whipple_armor: "moduleWhippleArmor",
};

export const moduleIcon = (m: ModuleKind, size: IconSize = "sm") => icon(MODULE_ICON[m], size, MODULE_LABEL[m]);

// What each module DOES, one line (for button/chip titles).
export const MODULE_TIP: Record<ModuleKind, string> = {
  mass_driver: "Weapon: fires DRIVERS (harder hit) — countered by Whipple Armor.",
  torpedo_rack: "Weapon: fires TORPEDOES (hardest hit, ignores armor) — countered by Point-Defense.",
  point_defense_screen: "Weapon+defense: weak beam, but adds torpedo INTERCEPTION for the side.",
  reflective_plating: "Armor: blunts incoming BEAM into this ship.",
  whipple_armor: "Armor: blunts incoming DRIVER into this ship.",
};

// §fitting: per-module FITTING-POINT costs + per-hull budgets (mirrors sim
// ModuleKind::fitting_cost / ship::fitting_points) — the SECOND constraint
// besides slots; both render in the fitting bar and gate the queue button.
export const MODULE_FIT_COST: Record<ModuleKind, number> = {
  mass_driver: 2, torpedo_rack: 3, point_defense_screen: 2, reflective_plating: 2, whipple_armor: 3,
};

export const fitCost = (mods: ModuleKind[]): number => mods.reduce((s, m) => s + (MODULE_FIT_COST[m] ?? 0), 0);

// The hull's standing affinity note (shown in the build detail even unfitted).
export const HULL_AFFINITY_NOTE: Record<string, string> = {
  raider: "torpedo ×1.25", corvette: "interception ×1.25",
  destroyer: "beam ×1.20", cruiser: "protection ×1.20", battleship: "driver ×1.20 · siege anchor ×1.25",
  dreadnought: "interception ×1.30 (platform-grade screen)", titan: "all weapons ×1.10",
};

// §fitting: the FITTING BAR — used/total points with per-module cost chips;
// red when the composed fit would overflow the hull's budget.
export function fittingBar(kind: string, mods: ModuleKind[]): string {
  const total = FITTING_POINTS[kind] ?? 0;
  const used = fitCost(mods);
  const over = used > total;
  const chips = mods.map((m) => `<span class="fit-cost-chip" title="${esc(MODULE_LABEL[m])} costs ${MODULE_FIT_COST[m]} pts">${moduleIcon(m, "sm")}${MODULE_FIT_COST[m]}</span>`).join("");
  return `<span class="fitbar${over ? " is-over" : ""}" title="Fitting points — every module costs points against the hull's budget (the second constraint besides slots).">` +
    `fit <b>${used}/${total}</b> pts${chips ? ` ${chips}` : ""}${over ? ` <span class="fitbar-over">OVER BUDGET</span>` : ""}</span>`;
}

// Sol's module spread (mirrors sim MODULE_BUY_MULT / MODULE_SELL_MULT) — DISPLAY
// only; the server prices the real charge on execution (shown "~").
export const MODULE_BUY_MULT = 2.0, MODULE_SELL_MULT = 0.5;

// The FIT the player is composing for the next warship build (module slugs, ≤2).
export let pendingFit: ModuleKind[] = [];

// §modules Part B3: the module FORGE for a body with an Armaments Complex —
// the system ledger line + one manufacture button per module (BuildModule),
// costs/afford drawn from the shared build_options channel ("module:<slug>").
export function moduleForge(dyn: SystemStateView | undefined): string {
  const ledger = dyn?.modules ?? {};
  const onHand = MODULE_ALL.filter((m) => (ledger[m] ?? 0) > 0);
  const ledgerLine = `<div class="mhint module-ledger" style="margin-top:2px">${icon("manifest", "sm")} ledger: ${onHand.length ? onHand.map((m) => `<span>${moduleIcon(m, "sm")} ${esc(MODULE_LABEL[m])} ×${ledger[m]}</span>`).join(" · ") : "empty"}</div>`;
  const have = new Map((dyn?.stockpile ?? []).map((s) => [s.commodity, s.units]));
  const btns = MODULE_ALL.map((m) => {
    const o = buildOption(`module:${m}`);
    if (!o) return "";
    const afford = o.costs.every((c) => (have.get(c.commodity as Commodity) ?? 0) >= c.units);
    const cost = o.costs.map((c) => `${commodityIcon(c.commodity as Commodity, "sm")}${c.units}`).join(" ");
    return `<button class="act build-opt" data-build="module:${m}" ${afford ? "" : "disabled"} title="${esc(MODULE_TIP[m])} — costs draw from this system's stockpile.">` +
      `<span class="bo-name">${moduleIcon(m, "md")} ${esc(MODULE_LABEL[m])}</span><span class="bo-cost">${cost} · ${icon("time", "sm")}${o.build_secs}s</span></button>`;
  }).join("");
  return ledgerLine + `<div class="build-grid" style="margin-top:4px">${btns}</div>`;
}

// §modules Part B4: the FIT PICKER above a yard's warship builds — toggle chips
// for modules IN THE LEDGER (only what you have can be fitted); the composed fit
// (≤2) is clamped per-hull at dispatch. Empty ledger → no picker (nothing to fit).
export function fitPicker(dyn: SystemStateView | undefined): string {
  const ledger = dyn?.modules ?? {};
  const avail = MODULE_ALL.filter((m) => (ledger[m] ?? 0) > 0);
  if (!avail.length) return "";
  pendingFit = pendingFit.filter((m) => (ledger[m] ?? 0) > 0); // drop now-absent picks
  const chips = avail.map((m) => {
    const on = pendingFit.includes(m);
    return `<button class="act fit-chip${on ? " is-on" : ""}" data-fit="${m}" title="${esc(MODULE_TIP[m])}">${moduleIcon(m, "sm")} ${esc(MODULE_LABEL[m])}${on ? " ✓" : ""}</button>`;
  }).join("");
  const cur = pendingFit.length ? pendingFit.map((m) => moduleIcon(m, "sm")).join(" ") : "stock (unfitted)";
  return `<div class="mhint" style="margin:4px 0 2px" title="Pick up to 2 modules to fit the next warship built here; a ship takes as many as its hull has slots (Interceptor/Corvette 2, Scout 1).">${svgIcon("action-build", "sm")} fit next build: <b>${cur}</b></div>` +
    `<div class="fit-row">${chips}</div>`;
}


// §planetary-opportunities: the server scores these from the SAME capped
// natural multipliers as production. The client only names that surveyed fact;
// it never invents a recommendation from partial geology. Keep the shortlist
// compact: the point is the colony's identity, not another wall of numbers.
export function colonyOpportunityBlock(dyn: SystemStateView | undefined): string {
  const opportunities = dyn?.opportunities ?? [];
  if (!opportunities.length) return "";
  const rows = opportunities.slice(0, 4).map((o) => {
    const tone = o.tier === "jackpot" ? "positive" : o.tier === "exceptional" ? "accent" : "neutral";
    const place = o.body_name ? ` · ${esc(o.body_name)}` : "";
    return `<div class="sp-line colony-role" title="${esc(o.reason)}">` +
      `${icon(COLONY_ROLE_ICON[o.role], "md", o.title)}${badge(tone, o.tier)} <b>${esc(o.title)}</b> <span class="positive">×${o.score.toFixed(2)}</span>${place}</div>`;
  }).join("");
  return `<div class="deps-head" style="margin-top:8px" title="Roles revealed by the survey. Scores compare this natural site with the dependable home baseline; staffing, structures and research come later.">Colony opportunities</div>${rows}`;
}


// §body-management: the monolithic buildPanel is gone — its pool readout
// lives in the summary, its rows on the body panels (openPlanetPanel).

// ---- §build-panel: the dedicated structure builder ---------------------------
// A client-only UI over the SAME DevelopSystem command — the per-structure grid
// that used to live inline on the planet panel moved here wholesale, so the
// planet panel stays a lens on the body while you choose what to build. All the
// slot / afford / tier / deposit gating below is the ONE source of truth shared
// by the row list, the detail, and the Queue button.
// Structure → the closest registry icon (art only; mirrors systemview's family).
export const STRUCT_ICON: Record<string, IconKey> = {
  mining_complex: "extractor", volatile_harvester: "extractor", bioharvester: "extractor",
  smelter: "refinery", electronics_fabricator: "refinery", chemical_works: "refinery",
  fuel_refinery: "refinery", machine_works: "build", armaments_complex: "build", shipyard: "shipyard",
  agroplex: "habitat", habitat: "habitat", orbital_warehouse: "orbital_warehouse",
  sensor_array: "sensor", defense_platform: "defense", academy: "habitat",
};

// Producers scale output by the tier-throughput curve (mirrors sim TIER_THROUGHPUT).
export const TIER_THROUGHPUT = [0, 1.0, 2.2, 3.8, 6.0];

export const THROUGHPUT_STRUCTS = new Set(["mining_complex", "volatile_harvester", "bioharvester", "smelter", "electronics_fabricator", "chemical_works", "fuel_refinery", "machine_works", "armaments_complex", "agroplex", "academy"]);

// One-line "what it does" + "what it enables" per structure (client flavor, kept
// consistent with the sim's recipes — production.rs CONVERTERS / build.rs).
export const STRUCT_INFO: Record<string, { desc: string; effect: string }> = {
  mining_complex: { desc: "Mines the body's Metallic Ore, Silicates, or Rare-Element deposit.", effect: "Feeds raw ore into the system stockpile." },
  volatile_harvester: { desc: "Draws Volatiles from the body's gas/ice deposit.", effect: "Feeds Volatiles — fuel & polymer feedstock." },
  bioharvester: { desc: "Harvests Biomass from the body's living deposit.", effect: "Feeds Biomass — food & polymer feedstock." },
  smelter: { desc: "Smelts Metallic Ore (+Fuel) into Alloys.", effect: "Unlocks Alloys production." },
  electronics_fabricator: { desc: "Fabricates Electronics from Rare Elements + Silicates.", effect: "Unlocks Electronics production." },
  chemical_works: { desc: "Processes Volatiles + Biomass into Polymers.", effect: "Unlocks Polymers production." },
  fuel_refinery: { desc: "Refines Volatiles into Fuel.", effect: "Unlocks Fuel — powers movement + smelting." },
  machine_works: { desc: "Builds Machinery from Alloys + Electronics + Fuel.", effect: "Unlocks Machinery — the build-cost backbone." },
  armaments_complex: { desc: "Assembles Armaments from Alloys + Electronics + Polymers.", effect: "Unlocks Armaments + on-site module manufacture." },
  shipyard: { desc: "An orbital yard that lays down light hulls here.", effect: "Builds Freighter/Scout/Colony (I) and Interceptor/Corvette (II). Its tier is its slipway count." },
  naval_drydock: { desc: "A heavy drydock for ships of the line. Needs a Shipyard II here.", effect: "Builds Destroyer (I), Cruiser (II), Battleship (III). Its own slipways." },
  capital_slipway: { desc: "A super-capital slipway — the deepest yard. Needs a Naval Drydock III here.", effect: "Builds Dreadnought (I) and Titan (II). A season's investment on capturable ground." },
  ordnance_foundry: { desc: "An outfitting yard — it changes what a hull carries rather than laying new ones.", effect: "Installs refits here (a forward foundry refits without a construction yard)." },
  agroplex: { desc: "Grows Provisions from Biomass.", effect: "Feeds the colony — keeps it Well Supplied." },
  garrison: { desc: "Barracks, armories and a drop-troop depot. Standing troops defend this ground; they also give you the hull to take someone else's.", effect: "Stiffens defense (a besieger's clock runs slower here) and holds off landings \u2014 25 marines per tier. Builds Troop Transports. Eats Provisions: an unfed garrison stops counting." },
  habitat: { desc: "Housing that lifts this body's population ceiling.", effect: "+population cap & workforce; boosts output when fed." },
  orbital_warehouse: { desc: "An orbital warehouse that raises storage capacity.", effect: "+400 storage cap per tier." },
  sensor_array: { desc: "A standing sensor array over the system.", effect: "Extends detection range around this system — see rivals sooner, and read what they are carrying." },
  defense_platform: { desc: "Static defenses that fight raiders at the system.", effect: "+1 defense tier vs. attackers (can be worn down)." },
  academy: { desc: "Trains specialists and powers corporate research.", effect: "Enables specialist training + a research contribution." },
};


export const romanTier = (n: number): string => ROMAN[n] ?? String(n);


// Build-panel state: which body it targets + the currently-selected structure.
export let buildPanelBuilt = false;

export let buildTargetBodyId: string | null = null;
 // the body BOTH builders target (shared)
export let buildSelectedKey: string | null = null;
 // struct builder selection
export let shipSelectedKind: string | null = null;
 // ship builder selection
export let shipQty = 1;
 // ship builder quantity

// §build-panel: the shared SHELL for both builders (structures + ships) — same
// chrome, dock, and dimensions (`.build-shell` in the CSS). The two panels are
// siblings, never open together (opening one closes the other via closeBuildPanel).
export function panelShellHtml(el: HTMLElement, eyebrow: string, title: string, chips: string, listHtml: string, detailHtml: string, footHtml: string): void {
  setHtml(el,
    `<div class="bp-head"><div class="panel-title"><div><div class="eyebrow">${esc(eyebrow)}</div><h2>${esc(title)}</h2></div></div>` +
    `<button class="pp-close" data-bp="close" title="Close" aria-label="Close">✕</button></div>` +
    `<div class="bp-pools">${chips}</div>` +
    `<div class="bp-body"><div class="bp-list">${listHtml}</div><div class="bp-detail">${detailHtml}</div></div>` +
    footHtml);
  el.classList.add("is-open");
}

export function buildBuildPanel(): void {
  if (buildPanelBuilt) return;
  buildPanelBuilt = true;
  $("build-panel").addEventListener("click", (e) => {
    const t = e.target as HTMLElement;
    if (t.closest("[data-bp='close']")) { closeBuildPanel(); return; }
    const row = t.closest("[data-bp-row]") as HTMLElement | null;
    if (row) { buildSelectedKey = row.dataset.bpRow ?? null; renderBuildPanel(); return; }
    if (t.closest("[data-bp='queue']")) { queueSelectedBuild(); return; }
  });
  $("build-ship-panel").addEventListener("click", (e) => {
    const t = e.target as HTMLElement;
    if (t.closest("[data-bp='close']")) { closeBuildPanel(); return; }
    const qbtn = t.closest("[data-bp-qty]") as HTMLElement | null;
    if (qbtn) { shipQty = Math.max(1, Number(qbtn.dataset.bpQty) || 1); renderShipPanel(); return; }
    const row = t.closest("[data-bp-row]") as HTMLElement | null;
    if (row) { shipSelectedKind = row.dataset.bpRow ?? null; renderShipPanel(); return; }
    // §fitting: the detail pane's fit chips + doctrine-fit library.
    const fitBtn = t.closest("[data-fit]") as HTMLElement | null;
    if (fitBtn) {
      const m = fitBtn.dataset.fit as ModuleKind;
      const cap = MODULE_SLOTS[shipSelectedKind ?? ""] ?? 2;
      if (pendingFit.includes(m)) pendingFit = pendingFit.filter((x) => x !== m);
      else if (pendingFit.length < cap) pendingFit.push(m);
      renderShipPanel();
      return;
    }
    const pick = t.closest("[data-fitpick]") as HTMLElement | null;
    if (pick) {
      const f = (state.syndicate?.fits ?? []).find((x) => x.name === pick.dataset.fitpick);
      if (f) { pendingFit = [...f.modules]; shipSelectedKind = f.kind; renderShipPanel(); }
      return;
    }
    const del = t.closest("[data-fitdel]") as HTMLElement | null;
    if (del && net) { net.send({ type: "DeleteFit", name: del.dataset.fitdel ?? "" }); return; }
    if (t.closest("[data-fitsave]") && net && shipSelectedKind) {
      const sid = viewedSystemId();
      const ledger = sid ? moduleLedgerAt(sid) : {};
      const eff = pendingFit.filter((m) => (ledger[m] ?? 0) > 0).slice(0, MODULE_SLOTS[shipSelectedKind] ?? 0);
      if (!eff.length || !fitLegal(shipSelectedKind, eff)) return;
      const name = (window.prompt("Fit name (≤24 chars):") ?? "").trim();
      if (name) net.send({ type: "SaveFit", name, ship: shipSelectedKind as ShipKind, loadout: eff });
      return;
    }
    if (t.closest("[data-bp='queue']")) { queueSelectedShips(); return; }
  });
}

export function openBuildPanel(bodyId: string): void {
  buildBuildPanel();
  // Toggle: the same body's builder re-clicked closes it (the button is a switch).
  const toggleOff = buildTargetBodyId === bodyId && $("build-panel").classList.contains("is-open");
  closeBuildPanel(); // also closes the ship builder — the two never coexist
  if (toggleOff) return;
  buildTargetBodyId = bodyId;
  renderBuildPanel();
}

export function openShipPanel(bodyId: string): void {
  buildBuildPanel();
  const toggleOff = buildTargetBodyId === bodyId && $("build-ship-panel").classList.contains("is-open");
  closeBuildPanel(); // also closes the structure builder
  if (toggleOff) return;
  buildTargetBodyId = bodyId;
  shipQty = 1;
  renderShipPanel();
}

export function closeBuildPanel(): void {
  buildTargetBodyId = null;
  buildSelectedKey = null;
  shipSelectedKind = null;
  shipQty = 1;
  $("build-panel").classList.remove("is-open");
  $("build-ship-panel").classList.remove("is-open");
}

export function refreshBuildPanel(): void {
  if (!buildTargetBodyId) return;
  if ($("build-panel").classList.contains("is-open")) {
    if (renderDeferred("build-panel", refreshBuildPanel)) return; // §single-click
    renderBuildPanel();
  } else if ($("build-ship-panel").classList.contains("is-open")) {
    if (renderDeferred("build-ship-panel", refreshBuildPanel)) return;
    renderShipPanel();
  }
}

export function queueSelectedBuild(): void {
  const sid = viewedSystemId();
  if (!sid || !net || !buildTargetBodyId || !buildSelectedKey) return;
  const dyn = state.systems.find((s) => s.id === sid);
  const body = dyn?.bodies?.find((b) => String(b.id) === buildTargetBodyId);
  const o = body ? buildOption(buildSelectedKey) : undefined;
  if (!dyn || !body || !o) return;
  const st = structOption(o, dyn, body, bodyPoolUsage(body, dyn));
  if (!st.buildable) return; // the button is disabled, but never trust the DOM
  // §byte-identical: exactly the DevelopSystem the inline buttons sent — the body
  // panel names its body, the sim soft-rejects on arrival as always.
  dispatchBuildKey(buildSelectedKey, sid, Number(buildTargetBodyId));
  readout().innerHTML =
    `Queued <b>${esc(o.label)}${st.tierUp ? ` ×${st.targetTier}` : ""}</b> on ${esc(body.name)} — ` +
    `it appears under construction. <span class="dim">A soft-reject (no slot / short on goods) shows in the Log.</span>`;
  buildSelectedKey = null; // clear so several can be queued back-to-back
  renderBuildPanel();
  refreshOpenBodyPanel(); // the queue row lands on the next View push
  updateSysviewManage();
}

export function buildRowHtml(st: StructOpt): string {
  const sel = st.o.key === buildSelectedKey ? " is-sel" : "";
  const off = st.buildable ? "" : " is-off";
  const badge = st.foundsNew ? `<span class="bp-row-tier">new</span>` : `<span class="bp-row-tier">▲ ×${st.targetTier}</span>`;
  const short = st.noDeposit ? "no deposit" : st.poolFull ? "pool full" : !st.afford ? "short on goods" : "";
  const reason = short ? `<span class="bp-row-reason">${short}</span>` : "";
  return `<button class="bp-row${sel}${off}" data-bp-row="${st.o.key}" title="${esc(st.buildable ? st.o.label : st.reason)}">` +
    `<span class="bp-row-ic">${icon(STRUCT_ICON[st.o.key] ?? "build", "sm")}</span>` +
    `<span class="bp-row-name">${esc(st.o.label)}</span>${badge}${reason}</button>`;
}

export function buildDetailHtml(o: BuildOpt, dyn: SystemStateView, body: BodyView, pools: PoolUse): string {
  const st = structOption(o, dyn, body, pools);
  const info = STRUCT_INFO[o.key] ?? { desc: "", effect: "" };
  const supply = constructionStock(dyn);
  const costRows = o.costs.map((c) => {
    const commodity = c.commodity as Commodity;
    const has = supply.available.get(commodity) ?? 0;
    const stockpiled = supply.stockpile.get(commodity) ?? 0;
    const freight = supply.freighters.get(commodity) ?? 0;
    const shortC = has < c.units;
    return `<div class="bp-cost-row${shortC ? " is-short" : ""}">` +
      `<span class="bp-cost-c">${commodityIcon(commodity, "sm")} ${esc(label(c.commodity))}</span>` +
      `<span class="bp-cost-n">${c.units} <span class="bp-cost-have">have ${constructionStockTotal(stockpiled, freight)}</span></span></div>`;
  }).join("");
  // The sensor bubble is not drawn on the map — a single ring
  // claimed a certainty detection never had (`bubble × signature` means a quiet
  // raider is caught at 0.4× it and a loud fleet well outside it). So the Sensor
  // Array REPORTS its reach as a number instead, here, where the decision to
  // build it is actually made.
  const sensorLine = (() => {
    if (o.key !== "sensor_array") return "";
    const g = state.galaxy;
    if (!g?.sensor_array_base) return "";
    const reach = (t: number) => Math.round(g.sensor_array_base + g.sensor_array_per_tier * Math.max(0, t - 1));
    const now = st.targetTier > 1 ? reach(st.targetTier - 1) : 0;
    const next = reach(st.targetTier);
    return `<div class="bp-note">Detection reach <b>${fmt(next)} su</b>` +
      (now > 0 ? ` <span class="dim">(from ${fmt(now)})</span>` : "") +
      ` <span class="dim">— against a reference contact; a quiet hull is seen closer, a fast or large one farther.</span></div>`;
  })();

  const slotLine = st.foundsNew
    ? `Claims a <b>${POOL_LABEL[st.pool]}</b> slot — ${pools[st.pool].used} → ${pools[st.pool].used + 1} / ${pools[st.pool].total}.`
    : `Deepens in place — no new slot consumed.`;
  const from = Math.min(4, st.targetTier - 1), to = Math.min(4, st.targetTier);
  let framing: string;
  if (st.tierUp) {
    let delta = "";
    if (THROUGHPUT_STRUCTS.has(o.key) && TIER_THROUGHPUT[from] && TIER_THROUGHPUT[to]) {
      const pct = Math.round((TIER_THROUGHPUT[to] / TIER_THROUGHPUT[from] - 1) * 100);
      delta = ` Throughput ×${TIER_THROUGHPUT[from]} → ×${TIER_THROUGHPUT[to]} <span class="tone-up">(+${pct}%)</span>.`;
    }
    framing = `<div class="bp-upgrade"><b>Upgrade</b> — Tier ${romanTier(st.targetTier - 1)} → ${romanTier(st.targetTier)}.${delta}</div>`;
  } else {
    framing = `<div class="bp-upgrade"><b>New structure</b> — founds Tier I.</div>`;
  }
  return `<div class="bp-d-head">${icon(STRUCT_ICON[o.key] ?? "build", "md")} <b>${esc(o.label)}</b>` +
    `<span class="bp-d-tier">${st.foundsNew ? "new" : `→ ×${st.targetTier}`}</span></div>` +
    `<div class="bp-d-desc">${esc(info.desc)}</div>${framing}` +
    `<div class="bp-d-sec">Recipe — required vs. local supply</div><div class="bp-costs">${costRows}</div>` +
    (st.afford ? "" : `<div class="bp-d-warn">Short on goods — it waits (or soft-rejects) until local supply covers it.</div>`) +
    (st.noDeposit ? `<div class="bp-d-warn">No matching deposit on this body — found it on a body that has one.</div>` : "") +
    `<div class="bp-d-sec">Build time</div><div class="bp-d-line">${icon("time", "sm")} ${fmtBuildDur(o.build_secs * body.construction_time_mult)} on this ${esc(label(body.environment))} world` +
    (Math.abs(body.construction_time_mult - 1) > 0.001 ? ` <span class="dim">(planet ×${body.construction_time_mult.toFixed(2)}; base ${fmtBuildDur(o.build_secs)})</span>` : "") + `.</div>` +
    `<div class="bp-d-sec">Slot</div><div class="bp-d-line">${slotLine}</div>` +
    sensorLine +
    `<div class="bp-d-sec">Enables</div><div class="bp-d-line">${esc(info.effect)}</div>`;
}

export function renderBuildPanel(): void {
  const el = $("build-panel");
  if (!buildTargetBodyId) { el.classList.remove("is-open"); return; }
  const sid = viewedSystemId();
  const dyn = sid ? state.systems.find((s) => s.id === sid) : undefined;
  const body = dyn?.bodies?.find((b) => String(b.id) === buildTargetBodyId);
  if (!dyn || !body) { closeBuildPanel(); return; }
  const pools = bodyPoolUsage(body, dyn);
  const poolChips = (["resource", "industrial", "infrastructure"] as Pool[]).map((k) => {
    const p = pools[k];
    return `<span class="bp-pool${p.used >= p.total ? " is-full" : ""}" title="${POOL_LABEL[k]} slots used / total on this body">${POOL_LABEL[k]} ${p.used}/${p.total}</span>`;
  }).join("");
  // LEFT: every non-ship structure buildable here, grouped by slot pool. Rows that
  // fail a gate still render, greyed, with the reason inline.
  const opts = ((state.galaxy?.build_options ?? []) as BuildOpt[]).filter((o) => !SHIP_KEYS.has(o.key) && !!POOL_OF[o.key]);
  const byPool: Record<Pool, StructOpt[]> = { resource: [], industrial: [], infrastructure: [] };
  for (const o of opts) byPool[POOL_OF[o.key]].push(structOption(o, dyn, body, pools));
  const groups = (["resource", "industrial", "infrastructure"] as Pool[]).map((k) => {
    if (!byPool[k].length) return "";
    return `<div class="bp-group">${POOL_LABEL[k]} <span class="bp-group-n">${pools[k].used}/${pools[k].total}</span></div>` +
      byPool[k].map(buildRowHtml).join("");
  }).join("");
  const selOpt = buildSelectedKey ? buildOption(buildSelectedKey) : undefined;
  const detail = selOpt
    ? buildDetailHtml(selOpt as BuildOpt, dyn, body, pools)
    : `<div class="bp-detail-empty">Select a structure to see its recipe, build time, slot, and effect.</div>`;
  // FOOTER: Queue + a live note of what's already queued on THIS body.
  const selSt = selOpt ? structOption(selOpt as BuildOpt, dyn, body, pools) : null;
  const canQueue = !!selSt && selSt.buildable;
  const qTip = !selSt ? "Select a structure first." : selSt.buildable ? "Queue this build — draws from local supply." : selSt.reason;
  const queued = (dyn.builds ?? []).filter((j) => j.body_id === body.id && !SHIP_KEYS.has(j.key));
  const queuedNote = queued.length
    ? `Already queued here: <b>${queued.map((j) => esc(buildLabel(j.key))).join(", ")}</b>.`
    : "Nothing queued on this body yet.";
  const foot = `<div class="bp-foot"><button class="act bp-queue" data-bp="queue" ${canQueue ? "" : "disabled"} title="${esc(qTip)}">${icon("build", "sm")} Queue build</button>` +
    `<div class="bp-queued">${queuedNote}</div></div>`;
  panelShellHtml(el, "build", `Build on ${body.name}`, poolChips, groups, detail, foot);
}


// ---- §build-ship-panel: the dedicated SHIP builder (mirrors the structure
// builder; shares its shell/dock/breakpoint via `.build-shell`). Opened from the
// shipyard body's "Build ship" button; a sibling of the structure builder — the
// two never render together. Client-only over the SAME BuildShip command. ----
export const SHIP_ORDER = ["scout", "corvette", "raider", "convoy", "colony", "destroyer", "cruiser", "battleship", "dreadnought", "titan"];

export const SHIP_HULL_ICON: Record<string, IconKey> = {
  convoy: "convoy", raider: "raider", corvette: "corvette", colony: "colony", scout: "scout",
  // §ladder: no dedicated icons yet — the fleet mark stands in.
  destroyer: "fleet", cruiser: "fleet", battleship: "fleet", dreadnought: "fleet", titan: "fleet",
};

export function shipRowHtml(st: ShipOpt): string {
  const sel = st.o.key === shipSelectedKind ? " is-sel" : "";
  const off = st.buildable ? "" : " is-off";
  const info = SHIP_STATS[st.o.key];
  const gate = SHIP_YARD[st.o.key] ?? { yard: "shipyard", tier: 1 };
  const short = st.foundingLocked ? "founding programme"
    : !hullResearched(st.o.key) ? "needs research"
    : st.yardShort ? `needs ${(YARD_TITLE[gate.yard] ?? gate.yard).toLowerCase()} ${romanTier(st.needTier)}`
    : st.slipsFull ? "slipways full"
    : !st.afford ? "short on goods" : "";
  const reason = short ? `<span class="bp-row-reason">${short}</span>` : "";
  return `<button class="bp-row bp-ship-row${sel}${off}" data-bp-row="${st.o.key}" title="${esc(info?.role ?? st.o.label)}">` +
    `<span class="bp-row-ic">${icon(SHIP_HULL_ICON[st.o.key] ?? "fleet", "sm")}</span>` +
    `<span class="bp-ship-main"><span class="bp-ship-top"><span class="bp-row-name">${esc(st.o.label)}</span>` +
    `<span class="bp-row-tier">${fmtBuildDur(st.o.build_secs)}</span>${reason}</span>` +
    `<span class="bp-ship-role">${esc(info?.role ?? "")}</span></span></button>`;
}

export function shipDetailHtml(o: BuildOpt, dyn: SystemStateView, body: BodyView): string {
  const st = shipOption(o, dyn);
  const info = SHIP_STATS[o.key];
  const q = Math.max(1, shipQty);
  const supply = constructionStock(dyn);
  const have = supply.available;
  // QUANTITY stepper — 1 / 5 / 10 / max-affordable; the total cost + time track it.
  const qbtn = (n: number, lbl: string) => `<button class="bp-qty-btn${q === n ? " is-on" : ""}" data-bp-qty="${n}" ${n < 1 ? "disabled" : ""}>${lbl}</button>`;
  const stepper = `<div class="bp-qty"><span class="bp-qty-lbl">Quantity</span>${qbtn(1, "1")}${qbtn(5, "5")}${qbtn(10, "10")}${qbtn(st.maxAff, `Max ${st.maxAff}`)}<span class="bp-qty-cur">building <b>${q}</b></span></div>`;
  // Cost table — the TOTAL (unit × q) reads largest; unit shown small alongside.
  const costRows = o.costs.map((c) => {
    const commodity = c.commodity as Commodity;
    const need = c.units * q;
    const has = have.get(commodity) ?? 0;
    const stockpiled = supply.stockpile.get(commodity) ?? 0;
    const freight = supply.freighters.get(commodity) ?? 0;
    const short = has < need;
    return `<div class="bp-cost-row${short ? " is-short" : ""}">` +
      `<span class="bp-cost-c">${commodityIcon(commodity, "sm")} ${esc(label(c.commodity))}</span>` +
      `<span class="bp-cost-n"><b class="bp-cost-total">${need}</b>${q > 1 ? ` <span class="bp-cost-mul">${c.units}×${q}</span>` : ""} <span class="bp-cost-have">have ${constructionStockTotal(stockpiled, freight)}</span></span></div>`;
  }).join("");
  const affordsQ = q <= st.maxAff;
  // Build time — per-ship at this yard's current throughput (staffed-yard bonus
  // shown; the shown-math law). N hulls build in PARALLEL, so the batch time == 1.
  const boost = shipyardBoost(dyn, body);
  const siteTime = body.ship_build_time_mult ?? 1;
  const per = Math.max(1, Math.round(o.build_secs * siteTime / boost));
  const timeLine = boost > 1.001 || Math.abs(siteTime - 1) > 0.001
    ? `${icon("time", "sm")} <b>${fmtBuildDur(per)}</b> each — ${siteTime < 0.999 ? `low-gravity ×${siteTime.toFixed(2)} · ` : ""}staffed-yard ×${boost.toFixed(2)} (base ${fmtBuildDur(o.build_secs)}).${q > 1 ? ` The ${q} build in parallel.` : ""}`
    : `${icon("time", "sm")} <b>${fmtBuildDur(per)}</b> each${q > 1 ? ` · the ${q} build in parallel` : ""}. <span class="dim">Assign workers to the Shipyard to build faster.</span>`;
  const stat = (lbl: string, val: string) => `<div class="bp-stat"><span class="bp-stat-l">${lbl}</span><span class="bp-stat-v">${val}</span></div>`;
  const stats = info ? `<div class="bp-stats">${stat("Speed", `${info.speed}`)}${stat("Hull mass", `${info.hull}`)}${stat("Attack", `${info.atk}`)}${stat("Defense", `${info.def}`)}${stat("Module slots", `${info.slots}`)}${stat("Fit points", `${FITTING_POINTS[o.key] ?? 0}`)}</div>` : "";
  // §fitting: the FITTING section — chips (ledger-gated), the used/total bar
  // with per-module costs (red on overflow), the hull's affinity line, and the
  // syndicate's saved DOCTRINE FITS for this hull (pick / save / delete).
  let fitting = "";
  const slots = MODULE_SLOTS[o.key] ?? 0;
  if (slots > 0) {
    const ledger = moduleLedgerAt(dyn.id);
    const avail = MODULE_ALL.filter((m) => (ledger[m] ?? 0) > 0);
    const chips = avail.map((m) => {
      const on = pendingFit.includes(m);
      return `<button class="act fit-chip${on ? " is-on" : ""}" data-fit="${m}" title="${esc(MODULE_TIP[m])} Costs ${MODULE_FIT_COST[m]} fitting pts.">${moduleIcon(m, "sm")} ${esc(MODULE_LABEL[m])}${on ? " ✓" : ""}</button>`;
    }).join("");
    const eff = pendingFit.filter((m) => (ledger[m] ?? 0) > 0).slice(0, slots);
    const aff = affinityLine(o.key, eff);
    const affNote = HULL_AFFINITY_NOTE[o.key]
      ? `<div class="bp-d-line" title="Hull affinity — a named factor the sim applies to that module family on this hull.">${icon("intel", "sm")} hull affinity: <b>${esc(HULL_AFFINITY_NOTE[o.key])}</b>${aff ? ` — <span class="tone-up">${esc(aff)} active</span>` : ""}</div>`
      : "";
    // Saved doctrine fits for THIS hull (syndicate-wide; owner-only view).
    const fits = (state.syndicate?.fits ?? []).filter((f) => f.kind === o.key);
    const fitChips = fits.map((f) =>
      `<span class="fit-saved"><button class="act fit-chip" data-fitpick="${esc(f.name)}" title="Apply this doctrine fit: ${f.modules.map((m) => esc(MODULE_LABEL[m])).join(" + ") || "stock"}">${esc(f.name)}</button>` +
      `<button class="act fit-del" data-fitdel="${esc(f.name)}" title="Delete this fit from the syndicate library">✕</button></span>`).join(" ");
    const canSave = eff.length > 0 && fitLegal(o.key, eff);
    const fitLib = state.syndicate
      ? `<div class="bp-d-line" style="margin-top:2px">${fitChips || `<span class="dim">no saved fits for this hull</span>`} ` +
        `<button class="act fit-chip" data-fitsave="1" ${canSave ? "" : "disabled"} title="${canSave ? "Save the composed fit as a named syndicate doctrine fit" : "Compose a legal, non-empty fit first"}">💾 save fit…</button></div>`
      : `<div class="bp-d-line dim">Join a syndicate to share doctrine fits.</div>`;
    fitting = `<div class="bp-d-sec">Fitting — next build${eff.length ? "" : " (stock)"}</div>` +
      `<div class="bp-d-line">${fittingBar(o.key, pendingFit.slice(0, Math.max(slots, pendingFit.length)))} <span class="dim">· ${slots} slot${slots > 1 ? "s" : ""}</span></div>` +
      (avail.length ? `<div class="fit-row">${chips}</div>` : `<div class="bp-d-line dim">No modules in this system's ledger — manufacture some at an Armaments Complex.</div>`) +
      affNote + fitLib;
  }
  return `<div class="bp-d-head">${icon(SHIP_HULL_ICON[o.key] ?? "fleet", "md")} <b>${esc(o.label)}</b><span class="bp-d-tier">${st.yardShort ? `needs yard ${romanTier(st.needTier)}` : `${info?.slots ?? 0} slots`}</span></div>` +
    `<div class="bp-d-desc">${esc(info?.role ?? "")}</div>` +
    stepper +
    `<div class="bp-d-sec">Recipe — total for ${q}, vs. local supply</div><div class="bp-costs">${costRows}</div>` +
    (st.foundingLocked ? `<div class="bp-d-warn">Complete the Founding Programme to unlock Colony Ships and expansion.</div>` : "") +
    (st.yardShort ? `<div class="bp-d-warn">Requires Shipyard tier ${romanTier(st.needTier)} here — this system's yard is tier ${romanTier(st.yardTier)}.</div>` : "") +
    (!affordsQ ? `<div class="bp-d-warn">Local supply covers ${st.maxAff} right now — queue that many, or wait for production.</div>` : "") +
    `<div class="bp-d-sec">Build time</div><div class="bp-d-line">${timeLine}</div>` +
    `<div class="bp-d-sec">Stats</div>${stats}<div class="bp-d-line" style="margin-top:4px">${esc(info?.cap ?? "")}</div>` +
    fitting;
}

export function queueSelectedShips(): void {
  const sid = viewedSystemId();
  if (!sid || !net || !buildTargetBodyId || !shipSelectedKind) return;
  const dyn = state.systems.find((s) => s.id === sid);
  const body = dyn?.bodies?.find((b) => String(b.id) === buildTargetBodyId);
  const o = body ? buildOption(shipSelectedKind) : undefined;
  if (!dyn || !body || !o) return;
  const st = shipOption(o, dyn);
  const q = Math.min(Math.max(1, shipQty), st.maxAff);
  if (!st.buildable || q < 1) return; // the button is disabled, but never trust the DOM
  // §fitting: never dispatch an over-budget fit (the sim would soft-reject each).
  const ledger = moduleLedgerAt(sid);
  const eff = pendingFit.filter((m) => (ledger[m] ?? 0) > 0).slice(0, MODULE_SLOTS[shipSelectedKind] ?? 0);
  if (eff.length && !fitLegal(shipSelectedKind, eff)) return;
  // §byte-identical: N × the exact BuildShip the inline row sent (the loadout comes
  // from the yard's fit picker via dispatchBuildKey, clamped per hull as before).
  for (let i = 0; i < q; i++) dispatchBuildKey(shipSelectedKind, sid);
  readout().innerHTML =
    `Queued <b>${q}× ${esc(o.label)}</b> at ${esc(body.name)} — building at the orbital yard. ` +
    `<span class="dim">Spawns here; a fuel-short or over-queued build shows in the Log.</span>`;
  shipQty = 1; // reset for the next batch (mix a Corvette + two Convoys without reopening)
  renderShipPanel();
  refreshOpenBodyPanel();
  updateSysviewManage();
}

export function renderShipPanel(): void {
  const el = $("build-ship-panel");
  if (!buildTargetBodyId) { el.classList.remove("is-open"); return; }
  const sid = viewedSystemId();
  const dyn = sid ? state.systems.find((s) => s.id === sid) : undefined;
  const body = dyn?.bodies?.find((b) => String(b.id) === buildTargetBodyId);
  if (!dyn || !body) { closeBuildPanel(); return; }
  const yardTier = dyn.shipyard_tier ?? 0;
  const chip = `<span class="bp-pool">${icon("shipyard", "sm")} Shipyard ${romanTier(yardTier)}</span>`;
  const opts = ((state.galaxy?.build_options ?? []) as BuildOpt[]).filter((o) => SHIP_KEYS.has(o.key));
  const ordered = SHIP_ORDER.map((k) => opts.find((o) => o.key === k)).filter((o): o is BuildOpt => !!o);
  const list = ordered.map((o) => shipRowHtml(shipOption(o, dyn))).join("");
  const selOpt = shipSelectedKind ? buildOption(shipSelectedKind) : undefined;
  const detail = selOpt
    ? shipDetailHtml(selOpt as BuildOpt, dyn, body)
    : `<div class="bp-detail-empty">Select a hull to see its recipe, stats, and build time. Set a quantity, then queue the batch.</div>`;
  const selSt = selOpt ? shipOption(selOpt as BuildOpt, dyn) : null;
  const q = Math.max(1, shipQty);
  // §fitting: the queue button also gates on the composed fit's LEGALITY —
  // what dispatch will actually send (ledger-filtered, slot-clamped).
  const selKey = shipSelectedKind ?? "";
  const ledger = sid ? moduleLedgerAt(sid) : {};
  const effFit = pendingFit.filter((m) => (ledger[m] ?? 0) > 0).slice(0, MODULE_SLOTS[selKey] ?? 0);
  const fitOk = !effFit.length || fitLegal(selKey, effFit);
  const canQueue = !!selSt && selSt.buildable && q >= 1 && q <= selSt.maxAff && fitOk;
  const qTip = !selSt ? "Select a hull first." : !selSt.buildable ? selSt.reason : q > selSt.maxAff ? `Local supply covers ${selSt.maxAff} right now.` : !fitOk ? "The composed fit exceeds this hull's fitting budget — drop a module." : "Queue this batch — draws from local supply.";
  // The yard's line: every ship job in the SYSTEM (ships build at the best yard).
  const queued = (dyn.builds ?? []).filter((j) => SHIP_KEYS.has(j.key));
  const queuedNote = queued.length ? `At the yard: <b>${queued.map((j) => esc(buildLabel(j.key))).join(", ")}</b>.` : "Nothing at the yard yet.";
  const foot = `<div class="bp-foot"><button class="act bp-queue" data-bp="queue" ${canQueue ? "" : "disabled"} title="${esc(qTip)}">${icon("build", "sm")} Queue build${selSt && q > 1 ? ` ×${q}` : ""}</button>` +
    `<div class="bp-queued">${queuedNote}</div></div>`;
  panelShellHtml(el, "build ship", `Build at ${body.name}`, chip, list, detail, foot);
}

