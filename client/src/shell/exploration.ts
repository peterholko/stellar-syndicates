import type { CoreContext } from "./types";
import type { ExpeditionTask, ExplorationSiteView, ExplorationJournalEntry } from "../protocol";
import type { ViewState } from "../state";
import { label } from "../icons";
import { shipKindLabel } from "../core/derive/fleet";
import { deepExpeditionReason, expeditionCapable, explorationOrder, siteArt, siteStatus, siteTitle } from "../core/derive/exploration";
import { MODULES, isBlueprintOnly } from "../core/derive/equipment";
import "../styles/exploration.css";

const esc = (s: string): string => s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);
const attrs = (act: string, site?: string, fleet?: string) => `data-deck-act="exploration" data-mobile-act="exploration" data-explore-act="${act}"${site ? ` data-site="${esc(site)}"` : ""}${fleet ? ` data-fleet="${esc(fleet)}"` : ""}`;
const art = (site: ExplorationSiteView) => siteArt(site)
  ? `<img src="${siteArt(site)}" alt="${esc(site.details!.kind)}" width="128" height="128">`
  : `<span class="exploration-unknown" aria-label="Unidentified contact">◇</span>`;

type JournalUi = { owner: string | null; tab: string; filter: string; system: string | null; pending: Map<string, ExplorationJournalEntry> };
const views = new WeakMap<ViewState, JournalUi>();
function ui(st: ViewState): JournalUi {
  let value = views.get(st);
  if (!value || value.owner !== st.playerId) {
    value = { owner: st.playerId, tab: "contacts", filter: "all", system: null, pending: new Map() }; views.set(st, value);
  }
  for (const [id, entry] of value.pending) {
    const saved = st.explorationJournal?.find(e => e.id === id);
    if (saved?.note === entry.note && saved.pinned === entry.pinned || !saved && !entry.note && !entry.pinned) value.pending.delete(id);
  }
  return value;
}
function entryFor(st: ViewState, id: string, kind: ExplorationJournalEntry["kind"]): ExplorationJournalEntry {
  return ui(st).pending.get(id) ?? st.explorationJournal?.find(e => e.id === id) ?? { id, kind, note: "", pinned: false };
}
function journalFull(st: ViewState, id: string): boolean {
  const entries = new Map((st.explorationJournal ?? []).map(e => [e.id, e]));
  for (const [key, value] of ui(st).pending) {
    if (value.pinned || value.note) entries.set(key, value); else entries.delete(key);
  }
  return !entries.has(id) && entries.size >= 256; // mirrors JOURNAL_LIMIT
}
function noteHtml(st: ViewState, id: string, kind: ExplorationJournalEntry["kind"]): string {
  const entry = entryFor(st, id, kind);
  const full = journalFull(st, id);
  return `<section class="exploration-note" id="explore-note-${kind}-${esc(id)}"><header><h4>Private notes</h4>
    <button type="button" ${attrs("pin", id)} data-kind="${kind}" aria-pressed="${entry.pinned}" ${full ? "disabled" : ""}>${entry.pinned ? "★ Pinned" : "☆ Pin on map"}</button></header>
    <textarea aria-label="Private exploration notes" maxlength="400" rows="3" placeholder="Promising worlds, preparations, or a reason to return…">${esc(entry.note)}</textarea>
    <div class="exploration-actions"><small>${full ? "Journal full · clear a saved entry first" : ui(st).pending.has(id) ? "Saving…" : "Private to your corporation"}</small><button type="button" ${attrs("note", id)} data-kind="${kind}" ${full ? "disabled" : ""}>Save note</button></div></section>`;
}
const age = (st: ViewState, t: number) => `${Math.max(0, Math.floor(st.simTime - t))}s ago`;
export const explorationFocused = (st: ViewState): boolean => !!st.selectedExplorationSiteId || ui(st).tab !== "contacts";
const goods = (items: Record<string, number | undefined>) => Object.entries(items).filter(([, n]) => n! > 0).map(([kind, n]) => `${n} ${label(kind)}`).join(" · ");
function navigation(st: ViewState): string {
  return `<nav class="exploration-tabs" aria-label="Exploration sections">${["contacts", "journal", "blueprints"].map(tab =>
    `<button type="button" ${attrs(`tab-${tab}`)} aria-pressed="${ui(st).tab === tab}">${label(tab)}</button>`).join("")}</nav>`;
}

