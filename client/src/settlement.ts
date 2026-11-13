import type { BodyView, ClientMsg, Commodity, FoundingProfile, ServerMsg, SettlementProject, SystemStateView } from "./protocol";
import type { ViewState } from "./state";
import { label, structureImage } from "./icons";
import { WARP_FACTOR } from "./core/derive/fleet";
import { FOUNDING_SURFACES, planetSurfaceArt, type HabitableSurface } from "./planet-surfaces";
import { clearStoredInvite, storedInvite } from "./core/derive/social";

export const FOUNDING_STORY = [
  { art: "wormhole-opening-selected", title: "Beyond the Milky Way",
    text: "The Terran Charter Authority has opened a wormhole to another galaxy. Beyond its shimmering gate lie uncharted stars, untapped riches, and worlds waiting for their first settlers. The first expeditions are crossing. Your future waits on the other side.",
    alt: "Expedition ships leave Earth for a blue wormhole far out in open space, with another galaxy visible through the gate." },
  { art: "stellar-charter", title: "Stellar Charter",
    text: "{corporation} has been granted a stellar charter by the Terran Charter Authority, the privilege to settle a new planet. Seven thousand settlers are ready to begin a new life under your leadership. The Authority will not forget what you owe.",
    alt: "A civilian official hands a planetary charter to an expedition leader aboard an orbital station." },
  { art: "first-landing", title: "A world to call home",
    text: "Start with a small settlement, not an empire. Develop your colony, turn its resources into trade, and decide what your corporation will become.",
    alt: "Settlers unload their landing ship beside the first habitats and storehouses in a green river valley." },
  { art: "stay-vigilant-firing", title: "Stay vigilant",
    text: "The frontier holds danger as well as opportunity. Pirates prey on unguarded ships, and privateers from other charters may come for your hard-won resources. Stay vigilant, protect your trade routes, and be ready to defend your new home.",
    alt: "Pirate and privateer ships fire a few shots at a civilian freighter among asteroids above a frontier world." },
] as const;
/** §founding: after the story, the landing card for the world the server
 * dealt (no choice: the three home worlds are close in value). */
const LANDING_STEP = FOUNDING_STORY.length;

export const FOUNDING_PROFILES: { id: FoundingProfile; name: string; terrain: HabitableSurface; opportunity: string; detail: string }[] = [
  { id: "mineral_highlands", name: "Mineral Highlands", terrain: FOUNDING_SURFACES.mineral_highlands, opportunity: "A second mineral to export",
    detail: "Prospecting reveals a modest Cuprite seam: 3,000 units, worked by your Mining Complex alongside Ferrite." },
  { id: "verdant_lowlands", name: "Verdant Lowlands", terrain: FOUNDING_SURFACES.verdant_lowlands, opportunity: "More room to grow your food economy",
    detail: "Prospecting finds fertile ground, increasing this world's Biomass deposit output by 25%. Processing still needs workforce." },
  { id: "coastal_plains", name: "Broad Coastal Plains", terrain: FOUNDING_SURFACES.coastal_plains, opportunity: "An extra place for infrastructure",
    detail: "Prospecting identifies stable building land, opening one additional infrastructure slot on your home world." },
];

export const SETTLEMENT_PROJECTS: { id: SettlementProject; key: string; title: string; detail: string; costs: [Commodity, number][]; seconds: number; art: string }[] = [
  { id: "prospect", key: "camp_prospect", title: "Prospect the home world", detail: "Reveal your chosen world's local opportunity and locate accessible Ferrite workings.", costs: [], seconds: 60, art: "mining_complex" },
  { id: "surface_workings", key: "camp_workings", title: "Open surface workings", detail: "Increase the home Ferrite deposit's output by 50%. Requires a completed survey and Mining Complex; does not unlock Tier II.", costs: [["alloys", 8], ["machinery", 2]], seconds: 120, art: "mining_complex" },
  { id: "storehouse", key: "camp_storehouse", title: "Expand the storehouse", detail: "Enlarge the colony ship's holds from 1,000 to 1,300 goods. More export buffer between visits; ship loading speed is unchanged.", costs: [["alloys", 7], ["machinery", 1]], seconds: 90, art: "warehouse" },
];

