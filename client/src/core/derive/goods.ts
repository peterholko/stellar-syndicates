// §goods-tabs (Peter, Nov 8 2026: "tabs at the markets, free trader and even
// stockpile for resources, module / parts"): one split of every list of goods,
// shared by the Hub market, the free trader and the stores in both shells.
import type { Commodity } from "../../protocol";
import { isModuleGood } from "./equipment";
import { lightBase } from "./fleet";

export type GoodsTab = "resources" | "parts" | "modules";
export const GOODS_TABS: readonly GoodsTab[] = ["resources", "parts", "modules"];
export const GOODS_TAB_LABEL: Readonly<Record<GoodsTab, string>> = { resources: "Resources", parts: "Parts", modules: "Modules" };

/** What a hull is built from: reactors, drives and frames of every size, the
 * Nav Computer, the Cargo Bay and the guns (sim `Commodity::is_part`). */
const PART_BASES: ReadonlySet<string> = new Set([
  "fission_reactor", "fusion_reactor", "advanced_fusion_reactor", "antimatter_reactor",
  "zephyr_drive", "gale_drive", "tempest_drive", "maelstrom_drive",
  "hull_sections", "nav_computer", "cargo_bay", "pulse_laser", "beam_battery",
]);

/** A good's tab: a module, a hull part, or a resource (ores, materials, fuel,
 * food and munitions). */
export function goodsTab(c: Commodity): GoodsTab {
  if (isModuleGood(c)) return "modules";
  return PART_BASES.has(lightBase(c)) ? "parts" : "resources";
}

/** The tabs a list of goods fills, in tab order. */
export function goodsTabsOf(goods: readonly Commodity[]): GoodsTab[] {
  const seen = new Set(goods.map(goodsTab));
  return GOODS_TABS.filter((tab) => seen.has(tab));
}

/** The tab to show: the chosen one while it has goods, else the first that does. */
export function pickGoodsTab(chosen: GoodsTab, available: readonly GoodsTab[]): GoodsTab {
  return available.includes(chosen) ? chosen : available[0] ?? chosen;
}

export const isGoodsTab = (value: string | undefined): value is GoodsTab => (GOODS_TABS as readonly string[]).includes(value ?? "");