/** Shared shells, received reports only. No clock can reveal a type, empty a
 * site or claim an outpost. Fleet buttons keep identity stable across Views. */
export function explorationHtml(st: ViewState): string {
  const view = ui(st);
  const site = st.explorationSites.find(s => s.id === st.selectedExplorationSiteId);
  if (site) return navigation(st) + siteDetail(st, site);
  if (view.tab === "blueprints") return navigation(st) + blueprintsHtml(st);
  if (view.tab === "journal") return navigation(st) + journalHtml(st);
  const cc = st.commandCenter;
  const sorted = st.explorationSites.filter(s => view.filter === "all" || (view.filter === "leads" && (s.details?.lead || s.details?.opportunity?.has_lead && !s.details.studied))
    || (view.filter === "danger" && s.details?.guarded) || (view.filter === "pinned" && entryFor(st, s.id, "site").pinned))
    .sort((a, b) => !cc ? a.id.localeCompare(b.id)
    : Math.hypot(a.pos.x - cc.x, a.pos.y - cc.y) - Math.hypot(b.pos.x - cc.x, b.pos.y - cc.y));
  return navigation(st) + `<section class="exploration-panel"><header><h3>Exploration</h3><span>${sorted.length} contacts</span></header>
    <div class="exploration-tabs">${["all", "leads", "danger", "pinned"].map(f => `<button type="button" ${attrs(`filter-${f}`)} aria-pressed="${view.filter === f}">${label(f)}</button>`).join("")}</div>
    ${sorted.length ? `<div class="exploration-list">${sorted.map(s => `<button type="button" id="explore-row-${esc(s.id)}" class="exploration-row" ${attrs("open", s.id)}>
      ${art(s)}<span><b>${entryFor(st, s.id, "site").pinned ? "★ " : ""}${esc(siteTitle(s))}</b><small>${esc(siteStatus(s))}${cc ? ` · ${Math.round(Math.hypot(s.pos.x - cc.x, s.pos.y - cc.y)).toLocaleString()} su` : ""}</small><small>Report ${age(st, s.reported_at)}</small></span><span aria-hidden="true">›</span></button>`).join("")}</div>`
      : `<p>No contacts reported. Send a Scout beyond your home region.</p>`}</section>`;
}

