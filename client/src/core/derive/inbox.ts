// §log (Oct 28 2026): ONE inbox for both shells. The desktop Log's ~25 rules
// and mobile's treaty, invite and battle cards merge here; each shell only
// draws rows and runs their `Go` descriptors. Rows are facts, not sentences:
// the old explanation rides along as `hint` (a tooltip). Grouped by what kind
// of attention they need, one primary action each; dismiss and snooze persist
// per corporation in browser storage.

import type { IconKey } from "../../icons";
import { label } from "../../icons";
import { fleetCargoUnits, fleetCountLabel, type ClientMsg, type EntityId, type TimelineEntry, type Vec2 } from "../../protocol";
import { reportMarkKey } from "../../battlehistory";
import { liveSimTime, state, type ViewState } from "../../state";
import { traitLine } from "./captains";
import { blockadeBrief, blockadeStage, siegeClock } from "./blockade";
import { regimeStore, shipKindLabel } from "./fleet";
import { fmtBuildDur as fmtDur } from "./format";
import { freshSurveyReports, locName, REPORT_RECENT_S, systemName } from "./geo";
import type { HistoryFilter } from "./history";
import { pirateSite } from "./pirates";
import { nodeBonusDesc } from "./research";
import { attuneSecs, crackdownTitle, demandText, journeySteps, openEvidence } from "./season";

/** Where a row or report action leads; each shell runs these its own way. */
export type Go =
  | { to: "system"; id: EntityId }
  | { to: "fleet"; id: EntityId }
  | { to: "panel"; panel: GoPanel; params?: Record<string, string>; query?: Record<string, string> }
  /** A direct order: answer a treaty, join a syndicate, take an operation. */
  | { to: "send"; command: ClientMsg; done?: string }
  /** A fleet order that rides the light: the shell's own confirm flow. */
  | { to: "withdraw"; fleet: EntityId; pos: Vec2 }
  | { to: "history"; filter?: HistoryFilter; system?: EntityId }
  | { to: "showAll" };

export type GoPanel = "logistics" | "operations" | "syndicate" | "research" | "signals" | "market" | "rankings"
  | "comms" | "faction" | "battle" | "production" | "build" | "world";

export type InboxGroup = "threats" | "supply" | "fleets" | "opportunities";
export const INBOX_GROUPS: readonly { key: InboxGroup; label: string }[] = [
  { key: "threats", label: "Threats" },
  { key: "supply", label: "Supply" },
  { key: "fleets", label: "Fleets" },
  { key: "opportunities", label: "Opportunities" },
];

export type InboxTone = "bad" | "warn" | "info" | "neutral";

export interface InboxAction {
  label: string;
  go: Go;
  icon?: IconKey;
  danger?: boolean;
}

export interface InboxItem {
  /** Stable, for read marks, dismiss and snooze. */
  key: string;
  group: InboxGroup;
  /** Order within the group (higher first). */
  weight: number;
  tone: InboxTone;
  icon: IconKey;
  /** One line, with its place. */
  title: string;
  /** The numbers, short. */
  facts: string;
  /** Why it matters and what to do: a tooltip, never body text. */
  hint: string;
  /** Sim-time of the information it rests on. */
  age?: number;
  /** The first is the row's button; the rest sit in its menu. */
  actions: InboxAction[];
}

/** Hostile Interceptors this near an owned system make the inbox (the old
 * 80k sensor bubble × 1.6). A plain distance: what is shown is already fogged. */
const HOSTILE_CONCERN_SU = 128_000;
const MAX_HOSTILE_ITEMS = 4;
const IDLE_UNITS = 30;
/** A raid warning older than this, with no later line, is stale. */
const RAID_WARNING_STALE_S = 20 * 60;
/** §founding R8: until the privateer's bounty is in, side business stays out
 * of the Inbox, so the guide and real threats stay on top. */
const CHARTER_QUIET = ["nodeopen:", "nodes:", "operations:", "surveyops", "surveyrep:", "enclave:", "idle:", "queue:",
  "invites:", "idlefleet:", "loaded:", "storage:", "refinery:", "noauto"];
/** §node: an unclaimed node is news only this near a system you own: a
 * home's neighbourhood ring ends here (sim `NEIGHBOURHOOD_OUTER_SU`). */
const NODE_NEARBY_SU = 70_000;

