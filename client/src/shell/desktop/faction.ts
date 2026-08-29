import { fmt } from "../../core/derive/format";
import { TCA_INCIDENT_LOSS_UI } from "../../core/derive/orders";
import { projectedBand } from "../../core/derive/research";
import { state } from "../../state";
import { $, badge, esc } from "./mapchrome";



// --- §TCA Phase 2: the FACTION panel — your charter status --------------------
// A top-navbar destination of its own (⚖ / `C`), beside Syndicate: the charter is
// WHO YOU ARE to the Authority, not something you shop for, so it no longer rides
// along in a Market tab. The Market's Freight tab still books the carrier this
// market terms. Re-rendered only when the charter CHANGES (a signature guard),
// so a half-typed reinstatement figure is never wiped by a 10 Hz View.
export let lastFactionSig = "";

export function openFaction(): void {
  $("faction-panel").classList.add("is-open");
  $("nav-faction").classList.add("is-active");
  lastFactionSig = ""; // force a fresh render on open
  updateFactionPanel();
}

export function closeFaction(): void {
  $("faction-panel").classList.remove("is-open");
  $("nav-faction").classList.remove("is-active");
}

export function toggleFaction(): void {
  if ($("faction-panel").classList.contains("is-open")) closeFaction();
  else openFaction();
}


/// The charter chip + band ladder + (when it bites) the live cost of the band,
/// and the reinstatement control. Rendered as a LEGAL STATUS — a ladder of named
/// bands with their thresholds — rather than a reputation bar, because that is
/// what it is: priced outlawry, with the price written down.
export function updateFactionPanel(): void {
  const el = $("faction-panel");
  if (!el.classList.contains("is-open")) return;
  // Never rebuild under the player's fingers (the reinstatement field).
  const ae = document.activeElement;
  if (ae && el.contains(ae) && ae.tagName === "INPUT") return;
  const ch = state.charter;
  const sig = JSON.stringify([ch, state.charterLadder]);
  if (sig === lastFactionSig && el.innerHTML) return;
  lastFactionSig = sig;
  if (!ch) {
    el.innerHTML = factionShell(`<div class="fp-note">No charter on file yet.</div>`);
    return;
  }
  const tone =
    ch.status === "good_standing" ? "positive"
    : ch.status === "sanctioned" ? "neutral"
    : ch.status === "suspended" ? "warn"
    : "negative";
  const rows = state.charterLadder
    .map(([title, at], i) => {
      const active = title === ch.title;
      // The first row is the ceiling ("at 100"); the rest read as "below/at N".
      const bound = i === 0 ? `at ${at.toFixed(0)}` : i === 1 ? `below ${at.toFixed(0)}` : `at ${at.toFixed(0)}`;
      return `<div class="ord${active ? " is-active" : ""}"${active ? ' style="font-weight:600"' : ""}>` +
        `${active ? "▸ " : "· "}${esc(title)} <span class="dim">${bound}</span></div>`;
    })
    .join("");
  // What the band is costing right now — shown only when it actually bites.
  const bites = ch.tariff_mult > 1.0 || ch.market_penalty_frac > 0;
  const cost = bites
    ? `<div class="mhint">Freight tariff <b>×${ch.tariff_mult.toFixed(2)}</b> · Exchange penalty ` +
      `<b>${(ch.market_penalty_frac * 100).toFixed(1)}%</b> of trade value.</div>`
    : `<div class="mhint dim">In good standing you pay no tariff and no Exchange penalty.</div>`;
  const shortfall = Math.max(0, ch.max_standing - ch.standing);
  const pay = shortfall > 0
    ? `<div class="composer__row"><label>Reinstate</label>` +
      `<input type="number" id="ch-points" min="1" step="1" value="${Math.ceil(Math.min(shortfall, 20))}" style="width:6em" />` +
      `<button class="act" id="ch-pay" title="Buy charter standing back from the Authority. The credits are burned, and you are only ever charged for points actually restored.">Pay</button>` +
      `<span class="dim" id="ch-cost"></span></div>`
    : "";
  el.innerHTML = factionShell(
    `<div><div class="fp-sub">Your charter</div><div class="fp-name">⚖ Terran Charter Authority</div></div>` +
    `<div class="sp-line">${badge(tone, esc(ch.title))} <b>${ch.standing.toFixed(0)}</b><span class="dim">/${ch.max_standing.toFixed(0)}</span></div>` +
    `<div class="mkt-orders">${rows}</div>` +
    cost +
    pay +
    `<div class="fp-note">The Authority issued your charter and runs the Hub Exchange. Standing is a legal status, not a reputation: each band names a tariff on Authority freight and a cut of every Exchange trade. Citations land only when their light reaches the Market Hub; standing regenerates in the meantime.</div>`,
  );
  if (shortfall > 0) syncReinstateCost();
}


/// The faction panel's chrome (head + body), shared by the empty and live states.
export function factionShell(body: string): string {
  return `<div class="pp-head"><b>FACTION</b><button class="pp-close" data-fp="close" title="Close">✕</button></div>` +
    `<div class="pp-body">${body}</div>`;
}


/// Live cost preview for the reinstatement control.
export function syncReinstateCost(): void {
  const ch = state.charter;
  const inp = $("ch-points") as HTMLInputElement | null;
  const out = $("ch-cost");
  if (!ch || !inp || !out) return;
  const want = Math.max(0, Math.floor(Number(inp.value) || 0));
  const restorable = Math.max(0, ch.max_standing - ch.standing);
  const points = Math.min(want, restorable);
  out.textContent = `${fmt(points * ch.reinstate_cost_per_point)} Cr for ${points.toFixed(0)} pts`;
}


/// Confirm a hostile act against an Authority hull. NEVER a hard block — the
/// whole design is priced outlawry, so the player is told the price and then
/// allowed to pay it.
export function confirmAuthorityHostility(what: string): boolean {
  const band = projectedBand(TCA_INCIDENT_LOSS_UI);
  return window.confirm(
    `${what}\n\nThis will be CITED by the Terran Charter Authority once its light reaches the Wormhole Hub.\n` +
      `Projected charter status: ${band}.\n\nProceed?`,
  );
}

