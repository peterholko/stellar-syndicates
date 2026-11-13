// §charter-advisor (docs/charter-advisor.md): when the Charter Advisor speaks
// and when they nudge, over either shell (boot.ts keeps one). Their moments
// go through the story pop-up queue; what they have said and nudged is
// remembered per account and galaxy in this browser profile, across reloads
// and shell changes. Nothing here reaches the server.
import type { FoundingView } from "./protocol";
import type { StoryEvent, StoryKey } from "./story-events";
import { actionAdvice, adviceKey, advisorEvent, advisorMemoryKey, advisorOnDuty, charterAdvisor,
  type Advisor, type AdvisorMoment } from "./core/derive/advisor";
import { FOUNDING_STEP, type FoundingPlan } from "./core/derive/founding";

/** An action left unstarted this long, in the foreground, gets the nudge. */
export const STALL_NUDGE_MS = 180_000;
/** The raid's moment waits this long after the sighting, so the camera's
 * turn to the raider (boot.ts `noticeRaid`) lands first. */
export const RAID_MOMENT_DELAY_MS = 2_000;
/** The goodbye follows the charter's last pop-ups by this gap. */
export const GOODBYE_GAP_MS = 3_000;

/** Blockers the pirates script (a strike, the colony's fall): their own
 * transmissions tell them, so they are never the advisor's first blocker. */
const SCRIPTED = new Set(["attack-strike", "defend-barge", "restart-corporation"]);
const MOMENTS: readonly AdvisorMoment[] = ["intro", "raid", "left", "blocked", "goodbye", "freighter_lost"];
/** The raider-gone panel follows the raid's report pop-up by this gap. */
export const LEFT_GAP_MS = 2_000;
/** §freighter-lost: the advisor's word follows the pop-up on screen (a
 * pirate's transmission) by this gap. */
export const FREIGHTER_LOST_GAP_MS = 2_000;

/** The pop-up queue, as the desk uses it (`StoryEventUI`). */
export interface StoryQueue {
  readonly open: boolean;
  has(key: StoryKey): boolean;
  show(event: StoryEvent): void;
  showAfterGap(event: StoryEvent, gapMs: number): void;
}

export interface AdvisorDeskHooks {
  founding(): FoundingView | null | undefined;
  /** Whose memory: the galaxy instance and the player. */
  scope(): { instance?: string; playerId: string | null };
  /** Told this session (`ViewState.storyTold`). */
  storyTold(key: StoryKey): boolean;
  /** The guide's plan as it stands. */
  plan(): FoundingPlan | null;
  /** The landing dialog is up: the introduction waits for it. */
  settling(): boolean;
  /** The app is visible and connected. */
  foreground(): boolean;
  /** The guide's next-action control as drawn: not visible, or its state. */
  nextControl(): "none" | "enabled" | "disabled";
  /** Ring the guide's next action, the advisor's face beside the caption. */
  coachNext(caption: string, advisor: Advisor): void;
  /** Fleet orders in flight; a new one means the player is acting. */
  orders(): number;
  /** The advisor is on screen beside the raid's warning (the desktop deck's
   * tracking window), saying the raid's line there. */
  raidVoiced?(): boolean;
  /** §freighter-lost: the freighter hulls the corporation has afloat, as its
   * views report them (every own fleet, every view). */
  freighters?(): number;
  storage?: Pick<Storage, "getItem" | "setItem"> | null;
  timer?(run: () => void, ms: number): void;
}

interface Memory { told: AdvisorMoment[]; nudged: string[] }

export class AdvisorDesk {
  private scopeKey: string | null = null;
  private memory: Memory = { told: [], nudged: [] };
  private scheduled = new Set<AdvisorMoment>();
  private stallKey = "";
  private idleMs = 0;
  private orderCount = 0;
  /** §freighter-lost: the freighter count at the last view. */
  private freighterCount: number | null = null;

  constructor(private readonly stories: StoryQueue, private readonly hooks: AdvisorDeskHooks) {}

  /** Said in this browser profile, for this account and galaxy. */
  told(moment: AdvisorMoment): boolean {
    return this.memory.told.includes(moment);
  }

