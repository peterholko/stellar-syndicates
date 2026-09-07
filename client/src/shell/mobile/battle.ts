import {
  theaterAttach,
  theaterAvailable,
  theaterClose,
  theaterResetCamera,
  theaterSetTime,
  theaterShipAppearance,
} from "../../battletheater";
import { sideFamily, type SalvoFamily } from "../../core/derive/fleet";
import { nearestSystemName } from "../../core/derive/geo";
import { latestPendingOrder } from "../../core/derive/orders";
import {
  countClassLabel,
  type BattleRecordView,
  type RecordCount,
  type RoundNoteView,
  type RoundRecordView,
  type ShipKind,
} from "../../protocol";
import { liveSimTime } from "../../state";
import type { CoreContext } from "../types";
import type { SheetEntry, SheetView } from "./sheets";
import { SheetStack } from "./sheets";
import { sheetFingerprint } from "../signature";
import { BattleWithdrawPrompt } from "../battlewithdraw";

const FAMILY_COLOR: Record<SalvoFamily, string> = {
  beam: "var(--accent)",
  driver: "#e8a13a",
  torpedo: "#e0574b",
};
const FAMILY_LABEL: Record<SalvoFamily, string> = {
  beam: "beam",
  driver: "drivers",
  torpedo: "torpedoes",
};
const COUNT_CLASS_ORDER = ["one", "two_to_three", "four_to_seven", "eight_to_fifteen", "sixteen_to_thirty", "thirty_one_plus"];

const esc = (value: string): string => value.replace(
  /[&<>\"]/g,
  (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '\"': "&quot;" })[character]!,
);
const human = (value: string): string => value.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase());
const recordCount = (rows: RecordCount[], kind: ShipKind): RecordCount | undefined => rows.find((row) => row.kind === kind);
const propsOf = <T extends object>(entry: SheetEntry): Partial<T> => (entry.props && typeof entry.props === "object" ? entry.props : {}) as Partial<T>;

/** Portrait battle replay over the same arrived record as desktop.
 *
 * The mobile shell owns only transport and presentation state. It never
 * manufactures a round: running battles chase the arrived prefix, completed
 * records alone expose scrub and speed controls, and the shared Pixi theater
 * consumes the exact same truth keyframes.
 */
export class MobileBattleTheater {
  private readonly renderSignatures = new Map<SheetEntry["id"], string>();
  private id: string | null = null;
  private round = 0;
  private fraction = 0;
  private playing = false;
  private live = false;
  private speed = 4;
  private lastTs = 0;
  private lastFrontier = -1;
  private lastViewport = "";
  private readonly withdrawal = new BattleWithdrawPrompt();

  constructor(private readonly ctx: CoreContext, private readonly sheets: SheetStack) {}

  render(entry: SheetEntry): SheetView | null {
    if (entry.id !== "battle") return null;
    const id = propsOf<{ id: string }>(entry).id;
    const record = id ? this.ctx.state.battleRecords.find((candidate) => candidate.id === id) : undefined;
    this.bind(id ?? null, record);
    let view: SheetView;
    if (!id) view = { title: "Battle", eyebrow: "Observed theater", html: `<div class="m-empty">No battle selected.</div>`, detent: "full" };
    else if (!record) {
      view = {
        title: "Battle",
        eyebrow: "Observed theater · awaiting light",
        html: `<div class="m-empty"><b>Record still arriving — light-delay.</b><br>The battle marker has arrived; its first replay frame is still in transit.</div>`,
        detent: "full",
      };
    } else {
      view = {
        title: `Engagement ${esc(nearestSystemName(record.pos))}`,
        eyebrow: `${record.outcome === null ? "Battle · delayed observation" : "Battle replay"}${record.raid ? " · raid" : ""}`,
        html: this.viewer(record),
        detent: "full",
      };
    }
    this.rememberSignature(entry);
    return view;
  }

  refreshNeeded(entry: SheetEntry): boolean | null {
    const signature = this.signature(entry);
    return signature === null ? null : this.renderSignatures.get(entry.id) !== signature;
  }

