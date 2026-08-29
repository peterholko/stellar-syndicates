import { theaterAttach, theaterAvailable, theaterClose, theaterSetTime } from "../../battletheater";
import { affinityLine } from "../../core/derive/captains";
import { battleReportForRecord, battleViewerTimers, clearBattleAftermathTimer, clearBattleCloseTimer, HULL_MASS, recordForReport, type SalvoFamily, shipKindLabel, sideFamily, sumOwnComposition } from "../../core/derive/fleet";
import { arrivalLocal } from "../../core/derive/format";
import { nearestSystemName, systemName } from "../../core/derive/geo";
import { battleCommandDelay, latestPendingOrder, loadBattleMarks, saveBattleMarks } from "../../core/derive/orders";
import { groundTheaterAttach, groundTheaterAvailable, groundTheaterClose, groundTheaterSetTime, groundTheaterStep } from "../../groundtheater";
import { icon, type IconKey } from "../../icons";
import { type BattleRecordView, type CompCount, type CountClass, countClassLabel, type EntityId, type GhostView, type GroundRecordView, type KeyframeView, type ModuleKind, type RaidOutcome, type RecordCount, type RoundNoteView, type RoundRecordView, type ShipKind, type SideRecordView } from "../../protocol";
import { renderer } from "../../render";
import { liveSimTime, state } from "../../state";
import { net } from "./index";
import { $, badge, esc, renderDeferred, setHtml, svgIcon } from "./mapchrome";
import { openRail } from "./rail";
import { fmtCountdown, LIFECYCLE_MIN_S } from "./ship";
import { closePlanetPanel, closeSysviewManage, moduleIcon, showBreadcrumb } from "./sysview";
import { activateWorkspacePage, closeDesktopWorkspace, deactivateWorkspacePage } from "./workspace";


// --- §battle-aftermath: the battle-results panel + marker interaction --------
// Markers/reports come strictly from the owner-only `View.battle_reports`; the
// client only adds presentation state: VIEWED (marker dims) and DISMISSED
// (marker hidden — the report stays in the retained list). Both persist to
// localStorage so a reload keeps the read/dismissed status.
export function __init_battle_2931(): void {
loadBattleMarks();
}

// Clicking a top-center notification DISMISSES it (quick fade → remove). A
// battle report also opens its full results panel (the map marker + reports
// history persist — only the transient toast goes away). Delegated once on the
// persistent log root (§single-click pattern).
export function __init_battle_2936(): void {
$("reports-log").addEventListener("click", (e) => {
  const row = (e.target as HTMLElement).closest(".report") as HTMLElement | null;
  if (!row) return;
  if (row.dataset.reportId) openBattlePanel(Number(row.dataset.reportId));
  if (row.classList.contains("dismissing")) return; // already on its way out
  row.classList.add("dismissing");
  setTimeout(() => row.remove(), 200);
});
}


export let battlePanelBuilt = false;

export function buildBattlePanel(): void {
  if (battlePanelBuilt) return;
  battlePanelBuilt = true;
  $("battle-panel").addEventListener("click", (e) => {
    const el = (e.target as HTMLElement).closest("[data-act]") as HTMLElement | null;
    if (!el) return;
    if (el.dataset.act === "close") {
      if (openOngoingBattleId) battleForceHW.delete(openOngoingBattleId); // §perf: drop the loss tally
      openOngoingBattleId = null;
      renderer.selectedBattleMarkerId = null; // §aftermath-select: drop the ring
      deactivateWorkspacePage("battle-panel");
    } else if (el.dataset.act === "dismiss") {
      const id = Number(el.dataset.id);
      state.battleDismissed.add(id);
      if (renderer.selectedBattleMarkerId === id) renderer.selectedBattleMarkerId = null;
      saveBattleMarks();
      deactivateWorkspacePage("battle-panel");
    } else if (el.dataset.act === "withdraw" && net) {
      // §one-battle-one-icon: Withdraw an OWN engaged fleet straight from the
      // battle panel (its map marker is suppressed — no hidden sprite to hunt).
      const fleet = el.dataset.fleet;
      if (fleet) { net.send({ type: "Withdraw", fleet_id: fleet }); if (openOngoingBattleId) updateOngoingBattlePanel(); }
    } else if (el.dataset.act === "doctrine") {
      if (openOngoingBattleId) battleForceHW.delete(openOngoingBattleId); // §perf: drop the loss tally
      openOngoingBattleId = null;
      openRail("doctrine");
    } else if (el.dataset.act === "viewbattle" && el.dataset.record) {
      openBattleViewer(el.dataset.record);
    }
  });
}

// §one-battle-one-icon: the ongoing battle whose panel is open (client-local),
// so the View handler can keep its elapsed / echo countdowns / losses live.
export let openOngoingBattleId: string | null = null;

export function openBattlePanel(id: number): void {
  const r = state.battleReports.find((x) => x.id === id);
  if (!r) return; // rotated out of the retained list
  buildBattlePanel();
  state.battleViewed.add(id); // opening = viewed → the marker goes static/dim
  saveBattleMarks();
  const now = liveSimTime();
  const ago = (t: number) => fmtCountdown(Math.max(0, now - t));
  const youAtk = r.you === "attacker";
  const yourKind = youAtk ? r.attacker_kind : r.target_kind;
  const theirKind = youAtk ? r.target_kind : r.attacker_kind;
  const yourLoss = youAtk ? r.attacker_losses : r.target_losses;
  const theirLoss = youAtk ? r.target_losses : r.attacker_losses;
  const lossStr = (l: CompCount[]) => l.length ? l.map((c) => `${c.count} ${shipKindLabel(c.kind)}`).join(", ") : "nothing";
  // Outcome in the recipient's terms (victory / withdrawal / mutual disengage).
  const yourSideDied = r.outcome === "both_destroyed" || (youAtk ? r.outcome === "attacker_destroyed" : r.outcome === "target_destroyed");
  const theirSideDied = r.outcome === "both_destroyed" || (youAtk ? r.outcome === "target_destroyed" : r.outcome === "attacker_destroyed");
  const verdict = yourSideDied && theirSideDied ? badge("negative", "mutual destruction")
    : yourSideDied ? badge("negative", "defeat — your force destroyed")
      : theirSideDied ? badge("positive", "victory — their force destroyed")
        : badge("neutral", "withdrawal — both sides survive");
  const head =
    `<div class="pp-head"><div class="panel-title"><div><div class="eyebrow">battle result · delayed report</div>` +
    `<h2>Engagement ${esc(nearestSystemName(r.pos))}</h2></div></div>` +
    `<button class="pp-close" data-act="close" title="Close" aria-label="Close">✕</button></div>`;
  const body =
    `<div class="sp-line">${verdict}</div>` +
    `<div class="sp-sec">When</div>` +
    `<div class="sp-line">Concluded <b>${ago(r.at_time)}</b> ago · you learned <b>${ago(r.learned_at)}</b> ago <span class="dim">(light delay ${fmtCountdown(Math.max(0, r.learned_at - r.at_time))})</span></div>` +
    `<div class="sp-sec">Sides — as you learned them</div>` +
    `<div class="sp-line"><b>You</b> (${esc(youAtk ? "attacker" : "defender")}): ${esc(shipKindLabel(yourKind))}-led force</div>` +
    `<div class="sp-line"><b>Rival</b> (${esc(youAtk ? "defender" : "attacker")}): ${esc(shipKindLabel(theirKind))}-led force</div>` +
    `<div class="sp-sec" title="Outcomes are as of the light that reached your command center — the site may look different by now.">Losses</div>` +
    `<div class="sp-line">You lost: <b>${esc(lossStr(yourLoss))}</b></div>` +
    `<div class="sp-line">They lost: <b>${esc(lossStr(theirLoss))}</b></div>` +
    // §battle-records: watch the round-by-round replay (if its record is retained).
    ((): string => {
      const rec = recordForReport(r);
      return rec ? `<button class="act" data-act="viewbattle" data-record="${rec.id}" title="Watch the round-by-round replay of this battle.">${svgIcon("concept-fleet", "sm")} View battle replay</button>` : "";
    })() +
    `<button class="act" data-act="dismiss" data-id="${r.id}" title="Remove the map marker — the report stays in your log.">${icon("aftermath", "sm")} Dismiss marker</button>`;
  $("battle-panel").innerHTML = head + `<div class="pp-body">${body}</div>`;
  activateWorkspacePage("battle-panel");
}


