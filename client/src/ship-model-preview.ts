import { SHIP_MODELS } from "./ship-models.generated";
import type { ShipKind } from "./protocol";
import type { ModelPreview } from "./ship-model-preview-runtime";

/** A shadow root lets the live panel morph retain its canvas, camera and
 * controls. Only a changed `kind` reloads the model, never a stock/queue update.
 * `--ship-preview-height` sizes the stage; `turntable` sways the hull slowly
 * about its showcase angle until the player takes the camera. */
class ShipModelPreview extends HTMLElement {
  static observedAttributes = ["kind", "turntable"];
  private view = this.attachShadow({ mode: "open" });
  private engine: ModelPreview | null = null;
  private observer: IntersectionObserver | null = null;
  private generation = 0;
  private visible = false;
  private starting = false;

  connectedCallback(): void {
    if (!this.view.firstChild) this.createView();
    this.updatePoster();
    this.observer?.disconnect();
    this.observer = new IntersectionObserver(entries => {
      this.visible = entries.some(entry => entry.isIntersecting);
      this.engine?.setVisible(this.visible);
      if (this.visible && !this.engine) void this.start();
    });
    this.observer.observe(this);
  }

  disconnectedCallback(): void {
    // A DOM morph may move a keyed element within the same document. Preserve
    // that preview; a real route close releases its WebGL context and textures.
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

  attributeChangedCallback(name: string): void {
    if (!this.view.firstChild) return;
    if (name === "turntable") {
      this.engine?.setTurntable(this.hasAttribute("turntable"));
      return;
    }
    this.updatePoster();
    const kind = this.kind;
    if (kind && this.engine) void this.engine.setKind(kind);
  }

  private get kind(): ShipKind | null {
    const value = this.getAttribute("kind") ?? "";
    return Object.hasOwn(SHIP_MODELS, value) ? value as ShipKind : null;
  }

  private createView(): void {
    this.view.innerHTML = `<style>
      :host{display:block;isolation:isolate;margin:12px 0;color:#dbe8f5;font:12px system-ui,sans-serif;min-width:0}
      *{box-sizing:border-box}.stage{position:relative;height:var(--ship-preview-height,250px);overflow:hidden;border:1px solid #294054;border-radius:8px;background:radial-gradient(ellipse at 50% 43%,#203647 0%,#101c28 48%,#080f19 100%)}
      .stage::before{content:"";position:absolute;inset:0;pointer-events:none;background:linear-gradient(#40576816 1px,transparent 1px),linear-gradient(90deg,#40576816 1px,transparent 1px);background-size:32px 32px;mask-image:linear-gradient(transparent,#0008)}
      canvas,.poster{display:block;position:absolute;width:100%;height:100%;inset:0}.poster{object-fit:contain;padding:8px;pointer-events:none}
      canvas{touch-action:none;outline-offset:-4px;cursor:grab}canvas:active{cursor:grabbing}
      [data-state="ready"] .poster{visibility:hidden}.stage:not([data-state="ready"]) canvas{visibility:hidden}
      .status{position:absolute;bottom:10px;left:12px;right:12px;display:flex;align-items:center;gap:8px;color:#b9cbdb;pointer-events:none}
      .status button{pointer-events:auto}.stage[data-state="ready"] .status{display:none}
      .controls{display:flex;flex-wrap:wrap;align-items:center;gap:4px;margin-top:7px}
      button{appearance:none;font:inherit;color:#b8cbdc;background:#111d29;border:1px solid #304353;border-radius:4px;min-width:32px;min-height:30px;padding:4px 7px;cursor:pointer}
      button:hover,button[aria-pressed="true"]{background:#1e3b4b;border-color:#609eb6;color:#f0fbff}
      button:focus-visible,canvas:focus-visible{outline:2px solid #75d7f4;outline-offset:2px}
      button:disabled{opacity:.45;cursor:default}.zoom{margin-left:auto;display:flex;gap:4px}
      .help{margin:7px 0 0;color:#91a9bd;font-size:11px}.retry[hidden]{display:none}
      @media(max-width:500px){.stage{height:min(var(--ship-preview-height,220px),320px)}button{min-height:36px}.controls{gap:5px}}
    </style><div class="stage" data-state="loading">
      <img class="poster" alt="" decoding="async"><canvas tabindex="0" role="img"></canvas>
      <div class="status" role="status"><span>Loading model…</span><button class="retry" type="button" hidden>Retry</button></div>
    </div><div class="controls" role="group" aria-label="Ship model views">
      ${["Orbit", "Top", "Bottom", "Side", "Front", "Back"].map(view => `<button type="button" data-view="${view.toLowerCase()}" aria-pressed="${view === "Orbit"}" disabled>${view}</button>`).join("")}
      <span class="zoom"><button type="button" data-zoom="in" aria-label="Zoom in on model" disabled>+</button><button type="button" data-zoom="out" aria-label="Zoom out on model" disabled>−</button></span>
    </div><p class="help">Drag to rotate · scroll or pinch to zoom · arrow keys to turn</p>`;
    this.view.addEventListener("click", event => {
      const button = (event.target as Element).closest<HTMLButtonElement>("button");
      if (!button || button.disabled) return;
      if (button.classList.contains("retry")) {
        this.engine?.dispose(); this.engine = null;
        // A released/lost WebGL context belongs to its canvas. Retry with a
        // fresh canvas rather than handing that dead context to a new renderer.
        const canvas = document.createElement("canvas");
        canvas.tabIndex = 0; canvas.setAttribute("role", "img");
        this.view.querySelector("canvas")!.replaceWith(canvas);
        this.updatePoster();
        void this.start(); return;
      }
      if (button.dataset.view) {
        this.engine?.setTurntable(false);
        this.engine?.setView(button.dataset.view);
        for (const item of this.view.querySelectorAll("[data-view]")) item.setAttribute("aria-pressed", String(item === button));
      } else if (button.dataset.zoom) this.engine?.zoom(button.dataset.zoom === "in" ? .8 : 1.25);
    });
  }

  private updatePoster(): void {
    const kind = this.kind;
    if (!kind) { this.setStatus("error"); return; }
    const model = SHIP_MODELS[kind];
    const poster = this.view.querySelector<HTMLImageElement>(".poster")!;
    if (!poster.src.endsWith(model.poster)) poster.src = model.poster;
    poster.alt = `${model.title} model`;
    this.view.querySelector("canvas")!.setAttribute("aria-label", `${model.title} 3D preview. Drag or use arrow keys to rotate.`);
    for (const item of this.view.querySelectorAll<HTMLButtonElement>("[data-view]")) item.setAttribute("aria-pressed", String(item.dataset.view === "orbit"));
  }

  private setStatus(status: "loading" | "ready" | "error"): void {
    this.view.querySelector<HTMLElement>(".stage")!.dataset.state = status;
    this.view.querySelector(".status span")!.textContent = status === "error" ? "3D preview unavailable · showing ship image" : "Loading model…";
    this.view.querySelector<HTMLButtonElement>(".retry")!.hidden = status !== "error";
    for (const button of this.view.querySelectorAll<HTMLButtonElement>(".controls button")) button.disabled = status !== "ready";
  }

  private async start(): Promise<void> {
    if (this.starting || !this.kind || !this.isConnected) return;
    this.starting = true;
    this.setStatus("loading");
    const generation = ++this.generation;
    try {
      const { ModelPreview } = await import("./ship-model-preview-runtime");
      if (generation !== this.generation || !this.isConnected) return;
      this.engine = new ModelPreview(this.view.querySelector("canvas")!, status => this.setStatus(status));
      this.engine.setVisible(this.visible);
      this.engine.setTurntable(this.hasAttribute("turntable"));
      await this.engine.setKind(this.kind!);
    } catch (error) {
      if (generation !== this.generation) return;
      this.engine?.dispose(); this.engine = null;
      this.setStatus("error");
      console.warn("Ship preview unavailable", error);
    } finally { if (generation === this.generation) this.starting = false; }
  }
}

if (!customElements.get("ship-model-preview")) customElements.define("ship-model-preview", ShipModelPreview);