  handleClick(event: Event): boolean {
    const button = (event.target as Element).closest<HTMLElement>("[data-mobile-act]");
    const action = button?.dataset.mobileAct;
    if (!button || !action?.startsWith("battle-")) return false;
    const record = this.record();
    const frontier = (record?.rounds.length ?? 0) - 1;
    switch (action) {
      case "battle-withdraw-ask": case "battle-withdraw-confirm": case "battle-withdraw-cancel":
        if (this.id && button.dataset.battle === this.id && button.dataset.fleet) {
          this.withdrawal.handle(action.slice("battle-withdraw-".length), this.id, button.dataset.fleet, this.ctx);
          this.renderSignatures.delete("battle");
          this.sheets.refresh();
        }
        break;
      case "battle-play":
        if (!record || record.outcome === null || frontier < 0) break;
        if (!this.playing && this.round >= frontier) this.round = 0;
        this.live = false;
        this.playing = !this.playing;
        this.fraction = 0;
        this.sheets.refresh();
        break;
      case "battle-speed":
        if (!record || record.outcome === null) break;
        this.speed = Math.max(1, Number(button.dataset.speed) || 1);
        this.live = false;
        this.playing = false;
        this.fraction = 0;
        this.sheets.refresh();
        break;
      case "battle-round": {
        if (!record || record.outcome === null) break;
        const round = Number(button.dataset.round);
        if (!Number.isInteger(round) || round < 0 || round > frontier) break;
        this.round = round;
        this.live = false;
        this.playing = false;
        this.fraction = 0;
        this.sheets.refresh();
        break;
      }
      case "battle-camera-reset":
        theaterResetCamera();
        break;
    }
    return true;
  }

  sync(entry: SheetEntry | null): void {
    if (entry?.id !== "battle") {
      this.close();
      return;
    }
    const record = this.record();
    const mount = document.getElementById("m-battle-theater");
    if (!record || !mount || !record.rounds.some((round) => round.frame) || !theaterAvailable()) {
      theaterClose();
      return;
    }
    const rect = mount.getBoundingClientRect();
    const width = Math.max(240, Math.floor(rect.width));
    const height = Math.max(240, Math.floor(rect.height));
    this.lastViewport = `${width}x${height}`;
    theaterAttach(mount, record, this.ctx.state.galaxy?.pirate_id ?? null, { width, height, maxFps: 30 });
    theaterSetTime(this.round, this.fraction, this.live);
  }

  tick(now = performance.now()): void {
    if (!this.id) return;
    const record = this.record();
    if (!record) return;
    const frontier = record.rounds.length - 1;
    const dt = this.lastTs ? Math.min(0.25, (now - this.lastTs) / 1000) : 0;
    this.lastTs = now;
    let changed = false;
    if (this.live && frontier >= 0) {
      if (this.round < frontier) {
        const hz = this.ctx.state.tickHz || 30;
        const window = Math.max(0.2, (record.rounds[this.round + 1].tick - record.rounds[this.round].tick) / hz);
        this.fraction += dt / window;
        while (this.fraction >= 1 && this.round < frontier) {
          this.round++;
          this.fraction--;
          changed = true;
        }
        if (this.round >= frontier) this.fraction = 0;
      } else if (record.outcome !== null) {
        this.live = false;
        this.playing = false;
        this.speed = 4;
        this.fraction = 0;
        changed = true;
      }
    } else if (this.playing && frontier >= 0) {
      // Replay pacing = the record's own tick spacing (rounds are 1:1 with
      // engine steps), so 1× IS the battle at true speed.
      if (this.round < frontier) {
        const hz = this.ctx.state.tickHz || 30;
        const window = Math.max(0.2, (record.rounds[this.round + 1].tick - record.rounds[this.round].tick) / hz);
        this.fraction += dt * this.speed / window;
      }
      while (this.fraction >= 1 && this.round < frontier) {
        this.round++;
        this.fraction--;
        changed = true;
      }
      if (this.round >= frontier) {
        this.fraction = 0;
        this.playing = false;
        changed = true;
      }
    }
    theaterSetTime(this.round, Math.min(1, this.fraction), this.live);
    const mount = document.getElementById("m-battle-theater");
    if (mount) {
      const rect = mount.getBoundingClientRect();
      const viewport = `${Math.max(240, Math.floor(rect.width))}x${Math.max(240, Math.floor(rect.height))}`;
      if (viewport !== this.lastViewport) this.sync({ id: "battle", props: { id: this.id } });
    }
    if (frontier !== this.lastFrontier) {
      this.lastFrontier = frontier;
      changed = true;
    }
    if (changed) this.sheets.refresh();
  }

  close(): void {
    this.withdrawal.clear();
    if (!this.id && !this.lastViewport) return;
    this.id = null;
    this.playing = false;
    this.live = false;
    this.lastTs = 0;
    this.lastViewport = "";
    theaterClose();
  }

