const px = (value: string): number => Number.parseFloat(value) || 0;

/** Owns the fixed map chrome as one collision-free band. The strip stays
 * centered in the unobscured map while space permits; when either side rail
 * would touch it, the strip docks above the bottom row. At the narrowest map
 * widths zoom takes a second row above Founding, never a z-index gamble. */
export class DeckBottomBand {
  private frame: number | null = null;
  private readonly observer: ResizeObserver;

  constructor(
    private readonly root: HTMLElement,
    private readonly strip: HTMLElement,
    private readonly founding: HTMLElement,
    private readonly zoom: HTMLElement,
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
    const stripWidth = this.strip.hidden ? 0 : this.strip.offsetWidth;
    const stripHeight = this.strip.hidden ? 0 : this.strip.offsetHeight;
    const foundingWidth = this.founding.hidden ? 0 : this.founding.offsetWidth;
    const foundingHeight = this.founding.hidden ? 0 : this.founding.offsetHeight;
    const zoomWidth = this.zoom.hidden ? 0 : this.zoom.offsetWidth;
    const zoomHeight = this.zoom.hidden ? 0 : this.zoom.offsetHeight;

    const stripLeft = (width - stripWidth) / 2;
    const stripRight = stripLeft + stripWidth;
    const zoomLeft = width - zoomWidth;
    const foundingTouchesZoom = foundingWidth > 0 && zoomWidth > 0 && foundingWidth + gap > zoomLeft;
    const zoomBottom = foundingTouchesZoom ? foundingHeight + gap : 0;
    const stripTouchesBottom = stripWidth > 0 && (
      (foundingWidth > 0 && foundingWidth + gap > stripLeft)
      || (zoomWidth > 0 && stripRight + gap > zoomLeft)
    );
    const bottomExtent = Math.max(foundingHeight, zoomBottom + zoomHeight);
    const stripBottom = stripTouchesBottom ? bottomExtent + gap : 0;
    const occupiedHeight = Math.max(bottomExtent, stripBottom + stripHeight);

    this.root.style.setProperty("--deck-strip-bottom", `${stripBottom}px`);
    this.root.style.setProperty("--deck-zoom-bottom", `${zoomBottom}px`);
    this.root.style.setProperty("--deck-bottom-band-height", `${occupiedHeight}px`);
  }

  teardown(): void {
    this.observer.disconnect();
    if (this.frame !== null) cancelAnimationFrame(this.frame);
    this.frame = null;
    this.root.style.removeProperty("--deck-strip-bottom");
    this.root.style.removeProperty("--deck-zoom-bottom");
    this.root.style.removeProperty("--deck-bottom-band-height");
  }
}
