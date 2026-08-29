import {
  groundTheaterAttach,
  groundTheaterAvailable,
  groundTheaterClose,
  groundTheaterSetTime,
  groundTheaterStep,
} from "../../groundtheater";
import type { GroundRecordView } from "../../protocol";
import { liveSimTime } from "../../state";
import type { CoreContext } from "../types";
import type { SheetEntry, SheetView } from "./sheets";
import { SheetStack } from "./sheets";
import { sheetFingerprint } from "../signature";

const esc = (value: string): string => value.replace(
  /[&<>\"]/g,
  (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '\"': "&quot;" })[character]!,
);
const propsOf = <T extends object>(entry: SheetEntry): Partial<T> => (entry.props && typeof entry.props === "object" ? entry.props : {}) as Partial<T>;

/** Portrait host for the shared, fog-filtered ground record.
 *
 * Playback owns no combat state: a running landing follows only the rounds
 * whose light has arrived, while a concluded record can be replayed and
 * scrubbed. The canvas consumes the same record as desktop at every frame.
 */
export class MobileGroundTheater {
  private readonly renderSignatures = new Map<SheetEntry["id"], string>();
  private id: string | null = null;
  private round = 0;
  private fraction = 0;
  private live = false;
  private playing = false;
  private lastTs = 0;
  private lastFrontier = -1;
  private lastViewport = "";

  constructor(private readonly ctx: CoreContext, private readonly sheets: SheetStack) {}

