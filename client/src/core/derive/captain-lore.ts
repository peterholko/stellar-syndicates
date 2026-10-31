// §captain-intro (Peter, Oct 21 2026): who each captain is, for the panel that
// introduces one. The focus mirrors sim::captain::Captain::founding (the
// attribute a captain starts with at 2); a background is a line or two.
import type { CaptainAttribute, CaptainPortrait } from "../../protocol";

export const CAPTAIN_LORE: Record<CaptainPortrait, { focus: CaptainAttribute; background: string }> = {
  mara_venn: { focus: "command", background: "Flew escort on the Charter's first freight lanes and never lost a hull. Few words on the bridge; crews follow anyway." },
  elias_rook: { focus: "logistics", background: "Ran tanker manifests at the Market Hub before ever taking a helm. Counts every unit of fuel twice, and is rarely short." },
  sera_okafor: { focus: "navigation", background: "Plotted jump corridors for the Survey Office until the budget cuts. Feels a bad spool before the instruments do." },
  jun_arclight: { focus: "fieldcraft", background: "Six years of salvage on derelicts beyond the patrolled lanes. Reads a wreck the way others read a chart." },
  astrid_nystrom: { focus: "navigation", background: "Hauled ice through the outer belts, threading rock at speed. Prefers the long, clean jump to the short, crowded one." },
  henrik_sondergaard: { focus: "command", background: "Commanded an Authority cutter, then resigned rather than fire on a stranded freighter. Steady under fire, and stubborn about it." },
  luca_ferraro: { focus: "logistics", background: "Kept a mining colony fed through a blockade with favours and spare parts. Knows what every crate is worth, and who needs it." },
  karim_ben_youssef: { focus: "fieldcraft", background: "Charted unclaimed systems for prospecting syndicates. Patient, meticulous, first to notice what doesn't belong." },
  mateo_quispe: { focus: "navigation", background: "Learned to fly in thin air and bad weather on mountain shuttle runs. Calls space the easy part." },
  litia_naivalu: { focus: "command", background: "Led a corsair-hunting picket for the free traders. Calm voice, quick orders, a long memory for pirate names." },
  ana_luisa_nascimento: { focus: "logistics", background: "Wrote the supply schedule that kept three outposts alive through their first year. Nothing leaves that dock half-loaded." },
  elena_valdes: { focus: "fieldcraft", background: "A geologist turned surveyor turned captain, to reach the rocks no one else would. Has a nose for rich veins." },
  daphne_markou: { focus: "command", background: "Came up through the merchant marine and fought off pirates at nineteen. Believes a drilled crew wins before the shooting starts." },
  han_min_jae: { focus: "logistics", background: "Ran fleet maintenance for a freight cartel. Stretches a tank of Deuterium further than the specs allow." },
  nadine_ilunga: { focus: "navigation", background: "Courier pilot on the long Hub routes, famous for never missing a delivery window. Trusts the plot over rumour." },
  daichi_mori: { focus: "fieldcraft", background: "Former recon pilot who spent years watching pirate lanes from cold, dark berths. Misses nothing, says less." },
  chen_jianyu: { focus: "command", background: "Held a colony through its first raid with two patched gunboats. Plans for the worst, and is usually ready." },
  nattaya_chantarat: { focus: "logistics", background: "A decade as a customs officer at the Market Hub. Knows every trick of the trade, and every way cargo goes missing." },
  lin_xiaoyu: { focus: "navigation", background: "Test pilot for experimental jump drives; brought three of them home. Unshakeable, and a little reckless." },
  reina_kuroda: { focus: "fieldcraft", background: "Grew up aboard a deep-space survey tender and charted a first system at fourteen. Signals and anomalies are favourite puzzles." },
  yoon_seo_yeon: { focus: "logistics", background: "Authority supply officer who left to work for independents. Precise, unflappable, fond of a tidy manifest." },
};

export const CAPTAIN_ATTRIBUTES: readonly CaptainAttribute[] = ["command", "navigation", "fieldcraft", "logistics"];

/** What an attribute's points above 1 do, from sim::captain's multipliers;
 * "" while it does nothing yet. */
export function attributeEffect(attribute: CaptainAttribute, value: number): string {
  const over = Math.max(0, value - 1);
  const pct = attribute === "fieldcraft" ? Math.min(15, over * 3) : Math.min(10, over * 2);
  if (!pct) return "";
  return attribute === "command" ? `+${pct}% damage` : attribute === "navigation" ? `−${pct}% jump spool`
    : attribute === "fieldcraft" ? `−${pct}% survey time` : `−${pct}% fuel use`;
}