function siteDetail(st: ViewState, site: ExplorationSiteView): string {
  const d = site.details;
  const task: ExpeditionTask = d ? "recover" : "investigate";
  const fleets = st.ghosts.filter(g => expeditionCapable(g, task));
  const hasCargo = d && (Object.values(d.cargo).some(n => n! > 0) || Object.values(d.modules).some(n => n! > 0));
  const descriptions = { derelict: "Freighters recover cargo and fittings on arrival.", station: "Restore as a 60,000 su sensor outpost.",
    asteroids: "Freighters collect these finite resources on arrival.", anomaly: "Freighters collect material samples on arrival.",
    precursor: "Ancient equipment and a Cruiser research dossier." };
  const programme = d && st.research?.programmes.find(p => p.id === d.programme)?.name;
  const cargoRows = d ? [...Object.entries(d.cargo), ...Object.entries(d.modules)]
    .filter(([, n]) => n! > 0).map(([kind, n]) => `<div><span>${esc(label(kind))}</span><b>${n}</b></div>`).join("") : "";
  const buttons = fleets.map(f => {
    const manifest = f.cargo_manifest ?? (f.cargo ? [f.cargo] : []);
    const stock = (key: string) => manifest.filter(c => c.commodity === key).reduce((sum, c) => sum + c.units, 0);
    const canRestore = stock("machinery") >= 12 && stock("electronics") >= 8;
    const assignment = f.expedition?.site === site.id ? f.expedition : null;
    const activity = assignment && (f.survey_progress == null ? "Under way" :
      `${({ investigate: "Investigating", recover: "Recovering", restore: "Restoring", study: "Studying", extract: "Extracting" })[assignment.task]} · ${Math.round(f.survey_progress * 100)}%`);
    return `<div class="exploration-fleet"><b>${esc(shipKindLabel(f.kind))} · ${esc(f.id)}</b><div>
      ${activity ? `<span>${activity}</span>` : ""}
      ${!d || hasCargo ? `<button type="button" ${attrs(task, site.id, f.id)} ${assignment?.task === task ? "disabled" : ""}>Travel here</button>` : ""}
      ${d?.kind === "station" && !d.restored_by ? `<button type="button" ${attrs("restore", site.id, f.id)} ${canRestore && assignment?.task !== "restore" ? "" : "disabled"}>Restore · 60s</button>` : ""}
      </div>${d?.kind === "station" && !d.restored_by && !canRestore ? `<small>Carry 12 Machinery + 8 Electronics to restore.</small>` : ""}</div>`;
  }).join("");
  return `<section class="exploration-panel"><div class="exploration-actions"><button type="button" ${attrs("back")}>← Contacts</button><button type="button" ${attrs("center", site.id)}>Center on map</button></div>
    <header class="exploration-hero">${art(site)}<div><small>${esc(siteStatus(site))}</small><h3>${esc(siteTitle(site))}</h3>
      <p>${d ? descriptions[d.kind] : "Scouts investigate automatically on arrival. Safety comes first."}</p></div></header>
    <small>Last report ${age(st, site.reported_at)}${d?.environment ? ` · ${esc(label(d.environment))}` : ""}</small>
    ${d?.guarded ? `<p class="exploration-warning">Armed scavengers guard this site. Scout retreat remains automatic; bring escorts to clear it.</p>` : ""}
    ${d ? `<section><h4>On site</h4><div class="exploration-cargo">${cargoRows || "Empty"}</div>
      <p title="Only the first dossier for each technology contributes research work. Normal prerequisites still apply.">Dossier · ${esc(programme ?? label(d.programme))}<br><small>First copy: ${Math.round(d.research_fraction * 100)}% research work</small></p></section>` : ""}
    ${d?.restored_by ? `<p>${d.restored_by === st.playerId ? "Your sensor outpost" : "Restored by another corporation"}.</p>` : ""}
    ${d?.lead ? `<section class="exploration-lead"><h4>Recovered coordinates</h4><p>${esc(d.lead.clue)}</p><button type="button" ${attrs("open", d.lead.site)}>Follow lead →</button></section>` : ""}
    ${deepHtml(st, site)}
    ${!d || hasCargo || (d.kind === "station" && !d.restored_by) ? `<section><h4>${d ? "Dispatch Freighter" : "Dispatch Scout"}</h4>
      <div class="exploration-fleets">${buttons || `<p>${d ? "A Freighter" : "A Scout"} is required.</p>`}</div>
      </section>` : ""}
    ${noteHtml(st, site.id, "site")}</section>`;
}

