import type { Net } from "../net";
import type { GhostView, Vec2 } from "../protocol";
import { renderer } from "../render";
import { state, type PendingIntent } from "../state";
import { emplacementLabel, knownDeposits } from "./derive/geo";
import { guardCapable, jumpCapable, shipKindLabel } from "./derive/fleet";
import { jumpRangeAt, nebulaAt } from "./derive/nebula";
import { intentTargetLabel, SURVEY_SECS_UI } from "./derive/orders";
import type { CoreEvent } from "./events";
import { fleetCommandIntent, fleetCommandSummary, fleetCommandsValid, type FleetCommand } from "./fleetorders";

// §TCA: UI mirror of crates/sim/src/tca.rs::TCA_SOVEREIGN_RADIUS — keep in step.
// Used only to HEDGE the attack/raid readout when a target's last-seen position
// shelters in the bubble; the server judges the true position either way.
const TCA_SOVEREIGN_RADIUS = 900;

export const intentAiming: { jump: string | null; guard: string | null } = {
  jump: null,
  guard: null,
};

let netSource: () => Net | null = () => null;
let eventSink: (events: CoreEvent[]) => void = () => {};

export function bindIntentCore(
  net: () => Net | null,
  sink: (events: CoreEvent[]) => void,
): void {
  netSource = net;
  eventSink = sink;
}