  render(entry: SheetEntry): SheetView | null {
    if (entry.id !== "ground") return null;
    const id = propsOf<{ id: string }>(entry).id;
    const record = id ? this.ctx.state.groundRecords.find((candidate) => candidate.id === id) : undefined;
    this.bind(id ?? null, record);
    let view: SheetView;
    if (!id) view = { title: "Ground action", eyebrow: "Observed landing", html: `<div class="m-empty">No landing selected.</div>`, detent: "full" };
    else if (!record) {
      view = {
        title: "Ground action",
        eyebrow: "Observed landing · awaiting light",
        html: `<div class="m-empty">This landing record has not reached you.</div>`,
        detent: "full",
      };
    } else {
      const system = this.ctx.state.galaxy?.systems.find((candidate) => candidate.id === record.system);
      view = {
        title: `${record.attacking ? "Your landing" : "Landing"} · ${system?.name ?? "unknown system"}`,
        eyebrow: record.fidelity === "participant" ? "Ground assault · participant record" : "Ground assault · observed from orbit",
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
    if (!button || !action?.startsWith("ground-")) return false;
    const record = this.record();
    const frontier = (record?.rounds.length ?? 0) - 1;
    switch (action) {
      case "ground-play":
        if (!record || record.outcome === null || frontier < 0) break;
        if (!this.playing && this.round >= frontier) this.round = 0;
        this.live = false;
        this.playing = !this.playing;
        this.fraction = 0;
        this.sheets.refresh();
        break;
      case "ground-round": {
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
    }
    return true;
  }

  sync(entry: SheetEntry | null): void {
    if (entry?.id !== "ground") {
      this.close();
      return;
    }
    const record = this.record();
    const mount = document.getElementById("m-ground-stage");
    if (!record || !mount || !groundTheaterAvailable()) {
      groundTheaterClose();
      return;
    }
    const rect = mount.getBoundingClientRect();
    const width = Math.max(280, Math.floor(rect.width));
    const height = Math.max(300, Math.floor(rect.height));
    this.lastViewport = `${width}x${height}`;
    groundTheaterAttach(mount, record, { width, height });
    groundTheaterSetTime(this.round, this.fraction, this.live && this.round >= record.rounds.length - 1);
  }

  tick(now = performance.now()): void {
    if (!this.id) return;
    const record = this.record();
    if (!record) return;
    const frontier = record.rounds.length - 1;
    const dt = this.lastTs ? Math.min(0.25, (now - this.lastTs) / 1000) : 0;
    this.lastTs = now;
    let changed = false;
    if ((this.live || this.playing) && frontier > 0 && this.round < frontier) {
      const hz = this.ctx.state.tickHz || 30;
      const window = Math.max(0.2, (record.rounds[this.round + 1].tick - record.rounds[this.round].tick) / hz);
      this.fraction += dt / window;
      while (this.fraction >= 1 && this.round < frontier) {
        this.round++;
        this.fraction--;
        changed = true;
      }
      if (this.round >= frontier) {
        this.fraction = 0;
        if (this.live && record.outcome !== null) this.live = false;
        if (!this.live) this.playing = false;
        changed = true;
      }
    }
    if (this.live && record.outcome !== null && this.round >= frontier) {
      this.live = false;
      this.playing = false;
      this.fraction = 0;
      changed = true;
    }
    groundTheaterSetTime(this.round, Math.min(1, this.fraction), this.live && this.round >= frontier);
    groundTheaterStep(dt);
    const mount = document.getElementById("m-ground-stage");
    if (mount) {
      const rect = mount.getBoundingClientRect();
      const viewport = `${Math.max(280, Math.floor(rect.width))}x${Math.max(300, Math.floor(rect.height))}`;
      if (viewport !== this.lastViewport) this.sync({ id: "ground", props: { id: this.id } });
    }
    if (frontier !== this.lastFrontier) {
      this.lastFrontier = frontier;
      changed = true;
    }
    if (changed) this.sheets.refresh();
  }

  close(): void {
    if (!this.id && !this.lastViewport) return;
    this.id = null;
    this.live = false;
    this.playing = false;
    this.lastTs = 0;
    this.lastViewport = "";
    groundTheaterClose();
  }

  private bind(id: string | null, record: GroundRecordView | undefined): void {
    if (id === this.id) {
      if (record?.outcome === null) {
        this.live = true;
        this.playing = false;
      }
      return;
    }
    this.id = id;
    const frontier = (record?.rounds.length ?? 0) - 1;
    this.live = record?.outcome === null;
    this.round = this.live ? Math.max(0, frontier) : 0;
    this.fraction = 0;
    this.playing = !this.live && frontier > 0;
    this.lastTs = 0;
    this.lastFrontier = frontier;
    this.lastViewport = "";
  }

  private record(): GroundRecordView | undefined {
    return this.id ? this.ctx.state.groundRecords.find((candidate) => candidate.id === this.id) : undefined;
  }

  private rememberSignature(entry: SheetEntry): void {
    const signature = this.signature(entry);
    if (signature !== null) this.renderSignatures.set(entry.id, signature);
  }

  private signature(entry: SheetEntry): string | null {
    if (entry.id !== "ground") return null;
    const id = propsOf<{ id: string }>(entry).id ?? "";
    const record = id ? this.ctx.state.groundRecords.find((candidate) => candidate.id === id) : undefined;
    const recordSlice = record ? [
      record.id, record.system, record.started_at, record.fidelity, record.attacking,
      record.marines_landed, record.defenders_initial, record.garrison_tiers,
      record.suppression_at_drop, record.outcome,
      record.rounds.map((round) => [round.tick, round.notes ?? []]),
    ] : null;
    return sheetFingerprint([
      Math.floor(liveSimTime()), entry.props ?? null, this.round, this.live, this.playing, recordSlice,
    ]);
  }

  private viewer(record: GroundRecordView): string {
    const frontier = record.rounds.length - 1;
    const running = record.outcome === null;
    const outcome = running ? "FOLLOWING LIGHT" : record.outcome === "taken" ? "GROUND TAKEN" : "LANDING DESTROYED";
    const exact = record.marines_landed !== null && record.defenders_initial !== null;
    const strengths = exact
      ? `${record.marines_landed} marines · ${record.defenders_initial} defenders`
      : "Strengths unresolved from orbit";
    if (frontier < 0) {
      return `<div class="m-ground"><div class="m-battle-status ${running ? "is-live" : "is-complete"}"><i></i><b>${outcome}</b><span>Awaiting the first round's light.</span></div></div>`;
    }
    this.round = Math.max(0, Math.min(this.round, frontier));
    return `<div class="m-ground">` +
      `<div class="m-battle-status ${running ? "is-live" : "is-complete"}"><i></i><b>${outcome}</b><span>Round ${this.round + 1}/${record.rounds.length}${running ? " · arrived prefix" : ""}</span></div>` +
      `<div class="m-ground-summary"><span><b>${esc(strengths)}</b><small>${record.garrison_tiers} garrison tier${record.garrison_tiers === 1 ? "" : "s"}</small></span><em>${Math.round(record.suppression_at_drop * 100)}% pinned at drop</em></div>` +
      `<div class="gt-stage m-ground-stage" id="m-ground-stage"></div>` +
      this.transport(record) + `</div>`;
  }

  private transport(record: GroundRecordView): string {
    const running = record.outcome === null;
    const ticks = record.rounds.map((round, index) => {
      const beat = (round.notes?.length ?? 0) > 0;
      const cls = `${index === this.round ? " is-at" : beat ? " is-beat" : ""}`;
      return running
        ? `<i class="${cls.trim()}"></i>`
        : `<button type="button" class="${cls.trim()}" data-mobile-act="ground-round" data-round="${index}" aria-label="Show ground round ${index + 1}"></button>`;
    }).join("");
    if (running) return `<div class="m-ground-transport"><div class="gt-scrub m-ground-scrub">${ticks}<span class="is-beyond" aria-label="Later ground rounds have not reached you"></span></div><small>Following arrived light at the real landing pace.</small></div>`;
    return `<div class="m-ground-transport"><button type="button" class="m-wide-button" data-mobile-act="ground-play">${this.playing ? "❚❚ Pause replay" : "▶ Play replay"}</button><div class="gt-scrub m-ground-scrub">${ticks}</div><small>Round ${this.round + 1} of ${record.rounds.length}</small></div>`;
  }
}