  /** Nudged here already: `step:line` (see `tick`). */
  nudged(key: string): boolean {
    return this.memory.nudged.includes(key);
  }

  /** On every server message: the moments that follow the founding view. */
  update(): void {
    this.sync();
    const f = this.hooks.founding();
    const advisor = charterAdvisor(f);
    // §freighter-lost: views carry every own fleet and a merge keeps its
    // hulls, so fewer freighters than last time is one lost. Once, while the
    // officer is on duty.
    const freighters = this.hooks.freighters?.() ?? null;
    const lost = freighters !== null && this.freighterCount !== null && freighters < this.freighterCount;
    if (freighters !== null) this.freighterCount = freighters;
    if (lost && advisor && advisorOnDuty(f)) {
      this.speak("freighter_lost", advisor, () => !!advisorOnDuty(this.hooks.founding()), { gapMs: FREIGHTER_LOST_GAP_MS });
    }
    if (!f || !advisor) return;
    // The introduction, once the colony has landed and its landing card closed.
    if (f.stage !== "complete" && !this.hooks.settling()) {
      this.speak("intro", advisor, () => !!advisorOnDuty(this.hooks.founding()) && !this.hooks.settling());
    }
    // The raid, once command sees the raider, until its report is told.
    // §no-spoilers: nothing before `raid_seen`. §smelter-raid: it can come
    // while the Smelter is still the step.
    const raiding = () => {
      const now = this.hooks.founding();
      return !!now && FOUNDING_STEP[now.stage] <= FOUNDING_STEP.pirate_raid && !!now.raid_seen && !now.raided
        && !this.hooks.storyTold("pirates");
    };
    // Peter (Nov 9 2026): on the desktop the advisor speaks inside the raid's
    // warning; the pop-up is for a screen without it (the phone).
    if (raiding()) this.speak("raid", advisor, raiding, { delayMs: RAID_MOMENT_DELAY_MS, voiced: () => !!this.hooks.raidVoiced?.() });
    // §watch-the-pirate (Peter, Nov 9 2026): as the raider drops off the
    // colony's sensors after its raid, a large panel of the advisor's scorn.
    if (this.leftDue()) this.speak("left", advisor, () => this.leftDue(), { gapMs: LEFT_GAP_MS });
    // The goodbye, to a charter they guided.
    if (f.stage === "complete" && this.told("intro")) {
      this.speak("goodbye", advisor, () => this.hooks.founding()?.stage === "complete", { gapMs: GOODBYE_GAP_MS });
    }
  }

  /** §laser-battery-card: the raider-gone panel is still to come (due, and
   * not yet on screen); the Laser Battery's card waits behind it. */
  leftOwed(): boolean {
    this.sync();
    return !this.told("left") && this.leftDue();
  }

  /** The raider has dropped off the colony's sensors after its raid, with
   * an advisor on duty to say so. Not past the trader's step: by then it's
   * old news. */
  private leftDue(): boolean {
    const now = this.hooks.founding();
    return !!now && !!advisorOnDuty(now) && !!now.raided && !!now.raid_lost && FOUNDING_STEP[now.stage] <= FOUNDING_STEP.sell_ore;
  }

