// §goods-tabs (Peter, Nov 8 2026): the desktop's Resources · Parts · Modules
// strip over a list of goods (Hub market, free trader, stores). No strip when
// the list fills only one tab.
import { GOODS_TAB_LABEL, type GoodsTab } from "../../core/derive/goods";

export function goodsTabsHtml(act: string, active: GoodsTab, tabs: readonly GoodsTab[], aria: string): string {
  if (tabs.length < 2) return "";
  return `<nav class="deck-tabs deck-goods-tabs" aria-label="${aria}">`
    + tabs.map((tab) => `<button type="button" data-deck-act="${act}" data-goods-tab="${tab}" aria-selected="${tab === active}">${GOODS_TAB_LABEL[tab]}</button>`).join("")
    + `</nav>`;
}
