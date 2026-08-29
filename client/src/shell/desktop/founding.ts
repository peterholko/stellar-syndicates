import { fmtDur } from "../../core/derive/format";
import { foundingHomeSystemId } from "../../core/derive/geo";
import { label } from "../../icons";
import { state, type ViewState } from "../../state";
import { inboxFocusSystem } from "./checkin";
import { $, esc, setHtml, syncFoundingGuideClearance } from "./mapchrome";
import { openMarket, setMarketTab } from "./market";
import { openRail } from "./rail";
import { openResearch } from "./research";
import { selectShip } from "./ship";
import { enterSystem, openBodyPanelById } from "./sysview";


export const FOUNDING_STEP: Record<NonNullable<ViewState["founding"]>["stage"], number> = {
  build_shipyard: 1, build_mine: 2, build_convoy: 3, export_production: 4,
  defeat_privateer: 5, complete_export: 6, build_academy: 7, first_research: 8,
  build_scout: 9, survey_candidates: 10, build_colony: 11,
  establish_colony: 12, complete: 12,
};

export const FOUNDING_MINIMIZED_KEY = "stellar-syndicates:founding-guide-minimized";

export let foundingGuideMinimized = false;

export function __init_founding_7165(): void {
try {
  foundingGuideMinimized = localStorage.getItem(FOUNDING_MINIMIZED_KEY) === "1";
} catch {
  // Storage can be unavailable in locked-down browsers; the session toggle
  // still works, it simply will not survive a reload.
}
}


export function foundingGuideHeader(step: string, shield: string, title: string): string {
  const expanded = !foundingGuideMinimized;
  return `<div class="fg-top"><span class="fg-step">${esc(step)}</span>` +
    `<span class="fg-mini-title">${esc(title)}</span><span class="fg-shield">${esc(shield)}</span>` +
    `<button class="fg-minimize" type="button" data-founding-minimize aria-expanded="${expanded}" ` +
    `title="${expanded ? "Minimize tutorial" : "Expand tutorial"}" aria-label="${expanded ? "Minimize tutorial" : "Expand tutorial"}">${expanded ? "−" : "+"}</button></div>`;
}


// The programme assigns places, not answers. Before a report arrives these
// cards show only public spectral information; afterward they compare the
// server-scored, survey-gated opportunities that the ordinary system view knows.
export function foundingProspectCards(): string {
  const ids = state.founding?.survey_candidates ?? [];
  if (!ids.length || !state.galaxy) return "";
  const cards = ids.slice(0, 2).map((id, index) => {
    const fixed = state.galaxy!.systems.find((system) => system.id === id);
    const served = state.systems.find((system) => system.id === id);
    if (!fixed) return "";
    const surveyed = !!served?.bodies.some((body) => body.geology !== null);
    const top = served?.opportunities?.[0];
    const eyebrow = index === 0 ? "Population prospect" : "Industrial prospect";
    const finding = surveyed
      ? top
        ? `${esc(top.title)} · <b>×${top.score.toFixed(2)}</b>${top.body_name ? ` · ${esc(top.body_name)}` : ""}`
        : "Survey received · no standout specialty"
      : `${label(fixed.band)} spectrum · awaiting survey`;
    return `<button class="fg-prospect${surveyed ? " is-known" : ""}" data-founding-candidate="${esc(id)}">` +
      `<span><small>${esc(eyebrow)}</small><b>${esc(fixed.name)}</b></span><em>${finding}</em></button>`;
  }).join("");
  return cards ? `<div class="fg-compare">${cards}</div>` : "";
}


