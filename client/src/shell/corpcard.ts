// §medals: the corporation card and medal chips, shared by both shells. Built
// from the published rankings table only (core/derive/medals.ts).
import type { RankingRow } from "../protocol";
import { fmtDays, medalTotals, type CorpCard } from "../core/derive/medals";

const esc = (s: string) => s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c] ?? c);

/** Gold, silver and bronze counts as chips; nothing when there are none. */
export function medalChipsHtml(row: RankingRow): string {
  const [g, s, b] = medalTotals(row);
  if (!g && !s && !b) return "";
  const chip = (kind: string, n: number) => n ? `<i class="medal is-${kind}">${n}</i>` : "";
  return `<span class="medals" title="Weekly medals: ${g} gold, ${s} silver, ${b} bronze">${chip("gold", g)}${chip("silver", s)}${chip("bronze", b)}</span>`;
}

/** The card's body. `back` is the button that returns to the table. */
export function corpCardHtml(card: CorpCard, back: string): string {
  const { row } = card;
  const titles = row.titles.map((t) => `<em>${esc(t)}</em>`).join("");
  const count = (kind: string, n: number) => n ? `<i class="medal is-${kind}">${n}</i>` : "";
  const medals = card.medals.map((m) => `<div class="corp-card__medal"><span>${esc(m.label)}</span>`
    + `<span class="medals">${count("gold", m.gold)}${count("silver", m.silver)}${count("bronze", m.bronze)}</span></div>`).join("")
    || `<p class="corp-card__none">None yet.</p>`;
  const standing = card.standing.map((s) => `<div class="corp-card__rank"><span>${esc(s.label)}</span>`
    + `<b>${s.rank !== null ? `#${s.rank}` : "—"}<small>/${s.of}</small></b><output>${esc(s.text)}</output></div>`).join("");
  return `<article class="corp-card"><header><h2>${esc(row.name)}</h2>${card.me ? `<small>you</small>` : ""}${titles}${medalChipsHtml(row)}</header>`
    + `<section><h3>Medals <small>Week ${card.week} · awards in ${fmtDays(card.left)}</small></h3>${medals}</section>`
    + `<section><h3>Standing</h3>${standing}</section>${back}</article>`;
}
