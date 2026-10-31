// §syndicate-projects (R6): shared reads of the weekly ladder and the bonuses,
// for both shells.

import type { Commodity, SyndicateBonus, SyndicateProjectsView, SystemStateView } from "../../protocol";

export const BONUS_LABEL: Record<SyndicateBonus, string> = { freight: "Freight", research: "Research", migration: "Migration" };
const BONUS_WHAT: Record<SyndicateBonus, string> = { freight: "freighter holds", research: "Academy output", migration: "settlers" };
const ROMAN = ["0", "I", "II", "III", "IV", "V"];

export const roman = (n: number): string => ROMAN[n] ?? String(n);

type Bonus = SyndicateProjectsView["bonuses"][number];

/** "Freight II · +12% freighter holds". */
export function bonusLine(b: Bonus): string {
  return b.level ? `${BONUS_LABEL[b.bonus]} ${roman(b.level)} · +${Math.round(b.per_level * b.level * 100)}% ${BONUS_WHAT[b.bonus]}`
    : `${BONUS_LABEL[b.bonus]} · +${Math.round(b.per_level * 100)}% ${BONUS_WHAT[b.bonus]} a level`;
}

/** The rung in progress, or null once the week's ladder is done. */
export function currentRung(p: SyndicateProjectsView): SyndicateProjectsView["rungs"][number] | null {
  return p.rungs[p.rung] ?? null;
}

/** Own worlds holding the rung's goods, most first: [system, units]. */
export function givingWorlds(systems: SystemStateView[], me: string | null, goods: Commodity): [SystemStateView, number][] {
  return systems
    .filter((s) => s.owner === me)
    .map((s): [SystemStateView, number] => [s, Math.floor(s.stockpile?.find((slot) => slot.commodity === goods)?.units ?? 0)])
    .filter(([, units]) => units > 0)
    .sort((a, b) => b[1] - a[1]);
}