  private bind(id: string | null, record: BattleRecordView | undefined): void {
    if (id !== this.id) this.withdrawal.clear();
    if (id === this.id) {
      if (record && this.lastFrontier < 0) {
        const frontier = record.rounds.length - 1;
        this.live = record.outcome === null;
        this.speed = this.live ? 1 : 4;
        this.round = this.live ? Math.max(0, frontier) : 0;
        this.playing = !this.live && frontier > 0;
        this.lastFrontier = frontier;
      }
      if (record?.outcome === null) {
        this.live = true;
        this.playing = false;
        this.speed = 1;
      }
      return;
    }
    this.id = id;
    const frontier = (record?.rounds.length ?? 0) - 1;
    this.live = record?.outcome === null;
    this.speed = this.live ? 1 : 4;
    this.round = this.live ? Math.max(0, frontier) : 0;
    this.fraction = 0;
    this.playing = !this.live && frontier > 0;
    this.lastTs = 0;
    this.lastFrontier = frontier;
    this.lastViewport = "";
  }

  private record(): BattleRecordView | undefined {
    return this.id ? this.ctx.state.battleRecords.find((candidate) => candidate.id === this.id) : undefined;
  }

  private rememberSignature(entry: SheetEntry): void {
    const signature = this.signature(entry);
    if (signature !== null) this.renderSignatures.set(entry.id, signature);
  }

  private signature(entry: SheetEntry): string | null {
    if (entry.id !== "battle") return null;
    const id = propsOf<{ id: string }>(entry).id ?? "";
    const record = id ? this.ctx.state.battleRecords.find((candidate) => candidate.id === id) : undefined;
    const roundIndex = record ? Math.max(0, Math.min(this.round, record.rounds.length - 1)) : 0;
    const round = record?.rounds[roundIndex];
    const recordSlice = record ? [
      record.id, record.system, record.started_at, record.raid, record.fidelity, record.own_side, record.sides,
      record.light_frontier_tick, record.outcome, record.rounds.length,
      round ? [round.tick, round.counts, round.kills, round.dealt, round.notes, !!round.frame] : null,
      record.rounds.slice(0, roundIndex + 1).map((candidate) => candidate.notes),
      record.rounds.map((candidate) => !!candidate.frame),
    ] : null;
    const battle = this.ctx.state.battles.find((candidate) => candidate.id === id);
    const participants = battle?.participants ?? [];
    return sheetFingerprint([
      Math.floor(liveSimTime()), entry.props ?? null, this.round, this.live, this.playing, this.speed,
      this.ctx.renderer.viewMode.type, recordSlice, battle,
      this.ctx.state.commandCenter, this.ctx.state.galaxy?.c,
      this.ctx.state.ghosts.filter((fleet) => fleet.own && participants.includes(fleet.id)).map((fleet) => [fleet.id, fleet.kind, latestPendingOrder(fleet.id)]),
    ]);
  }

  private viewer(record: BattleRecordView): string {
    const frontier = record.rounds.length - 1;
    const running = record.outcome === null;
    const semantic = this.ctx.renderer.viewMode.type === "battle";
    const outcome = record.outcome === null ? "FOLLOWING LIGHT" : outcomeLabel(record);
    const statusClass = running ? "is-live" : "is-complete";
    if (frontier < 0) {
      return `<div class="m-battle"><div class="m-battle-status ${statusClass}"><i></i><b>${outcome}</b><span>Awaiting the first round's light.</span></div>` +
        this.viewerActions(record.id, semantic) + `</div>`;
    }
    this.round = Math.max(0, Math.min(this.round, frontier));
    const current = record.rounds[this.round];
    const participant = record.fidelity === "participant";
    const platformGone = record.rounds.slice(0, this.round + 1).some((round) => round.notes.some((note) => note.kind === "platform_destroyed"));
    const salvos = this.salvos(record, current, participant);
    const sides = `<div class="bv-arena m-battle-arena">${this.side(record, current, 0, participant, false)}${salvos}${this.side(record, current, 1, participant, platformGone)}</div>`;
    const hasTheater = record.rounds.some((round) => round.frame);
    const theater = hasTheater && theaterAvailable()
      ? `<section class="m-tactical-stage"><div id="m-battle-theater" class="m-battle-theater"></div><button type="button" data-mobile-act="battle-camera-reset">Reset camera</button></section>`
      : current.frame ? this.truthMap(current, record) : "";
    const notes = current.notes.length ? `<div class="bv-notes">${current.notes.map(noteHtml).join("")}</div>` : "";
    return `<div class="m-battle">` +
      `<div class="m-battle-status ${statusClass}"><i></i><b>${esc(outcome)}</b><span>Round ${this.round + 1}/${record.rounds.length}${running ? " · arrived prefix" : ""}</span></div>` +
      `<div class="bv-sub"><span class="bv-vs"><span class="${record.own_side === 0 ? "you" : "foe"}">${record.own_side === 0 ? "You" : "Attackers"}</span> vs <span class="${record.own_side === 1 ? "you" : "foe"}">${record.own_side === 1 ? "You" : "Defenders"}</span></span><span>${participant ? "participant record" : "sensor estimate"}</span></div>` +
      this.viewerActions(record.id, semantic) + sides + theater + notes + this.transport(record) + `</div>`;
  }