// §one-battle-one-icon: open (and keep live) the ONGOING battle panel — clicking
// the single battle icon. Participants are shown AS KNOWN TO THE VIEWER (own
// fleets: full composition + the three verbs with echo countdowns; rivals:
// whatever the site-reveal already granted). Own engaged fleets are reachable
// here even though their map markers are suppressed.
export let lastOngoingBattleSig = "";

export function openOngoingBattlePanel(id: string): void {
  buildBattlePanel();
  openOngoingBattleId = id;
  lastOngoingBattleSig = ""; // force a fresh paint on open
  activateWorkspacePage("battle-panel");
  updateOngoingBattlePanel();
}

// §live-battle-panel running-loss tracking. Purely a HIGH-WATER of the viewer's
// ALREADY-DELIVERED light — never anything the ghosts didn't carry, so it can't
// leak: own fleets are tracked at EXACT counts (own light); rivals only at the
// site-revealed SIZE BUCKET (the fog never grants exact rival counts). Keyed by
// battle id; a distant viewer's staler ghosts naturally yield a laggier tally.
export type BattleForceHW = { own: Map<ShipKind, number>; rivalPeak: Map<EntityId, CountClass> };

export const battleForceHW = new Map<string, BattleForceHW>();

export const COUNT_CLASS_ORD: Record<CountClass, number> = {
  one: 0, two_to_three: 1, four_to_seven: 2, eight_to_fifteen: 3, sixteen_to_thirty: 4, thirty_one_plus: 5,
};


// Per-class ship glyph for the live force strip (reuses the shared UI icon set).
export const SHIP_ICON: Record<ShipKind, string> = {
  convoy: "concept-convoy", raider: "action-attack-raid", corvette: "concept-fleet",
  colony: "action-claim-system", scout: "action-survey-scout",
  // §ladder: no dedicated svg art yet — the fleet concept stands in (a real
  // capital sheet arrives separately; see PR note).
  destroyer: "concept-fleet", cruiser: "concept-fleet", battleship: "concept-fleet",
  dreadnought: "concept-fleet", titan: "concept-fleet",
  // §ground: a troopship is a settlement hull with rifles — reuse the claim glyph.
  transport: "action-claim-system",
  // §TCA: the Authority's common carrier — drawn with the hauler glyph.
  freighter: "concept-convoy",
  // §emplacements: the crane — the hauler glyph stands in.
  builder: "concept-convoy",
};

// One force-strip chip: a ship-class icon + the count still standing. `lost` (own,
// exact) draws a red "−k"; a fully-wiped class dims + strikes its count. `est`
// replaces the number with a fog bucket (rivals), and `shrunk` marks a bucket that
// visibly fell. The count IS the progress signal — it falls as the battle grinds.
export function shipChip(kind: ShipKind, count: number, opts: { lost?: number; est?: string; shrunk?: boolean } = {}): string {
  const wiped = opts.est === undefined && count <= 0;
  const num = opts.est ?? String(count);
  const tail = opts.lost && opts.lost > 0
    ? `<span class="fs-fallen">−${opts.lost}</span>`
    : opts.shrunk ? `<span class="fs-fallen">▾</span>` : "";
  return `<span class="fs-chip${wiped ? " lost" : ""}" title="${esc(shipKindLabel(kind))}">` +
    `${svgIcon(SHIP_ICON[kind], "md")}<span class="fs-n">${esc(num)}</span>${tail}</span>`;
}

// A labelled side of the force strip. `chips` empty → a dim placeholder.
export function forceSide(label: string, cls: string, chips: string): string {
  return `<div class="fs-row"><span class="fs-side ${cls}">${esc(label)}</span>` +
    `<span class="fs-chips">${chips || `<span class="fs-empty">—</span>`}</span></div>`;
}


export function updateOngoingBattlePanel(): void {
  const id = openOngoingBattleId;
  if (id === null) return;
  const b = state.battles.find((x) => x.id === id);
  const panel = $("battle-panel");
  if (!b) {
    // The battle's light now shows it CONCLUDED (it left the live set). Close;
    // the aftermath marker + report carry the outcome. Drop its loss tracking.
    battleForceHW.delete(id);
    openOngoingBattleId = null;
    deactivateWorkspacePage("battle-panel");
    return;
  }
  const now = liveSimTime();
  // Observed elapsed: the viewer sees the battle as of (now − age); it began at
  // started_at. Light-honest — never ahead of their light.
  const observed = Math.max(0, now - b.age - b.started_at);
  const parts = new Set(b.participants);
  const involved = state.ghosts.filter((g) => parts.has(g.id));
  const ownFleets = involved.filter((g) => g.own);
  const rivalFleets = involved.filter((g) => !g.own);
  const compStr = (g: GhostView): string => {
    const comp = g.composition ?? [];
    return comp.length ? comp.map((c) => `${c.count} ${shipKindLabel(c.kind)}`).join(", ") : shipKindLabel(g.kind);
  };

  // Advance the high-water tally from THIS view's already-delivered light.
  const hw: BattleForceHW = battleForceHW.get(id) ?? { own: new Map<ShipKind, number>(), rivalPeak: new Map<EntityId, CountClass>() };
  const ownNow = sumOwnComposition(ownFleets);
  for (const [k, n] of ownNow) hw.own.set(k, Math.max(hw.own.get(k) ?? 0, n));
  for (const g of rivalFleets) {
    const prev = hw.rivalPeak.get(g.id);
    if (prev === undefined || COUNT_CLASS_ORD[g.count_class] > COUNT_CLASS_ORD[prev]) hw.rivalPeak.set(g.id, g.count_class);
  }
  battleForceHW.set(id, hw);

  // §perf: the high-water accumulation ABOVE must run every View (never miss a
  // peak), but the chip/withdraw/countdown DOM below was rebuilt at 10 Hz. Gate the
  // rebuild on a content signature; the 1 s Math.floor(now) heartbeat keeps the
  // "raging / as-of / order-lag" countdowns ticking at their whole-second cadence
  // AND is the safety net so nothing the signature omits can stay stale beyond 1 s.
  const obSig = JSON.stringify([
    [...hw.own.entries()].sort(),
    [...hw.rivalPeak.entries()].sort(),
    [...ownNow.entries()].sort(),
    rivalFleets.map((g) => [g.id, g.count_class]),
    ownFleets.map((g) => g.id),
    b.own, b.participants.length,
    state.battleRecords.some((r) => r.id === b.id),
    Math.floor(now),
  ]);
  if (obSig === lastOngoingBattleSig && panel.innerHTML) return;
  lastOngoingBattleSig = obSig;

  // OWN force strip: one chip per ship CLASS still standing (exact, own light),
  // each carrying its running losses (peak − now) as a red "−k". Kinds sorted for
  // a stable order. A wiped class stays visible (dim, struck) so the toll shows.
  const ownChips = [...hw.own.keys()].sort().map((k) => {
    const cur = ownNow.get(k) ?? 0;
    return shipChip(k, cur, { lost: (hw.own.get(k) ?? 0) - cur });
  }).join("");
  // RIVAL force strip: one chip per site-revealed fleet — flagship glyph + fog
  // SIZE BUCKET (never an exact count), with a ▾ when the bucket has shrunk.
  const rivalChips = rivalFleets.map((g) => {
    const peak = hw.rivalPeak.get(g.id);
    const shrunk = peak !== undefined && COUNT_CLASS_ORD[peak] > COUNT_CLASS_ORD[g.count_class];
    return shipChip(g.kind, 0, { est: `~${countClassLabel(g.count_class)}`, shrunk });
  }).join("");

  // Compact per-fleet Withdraw (own engaged fleets) — the map markers are
  // suppressed, so these are the only handle. A tiny echo tag if an order is pending.
  const withdrawRow = ownFleets.length
    ? `<div class="wd-row">` + ownFleets.map((g) => {
        const pend = latestPendingOrder(g.id);
        let echo = "";
        if (pend && pend.response_at - pend.arrives_at >= LIFECYCLE_MIN_S) {
          const inTransit = now < pend.arrives_at;
          const timing = inTransit
            ? `▸${fmtCountdown(pend.arrives_at - now)}`
            : `◂${fmtCountdown(pend.response_at - now)}`;
          echo = ` <span class="fs-echo">${timing}</span>`;
        }
        return `<button class="wd-btn" data-act="withdraw" data-fleet="${g.id}" title="Break off ${esc(compStr(g))} and flee home — light-delayed">` +
          `↩ ${svgIcon(SHIP_ICON[g.kind], "sm")}<span class="fs-echo">${esc(compStr(g))}</span>${echo}</button>`;
      }).join("") + `</div>`
    : "";

  // §3 COMMAND DELAY, condensed to one line: one-way CC→anchor time + the local
  // wall-clock an order issued now would land at — plus a terse reach verdict.
  const delay = battleCommandDelay(b);
  const cmdDelayLine = delay !== null
    ? `<div class="sp-line dim">${svgIcon("action-standing-order", "sm")} Order lag <b style="color:var(--ink)">${fmtCountdown(delay)}</b> → lands ~${esc(arrivalLocal(delay))}` +
      (delay > 20 ? ` · <span style="color:#e88">too far to steer</span>` : ` · <span style="color:var(--accent)">still in reach</span>`) + `</div>`
    : "";

  const head =
    `<div class="pp-head"><div class="panel-title"><div><div class="eyebrow">${badge("negative", "battle raging")} · as of ${fmtCountdown(b.age)} ago</div>` +
    `<h2>Engagement ${esc(nearestSystemName(b.pos))}</h2></div></div>` +
    `<button class="pp-close" data-act="close" title="Close" aria-label="Close">✕</button></div>`;
  const ragingLine = `<div class="sp-line dim">Raging <b style="color:var(--ink)">${fmtCountdown(observed)}</b> · forces remaining by your light</div>`;
  // An ongoing engagement opens pinned to the arrived-light frontier. This is
  // deliberately DELAYED follow, never a window into true-space combat.
  const viewBtn = state.battleRecords.some((r) => r.id === b.id)
    ? `<button class="act" data-act="viewbattle" data-record="${b.id}" title="Follow new battle rounds as their light reaches you.">${svgIcon("concept-fleet", "sm")} Follow Battle · Delayed</button>`
    : "";
  const body =
    ragingLine +
    (b.own
      ? `<div class="force-strip">${forceSide("You", "you", ownChips)}${forceSide("Enemy", "foe", rivalChips)}</div>` +
        withdrawRow +
        cmdDelayLine +
        viewBtn +
        `<button class="act" data-act="doctrine" title="Change your corp fleet doctrine — the standing engage/retreat/escort policy your fleets follow.">${icon("doctrine", "sm")} Doctrine ▸</button>`
      : `<div class="force-strip">${forceSide("Forces", "foe", rivalChips)}</div>` +
        viewBtn +
        `<div class="mhint dim" title="You see this fight only by its weapons-fire light — you have no forces here.">no forces here</div>`);
  panel.innerHTML = head + `<div class="pp-body">${body}</div>`;
}


