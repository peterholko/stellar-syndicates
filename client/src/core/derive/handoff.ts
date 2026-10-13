import { state } from "../../state";
import type { BodyView, OperationView } from "../../protocol";
import { constructionStock } from "./fleet";
import { foundingHomeSystemId } from "./geo";
import { SHIP_YARD } from "./market";

export type HandoffGoalId = "colony" | "upgrade";
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
  // §charter v3: the charter itself teaches the Shipyard, research, surveys and
  // belt mining. Its handover is the first colony and a stronger escort.
  if (founding?.stage !== "complete") return [];
  const homeId = foundingHomeSystemId();
  const home = state.systems.find(s => s.id === homeId);
  if (!home) return [];
  const own = state.ghosts.filter(g => g.own);
  const fleet = (kind: string) => own.find(g => g.composition?.some(c => c.kind === kind && (c.count ?? 0) > 0));
  const upgraded = own.find(g => g.composition?.some(c =>
    ["corvette", "destroyer", "cruiser", "battleship", "dreadnought", "titan"].includes(c.kind) && (c.count ?? 0) > 0));
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
  // A habitable world the corporation has charted and nobody holds.
  const garden = state.systems.find(s => s.owner == null && s.bodies.some(b => b.habitable));
  const colonyShip = fleet("colony");
  const settled = state.systems.filter(s => s.owner === state.playerId).length >= 2;
  const colony: HandoffGoal = {
    id: "colony", title: "Found your first colony", done: settled,
    status: settled ? "Second system held" : colonyShip ? "Colony Ship ready" : garden ? "Habitable world charted" : "Next goal",
    summary: "Chart a habitable world, then send a Colony Ship to claim it.",
    requirements: [
      { text: "Chart a habitable world (a probe survey reveals one)", met: !!garden || settled },
      { text: `Colony Ship · Shipyard I${building("colony") && !colonyShip ? " · building" : ""}`, met: !!colonyShip || settled },
      { text: "Claim the system", met: settled },
    ],
    costs: !colonyShip && !settled ? costs("colony") : [],
    payoff: "A second system: more people, more slots, more supply",
    action: settled ? { kind: "system", id: home.id } : !garden ? build("survey_probe")
      : colonyShip ? { kind: "fleet", id: colonyShip.id } : build("colony"),
    actionLabel: settled ? "Develop home" : !garden ? "Build Survey Probe"
      : colonyShip ? "Select Colony Ship" : building("colony") ? "View Colony Ship build" : "Build Colony Ship",
    funding: funding("salvage"),
    prospect: garden?.id ?? null,
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
  return [colony, upgrade];
}
