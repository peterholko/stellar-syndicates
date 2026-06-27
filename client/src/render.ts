// Pixi.js renderer. For M1 this is a deliberately near-blank galaxy canvas: a
// static starfield plus an on-canvas readout of the player's id and the live
// authoritative tick, proving the per-player stream is flowing. The real
// delayed/fogged galaxy map arrives in M2/M3; the scene-graph scaffolding here
// is built to grow into it.

import { Application, Container, Graphics, Text, TextStyle } from "pixi.js";
import type { ViewState } from "./state";
import { formatId } from "./protocol";

export class Renderer {
  private app = new Application();
  private world = new Container();
  private centerTick!: Text;
  private centerLabel!: Text;
  private idLabel!: Text;

  async init(mount: HTMLElement): Promise<void> {
    await this.app.init({
      background: "#05070d",
      resizeTo: window,
      antialias: true,
      autoDensity: true,
      resolution: window.devicePixelRatio || 1,
    });
    mount.appendChild(this.app.canvas);

    this.app.stage.addChild(this.world);
    this.drawStarfield();

    const labelStyle = new TextStyle({
      fill: "#6b7a90",
      fontFamily: "ui-monospace, monospace",
      fontSize: 13,
      letterSpacing: 2,
    });
    this.centerLabel = new Text({ text: "AUTHORITATIVE TICK", style: labelStyle });
    this.centerLabel.anchor.set(0.5);

    this.centerTick = new Text({
      text: "—",
      style: new TextStyle({
        fill: "#4fc3ff",
        fontFamily: "ui-monospace, monospace",
        fontSize: 64,
        fontWeight: "700",
      }),
    });
    this.centerTick.anchor.set(0.5);

    this.idLabel = new Text({
      text: "",
      style: new TextStyle({
        fill: "#46506a",
        fontFamily: "ui-monospace, monospace",
        fontSize: 12,
      }),
    });
    this.idLabel.anchor.set(0.5);

    this.app.stage.addChild(this.centerLabel, this.centerTick, this.idLabel);
    this.layout();
    window.addEventListener("resize", () => this.layout());
  }

  private drawStarfield(): void {
    // Deterministic-ish scatter (not gameplay; purely decorative).
    const stars = new Graphics();
    let s = 0x12345;
    const rand = () => {
      s = (s * 1103515245 + 12345) & 0x7fffffff;
      return s / 0x7fffffff;
    };
    for (let i = 0; i < 400; i++) {
      const x = rand() * 2200;
      const y = rand() * 1400;
      const r = rand() * 1.4 + 0.2;
      const a = rand() * 0.5 + 0.15;
      stars.circle(x, y, r).fill({ color: 0xb8c6dd, alpha: a });
    }
    this.world.addChild(stars);
  }

  private layout(): void {
    const cx = this.app.renderer.width / this.app.renderer.resolution / 2;
    const cy = this.app.renderer.height / this.app.renderer.resolution / 2;
    this.centerLabel.position.set(cx, cy - 52);
    this.centerTick.position.set(cx, cy);
    this.idLabel.position.set(cx, cy + 46);
  }

  // Reflect the latest view each frame.
  update(state: ViewState): void {
    this.centerTick.text = state.link === "online" ? state.tick.toLocaleString() : "—";
    this.centerTick.style.fill = state.link === "online" ? "#4fc3ff" : "#46506a";
    this.idLabel.text = state.playerId !== null
      ? `${state.name}  ·  id ${formatId(state.playerId)}`
      : "";
  }
}