function deepHtml(st: ViewState, site: ExplorationSiteView): string {
  const d = site.details, o = d?.opportunity;
  if (!o) return "";
  const complete = o.task === "study" ? d?.studied : !Object.values(o.cargo).some(n => n! > 0);
  const requirements = { scout: "Scout", research_team: "Scout + Recon Suite, Nebula Spectrometer, or Fieldcraft 2 Captain",
    fuelled_freighter: "Freighter · Fuel carried as cargo", shielded_freighter: "Freighter · Polymers for disposable shielding + Fuel cargo" };
  const fleets = st.ghosts.filter(f => expeditionCapable(f, o.task));
  return `<section class="exploration-deep"><header><h4>${o.task === "study" ? "Deep investigation" : "Prepared extraction"}</h4><span>${complete ? "Complete" : `${o.seconds}s on site`}</span></header>
    <p>${esc(requirements[o.requirement])}</p>${goods(o.costs) ? `<small>Per expedition: ${esc(goods(o.costs))}</small>` : ""}
    <p>${o.blueprint ? `Blueprint · ${esc(label(o.blueprint))}` : esc(goods(o.cargo))}${o.has_lead ? " · New coordinates" : ""}</p>
    ${complete ? `<small>${o.blueprint ? "Manufacturing license received at command." : "This deep cache is exhausted."}</small>` : `<div class="exploration-fleets">${fleets.map(f => {
      const reason = deepExpeditionReason(site, f);
      const assigned = f.expedition?.site === site.id && f.expedition.task === o.task;
      return `<div class="exploration-fleet" id="deep-fleet-${esc(f.id)}"><b>${esc(shipKindLabel(f.kind))} · ${esc(f.id)}</b>
        <button type="button" ${attrs(o.task, site.id, f.id)} ${reason || assigned ? "disabled" : ""}>${assigned ? (f.survey_progress == null ? "Under way" : `${Math.round(f.survey_progress * 100)}%`) : o.task === "study" ? "Investigate deeper" : "Extract cache"}</button>${reason ? `<small>${esc(reason)}</small>` : ""}</div>`;
    }).join("") || `<small>${o.task === "study" ? "Scout" : "Freighter"} required.</small>`}</div>`}</section>`;
}

function journalHtml(st: ViewState): string {
  const system = st.galaxy?.systems.find(s => s.id === ui(st).system);
  if (system) {
    const report = st.systems.find(s => s.id === system.id);
    return `<section class="exploration-panel"><div class="exploration-actions"><button type="button" ${attrs("journal-back")}>← Journal</button><button type="button" ${attrs("system-center", system.id)}>Center on map</button></div>
      <h3>${esc(system.name)}</h3>${(report?.opportunities ?? []).slice(0, 3).map(o => `<p><b>${esc(o.body_name ?? o.title)}</b> · ${esc(o.reason)}</p>`).join("")}
      ${noteHtml(st, system.id, "system")}</section>`;
  }
  const entries = new Map((st.explorationJournal ?? []).map(e => [e.id, e]));
  for (const [id, entry] of ui(st).pending) entries.set(id, entry);
  const sites = st.explorationSites.filter(s => entries.get(s.id)?.pinned || entries.get(s.id)?.note || s.details?.lead
    || s.details?.opportunity?.has_lead && !s.details.studied);
  const systems = (st.galaxy?.systems ?? []).filter(s => entries.has(s.id)
    || st.systems.some(r => r.id === s.id && r.deposits != null));
  return `<section class="exploration-panel"><h3>Discovery journal</h3><h4>Leads & saved sites</h4><div class="exploration-list">${sites.map(s =>
    `<button type="button" class="exploration-row" id="journal-site-${esc(s.id)}" ${attrs("open", s.id)}><span><b>${entries.get(s.id)?.pinned ? "★ " : ""}${esc(siteTitle(s))}</b><small>${esc(siteStatus(s))} · ${age(st, s.reported_at)}</small><small class="exploration-preview-note">${esc(entries.get(s.id)?.note ?? "")}</small></span></button>`).join("") || "Pin a contact or recover a lead to keep it here."}</div>
    <h4>Surveyed worlds & systems</h4><div class="exploration-list">${systems.map(s => `<button type="button" class="exploration-row" id="journal-system-${esc(s.id)}" ${attrs("system-open", s.id)}><span><b>${entries.get(s.id)?.pinned ? "★ " : ""}${esc(s.name)}</b><small class="exploration-preview-note">${esc(entries.get(s.id)?.note ?? st.systems.find(r => r.id === s.id)?.opportunities?.[0]?.reason ?? "Add a note or map pin")}</small></span></button>`).join("") || "Survey a system to record its opportunities."}</div></section>`;
}

