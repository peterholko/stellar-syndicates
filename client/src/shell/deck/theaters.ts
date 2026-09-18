import {
  theaterAttach,
  theaterAvailable,
  theaterClose,
  theaterResetCamera,
  theaterSetTime,
  theaterShipAppearance,
} from "../../battletheater";
import {
  battleReportForRecord,
  battleViewerTimers,
  clearBattleAftermathTimer,
  clearBattleCloseTimer,
} from "../../core/derive/fleet";
import { fmtDur, informationDelay } from "../../core/derive/format";
import { nearestSystemName, systemName } from "../../core/derive/geo";
import { latestPendingOrder } from "../../core/derive/orders";
import {
  groundTheaterAttach,
  groundTheaterAvailable,
  groundTheaterClose,
  groundTheaterSetTime,
  groundTheaterStep,
} from "../../groundtheater";
import { icon } from "../../icons";
import {
  countClassLabel,
  type BattleRecordView,
  type GroundRecordView,
  type KeyframeView,
  type PlayerId,
  type RecordCount,
  type RoundNoteView,
} from "../../protocol";
import { setHtml } from "../dom";
import { BattleWithdrawPrompt } from "../battlewithdraw";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

const BATTLE_STALE_MS = 4_500;
const SEMANTIC_TRANSITION_MS = 480;

interface TheaterHooks {
  go(route: DeckRoute): void;
  openDoctrine(): void;
  notice(html: string): void;
  battleSelectionChanged?(): void;
}

type BattleOpenOptions = { semantic?: boolean };

/** Deck-owned modal hosts around the shared replay engines. The playback
 * cursor never crosses the arrived round prefix: an unresolved record chases
 * that frontier at the game's pace. A live viewer finishes the arrived ending
 * before exposing replay transport, and stays open until the player leaves.
 * Opening an already-concluded replay retains its historical 4x default. */
export class DeckTheaters {
  private battleId: string | null = null;
  private battleRound = 0;
  private battlePlaying = false;
  private battleSpeed = 4;
  private battleLive = false;
  private battleAccum = 0;
  private battleLastTs = 0;
  private battleLoop = 0;
  private battleSignature = "";
  private battleSemantic = false;
  private battleClosing = false;
  private battleLastAge: number | null = null;
  private battleLastFrontier = -1;
  private battleLastArrivalWallMs = 0;
  private readonly withdrawal = new BattleWithdrawPrompt();

  private groundId: string | null = null;
  private groundRound = 0;
  private groundFrac = 0;
  private groundLive = false;
  private groundPlaying = false;
  private groundLastTs = 0;
  private groundLoop = 0;
  private groundSignature = "";

  constructor(
    private readonly battleRoot: HTMLElement,
    private readonly battleCard: HTMLElement,
    private readonly groundRoot: HTMLElement,
    private readonly groundCard: HTMLElement,
    private readonly ctx: CoreContext,
    private readonly hooks: TheaterHooks,
    signal: AbortSignal,
  ) {
    battleRoot.addEventListener("click", (event) => this.battleClick(event), { signal });
    groundRoot.addEventListener("click", (event) => this.groundClick(event), { signal });
    // Wheel input belongs to the theater camera, even after zoom-to-enter.
    // Zooming out clamps at its overview limit; leaving is an explicit close,
    // never a scroll gesture captured by this overlay before the canvas.
  }

  get isOpen(): boolean { return this.battleId !== null || this.groundId !== null; }
  get activeBattleId(): string | null { return this.battleId; }

