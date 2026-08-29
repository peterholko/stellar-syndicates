export interface DeckToast {
  html: string;
  tone?: "quiet" | "good" | "warn" | "bad";
}

export class DeckToasts {
  constructor(private readonly root: HTMLElement) {}

  push(toast: DeckToast): void {
    const item = document.createElement("div");
    item.className = `deck-toast deck-toast--${toast.tone ?? "quiet"}`;
    item.innerHTML = toast.html;
    this.root.prepend(item);
  }

  teardown(): void {
    this.root.replaceChildren();
  }
}
