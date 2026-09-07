import { state } from "../../state";
import type { BodyView, OperationView, SystemStateView } from "../../protocol";
import { constructionStock } from "./fleet";
import { foundingHomeSystemId } from "./geo";
import { colonyPurpose } from "./colony";
import { SHIP_YARD } from "./market";

export type HandoffGoalId = "explore" | "upgrade" | "colony";
export type HandoffAction =
  | { kind: "build"; system: string; body: number; mode: "ships" | "structures"; select: string }
  | { kind: "fleet"; id: string }
  | { kind: "world"; system: string; body: number }
  | { kind: "system"; id: string }
  | { kind: "research" }
  | { kind: "warehouse" }
  | { kind: "operations" };
export interface HandoffGoal {
  id: HandoffGoalId;
  title: string;
  summary: string;
  status: string;
  done: boolean;
  requirements: { text: string; met: boolean }[];
  costs: { commodity: string; units: number; stock: number }[];
  payoff: string;
  action: HandoffAction | null;
  actionLabel: string;
  funding: OperationView | null;
  prospect: string | null;
}

/** Advice over the RECEIVED founding, fleet, survey and contract picture. These
 * cards neither complete objectives nor grant rewards: the tutorial and normal
 * operation reports remain the only authorities. No timer, localStorage flag or
 * unreported battle outcome can graduate a player or reveal a prospect here. */