function blueprintsHtml(st: ViewState): string {
  return `<section class="exploration-panel"><h3>Discovery blueprints</h3>${MODULES.filter(m => isBlueprintOnly(m.kind)).map(m => {
    const known = st.research?.blueprints?.includes(m.kind);
    const lead = st.explorationSites.find(s => s.details?.opportunity?.blueprint === m.kind && !s.details.studied);
    return `<article class="exploration-deep"><header><h4>${esc(m.name)}</h4><span>${known ? "Licensed" : "Undiscovered"}</span></header><p>${esc(m.role)}</p>
      ${known ? `<small>Build → Modules · ${m.kind === "prismatic_lance" ? "Armaments Complex" : "staffed Shipyard"}</small>` : lead ? `<button type="button" ${attrs("open", lead.id)}>Investigate reported source →</button>` : `<small>Search wrecks, observatories and nebula sites.</small>`}</article>`;
  }).join("")}</section>`;
}

export function handleExplorationAction(button: HTMLElement, ctx: CoreContext): boolean {
  const action = button.dataset.exploreAct;
  if (!action) return false;
  const view = ui(ctx.state);
  if (action.startsWith("tab-")) { view.tab = action.slice(4); view.system = null; ctx.state.selectedExplorationSiteId = null; ctx.renderer.stateVersion++; return true; }
  if (action.startsWith("filter-")) { view.filter = action.slice(7); ctx.renderer.stateVersion++; return true; }
  if (action === "journal-back") { view.system = null; return true; }
  if (action === "system-open" || action === "system-center") {
    const system = ctx.state.galaxy?.systems.find(s => s.id === button.dataset.site);
    if (system) { view.tab = "journal"; view.system = system.id; if (action === "system-center") ctx.renderer.centerOnWorld(system.pos); }
    ctx.renderer.stateVersion++; return true;
  }
  if (action === "note" || action === "pin") {
    const id = button.dataset.site!, kind = button.dataset.kind === "system" ? "system" : "site";
    if (!(kind === "site" ? ctx.state.explorationSites.some(s => s.id === id) : ctx.state.galaxy?.systems.some(s => s.id === id))) return true;
    if (journalFull(ctx.state, id)) return true;
    const old = entryFor(ctx.state, id, kind);
    const text = button.closest(".exploration-note")?.querySelector("textarea")?.value ?? old.note;
    const entry = { id, kind, pinned: action === "pin" ? !old.pinned : old.pinned,
      note: [...text].filter(c => c === "\n" || !/[\p{Cc}]/u.test(c)).slice(0, 400).join("").trim() } as ExplorationJournalEntry;
    ctx.send({ type: "AnnotateExploration", entry }); view.pending.set(id, entry); ctx.renderer.stateVersion++; return true;
  }
  if (action === "back") { ctx.state.selectedExplorationSiteId = null; ctx.renderer.stateVersion++; return true; }
  const site = ctx.state.explorationSites.find(s => s.id === button.dataset.site);
  if (!site) return true;
  if (action === "open" || action === "center") {
    ctx.state.selectedExplorationSiteId = site.id;
    if (action === "center") ctx.renderer.centerOnWorld(site.pos);
    ctx.renderer.stateVersion++;
    return true;
  }
  if (!["investigate", "recover", "restore", "study", "extract"].includes(action)) return true;
  const fleet = ctx.state.ghosts.find(g => g.id === button.dataset.fleet && g.own);
  if (!fleet) return true;
  const command = explorationOrder(ctx.state, site, fleet, action as ExpeditionTask);
  if (command) ctx.intent.beginFleetCommand(command);
  return true;
}