// --- §battle-records Part A3: the BATTLE VIEWER (the light-cone replay) --------
// A centered overlay (#battle-viewer) that plays a battle round-by-round from
// `state.battleRecords`. Because nothing outruns light, the replay IS the battle
// as far as the viewer is concerned: only the ARRIVED round prefix exists (up to
// `light_frontier_tick`); rounds beyond it draw as a hatched "beyond your light
// cone" zone, and a still-running fight pins playback LIGHT-LIVE to the frontier,
// flipping to the outcome chip when the end light lands. Participant fidelity
// shows exact bars + damage-dealt salvo arrows + shown-math; a bucket-fidelity
// third party sees CountClass labels only (no dealt, no tooltip) — the fog law.
export let openBattleViewerId: string | null = null;

export let bvRound = 0;
 // the round index currently shown
export let bvPlaying = false;

export let bvSpeed = 4;
 // completed replays: 1× | 4× | 16×; running fights have no playback rate
export let bvLive = false;
 // pinned to the arriving light frontier (a running battle)
export let bvAccum = 0;
 // fractional-round playback accumulator
export let bvLastTs = 0;

export let bvLoopRunning = false;

export let lastBattleViewerSig = "";
 // §perf: skip identical 10 Hz viewer rebuilds
export let bvSemantic = false;
 // entered by map zoom; presentation still follows the player's arrived light
export let bvClosing = false;

export let bvLastBattleAge: number | null = null;
 // last SERVED BattleView.age, never geometry-derived
export let bvLastFrontier = -1;

export let bvLastArrivalWallMs = 0;

export let bvHandoffArmed = false;
 // only a conclusion consumed while following auto-hands off
export const BV_ROUND_SECS = 0.55;
 // wall-seconds per round at 1× playback
export const BV_STALE_MS = 4500;

export const BV_TRANSITION_MS = 480;
 // matches render.ts's semantic-view crossfade

export const bvRecordFor = (id: string): BattleRecordView | undefined => state.battleRecords.find((r) => r.id === id);


export let battleViewerBuilt = false;

export function buildBattleViewer(): void {
  if (battleViewerBuilt) return;
  battleViewerBuilt = true;
  $("battle-viewer").addEventListener("click", (e) => {
    const el = (e.target as HTMLElement).closest("[data-act]") as HTMLElement | null;
    if (!el) return;
    const rec = openBattleViewerId ? bvRecordFor(openBattleViewerId) : undefined;
    const frontier = rec ? rec.rounds.length - 1 : -1;
    switch (el.dataset.act) {
      case "close":
        closeBattleViewer();
        break;
      case "play":
        if (rec?.outcome === null) break; // an unresolved fight only follows arrived light at real pace
        // Every transport action is an explicit replay decision. It drops the
        // arrival-frontier follow even if the user happens to be at that round.
        bvLive = false;
        bvHandoffArmed = false;
        if (!bvPlaying && rec && bvRound >= frontier) bvRound = 0;
        bvPlaying = !bvPlaying;
        bvAccum = 0;
        clearBattleAftermathTimer();
        renderBattleViewer();
        break;
      case "speed":
        if (rec?.outcome === null) break;
        bvLive = false;
        bvHandoffArmed = false;
        bvPlaying = false;
        bvSpeed = Number(el.dataset.speed) || 1;
        bvAccum = 0;
        clearBattleAftermathTimer();
        renderBattleViewer();
        break;
      case "round": {
        if (rec?.outcome === null) break;
        bvRound = Number(el.dataset.round) || 0;
        bvLive = false;
        bvHandoffArmed = false;
        bvPlaying = false;
        bvAccum = 0;
        clearBattleAftermathTimer();
        renderBattleViewer();
        break;
      }
    }
  });
  // The theater has its own tactical camera wheel. In semantic mode an outward
  // wheel gesture instead mirrors System View and returns to the galaxy; replay
  // overlays keep the theater's existing wheel controls unchanged.
  let zoomOutAccum = 0;
  $("battle-viewer").addEventListener("wheel", (e: WheelEvent) => {
    if (!bvSemantic) return;
    if (e.deltaY > 0) {
      zoomOutAccum += e.deltaY;
      if (zoomOutAccum > 60) {
        e.preventDefault();
        e.stopPropagation();
        zoomOutAccum = 0;
        closeBattleViewer();
      }
    } else {
      zoomOutAccum = 0;
    }
  }, { passive: false, capture: true });
}


// ============ §ground G3: THE GROUND VIEWER =================================
// The host shell around `groundtheater.ts`: playback clock, scrubber, and the
// same light-cone honesty as the battle viewer — the scrubber simply has no
// rounds beyond the frontier, because the record has none. Nothing here can
// show a player something that has not reached them.

export let groundViewerBuilt = false;

export let openGroundViewerId: string | null = null;

export let gvRound = 0;

export let gvFrac = 0;

export let gvLive = false;

export let gvPlaying = false;

export let gvLastTs = 0;

export let gvLoopRunning = false;

export let lastGroundViewerSig = "";


export const gvRecordFor = (id: string): GroundRecordView | undefined => state.groundRecords.find((r) => r.id === id);


// §ground G3: the "watch the landing" affordance is rendered by `groundLine`,
// which appears in TWO panels (the colony sheet and the system rail). One
// delegated listener on the document covers both, rather than duplicating the
// handler in each panel's own click router.
export let landingDelegateBound = false;

export function bindLandingDelegate(): void {
  if (landingDelegateBound) return;
  landingDelegateBound = true;
  document.addEventListener("click", (e) => {
    const el = (e.target as HTMLElement).closest("[data-act='watch-landing']") as HTMLElement | null;
    if (el?.dataset.landing) openGroundViewer(el.dataset.landing);
  });
}