/** Deterministic, owner-gated derivation from the served View only. */
export function computeInbox(st: ViewState = state): InboxItem[] {
  const out: InboxItem[] = [];
  if (st.playerId === null || !st.galaxy) return out;
  const galaxy = st.galaxy;
  const owned = st.systems.filter((system) => system.owner === st.playerId);
  const ownedIds = new Set(owned.map((system) => system.id));
  const active = st.standingOrders.filter((order) => order.status === "active");
  const now = liveSimTime();
  const sysPos = (id: string): Vec2 | null => galaxy.systems.find((system) => system.id === id)?.pos ?? null;
  const show = (id: EntityId, text = "Show"): InboxAction => ({ label: text, go: { to: "system", id } });
  const fleetGo = (id: EntityId, text = "Open fleet"): InboxAction => ({ label: text, go: { to: "fleet", id } });
  const logistics = (query: Record<string, string>, text = "Set supply route"): InboxAction =>
    ({ label: text, icon: "doctrine", go: { to: "panel", panel: "logistics", query } });

  // ── Threats ──
  // §log: a raid the server warned of, until it lands or goes stale.
  for (const [system, warning] of latestRaidWarnings(st.timeline, now)) {
    const name = systemName(system);
    const eta = warning.data?.eta ? warning.data.eta - now : null;
    out.push({ key: `raid:${system}:${warning.id ?? warning.at_time}`, group: "threats", weight: 98, tone: "bad", icon: "raid",
      title: `Raid inbound · ${name}`,
      facts: [warning.data?.ships ? `${warning.data.ships} raiders` : "", eta !== null ? (eta > 30 ? `ETA ~${Math.ceil(eta / 60)}m` : "arriving") : ""].filter(Boolean).join(" · "),
      hint: "Pirates take up to 10% of each good, 60 units in all. Interceptors at home fight them off; deliveries wait while they hold the system.",
      age: warning.at_time, actions: [{ label: "Defend", go: { to: "system", id: system } }] });
  }
  for (const system of st.systems) {
    const blockade = system.blockade;
    if (!blockade) continue;
    const stage = blockadeStage(blockade);
    const brief = blockadeBrief(blockade, now, galaxy.siege_secs, system.ground?.marines_needed);
    const name = systemName(system.id);
    if (system.owner === st.playerId) {
      const pirates = blockade.by_name === "Pirates";
      out.push({ key: `blockade-${stage}:${system.id}`, group: "threats", weight: stage === "siege" ? 100 : stage === "held" ? 90 : 86,
        tone: stage === "contested" ? "warn" : "bad", icon: stage === "siege" ? "siege" : pirates ? "raid" : "blockade",
        title: `${brief.title} · ${name}`,
        facts: stage === "siege" ? `falls in ${fmtDur(siegeClock(blockade, now, galaxy.siege_secs)?.left ?? 0)}`
          : stage === "held" ? (pirates ? "≤10% of each good · 60 units" : "logistics cut · stores stripped") : "defenses fighting",
        hint: brief.detail, age: blockade.since,
        actions: [{ label: "Send defenders", go: { to: "system", id: system.id } }] });
    } else if (blockade.by_me) {
      const ripe = siegeClock(blockade, now, galaxy.siege_secs)?.ripe ?? false;
      if (stage !== "contested" && !ripe) continue;
      out.push({ key: ripe ? `siege-ripe:${system.id}` : `blockade-contesting:${system.id}`, group: "threats", weight: ripe ? 84 : 60,
        tone: ripe ? "info" : "warn", icon: ripe ? "siege" : "blockade", title: `${brief.title} · ${name}`,
        facts: ripe ? "land marines" : "your fleet is fighting", hint: brief.detail, age: blockade.since,
        actions: [show(system.id)] });
    }
  }
  for (const battle of st.battles) {
    if (!battle.own) continue;
    const ownFleet = st.ghosts.find((fleet) => fleet.own && battle.participants.includes(fleet.id));
    const actions: InboxAction[] = [{ label: "Open battle", go: { to: "panel", panel: "battle", params: { id: battle.id, label: "Ongoing battle" } } }];
    if (ownFleet) actions.push({ label: "Withdraw", danger: true, go: { to: "withdraw", fleet: ownFleet.id, pos: battle.pos } });
    out.push({ key: `battle:${battle.id}`, group: "threats", weight: 92, tone: "bad", icon: "battle",
      title: `Engaged near ${locName(battle.pos)}`, facts: `${battle.participants.length} fleets`,
      hint: "A battle is underway. Reinforce, or withdraw to break off (the order is light-delayed).",
      age: battle.started_at, actions });
  }
  const hostiles: InboxItem[] = [];
  for (const fleet of st.ghosts) {
    if (fleet.own || fleet.ally || fleet.kind !== "raider") continue;
    let near: { id: string; distance: number; pos: Vec2 } | null = null;
    for (const system of owned) {
      const pos = sysPos(system.id);
      if (!pos) continue;
      const distance = Math.hypot(fleet.pos.x - pos.x, fleet.pos.y - pos.y);
      if (distance <= HOSTILE_CONCERN_SU && (!near || distance < near.distance)) near = { id: system.id, distance, pos };
    }
    if (!near) continue;
    const speed = Math.hypot(fleet.vel.x, fleet.vel.y);
    const closing = speed > 1 && fleet.vel.x * (near.pos.x - fleet.pos.x) + fleet.vel.y * (near.pos.y - fleet.pos.y) > 0;
    const size = fleet.composition ? `${fleet.composition.reduce((sum, stack) => sum + stack.count, 0)}` : `~${fleetCountLabel(fleet)}`;
    hostiles.push({ key: `hostile:${fleet.id}:${near.id}`, group: "threats", weight: 85 + (fleet.pirate ? 3 : 0), tone: "warn", icon: "warning",
      title: `${fleet.pirate ? "Pirate" : "Hostile"} Interceptors · near ${systemName(near.id)}`,
      facts: [`${size} ships`, closing ? `closing · ~${fmtDur(near.distance / speed)}` : `${Math.round(near.distance).toLocaleString()} su · holding`].join(" · "),
      hint: fleet.composition ? "A delayed sighting." : "A delayed sighting; its size is an estimate outside your sensor coverage.",
      age: fleet.age, actions: [show(near.id)] });
  }
  hostiles.sort((a, b) => (b.age ?? 0) - (a.age ?? 0)).slice(0, MAX_HOSTILE_ITEMS).forEach((item) => out.push(item));
  for (const report of st.captureReports) {
    if (now - report.learned_at > REPORT_RECENT_S) continue;
    out.push({ key: `capture:${report.id}`, group: report.captor ? "opportunities" : "threats", weight: report.captor ? 28 : 82,
      tone: report.captor ? "info" : "bad", icon: report.captor ? "captured" : "lost",
      title: report.captor ? `Captured ${locName(report.pos)}` : `Lost ${locName(report.pos)}`, facts: "",
      hint: report.captor ? "Territory taken, plunder seized." : "Rival marines landed at full siege and took the system.",
      age: report.learned_at,
      actions: [{ label: "Open report", go: { to: "panel", panel: "battle", params: { id: String(report.id), report: "capture", label: "Capture report" } } }] });
  }

  // ── The Authority's demands (§season) ──
  const squeeze = st.charter?.squeeze;
  if (squeeze?.rise) {
    const rise = squeeze.rise;
    out.push({ key: `levy:${rise.due}`, group: "threats", weight: 90, tone: "warn", icon: "credits",
      title: `Levy rise · +${Math.round(rise.points * 100)}%`,
      facts: `${label(rise.commodity)} · ${Math.round(rise.trade).toLocaleString()} Cr trade · ${fmtDur(Math.max(0, rise.due - now))} to answer`,
      hint: `Accept: your levy on every Hub trade rises. Refuse: ${label(rise.commodity)} is embargoed at the Hub until ${Math.round(rise.arrears).toLocaleString()} Cr arrears are paid. Unanswered, it is accepted.`,
      actions: [
        { label: "Accept", go: { to: "send", command: { type: "AnswerLevy", accept: true }, done: "Levy accepted · answer on its way to the Hub." } },
        { label: "Refuse", danger: true, go: { to: "send", command: { type: "AnswerLevy", accept: false }, done: "Levy refused · answer on its way to the Hub." } },
        { label: "Faction", go: { to: "panel", panel: "faction" } },
      ] });
  }
  for (const req of squeeze?.requisitions ?? []) {
    const what = demandText(req.demand, label);
    out.push({ key: `requisition:${req.id}`, group: "threats", weight: 89, tone: "warn", icon: "warning",
      title: `Requisition · ${what}`,
      facts: `${req.demand.kind === "goods" ? "from your Hub warehouse · " : ""}${fmtDur(Math.max(0, req.due - now))} left`,
      hint: `Paid: standing holds. Refused or lapsed: standing falls and your levy rises; three refusals bring an Authority squadron.${squeeze!.refusals ? ` Refused so far: ${squeeze!.refusals}.` : ""}`,
      actions: [
        { label: "Pay", go: { to: "send", command: { type: "AnswerRequisition", id: req.id, pay: true }, done: `Requisition paid · ${what} on its way to the Hub.` } },
        { label: "Refuse", danger: true, go: { to: "send", command: { type: "AnswerRequisition", id: req.id, pay: false }, done: "Requisition refused." } },
        { label: "Faction", go: { to: "panel", panel: "faction" } },
      ] });
  }
  // §season phase 4: the relic journey.
  const season = st.season;
  if (season) {
    const known = galaxy.systems.map((system) => ({ id: system.id, pos: system.pos }));
    for (const step of journeySteps(season, st.charter?.squeeze?.resolve ?? 0, known, systemName, now, attuneSecs(season.maturity))) {
      out.push({ key: `journey:${step.key}`, group: step.key === "leak" ? "threats" : "opportunities",
        weight: step.key === "leak" ? 97 : 65, tone: step.key === "leak" ? "bad" : "info", icon: step.key === "leak" ? "warning" : "intel",
        title: step.title, facts: step.facts,
        hint: "Relics' bearings cross over the sealed Precursor worlds. Finding one tells you its Keystone can close every gate; an Authority spy will tell the Authority.",
        actions: step.system ? [show(step.system, "Show")] : [{ label: "Faction", go: { to: "panel", panel: "faction" } }] });
    }
  }
  // §season phase 3: evidence a Scout can still read.
  for (const c of openEvidence(st.season, now)) {
    out.push({ key: `evidence:${c.id}`, group: "opportunities", weight: 60, tone: "info", icon: "intel",
      title: `Evidence · ${crackdownTitle(c.kind)}`,
      facts: `Scout to the scene · ${fmtDur(Math.max(0, c.evidence_until - now))} left`,
      hint: "Some crackdowns are staged. A Scout at the scene reads the evidence: a false flag exposed lifts the crackdown and every colony's Resolve rises.",
      actions: c.system ? [show(c.system, "Show scene")] : [{ label: "Faction", go: { to: "panel", panel: "faction" } }] });
  }
  for (const embargo of squeeze?.embargoes ?? []) {
    out.push({ key: `embargo:${embargo.commodity}:${embargo.since}`, group: "supply", weight: 70, tone: "warn", icon: "market",
      title: `Embargo · ${label(embargo.commodity)}`,
      facts: `arrears ${Math.round(embargo.arrears).toLocaleString()} Cr`,
      hint: "No Hub trade or Authority freight of this good until the arrears are paid. Your own and your syndicate's freighters still carry it.",
      actions: [
        { label: "Pay arrears", go: { to: "send", command: { type: "PayArrears", commodity: embargo.commodity }, done: "Arrears paid · on their way to the Hub." } },
        { label: "Faction", go: { to: "panel", panel: "faction" } },
      ] });
  }

  // ── Supply ──
  for (const system of owned) {
    const name = systemName(system.id);
    if ((system.ally_garrison_ships ?? 0) > 0 && system.ally_garrison_fed === false) {
      out.push({ key: `garrison:${system.id}`, group: "supply", weight: 70, tone: "warn", icon: "garrison",
        title: `Ally garrison unfed · ${name}`, facts: `${system.ally_garrison_ships} ships · defense suspended`,
        hint: "Allied ships here don't defend until you supply Provisions.",
        actions: [logistics({ destination: system.id, commodity: "provisions" })] });
    }
    if (system.storage_cap > 0 && system.storage_used >= system.storage_cap * 0.85) {
      const full = system.storage_used >= system.storage_cap;
      out.push({ key: `storage:${system.id}`, group: "supply", weight: 55, tone: "warn", icon: "storage",
        title: `Storage ${full ? "full" : "nearly full"} · ${name}`, facts: `${system.storage_used.toLocaleString()} / ${system.storage_cap.toLocaleString()}`,
        hint: full ? "Production idles at the cap (nothing is lost). Ship goods out, automate it, or build an Orbital Warehouse."
          : "Plan a shipment or expand storage before production stops.",
        actions: [{ label: "Automate export", icon: "doctrine", go: { to: "panel", panel: "logistics", query: { source: system.id, destination: "hub" } } },
          { label: "Open production", go: { to: "panel", panel: "production", params: { id: system.id } } }] });
    }
    for (const body of system.bodies ?? []) {
      const grid = body.power;
      if (!grid || grid.power >= 0.999) continue;
      const fuel = grid.fuel ? label(grid.fuel) : "fuel";
      const plant = (body.structures.power_plant ?? 0) > 0;
      out.push({ key: `brownout:${system.id}:${body.id}`, group: "supply", weight: 57, tone: "warn", icon: "refinery",
        title: `Power ${Math.round(grid.power * 100)}% · ${body.name}`, facts: grid.no_fuel ? `plant out of ${fuel}` : plant ? "raise its plant" : "no power plant",
        hint: "Its mines, factories, yards and Academy slow down.",
        actions: [grid.no_fuel ? logistics({ destination: system.id, ...(grid.fuel ? { commodity: grid.fuel } : {}) })
          : { label: plant ? "Open world" : "Build plant", icon: "build", go: { to: "panel", panel: "world", params: { systemId: system.id, bodyId: String(body.id), worldLabel: body.name } } }] });
    }
    if (system.population > 0 && !system.habitat_fed) {
      out.push({ key: `habitat:${system.id}`, group: "supply", weight: 50, tone: "warn", icon: "habitat",
        title: `Food: ${label(system.food_state ?? "rationing")} · ${name}`, facts: "workforce slowed",
        hint: "Growth pauses until Provisions arrive. Ship some or set a standing order (nothing is lost, nobody dies).",
        actions: [logistics({ destination: system.id, commodity: "provisions" })] });
    }
    if (system.node?.awakened && !system.node.fed) {
      out.push({ key: `node:${system.id}`, group: "supply", weight: 68, tone: "warn", icon: "unfed",
        title: `${system.node.title} node unfed · ${name}`, facts: "bonus suspended",
        hint: `${nodeBonusDesc(system.node.bonus)} Ship its upkeep here or automate it (nothing is lost).`,
        actions: [logistics({ destination: system.id, commodity: "provisions" })] });
    }
    const volatiles = (system.stockpile ?? []).find((stack) => stack.commodity === "volatiles")?.units ?? 0;
    if (system.refinery_tier >= 1 && volatiles === 0) {
      out.push({ key: `refinery:${system.id}`, group: "supply", weight: 42, tone: "info", icon: "refinery",
        title: `Refinery idle · ${name}`, facts: "no Volatiles", hint: "Deuterium production stopped. Haul some in or automate it.",
        actions: [logistics({ destination: system.id, commodity: "volatiles" })] });
    }
  }

  // ── Fleets ──
  const engaged = new Set(st.battles.flatMap((battle) => battle.participants));
  for (const fleet of st.ghosts) {
    if (!fleet.own) continue;
    const name = `${shipKindLabel(fleet.kind)} fleet`;
    if (fleet.garrison_host && fleet.garrison_fed === false) {
      out.push({ key: `mygarr:${fleet.id}`, group: "fleets", weight: 40, tone: "warn", icon: "garrison",
        title: `Your garrison unfed · ${systemName(fleet.garrison_host)}`, facts: "not defending",
        hint: "The host is out of Provisions. Recall it, or wait for the host to resupply.", actions: [fleetGo(fleet.id)] });
    }
    if (fleet.stalled) {
      out.push({ key: `stranded:${fleet.id}`, group: "fleets", weight: 78, tone: "bad", icon: "fuel", title: `${name} out of fuel`, facts: "holding",
        hint: "Refuel it at a berth, or request Authority Astral Assistance from its fleet panel.", age: fleet.age, actions: [fleetGo(fleet.id)] });
    } else if (fleet.supplied === false) {
      out.push({ key: `unsupplied:${fleet.id}`, group: "fleets", weight: 72, tone: "warn", icon: "provisions", title: `${name} out of Provisions`,
        facts: "no new orders", hint: "It keeps its course and guns, but takes no new movement or attack order until supplied.",
        age: fleet.age, actions: [fleetGo(fleet.id)] });
    } else {
      const store = regimeStore(fleet), have = store.served;
      if (have !== null && store.capacity > 0 && have / store.capacity < 0.2) {
        out.push({ key: `fuel:${fleet.id}`, group: "fleets", weight: 66, tone: "warn",
          icon: store.fuel === "uranium" ? "uranium" : store.fuel === "antimatter" ? "antimatter" : "deuterium",
          title: `${name} low on ${label(store.fuel)}`, facts: `${Math.round(have)} / ${Math.round(store.capacity)}`,
          hint: "From the fleet's last served report.", age: fleet.age, actions: [fleetGo(fleet.id)] });
      }
    }
    const overdue = (st.pendingOrders.get(fleet.id) ?? []).filter((order) => !order.lost && now > order.response_at);
    if (overdue.length) {
      out.push({ key: `overdue:${fleet.id}:${overdue[0].id}`, group: "fleets", weight: 64, tone: "warn", icon: "echo",
        title: `${name} hasn't answered`, facts: `${overdue.length} order${overdue.length === 1 ? "" : "s"}`,
        hint: "Its response is overdue; only arrived fleet light can confirm delivery.", actions: [fleetGo(fleet.id)] });
    }
    const cargo = fleetCargoUnits(fleet);
    if (fleet.docked && cargo > 0) {
      out.push({ key: `loaded:${fleet.id}:${fleet.docked}`, group: "fleets", weight: 52, tone: "info", icon: "cargo",
        title: `${name} docked with cargo`, facts: `${cargo.toLocaleString()} units`,
        hint: fleet.docked === "hub" ? "Unload or sell its manifest at the Market Hub." : "Unload it, send it to market, or give it another haul.",
        age: fleet.age, actions: [fleetGo(fleet.id)] });
    }
    const parked = !fleet.docked && !fleet.guard_target && !engaged.has(fleet.id) && !st.orders[fleet.id]
      && !(fleet.path?.length) && !(st.pendingOrders.get(fleet.id)?.length) && Math.hypot(fleet.vel.x, fleet.vel.y) < 0.5;
    if (parked) {
      out.push({ key: `idlefleet:${fleet.id}`, group: "fleets", weight: 32, tone: "neutral", icon: "fleet", title: `${name} idle`, facts: "no orders",
        hint: "Undocked, with no served course, guard target or command in flight.", age: fleet.age, actions: [fleetGo(fleet.id)] });
    }
  }

  // ── Opportunities ──
  for (const proposal of st.diplomacy?.incoming ?? []) {
    out.push({ key: `treaty:${proposal.id}`, group: "opportunities", weight: 48, tone: "info", icon: "ally",
      title: `${proposal.name} proposes ${label(proposal.treaty)}`, facts: proposal.expires_at > now ? `expires in ${fmtDur(proposal.expires_at - now)}` : "",
      hint: "A formal diplomatic change awaits your answer.",
      actions: [{ label: "Accept", go: { to: "send", command: { type: "RespondTreaty", proposal_id: proposal.id, accept: true }, done: "Treaty accepted." } },
        { label: "Decline", go: { to: "send", command: { type: "RespondTreaty", proposal_id: proposal.id, accept: false }, done: "Treaty declined." } }] });
  }
  for (const invite of st.syndicateInvites) {
    out.push({ key: `invites:${invite.id}`, group: "opportunities", weight: 38, tone: "info", icon: "syndicate",
      title: `Invitation · ${invite.name}`, facts: "", hint: "A syndicate invited you to join.",
      actions: [{ label: "Join", go: { to: "send", command: { type: "AcceptSyndicateInvite", syndicate_id: invite.id }, done: `Joining ${invite.name}.` } },
        { label: "Open syndicate", go: { to: "panel", panel: "syndicate" } }] });
  }
  const offered = st.operations.filter((operation) => operation.state === "offered" || (operation.state === "active" && !operation.joined));
  if (offered.length) {
    const soonest = Math.min(...offered.map((operation) => operation.expires_at));
    out.push({ key: `operations:${offered.map((operation) => operation.id).join(",")}`, group: "opportunities", weight: 39, tone: "info", icon: "manifest",
      title: `${offered.length} operation${offered.length === 1 ? "" : "s"} offered`, facts: soonest > now ? `first expires in ${fmtDur(soonest - now)}` : "",
      hint: "Arrived contracts and strategic opportunities, until they expire.", actions: [{ label: "Open", go: { to: "panel", panel: "operations" } }] });
  }
  for (const system of st.systems) {
    const tier = system.intel?.enclave_tier ?? 0;
    if (tier <= 0) continue;
    const site = pirateSite(tier, st.founding?.den === system.id, st.founding?.pirates?.surname)!;
    out.push({ key: `enclave:${system.id}`, group: "opportunities", weight: 58, tone: "warn", icon: "raider",
      title: `${site.title} · ${systemName(system.id)}`, facts: "", hint: site.goal, age: system.intel?.observed_at, actions: [show(system.id)] });
  }
  for (const system of owned) {
    const name = systemName(system.id);
    const total = (system.stockpile ?? []).reduce((sum, stack) => sum + stack.units, 0);
    const covered = active.some((order) => order.source.kind === "system" && order.source.id === system.id);
    if (total >= IDLE_UNITS && !covered && !(system.storage_cap > 0 && system.storage_used >= system.storage_cap)) {
      out.push({ key: `idle:${system.id}`, group: "opportunities", weight: 30, tone: "info", icon: "market",
        title: `${total.toLocaleString()} units without a route · ${name}`, facts: "", hint: "No standing order ships from here; automate it so it works while you're away.",
        actions: [{ label: "Automate export", icon: "doctrine", go: { to: "panel", panel: "logistics", query: { source: system.id, destination: "hub" } } },
          { label: "Open production", go: { to: "panel", panel: "production", params: { id: system.id } } }] });
    }
    if (system.slots_total > 0 && system.slots_used === 0 && system.builds.length === 0) {
      out.push({ key: `queue:${system.id}`, group: "opportunities", weight: 34, tone: "info", icon: "build",
        title: `Nothing built · ${name}`, facts: `${system.slots_total} slots free`, hint: "Develop it: Mining Complex, Orbital Warehouse, Sensor…",
        actions: [{ label: "Build", go: { to: "panel", panel: "build", params: { id: system.id } } }] });
    }
  }
  // §node: a midgame prize. Nothing during the charter, then one row for the
  // nodes near a system you own, nearest first (never the galaxy's count).
  const charterDone = !st.founding || st.founding.stage === "complete";
  const homes = owned.map((system) => sysPos(system.id)).filter((pos): pos is Vec2 => pos !== null);
  const reach = (id: string): number => {
    const pos = sysPos(id);
    return pos && homes.length ? Math.min(...homes.map((home) => Math.hypot(home.x - pos.x, home.y - pos.y))) : Infinity;
  };
  const nearNodes = charterDone ? st.systems.filter((system) => system.node && reach(system.id) <= NODE_NEARBY_SU) : [];
  const awakenIn = (galaxy.node_awakening_time ?? 0) - now;
  if (nearNodes.some((system) => !system.node!.awakened) && awakenIn > 0) {
    out.push({ key: "nodes:awakening", group: "opportunities", weight: 44, tone: "info", icon: "intel",
      title: `Exotic nodes awaken in ${fmtDur(awakenIn)}`, facts: `${nearNodes.length} nearby`,
      hint: "First arrival claims an unowned node. Stage colony ships and fleets now.", actions: [] });
  }
  const open = nearNodes.filter((system) => system.node!.awakened && !system.owner)
    .map((system) => ({ system, distance: reach(system.id) })).sort((a, b) => a.distance - b.distance);
  if (open.length) {
    const { system: nearest, distance } = open[0];
    out.push({ key: `nodeopen:${nearest.id}`, group: "opportunities", weight: 41, tone: "info", icon: "claim",
      title: open.length === 1 ? `${nearest.node!.title} node unclaimed · ${systemName(nearest.id)}` : `${open.length} exotic nodes nearby`,
      facts: [open.length > 1 ? `nearest ${systemName(nearest.id)}` : "", `${Math.round(distance).toLocaleString()} su`].filter(Boolean).join(" · "),
      hint: `${nearest.node!.title}: ${nodeBonusDesc(nearest.node!.bonus)} Send a colony ship: first arrival claims it.`,
      actions: [show(nearest.id, open.length > 1 ? "Show nearest" : "Show")] });
  }
  for (const [systemId, at] of freshSurveyReports) {
    const dynamic = st.systems.find((system) => system.id === systemId);
    const info = galaxy.systems.find((system) => system.id === systemId);
    if (!dynamic?.deposits || !info) continue;
    const roles = (dynamic.opportunities ?? []).slice(0, 2).map((opportunity) => `${opportunity.tier === "jackpot" ? "Jackpot · " : ""}${opportunity.title}`);
    const summary = dynamic.deposits.slice(0, 3).map((deposit) => `${label(deposit.resource)} ×${deposit.richness.toFixed(2)}`).join(" · ");
    out.push({ key: `surveyrep:${systemId}`, group: "opportunities", weight: 45, tone: "info", icon: "intel",
      title: `Survey report · ${systemName(systemId)}`, facts: roles.join(" · ") || summary || "barren",
      hint: [summary, dynamic.trait ? traitLine(dynamic.trait).title : "", dynamic.owner === null ? "Unowned: claim it and send a Colony Ship if it's worth holding." : ""].filter(Boolean).join(". "),
      age: at, actions: [show(systemId)] });
  }
  const nearSu = 3000;
  const ownedPositions = owned.map((system) => sysPos(system.id)).filter((pos): pos is Vec2 => pos !== null);
  const richUnsurveyed = st.systems.filter((system) => {
    if (system.deposits !== null) return false;
    const info = galaxy.systems.find((candidate) => candidate.id === system.id);
    return !!info && info.band === "rich" && ownedPositions.some((pos) => Math.hypot(pos.x - info.pos.x, pos.y - info.pos.y) <= nearSu);
  });
  if (richUnsurveyed.length) {
    out.push({ key: "surveyops", group: "opportunities", weight: 36, tone: "info", icon: "sensor",
      title: `${richUnsurveyed.length} rich system${richUnsurveyed.length === 1 ? "" : "s"} unsurveyed`, facts: `within ${nearSu.toLocaleString()} su`,
      hint: "Its deposits and rare features are unknown until surveyed.", actions: [show(richUnsurveyed[0].id, "Show nearest")] });
  }
  const allyIds = new Set(st.systems.filter((system) => system.ally).map((system) => system.id));
  for (const order of active) {
    const refs: string[] = [];
    if (order.source.kind === "system" && !ownedIds.has(order.source.id)) refs.push(systemName(order.source.id));
    if (order.dest.kind === "system" && !ownedIds.has(order.dest.id) && !allyIds.has(order.dest.id)) refs.push(systemName(order.dest.id));
    if (!refs.length) continue;
    out.push({ key: `order:${order.id}`, group: "supply", weight: 46, tone: "warn", icon: "doctrine",
      title: `Logistics rule ${order.id} points at a lost system`, facts: refs.join(" & "), hint: "Update or clear it.",
      actions: [{ label: "Open logistics", go: { to: "panel", panel: "logistics" } }] });
  }
  for (const report of st.battleReports) {
    if (st.battleViewed.has(reportMarkKey(report)) || now - report.learned_at > REPORT_RECENT_S) continue;
    out.push({ key: `report:${report.id}`, group: "threats", weight: 26, tone: "info", icon: "aftermath",
      title: `Battle report · near ${locName(report.pos)}`, facts: "", hint: "Losses and the outcome.", age: report.learned_at,
      actions: [{ label: "Open", go: { to: "panel", panel: "battle", params: { id: String(report.id), report: "battle", label: "Battle report" } } }] });
  }
  if (owned.length > 0 && active.length === 0 && out.length === 0) {
    out.push({ key: "noauto", group: "opportunities", weight: 20, tone: "info", icon: "doctrine", title: "No standing orders", facts: `${owned.length} system${owned.length > 1 ? "s" : ""}`,
      hint: "Automate supply so it works while you're away.", actions: [{ label: "Open logistics", go: { to: "panel", panel: "logistics" } }] });
  }
  const quiet = !!st.founding && st.founding.stage !== "complete" && !st.founding.bounty_received;
  const rows = quiet ? out.filter((item) => !CHARTER_QUIET.some((prefix) => item.key.startsWith(prefix))) : out;
  const order = (group: InboxGroup) => INBOX_GROUPS.findIndex((entry) => entry.key === group);
  return rows.sort((a, b) => order(a.group) - order(b.group) || b.weight - a.weight || a.key.localeCompare(b.key));
}