const esc = (s: string) => s.replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
const art = (terrain: HabitableSurface) => planetSurfaceArt(terrain).full;
const storyImage = (slide: typeof FOUNDING_STORY[number]) => {
  const base = `/art/derived/founding-intro/v1/${slide.art}`;
  return `<img class="settlement-story-image" src="${base}-1280.webp" srcset="${[768, 1280, 1672].map(w => `${base}-${w}.webp ${w}w`).join(", ")}" sizes="(max-width: 1082px) calc(100vw - 32px), 1050px" width="1672" height="941" alt="${esc(slide.alt)}" decoding="async">`;
};

export function projectStatus(body: BodyView, system: SystemStateView, project: typeof SETTLEMENT_PROJECTS[number]): string {
  const camp = body.founding_camp;
  if (!camp) return "Unavailable";
  if (camp.completed.includes(project.id)) return "Complete";
  const job = system.builds.find(j => j.body_id === body.id && j.key === project.key);
  if (job) return job.queued ? "Queued" : "Under construction";
  if (project.id === "surface_workings" && (!camp.completed.includes("prospect") || !body.structures.mining_complex)) return "Needs survey and mine";
  if (project.costs.some(([c, n]) => {
    const reserved = (system.industry?.reservations ?? []).reduce((sum, r) => sum + (r.goods[c] ?? 0), 0);
    return (system.stockpile?.find(s => s.commodity === c)?.units ?? 0) - reserved < n;
  })) return "Needs materials";
  return "Ready";
}

/** One native modal for either shell: no corporation or timer is created by
 * advancing slides, skipping the story, or reconnecting. */
export class SettlementUI {
  private dialog = document.createElement("dialog");
  private step = 0;
  /** The home world dealt with the founding offer. */
  private profile: FoundingProfile | null = null;
  private founding = false;
  private submitting = false;
  private identity = "";
  private site: { system: string; body: number } | null = null;
  private review: SettlementProject | null = null;
  private notice = "";
  private signature = "";
  /** §settle-near-friends (R7): the invite this landing redeems, once answered. */
  private invite: Extract<ServerMsg, { type: "InviteStatus" }> | null = null;

  constructor(private state: ViewState, private send: (msg: ClientMsg) => void) {
    // §settle-near-friends (R7): keep an `?invite=` link's code through sign-in.
    storedInvite();
    this.dialog.className = "settlement-dialog";
    this.dialog.setAttribute("aria-labelledby", "settlement-title");
    document.body.append(this.dialog);
    this.dialog.addEventListener("cancel", event => {
      if (this.founding) {
        event.preventDefault();
        if (!this.submitting) { this.step = LANDING_STEP; this.dialog.scrollTop = 0; this.render(); this.focusTitle(); }
      }
    });
    this.dialog.addEventListener("click", event => {
      const button = (event.target as HTMLElement).closest<HTMLButtonElement>("button[data-settlement-act]");
      if (!button || button.disabled) return;
      const act = button.dataset.settlementAct;
      if (this.founding && this.submitting) return;
      if (act === "close") { this.dialog.close(); this.site = null; return; }
      if (act === "skip") this.step = LANDING_STEP;
      if (act === "next") this.step = Math.min(LANDING_STEP, this.step + 1);
      if (act === "back") this.step = Math.max(0, this.step - 1);
      if (act === "land" && this.founding && this.step === LANDING_STEP && !this.submitting && this.state.link === "online") {
        this.submitting = true;
        this.notice = "Landing order sent. Waiting for your colony report…";
        this.send({ type: "FoundColony" });
      }
      if (act === "review") this.review = button.dataset.project as SettlementProject;
      if (act === "cancel") this.review = null;
      if (act === "confirm" && this.review && this.site && this.state.link === "online") {
        const current = this.currentSite();
        const project = SETTLEMENT_PROJECTS.find(p => p.id === this.review);
        if (current && project && projectStatus(current.body, current.system, project) === "Ready") {
          this.send({ type: "DevelopSettlement", system_id: this.site.system, body_id: this.site.body, project: this.review });
          this.notice = "Order sent. Materials and progress update when the local report arrives.";
        }
        this.review = null;
      }
      if (["skip", "next", "back"].includes(act ?? "")) this.dialog.scrollTop = 0;
      this.render();
      if (["skip", "next", "back"].includes(act ?? "")) this.focusTitle();
      if (act === "review") [...this.dialog.querySelectorAll<HTMLButtonElement>("button")]
        .find(b => b.dataset.settlementAct === "confirm")?.focus();
    });
    document.addEventListener("click", event => {
      const button = (event.target as HTMLElement).closest<HTMLElement>("[data-settlement-open]");
      if (!button || this.founding) return;
      this.site = { system: button.dataset.system!, body: Number(button.dataset.body) };
      this.review = null; this.notice = "";
      this.render();
      if (this.currentSite()) this.dialog.showModal();
    });
  }