export function buildGroundViewer(): void {
  if (groundViewerBuilt) return;
  groundViewerBuilt = true;
  $("ground-viewer").addEventListener("click", (e) => {
    const el = (e.target as HTMLElement).closest("[data-act]") as HTMLElement | null;
    if (!el) return;
    const rec = openGroundViewerId ? gvRecordFor(openGroundViewerId) : undefined;
    const frontier = rec ? rec.rounds.length - 1 : -1;
    switch (el.dataset.act) {
      case "close":
        closeGroundViewer();
        break;
      case "play":
        if (!gvPlaying && rec && rec.outcome !== null && gvRound >= frontier) { gvRound = 0; gvLive = false; }
        gvPlaying = !gvPlaying;
        renderGroundViewer();
        break;
      case "round":
        gvRound = Number(el.dataset.round) || 0;
        gvFrac = 0;
        gvLive = rec !== undefined && rec.outcome === null && gvRound >= frontier;
        gvPlaying = false;
        renderGroundViewer();
        break;
    }
  });
}


export function openGroundViewer(id: string): void {
  const rec = gvRecordFor(id);
  if (!rec) return; // no access → no viewer (fog); the affordance is guarded too
  buildGroundViewer();
  openGroundViewerId = id;
  const running = rec.outcome === null;
  const frontier = rec.rounds.length - 1;
  gvLive = running;
  gvRound = running ? Math.max(0, frontier) : 0;
  gvFrac = 0;
  gvPlaying = !running && frontier > 0; // auto-play a concluded landing from the top
  gvLastTs = 0;
  lastGroundViewerSig = "";
  renderGroundViewer();
  if (!gvLoopRunning) {
    gvLoopRunning = true;
    requestAnimationFrame(gvTick);
  }
}


export function closeGroundViewer(): void {
  openGroundViewerId = null;
  gvPlaying = false;
  $("ground-viewer").classList.remove("is-open");
  groundTheaterClose();
}


export function refreshOpenGroundViewer(): void {
  if (openGroundViewerId !== null) renderGroundViewer();
}


/// Playback clock. Like the battle viewer, a LIVE landing chases the light
/// frontier at the fight's real pace and holds when caught up; a concluded one
/// replays from the top.
export function gvTick(ts: number): void {
  if (openGroundViewerId === null) { gvLoopRunning = false; return; }
  const rec = gvRecordFor(openGroundViewerId);
  if (!rec) { closeGroundViewer(); gvLoopRunning = false; return; }
  const frontier = rec.rounds.length - 1;
  const dt = gvLastTs ? Math.min(0.25, (ts - gvLastTs) / 1000) : 0;
  gvLastTs = ts;
  if ((gvPlaying || gvLive) && frontier > 0) {
    const hz = state.tickHz || 30;
    const at = Math.min(gvRound, frontier - 1);
    const wnd = Math.max(0.2, (rec.rounds[at + 1].tick - rec.rounds[at].tick) / hz);
    gvFrac += dt / wnd;
    while (gvFrac >= 1 && gvRound < frontier) { gvRound++; gvFrac -= 1; }
    if (gvRound >= frontier) {
      gvFrac = 0;
      if (gvLive && rec.outcome !== null) { gvLive = false; gvPlaying = false; }
      else if (!gvLive) { gvPlaying = false; }
      renderGroundViewer();
    }
  }
  // Idle motion (falling fire, drifting scatter) runs regardless, so a held
  // frame still reads as a live battlefield rather than a screenshot.
  groundTheaterStep(dt);
  requestAnimationFrame(gvTick);
}


export function renderGroundViewer(): void {
  const id = openGroundViewerId;
  if (id === null) return;
  const rec = gvRecordFor(id);
  if (!rec) { closeGroundViewer(); return; }
  const panel = $("ground-viewer");
  panel.classList.add("is-open");
  const frontier = rec.rounds.length - 1;
  // Rebuild the chrome only when something structural changed — the canvas
  // repaints every frame regardless (§single-click: don't churn the DOM).
  const sig = JSON.stringify([id, rec.rounds.length, rec.outcome, gvRound, gvPlaying, gvLive]);
  if (sig !== lastGroundViewerSig) {
    lastGroundViewerSig = sig;
    const sysName = systemName(rec.system);
    const who = rec.attacking ? "Your landing" : "A landing on your ground";
    const verdict = rec.outcome === null
      ? badge("negative", "landing in progress")
      : rec.outcome === "taken"
        ? badge(rec.attacking ? "accent" : "negative", "ground taken")
        : badge(rec.attacking ? "negative" : "accent", "landing destroyed");
    const scrub = rec.rounds
      .map((r, i) => {
        const beat = (r.notes ?? []).length > 0;
        const cls = i === gvRound ? "is-at" : beat ? "is-beat" : "";
        const title = beat ? (r.notes ?? []).join(", ") : `round ${i + 1}`;
        return `<button data-act="round" data-round="${i}" class="${cls}" title="${esc(title)}"></button>`;
      })
      .join("");
    setHtml(panel,
      `<div class="panel-title"><div><div class="eyebrow">${esc(rec.fidelity === "participant" ? "ground assault" : "observed from orbit")}</div>` +
      `<h2>${esc(who)} — ${esc(sysName)}</h2></div>` +
      `<div class="panel-title__right">${verdict}<button class="pp-close" data-act="close">✕</button></div></div>` +
      `<div class="bv-sub">` +
      (rec.marines_landed !== null
        ? `<span><b>${rec.marines_landed}</b> marines landed against <b>${rec.defenders_initial}</b></span>`
        : `<span class="dim">Troop strengths are not resolvable from here — you are watching from orbit.</span>`) +
      `<span class="dim">· ${rec.garrison_tiers} garrison tier${rec.garrison_tiers === 1 ? "" : "s"}</span>` +
      `<span class="dim">· ${Math.round(rec.suppression_at_drop * 100)}% pinned at the drop</span>` +
      `</div>` +
      `<div class="gt-stage" id="gt-stage"></div>` +
      `<div class="gt-scrub">${scrub}</div>` +
      `<div class="bv-sub"><button class="pp-btn" data-act="play">${gvPlaying ? "❚❚ Pause" : "▶ Play"}</button>` +
      `<span class="dim">Round ${Math.min(gvRound + 1, rec.rounds.length)} of ${rec.rounds.length}` +
      (gvLive ? " · chasing your light cone — later rounds have not reached you yet" : "") +
      `</span></div>`);
    if (groundTheaterAvailable()) groundTheaterAttach($("gt-stage"), rec);
  }
  groundTheaterSetTime(gvRound, gvFrac, gvLive && gvRound >= frontier);
}


export type BattleViewerOpenOpts = { semantic?: boolean };


export function openBattleViewer(id: string, opts: BattleViewerOpenOpts = {}): void {
  const rec = bvRecordFor(id);
  if (!rec) return; // no access → no viewer (fog); the affordance is guarded too
  buildBattleViewer();
  clearBattleAftermathTimer();
  // A prior semantic close may still have its crossfade callback queued. It
  // must never be allowed to tear down a viewer opened during that interval.
  clearBattleCloseTimer();
  bvClosing = false;
  bvSemantic = opts.semantic === true;
  openBattleViewerId = id;
  const running = rec.outcome === null;
  const frontier = rec.rounds.length - 1;
  // Until conclusion light arrives, the viewer is the battle as known now:
  // it follows the arrived frontier at real battle pace and exposes no replay
  // speed. The 4× default belongs only to a completed historical record.
  bvLive = running;
  bvSpeed = running ? 1 : 4;
  bvHandoffArmed = bvSemantic && bvLive;
  bvRound = bvLive ? Math.max(0, frontier) : 0;
  bvPlaying = !bvLive && frontier > 0;
  bvAccum = 0;
  bvLastTs = 0;
  bvLastFrontier = frontier;
  bvLastArrivalWallMs = performance.now();
  bvLastBattleAge = state.battles.find((b) => b.id === id)?.age ?? null;
  lastBattleViewerSig = ""; // force a fresh paint on (re)open
  const viewer = $("battle-viewer");
  viewer.classList.toggle("is-semantic", bvSemantic);
  viewer.classList.remove("is-leaving");
  if (bvSemantic) {
    viewer.classList.add("is-entering");
    requestAnimationFrame(() => requestAnimationFrame(() => viewer.classList.remove("is-entering")));
  } else {
    viewer.classList.remove("is-entering");
  }
  renderBattleViewer();
  if (!bvLoopRunning) {
    bvLoopRunning = true;
    requestAnimationFrame(bvTick);
  }
}


