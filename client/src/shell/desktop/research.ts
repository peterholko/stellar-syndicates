import { fmtEta } from "../../core/derive/format";
import { researchQueueIds } from "../../core/derive/research";
import { type AcademyRow, type ProgrammeView } from "../../protocol";
import { state } from "../../state";
import { $, esc, researchIcon } from "./mapchrome";
import { activateWorkspacePage, deactivateWorkspacePage, workspacePageIsActive } from "./workspace";


// --- §research R6: the Programme Boards panel (top-navbar destination) ----------
// Owner-only (the View carries the viewer's corporation research). The
// whole 108-node tree as six Y-ladder boards; an active banner with the live rate
// + ETA + per-Academy contribution table (shown math); a queue strip you reorder
// (→ SetResearchQueue). Re-rendered only when something CHANGES (a coarse
// signature that includes the progress bucket, so the bar animates ~1 Hz).
export const FIELD_ORDER = ["propulsion", "materials", "computation", "weapons", "hulls", "life"];

export const FIELD_TITLE: Record<string, string> = {
  propulsion: "Propulsion", materials: "Materials", computation: "Computation",
  weapons: "Weapons", hulls: "Hulls", life: "Life",
};

export const SCHOOL_TITLE: Record<string, string> = {
  line_haul: "Line Haul", expedition: "Expedition", deep_crust: "Deep Crust", foundry: "Foundry",
  watch: "Watch", shadow: "Shadow", strike: "Strike", countermeasures: "Countermeasures",
  line: "Line", corsair: "Corsair", growth: "Growth", talent: "Talent",
};

export const ROMAN = ["", "I", "II", "III", "IV", "V", "VI", "VII", "VIII"];

export let lastResearchSig = "";


export function openResearch(): void {
  activateWorkspacePage("research-panel");
  lastResearchSig = "";
  updateResearchPanel();
}

export function closeResearch(): void {
  deactivateWorkspacePage("research-panel");
}

export function toggleResearch(): void {
  if (workspacePageIsActive("research-panel")) closeResearch();
  else openResearch();
}


export function researchNode(p: ProgrammeView, pos: number | null): string {
  const num = pos !== null ? `<span class="n">${pos + 1}</span> ` : "";
  const add = p.state === "available" ? ` data-rid="${esc(p.id)}"` : "";
  return `<div class="rp-node is-${p.state}"${add} title="${esc(p.blurb)}">` +
    `<div class="nm">${num}${esc(p.name)}</div><div class="bl">${esc(p.blurb)}</div></div>`;
}


export function researchBoard(fieldSlug: string, progs: ProgrammeView[], queue: string[]): string {
  const qpos = (id: string): number | null => {
    const i = queue.indexOf(id);
    return i >= 0 ? i : null;
  };
  const at = (school: string | null, tier: number) =>
    progs.filter((p) => (p.school ?? null) === school && p.tier === tier);
  // A tier-group: its Roman label, a gate bar if any node is sealed, then nodes.
  const group = (school: string | null, tier: number): string => {
    const nodes = at(school, tier);
    if (!nodes.length) return "";
    const sealed = nodes.find((n) => n.gate);
    let gate = "";
    if (sealed?.gate) {
      const g = sealed.gate;
      const pct = Math.max(0, Math.min(100, (g.current / Math.max(1e-9, g.threshold)) * 100));
      gate = `<div class="rp-gate">${esc(g.label)} ${Math.floor(g.current)} / ${Math.round(g.threshold)}</div>` +
        `<div class="rp-gatebar"><i style="width:${pct}%"></i></div>`;
    }
    const cards = nodes.map((n) => researchNode(n, qpos(n.id))).join("");
    return `<div class="rp-tier"><div class="lbl">Tier ${ROMAN[tier]}</div>${gate}${cards}</div>`;
  };
  const schools = Array.from(new Set(progs.filter((p) => p.school).map((p) => p.school as string)));
  let inner = group(null, 1) + group(null, 2);
  for (const s of schools) {
    inner += `<div class="lbl" style="color:#8fd3dd;margin-top:2px">⑂ ${esc(SCHOOL_TITLE[s] ?? s)}</div>`;
    // §ladder B2: Line (alone) runs past Tier V — the capital ladder VI–VIII.
    // group() renders nothing for tiers a school doesn't have.
    inner += group(s, 3) + group(s, 4) + group(s, 5) + group(s, 6) + group(s, 7) + group(s, 8);
  }
  return `<div class="rp-board"><h4>${researchIcon(fieldSlug, "lg")}<span>${esc(FIELD_TITLE[fieldSlug] ?? fieldSlug)}</span></h4>${inner}</div>`;
}