export function updateFoundingGuide(): void {
  const el = $("founding-guide");
  const f = state.founding;
  if (!f || (f.stage === "complete" && !f.protected)) {
    el.classList.remove("is-open");
    el.classList.remove("is-minimized");
    return;
  }
  const minLeft = Math.max(0, f.protection_min_until - state.simTime);
  const shield = f.protected
    ? minLeft > 0 ? `shield · ${fmtDur(minLeft)}` : "shield active"
    : "shield ended";
  if (f.stage === "complete") {
    const title = "Your second holding is established";
    setHtml(el,
      foundingGuideHeader("Founding complete", shield, title) +
      `<div class="fg-title">${title}</div>` +
      `<div class="fg-copy">The founding chapter is complete. Research, trade and further expansion now run on their ordinary clocks.</div>`);
    el.classList.toggle("is-minimized", foundingGuideMinimized);
    el.classList.add("is-open");
    return;
  }

  // The Academy milestone has two visible halves: construction, then a worker assignment.
  // Read both from the same served home-system picture as the planet panel so
  // completing the first half never leaves the guide looking oblivious.
  const foundingHomeId = foundingHomeSystemId();
  const foundingHome = foundingHomeId
    ? state.systems.find((system) => system.id === foundingHomeId)
    : undefined;
  const mineBody = foundingHome?.bodies.find(
    (body) => (body.structures?.mining_complex ?? 0) > 0,
  );
  const mineStaffed = !!mineBody && (foundingHome?.assignments ?? []).some(
    (assignment) => assignment.body_id === mineBody.id
      && assignment.structure === "mining_complex"
      && assignment.workers > 0,
  );
  const academyBody = foundingHome?.bodies.find(
    (body) => (body.structures?.academy ?? 0) > 0,
  );
  const academyStaffed = !!academyBody && (foundingHome?.assignments ?? []).some(
    (assignment) => assignment.body_id === academyBody.id
      && assignment.structure === "academy"
      && (assignment.workers > 0
        || Object.values(assignment.specialists ?? {}).some((posted) => posted > 0)),
  );

  const content: Record<typeof f.stage, { title: string; copy: string; action: string; label: string }> = {
    build_shipyard: {
      title: "Build Shipyard I",
      copy: "Your local founding kit covers the Shipyard, Mining Complex and first Freighter—no market import is required yet. Establish orbital shipbuilding.",
      action: "home", label: "Open home system",
    },
    build_mine: mineBody ? {
      title: mineStaffed ? "Mining Complex staffed" : "Assign worker to Mining Complex I",
      copy: mineStaffed
        ? `Mining Complex I on ${mineBody.name} is staffed and producing Metallic Ore.`
        : `Mining Complex I is complete on ${mineBody.name}. Open that world and select Assign worker; an unstaffed mine produces no ore.`,
      action: "mine", label: `Open ${mineBody.name}`,
    } : {
      title: "Build Mining Complex I",
      copy: "Use the next part of the local kit to establish Mining Complex I on the ore body. Your Agroplex already produces the other export: Provisions.",
      action: "home", label: "Open home system",
    },
    build_convoy: {
      title: "Build your first Freighter",
      copy: "The remainder of the local founding kit is exactly 25 Alloys, 10 Machinery and 10 Polymers. Build the Freighter at your Shipyard without buying or importing anything.",
      action: "home", label: "Open home system",
    },
    export_production: {
      title: "Prepare and dispatch a Freighter",
      copy: "Load Provisions and Metallic Ore at home, then send the Freighter. Expect a privateer.",
      action: "convoy", label: "Select Freighter",
    },
    defeat_privateer: {
      title: "Guard the Freighter",
      copy: "The Freighter's 20,000-su sensor has found a slow, damaged Rogue Privateer converging from off its travel route. Send the Interceptor to catch it before it reaches the civilian hull. Victory pays 4,500 credits.",
      action: "privateer", label: "Select Rogue Privateer",
    },
    complete_export: {
      title: "Complete the guarded export",
      copy: "Deliver and sell both opening goods at the Market Hub using this or another Freighter.",
      action: "convoy", label: "Select Freighter",
    },
    build_academy: academyBody ? {
      title: academyStaffed ? "Academy staffed" : "Assign worker to Academy I",
      copy: academyStaffed
        ? `Academy I on ${academyBody.name} is staffed. Its first research report is reaching command.`
        : `Academy I is complete on ${academyBody.name}. Open that world and select Assign worker; an unstaffed Academy produces no research.`,
      action: "academy", label: `Open ${academyBody.name}`,
    } : {
      title: "Establish Academy I",
      copy: "Buy and import 25 Alloys, 15 Electronics and 20 Provisions. Build Academy I and assign at least one worker; this is your corporation's research engine.",
      action: "warehouse", label: "Open Market Warehouse",
    },
    first_research: {
      title: "Choose your first programme",
      copy: "Complete any Tier I programme. Drive Tuning accelerates travel; Deep Bores lifts extraction; Med Bays attract migrant liners faster. A founding grant leaves about 12 Academy-minutes of work.",
      action: "research", label: "Open Programme Boards",
    },
    build_scout: {
      title: "Build a Scout",
      copy: "Buy and import 15 Alloys, 8 Electronics and 8 Fuel, then build the Scout that will open the exploration chapter.",
      action: "warehouse", label: "Open Market Warehouse",
    },
    survey_candidates: {
      title: "Compare two expansion prospects",
      copy: "Survey both assigned systems. Use jump-drive hops between gravity wells and return to the home dock between sorties; straight warp both ways can exhaust the Scout's first tank. Exact tradeoffs stay unknown until each report reaches home.",
      action: "scout", label: "Select Scout",
    },
    build_colony: {
      title: "Build your first Colony Ship",
      copy: "Both reports authorize one exact Colony Ship kit in your Market Warehouse. Compare the discoveries, freight the kit home, and build the hull.",
      action: "warehouse", label: "Open Market Warehouse",
    },
    establish_colony: {
      title: "Establish your second holding",
      copy: "Choose the prospect that fits your strategy, send the Colony Ship there, and establish the colony. The less glamorous system can remain a later specialty outpost.",
      action: "colony", label: "Select Colony Ship",
    },
  };
  const c = content[f.stage];
  const selectable = c.action === "home"
    ? !!foundingHomeSystemId()
    : c.action === "mine"
      ? !!foundingHomeId && !!mineBody
    : c.action === "academy"
      ? !!foundingHomeId && !!academyBody
    : c.action === "interceptor"
      ? !!f.interceptor && state.ghosts.some((g) => g.id === f.interceptor)
      : c.action === "privateer"
        ? !!f.privateer && state.ghosts.some((g) => g.id === f.privateer)
        : c.action === "convoy"
          ? state.ghosts.some((g) => g.own && g.composition?.some((x) => x.kind === "convoy"))
          : c.action === "scout"
            ? state.ghosts.some((g) => g.own && g.composition?.some((x) => x.kind === "scout"))
            : c.action === "colony"
              ? state.ghosts.some((g) => g.own && g.composition?.some((x) => x.kind === "colony"))
            : true;
  const prospects = ["survey_candidates", "build_colony", "establish_colony"].includes(f.stage)
    ? foundingProspectCards()
    : "";
  setHtml(el,
    foundingGuideHeader(`Founding ${FOUNDING_STEP[f.stage]}/12`, shield, c.title) +
    `<div class="fg-title">${esc(c.title)}</div><div class="fg-copy">${esc(c.copy)}</div>` +
    prospects +
    `<button class="fg-action" data-founding-action="${c.action}"${selectable ? "" : " disabled"}>${esc(c.label)}</button>`);
  el.classList.toggle("is-minimized", foundingGuideMinimized);
  el.classList.add("is-open");
}


