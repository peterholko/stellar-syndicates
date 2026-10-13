// The galaxy map's two families of points of interest, told apart at a glance.
// Exploration CONTACTS are sensor circles in cyan: a signal glyph in a dashed
// ring until scanned, then the site's kind with a pip for its state. Operation
// CONTRACTS are tags coloured by their issuer: hollow when offered, filled once
// joined. Pure rules only: sizes by zoom, glyphs, colours, anchors and copy.
import type { ExplorationSiteView, OperationIssuer, OperationView, SystemInfo } from "../../protocol";
import type { MapGlyph } from "../../map-glyphs.generated";
import { siteStatus, siteTitle } from "./exploration";
import { fmt, fmtDur, operationTitle } from "./format";

export const CONTACT_COLOR = 0x8fd3ff;
const CONTACT_PALE = 0xdcefff;
const CONTACT_DEPLETED = 0x7d8a98;
/** Where a site's state shows: a small pip on its badge. */
const CONTACT_PIPS: Record<string, number> = {
  "Guarded · escorts recommended": 0xef6b5e,
  "New coordinates recovered": 0xf3c969,
  "Deeper opportunity": 0xb08cff,
  "Recovery available": 0x6fd08c,
  "Restored sensor outpost": 0x5fd3c0,
};

export const ISSUER_COLOR: Record<OperationIssuer, number> = {
  authority: 0xf3c969, market: 0x6fd08c, survey_office: 0x5fd3c0,
  salvage_office: 0xf0a35e, regional_council: 0x7fb2ff, syndicate: 0xa98cff,
};
export const ISSUER_LABEL: Record<OperationIssuer, string> = {
  authority: "Authority", market: "Market", survey_office: "Survey Office",
  salvage_office: "Salvage Office", regional_council: "Regional Council", syndicate: "Syndicate",
};
type ContractKind = OperationView["kind"]["kind"];
const CONTRACT_GLYPH: Record<ContractKind, MapGlyph> = {
  combat_objective: "contract-combat", privateer_patrol: "contract-patrol", pirate_bounty: "contract-bounty",
  survey_expedition: "contract-survey", market_delivery: "contract-delivery", rescue_salvage: "contract-salvage",
  prize_recovery: "contract-salvage", convoy_escort: "contract-escort", freight_escort: "contract-escort",
  authority_enforcement: "contract-enforcement", strategic_control: "contract-control",
  regional_mandate: "contract-mandate", syndicate_megaproject: "contract-megaproject",
};
const CONTRACT_LABEL: Record<ContractKind, string> = {
  combat_objective: "Combat", privateer_patrol: "Patrol", pirate_bounty: "Bounty", survey_expedition: "Survey",
  market_delivery: "Delivery", rescue_salvage: "Salvage", prize_recovery: "Prize", convoy_escort: "Escort",
  freight_escort: "Escort", authority_enforcement: "Enforcement", strategic_control: "Control",
  regional_mandate: "Mandate", syndicate_megaproject: "Megaproject",
};

/** Names appear beside identified contacts from this zoom (always when selected or pinned). */
export const CONTACT_LABEL_ZOOM = 12;
/** Contract tags spell out their kind from this zoom (a joined contract always does). */
export const CONTRACT_LABEL_ZOOM = 6;
/** A contract this close to a star belongs to it, and stands beside it. */
export const CONTRACT_AT_STAR_SU = 300;

const clamp01 = (value: number) => Math.min(1, Math.max(0, value));
/** Smoothstep in log-zoom space (the wheel is multiplicative) between two zooms. */
const zoomRamp = (zoom: number, from: number, to: number) => {
  const t = clamp01(Math.log(Math.max(zoom, 1e-6) / from) / Math.log(to / from));
  return t * t * (3 - 2 * t);
};
/** A contact's badge diameter (px): compact on the overview, legible up close. */
export function contactBadgePx(zoom: number): number { return 18 + 10 * zoomRamp(zoom, 1, 24); }
/** A contract tag's height (px). */
export function contractTagPx(zoom: number): number { return 18 + 6 * zoomRamp(zoom, 1, 24); }