export function finishBattleViewerClose(after?: () => void, semanticClosed = false): void {
  clearBattleCloseTimer();
  openBattleViewerId = null;
  bvPlaying = false;
  bvLive = false;
  bvHandoffArmed = false;
  bvClosing = false;
  const viewer = $("battle-viewer");
  viewer.classList.remove("is-open", "is-semantic", "is-entering", "is-leaving");
  // A compact replay can be opened over System View; closing it must not tear
  // down that view's breadcrumb. Only the semantic battle doorway owns it.
  if (semanticClosed) $("breadcrumb").classList.remove("is-open", "is-battle");
  theaterClose(); // stop the theater's ticker — the map is never affected
  after?.();
}


export function closeBattleViewer(after?: () => void): void {
  clearBattleAftermathTimer();
  // Renderer mode is the authoritative fallback. If a stale DOM lifecycle ever
  // loses `bvSemantic`, Back/Esc must still be able to leave the battle scene.
  const closesSemanticView = bvSemantic || renderer.viewMode.type === "battle";
  if (!closesSemanticView) {
    finishBattleViewerClose(after);
    return;
  }
  if (bvClosing) return;
  bvClosing = true;
  renderer.exitBattleView();
  $("battle-viewer").classList.add("is-leaving");
  battleViewerTimers.close = window.setTimeout(() => {
    battleViewerTimers.close = null;
    if (!bvClosing) return; // cancelled/reopened: this callback is stale
    bvSemantic = false;
    finishBattleViewerClose(after, true);
  }, BV_TRANSITION_MS);
}


/// Optional semantic-zoom doorway. Every input is already in the player's
/// light-gated view: the observed battle marker supplies both position and age,
/// and the record supplies only the arrived round prefix.
export function enterBattleViewer(id: string): void {
  // Do not reverse a semantic exit while its camera/overlay crossfade is still
  // active. Re-entry after the single 480 ms handoff is safe and deterministic.
  if (bvClosing) return;
  const battle = state.battles.find((b) => b.id === id);
  const rec = bvRecordFor(id);
  // Semantic zoom is a doorway into a battle happening in the player's served
  // picture, never into historical aftermath. Completed records keep the
  // ordinary compact replay door and cannot become a map LOD.
  if (!battle || !rec || rec.outcome !== null) return;
  renderer.enterBattleView(id, battle.pos);
  showBreadcrumb(`BATTLE ${nearestSystemName(battle.pos)}`);
  $("breadcrumb").classList.add("is-battle");
  closePlanetPanel();
  closeSysviewManage();
  closeDesktopWorkspace();
  openOngoingBattleId = null;
  openBattleViewer(id, { semantic: true });
}


/// Once the final arrived frame has played, wait for the ordinary delayed
/// aftermath report. Participants hand off through that existing panel; sensor
/// observers (who have no private report) remain on the concluded replay.
export function maybeScheduleBattleAftermath(rec: BattleRecordView): void {
  if (!bvSemantic || bvClosing || !bvHandoffArmed || bvLive || rec.outcome === null || battleViewerTimers.aftermath !== null) return;
  const report = battleReportForRecord(rec);
  if (!report) return;
  battleViewerTimers.aftermath = window.setTimeout(() => {
    battleViewerTimers.aftermath = null;
    closeBattleViewer(() => openBattlePanel(report.id));
  }, 900);
}


/// The playback clock — advances the shown round while playing, clamped to the
/// arrived light frontier. Self-stops when the viewer closes.
export function bvTick(ts: number): void {
  if (openBattleViewerId === null) { bvLoopRunning = false; return; }
  const rec = bvRecordFor(openBattleViewerId);
  if (!rec) { closeBattleViewer(); bvLoopRunning = false; return; }
  const frontier = rec.rounds.length - 1;
  if (frontier !== bvLastFrontier) {
    bvLastFrontier = frontier;
    bvLastArrivalWallMs = ts;
  }
  const battleAge = state.battles.find((b) => b.id === rec.id)?.age;
  if (battleAge !== undefined) bvLastBattleAge = battleAge;
  const dt = bvLastTs ? Math.min(0.25, (ts - bvLastTs) / 1000) : 0;
  if (bvLive && frontier >= 0) {
    // LIGHT-LIVE is a CHASE, not a slideshow: each newly-arrived round plays
    // through smoothly at the battle's REAL pace (wall-seconds per recorded
    // round), trailing the light frontier; the scene holds with idle drift
    // only when fully caught up. Everything shown has already arrived — the
    // light-cone law is untouched, this is presentation of arrived truth.
    if (bvRound < frontier) {
      const hz = state.tickHz || 30;
      const wnd = Math.max(0.2, (rec.rounds[bvRound + 1].tick - rec.rounds[bvRound].tick) / hz);
      bvAccum += dt / wnd;
      let changed = false;
      while (bvAccum >= 1 && bvRound < frontier) { bvRound++; bvAccum -= 1; changed = true; }
      if (bvRound >= frontier) bvAccum = 0; // caught up — hold at the newest light
      if (changed) renderBattleViewer();
    } else {
      bvAccum = 0;
      if (rec.outcome !== null) {
        // The ending's light arrived and its last window played out.
        bvLive = false;
        bvPlaying = false;
        bvSpeed = 4; // replay controls appear only now, with their historical default
        renderBattleViewer();
        maybeScheduleBattleAftermath(rec);
      }
    }
  } else if (bvPlaying && frontier >= 0) {
    bvAccum += (dt * bvSpeed) / BV_ROUND_SECS;
    let changed = false;
    while (bvAccum >= 1 && bvRound < frontier) { bvRound++; bvAccum -= 1; changed = true; }
    if (bvRound >= frontier) {
      bvAccum = 0;
      // A completed replay pauses when it reaches its final arrived round.
      bvPlaying = false;
      changed = true;
    }
    if (changed) renderBattleViewer();
  }
  if (!bvLive && rec.outcome !== null && bvRound >= frontier) maybeScheduleBattleAftermath(rec);
  // §theater: push transport time every frame — round + fractional progress
  // drives the theater's interpolation; LIGHT-LIVE pins it to the frontier.
  // bvAccum simply stops advancing while paused, so a pause holds the scene
  // mid-window instead of snapping it back to the keyframe.
  theaterSetTime(bvRound, Math.min(1, bvAccum), bvLive);
  bvLastTs = ts;
  requestAnimationFrame(bvTick);
}


/// Keep an open viewer live as new light arrives (called from the View handler).
export function refreshOpenBattleViewer(): void {
  if (openBattleViewerId === null || !$("battle-viewer").classList.contains("is-open")) return;
  const rec = bvRecordFor(openBattleViewerId);
  if (rec) {
    if (rec.outcome === null) {
      // A running fight cannot fall back into the replay transport as Views
      // refresh. It remains a read-only follow of the arrived-light frontier.
      bvLive = true;
      bvPlaying = false;
      bvSpeed = 1;
    }
    const battleAge = state.battles.find((b) => b.id === rec.id)?.age;
    if (battleAge !== undefined) bvLastBattleAge = battleAge;
    if (rec.rounds.length - 1 !== bvLastFrontier) {
      bvLastFrontier = rec.rounds.length - 1;
      bvLastArrivalWallMs = performance.now();
    }
  }
  renderBattleViewer();
}


export const bvRC = (arr: RecordCount[], k: ShipKind): RecordCount | undefined => arr.find((rc) => rc.kind === k);


// §modules B5: the salvo FAMILY typing. A side's dominant weapon = the hardest
// hitter it brought (torpedo > driver > beam), derived from its participant-only
// initial loadouts; drives the replay's salvo arrow color + label. `beam` is the
// stock default (unfitted brawlers / no weapon modules).
export const FAMILY_COLOR: Record<SalvoFamily, string> = { beam: "var(--accent)", driver: "#e8a13a", torpedo: "#e0574b" };

export const FAMILY_LABEL: Record<SalvoFamily, string> = { beam: "beam", driver: "drivers", torpedo: "torpedoes" };