export function updateResearchPanel(): void {
  const el = $("research-panel");
  if (!el.classList.contains("is-open")) return;
  const r = state.research;
  const sig = r
    ? JSON.stringify([
        r.programmes.map((p) => p.state),
        r.queue, r.active?.id, r.stalled,
        r.active ? Math.round((r.active.progress / Math.max(1, r.active.cost)) * 200) : 0,
        Math.round(r.rate * 100),
        r.academies.map((a) => [a.rate.toFixed(2), a.supplied]),
        r.programmes.filter((p) => p.state === "locked" && p.gate).map((p) => Math.round((p.gate!.current / Math.max(1e-9, p.gate!.threshold)) * 40)),
      ])
    : "none";
  if (sig === lastResearchSig && el.innerHTML) return;
  lastResearchSig = sig;

  let body = "";
  if (!r) {
    body = `<div class="rp-note">Research data is unavailable. Reconnect to restore your corporation's Programme Boards.</div>`;
  } else {
    // Active banner.
    if (r.active) {
      const a = r.active;
      const pct = Math.max(0, Math.min(100, (a.progress / Math.max(1e-9, a.cost)) * 100));
      const eta = a.eta_secs != null ? `ETA ${fmtEta(a.eta_secs)}` : (r.stalled ? `<span class="rp-stalled">stalled</span>` : `no supply`);
      const acadRows = r.academies.length
        ? `<div class="rp-acad"><div class="hd">Academy</div><div class="hd">tier</div><div class="hd">rate/s</div>` +
          r.academies.map((x: AcademyRow) =>
            `<div class="${x.supplied ? "" : "amber"}">${esc(x.system)}${x.supplied ? "" : " ⚠"}</div>` +
            `<div>T${x.tier}</div><div>${x.rate.toFixed(2)}</div>`).join("") +
          `</div>`
        : `<div class="rp-acad"><div class="amber">No staffed Academy is contributing — assign a worker to an Academy.</div></div>`;
      const aField = r.programmes.find((p) => p.id === a.id)?.field ?? "";
      body += `<div class="rp-active">${researchIcon(aField, "xl")}<div class="rp-a-main"><div class="rp-a-top"><span class="rp-a-name">${esc(a.name)}</span>` +
        `<span class="rp-a-eta">${esc(String(Math.round(a.progress))) } / ${Math.round(a.cost)}·s · ${eta} · ${r.rate.toFixed(2)}/s</span></div>` +
        `<div class="rp-bar"><i style="width:${pct}%"></i></div>${acadRows}</div></div>`;
    } else {
      body += `<div class="rp-idle">No active programme. Pick any open node below to queue it — the front of the queue starts accruing.</div>`;
    }
    // Queue strip.
    const q = researchQueueIds();
    const chips = q.map((id, i) => {
      const p = r.programmes.find((x) => x.id === id);
      const nm = p ? p.name : id;
      return `<span class="rp-q-chip"><span class="n">${i + 1}</span>${researchIcon(p?.field ?? "", "sm")}${esc(nm)}` +
        `<button data-qup="${i}" title="Earlier">▲</button><button data-qdown="${i}" title="Later">▼</button>` +
        `<button data-qrm="${i}" title="Remove">✕</button></span>`;
    }).join("");
    body += `<div class="rp-queue"><span class="rp-q-label">Queue</span>${q.length ? chips : `<span class="rp-q-empty">empty — click an open programme to add it</span>`}</div>`;
    // Six boards.
    const boards = FIELD_ORDER.map((f) => researchBoard(f, r.programmes.filter((p) => p.field === f), q)).join("");
    body += `<div class="rp-boards">${boards}</div>`;
    body += `<div class="rp-note">Tech sheets are private — nothing here leaks to rivals. Completing a programme applies its effect instantly across your corporation.</div>`;
  }
  // Refresh ONLY the scroll body, keeping the .rp-body element itself across
  // ticks so its scrollTop survives. The active programme's progress bar
  // advances the render signature almost every tick; replacing the whole panel
  // recreated .rp-body each time and snapped the list back to the top. Building
  // the head+body shell once and updating just the body's children keeps the
  // scroll position put.
  let bodyEl = el.querySelector<HTMLElement>(".rp-body");
  if (!bodyEl) {
    el.innerHTML = `<div class="rp-head"><b>🔬 RESEARCH — PROGRAMME BOARDS</b><button class="rp-close" data-rp="close" title="Close">✕</button></div><div class="rp-body"></div>`;
    bodyEl = el.querySelector<HTMLElement>(".rp-body")!;
  }
  bodyEl.innerHTML = body;
}
