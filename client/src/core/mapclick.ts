import { formatId, type EmplacementView, type GhostView, type SystemInfo, type Vec2 } from "../protocol";
import type { Renderer } from "../render";
import { JUMP_DEPARTURE_TTL_S, liveSimTime, type PendingIntent, type ViewState } from "../state";
import type { SystemBodyDetail } from "../systemview";
import { emplacementLabel, gravityWellAt, knownDeposits } from "./derive/geo";
import { guardCapable, jumpCapable, shipKindLabel } from "./derive/fleet";
import { jumpRangeAt } from "./derive/nebula";
import { armedSelection } from "./derive/orders";
import { jumpDepartureKey } from "./session";

export type EmplacementKind = EmplacementView["kind"];

export interface MapClickCtx {
  state: ViewState;
  renderer: Renderer;
  jumpAiming: string | null;
  guardAiming: string | null;
  emplaceArmed: EmplacementKind | null;
}

export type SelectTarget = (
  | { type: "fleet"; id: string }
  | { type: "jumpDeparture"; key: string }
  | { type: "emplacement"; id: string }
  | { type: "system"; id: string }
  | { type: "anchor"; readout: string }
  | { type: "hub" }
  | { type: "ongoingBattle"; id: string }
  | { type: "aftermath"; id: number }
  | { type: "capture"; id: number }
  | { type: "systemBody"; detail: SystemBodyDetail }
  | { type: "clearSystemBody" }
) & { readout?: string };

export type MapClickResult =
  | { kind: "intent"; intent: PendingIntent; clearAiming?: "jump" | "guard"; readout?: string }
  | { kind: "reject"; reason: string; clearAiming?: "jump" | "guard" }
  | { kind: "select"; target: SelectTarget }
  | { kind: "none" };