// §tactical T3: the TRUTH MAP — an SVG top-down of the recorded keyframe.
// Real positions, torpedo salvos, and exact deaths; ship dots scale with mass
// class, dim with damage; platforms draw as emplacement squares. The viewer's
// own side is always cyan, the foe red (bearing-agnostic legibility).
export function bvTruthMap(f: KeyframeView, ownSide: number | null): string {
  const R = 1450; // arena + withdraw margin (battle-local coords)
  const sx = (x: number) => ((x + R) / (2 * R)) * 100;
  const sy = (y: number) => ((y + R) / (2 * R)) * 100;
  const colOf = (side: number) => (ownSide === null ? (side === 0 ? "#e0574b" : "#5ad1e0") : side === ownSide ? "#5ad1e0" : "#e0574b");
  const dots = f.ships.map((s) => {
    const r = s.plat ? 1.6 : Math.max(0.7, Math.min(2.6, Math.sqrt((HULL_MASS[s.kind] ?? 400) / 1000)));
    const o = (0.35 + 0.65 * Math.max(0, Math.min(1, s.hp))).toFixed(2);
    const c = colOf(s.side);
    return s.plat
      ? `<rect x="${(sx(s.x) - r).toFixed(1)}" y="${(sy(s.y) - r).toFixed(1)}" width="${(2 * r).toFixed(1)}" height="${(2 * r).toFixed(1)}" fill="${c}" opacity="${o}"><title>Defense Platform tier</title></rect>`
      : `<circle cx="${sx(s.x).toFixed(1)}" cy="${sy(s.y).toFixed(1)}" r="${r.toFixed(1)}" fill="${c}" opacity="${o}"><title>${esc(shipKindLabel(s.kind))} — ${Math.round(s.hp * 100)}% hull</title></circle>`;
  }).join("");
  const fish = f.torpedoes.map((t) =>
    `<g transform="translate(${sx(t.x).toFixed(1)},${sy(t.y).toFixed(1)})"><path d="M0,-1.6 L1.2,0 L0,1.6 L-1.2,0 Z" fill="${FAMILY_COLOR.torpedo}"><title>${t.n} torpedo${t.n > 1 ? "es" : ""} in flight</title></path>` +
    (t.n > 1 ? `<text x="1.8" y="1" font-size="3" fill="${FAMILY_COLOR.torpedo}">${t.n}</text>` : "") + `</g>`).join("");
  const deaths = f.deaths.map((d) =>
    `<g transform="translate(${sx(d.x).toFixed(1)},${sy(d.y).toFixed(1)})" opacity="0.85"><path d="M-1.4,-1.4 L1.4,1.4 M-1.4,1.4 L1.4,-1.4" stroke="${colOf(d.side)}" stroke-width="0.5"><title>${esc(shipKindLabel(d.kind))} destroyed here</title></path></g>`).join("");
  return `<div class="bv-truth" title="The recorded battle truth — real positions this round (participant intel).">` +
    `<svg viewBox="0 0 100 100" preserveAspectRatio="xMidYMid meet">` +
    `<circle cx="50" cy="50" r="${(1000 / R) * 50}" fill="none" stroke="rgba(255,255,255,0.10)" stroke-dasharray="2 2"/>` +
    dots + fish + deaths + `</svg></div>`;
}


// A compact per-stack fit summary for a side header (participant only).
// §fitting: stacks whose hull carries an AFFINITY for their fit show the named
// factor (law 4 — every multiplier is a legible line).
export function bvFitLine(sv: SideRecordView): string {
  const fits = sv.loadouts ?? [];
  // §ladder B4: a side's christened Titan leads its fit line — the one channel
  // a rival ever meets the name through (participant records only).
  const flag = sv.flagship_name
    ? `<span class="tone-up" title="This side's flagship Titan — participant intel.">⚑ ${esc(sv.flagship_name)}</span>`
    : "";
  if (!fits.length && !flag) return "";
  const parts = fits.map((st) => {
    const aff = affinityLine(st.kind, st.modules as ModuleKind[]);
    const mult = aff?.match(/×[\d.]+/)?.[0] ?? "×1.25";
    const tag = aff ? ` <span class="tone-up" title="${esc(aff)} — hull affinity, a named factor in this stack's damage.">${mult}</span>` : "";
    return `<span class="module-inline">${st.n}× ${st.modules.map((m) => moduleIcon(m as ModuleKind, "sm")).join("")} ${esc(shipKindLabel(st.kind))}${tag}</span>`;
  });
  return `<div class="bv-fits" title="What this side was fitted with — participant intel.">${[flag, ...parts].filter(Boolean).join(" · ")}</div>`;
}


/// One side's column: a per-kind survivor bar (participant: exact; bucket:
/// CountClass label), a kill flash, and the defender's platform block.
export function bvSideHtml(rec: BattleRecordView, rd: RoundRecordView, s: 0 | 1, participant: boolean, platGone: boolean): string {
  const mine = rec.own_side === s;
  const cls = `bv-side ${s === 1 ? "right " : ""}${mine ? "mine" : ""}`;
  const role = s === 0 ? "Attackers" : "Defenders";
  // §modules B5: a weapon-family pip (participant only) + the per-stack fit line.
  const fam = participant ? sideFamily(rec.sides[s]) : null;
  const famPip = fam
    ? ` <span class="bv-fampip" style="color:${FAMILY_COLOR[fam]}" title="This side's dominant weapon — its salvos are typed ${FAMILY_LABEL[fam]}.">● ${esc(FAMILY_LABEL[fam])}</span>`
    : "";
  const hd = `<div class="bv-side__hd">${mine ? badge("neutral", "you") : ""}${esc(role)}${famPip}</div>` +
    (participant ? bvFitLine(rec.sides[s]) : "");
  const rows = rec.sides[s].initial.map((op) => {
    const k = op.kind;
    const surv = bvRC(rd.counts[s], k);
    const kill = bvRC(rd.kills[s], k);
    const gone = surv === undefined;
    let pct: number;
    let nlabel: string;
    if (participant) {
      const openN = op.exact ?? 0;
      const survN = surv?.exact ?? 0;
      pct = openN > 0 ? (survN / openN) * 100 : 0;
      nlabel = `×${survN}`;
    } else {
      const ord = surv ? COUNT_CLASS_ORD[surv.class] : -1;
      pct = ord >= 0 ? ((ord + 1) / 6) * 100 : 0;
      nlabel = surv ? countClassLabel(surv.class) : "—";
    }
    const killTag = participant
      ? (kill?.exact ? ` <span class="bv-krow__kill">−${kill.exact}</span>` : "")
      : (kill ? ` <span class="bv-krow__kill">▾</span>` : "");
    const nStyle = gone ? ' style="text-decoration:line-through;color:var(--dim)"' : "";
    return `<div class="bv-krow" title="${esc(shipKindLabel(k))}">${svgIcon(SHIP_ICON[k], "sm")}` +
      `<div class="bv-krow__bar"><div class="bv-krow__fill${gone ? " gone" : ""}" style="width:${gone ? 100 : Math.max(5, pct)}%"></div></div>` +
      `<span class="bv-krow__n"${nStyle}>${esc(nlabel)}${killTag}</span></div>`;
  }).join("");
  const plat = s === 1 && rec.sides[1].platform_tiers > 0
    ? `<div class="bv-plat${platGone ? " gone" : ""}">${icon("defense", "sm")} Platform ×${rec.sides[1].platform_tiers}</div>`
    : "";
  return `<div class="${cls}">${hd}${rows}${plat}</div>`;
}


export const BV_NOTE_META: Record<string, { cls: string; icon: IconKey; text: (side: string) => string }> = {
  joined: { cls: "join", icon: "reinforce", text: (s) => `Reinforcements join the ${s.toLowerCase()}` },
  retreat_tripped: { cls: "retreat", icon: "withdraw", text: (s) => `The ${s.toLowerCase()} trip their retreat threshold — withdrawing` },
  withdraw_ordered: { cls: "retreat", icon: "withdraw", text: (s) => `A withdraw order reaches the ${s.toLowerCase()}` },
  disengage_exposure: { cls: "retreat", icon: "withdraw", text: (s) => `The ${s.toLowerCase()} break off — parting-shot exposure` },
  platform_destroyed: { cls: "", icon: "defense", text: () => `The Defense Platform is destroyed` },
  mutual_disengage: { cls: "", icon: "withdraw", text: () => `Mutual disengage — the grind breaks off` },
};

export function bvNoteBanner(n: RoundNoteView): string {
  const meta = BV_NOTE_META[n.kind] ?? { cls: "", icon: "battle" as IconKey, text: () => n.kind };
  const side = n.side === 0 ? "Attackers" : n.side === 1 ? "Defenders" : "";
  return `<div class="bv-note ${meta.cls}">${icon(meta.icon, "sm")} ${esc(meta.text(side))}</div>`;
}