  /** Once a second: the first blocker and the stall nudge. */
  tick(dtMs: number): void {
    this.sync();
    const f = this.hooks.founding();
    const advisor = advisorOnDuty(f);
    if (!f || !advisor) {
      this.stallKey = "";
      this.idleMs = 0;
      return;
    }
    const plan = this.hooks.plan();
    const step = plan?.primary ?? null;
    // The first blocker the player can act on: the advisor says so once,
    // then points at its action. Not the scripted raid, not a wait.
    if (plan?.blocked && step && !step.status && !step.quiet && f.stage !== "pirate_raid" && !SCRIPTED.has(step.action)) {
      const still = () => {
        const now = this.hooks.plan();
        return !!now?.blocked && now.primary?.title === step.title;
      };
      this.speak("blocked", advisor, still, { then: () => {
        const now = this.hooks.plan()?.primary;
        if (still() && now) this.hooks.coachNext(now.reason ?? now.title, advisor);
      } });
    }
    // The stall nudge: a new action, or a fresh order, starts the clock again.
    const key = step ? `${f.stage}|${step.action}|${step.title}` : "";
    if (key !== this.stallKey) {
      this.stallKey = key;
      this.idleMs = 0;
    }
    const orders = this.hooks.orders();
    if (orders > this.orderCount) this.idleMs = 0;
    this.orderCount = orders;
    if (!plan || !step || plan.blocked || step.status || step.quiet) return;
    const nudge = `${FOUNDING_STEP[f.stage]}:${adviceKey(step, f.stage)}`;
    if (this.nudged(nudge)) return;
    // Foreground play only: paused while hidden, offline, or behind a pop-up,
    // and while the guide's control is out of sight or disabled.
    if (!this.hooks.foreground() || this.stories.open || this.hooks.settling() || this.hooks.nextControl() !== "enabled") return;
    this.idleMs += dtMs;
    if (this.idleMs < STALL_NUDGE_MS) return;
    const line = actionAdvice(step, f);
    if (!line) return;
    this.memory.nudged.push(nudge);
    this.save();
    this.hooks.coachNext(line, advisor);
  }

  /** The player took the guide's action: its clock starts again. */
  acted(): void {
    this.idleMs = 0;
  }

  /** The pop-up queue put a story on screen: the advisor's are remembered. */
  shown(key: StoryKey): void {
    const moment = MOMENTS.find((m) => key === `advisor_${m}`);
    if (!moment || this.told(moment)) return;
    this.memory.told.push(moment);
    this.save();
  }

  private speak(moment: AdvisorMoment, advisor: Advisor, due: () => boolean,
    opts: { delayMs?: number; gapMs?: number; then?: () => void; voiced?: () => boolean } = {}): void {
    const key: StoryKey = `advisor_${moment}`;
    if (this.told(moment) || this.scheduled.has(moment) || this.stories.has(key)) return;
    const event = advisorEvent(moment, advisor, due, opts.then);
    if (opts.delayMs) {
      const scope = this.scopeKey;
      this.scheduled.add(moment);
      (this.hooks.timer ?? ((run, ms) => setTimeout(run, ms)))(() => {
        this.scheduled.delete(moment);
        if (scope !== this.scopeKey || !due() || this.told(moment) || this.stories.has(key)) return;
        // Said on screen already: remembered as told, no pop-up.
        if (opts.voiced?.()) this.shown(key);
        else this.stories.show(event);
      }, opts.delayMs);
    } else if (opts.gapMs) {
      this.stories.showAfterGap(event, opts.gapMs);
    } else {
      this.stories.show(event);
    }
  }

  /** Load this account's and galaxy's memory when either changes. */
  private sync(): void {
    const { instance, playerId } = this.hooks.scope();
    const key = advisorMemoryKey(instance, playerId);
    if (key === this.scopeKey) return;
    this.scopeKey = key;
    this.memory = { told: [], nudged: [] };
    this.scheduled.clear();
    this.freighterCount = null;
    this.stallKey = "";
    this.idleMs = 0;
    if (!key) return;
    try {
      const raw = this.storage()?.getItem(key);
      const saved = raw ? JSON.parse(raw) as { told?: unknown; nudged?: unknown } : null;
      const list = (value: unknown): string[] => Array.isArray(value) ? value.filter((v): v is string => typeof v === "string") : [];
      this.memory = { told: MOMENTS.filter((m) => list(saved?.told).includes(m)), nudged: list(saved?.nudged) };
    } catch { /* unavailable or corrupt: this session remembers */ }
  }

  private save(): void {
    try {
      if (this.scopeKey) this.storage()?.setItem(this.scopeKey, JSON.stringify(this.memory));
    } catch { /* session-only */ }
  }

  private storage(): Pick<Storage, "getItem" | "setItem"> | null {
    if (this.hooks.storage !== undefined) return this.hooks.storage;
    try { return localStorage; } catch { return null; }
  }
}