  private viewerActions(battleId: string, semantic: boolean): string {
    return `<div class="m-action-grid m-action-grid--top">` +
      this.withdrawal.html(battleId, this.ctx, "data-mobile-act", "battle-withdraw") +
      (semantic ? `<button type="button" class="m-primary" data-mobile-act="semantic-exit">Back to galaxy</button>` : "") + `</div>`;
  }

  private side(record: BattleRecordView, round: RoundRecordView, side: 0 | 1, participant: boolean, platformGone: boolean): string {
    const mine = record.own_side === side;
    const family = participant ? sideFamily(record.sides[side]) : null;
    const hullLabel = (kind: ShipKind) => theaterShipAppearance(record, side, kind, this.ctx.state.galaxy?.pirate_id ?? null).label;
    const fits = participant ? (record.sides[side].loadouts ?? []).map((fit) => `${fit.n}× ${hullLabel(fit.kind)} · ${fit.modules.map(human).join(" + ") || "stock"}`).join(" · ") : "";
    const rows = record.sides[side].initial.map((opening) => {
      const survivor = recordCount(round.counts[side], opening.kind);
      const killed = recordCount(round.kills[side], opening.kind);
      const gone = !survivor;
      let pct = 0;
      let count = "—";
      if (participant) {
        const initial = opening.exact ?? 0;
        const left = survivor?.exact ?? 0;
        pct = initial > 0 ? left / initial * 100 : 0;
        count = `×${left}`;
      } else if (survivor) {
        pct = (COUNT_CLASS_ORDER.indexOf(survivor.class) + 1) / COUNT_CLASS_ORDER.length * 100;
        count = countClassLabel(survivor.class);
      }
      const loss = participant ? (killed?.exact ? `<span class="bv-krow__kill">−${killed.exact}</span>` : "") : killed ? `<span class="bv-krow__kill">▾</span>` : "";
      return `<div class="bv-krow"><span class="m-battle-kind">${esc(hullLabel(opening.kind))}</span><div class="bv-krow__bar"><div class="bv-krow__fill${gone ? " gone" : ""}" style="width:${gone ? 100 : Math.max(5, pct).toFixed(1)}%"></div></div><span class="bv-krow__n${gone ? " is-gone" : ""}">${esc(count)}${loss}</span></div>`;
    }).join("");
    const platform = side === 1 && record.sides[1].platform_tiers > 0 ? `<div class="bv-plat${platformGone ? " gone" : ""}">Defense Platform ×${record.sides[1].platform_tiers}</div>` : "";
    return `<section class="bv-side${side === 1 ? " right" : ""}${mine ? " mine" : ""}"><div class="bv-side__hd">${mine ? `<strong>YOU</strong>` : ""}${side === 0 ? "Attackers" : "Defenders"}${family ? `<span class="bv-fampip" style="color:${FAMILY_COLOR[family]}">● ${FAMILY_LABEL[family]}</span>` : ""}</div>${fits ? `<div class="bv-fits">${esc(fits)}</div>` : ""}${rows}${platform}</section>`;
  }

  private salvos(record: BattleRecordView, round: RoundRecordView, participant: boolean): string {
    if (!participant || !round.dealt) return `<div class="bv-salvos m-battle-salvos"><span>exact fire strength fogged</span></div>`;
    const max = Math.max(1e-6, ...record.rounds.flatMap((candidate) => candidate.dealt ? [candidate.dealt[0], candidate.dealt[1]] : [0]));
    const height = (damage: number) => Math.max(8, damage / max * 38);
    const attacker = sideFamily(record.sides[0]);
    const defender = sideFamily(record.sides[1]);
    return `<div class="bv-salvos m-battle-salvos"><div class="bv-arrow r" style="height:${height(round.dealt[0]).toFixed(1)}px;background:${FAMILY_COLOR[attacker]};color:${FAMILY_COLOR[attacker]}"><span>${round.dealt[0].toFixed(1)}</span></div><div class="bv-arrow l" style="height:${height(round.dealt[1]).toFixed(1)}px;background:${FAMILY_COLOR[defender]};color:${FAMILY_COLOR[defender]}"><span>${round.dealt[1].toFixed(1)}</span></div></div>`;
  }