  openBattle(id: string, options: BattleOpenOptions = {}): boolean {
    const record = this.battleRecord(id);
    if (!record) return false;
    this.withdrawal.clear();
    this.closeGround();
    clearBattleAftermathTimer();
    clearBattleCloseTimer();
    this.battleClosing = false;
    this.battleSemantic = options.semantic === true;
    this.battleId = id;
    this.hooks.battleSelectionChanged?.();
    const running = record.outcome === null;
    const frontier = record.rounds.length - 1;
    this.battleLive = running;
    this.battleSpeed = running ? 1 : 4;
    this.battleRound = running ? Math.max(0, frontier) : 0;
    this.battlePlaying = !running && frontier > 0;
    this.battleAccum = 0;
    this.battleLastTs = 0;
    this.battleLastFrontier = frontier;
    this.battleLastArrivalWallMs = performance.now();
    this.battleLastAge = this.ctx.state.battles.find((battle) => battle.id === id)?.age ?? null;
    this.battleSignature = "";
    this.battleRoot.hidden = false;
    this.battleRoot.classList.toggle("is-semantic", this.battleSemantic);
    this.battleRoot.classList.remove("is-leaving");
    if (this.battleSemantic) {
      this.battleRoot.classList.add("is-entering");
      requestAnimationFrame(() => requestAnimationFrame(() => this.battleRoot.classList.remove("is-entering")));
    } else {
      this.battleRoot.classList.remove("is-entering");
    }
    this.renderBattle();
    if (!this.battleLoop) this.battleLoop = requestAnimationFrame((time) => this.tickBattle(time));
    return true;
  }

  enterBattle(id: string): boolean {
    if (this.battleClosing) return false;
    const battle = this.ctx.state.battles.find((candidate) => candidate.id === id);
    const record = this.battleRecord(id);
    if (!record) return false;
    if (battle && record.outcome === null) {
      // Running: semantic-zoom the map into the fight and follow the light.
      this.ctx.renderer.enterBattleView(id, battle.pos);
      this.hooks.go({ name: "battle", params: { id, label: "Ongoing battle" } });
    } else if (record.outcome !== null) {
      // Concluded: same gesture opens the recorded replay (no battle view —
      // the map stays put behind the theater).
      this.hooks.go({ name: "battle", params: { id, label: "Battle replay" } });
    } else {
      return false; // record exists but the battle is gone from the live list — mid-transition; ignore
    }
    return this.openBattle(id, { semantic: true });
  }

  openGround(id: string): boolean {
    const record = this.groundRecord(id);
    if (!record) return false;
    this.closeBattle();
    this.groundId = id;
    const frontier = record.rounds.length - 1;
    this.groundLive = record.outcome === null;
    this.groundRound = this.groundLive ? Math.max(0, frontier) : 0;
    this.groundFrac = 0;
    this.groundPlaying = !this.groundLive && frontier > 0;
    this.groundLastTs = 0;
    this.groundSignature = "";
    this.groundRoot.hidden = false;
    this.renderGround();
    if (!this.groundLoop) this.groundLoop = requestAnimationFrame((time) => this.tickGround(time));
    return true;
  }

  closeTop(): boolean {
    if (this.groundId !== null) {
      this.closeGround();
      return true;
    }
    if (this.battleId !== null) {
      this.closeBattle();
      return true;
    }
    return false;
  }

  onViewTick(): void {
    if (this.battleId !== null) {
      const record = this.battleRecord(this.battleId);
      if (record?.outcome === null) {
        this.battleLive = true;
        this.battlePlaying = false;
        this.battleSpeed = 1;
      }
      const age = this.ctx.state.battles.find((battle) => battle.id === this.battleId)?.age;
      if (age !== undefined) this.battleLastAge = age;
      if (record && record.rounds.length - 1 !== this.battleLastFrontier) {
        this.battleLastFrontier = record.rounds.length - 1;
        this.battleLastArrivalWallMs = performance.now();
      }
      this.renderBattle();
    }
    if (this.groundId !== null) this.renderGround();
  }

  teardown(): void {
    clearBattleAftermathTimer();
    clearBattleCloseTimer();
    if (this.battleLoop) cancelAnimationFrame(this.battleLoop);
    if (this.groundLoop) cancelAnimationFrame(this.groundLoop);
    this.battleLoop = 0;
    this.groundLoop = 0;
    this.finishBattleClose();
    this.closeGround();
  }

