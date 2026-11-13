import type * as intentCore from "../core/intent";
import type { CoreEvent } from "../core/events";
import type { Net } from "../net";
import type { ClientMsg } from "../protocol";
import type { Renderer } from "../render";
import type { ViewState } from "../state";

export type IntentMachine = typeof intentCore;

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface CoreContext {
  state: ViewState;
  net: Net;
  renderer: Renderer;
  intent: IntentMachine;
  send(msg: ClientMsg): void;
}

export interface FramePolicy {
  /// Zero follows the display refresh; mobile caps both presentation work and
  /// Pixi's ticker at 30 Hz to fit phone CPU/GPU budgets.
  maxFps: number;
  /// False only when an opaque full-height mobile sheet covers the map. Theater
  /// rendering remains independent and continues while the galaxy ticker rests.
  renderGalaxy: boolean;
}

export interface Shell {
  mount(root: HTMLElement, ctx: CoreContext): Promise<void>;
  onCore(events: CoreEvent[]): void;
  /** §raid-focus: take the player to an incoming raid on the galaxy map: the
   * raider selected, framed with the home it closes on. */
  focusRaid?(id: string, homeId: string): void;
  /** §export-follow: take the player to the first shipment leaving home, the
   * freighter selected and framed with home, and follow it out of the well. */
  focusShipment?(id: string, homeId: string): void;
  /** §privateer-contract: Contracts (Operations), the charter's contract
   * pinned on top with its Accept ringed. */
  openContract?(): void;
  onViewTick(): void;
  framePolicy(): FramePolicy;
  cameraRect(): Rect;
  teardown(): void;
}
