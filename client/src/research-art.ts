// §research-panel (Nov 1 2026): research emblem markup. A programme shows its
// own emblem once the approved master has been derived; until then, or if the
// request fails, its field's emblem; failing that, the field's UI icon. The
// generated manifest lists only delivered files, so a missing emblem is never
// requested. A failed request is remembered, so a re-render cannot retry it.

import "./styles/research.css";
import { icon, type IconKey } from "./icons";
import { RESEARCH_FIELD_ART, RESEARCH_PROGRAMME_ART } from "./research-art.generated";

const failed = new Set<string>();
let listening = false;

export function fieldIconKey(field: string): IconKey {
  if (field === "propulsion") return "jump";
  if (field === "materials") return "moduleWhippleArmor";
  if (field === "computation") return "intel";
  if (field === "weapons") return "attack";
  if (field === "hulls") return "fleet";
  return "population";
}

export const hasProgrammeEmblem = (id: string): boolean => !!RESEARCH_PROGRAMME_ART[id] && !failed.has(`programme:${id}`);

/** The art a programme (or, with no id, its field) resolves to right now. */
export function emblemSource(field: string, id: string | null): { key: string; levels: Readonly<Record<string, string>> } | null {
  if (id && hasProgrammeEmblem(id)) return { key: `programme:${id}`, levels: RESEARCH_PROGRAMME_ART[id] };
  const fieldArt = RESEARCH_FIELD_ART[field];
  if (fieldArt && !failed.has(`field:${field}`)) return { key: `field:${field}`, levels: fieldArt };
  return null;
}

/** An emblem `px` CSS pixels square. Art beside a name is decorative (empty
 * alt). `eager` is for the one selected emblem; cards load lazily. */
export function researchEmblem(field: string, id: string | null, px: number, opts: { eager?: boolean; cls?: string } = {}): string {
  listen();
  const cls = opts.cls ? ` ${opts.cls}` : "";
  const source = emblemSource(field, id);
  if (!source) {
    return `<span class="research-emblem research-emblem--icon${cls}" style="--emblem:${px}px" data-research-field="${esc(field)}" aria-hidden="true">${icon(fieldIconKey(field), "lg")}</span>`;
  }
  const sizes = Object.keys(source.levels).map(Number).sort((a, b) => a - b);
  const src = source.levels[sizes.find(size => size >= px) ?? sizes[sizes.length - 1]];
  const srcset = sizes.map(size => `${source.levels[size]} ${size}w`).join(", ");
  return `<img class="research-emblem${cls}" src="${src}" srcset="${srcset}" sizes="${px}px" width="${px}" height="${px}" alt="" decoding="async" loading="${opts.eager ? "eager" : "lazy"}"${opts.eager ? ` fetchpriority="high"` : ""} data-research-art="${esc(source.key)}" data-research-field="${esc(field)}" data-research-px="${px}">`;
}

/** Record a failed emblem request; later markup skips it. Exported for tests. */
export function markEmblemFailed(key: string): void {
  failed.add(key);
}

// One capture-phase listener for every emblem: on a failed request, remember
// it and swap this element to the next fallback in place. The next render
// produces the same fallback, so nothing is ever requested twice.
function listen(): void {
  if (listening || typeof document === "undefined" || typeof document.addEventListener !== "function") return;
  listening = true;
  document.addEventListener("error", (event) => {
    const img = event.target;
    if (!(img instanceof HTMLImageElement) || !img.classList.contains("research-emblem")) return;
    const key = img.dataset.researchArt;
    const field = img.dataset.researchField ?? "";
    if (!key || failed.has(key)) return;
    markEmblemFailed(key);
    const px = Number(img.dataset.researchPx) || img.width || 64;
    const cls = [...img.classList].filter(name => name !== "research-emblem").join(" ");
    const programme = key.startsWith("programme:") ? key.slice("programme:".length) : null;
    img.outerHTML = researchEmblem(field, programme, px, { eager: img.loading === "eager", cls });
  }, true);
}

const esc = (value: string): string => value.replace(/[&<>"']/g, (char) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[char]!);