const esc = (value: string): string => value.replace(
  /[&<>"]/g,
  (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[character]!,
);

// §co-location cycling: the last selection click's spot + the stack it hit, so a
// repeat click at the same spot advances through co-located selectables instead
// of re-picking the same one. Reset implicitly whenever the spot or stack changes.
let clickCycle: { sx: number; sy: number; keys: string; index: number } | null = null;

export function resolveSystemClick(sx: number, sy: number, ctx: MapClickCtx): MapClickResult {
  const detail = ctx.renderer.systemPick(sx, sy);
  return detail
    ? { kind: "select", target: { type: "systemBody", detail } }
    : { kind: "select", target: { type: "clearSystemBody" } };
}

// The map CLICK decision (select own ship · select a star system incl. home ·
// inspect a command anchor · raid a rival ghost · move order to empty space).
// Shells apply the returned selection/intent/rejection; legality stays shared.
export function resolveMapClick(
  sx: number,
  sy: number,
  mods: { shift: boolean; long: boolean; inspect?: boolean },
  ctx: MapClickCtx,
): MapClickResult {
  const state = ctx.state;
  const renderer = ctx.renderer;
  const inspect = mods.inspect ?? false;

  // Jump aiming owns the next map click, including clicks over systems. The
  // preview is anchored to the SERVED ghost and public well geometry only;
  // truth is deliberately left to the sim when the order arrives.
  if (!inspect && ctx.jumpAiming && state.galaxy) {
    const ship = state.ghosts.find((ghost) => ghost.id === ctx.jumpAiming && ghost.own);
    if (!ship || !jumpCapable(ship)) {
      return {
        kind: "reject",
        reason: `<span style="color:var(--warn)">That fleet is no longer available for a jump.</span>`,
        clearAiming: "jump",
      };
    }
    const dest = renderer.screenToWorld(sx, sy);
    const distance = Math.hypot(dest.x - ship.pos.x, dest.y - ship.pos.y);
    const jumpRange = jumpRangeAt(state.galaxy, ship.pos);
    const originWell = gravityWellAt(ship.pos, state);
    const destinationWell = gravityWellAt(dest, state);
    if (distance > jumpRange + 1e-6) {
      return {
        kind: "reject",
        reason: `<span style="color:var(--warn)"><b>Out of jump range.</b> ` +
          `${Math.round(distance).toLocaleString()} su from the served sighting; maximum ` +
          `${Math.round(jumpRange).toLocaleString()} su.</span>`,
      };
    }
    if (originWell) {
      return {
        kind: "reject",
        reason: `<span style="color:var(--warn)"><b>Cannot spool here.</b> ` +
          `The served sighting is inside ${esc(originWell)}.</span>`,
      };
    }
    if (destinationWell) {
      return {
        kind: "reject",
        reason: `<span style="color:var(--warn)"><b>Cannot jump there.</b> ` +
          `The destination is inside ${esc(destinationWell)}.</span>`,
      };
    }
    return {
      kind: "intent",
      intent: { shipId: ship.id, verb: "jump", dest },
      clearAiming: "jump",
    };
  }

  // Explicit escort targeting owns the next map click. Only another OWN,
  // presently served fleet is a legal charge; the sim re-checks ownership
  // when the light-delayed order reaches the Interceptor.
  if (!inspect && ctx.guardAiming) {
    const interceptor = state.ghosts.find(
      (ghost) => ghost.id === ctx.guardAiming && guardCapable(ghost),
    );
    if (!interceptor) {
      return {
        kind: "reject",
        reason: `<span style="color:var(--warn)">That Interceptor is no longer available.</span>`,
        clearAiming: "guard",
      };
    }
    const target = state.ghosts
      .filter((ghost) => ghost.own && ghost.id !== interceptor.id && !ghost.docked)
      .map((ghost) => {
        const point = renderer.worldToScreen(ghost.pos);
        return { g: ghost, d: Math.hypot(point.x - sx, point.y - sy) };
      })
      .filter(({ g, d }) => d < Math.max(24, renderer.fleetHitRadius(g)))
      .sort((a, b) => a.d - b.d || a.g.id.localeCompare(b.g.id))[0]?.g;
    if (!target) {
      return {
        kind: "reject",
        reason: `<span style="color:var(--warn)"><b>Choose one of your fleet markers.</b> ` +
          `Docked fleets are assigned after they undock. <span class="dim">Esc cancels.</span></span>`,
      };
    }
    return {
      kind: "intent",
      intent: { shipId: interceptor.id, verb: "guard", targetId: target.id, dest: target.pos },
      clearAiming: "guard",
    };
  }

  // §contestable-territory Part 1: BLOCKADE PREVIEW. With one of your RAIDER
  // fleets selected, clicking a rival-owned system proposes a blockade there.
  if (!inspect) {
    const selF = state.selectedShipId
      ? state.ghosts.find((ghost) => ghost.id === state.selectedShipId)
      : undefined;
    if (selF && selF.own && selF.kind === "raider" && state.galaxy) {
      let hitSys: SystemInfo | null = null;
      let bestD = Infinity;
      for (const sys of state.galaxy.systems) {
        const point = renderer.worldToScreen(sys.pos);
        const distance = Math.hypot(point.x - sx, point.y - sy);
        if (distance < Math.max(15, renderer.systemHitRadius(sys)) && distance < bestD) {
          bestD = distance;
          hitSys = sys;
        }
      }
      const dyn = hitSys ? state.systems.find((system) => system.id === hitSys!.id) : undefined;
      const rival = dyn && dyn.owner !== null && dyn.owner !== state.playerId;
      if (hitSys && rival) {
        return {
          kind: "intent",
          intent: { shipId: selF.id, verb: "blockade", targetId: hitSys.id, dest: hitSys.pos },
        };
      }
    }

    // §explore Part 2 — SURVEY-ON-CLICK (the blockade idiom for the scout's
    // second job): a SCOUT-carrying own fleet selected + click an UNSURVEYED
    // system → order a survey.
    if (selF && selF.own && state.galaxy
      && (selF.composition ?? []).some((stack) => stack.kind === "scout" && stack.count > 0)) {
      let hitSys: SystemInfo | null = null;
      let bestD = Infinity;
      for (const sys of state.galaxy.systems) {
        const point = renderer.worldToScreen(sys.pos);
        const distance = Math.hypot(point.x - sx, point.y - sy);
        if (distance < Math.max(15, renderer.systemHitRadius(sys)) && distance < bestD) {
          bestD = distance;
          hitSys = sys;
        }
      }
      if (hitSys && knownDeposits(hitSys.id, state) === null) {
        return {
          kind: "intent",
          intent: { shipId: selF.id, verb: "survey", targetId: hitSys.id, dest: hitSys.pos },
        };
      }
    }
  }

  const SYSTEM_BIAS = 5;
  const CLICK_CYCLE_PX = 10;
  type Candidate = {
    key: string;
    sortD: number;
    label: string;
    target: SelectTarget;
    readout: string;
    enemy?: GhostView;
    ownFleet?: boolean;
  };
  const cands: Candidate[] = [];

  const engagedIds = new Map<string, Vec2>();
  for (const battle of state.battles) {
    for (const participant of battle.participants) engagedIds.set(participant, battle.pos);
  }
  const besieged = new Set(
    state.systems.filter((system) => system.blockade !== null).map((system) => system.id),
  );

  const selected = state.selectedShipId
    ? state.ghosts.find((ghost) => ghost.id === state.selectedShipId)
    : undefined;
  const haveOwn = !!selected && selected.own;
  const haveRaider = haveOwn && selected!.kind === "raider";
  const haveStrike = haveOwn
    && !!selected!.composition?.some((stack) => stack.kind === "raider");

  for (const ghost of state.ghosts) {
    const battlePos = engagedIds.get(ghost.id);
    // Ordinary berths live in the system/Hub and Fleets panels, not as
    // invisible targets over a star. A battle or blockade exposes the fleet on
    // the map again, matching the renderer's anti-concealment exceptions.
    if (ghost.docked && !battlePos && !besieged.has(ghost.docked)) continue;
    const point = battlePos
      ? renderer.worldToScreen(battlePos)
      : renderer.fleetScreenPosition(ghost);
    const distance = Math.hypot(point.x - sx, point.y - sy);
    const radius = Math.max(24, renderer.fleetHitRadius(ghost));
    if (distance >= radius) continue;
    if (ghost.own) {
      cands.push({
        key: `ship:${ghost.id}`,
        sortD: distance,
        label: shipKindLabel(ghost.kind),
        target: { type: "fleet", id: ghost.id },
        ownFleet: true,
        readout: `<b>${esc(shipKindLabel(ghost.kind))}</b> selected${ghost.docked ? " at its berth" : battlePos ? " in battle" : ""} — details in the panel. ` +
          `Click empty space to move it · click a <span style="color:#ff7a6b">rival</span> to raid · press <b>R</b> to recall.`,
      });
    } else {
      const contact = ghost.tca && ghost.kind === "freighter"
        ? ghost.rescue_service
          ? "AAA Rescue Tender"
          : ghost.migrant
            ? "Authority Migrant Liner"
            : "Authority Freighter"
        : shipKindLabel(ghost.kind);
      cands.push({
        key: `ship:${ghost.id}`,
        sortD: distance,
        label: contact,
        target: { type: "fleet", id: ghost.id },
        readout: `<b>${esc(contact)}</b> selected — its light-delayed details are in the panel.`,
        enemy: ghost,
      });
    }
  }

  const battleAtClick = renderer.battlePick(sx, sy);
  if (battleAtClick !== null) {
    // One engagement-id marker family, including the reliable-record handoff
    // while its ending is still arriving separately from the latest View.
    const battle = state.battles.find((candidate) => candidate.id === battleAtClick);
    const record = battle ? undefined : state.battleRecords.find((candidate) => candidate.id === battleAtClick);
    if (battle) {
      const point = renderer.worldToScreen(battle.pos);
      cands.push({
        key: `battle:${battle.id}`,
        sortD: Math.hypot(point.x - sx, point.y - sy),
        label: "ongoing battle",
        target: { type: "ongoingBattle", id: battle.id },
        readout: `<b>Battle in progress</b> — open the arrived combat picture.`,
      });
    } else if (record) {
      const point = renderer.worldToScreen(record.pos);
      cands.push({
        key: `battle:${record.id}`,
        sortD: Math.hypot(point.x - sx, point.y - sy),
        label: record.outcome !== null ? "concluded battle" : "battle report arriving",
        target: { type: "ongoingBattle", id: record.id },
        readout: record.outcome !== null ? `<b>Battle concluded</b> — open the recorded replay.` : `<b>Battle report arriving</b> — open the arrived combat picture.`,
      });
    }
  }

  const jumpNow = liveSimTime();
  for (const departure of state.jumpDepartures) {
    if (jumpNow - departure.learned_at >= JUMP_DEPARTURE_TTL_S) continue;
    const point = renderer.worldToScreen(departure.pos);
    const distance = Math.hypot(point.x - sx, point.y - sy);
    if (distance >= renderer.jumpDepartureHitRadius()) continue;
    const key = jumpDepartureKey(departure);
    const owner = departure.owner_name?.trim() || `Corporation ${formatId(departure.owner)}`;
    cands.push({
      key: `jump:${key}`,
      sortD: distance,
      label: `${shipKindLabel(departure.kind)} jump scar`,
      target: { type: "jumpDeparture", key },
      readout: `<b>Jump departure</b> selected — ${esc(owner)}'s ${esc(shipKindLabel(departure.kind))} fleet jumped away from here. ` +
        `<span class="dim">Destination unknown; details in the panel.</span>`,
    });
  }

  for (const emplacement of state.emplacements) {
    const point = renderer.worldToScreen(emplacement.pos);
    const distance = Math.hypot(point.x - sx, point.y - sy);
    if (distance >= renderer.emplacementHitRadius()) continue;
    const striker = emplacement.own ? undefined : armedSelection(state);
    const name = emplacementLabel(emplacement.kind);
    if (striker) {
      cands.push({
        key: `emp:${emplacement.id}`,
        sortD: distance,
        label: `demolish ${name}`,
        target: { type: "emplacement", id: emplacement.id },
        readout: `Demolition preview: tear down a rival <b>${esc(name)}</b>. ` +
          `<span class="dim">The fleet must hold station there to finish the job; driven off, the work resets.</span>`,
      });
    } else {
      cands.push({
        key: `emp:${emplacement.id}`,
        sortD: distance,
        label: name,
        target: { type: "emplacement", id: emplacement.id },
        readout: `<b>${esc(name)}</b> selected — details in the panel.`,
      });
    }
  }

  if (state.galaxy) {
    for (const system of state.galaxy.systems) {
      const point = renderer.worldToScreen(system.pos);
      const distance = Math.hypot(point.x - sx, point.y - sy);
      const radius = Math.max(15, renderer.systemHitRadius(system));
      if (distance < radius) {
        cands.push({
          key: `sys:${system.id}`,
          sortD: distance - SYSTEM_BIAS,
          label: system.name,
          target: { type: "system", id: system.id },
          readout: `<b>${esc(system.name)}</b> selected — details in the rail.`,
        });
      }
    }
  }

  if (cands.length) {
    // A visible fleet is a more precise hit than the enlarged star affordance
    // beneath it. Keep co-location cycling for the remaining stack, but make
    // the player's own hull the first result.
    cands.sort((a, b) => Number(!a.ownFleet) - Number(!b.ownFleet) || a.sortD - b.sortD);
    const keys = cands.map((candidate) => candidate.key).join(",");
    const previous = clickCycle;
    const same = previous !== null
      && Math.hypot(previous.sx - sx, previous.sy - sy) <= CLICK_CYCLE_PX
      && previous.keys === keys;
    const index = same ? (previous!.index + 1) % cands.length : 0;
    clickCycle = { sx, sy, keys, index };
    const chosen = cands[index];
    // A selected fleet turns a star into a named movement target. This is the
    // core map gesture: clicking the destination means "go there", including
    // colony ships whose exact claim point sits under the star's hit circle.
    // Blockade and survey clicks were resolved above and keep their own verbs.
    if (!inspect && chosen.target.type === "system" && haveOwn) {
      const systemId = chosen.target.id;
      const system = state.galaxy?.systems.find((candidate) => candidate.id === systemId);
      if (system) {
        return {
          kind: "intent",
          intent: { shipId: selected!.id, verb: "move", targetId: system.id, dest: system.pos },
          readout: `Move <b>${esc(shipKindLabel(selected!.kind))}</b> to <b>${esc(system.name)}</b>.`,
        };
      }
    }
    if (!inspect && chosen.enemy && (mods.shift || mods.long) && haveStrike) {
      return {
        kind: "intent",
        intent: {
          shipId: selected!.id,
          verb: "attack",
          targetId: chosen.enemy.id,
          dest: chosen.enemy.pos,
        },
      };
    }
    if (!inspect && chosen.enemy && cands.length === 1 && haveRaider) {
      return {
        kind: "intent",
        intent: {
          shipId: selected!.id,
          verb: "raid",
          targetId: chosen.enemy.id,
          dest: chosen.enemy.pos,
        },
      };
    }
    if (!inspect && chosen.target.type === "emplacement") {
      const emplacementId = chosen.target.id;
      const striker = armedSelection(state);
      const emplacement = state.emplacements.find((candidate) => candidate.id === emplacementId);
      if (striker && emplacement && !emplacement.own) {
        let demolitionReadout = chosen.readout;
        if (cands.length > 1) {
          const next = cands[(index + 1) % cands.length];
          demolitionReadout += ` <span class="dim">· ${cands.length} here — click again for <b>${esc(next.label)}</b>.</span>`;
        }
        return {
          kind: "intent",
          intent: {
            shipId: striker.id,
            verb: "demolish",
            targetId: emplacement.id,
            dest: emplacement.pos,
          },
          readout: demolitionReadout,
        };
      }
    }
    let message = chosen.readout;
    if (chosen.enemy && cands.length === 1 && haveStrike && !haveRaider) {
      message += ` <span class="dim">Shift+click it to ATTACK with your selected fleet.</span>`;
    }
    if (cands.length > 1) {
      const next = cands[(index + 1) % cands.length];
      message += ` <span class="dim">· ${cands.length} here — click again for <b>${esc(next.label)}</b>.</span>`;
    }
    return {
      kind: "select",
      target: { ...chosen.target, readout: message },
    };
  }

  let anchorPick = null as ViewState["anchors"][number] | null;
  let bestA = 14;
  for (const anchor of state.anchors) {
    const point = renderer.worldToScreen(anchor.pos);
    const distance = Math.hypot(point.x - sx, point.y - sy);
    if (distance < bestA) {
      bestA = distance;
      anchorPick = anchor;
    }
  }
  if (anchorPick) {
    const ownAnchor = anchorPick.owner !== null && anchorPick.owner === state.playerId;
    const message = ownAnchor
      ? `<b>Your command center</b> — your vantage on the galaxy. Everything you see is light-delayed from here; nothing reaches you faster than its light.`
      : anchorPick.owner !== null
        ? `<b>Rival command base</b> — a rival corporation commands from here. <span class="dim">You can see the base, but its systems, stockpiles &amp; orders never leak. To contest a rival, <b>claim and hold the star systems</b> around it.</span>`
        : `<b>Empty command site</b> — no corporation is based here.`;
    return { kind: "select", target: { type: "anchor", readout: message } };
  }

  if (state.galaxy) {
    const hub = renderer.worldToScreen(state.galaxy.hub);
    if (Math.hypot(hub.x - sx, hub.y - sy) < Math.max(24, renderer.hubHitRadius())) {
      if (!inspect && haveOwn) {
        return {
          kind: "intent",
          intent: { shipId: selected!.id, verb: "move", targetId: "hub", dest: state.galaxy.hub },
          readout: `Move <b>${esc(shipKindLabel(selected!.kind))}</b> to the <b>Market Hub</b>.`,
        };
      }
      return { kind: "select", target: { type: "hub" } };
    }
  }

  const battle = renderer.battlePick(sx, sy);
  if (battle !== null) return { kind: "select", target: { type: "ongoingBattle", id: battle } };

  const capture = renderer.capturePick(sx, sy);
  if (capture !== null) return { kind: "select", target: { type: "capture", id: capture } };

  if (!inspect && haveOwn) {
    return {
      kind: "intent",
      intent: { shipId: selected!.id, verb: "move", dest: renderer.screenToWorld(sx, sy) },
    };
  }
  return { kind: "none" };
}
