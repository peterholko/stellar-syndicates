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
  onViewTick(): void;
  framePolicy(): FramePolicy;
  cameraRect(): Rect;
  teardown(): void;
}
