// §social (R5): the Comms page — the syndicate channel and corporation mail.
// Text crosses the Authority's relay at once; light lag is only a label.

import type { MailItem } from "../../protocol";
import type { ViewState } from "../../state";
import { ago, markChannelSeen, presenceText } from "../../core/derive/social";
import type { CoreContext } from "../types";
import type { DeckRoute } from "./router";

const esc = (value: string): string => value.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "\"": "&quot;", "'": "&#39;" })[c]!);

/** The Comms page for `query` (tab, folder, mail, compose). */
export function commsHtml(st: ViewState, query: Record<string, string> = {}): string {
  const social = st.social;
  const tab = query.tab === "mail" ? "mail" : "channel";
  const unread = social?.unread ?? 0;
  const tabs = `<nav class="deck-tabs" aria-label="Comms"><button type="button" data-deck-act="strategic-comms-tab" data-tab="channel" aria-selected="${tab === "channel"}">Channel</button><button type="button" data-deck-act="strategic-comms-tab" data-tab="mail" aria-selected="${tab === "mail"}">Mail${unread ? ` <small>${unread}</small>` : ""}</button></nav>`;
  const body = tab === "mail" ? mailHtml(st, query) : channelHtml(st);
  return `<section class="deck-page deck-comms"><header class="deck-page__lead"><span>Authority relay · no light lag</span><h2>Comms</h2></header>${tabs}${body}</section>`;
}

function channelHtml(st: ViewState): string {
  const social = st.social;
  if (!st.syndicate) return `<div class="deck-empty-inline">Join a syndicate to use its channel. <button type="button" data-deck-act="strategic-comms-syndicate">Syndicate</button></div>`;
  markChannelSeen(social);
  const me = st.playerId;
  const muted = new Set(social?.muted ?? []);
  const members = (social?.members ?? []).map(m => `<span class="deck-comms__member${m.online ? " is-online" : ""}"><i></i>${esc(m.name)}<small>${esc(presenceText(m))}</small>${m.player !== me ? `<button type="button" data-deck-act="strategic-comms-mute" data-player="${esc(m.player)}" data-mute="${muted.has(m.player) ? "0" : "1"}" title="${muted.has(m.player) ? "Show their lines and mail" : "Hide their lines and mail"}">${muted.has(m.player) ? "Unmute" : "Mute"}</button>` : ""}</span>`).join("");
  const lines = (social?.channel ?? []).map(line => `<div class="deck-chat-line${line.author === me ? " is-own" : ""}"><b>${esc(line.author_name)}</b><small>${ago(line.at)}</small>${line.author !== me ? `<button type="button" class="deck-chat-report" data-deck-act="strategic-comms-report" data-kind="chat" data-id="${line.id}" title="Report to the operators">⚑</button>` : ""}<p>${esc(line.text)}</p></div>`).join("");
  return `<section class="deck-section"><div class="deck-comms__members">${members}</div><div class="deck-comms__log" role="log" aria-live="polite">${lines || `<div class="deck-empty-inline">No messages yet.</div>`}</div><div class="deck-comms__compose"><input id="deck-chat-input" data-deck-enter="strategic-comms-say" maxlength="400" placeholder="Message ${esc(st.syndicate.name)}" autocomplete="off"><button type="button" class="is-primary" data-deck-act="strategic-comms-say">Send</button></div></section>`;
}

function mailHtml(st: ViewState, query: Record<string, string>): string {
  const social = st.social;
  const folder = query.folder === "sent" ? "sent" : "inbox";
  if (query.compose) return composeHtml(st, query.to ?? "", query.subject ?? "");
  const mails: MailItem[] = (folder === "sent" ? social?.sent : social?.inbox) ?? [];
  const open = mails.find(m => String(m.id) === query.mail);
  const folders = `<div class="deck-comms__folders"><button type="button" data-deck-act="strategic-comms-folder" data-folder="inbox" aria-pressed="${folder === "inbox"}">Inbox</button><button type="button" data-deck-act="strategic-comms-folder" data-folder="sent" aria-pressed="${folder === "sent"}">Sent</button><button type="button" class="is-primary" data-deck-act="strategic-comms-compose">New mail</button></div>`;
  if (open) {
    const who = folder === "sent" ? `To ${open.to_name}` : `From ${open.from_name}`;
    const actions = folder === "inbox"
      ? `<button type="button" class="is-primary" data-deck-act="strategic-comms-compose" data-to="${esc(open.from_name)}" data-subject="${esc(open.subject.startsWith("Re: ") ? open.subject : `Re: ${open.subject}`)}">Reply</button><button type="button" data-deck-act="strategic-comms-report" data-kind="mail" data-id="${open.id}">Report</button>`
      : "";
    return `${folders}<article class="deck-mail-open"><header><button type="button" data-deck-act="strategic-comms-folder" data-folder="${folder}" aria-label="Back">←</button><div><b>${esc(open.subject)}</b><small>${esc(who)} · ${ago(open.at)} ago</small></div></header><p>${esc(open.body)}</p><footer>${actions}<button type="button" class="is-danger" data-deck-act="strategic-comms-delete" data-id="${open.id}">Delete</button></footer></article>`;
  }
  const rows = mails.map(m => `<button type="button" class="deck-mail-row${folder === "inbox" && !m.read ? " is-unread" : ""}" data-deck-act="strategic-comms-open" data-folder="${folder}" data-id="${m.id}" data-unread="${folder === "inbox" && !m.read ? "1" : ""}"><b>${esc(folder === "sent" ? m.to_name : m.from_name)}</b><span>${esc(m.subject)}</span><small>${ago(m.at)}</small></button>`).join("");
  return `${folders}<div class="deck-mail-list">${rows || `<div class="deck-empty-inline">${folder === "sent" ? "Nothing sent." : "No mail."}</div>`}</div>`;
}