  onMessage(msg: ServerMsg): void {
    if (msg.type === "Welcome") {
      const identity = `${msg.galaxy.instance_id}:${msg.player_id}`;
      if (identity !== this.identity) { this.step = 0; this.profile = null; }
      if (identity !== this.identity) { this.founding = false; this.dialog.close(); }
      this.identity = identity; this.site = null; this.submitting = false; this.notice = "";
    }
    if (msg.type === "FoundingOffer") {
      this.founding = true; this.submitting = false; this.profile = msg.profile;
      const code = storedInvite();
      if (code) this.send({ type: "RedeemInvite", code });
      this.render(); if (!this.dialog.open) this.dialog.showModal();
    }
    if (msg.type === "InviteStatus") {
      this.invite = msg;
      if (!msg.ok) clearStoredInvite();
    }
    if (msg.type === "Error" && this.dialog.open) { this.notice = msg.message; this.submitting = false; }
    if (msg.type === "View" && this.founding && msg.systems.some(s => s.owner === this.state.playerId)) {
      this.founding = false; this.submitting = false; this.dialog.close();
      clearStoredInvite(); this.invite = null;
    }
    if (this.dialog.open) this.render();
  }

  /** The prologue, the landing card or a camp project is on screen
   * (§charter-advisor: the advisor introduces themselves after it). */
  get open(): boolean {
    return this.dialog.open;
  }

  connectionChanged(): void {
    if (this.state.link !== "online") this.submitting = false;
    if (this.dialog.open) this.render();
  }

  endSession(): void {
    this.founding = false; this.site = null; this.review = null;
    this.submitting = false; this.dialog.close();
  }

  private currentSite(): { system: SystemStateView; body: BodyView } | null {
    const system = this.state.systems.find(s => s.id === this.site?.system && s.owner === this.state.playerId);
    const body = system?.bodies.find(b => b.id === this.site?.body && b.founding_camp);
    return system && body ? { system, body } : null;
  }

  private focusTitle(): void {
    this.dialog.querySelector<HTMLElement>("#settlement-title")?.focus({ preventScroll: true });
  }