export function postVictoryHandoff(): HandoffGoal[] {
  const founding = state.founding;
  if (!founding?.bounty_received) return [];
  const homeId = foundingHomeSystemId();
  const home = state.systems.find(s => s.id === homeId);
  if (!home) return [];
  const own = state.ghosts.filter(g => g.own);
  const fleet = (kind: string) => own.find(g => g.composition?.some(c => c.kind === kind && (c.count ?? 0) > 0));
  const scout = fleet("scout");
  const colony = fleet("colony");
  const upgraded = own.find(g => g.composition?.some(c =>
    ["corvette", "destroyer", "cruiser", "battleship", "dreadnought", "titan"].includes(c.kind) && (c.count ?? 0) > 0));
  const candidates = founding.survey_candidates.map(id => state.systems.find(s => s.id === id)).filter((s): s is SystemStateView => !!s);
  const surveyed = (s: SystemStateView) => s.bodies.some(b => b.geology != null);
  const reports = candidates.filter(surveyed);
  const needed = Math.min(2, founding.survey_candidates.length) || 2;
  // expansion_unlocked is the server's arrived milestone, including the fallback
  // two-survey path for old saves without assigned prospects.
  const explorationDone = founding.expansion_unlocked;
  const colonyDone = founding.stage === "complete";
  const researched = explorationDone || state.research?.programmes.some(p => p.tier === 1 && p.state === "completed")
    || ["build_scout", "survey_candidates", "build_colony", "establish_colony", "complete"].includes(founding.stage);
  const academy = home.bodies.find(b => (b.structures.academy ?? 0) > 0);
  const academyStaffed = academy && home.assignments.some(a => a.body_id === academy.id && a.structure === "academy"
    && (a.workers > 0 || Object.values(a.specialists).some(n => n > 0)));
  const shipyard = [...home.bodies].sort((a, b) => (b.structures.shipyard ?? 0) - (a.structures.shipyard ?? 0))[0];
  const build = (select: string, mode: "ships" | "structures" = "ships", body?: BodyView): HandoffAction | null => {
    const site = body ?? shipyard;
    return site ? { kind: "build", system: home.id, body: site.id, mode, select } : null;
  };
  const funding = (chapter: "escort" | "salvage" | "production") => state.operations.find(o =>
    o.briefing?.follow_up === chapter && (o.state === "active" && o.joined
      || o.state === "offered" && o.expires_at > state.simTime)) ?? null;
  const costs = (kind: string) => {
    const stock = constructionStock(home).available;
    return (state.galaxy?.build_options.find(o => o.key === kind)?.costs ?? [])
      .map(c => ({ ...c, stock: stock.get(c.commodity) ?? 0 }));
  };
  const yardReady = (kind: string) => (home.structures?.shipyard ?? 0) >= SHIP_YARD[kind].tier;
  const building = (kind: string) => home.builds.some(j => j.key === kind);
  const missingId = founding.survey_candidates.find(id => !reports.some(s => s.id === id));
  const availableProspects = reports.filter(s => s.owner == null);
  const prospect = [...availableProspects].sort((a, b) => {
    const score = (s: SystemStateView) => Math.max(0, ...(s.opportunities ?? []).map(o => o.score));
    return score(b) - score(a);
  })[0];
  const purpose = prospect ? colonyPurpose(prospect, home) : null;
  const colonySystem = colonyDone ? state.systems.find(s => s.owner === state.playerId && s.id !== home.id) : undefined;

  const explorationAction: HandoffAction | null = explorationDone ? { kind: "warehouse" }
    : !researched ? !academy ? build("academy", "structures", [...home.bodies].sort((a, b) =>
      (b.infrastructure_slots ?? 0) - (a.infrastructure_slots ?? 0))[0])
      : !academyStaffed ? { kind: "world", system: home.id, body: academy.id } : { kind: "research" }
    : scout ? { kind: "fleet", id: scout.id } : build("scout");
  const explore: HandoffGoal = {
    id: "explore", title: "Survey your next worlds", done: explorationDone,
    status: explorationDone ? "Surveys received" : `${Math.min(reports.length, needed)}/${needed} reports`,
    summary: "Compare a population world with an industrial prospect.",
    requirements: [
      { text: "Staff Academy I · finish first research", met: researched },
      { text: `Scout · Shipyard I${building("scout") && !scout ? " · building" : ""}`, met: !!scout || explorationDone },
      { text: `${explorationDone ? needed : Math.min(reports.length, needed)}/${needed} survey reports received`, met: explorationDone },
    ],
    costs: !scout && !explorationDone ? costs("scout") : [],
    payoff: "Colony Ship kit in your Market Warehouse · expansion unlocked",
    action: explorationAction,
    actionLabel: explorationDone ? "View colony kit" : !researched ? !academy ? "Build Academy" : !academyStaffed ? "Staff Academy" : "Choose research"
      : scout ? "Select Scout" : building("scout") ? "View Scout build" : "Build Scout",
    funding: funding("salvage"),
    prospect: missingId ?? reports[0]?.id ?? null,
  };
  const upgrade: HandoffGoal = {
    id: "upgrade", title: "Reinforce your fleet", done: !!upgraded,
    status: upgraded ? "Combat reinforcement reported" : building("corvette") ? "Building" : "Optional upgrade",
    summary: "Add a Corvette for tougher escorts and pirate encounters.",
    requirements: [
      { text: "Shipyard II", met: yardReady("corvette") },
      { text: "Build a Corvette", met: !!upgraded },
    ],
    costs: !upgraded ? costs("corvette") : [],
    payoff: "A stronger combat hull · your starting Interceptor stays useful",
    action: upgraded ? { kind: "fleet", id: upgraded.id } : build(yardReady("corvette") ? "corvette" : "shipyard", yardReady("corvette") ? "ships" : "structures"),
    actionLabel: upgraded ? "Select combat fleet" : !yardReady("corvette") ? "Upgrade Shipyard" : building("corvette") ? "View Corvette build" : "Build Corvette",
    funding: funding("escort"),
    prospect: null,
  };
  const settle: HandoffGoal = {
    id: "colony", title: "Found a specialist colony", done: colonyDone,
    status: colonyDone ? "Colony established" : explorationDone ? "Expansion unlocked" : "After the surveys",
    summary: purpose ? `${state.galaxy?.systems.find(s => s.id === prospect?.id)?.name ?? "Surveyed prospect"}: ${purpose.headline}.`
      : "Choose the world that supplies what home lacks.",
    requirements: [
      { text: "Expansion unlocked by received surveys", met: explorationDone },
      { text: "Colony Ship · Shipyard I", met: !!colony || colonyDone },
      { text: "Surveyed, unclaimed destination", met: !!prospect || colonyDone },
    ],
    costs: explorationDone && !colony && !colonyDone ? costs("colony") : [],
    payoff: purpose ? `${purpose.homeNeed}${purpose.imports.length ? ` Imports: ${purpose.imports.map(c => c.replaceAll("_", " ")).join(", ")}.` : ""}`
      : "A second production base · connect it to home with freight",
    action: colonySystem ? { kind: "system", id: colonySystem.id }
      : !explorationDone ? explorationAction : colony ? { kind: "fleet", id: colony.id } : build("colony"),
    actionLabel: colonySystem ? "Open colony" : !explorationDone ? explore.actionLabel : colony ? "Select Colony Ship" : building("colony") ? "View Colony Ship build" : "Build Colony Ship",
    funding: funding("production"),
    prospect: prospect?.id ?? null,
  };
  return [explore, upgrade, settle];
}