export function __init_founding_7356(): void {
$("founding-guide").addEventListener("click", (e) => {
  if ((e.target as HTMLElement).closest("[data-founding-minimize]")) {
    foundingGuideMinimized = !foundingGuideMinimized;
    try {
      localStorage.setItem(FOUNDING_MINIMIZED_KEY, foundingGuideMinimized ? "1" : "0");
    } catch {
      // Session-only fallback; see initialization above.
    }
    updateFoundingGuide();
    syncFoundingGuideClearance();
    return;
  }
  const candidate = (e.target as HTMLElement).closest<HTMLButtonElement>("[data-founding-candidate]")?.dataset.foundingCandidate;
  if (candidate) {
    inboxFocusSystem(candidate);
    return;
  }
  const action = (e.target as HTMLElement).closest<HTMLButtonElement>("[data-founding-action]")?.dataset.foundingAction;
  if (!action) return;
  if (action === "home") {
    const id = foundingHomeSystemId();
    if (id) { state.selectedSystemId = id; openRail("system"); }
  } else if (action === "mine") {
    const id = foundingHomeSystemId();
    const dyn = id ? state.systems.find((system) => system.id === id) : undefined;
    const body = dyn?.bodies.find((candidate) => (candidate.structures?.mining_complex ?? 0) > 0);
    const sys = id ? state.galaxy?.systems.find((candidate) => candidate.id === id) : undefined;
    if (sys && body) {
      enterSystem(sys);
      openBodyPanelById(String(body.id));
    }
  } else if (action === "academy") {
    const id = foundingHomeSystemId();
    const dyn = id ? state.systems.find((system) => system.id === id) : undefined;
    const body = dyn?.bodies.find((candidate) => (candidate.structures?.academy ?? 0) > 0);
    const sys = id ? state.galaxy?.systems.find((candidate) => candidate.id === id) : undefined;
    if (sys && body) {
      enterSystem(sys);
      openBodyPanelById(String(body.id));
    }
  } else if (action === "warehouse") {
    openMarket(); setMarketTab("warehouse");
  } else if (action === "research") {
    openResearch();
  } else if (action === "interceptor" && state.founding?.interceptor) {
    selectShip(state.founding.interceptor);
  } else if (action === "privateer" && state.founding?.privateer) {
    selectShip(state.founding.privateer);
  } else if (action === "convoy" || action === "scout" || action === "colony") {
    const kind = action as "convoy" | "scout" | "colony";
    const fleet = state.ghosts.find((g) => g.own && g.composition?.some((x) => x.kind === kind));
    if (fleet) selectShip(fleet.id);
  }
});
}