  private render(): void {
    const profile = FOUNDING_PROFILES.find(p => p.id === this.profile) ?? FOUNDING_PROFILES[0];
    const offline = this.state.link !== "online";
    let html = "";
    if (this.founding) {
      const status = offline ? "Connection lost. You can keep reading; reconnect before confirming settlement."
        : esc(this.notice);
      const statusLine = status ? `<p${this.step < LANDING_STEP ? ' class="settlement-muted settlement-story-status"' : ""} role="status">${status}</p>` : "";
      if (this.step < LANDING_STEP) {
        const slide = FOUNDING_STORY[this.step];
        const caption = slide.text.replace("{corporation}", () => this.state.name?.trim() || "Your corporation");
        html = `<div class="settlement-story"><header class="settlement-story-header"><div><small>STELLAR SYNDICATES</small><span>Prologue · ${this.step + 1} / ${FOUNDING_STORY.length}</span></div><button type="button" data-settlement-act="skip">Skip story</button></header><figure class="settlement-story-frame">${storyImage(slide)}</figure><div class="settlement-content settlement-story-caption"><h1 id="settlement-title" tabindex="-1">${esc(slide.title)}</h1><p>${esc(caption)}</p>${statusLine}<footer><button type="button" data-settlement-act="back" ${this.step === 0 ? "disabled" : ""}>Back</button><button type="button" class="settlement-primary" data-settlement-act="next">${this.step === LANDING_STEP - 1 ? "Your world" : "Continue"}</button></footer></div></div>`;
      } else {
        const content = `<p>Your colony ship lands and becomes the Command Center: holds for 1,000 goods, and in them the engines of your first freighter. 7,000 settlers live aboard until you build a Habitat.</p><div class="settlement-kit">${structureImage("command_center", 1, "lg")}</div><p>Your charter: mine Ferrite, smelt Alloys, ship them to the Market Hub. House your settlers first.</p>`;
        const invite = this.invite;
        const inviteLine = !invite ? "" : invite.ok
          ? `<p class="settlement-invite">Invited by <b>${esc(invite.from ?? "a friend")}</b>${invite.near ? ` · home near ${esc(invite.near)}` : ""}${invite.syndicate ? ` · joins ${esc(invite.syndicate)}` : ""}</p>`
          : `<p class="settlement-invite is-refused">${esc(invite.message ?? "Invite refused.")}</p>`;
        html = `<div class="settlement-hero" style="--settlement-art:url('${art(profile.terrain)}')"><small>STELLAR SYNDICATES · FOUNDING CHARTER</small><h1 id="settlement-title" tabindex="-1">Settle the ${profile.name}</h1><span>${profile.opportunity}</span></div><div class="settlement-content">${inviteLine}${content}${statusLine}<footer><button type="button" data-settlement-act="back" ${this.submitting ? "disabled" : ""}>Back</button><button type="button" class="settlement-primary" data-settlement-act="land" ${this.submitting || offline ? "disabled" : ""}>${this.submitting ? "Establishing colony…" : "Confirm settlement"}</button></footer></div>`;
      }
    } else {
      const current = this.currentSite();
      if (!current) { this.dialog.close(); return; }
      const { system, body } = current;
      const chosen = FOUNDING_PROFILES.find(p => p.id === body.founding_camp!.profile)!;
      const delay = Math.ceil(Math.hypot((this.state.galaxy?.systems.find(s => s.id === system.id)?.pos.x ?? 0) - (this.state.commandCenter?.x ?? 0),
        (this.state.galaxy?.systems.find(s => s.id === system.id)?.pos.y ?? 0) - (this.state.commandCenter?.y ?? 0)) / ((this.state.galaxy?.c ?? 1) * WARP_FACTOR));
      html = `<div class="settlement-content"><header><small>${chosen.name} · Founding Camp</small><button type="button" data-settlement-act="close" aria-label="Close settlement projects">×</button><h1 id="settlement-title">Develop ${esc(body.name)}</h1></header><p>${chosen.detail}</p><p>Housing: ${Math.round(body.population * 1e6).toLocaleString()} / ${Math.round((body.population_capacity ?? 0) * 1e6).toLocaleString()} · System storage: ${system.storage_used} / ${system.storage_cap}</p><div class="settlement-projects">${SETTLEMENT_PROJECTS.map(p => {
        const status = projectStatus(body, system, p);
        return `<article>${structureImage(p.art, 1, "md")}<div><h2>${p.title}</h2><p>${p.detail}</p><small>${p.costs.length ? p.costs.map(([c, n]) => `${n} ${label(c)}`).join(" · ") : "No goods required"} · ${p.seconds}s base</small><button type="button" data-settlement-act="review" data-project="${p.id}" ${status !== "Ready" || offline ? "disabled" : ""}>${status === "Ready" ? "Review project" : status}</button></div></article>`;
      }).join("")}</div>${this.review ? `<section class="settlement-review" role="group" aria-label="Confirm project"><b>${SETTLEMENT_PROJECTS.find(p => p.id === this.review)?.title}</b><p>Order travel: ${delay}s. Uses local unreserved stock; construction takes its turn in this planet's queue. Progress appears after report light returns.</p><button type="button" class="settlement-primary" data-settlement-act="confirm" ${offline ? "disabled" : ""}>Confirm project</button><button type="button" data-settlement-act="cancel">Cancel</button></section>` : ""}<p role="status">${offline ? "Reconnecting…" : esc(this.notice)}</p><p class="settlement-muted">These are optional investments, not new export requirements. Keep materials for the Service Berth and your preferred next step.</p></div>`;
    }
    if (html === this.signature) return;
    const focused = this.dialog.contains(document.activeElement) ? (document.activeElement as HTMLElement).dataset : null;
    const scroll = this.dialog.scrollTop;
    this.dialog.innerHTML = html; this.signature = html;
    if (focused?.settlementAct) {
      const match = [...this.dialog.querySelectorAll<HTMLButtonElement>("button")].find(b => b.dataset.settlementAct === focused.settlementAct && b.dataset.project === focused.project && b.dataset.profile === focused.profile);
      const target = match ?? [...this.dialog.querySelectorAll<HTMLButtonElement>("button")]
        .find(b => ["land", "next", "confirm"].includes(b.dataset.settlementAct ?? "") && !b.disabled);
      target?.focus({ preventScroll: true });
    }
    this.dialog.scrollTop = scroll;
  }
}