/// The battle's outcome as a verdict chip. From the viewer's own side when a
/// participant; a neutral factual label for a bucket-fidelity third party.
export function bvOutcomeChip(rec: BattleRecordView, outcome: RaidOutcome): string {
  const atkDied = outcome === "attacker_destroyed" || outcome === "both_destroyed";
  const defDied = outcome === "target_destroyed" || outcome === "both_destroyed";
  if (rec.own_side === null) {
    const label = outcome === "both_destroyed" ? "mutual destruction"
      : atkDied ? "attackers destroyed"
        : defDied ? "defenders destroyed"
          : "both withdrew";
    return badge("neutral", label);
  }
  const youDied = rec.own_side === 0 ? atkDied : defDied;
  const themDied = rec.own_side === 0 ? defDied : atkDied;
  if (youDied && themDied) return badge("negative", "mutual destruction");
  if (youDied) return badge("negative", "defeat — your force destroyed");
  if (themDied) return badge("positive", "victory — their force destroyed");
  return badge("neutral", "both withdrew");
}


export function renderBattleViewer(): void {
  if (openBattleViewerId === null) return;
  if (renderDeferred("battle-viewer", renderBattleViewer)) return; // §single-click guard
  const rec = bvRecordFor(openBattleViewerId);
  if (!rec) { closeBattleViewer(); return; }
  const participant = rec.fidelity === "participant";
  const running = rec.outcome === null;
  const frontier = rec.rounds.length - 1;
  if (bvLive && frontier >= 0) bvRound = Math.min(bvRound, frontier); // the chase advances; never snap-jump
  bvRound = Math.max(0, Math.min(bvRound, Math.max(0, frontier)));
  const wallNow = performance.now();
  const stalled = running && bvLastArrivalWallMs > 0 && wallNow - bvLastArrivalWallMs >= BV_STALE_MS;
  const stale = running && ((bvLastBattleAge ?? 0) >= 8 || stalled);

  // §perf: this rebuilds the whole overlay AND re-mounts the WebGL canvas; it ran
  // 10x/s off the View while a replay was open. Skip when nothing that affects the
  // render changed — the 1 s liveSimTime() heartbeat keeps the "light reached you
  // N ago" line ticking at its whole-second cadence; new light (rounds.length),
  // scrubbing/playback (bvRound/bvPlaying/bvSpeed) and the outcome all invalidate.
  const bvSig = JSON.stringify([
    openBattleViewerId, rec.rounds.length, rec.outcome, bvRound, bvPlaying, bvSpeed,
    bvLive, bvSemantic, bvLastBattleAge === null ? null : Math.ceil(bvLastBattleAge),
    stale, Math.floor(wallNow / 1000),
  ]);
  if (bvSig === lastBattleViewerSig && $("battle-viewer").childElementCount) return;
  lastBattleViewerSig = bvSig;

  const head =
    `<div class="pp-head"><div class="panel-title"><div>` +
    `<div class="eyebrow">${svgIcon("concept-fleet", "sm")} ${running ? "battle · delayed observation" : "battle replay"}${rec.raid ? " · raid" : ""}${participant ? "" : " · sensor estimate"}</div>` +
    `<h2>Engagement ${esc(nearestSystemName(rec.pos))}</h2></div></div>` +
    `<button class="pp-close" data-act="close" title="Close (Esc)" aria-label="Close">✕</button></div>`;

  const label0 = rec.own_side === 0 ? "You" : "Attackers";
  const label1 = rec.own_side === 1 ? "You" : "Defenders";
  const statusChip = rec.outcome
    ? bvOutcomeChip(rec, rec.outcome)
    : badge("warn", "◉ FOLLOWING LIGHT");
  const counter = frontier < 0 ? "no rounds yet" : `round ${bvRound + 1} / ${rec.rounds.length}${running ? " +" : ""}`;
  const sub = `<div class="bv-sub"><span class="bv-vs"><span class="${rec.own_side === 0 ? "you" : "foe"}">${esc(label0)}</span> vs <span class="${rec.own_side === 1 ? "you" : "foe"}">${esc(label1)}</span></span> ${statusChip}<span class="bv-count">${esc(counter)}</span></div>`;
  const ageText = bvLastBattleAge === null ? "arrival frontier" : `as of ~${Math.ceil(bvLastBattleAge)}s ago`;
  const liveBar = running
    ? `<div class="bv-livebar${stale ? " is-stale" : ""}${bvLive ? " is-following" : ""}">` +
      `<span class="bv-livedot"></span><b>FOLLOWING</b> · real battle pace · ${esc(ageText)}` +
      `${stalled ? `<span class="bv-stall">light in transit · holding last arrival</span>` : ""}</div>`
    : `<div class="bv-livebar is-complete"><span class="bv-livedot"></span><b>COMPLETE</b> · conclusion light arrived</div>`;

  let arena = `<div class="bv-empty">Awaiting the first round's light…</div>`;
  let notes = "";
  let agoline = "";
  if (frontier >= 0) {
    const rd = rec.rounds[bvRound];
    const platGone = rec.rounds.slice(0, bvRound + 1).some((r) => r.notes.some((n) => n.kind === "platform_destroyed"));
    // Salvo gutter: arrows scaled by damage dealt (participant); a glyph for bucket.
    let salvos: string;
    if (participant && rd.dealt) {
      const maxDealt = Math.max(1e-6, ...rec.rounds.flatMap((r) => (r.dealt ? [r.dealt[0], r.dealt[1]] : [0])));
      const w = (d: number) => Math.max(8, (d / maxDealt) * 88);
      const mute = (d: number) => (d < maxDealt * 0.03 ? " mute" : "");
      // §modules B5: TYPE each salvo arrow by its firing side's weapon family.
      const famA = sideFamily(rec.sides[0]), famD = sideFamily(rec.sides[1]);
      salvos = `<div class="bv-salvos">` +
        `<div class="bv-arrow r${mute(rd.dealt[0])}" style="width:${w(rd.dealt[0])}%; background:${FAMILY_COLOR[famA]}" title="attackers' ${FAMILY_LABEL[famA]} dealt ${rd.dealt[0].toFixed(2)} this round"></div>` +
        `<div class="bv-arrow l${mute(rd.dealt[1])}" style="width:${w(rd.dealt[1])}%; margin-left:auto; background:${FAMILY_COLOR[famD]}" title="defenders' ${FAMILY_LABEL[famD]} dealt ${rd.dealt[1].toFixed(2)} this round"></div>` +
        `</div>`;
    } else {
      salvos = `<div class="bv-salvos" style="align-items:center;color:var(--dim)" title="exact fire strength is fogged — you see only the size buckets">⚔</div>`;
    }
    arena = `<div class="bv-arena">${bvSideHtml(rec, rd, 0, participant, false)}${salvos}${bvSideHtml(rec, rd, 1, participant, platGone)}</div>`;
    // §theater: participant records with truth keyframes get the FULL battle
    // theater (ship sprites + weapon FX, an interpolating replayer of real
    // positions). If the theater can't run (no WebGL / init failed), the SVG
    // truth map is the per-round fallback; frameless records keep the columns.
    if (rec.rounds.some((r) => r.frame) && theaterAvailable()) {
      arena += `<div class="bv-theater" id="bv-theater-mount"></div>`;
    } else if (rd.frame) {
      arena += bvTruthMap(rd.frame, rec.own_side);
    }
    notes = rd.notes.length ? `<div class="bv-notes">${rd.notes.map(bvNoteBanner).join("")}</div>` : "";
    const intoFight = Math.max(0, rd.tick / state.tickHz - rec.started_at);
    agoline = `<div class="bv-agoline">at +${fmtCountdown(intoFight)} into the fight · arrived record round ${bvRound + 1}${participant ? "" : " · size estimates only"}</div>`;
  }

  const playIcon = bvPlaying ? "❚❚ Pause" : "▶ Play";
  const speeds = [1, 4, 16].map((sp) => `<button class="bv-btn${bvSpeed === sp ? " on" : ""}" data-act="speed" data-speed="${sp}">${sp}×</button>`).join("");
  const ticks = rec.rounds.map((_r, i) => `<div class="bv-tick${i < bvRound ? " seen" : ""}${i === bvRound ? " cur" : ""}"${running ? "" : ` data-act="round" data-round="${i}"`} title="round ${i + 1}"></div>`).join("");
  const hatch = running ? `<div class="bv-hatch" title="beyond your light cone — later rounds haven't reached you yet"></div>` : "";
  // An unresolved battle has no transport controls: the only honest view is
  // the arrived-light frontier advancing at real battle pace. Once conclusion
  // light arrives, this same strip becomes an ordinary controllable replay.
  const replayTransport = frontier < 0 ? "" : `<div class="bv-transport">` +
      `<button class="bv-btn" data-act="play">${playIcon}</button>` +
      `<span class="bv-speeds">${speeds}</span>` +
      `<div class="bv-scrub">${ticks || `<div class="bv-tick cur"></div>`}</div></div>${agoline}`;
  const liveTransport = frontier < 0
    ? ""
    : `<div class="bv-transport"><div class="bv-scrub">${ticks}${hatch}</div></div>${agoline}`;
  const transport = running ? liveTransport : replayTransport;

  setHtml($("battle-viewer"), head + liveBar + sub + arena + notes + transport);
  $("battle-viewer").classList.add("is-open");
  // §theater: (re)mount the persistent canvas into the fresh DOM — the holder
  // is re-appended, so the WebGL context survives innerHTML rebuilds.
  const thMount = document.getElementById("bv-theater-mount");
  if (thMount) theaterAttach(thMount, rec, state.galaxy?.pirate_id ?? null);
  else theaterClose();
}