export interface ContactStyle {
  glyph: MapGlyph;
  unknown: boolean;
  ring: number;
  glyphTint: number;
  pip: number | null;
  alpha: number;
  status: string;
}

export function contactStyle(site: ExplorationSiteView): ContactStyle {
  const status = siteStatus(site);
  if (!site.details) return { glyph: "contact-unknown", unknown: true, ring: CONTACT_COLOR, glyphTint: CONTACT_COLOR, pip: null, alpha: .9, status };
  const depleted = status === "Depleted";
  return { glyph: `site-${site.details.kind}` as MapGlyph, unknown: false,
    ring: depleted ? CONTACT_DEPLETED : CONTACT_COLOR, glyphTint: depleted ? CONTACT_DEPLETED : CONTACT_PALE,
    pip: CONTACT_PIPS[status] ?? null, alpha: depleted ? .45 : 1, status };
}

export interface ContractStyle {
  glyph: MapGlyph;
  color: number;
  filled: boolean;
  label: string;
  /** Joined, running contracts show their progress (0–1). */
  progress: number | null;
}

export function contractStyle(op: OperationView): ContractStyle {
  return {
    glyph: CONTRACT_GLYPH[op.kind.kind] ?? "contract-combat",
    color: ISSUER_COLOR[op.issuer] ?? ISSUER_COLOR.authority,
    filled: op.joined,
    label: CONTRACT_LABEL[op.kind.kind] ?? "Contract",
    progress: op.state === "active" && op.joined && op.goal > 0 ? clamp01(op.progress / op.goal) : null,
  };
}

/** Only offered and running contracts stand on the map. */
export function contractOnMap(op: OperationView): boolean {
  return op.state === "offered" || op.state === "active";
}

/** The star a contract belongs to, when it targets one: its tag stands beside
 * the star instead of under it. */
export function contractStar(op: OperationView, systems: readonly SystemInfo[]): SystemInfo | null {
  let best: SystemInfo | null = null, bestDistance = CONTRACT_AT_STAR_SU;
  for (const system of systems) {
    const distance = Math.hypot(system.pos.x - op.target_pos.x, system.pos.y - op.target_pos.y);
    if (distance <= bestDistance) { best = system; bestDistance = distance; }
  }
  return best;
}

/** One line for the map's hover tip. */
export function contactBrief(site: ExplorationSiteView, sending?: string): string {
  return [siteTitle(site), siteStatus(site), sending ? `click to send ${sending}` : "click for details"].join(" · ");
}

export function contractBrief(op: OperationView, now: number): string {
  const parts = [operationTitle(op), ISSUER_LABEL[op.issuer] ?? "Contract"];
  if (op.reward.credits > 0) parts.push(`${fmt(op.reward.credits)} cr`);
  if (op.state === "active" && op.goal > 0) parts.push(`${Math.round(clamp01(op.progress / op.goal) * 100)}% done`);
  if (op.expires_at > now) parts.push(`${fmtDur(op.expires_at - now)} left`);
  parts.push(op.joined ? "click to open your contract" : "click to review the offer");
  return parts.join(" · ");
}

/** Screen-space hit testing shared by the layers' picks. */
export interface MarkerHit { id: string; x: number; y: number; w: number; h: number }
export function hitMarker(hits: readonly MarkerHit[], sx: number, sy: number, slop = 4): string | null {
  let best: string | null = null, bestDistance = Infinity;
  for (const hit of hits) {
    if (sx < hit.x - slop || sx > hit.x + hit.w + slop || sy < hit.y - slop || sy > hit.y + hit.h + slop) continue;
    const distance = Math.hypot(sx - (hit.x + hit.w / 2), sy - (hit.y + hit.h / 2));
    if (distance < bestDistance) { best = hit.id; bestDistance = distance; }
  }
  return best;
}