function composeHtml(st: ViewState, to: string, subject: string): string {
  const names = [...new Set(st.rankings.filter(r => r.player_id !== st.playerId).map(r => r.name))].sort();
  return `<section class="deck-section deck-mail-compose"><label>To<input id="deck-mail-to" list="deck-mail-corps" maxlength="40" value="${esc(to)}" autocomplete="off"></label><datalist id="deck-mail-corps">${names.map(n => `<option value="${esc(n)}"></option>`).join("")}</datalist><label>Subject<input id="deck-mail-subject" maxlength="80" value="${esc(subject)}"></label><label>Message<textarea id="deck-mail-body" maxlength="2000" rows="8"></textarea></label><div class="deck-comms__compose-actions"><button type="button" class="is-primary" data-deck-act="strategic-comms-send">Send</button><button type="button" data-deck-act="strategic-comms-folder" data-folder="inbox">Cancel</button></div></section>`;
}

/** Handle a `strategic-comms-*` action. */
export function commsAction(button: HTMLButtonElement, ctx: CoreContext, route: DeckRoute, go: (route: DeckRoute) => void): boolean {
  const action = button.dataset.deckAct ?? "";
  const d = button.dataset;
  const value = (id: string) => (document.getElementById(id) as HTMLInputElement | HTMLTextAreaElement | null)?.value ?? "";
  const query = route.query ?? {};
  if (action === "strategic-comms-tab") go({ name: "comms", query: { tab: d.tab ?? "channel" } });
  else if (action === "strategic-comms-syndicate") go({ name: "syndicate" });
  else if (action === "strategic-comms-say") {
    const text = value("deck-chat-input").trim();
    if (text) ctx.send({ type: "SyndicateSay", text });
    const input = document.getElementById("deck-chat-input") as HTMLInputElement | null;
    if (input) { input.value = ""; input.focus(); }
  } else if (action === "strategic-comms-folder") go({ name: "comms", query: { tab: "mail", folder: d.folder ?? "inbox" } });
  else if (action === "strategic-comms-open" && d.id) {
    if (d.unread) ctx.send({ type: "MarkMailRead", id: Number(d.id) });
    go({ name: "comms", query: { tab: "mail", folder: d.folder ?? "inbox", mail: d.id } });
  } else if (action === "strategic-comms-compose") {
    go({ name: "comms", query: { tab: "mail", compose: "1", ...(d.to ? { to: d.to } : {}), ...(d.subject ? { subject: d.subject } : {}) } });
  } else if (action === "strategic-comms-send") {
    const to = value("deck-mail-to").trim(), body = value("deck-mail-body").trim();
    if (!to || !body) return true;
    ctx.send({ type: "SendMail", to, subject: value("deck-mail-subject"), body });
    go({ name: "comms", query: { tab: "mail", folder: "sent" } });
  } else if (action === "strategic-comms-delete" && d.id) {
    ctx.send({ type: "DeleteMail", id: Number(d.id) });
    go({ name: "comms", query: { tab: "mail", folder: query.folder ?? "inbox" } });
  } else if (action === "strategic-comms-mute" && d.player) {
    ctx.send({ type: "MuteCorp", player: d.player, mute: d.mute === "1" });
  } else if (action === "strategic-comms-report" && d.id && (d.kind === "chat" || d.kind === "mail")) {
    ctx.send({ type: "ReportMessage", kind: d.kind, id: Number(d.id), reason: "reported in Comms" });
    button.disabled = true;
    button.textContent = "Reported";
  } else return false;
  return true;
}