const esc = (value: string): string => value.replace(
  /[&<>"]/g,
  (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[character]!,
);

function emitIntentChanged(options: {
  readout?: string;
  renderIntentBar?: boolean;
  refreshShip?: boolean;
} = {}): void {
  eventSink([{
    kind: "IntentChanged",
    intent: state.pendingIntent,
    jumpAiming: intentAiming.jump,
    guardAiming: intentAiming.guard,
    ...options,
  }]);
}

export function clearGuardAiming(preserveReadout = false): void {
  if (intentAiming.guard === null) return;
  intentAiming.guard = null;
  emitIntentChanged({
    readout: preserveReadout
      ? undefined
      : `<span class="dim">Guard targeting cancelled.</span>`,
  });
}

export function armGuardAiming(ship: GhostView): void {
  if (!guardCapable(ship)) return;
  clearJumpAiming(true);
  clearPendingIntent(true);
  intentAiming.guard = ship.id;
  emitIntentChanged({
    refreshShip: true,
    readout: `<b>Choose a fleet to guard.</b> Click another one of your fleet markers. ` +
      `<span class="dim">The Interceptor will form up, engage local threats, then resume its station.</span>`,
  });
}

export function clearJumpAiming(preserveReadout = false): void {
  if (intentAiming.jump === null) return;
  intentAiming.jump = null;
  renderer.jumpAimingShipId = null;
  renderer.stateVersion++;
  emitIntentChanged({
    readout: preserveReadout
      ? undefined
      : `<span class="dim">Jump aiming cancelled.</span>`,
  });
}

export function armJumpAiming(ship: GhostView): void {
  if (!state.galaxy || !jumpCapable(ship)) return;
  clearGuardAiming(true);
  clearPendingIntent(true);
  intentAiming.jump = ship.id;
  renderer.jumpAimingShipId = ship.id;
  renderer.stateVersion++;
  const range = jumpRangeAt(state.galaxy, ship.pos);
  const precursor = nebulaAt(state.galaxy, ship.pos)?.kind === "precursor_cloud";
  emitIntentChanged({
    refreshShip: true,
    readout: `<b>Jump drive armed.</b> Pick a point within ` +
      `<b>${Math.round(range).toLocaleString()} su</b>${precursor ? " <span class=\"dim\">(precursor field)</span>" : ""}. ` +
      `<span class="dim">Both ends must be clear of gravity wells · Esc cancels.</span>`,
  });
}

export function clearPendingIntent(preserveReadout = false): void {
  if (state.pendingIntent === null) return;
  state.pendingIntent = null;
  renderer.stateVersion++;
  let readoutHtml: string | undefined;
  if (!preserveReadout) {
    const selected = state.selectedShipId
      ? state.ghosts.find((ghost) => ghost.id === state.selectedShipId && ghost.own)
      : undefined;
    if (selected) {
      readoutHtml = `<b>${esc(shipKindLabel(selected.kind))}</b> selected — details in the panel. ` +
        `Click empty space to move it · click a <span style="color:#ff7a6b">rival</span> to raid · press <b>R</b> to recall.`;
    }
  }
  emitIntentChanged({ renderIntentBar: true, readout: readoutHtml });
}

function previewReadout(intent: PendingIntent): string {
  const target = intentTargetLabel(intent);
  const ship = state.ghosts.find((ghost) => ghost.id === intent.shipId && ghost.own);
  switch (intent.verb) {
    case "command": return "";
    case "move":
      return "";
    case "jump": {
      const range = ship ? jumpRangeAt(state.galaxy, ship.pos) : state.galaxy?.jump_range ?? 50_000;
      return `Jump preview from the fleet's <b>light-delayed sighting</b>. ` +
        `<span class="dim">The dashed circle is the estimated ${Math.round(range).toLocaleString()} su reach; ` +
        `the sim checks the true fleet, range, and gravity wells when the signal arrives. Jump fuel is unlimited during playtesting.</span>`;
    }
    case "raid":
      return `Raid preview: pursue <b>${esc(target)}</b> to intercept and steal cargo. ` +
        `<span class="dim">The target will be pursued from its true position when the order arrives.</span>`;
    case "attack":
      return `Attack preview: your fleet will pursue <b>${esc(target)}</b> into a <b>FULL battle</b> to destroy it; cargo is lost with the fleet.`;
    case "guard":
      return `Guard preview: form up with <b>${esc(target)}</b>. ` +
        `<span class="dim">The Interceptor reacts from its own sensors, breaks off to meet a threat, then resumes formation without another command-center round trip.</span>`;
    case "blockade":
      return `Blockade preview: take station at <b>${esc(target)}</b> and strangle its logistics; standing defense will contest it.`;
    case "demolish":
      return `Demolition preview: tear down <b>${esc(target)}</b>. ` +
        `<span class="dim">The fleet must hold station there to finish the job; driven off, the work resets.</span>`;
    case "survey":
      return `Survey preview: fly to <b>${esc(target)}</b> and dwell ~${SURVEY_SECS_UI}s — active sensing is LOUD. ` +
        `<span class="dim">The exact geology travels home at light speed.</span>`;
  }
}

export function beginPendingIntent(intent: PendingIntent): void {
  state.pendingIntent = intent;
  renderer.stateVersion++;
  emitIntentChanged({ renderIntentBar: true, readout: previewReadout(intent) });
}

export function beginFleetCommand(command: FleetCommand | FleetCommand[]): void {
  const intent = fleetCommandIntent(command, state);
  if (!intent) {
    // Selecting the existing setting cancels a different staged setting; it
    // must not leave a previously previewed Stealth order armed behind it.
    if (state.pendingIntent?.verb === "command"
      && fleetCommandsValid(Array.isArray(command) ? command : [command], state)) clearPendingIntent();
    return;
  }
  clearGuardAiming(true);
  clearJumpAiming(true);
  beginPendingIntent(intent);
}

function moveOrderReadout(ship: GhostView, dest: Vec2): string {
  const out = ship.age;
  let blind = "";
  if (ship.kind === "colony" && state.galaxy) {
    const near = state.galaxy.systems.find(
      (system) => Math.hypot(system.pos.x - dest.x, system.pos.y - dest.y) <= 150,
    );
    if (near && knownDeposits(near.id) === null) {
      blind = ` <span style="color:var(--warn)">Heading to <b>${esc(near.name)}</b> (${esc(near.band.toUpperCase())} band) — unsurveyed, claiming blind: its deposits, mineral grades and rare features are unknown.</span>`;
    }
  }
  return `Order away to <b>${esc(shipKindLabel(ship.kind))}</b>. ` +
    `Reaches it in <b>~${out.toFixed(0)}s</b> (your light), ` +
    `you'll see it respond <b>~${(out * 2).toFixed(0)}s</b> from now. ` +
    `<span class="dim">Estimated from a ${out.toFixed(0)}s-old sighting.</span>` + blind +
    (out > 8
      ? ` <span style="color:var(--warn)">That sighting is stale — the fleet has flown on since, and its picture will catch up in a rush once fresher light arrives.</span>`
      : "");
}

function jumpOrderReadout(ship: GhostView): string {
  const out = ship.age;
  const spool = state.galaxy?.jump_spool_s ?? 10;
  return `Jump order away to <b>${esc(shipKindLabel(ship.kind))}</b>. ` +
    `It should reach the fleet in <b>~${out.toFixed(0)}s</b>, spool for <b>~${spool.toFixed(0)}s</b>, then relocate instantly. ` +
    `<span class="dim">You see the departure and arrival only when their light reaches your command center.</span>`;
}

export function confirmPendingIntent(): void {
  const intent = state.pendingIntent;
  const ship = intent
    ? state.ghosts.find((ghost) => ghost.id === intent.shipId && ghost.own)
    : undefined;
  const net = netSource();
  if (intent?.verb === "command") {
    if (intent.commander !== state.playerId || !intent.commands || !fleetCommandsValid(intent.commands, state)) {
      clearPendingIntent();
      emitIntentChanged({ readout: "<b>Order cancelled</b> · the received fleet picture has changed." });
      return;
    }
    if (!net?.connected) {
      emitIntentChanged({ readout: "<b>Not connected</b> · confirm again after reconnecting." });
      return;
    }
    const summary = fleetCommandSummary(intent, state);
    // Consume before sending: repeated Confirm clicks cannot dispatch twice.
    // A cancel or a replacing preview never mutates orders/raids/telemetry.
    clearPendingIntent(true);
    for (const command of intent.commands) net.send(command);
    emitIntentChanged({ refreshShip: true, readout: `<b>Order sent</b> · ${esc(summary)}` });
    return;
  }
  if (!intent || !ship || !net) {
    clearPendingIntent();
    return;
  }
  const targetGhost = state.ghosts.find((ghost) => ghost.id === intent.targetId);
  const hubPos = state.galaxy?.hub;
  const targetPos = targetGhost?.pos ?? intent.dest;
  const believedSheltered = !!hubPos && !!targetPos
    && Math.hypot(targetPos.x - hubPos.x, targetPos.y - hubPos.y) < TCA_SOVEREIGN_RADIUS;
  const shelterNote = believedSheltered
    ? ` <span class="warn">Its last-seen position is inside the Authority's sovereign zone — the order will be refused unless it has left the bubble.</span>`
    : "";
  let readoutHtml: string | undefined;

  switch (intent.verb) {
    case "move":
      if (!intent.dest) break;
      {
        const ships = (intent.shipIds?.length ? intent.shipIds : [ship.id])
          .map((id) => state.ghosts.find((ghost) => ghost.id === id && ghost.own))
          .filter((ghost): ghost is GhostView => !!ghost);
        for (const member of ships) {
          net.send({ type: "MoveShip", ship_id: member.id, dest: intent.dest });
          state.orders[member.id] = intent.dest;
        }
        readoutHtml = ships.length > 1
          ? `<b>${ships.length} move orders sent</b> · each fleet receives its own signal and confirms on its own returning light. ` +
            `<span class="dim">Destination ${Math.round(intent.dest.x).toLocaleString()} · ${Math.round(intent.dest.y).toLocaleString()}.</span>`
          : moveOrderReadout(ship, intent.dest);
      }
      break;
    case "jump":
      if (!intent.dest) break;
      net.send({ type: "JumpShip", ship_id: ship.id, dest: intent.dest });
      delete state.orders[ship.id];
      readoutHtml = jumpOrderReadout(ship);
      break;
    case "raid":
      if (!intent.targetId) break;
      net.send({ type: "CommitRaid", raider_id: ship.id, target_id: intent.targetId });
      net.send({ type: "EstimateEngagement", attacker: ship.id, target: intent.targetId });
      if (!believedSheltered) state.raids[ship.id] = intent.targetId;
      delete state.orders[ship.id];
      readoutHtml =
        `Raid committed: your <b>${esc(shipKindLabel(ship.kind))}</b> → rival <b>${esc(targetGhost?.kind ?? "contact")}</b>. ` +
        `The order sets off at light speed; your raider will pursue the rival's <i>true</i> position, ` +
        `not the <b>${(targetGhost?.age ?? 0).toFixed(0)}s</b>-old ghost you see. ` +
        (ship.composition?.some((stack) => stack.kind === "raider")
          ? `<span class="dim">Shift+click to ATTACK (destroy) instead · Press R to recall.</span>`
          : `<span class="dim">Press R to recall — it may arrive too late.</span>`) + shelterNote;
      break;
    case "attack":
      if (!intent.targetId) break;
      net.send({ type: "AttackFleet", fleet_id: ship.id, target_id: intent.targetId });
      net.send({ type: "EstimateEngagement", attacker: ship.id, target: intent.targetId });
      if (!believedSheltered) state.raids[ship.id] = intent.targetId;
      delete state.orders[ship.id];
      readoutHtml =
        `Attack committed: your <b>${esc(shipKindLabel(ship.kind))}</b> → rival <b>${esc(targetGhost?.kind ?? "contact")}</b> to <b>destroy</b> it. ` +
        `A FULL battle (a raid steals cargo; an attack kills — cargo is lost with the fleet). ` +
        `Light-delayed pursuit of its <i>true</i> position. <span class="dim">Press R to recall — it may arrive too late.</span>` + shelterNote;
      break;
    case "guard":
      if (!intent.targetId) break;
      net.send({ type: "GuardFleet", interceptor_id: ship.id, target_id: intent.targetId });
      delete state.raids[ship.id];
      delete state.orders[ship.id];
      readoutHtml =
        `Guard order sent: your <b>${esc(shipKindLabel(ship.kind))}</b> → <b>${esc(targetGhost ? shipKindLabel(targetGhost.kind) : "friendly fleet")}</b>. ` +
        `The assignment begins when the signal reaches the Interceptor; defensive reactions after that are local and automatic.`;
      break;
    case "blockade": {
      if (!intent.targetId) break;
      const system = state.galaxy?.systems.find((candidate) => candidate.id === intent.targetId);
      net.send({ type: "BlockadeSystem", fleet_id: ship.id, system_id: intent.targetId });
      delete state.orders[ship.id];
      readoutHtml =
        `Blockade ordered: your <b>raider fleet</b> → <b>${esc(system?.name ?? "target system")}</b>. ` +
        `It sets off at light speed to take station and strangle the system's logistics; ` +
        `standing defense will contest it. <span class="dim">Recall (R) to break off.</span>`;
      break;
    }
    case "demolish": {
      if (!intent.targetId) break;
      const target = state.emplacements.find((emplacement) => emplacement.id === intent.targetId);
      net.send({ type: "DemolishEmplacement", fleet: ship.id, target: intent.targetId });
      readoutHtml =
        `<b>${esc(shipKindLabel(ship.kind))}</b> ordered to tear down a rival <b>${esc(target ? emplacementLabel(target.kind) : "structure")}</b> ` +
        `<span class="dim">(signal outbound). It must hold station there to finish the job — ` +
        `driven off, the work resets.</span>`;
      break;
    }
    case "survey": {
      if (!intent.targetId) break;
      const system = state.galaxy?.systems.find((candidate) => candidate.id === intent.targetId);
      net.send({ type: "SurveySystem", fleet_id: ship.id, system_id: intent.targetId });
      delete state.orders[ship.id];
      readoutHtml =
        `Survey ordered: your <b>scout fleet</b> → <b>${esc(system?.name ?? "target system")}</b>${system ? ` (${esc(system.band.toUpperCase())} band)` : ""}. ` +
        `It flies on-site and dwells ~${SURVEY_SECS_UI}s — active sensing is LOUD (detectable farther). ` +
        `<span class="dim">The exact geology travels home at light speed; allies receive a relayed copy.</span>`;
      break;
    }
  }
  emitIntentChanged({ refreshShip: true, readout: readoutHtml });
  clearPendingIntent(true);
}
