import type { ClientMsg, ServerMsg, TransactionEntry } from "../../protocol";
import type { ViewState } from "../../state";

const PAGE_SIZE = 25; // Mirrors the server's bounded history page, not a retention cap.
let requestSerial = 0;

export interface TransactionHistoryState {
  entries: TransactionEntry[];
  before: number | null;
  nextBefore: number | null;
  previous: (number | null)[];
  requestId: number;
  loaded: boolean;
  loading: boolean;
  requestedWallMs: number;
  newer: boolean;
  since: number | null;
}

export function emptyTransactions(): TransactionHistoryState {
  return { entries: [], before: null, nextBefore: null, previous: [], requestId: 0,
    loaded: false, loading: false, requestedWallMs: 0, newer: false, since: null };
}

/** Fetch locally held CC records on demand, never the whole ledger per View.
 * A new receipt does not pull someone reading an older page back to the top. */
export function requestTransactions(
  ctx: { state: ViewState; send(msg: ClientMsg): void },
  action: "initial" | "older" | "newer" | "latest" = "initial",
): void {
  const h = ctx.state.transactions;
  if (h.loading && Date.now() - h.requestedWallMs > 12_000) {
    // A saturated/disconnected discrete channel may lose a reply. Retry this
    // same page; never leave a permanent spinner or a stale response winning.
    h.loading = false;
    h.loaded = false;
  }
  if (ctx.state.link !== "online" || !ctx.state.playerId || h.loading) return;
  if (action === "initial" && h.loaded) return;
  if (action === "older") {
    if (h.nextBefore === null) return;
    h.previous.push(h.before);
    h.before = h.nextBefore;
  } else if (action === "newer") {
    if (!h.previous.length) return;
    h.before = h.previous.pop()!;
  } else if (action === "latest") {
    h.before = null;
    h.previous = [];
  }
  h.loading = true;
  h.requestedWallMs = Date.now();
  h.requestId = ++requestSerial;
  ctx.send({ type: "RequestTransactions", before: h.before, request_id: h.requestId });
}

export function applyTransactions(
  st: ViewState,
  msg: Extract<ServerMsg, { type: "Transactions" | "TransactionRecorded" }>,
): boolean {
  if (st.playerId !== msg.player_id) return false;
  const h = st.transactions;
  if (msg.type === "Transactions") {
    if (msg.request_id !== h.requestId) return false;
    h.entries = msg.entries;
    h.before = msg.before;
    h.nextBefore = msg.next_before;
    h.since = msg.since;
    h.loading = false;
    h.loaded = true;
    if (h.before === null) h.newer = false;
  } else if (h.loaded && !h.loading && h.before === null) {
    if (h.entries.some((entry) => entry.id === msg.entry.id)) return false;
    if (msg.entry.id !== (h.entries[0]?.id ?? 0) + 1) {
      h.loaded = false; // A missed live notification: reload the authoritative page.
      h.newer = true;
      return true;
    }
    h.entries.unshift(msg.entry);
    if (h.entries.length > PAGE_SIZE) {
      h.entries.length = PAGE_SIZE;
      h.nextBefore = h.entries[PAGE_SIZE - 1].id;
    }
  } else {
    h.newer = true;
  }
  return true;
}
