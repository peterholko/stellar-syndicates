import { SHIP_MODELS } from "./ship-models.generated";
import type { ShipArtKind } from "./shipart";
import { HULL_YARD, hullFill, isYardKind, plateUrls, YARD_SCENES, type YardKind } from "./shipyard-scene";
import type { ConstructionScene } from "./ship-construction-runtime";

/** §shipyard-view (Peter, Oct 21 2026): a hull under construction in its
 * yard: `<ship-construction-view kind="raider" yard="shipyard" progress="0.4">`.
 * The yard's interior plate, and over it the hull's live 3D model, built from
 * the stern as `progress` (0–1) rises; no `progress` while it waits for a slip.
 * A shadow root keeps the canvas through a panel's morph; only a new `kind`
 * reloads the model. The hull's image stands in until it loads, or for good
 * without WebGL. */
class ShipConstructionView extends HTMLElement {
  static observedAttributes = ["kind", "yard", "progress"];
  private view = this.attachShadow({ mode: "open" });
  private engine: ConstructionScene | null = null;
  private observer: IntersectionObserver | null = null;
  private generation = 0;
  private visible = false;
  private starting = false;

  connectedCallback(): void {
    if (!this.view.firstChild) this.createView();
    this.update();
    this.observer?.disconnect();
    this.observer = new IntersectionObserver(entries => {
      this.visible = entries.some(entry => entry.isIntersecting);
      this.engine?.setVisible(this.visible);
      if (this.visible && !this.engine) void this.start();
    });
    this.observer.observe(this);
  }

  disconnectedCallback(): void {
    // A morph may move this element within the document: keep its scene then.
    queueMicrotask(() => {
      if (this.isConnected) return;
      this.generation++;
      this.starting = false;
      this.observer?.disconnect();
      this.observer = null;
      this.engine?.dispose();
      this.engine = null;
      this.setStatus("loading");
    });
  }

  attributeChangedCallback(): void {
    if (this.view.firstChild) this.update();
  }

  private get kind(): ShipArtKind | null {
    const value = this.getAttribute("kind") ?? "";
    return Object.hasOwn(SHIP_MODELS, value) ? value as ShipArtKind : null;
  }

  private get yard(): YardKind {
    const value = this.getAttribute("yard");
    return isYardKind(value) ? value : (this.kind && HULL_YARD[this.kind]) || "shipyard";
  }

  /** 0–1 built; null while the hull waits for a slip. */
  private get progress(): number | null {
    const value = this.getAttribute("progress");
    const number = value === null || value === "" ? NaN : Number(value);
    return Number.isFinite(number) ? Math.max(0, Math.min(1, number)) : null;
  }

  private createView(): void {
    this.view.innerHTML = `<style>
      :host{display:block;min-width:0}
      .stage{position:relative;aspect-ratio:16/9;overflow:hidden;border-radius:8px;background:#05070d}
      .plate,canvas{position:absolute;inset:0;width:100%;height:100%}
      .plate{object-fit:cover}
      canvas{transform:translate(calc((var(--cx) - .5) * 100%),calc((var(--cy) - .5) * 100%))}
      .poster{position:absolute;left:calc(var(--cx) * 100%);top:calc(var(--cy) * 100%);width:calc(var(--fill) * 100%);transform:translate(-50%,-50%);opacity:.9;pointer-events:none}
      [data-state="ready"] .poster{visibility:hidden}.stage:not([data-state="ready"]) canvas{visibility:hidden}
    </style><div class="stage" data-state="loading"><img class="plate" alt="" decoding="async"><img class="poster" alt="" decoding="async"><canvas role="img"></canvas></div>`;
  }

  private update(): void {
    const kind = this.kind;
    const yard = this.yard;
    const scene = YARD_SCENES[yard];
    const stage = this.view.querySelector<HTMLElement>(".stage")!;
    const plate = this.view.querySelector<HTMLImageElement>(".plate")!;
    const urls = plateUrls(yard);
    if (plate.getAttribute("src") !== urls.src) {
      plate.srcset = urls.srcset;
      plate.sizes = "(max-width: 900px) 100vw, 860px";
      plate.src = urls.src;
    }
    const fill = kind ? hullFill(kind, yard) : scene.fill;
    stage.style.setProperty("--cx", String(scene.cx));
    stage.style.setProperty("--cy", String(scene.cy));
    stage.style.setProperty("--fill", String(fill));
    const poster = this.view.querySelector<HTMLImageElement>(".poster")!;
    const progress = this.progress;
    if (kind) {
      if (!poster.src.endsWith(SHIP_MODELS[kind].poster)) poster.src = SHIP_MODELS[kind].poster;
      this.view.querySelector("canvas")!.setAttribute("aria-label", `${SHIP_MODELS[kind].title} under construction${
        progress === null ? ", waiting for a slip" : `, ${Math.round(progress * 100)}% built`}`);
    }
    if (kind && this.engine) void this.engine.show(kind, scene, fill);
    this.engine?.setProgress(progress);
  }

  private setStatus(status: "loading" | "ready" | "error"): void {
    this.view.querySelector<HTMLElement>(".stage")!.dataset.state = status;
  }

  private async start(): Promise<void> {
    if (this.starting || !this.kind || !this.isConnected) return;
    this.starting = true;
    const generation = ++this.generation;
    try {
      const { ConstructionScene } = await import("./ship-construction-runtime");
      if (generation !== this.generation || !this.isConnected) return;
      this.engine = new ConstructionScene(this.view.querySelector("canvas")!, status => this.setStatus(status));
      this.engine.setVisible(this.visible);
      this.engine.setProgress(this.progress);
      const yard = this.yard;
      await this.engine.show(this.kind!, YARD_SCENES[yard], hullFill(this.kind!, yard));
    } catch (error) {
      if (generation !== this.generation) return;
      this.engine?.dispose(); this.engine = null;
      this.setStatus("error");
      console.warn("Shipyard view unavailable", error);
    } finally { if (generation === this.generation) this.starting = false; }
  }
}

if (!customElements.get("ship-construction-view")) customElements.define("ship-construction-view", ShipConstructionView);