/** §log: the newest raid warning per home whose episode hasn't moved on. */
export function latestRaidWarnings(timeline: readonly TimelineEntry[], now: number): [EntityId, TimelineEntry][] {
  const latest = new Map<string, TimelineEntry>();
  for (const entry of timeline) {
    if (entry.kind !== "raid" || !entry.group || !entry.refs?.system) continue;
    const seen = latest.get(entry.group);
    if (!seen || entry.at_time >= seen.at_time) latest.set(entry.group, entry);
  }
  return [...latest.values()]
    .filter((entry) => entry.phase === "inbound" && now - entry.at_time < RAID_WARNING_STALE_S
      && !(entry.data?.eta && now > entry.data.eta + 300))
    .map((entry) => [entry.refs!.system!, entry]);
}

// ── Dismiss, snooze and History's read mark, per corporation ──

export interface InboxPrefs {
  /** Keys dismissed until they leave the inbox. */
  dismissed: string[];
  /** Key → sim-time it comes back. */
  snoozed: Record<string, number>;
  /** History rows newer than this sim-time are unread. */
  readThrough: number | null;
  /** Inbox keys already shown (the Log's badge counts the rest). */
  seen: string[];
  /** §season: News items heard after this sim-time are unread. */
  newsRead?: number | null;
}