  private battleClick(event: Event): void {
    const target = event.target as Element;
    if (target.closest("[data-deck-theater-dismiss]")) {
      this.closeBattle();
      return;
    }
    const button = target.closest<HTMLButtonElement>("[data-deck-theater-act]");
    if (!button) return;
    const record = this.battleId ? this.battleRecord(this.battleId) : undefined;
    const frontier = record ? record.rounds.length - 1 : -1;
    switch (button.dataset.deckTheaterAct) {
      case "close": this.closeBattle(); break;
      case "play":
        if (!record || record.outcome === null || this.battleLive) break;
        this.battleLive = false;
        if (!this.battlePlaying && record && this.battleRound >= frontier) this.battleRound = 0;
        this.battlePlaying = !this.battlePlaying;
        this.battleAccum = 0;
        clearBattleAftermathTimer();
        this.renderBattle(true);
        break;
      case "speed":
        if (!record || record.outcome === null || this.battleLive) break;
        this.battleLive = false;
        this.battlePlaying = false;
        this.battleSpeed = Number(button.dataset.speed) || 1;
        this.battleAccum = 0;
        clearBattleAftermathTimer();
        this.renderBattle(true);
        break;
      case "round":
        if (!record || record.outcome === null || this.battleLive) break;
        this.battleRound = Number(button.dataset.round) || 0;
        this.battleLive = false;
        this.battlePlaying = false;
        this.battleAccum = 0;
        clearBattleAftermathTimer();
        this.renderBattle(true);
        break;
      case "report": {
        if (!record || record.outcome === null || this.battleLive) break;
        const report = battleReportForRecord(record);
        if (report) this.closeBattle(() => this.hooks.go({ name: "battle", params: { id: String(report.id), report: "battle", label: "Battle aftermath" } }));
        break;
      }
      case "withdraw-ask": case "withdraw-confirm": case "withdraw-cancel":
        if (this.battleId && button.dataset.battle === this.battleId && button.dataset.fleet) {
          const notice = this.withdrawal.handle(button.dataset.deckTheaterAct.slice("withdraw-".length), this.battleId, button.dataset.fleet, this.ctx);
          if (notice) this.hooks.notice(notice);
          this.renderBattle(true);
        }
        break;
      case "doctrine":
        this.closeBattle();
        this.hooks.openDoctrine();
        break;
      case "reset-camera": theaterResetCamera(); break;
    }
  }

  private groundClick(event: Event): void {
    const target = event.target as Element;
    if (target.closest("[data-deck-theater-dismiss]")) {
      this.closeGround();
      return;
    }
    const button = target.closest<HTMLButtonElement>("[data-deck-ground-act]");
    if (!button) return;
    const record = this.groundId ? this.groundRecord(this.groundId) : undefined;
    const frontier = record ? record.rounds.length - 1 : -1;
    if (button.dataset.deckGroundAct === "close") this.closeGround();
    else if (button.dataset.deckGroundAct === "play") {
      if (!this.groundPlaying && record && record.outcome !== null && this.groundRound >= frontier) {
        this.groundRound = 0;
        this.groundLive = false;
      }
      this.groundPlaying = !this.groundPlaying;
      this.renderGround(true);
    } else if (button.dataset.deckGroundAct === "round") {
      this.groundRound = Number(button.dataset.round) || 0;
      this.groundFrac = 0;
      this.groundLive = record !== undefined && record.outcome === null && this.groundRound >= frontier;
      this.groundPlaying = false;
      this.renderGround(true);
    }
  }

  private closeBattle(after?: () => void): void {
    clearBattleAftermathTimer();
    const semantic = this.battleSemantic || this.ctx.renderer.viewMode.type === "battle";
    if (!semantic) {
      this.finishBattleClose(after);
      return;
    }
    if (this.battleClosing) return;
    this.battleClosing = true;
    this.ctx.renderer.exitBattleView();
    this.battleRoot.classList.add("is-leaving");
    battleViewerTimers.close = window.setTimeout(() => {
      battleViewerTimers.close = null;
      if (!this.battleClosing) return;
      this.battleSemantic = false;
      this.finishBattleClose(after);
    }, SEMANTIC_TRANSITION_MS);
  }