  private transport(record: BattleRecordView): string {
    const running = record.outcome === null;
    const ticks = record.rounds.map((_round, index) => running
      ? `<i class="bv-tick${index < this.round ? " seen" : ""}${index === this.round ? " cur" : ""}"></i>`
      : `<button type="button" class="bv-tick${index < this.round ? " seen" : ""}${index === this.round ? " cur" : ""}" data-mobile-act="battle-round" data-round="${index}" aria-label="Show round ${index + 1}"></button>`).join("");
    if (running) return `<div class="bv-transport m-battle-transport"><div class="bv-scrub">${ticks}<span class="bv-hatch" aria-label="Later rounds have not reached you"></span></div><small>Following arrived light at real battle pace.</small></div>`;
    const speeds = [1, 4, 16].map((speed) => `<button type="button" class="bv-btn${this.speed === speed ? " on" : ""}" data-mobile-act="battle-speed" data-speed="${speed}">${speed}×</button>`).join("");
    return `<div class="bv-transport m-battle-transport"><div class="m-battle-playback"><button type="button" class="bv-btn" data-mobile-act="battle-play">${this.playing ? "❚❚ Pause" : "▶ Play"}</button><span class="bv-speeds">${speeds}</span></div><div class="bv-scrub">${ticks}</div><small>Round ${this.round + 1} of ${record.rounds.length}</small></div>`;
  }

  private truthMap(round: RoundRecordView, record: BattleRecordView): string {
    const frame = round.frame;
    if (!frame) return "";
    const radius = 1450;
    const x = (value: number) => (value + radius) / (radius * 2) * 100;
    const y = (value: number) => (value + radius) / (radius * 2) * 100;
    const color = (side: number) => record.own_side === null ? (side === 0 ? "#e0574b" : "#5ad1e0") : side === record.own_side ? "#5ad1e0" : "#e0574b";
    const ships = frame.ships.map((ship) => `<circle cx="${x(ship.x).toFixed(1)}" cy="${y(ship.y).toFixed(1)}" r="${ship.plat ? 2 : 1.2}" fill="${color(ship.side)}" opacity="${(0.35 + 0.65 * ship.hp).toFixed(2)}"></circle>`).join("");
    const torpedoes = frame.torpedoes.map((torpedo) => `<circle cx="${x(torpedo.x).toFixed(1)}" cy="${y(torpedo.y).toFixed(1)}" r="1" fill="#e0574b"></circle>`).join("");
    return `<div class="m-battle-truth"><svg viewBox="0 0 100 100" role="img" aria-label="Recorded tactical positions"><circle cx="50" cy="50" r="34.5" fill="none" stroke="rgba(255,255,255,.14)" stroke-dasharray="2 2"></circle>${ships}${torpedoes}</svg></div>`;
  }
}

const NOTE_TEXT: Record<string, (side: string) => string> = {
  joined: (side) => `Reinforcements join the ${side}`,
  retreat_tripped: (side) => `The ${side} trip their retreat threshold`,
  withdraw_ordered: (side) => `A withdraw order reaches the ${side}`,
  disengage_exposure: (side) => `The ${side} break off under parting fire`,
  platform_destroyed: () => "The Defense Platform is destroyed",
  mutual_disengage: () => "Mutual disengagement",
};

function noteHtml(note: RoundNoteView): string {
  const side = note.side === 0 ? "attackers" : note.side === 1 ? "defenders" : "fleets";
  const text = NOTE_TEXT[note.kind]?.(side) ?? human(note.kind);
  const cls = ["retreat_tripped", "withdraw_ordered", "disengage_exposure"].includes(note.kind) ? " retreat" : note.kind === "joined" ? " join" : "";
  return `<div class="bv-note${cls}">${esc(text)}</div>`;
}

function outcomeLabel(record: BattleRecordView): string {
  const outcome = record.outcome;
  if (!outcome) return "Following light";
  const attackerLost = outcome === "attacker_destroyed" || outcome === "both_destroyed";
  const defenderLost = outcome === "target_destroyed" || outcome === "both_destroyed";
  if (record.own_side === null) return outcome === "both_destroyed" ? "Mutual destruction" : attackerLost ? "Attackers destroyed" : defenderLost ? "Defenders destroyed" : "Both withdrew";
  const youLost = record.own_side === 0 ? attackerLost : defenderLost;
  const foeLost = record.own_side === 0 ? defenderLost : attackerLost;
  if (youLost && foeLost) return "Mutual destruction";
  if (youLost) return "Defeat";
  if (foeLost) return "Victory";
  return "Both withdrew";
}