// §theater: a SCRIPTED demo record (debug + the acceptance spot-check rig).
// `__ss.theaterDemo()` fabricates a deterministic participant record with
// truth keyframes — capitals, torpedo salvos, PD screens, deaths, a platform,
// a retreat — and opens the viewer on it. Pure client-side; touches nothing.
export function theaterDemo(titanDown = false, big = false): void {
  const mk = (side: number, kind: ShipKind, x: number, y: number, hp = 1, plat = false) => ({ side, kind, x, y, hp, plat });
  const rounds: BattleRecordView["rounds"] = [];
  const N = 10;
  for (let i = 0; i < N; i++) {
    const t = i / (N - 1);
    const ax = -880 + 620 * t; // attackers close from the left
    const ships: KeyframeView["ships"] = [
      ...(titanDown && i >= 9 ? [] : [mk(0, "titan", ax - 60, 0, titanDown ? 1 - 0.9 * t : 1 - 0.25 * t)]),
      mk(0, "battleship", ax - 20, 120, 1 - 0.35 * t),
      ...[0, 1, 2, 3].map((k) => mk(0, "corvette", ax + 40, -160 + k * 90, 1 - 0.3 * t * ((k % 2) + 1) / 2)),
      ...[0, 1, 2, 3].map((k) => (i < 8 || k > 0 ? mk(0, "raider", ax + 90 + 30 * Math.sin(t * 6 + k), -220 + k * 140, 1 - 0.2 * t) : null)),
      ...[...Array(i < 5 ? 8 : i < 7 ? 6 : 4)].map((_, k) => mk(1, "raider", 320 + 25 * Math.cos(t * 5 + k), -260 + k * 76, 1 - 0.45 * t)),
      mk(1, "corvette", 250, -60, 1 - 0.3 * t),
      mk(1, "corvette", 250, 60, 1 - 0.3 * t),
      mk(1, "convoy", 540, 30, 1 - 0.5 * t),
      mk(1, "corvette", 600, 0, 1 - 0.6 * t, true),
      mk(1, "corvette", 600, 40, 1, true),
    ].filter((s): s is NonNullable<typeof s> => s !== null);
    if (big) {
      // 60v60 with capitals — the budget/degradation acceptance scene.
      for (let k = 0; k < 46; k++) {
        ships.push(mk(0, k % 3 === 0 ? "raider" : "corvette", ax + 60 + (k % 8) * 34, -300 + Math.floor(k / 8) * 52, 1 - 0.3 * t));
      }
      ships.push(mk(0, "dreadnought", ax - 90, -80, 1 - 0.2 * t));
      for (let k = 0; k < 44; k++) {
        ships.push(mk(1, k % 4 === 0 ? "corvette" : "raider", 300 + (k % 8) * 30, -280 + Math.floor(k / 8) * 50, 1 - 0.4 * t));
      }
      ships.push(mk(1, "battleship", 560, -60, 1 - 0.35 * t), mk(1, "cruiser", 560, 90, 1 - 0.3 * t));
    }
    const torps: KeyframeView["torpedoes"] = i >= 2 && i <= 8
      ? [{ side: 0, x: ax + 200 + 180 * ((i % 3) / 3), y: -20, n: Math.max(2, 10 - i) }]
      : [];
    const deaths: KeyframeView["deaths"] = [];
    if (i === 5) deaths.push({ step: 3, side: 1, kind: "raider", x: 340, y: -110 });
    if (i === 7) deaths.push({ step: 1, side: 1, kind: "raider", x: 355, y: 30 }, { step: 4, side: 1, kind: "raider", x: 310, y: 96 });
    if (i === 8) deaths.push({ step: 2, side: 0, kind: "raider", x: ax + 90, y: -220 });
    if (titanDown && i === 9) deaths.push({ step: 3, side: 0, kind: "titan", x: ax - 60, y: 0 });
    const rc = (kind: ShipKind, n: number) => ({ kind, exact: n, class: "one" as CountClass });
    rounds.push({
      tick: i * 15,
      counts: [
        [rc("titan", titanDown && i >= 9 ? 0 : 1), rc("battleship", 1), rc("corvette", 4), rc("raider", i < 8 ? 4 : 3)],
        [rc("raider", i < 5 ? 8 : i < 7 ? 6 : 4), rc("corvette", 2), rc("convoy", 1)],
      ],
      kills: [
        [i === 8 ? rc("raider", 1) : rc("raider", 0)].filter((k) => k.exact),
        [i === 5 ? rc("raider", 1) : i === 7 ? rc("raider", 2) : rc("raider", 0)].filter((k) => k.exact),
      ],
      dealt: [26 + i * 5, 18 + i * 3],
      notes: i === 6 ? [{ kind: "retreat_tripped", side: 1, comp: null }] : i === 9 ? [{ kind: "withdraw_ordered", side: 1, comp: null }] : [],
      frame: { ships, torpedoes: torps, deaths },
    });
  }
  const rec: BattleRecordView = {
    id: "demo-battle" as unknown as BattleRecordView["id"],
    pos: { x: 0, y: 0 },
    system: null,
    started_at: 0,
    raid: false,
    fidelity: "participant",
    own_side: 0,
    sides: [
      { corp: "1", posture: "engage_any", platform_tiers: 0, initial: rounds[0].counts[0], loadouts: [
        { kind: "raider", modules: ["torpedo_rack"], n: 4 },
        { kind: "corvette", modules: ["whipple_armor"], n: 2 },
      ], flagship_name: "Emberfall" },
      { corp: "2", posture: null, platform_tiers: 2, initial: rounds[0].counts[1], loadouts: [
        { kind: "corvette", modules: ["point_defense_screen"], n: 2 },
        { kind: "raider", modules: ["mass_driver"], n: 4 },
        { kind: "raider", modules: ["reflective_plating"], n: 3 },
      ], flagship_name: null },
    ],
    rounds,
    light_frontier_tick: (N - 1) * 15,
    outcome: "target_destroyed" as BattleRecordView["outcome"],
  };
  state.battleRecords = state.battleRecords.filter((r) => r.id !== rec.id).concat([rec]);
  openBattleViewer(rec.id);
}


// §theater: the LIVE demo rig — streams the scripted record's rounds in on a
// wall-clock timer (simulated arriving light) so LIGHT-LIVE chase playback
// can be watched without staging a real battle: `__ss.theaterDemoLive()`.
export let demoLiveTimer: number | null = null;

export function theaterDemoLive(intervalMs = 1800): void {
  if (demoLiveTimer !== null) { clearInterval(demoLiveTimer); demoLiveTimer = null; }
  theaterDemo(); // installs the full scripted record
  const full = state.battleRecords.find((r) => r.id === "demo-battle");
  if (!full) return;
  const allRounds = full.rounds;
  let upto = 2;
  const install = () => {
    const rec: BattleRecordView = {
      ...full,
      rounds: allRounds.slice(0, upto),
      outcome: upto >= allRounds.length ? full.outcome : null,
      light_frontier_tick: allRounds[upto - 1].tick,
    };
    state.battleRecords = state.battleRecords.filter((r) => r.id !== rec.id).concat([rec]);
    refreshOpenBattleViewer();
  };
  install();
  openBattleViewer(full.id); // running records always follow their arrived frontier
  demoLiveTimer = window.setInterval(() => {
    upto++;
    install();
    if (upto >= allRounds.length && demoLiveTimer !== null) {
      clearInterval(demoLiveTimer);
      demoLiveTimer = null;
    }
  }, intervalMs);
}
