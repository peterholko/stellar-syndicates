import { fmtEta } from "../../core/derive/format";
import { operationSystemName } from "../../core/derive/geo";
import { liveSimTime, state } from "../../state";
import { $, esc } from "./mapchrome";


// --- §syndicates: the alliance panel (top-navbar destination) ------------------
// Create / invite (by corp name) / accept / leave / dissolve. Strictly owner-only
// content — the View only carries YOUR roster + YOUR pending invites, never a
// rival's. Non-engagement itself is mechanical (server-side); this panel is how
// you form the pact. Re-rendered only when the roster/invites CHANGE (a signature
// guard), so a half-typed name is never wiped by a 10 Hz View.
export let lastSyndicateSig = "";

export function openSyndicate(): void {
  $("syndicate-panel").classList.add("is-open");
  $("nav-syndicate").classList.add("is-active");
  lastSyndicateSig = ""; // force a fresh render on open
  updateSyndicatePanel();
}

export function closeSyndicate(): void {
  $("syndicate-panel").classList.remove("is-open");
  $("nav-syndicate").classList.remove("is-active");
}

export function toggleSyndicate(): void {
  if ($("syndicate-panel").classList.contains("is-open")) closeSyndicate();
  else openSyndicate();
}

export function updateSyndicatePanel(): void {
  const el = $("syndicate-panel");
  if (!el.classList.contains("is-open")) return;
  const s = state.syndicate;
  const invites = state.syndicateInvites;
  const sig = JSON.stringify([s, invites, state.playerId, state.diplomacy, state.selectedSystemId]);
  if (sig === lastSyndicateSig && el.innerHTML) return; // no roster change → keep DOM (+ any typing)
  lastSyndicateSig = sig;
  let body = "";
  if (s) {
    const roster = s.members
      .map((m) => {
        const tag = m.id === state.playerId ? `<span class="you">you · ${esc(m.role)}</span>` : `<span class="fdr">${esc(m.role)}</span>`;
        const maySet = s.is_founder && m.id !== s.founder && m.id !== state.playerId;
        const controls = maySet ? `<span style="margin-left:auto;display:flex;gap:3px">` +
          (["member", "quartermaster", "officer"] as const).map((role) => `<button class="act" data-sy="role" data-member="${esc(m.id)}" data-role="${role}" title="Set ${role}" style="padding:2px 5px">${role[0].toUpperCase()}</button>`).join("") + `</span>` : "";
        return `<div class="sy-row">🟢 <span>${esc(m.name)}</span>${tag}${controls}</div>`;
      })
      .join("");
    body += `<div><div class="sy-sub">Syndicate</div><div class="sy-name">🤝 ${esc(s.name)}</div></div>`;
    body += `<div><div class="sy-sub">Members (${s.members.length})</div>${roster}</div>`;
    if (["founder", "officer"].includes(s.my_role)) {
      const invited = s.invited.length ? `<div class="sy-note">Invited: ${s.invited.map(esc).join(", ")}</div>` : "";
      body += `<div><div class="sy-sub">Invite a corp (by name)</div><div class="sy-inv"><input id="sy-invite-name" type="text" placeholder="corp name" maxlength="32" />` +
        `<button class="act" data-sy="invite">Invite</button></div>${invited}</div>`;
    }
    const dissolve = s.is_founder ? `<button class="act act--danger" data-sy="dissolve">Dissolve</button>` : "";
    body += `<div class="sy-inv"><button class="act act--danger" data-sy="leave">Leave</button>${dissolve}</div>`;
    if (["founder", "officer", "quartermaster"].includes(s.my_role)) {
      body += `<div><div class="sy-sub">Shared operation</div><div class="sy-note" style="border-top:0;padding-top:3px">Select a member-owned system on the map, then begin a three-stage syndicate project there.</div>` +
        `<button class="act" data-sy="project" ${state.selectedSystemId ? "" : "disabled"}>Start at ${state.selectedSystemId ? esc(operationSystemName(state.selectedSystemId)) : "selected system"}</button></div>`;
    }
    body += `<div class="sy-note">Members never auto-engage each other, and can't raid / attack / blockade one another. Ally ships & systems tint <b style="color:#9df0b3">green</b> as their membership light reaches you.</div>`;
  } else {
    body += `<div class="sy-note">A syndicate is a mutual non-engagement pact: members can't raid, attack, or blockade each other, and their pickets leave allies alone.</div>`;
    body += `<div><div class="sy-sub">Found a syndicate</div><div class="sy-inv"><input id="sy-create-name" type="text" placeholder="syndicate name" maxlength="32" />` +
      `<button class="act act--primary" data-sy="create">Create</button></div></div>`;
    if (invites.length) {
      const list = invites
        .map((i) => `<div class="sy-row">🤝 <span>${esc(i.name)}</span><button class="act" data-sy="accept" data-sid="${esc(i.id)}" style="margin-left:auto">Accept</button></div>`)
        .join("");
      body += `<div><div class="sy-sub">Invitations</div>${list}</div>`;
    } else {
      body += `<div class="sy-note">No pending invitations.</div>`;
    }
  }
  const dip = state.diplomacy;
  body += `<div><div class="sy-sub">Diplomacy</div>`;
  if (dip?.incoming.length) {
    body += dip.incoming.map((p) => `<div class="sy-row"><span>${esc(p.name)} proposes ${esc(p.treaty.replace("_", " "))}</span><span style="margin-left:auto;display:flex;gap:4px"><button class="act" data-sy="treaty-response" data-proposal="${p.id}" data-accept="1">Accept</button><button class="act" data-sy="treaty-response" data-proposal="${p.id}" data-accept="0">Decline</button></span></div>`).join("");
  }
  if (dip?.relations.length) {
    body += dip.relations.map((r) => `<div class="sy-row"><span>${esc(r.name)}</span><span class="fdr">${r.war_activates_at ? `war in ${fmtEta(r.war_activates_at - liveSimTime())}` : r.reprisal_until && r.reprisal_until > liveSimTime() ? `reprisal · ${fmtEta(r.reprisal_until - liveSimTime())}` : esc(r.state.replace("_", " "))}</span>` +
      (["non_aggression", "ceasefire"].includes(r.state) ? `<button class="act" data-sy="cancel-treaty" data-target="${esc(r.other)}" style="margin-left:auto;padding:2px 5px">End</button>` : "") + `</div>`).join("");
  } else {
    body += `<div class="sy-note" style="border-top:0;padding-top:3px">No arrived bilateral agreements or declarations.</div>`;
  }
  body += `<div class="sy-inv"><input id="sy-dip-name" type="text" placeholder="corporation name" maxlength="32" /><button class="act" data-sy="propose-nap">Offer pact</button><button class="act" data-sy="propose-ceasefire">Offer ceasefire</button><button class="act act--danger" data-sy="declare-war">Declare war</button></div>`;
  body += `<div class="sy-note">War activates only after the declaration reaches the target and its notice window expires. Treaty cancellation and syndicate departure have the same no-surprise separation protection.</div></div>`;
  el.innerHTML = `<div class="pp-head"><b>SYNDICATE</b><button class="pp-close" data-sy="close" title="Close">✕</button></div><div class="pp-body">${body}</div>`;
}