  private finishBattleClose(after?: () => void): void {
    clearBattleCloseTimer();
    this.withdrawal.clear();
    this.battleId = null;
    this.hooks.battleSelectionChanged?.();
    this.battlePlaying = false;
    this.battleLive = false;
    this.battleClosing = false;
    this.battleRoot.hidden = true;
    this.battleRoot.classList.remove("is-semantic", "is-entering", "is-leaving");
    theaterClose();
    after?.();
  }

  private closeGround(): void {
    this.groundId = null;
    this.groundPlaying = false;
    this.groundRoot.hidden = true;
    groundTheaterClose();
  }

  private tickBattle(timestamp: number): void {
    this.battleLoop = 0;
    if (this.battleId === null) return;
    const record = this.battleRecord(this.battleId);
    if (!record) {
      this.closeBattle();
      return;
    }
    const frontier = record.rounds.length - 1;
    if (frontier !== this.battleLastFrontier) {
      this.battleLastFrontier = frontier;
      this.battleLastArrivalWallMs = timestamp;
    }
    const age = this.ctx.state.battles.find((battle) => battle.id === record.id)?.age;
    if (age !== undefined) this.battleLastAge = age;
    const dt = this.battleLastTs ? Math.min(0.25, (timestamp - this.battleLastTs) / 1_000) : 0;
    if (this.battleLive && frontier >= 0) {
      if (this.battleRound < frontier) {
        const hz = this.ctx.state.tickHz || 30;
        const windowSeconds = Math.max(0.2, (record.rounds[this.battleRound + 1].tick - record.rounds[this.battleRound].tick) / hz);
        // Match sim seconds per wall second (including accelerated playtests),
        // not replay speed. The arrived frontier still caps every advancement.
        this.battleAccum += dt * this.ctx.state.pacingScale / windowSeconds;
        let changed = false;
        while (this.battleAccum >= 1 && this.battleRound < frontier) {
          this.battleRound++;
          this.battleAccum--;
          changed = true;
        }
        if (this.battleRound >= frontier) this.battleAccum = 0;
        if (changed) this.renderBattle(true);
      } else {
        this.battleAccum = 0;
        if (record.outcome !== null) {
          this.battleLive = false;
          this.battlePlaying = false;
          this.renderBattle(true);
        }
      }
    } else if (this.battlePlaying && frontier >= 0) {
      // Replay pacing = the record's own tick spacing (rounds are 1:1 with
      // engine steps), so 1× IS the battle at true speed.
      if (this.battleRound < frontier) {
        const hz = this.ctx.state.tickHz || 30;
        const windowSeconds = Math.max(0.2, (record.rounds[this.battleRound + 1].tick - record.rounds[this.battleRound].tick) / hz);
        this.battleAccum += dt * this.battleSpeed / windowSeconds;
      }
      let changed = false;
      while (this.battleAccum >= 1 && this.battleRound < frontier) {
        this.battleRound++;
        this.battleAccum--;
        changed = true;
      }
      if (this.battleRound >= frontier) {
        this.battleAccum = 0;
        this.battlePlaying = false;
        changed = true;
      }
      if (changed) this.renderBattle(true);
    }
    theaterSetTime(this.battleRound, Math.min(1, this.battleAccum), this.battleLive);
    this.battleLastTs = timestamp;
    this.battleLoop = requestAnimationFrame((time) => this.tickBattle(time));
  }

