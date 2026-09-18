import type { TransactionEntry } from "../protocol";
import type { ViewState } from "../state";
import { label } from "../icons";
import { rejectText } from "../core/derive/format";
import "../styles/transactions.css";

const esc = (text: string): string => text.replace(/[&<>\"]/g,
  (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '\"': "&quot;" })[c]!);
const money = (n: number): string => n.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });

/** Game timestamps, not the browser's receipt wall clock. */
export function transactionTime(time: number): string {
  const s = Math.max(0, Math.floor(time));
  return `T+${Math.floor(s / 3600).toString().padStart(2, "0")}:${Math.floor(s / 60 % 60).toString().padStart(2, "0")}:${(s % 60).toString().padStart(2, "0")}`;
}

interface TransactionSummary {
  action: string;
  item: string;
  units: number | null;
  unitPrice: number | null;
  fees: number | null;
  net: number | null;
  note: string;
}

export function transactionSummary(entry: TransactionEntry, state: ViewState): TransactionSummary {
  const d = entry.details;
  const result: TransactionSummary = { action: "", item: "", units: null, unitPrice: null, fees: null, net: null, note: "" };
  const place = (id: string | null): string => id
    ? state.galaxy?.systems.find((system) => system.id === id)?.name ?? "System" : "Market Warehouse";
  if (d.kind === "earlier_report") {
    return { ...result, action: "Earlier report", note: d.text };
  }
  if (d.kind === "service") {
    return { ...result, action: d.name, net: -d.cost };
  }
  if (d.kind === "purchase" || d.kind === "sale") {
    const fees = d.kind === "purchase" ? d.fees : 0;
    return { ...result, action: d.kind === "purchase" ? "Purchased" : "Sold", item: label(d.item),
      units: d.units, unitPrice: d.unit_price, fees,
      net: d.units * d.unit_price * (d.kind === "purchase" ? -1 : 1) - fees,
      note: d.kind === "purchase" ? (d.fleet ? "Fleet refuelling" : place(d.system)) : "" };
  }
  const t = d.trade;
  if ("commodity" in t) result.item = label(t.commodity);
  if ("units" in t) result.units = t.units;
  switch (t.event) {
    case "Bought": case "Sold": case "LimitFilled": {
      const buy = t.event === "Bought" || (t.event === "LimitFilled" && t.side === "buy");
      result.action = t.event === "LimitFilled" ? `Limit ${buy ? "buy" : "sell"} filled` : buy ? "Bought" : "Sold";
      result.unitPrice = t.unit_price;
      result.fees = t.penalty ?? 0;
      // Trade value, NOT wallet delta: limit orders already reserved credits.
      result.net = t.units * t.unit_price * (buy ? -1 : 1) - result.fees;
      break;
    }
    case "LimitPlaced": case "LimitCancelled":
      result.action = `Limit ${t.side} ${t.event === "LimitPlaced" ? "placed" : "cancelled"}`;
      result.note = `Limit ${money(t.limit_price)} Cr`; break;
    case "Delivered": result.action = "Delivered"; result.note = place(t.system); break;
    case "Loaded": result.action = "Loaded"; result.note = `From ${place(t.system)}`; break;
    case "Unloaded": result.action = "Unloaded"; result.note = `Into ${place(t.system)}`; break;
    case "SellDispatched": result.action = "Sale dispatched"; break;
    case "AutoDispatched": result.action = "Auto-dispatched"; result.note = place(t.source); break;
    case "StockDispatched": result.action = "Stock dispatched"; result.note = place(t.system); break;
    case "SupplyDiverted": result.action = "Cargo diverted"; result.note = label(t.action); break;
    case "StorageOverflow": result.action = "Storage overflow"; result.note = place(t.system); break;
    case "Rejected": result.action = "Rejected"; result.note = rejectText(t); break;
    case "FreightBooked":
      result.action = "Freight booked"; result.fees = t.fee; result.net = -t.fee;
      result.note = t.direction === "outbound" ? `To ${place(t.system)}` : `From ${place(t.system)}`; break;
    case "FreightMoved": result.action = label(t.stage); result.note = place(t.system); break;
    case "CharterReinstated": result.action = "Charter reinstated"; result.net = -t.cost; break;
  }
  return result;
}

export function transactionsHtml(state: ViewState): string {
  const h = state.transactions;
  const online = state.link === "online";
  const button = (action: string, text: string, enabled: boolean): string =>
    `<button type="button" data-deck-act="transactions-${action}" data-mobile-act="transactions-${action}"${enabled && online && !h.loading ? "" : " disabled"}>${text}</button>`;
  const rows = h.entries.map((entry) => {
    const row = transactionSummary(entry, state);
    const amount = (n: number | null): string => n === null ? "—" : money(n);
    const time = transactionTime(entry.occurred_at ?? entry.reported_at);
    const received = `Report received ${transactionTime(entry.reported_at)}`;
    return `<tr data-transaction-id="${entry.id}"><td class="market-transactions__time" title="${received}">${time}${entry.occurred_at === null ? "<small>report time</small>" : ""}</td>` +
      `<td class="market-transactions__detail"><b>${esc(row.action)}</b>${row.item ? `<span>${esc(row.item)}</span>` : ""}${row.note ? `<small>${esc(row.note)}</small>` : ""}</td>` +
      `<td data-label="Qty">${row.units === null ? "—" : row.units.toLocaleString()}</td><td data-label="Unit · Cr">${amount(row.unitPrice)}</td>` +
      `<td data-label="Fees · Cr">${amount(row.fees)}</td><td data-label="Net · Cr" class="${row.net === null || row.net === 0 ? "" : row.net > 0 ? "is-credit" : "is-debit"}">${row.net === null ? "—" : `${row.net > 0 ? "+" : row.net < 0 ? "−" : ""}${money(Math.abs(row.net))}`}</td></tr>`;
  }).join("");
  return `<section class="market-transactions" aria-label="Transaction history" aria-busy="${h.loading}">` +
    `<header><h3>Transactions</h3><span>${online ? "Received reports" : "Offline · saved reports"}</span></header>` +
    `<div class="market-transactions__pages">${button("newer", "Newer", h.previous.length > 0)}${button("older", "Older", h.nextBefore !== null)}${button("latest", h.newer ? "New transactions · Latest" : h.before === null ? "Refresh" : "Latest", true)}</div>` +
    (rows ? `<table><thead><tr><th>Time</th><th>Transaction</th><th>Qty</th><th>Unit · Cr</th><th>Fees · Cr</th><th>Net · Cr</th></tr></thead><tbody>${rows}</tbody></table>`
      : `<p class="market-transactions__empty">${h.loading ? "Loading transactions…" : !h.loaded ? "Connect to load transaction history." : "No transactions received yet."}</p>`) +
    `<footer>Net = sale proceeds or purchase cost, including fees.${h.since !== null && h.since > 1 ? ` Full records from ${transactionTime(h.since)}; older reports may be partial.` : ""}</footer></section>`;
}
