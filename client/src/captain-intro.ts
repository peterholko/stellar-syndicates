// §captain-intro (Peter, Oct 21 2026): assigning a captain opens a panel that
// introduces them (portrait, rank, specialty, background, attributes), not
// just a confirm. One native modal shared by both shells.
import "./styles/settlement.css";
import "./styles/captain-intro.css";
import { CAPTAIN_ATTRIBUTES, CAPTAIN_LORE, attributeEffect } from "./core/derive/captain-lore";
import { captainTitle } from "./core/derive/captains";
import { label } from "./icons";
import type { CaptainRosterView } from "./protocol";
import type { FleetCommand } from "./core/fleetorders";
import { shipKindLabel } from "./core/derive/fleet";
import type { ViewState } from "./state";

export interface CaptainIntro {
  captain: CaptainRosterView;
  /** The hull they would command, e.g. "Interceptor". */
  fleet: string;
  assign(): void;
}

let dialog: HTMLDialogElement | null = null;
let pending: CaptainIntro | null = null;

const esc = (s: string) => s.replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);

export function captainIntroHtml({ captain, fleet }: Pick<CaptainIntro, "captain" | "fleet">): string {
  const report = captain.report;
  const lore = CAPTAIN_LORE[captain.portrait];
  const age = report?.portrait_age ?? "young";
  const rank = report ? captainTitle(report.title) : "Officer";
  const stats = report ? CAPTAIN_ATTRIBUTES.map(attribute => {
    const value = report.attributes[attribute];
    const effect = attributeEffect(attribute, value);
    return `<div${attribute === lore?.focus ? ` class="is-focus"` : ""}><dt>${esc(label(attribute))}</dt><dd><b>${value}</b>${effect ? `<small>${esc(effect)}</small>` : ""}</dd></div>`;
  }).join("") : "";
  return `<div class="captain-intro"><header class="captain-intro__head"><small>Your captain</small>`
    + `<button type="button" class="captain-intro__close" data-captain-act="close" aria-label="Close">✕</button></header>`
    + `<div class="captain-intro__body"><img class="captain-intro__portrait" src="/art/captains/${encodeURIComponent(`${captain.portrait}_${age}`)}.png" alt="Portrait of ${esc(captain.name)}" width="512" height="512" decoding="async">`
    + `<div class="captain-intro__who"><h1 id="captain-intro-title" tabindex="-1">${esc(captain.name)}</h1>`
    + (lore ? `<p class="captain-intro__rank">${esc(label(lore.focus))} specialist</p>` : "")
    + (lore ? `<p class="captain-intro__bio">${esc(lore.background)}</p>` : "")
    + (stats ? `<dl class="captain-intro__stats">${stats}</dl>` : "")
    // Rank, not its command points (Peter, Oct 21 2026: "authority" read as the Authority).
    + (report ? `<p class="captain-intro__level">Level ${report.level} · ${esc(rank)}</p>` : "")
    + `</div></div><footer class="captain-intro__foot"><button type="button" data-captain-act="close">Not now</button>`
    + `<button type="button" class="settlement-primary" data-captain-act="assign">Assign to ${esc(fleet)}</button></footer></div>`;
}

/** For a guide's one-click AssignCaptain: introduce the captain, and send
 * the order (`send`) only on Assign. False when it is not such an order. */
export function introduceCaptainFor(st: ViewState, commands: readonly FleetCommand[], send: () => void): boolean {
  const order = commands.length === 1 && commands[0].type === "AssignCaptain" ? commands[0] : null;
  const captain = order ? st.captains.find(entry => entry.id === order.captain_id) : undefined;
  const fleet = order ? st.ghosts.find(entry => entry.id === order.fleet_id) : undefined;
  if (!captain || !fleet) return false;
  showCaptainIntro({ captain, fleet: shipKindLabel(fleet.kind), assign: send });
  return true;
}

/** Introduce `captain`; Assign runs `assign` (the order's ordinary path). */
export function showCaptainIntro(intro: CaptainIntro): void {
  if (!dialog) {
    dialog = document.createElement("dialog");
    dialog.className = "settlement-dialog captain-intro-dialog";
    dialog.setAttribute("aria-labelledby", "captain-intro-title");
    document.body.append(dialog);
    dialog.addEventListener("click", event => {
      const act = (event.target as Element).closest<HTMLElement>("[data-captain-act]")?.dataset.captainAct;
      if (act === "assign") pending?.assign();
      if (act === "assign" || act === "close") dialog?.close();
    });
    dialog.addEventListener("close", () => { pending = null; });
  }
  pending = intro;
  dialog.innerHTML = captainIntroHtml(intro);
  if (!dialog.open) dialog.showModal();
  dialog.querySelector<HTMLButtonElement>("[data-captain-act=\"assign\"]")?.focus();
}