  private tickGround(timestamp: number): void {
    this.groundLoop = 0;
    if (this.groundId === null) return;
    const record = this.groundRecord(this.groundId);
    if (!record) {
      this.closeGround();
      return;
    }
    const frontier = record.rounds.length - 1;
    const dt = this.groundLastTs ? Math.min(0.25, (timestamp - this.groundLastTs) / 1_000) : 0;
    this.groundLastTs = timestamp;
    if ((this.groundPlaying || this.groundLive) && frontier > 0) {
      const hz = this.ctx.state.tickHz || 30;
      const at = Math.min(this.groundRound, frontier - 1);
      const windowSeconds = Math.max(0.2, (record.rounds[at + 1].tick - record.rounds[at].tick) / hz);
      this.groundFrac += dt / windowSeconds;
      while (this.groundFrac >= 1 && this.groundRound < frontier) {
        this.groundRound++;
        this.groundFrac--;
      }
      if (this.groundRound >= frontier) {
        this.groundFrac = 0;
        if (this.groundLive && record.outcome !== null) {
          this.groundLive = false;
          this.groundPlaying = false;
        } else if (!this.groundLive) {
          this.groundPlaying = false;
        }
        this.renderGround(true);
      }
    }
    groundTheaterStep(dt);
    this.groundLoop = requestAnimationFrame((time) => this.tickGround(time));
  }

  private renderBattle(force = false): void {
    if (this.battleId === null) return;
    const record = this.battleRecord(this.battleId);
    if (!record) {
      this.closeBattle();
      return;
    }
    const running = this.battleLive || record.outcome === null;
    const frontier = record.rounds.length - 1;
    if (this.battleLive && frontier >= 0) this.battleRound = Math.min(this.battleRound, frontier);
    this.battleRound = Math.max(0, Math.min(this.battleRound, Math.max(0, frontier)));
    const wallNow = performance.now();
    const stalled = running && this.battleLastArrivalWallMs > 0 && wallNow - this.battleLastArrivalWallMs >= BATTLE_STALE_MS;
    const stale = running && ((this.battleLastAge ?? 0) >= 8 || stalled);
    const signature = JSON.stringify([
      record.id, record.rounds.length, record.outcome, this.battleRound, this.battlePlaying,
      this.battleSpeed, this.battleLive, this.battleSemantic,
      this.battleLastAge === null ? null : Math.ceil(this.battleLastAge), stale,
      this.withdrawSignature(record.id), Math.floor(wallNow / 1_000),
    ]);
    if (force || signature !== this.battleSignature) {
      this.battleSignature = signature;
      setHtml(this.battleCard, this.battleHtml(record, stale, stalled));
    }
    this.battleRoot.hidden = false;
    // The Pixi holder is persistent while the card is morph-rendered. Reattach
    // on every render attempt so no chrome rebuild can strand its live canvas.
    const mount = this.battleCard.querySelector<HTMLElement>("[data-battle-theater-mount]");
    if (mount && theaterAvailable()) theaterAttach(mount, record, this.ctx.state.galaxy?.pirate_id ?? null, undefined, this.ctx.state);
    else theaterClose();
    theaterSetTime(this.battleRound, Math.min(1, this.battleAccum), this.battleLive);
  }

