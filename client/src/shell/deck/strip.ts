export class DeckCommandStrip {
  constructor(private readonly root: HTMLElement) {}

  clear(): void {
    this.root.replaceChildren();
  }
}
