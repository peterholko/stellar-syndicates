const px = (value: string): number => Number.parseFloat(value) || 0;
/** The strip's widest (deck.css) and narrowest workable width (px): below the
 * latter its 320 px layout no longer holds the Transit control. */
const STRIP_MAX_PX = 720;
const STRIP_MIN_PX = 280;

/** Owns the fixed map chrome as one collision-free band. The strip stays
 * centered in the unobscured map while space permits; when either side rail
 * would touch it, it squeezes into the bottom row between them, and docks
 * above that row only when the gap is too narrow. At the narrowest map
 * widths zoom takes a second row above Founding, never a z-index gamble. */
export class DeckBottomBand {
  private frame: number | null = null;
  private layoutSignature = "";
  private readonly observer: ResizeObserver;

  constructor(
    private readonly root: HTMLElement,
    private readonly strip: HTMLElement,
    private readonly founding: HTMLElement,
    private readonly zoom: HTMLElement,
    private readonly onLayout: () => void,
    signal: AbortSignal,
  ) {
    this.observer = new ResizeObserver(() => this.schedule());
    for (const node of [root, strip, founding, zoom]) this.observer.observe(node);
    signal.addEventListener("abort", () => this.teardown(), { once: true });
    this.schedule();
  }

  occupiedHeight(): number {
    return px(this.root.style.getPropertyValue("--deck-bottom-band-height"));
  }

  refresh(): void {
    this.schedule();
  }

  private schedule(): void {
    if (this.frame !== null) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = null;
      this.layout();
    });
  }

  private layout(): void {
    const width = this.root.clientWidth;
    const gap = px(getComputedStyle(document.documentElement).getPropertyValue("--space-3"));
    const stripHeight = this.strip.hidden ? 0 : this.strip.offsetHeight;
    const foundingWidth = this.founding.hidden ? 0 : this.founding.offsetWidth;
    const foundingHeight = this.founding.hidden ? 0 : this.founding.offsetHeight;
    const zoomWidth = this.zoom.hidden ? 0 : this.zoom.offsetWidth;
    const zoomHeight = this.zoom.hidden ? 0 : this.zoom.offsetHeight;

    const zoomLeft = width - zoomWidth;
    const foundingTouchesZoom = foundingWidth > 0 && zoomWidth > 0 && foundingWidth + gap > zoomLeft;
    const zoomBottom = foundingTouchesZoom ? foundingHeight + gap : 0;
    const bottomExtent = Math.max(foundingHeight, zoomBottom + zoomHeight);
    // The strip is centred while it clears Founding and zoom. Otherwise it
    // squeezes into the bottom row between them, narrower and taller (Peter,
    // Oct 27 2026: not "floating in the middle", but "at the bottom beside
    // the tutorial panel"). Only a gap narrower than its narrowest layout
    // sends it above the bottom row. Zoom never moves for it. A hidden strip
    // keeps its place, so it appears there at once.
    const natural = Math.min(STRIP_MAX_PX, width - 2 * gap);
    const centred = (width - natural) / 2;
    const zoomInRow = zoomWidth > 0 && zoomBottom === 0;
    const clears = (foundingWidth === 0 || foundingWidth + gap <= centred)
      && (!zoomInRow || centred + natural + gap <= zoomLeft);
    const slotLeft = foundingWidth > 0 ? foundingWidth + gap : 0;
    const slot = (zoomInRow ? zoomLeft - gap : width) - slotLeft;
    const squeezed = !clears && slot >= STRIP_MIN_PX;
    const stripWidth = squeezed ? Math.min(natural, slot) : natural;
    const stripLeft = squeezed ? slotLeft + (slot - stripWidth) / 2 : centred;
    const stripBottom = !clears && !squeezed ? bottomExtent + gap : 0;
    const occupiedHeight = Math.max(bottomExtent, stripHeight > 0 ? stripBottom + stripHeight : 0);

    this.root.style.setProperty("--deck-strip-left", `${stripLeft}px`);
    this.root.style.setProperty("--deck-strip-width", `${stripWidth}px`);
    this.root.style.setProperty("--deck-strip-bottom", `${stripBottom}px`);
    this.root.style.setProperty("--deck-zoom-bottom", `${zoomBottom}px`);
    this.root.style.setProperty("--deck-bottom-band-height", `${occupiedHeight}px`);
    // The merchant and shipyard pop-outs stand above the bottom row, whatever
    // the tutorial's height. (The planet view reserves a fixed control row
    // instead: it resizes only with the window.)
    const row = Math.max(stripHeight, zoomHeight);
    document.documentElement.style.setProperty("--deck-bottom-row-inset", `${row > 0 ? row + gap : 0}px`);
    const signature = [width, stripLeft, stripWidth, stripHeight, foundingWidth, foundingHeight, zoomWidth, zoomHeight, stripBottom, zoomBottom].join(":");
    if (signature !== this.layoutSignature) {
      this.layoutSignature = signature;
      this.onLayout();
    }
  }

  teardown(): void {
    this.observer.disconnect();
    if (this.frame !== null) cancelAnimationFrame(this.frame);
    this.frame = null;
    this.layoutSignature = "";
    this.root.style.removeProperty("--deck-strip-left");
    this.root.style.removeProperty("--deck-strip-width");
    this.root.style.removeProperty("--deck-strip-bottom");
    this.root.style.removeProperty("--deck-zoom-bottom");
    this.root.style.removeProperty("--deck-bottom-band-height");
    document.documentElement.style.removeProperty("--deck-bottom-row-inset");
  }
}