  private battleHtml(record: BattleRecordView, stale: boolean, stalled: boolean): string {
    // Receipt of the final packet is not completion of its on-screen playback.
    // Keep LIVE chrome through the last impact; outcome alone never unlocks it.
    const running = this.battleLive || record.outcome === null;
    const frontier = record.rounds.length - 1;
    const round = frontier >= 0 ? record.rounds[this.battleRound] : undefined;
    const label0 = record.own_side === 0 ? "You" : "Attackers";
    const label1 = record.own_side === 1 ? "You" : "Defenders";
    const ageText = this.battleLastAge === null ? "arrival frontier" : informationDelay(this.battleLastAge);
    const status = running
      ? `<div class="deck-theater-live${stale ? " is-stale" : ""}"><i></i><b>FOLLOWING LIGHT</b><span>real battle pace · ${esc(ageText)}</span>${stalled ? `<em>light in transit · holding last arrival</em>` : ""}</div>`
      : `<div class="deck-theater-live is-complete"><i></i><b>COMPLETE</b><span>${esc(outcomeText(record))}</span></div>`;
    const counter = frontier < 0 ? "awaiting first round" : `Round ${this.battleRound + 1} of ${record.rounds.length}${running ? " +" : ""}`;
    const pirateId = this.ctx.state.galaxy?.pirate_id ?? null;
    const forces = round ? `<div class="deck-theater-forces">${sideHtml(record, round.counts[0], 0, label0, pirateId)}<span>versus</span>${sideHtml(record, round.counts[1], 1, label1, pirateId)}</div>` : `<div class="deck-theater-empty">Awaiting the first round's light…</div>`;
    const notes = round?.notes.length ? `<div class="deck-theater-notes">${round.notes.map(noteHtml).join("")}</div>` : "";
    const hasFrames = record.rounds.some((entry) => entry.frame);
    const frame = round?.frame ?? null;
    const stage = hasFrames && theaterAvailable()
      ? `<div class="deck-theater-stage" data-battle-theater-mount></div>`
      : frame ? truthMap(frame, record.own_side) : "";
    const ticks = running ? "" : record.rounds.map((_entry, index) => `<button type="button" class="deck-theater-tick${index < this.battleRound ? " is-seen" : ""}${index === this.battleRound ? " is-current" : ""}" data-deck-theater-act="round" data-round="${index}" aria-label="Round ${index + 1}"></button>`).join("");
    const transport = frontier < 0 || running ? "" : `<div class="deck-theater-transport"><button type="button" data-deck-theater-act="play">${this.battlePlaying ? "Pause" : "Play"}</button><span class="deck-theater-speeds">${[1, 4, 16].map((speed) => `<button type="button" data-deck-theater-act="speed" data-speed="${speed}" aria-pressed="${this.battleSpeed === speed}">${speed}×</button>`).join("")}</span><div class="deck-theater-scrub">${ticks}</div></div>`;
    const elapsed = round ? Math.max(0, round.tick / (this.ctx.state.tickHz || 30) - record.started_at) : 0;
    const withdraw = this.withdrawHtml(record.id);
    const report = !running && battleReportForRecord(record) ? `<button type="button" data-deck-theater-act="report">Battle aftermath</button>` : "";
    return `<header class="deck-theater-head"><div><span>${record.raid ? "Raid" : "Battle"} · ${record.fidelity === "participant" ? "participant record" : "sensor estimate"}</span><h2>Engagement ${esc(nearestSystemName(record.pos))}</h2></div><div><b>${esc(counter)}</b><button type="button" data-deck-theater-act="close" aria-label="Close battle theater">✕</button></div></header>${status}${forces}${stage}${notes}${transport}${round ? `<p class="deck-theater-time">At +${fmtDur(elapsed)} into the fight · arrived record round ${this.battleRound + 1}</p>` : ""}${withdraw}<footer class="deck-theater-footer"><button type="button" data-deck-theater-act="reset-camera">Reset camera</button>${record.own_side !== null ? `<button type="button" data-deck-theater-act="doctrine">Fleet doctrine</button>` : ""}${report}<span>Esc closes · replay never advances beyond arrived light.</span></footer>`;
  }

  private withdrawHtml(recordId: string): string {
    const controls = this.withdrawal.html(recordId, this.ctx, "data-deck-theater-act", "withdraw");
    return controls ? `<section class="deck-theater-withdraw"><b>Engaged fleets</b><div>${controls}</div></section>` : "";
  }

  private withdrawSignature(recordId: string): unknown {
    const battle = this.ctx.state.battles.find((candidate) => candidate.id === recordId);
    if (!battle) return null;
    const ids = new Set(battle.participants);
    return this.ctx.state.ghosts.filter((fleet) => fleet.own && ids.has(fleet.id)).map((fleet) => [fleet.id, fleet.kind, latestPendingOrder(fleet.id)]);
  }