/** One hour of the game clock. */
export const SNOOZE_S = 3600;
const prefsKey = (player: string) => `ss.log.v1.${player}`;

export function loadInboxPrefs(player: string | null): InboxPrefs {
  const empty: InboxPrefs = { dismissed: [], snoozed: {}, readThrough: null, seen: [] };
  if (!player) return empty;
  try {
    const saved = JSON.parse(localStorage.getItem(prefsKey(player)) ?? "null") as Partial<InboxPrefs> | null;
    return { dismissed: saved?.dismissed ?? [], snoozed: saved?.snoozed ?? {}, readThrough: saved?.readThrough ?? null, seen: saved?.seen ?? [],
      ...(saved?.newsRead != null ? { newsRead: saved.newsRead } : {}) };
  } catch {
    return empty;
  }
}

export function saveInboxPrefs(player: string | null, prefs: InboxPrefs): void {
  if (!player) return;
  try {
    localStorage.setItem(prefsKey(player), JSON.stringify(prefs));
  } catch { /* storage is a convenience */ }
}

/** The items a player hasn't dismissed or snoozed. With `prune` (a loaded
 * view, so an absent item really left) it forgets keys that left: a
 * dismissed condition that clears and comes back is news again. */
export function visibleInbox(items: readonly InboxItem[], prefs: InboxPrefs, now: number, prune = true): InboxItem[] {
  if (prune) {
    const keys = new Set(items.map((item) => item.key));
    prefs.dismissed = prefs.dismissed.filter((key) => keys.has(key));
    prefs.seen = prefs.seen.filter((key) => keys.has(key));
    for (const [key, until] of Object.entries(prefs.snoozed)) if (until <= now || !keys.has(key)) delete prefs.snoozed[key];
  }
  return items.filter((item) => !prefs.dismissed.includes(item.key) && !(prefs.snoozed[item.key] > now));
}
