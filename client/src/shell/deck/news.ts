// §season: the Galactic News Network's headline ticker, at the end of the
// stores bar under the top bar (docs/endgame-keystone.md). One line: the
// newest item this corporation has heard; a click opens the Log's News tab.

import { ageLabel } from "../../core/derive/format";
import { newsEmblem, newsKindLabel, newsSourceName, tickerItem } from "../../core/derive/season";
import { liveSimTime } from "../../state";
import { setHtml } from "../dom";
import type { CoreContext } from "../types";

const esc = (s: string) => s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);

export class DeckNewsTicker {
  private signature = "";

  constructor(
    private readonly el: HTMLButtonElement,
    private readonly ctx: CoreContext,
    open: () => void,
    signal: AbortSignal,
  ) {
    el.addEventListener("click", open, { signal });
  }

  render(): void {
    const item = tickerItem(this.ctx.state.season);
    const now = liveSimTime();
    const since = item ? Math.max(0, now - item.heard_at) : 0;
    const age = !item ? "" : since < 60 ? "now" : ageLabel(since);
    const signature = item ? `${item.id}:${age}` : "";
    if (signature === this.signature) return;
    this.signature = signature;
    this.el.hidden = !item;
    if (!item) return;
    const source = newsSourceName(item.source);
    this.el.classList.toggle("is-yours", item.about_you);
    this.el.classList.toggle("is-free", item.source === "free_signal");
    this.el.title = `${source} · ${newsKindLabel(item.kind)}`;
    setHtml(this.el, `<img src="${newsEmblem(item.source)}" alt="${source}" decoding="async"><span>${esc(item.headline)}</span><small>${esc(age)}</small>`);
  }
}