  private renderGround(force = false): void {
    if (this.groundId === null) return;
    const record = this.groundRecord(this.groundId);
    if (!record) {
      this.closeGround();
      return;
    }
    const frontier = record.rounds.length - 1;
    this.groundRound = Math.max(0, Math.min(this.groundRound, Math.max(0, frontier)));
    const signature = JSON.stringify([record.id, record.rounds.length, record.outcome, this.groundRound, this.groundPlaying, this.groundLive]);
    if (force || signature !== this.groundSignature) {
      this.groundSignature = signature;
      const verdict = record.outcome === null ? "Landing in progress" : record.outcome === "taken" ? "Ground taken" : "Landing destroyed";
      const strength = record.marines_landed !== null && record.defenders_initial !== null
        ? `<span><b>${record.marines_landed}</b> marines landed against <b>${record.defenders_initial}</b></span>`
        : `<span>Troop strengths are unresolved from this observation.</span>`;
      const ticks = record.rounds.map((round, index) => `<button type="button" class="deck-ground-tick${index === this.groundRound ? " is-current" : ""}${(round.notes ?? []).length ? " is-beat" : ""}" data-deck-ground-act="round" data-round="${index}" aria-label="Ground round ${index + 1}"></button>`).join("");
      setHtml(this.groundCard, `<header class="deck-theater-head"><div><span>${record.fidelity === "participant" ? "Ground assault" : "Observed from orbit"}</span><h2>${record.attacking ? "Your landing" : "Landing report"} · ${esc(systemName(record.system))}</h2></div><div><b>${esc(verdict)}</b><button type="button" data-deck-ground-act="close" aria-label="Close ground theater">✕</button></div></header><div class="deck-ground-summary">${strength}<span>${record.garrison_tiers} garrison tier${record.garrison_tiers === 1 ? "" : "s"}</span><span>${Math.round(record.suppression_at_drop * 100)}% suppressed at drop</span></div><div class="deck-ground-stage" data-ground-theater-mount></div><div class="deck-ground-scrub">${ticks}</div><div class="deck-theater-transport"><button type="button" data-deck-ground-act="play">${this.groundPlaying ? "Pause" : "Play"}</button><span>Round ${Math.min(this.groundRound + 1, record.rounds.length)} of ${record.rounds.length}${this.groundLive ? " · chasing your light cone" : ""}</span></div><footer class="deck-theater-footer"><span>Esc closes · later rounds appear only when their light arrives.</span></footer>`);
    }
    this.groundRoot.hidden = false;
    const mount = this.groundCard.querySelector<HTMLElement>("[data-ground-theater-mount]");
    if (mount && groundTheaterAvailable()) groundTheaterAttach(mount, record);
    else groundTheaterClose();
    groundTheaterSetTime(this.groundRound, this.groundFrac, this.groundLive && this.groundRound >= frontier);
  }

  private battleRecord(id: string): BattleRecordView | undefined {
    return this.ctx.state.battleRecords.find((record) => record.id === id);
  }

  private groundRecord(id: string): GroundRecordView | undefined {
    return this.ctx.state.groundRecords.find((record) => record.id === id);
  }
}

function sideHtml(record: BattleRecordView, current: RecordCount[], side: 0 | 1, heading: string, pirateId: PlayerId | null): string {
  const participant = record.fidelity === "participant";
  const rows = record.sides[side].initial.map((initial) => {
    const now = current.find((entry) => entry.kind === initial.kind);
    const value = participant ? String(now?.exact ?? 0) : now ? countClassLabel(now.class) : "destroyed";
    const opening = participant ? initial.exact ?? 0 : initial.class;
    const pct = participant
      ? Math.max(0, Math.min(100, (now?.exact ?? 0) / Math.max(1, Number(opening)) * 100))
      : now ? Math.max(8, (countClassOrder(now.class) + 1) / 6 * 100) : 0;
    const appearance = theaterShipAppearance(record, side, initial.kind, pirateId);
    return `<div class="deck-theater-force-row${now ? "" : " is-lost"}"><span><img class="deck-theater-force-art" src="${escAttr(appearance.url)}" alt="" title="${escAttr(appearance.label)}"> ${esc(appearance.label)}</span><i><b style="width:${pct.toFixed(1)}%"></b></i><em>${esc(value)}</em></div>`;
  }).join("");
  return `<section class="deck-theater-side${record.own_side === side ? " is-own" : ""}"><header><b>${esc(heading)}</b>${record.sides[side].flagship_name ? `<span>⚑ ${esc(record.sides[side].flagship_name)}</span>` : ""}</header>${rows || `<span class="deck-muted">No arrived force detail</span>`}</section>`;
}

function noteHtml(note: RoundNoteView): string {
  const side = note.side === 0 ? "Attackers" : note.side === 1 ? "Defenders" : "Battle";
  const copy: Record<string, string> = {
    joined: `${side} receive reinforcements`,
    retreat_tripped: `${side} trip their retreat threshold`,
    withdraw_ordered: `A withdraw order reaches the ${side.toLowerCase()}`,
    disengage_exposure: `${side} break off under parting fire`,
    platform_destroyed: "Defense Platform destroyed",
    mutual_disengage: "Mutual disengagement",
  };
  return `<span>${icon(note.kind.includes("withdraw") || note.kind.includes("retreat") ? "withdraw" : "battle", "sm")} ${esc(copy[note.kind] ?? human(note.kind))}</span>`;
}

function outcomeText(record: BattleRecordView): string {
  if (!record.outcome) return "Following arrived light";
  const attackerLost = record.outcome === "attacker_destroyed" || record.outcome === "both_destroyed";
  const defenderLost = record.outcome === "target_destroyed" || record.outcome === "both_destroyed";
  if (record.own_side === null) return record.outcome === "both_destroyed" ? "Mutual destruction" : attackerLost ? "Attackers destroyed" : defenderLost ? "Defenders destroyed" : "Both withdrew";
  const ownLost = record.own_side === 0 ? attackerLost : defenderLost;
  const rivalLost = record.own_side === 0 ? defenderLost : attackerLost;
  return ownLost && rivalLost ? "Mutual destruction" : ownLost ? "Defeat" : rivalLost ? "Victory" : "Both withdrew";
}

function truthMap(frame: KeyframeView, ownSide: number | null): string {
  const radius = 1_500;
  const x = (value: number) => (value + radius) / (radius * 2) * 100;
  const y = (value: number) => (value + radius) / (radius * 2) * 100;
  const color = (side: number) => ownSide === null ? side === 0 ? "#ff7a6b" : "#4fc3ff" : side === ownSide ? "#4fc3ff" : "#ff7a6b";
  const ships = frame.ships.map((ship) => `<circle cx="${x(ship.x).toFixed(2)}" cy="${y(ship.y).toFixed(2)}" r="${ship.plat ? 1.7 : 1.1}" fill="${color(ship.side)}" opacity="${Math.max(.25, ship.hp).toFixed(2)}"></circle>`).join("");
  const deaths = frame.deaths.map((death) => `<path d="M${(x(death.x) - 1).toFixed(2)} ${(y(death.y) - 1).toFixed(2)}l2 2m0-2l-2 2" stroke="${color(death.side)}" stroke-width=".45"></path>`).join("");
  return `<div class="deck-theater-truth"><svg viewBox="0 0 100 100" aria-label="Recorded battle positions">${ships}${deaths}</svg></div>`;
}

function countClassOrder(value: string): number {
  return ["one", "two_to_three", "four_to_seven", "eight_to_fifteen", "sixteen_to_thirty", "thirty_one_plus"].indexOf(value);
}

function human(value: string): string { return value.replaceAll("_", " ").replace(/\b\w/g, (letter) => letter.toUpperCase()); }
function esc(value: unknown): string { return String(value ?? "").replace(/[&<>"']/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[character]!); }
function escAttr(value: unknown): string { return esc(value); }
