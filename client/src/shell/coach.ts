import "../styles/coach.css";

/** §founding R5: point at the exact control a guide link leads to.
 *
 * The routed view renders asynchronously and re-renders often, so the mark is
 * a body-level ring re-aimed at `[data-coach="…"]` every frame rather than a
 * class on an element that may be replaced. It never intercepts input, and it
 * clears on first use of the control, Escape, a new coach, or a timeout. */

interface Active {
  target: string;
  ring: HTMLElement;
  bubble: HTMLElement;
  frame: number;
  until: number;
  scrolled: boolean;
  onClick: (event: Event) => void;
  onKey: (event: KeyboardEvent) => void;
}

let active: Active | null = null;

/** §privateer-contract: the caption on the charter contract's Accept. */
export const CONTRACT_COACH = "Accept the contract here.";

const LOOK_FOR_MS = 4_000;
const SHOW_MS = 14_000;

/** The visible `[data-coach=target]`, an enabled one first. */
export function coachTarget(target: string): HTMLElement | null {
  return find(target);
}

function find(target: string): HTMLElement | null {
  const all = [...document.querySelectorAll<HTMLElement>(`[data-coach="${target}"]`)];
  const visible = all.filter(el => {
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== "hidden";
  });
  return visible.find(el => !(el as HTMLButtonElement).disabled) ?? visible[0] ?? null;
}

export function clearCoach(): void {
  if (!active) return;
  cancelAnimationFrame(active.frame);
  active.ring.remove();
  active.bubble.remove();
  document.removeEventListener("click", active.onClick, true);
  document.removeEventListener("keydown", active.onKey, true);
  active = null;
}

/** Ring `[data-coach=target]` with a short caption once it appears. The
 * caption goes below the control unless `above` is asked for (or there is no
 * room), e.g. when text beneath the control must stay readable.
 * §charter-advisor: `face` puts the Charter Advisor's face beside it, and
 * `right` sets the caption beside the control (clear of the card it sits
 * in), or above it where there is no room. */
export function coach(target: string, caption: string, placement: "auto" | "above" | "right" = "auto",
  face?: { src: string; alt: string }): void {
  clearCoach();
  const ring = document.createElement("div");
  ring.className = "coach-ring";
  ring.hidden = true;
  ring.setAttribute("aria-hidden", "true");
  const bubble = document.createElement("div");
  bubble.className = "coach-bubble";
  bubble.setAttribute("role", "status");
  if (face) {
    const img = document.createElement("img");
    img.src = face.src;
    img.alt = face.alt;
    img.decoding = "async";
    const text = document.createElement("span");
    text.textContent = caption;
    bubble.classList.add("has-face");
    bubble.append(img, text);
  } else {
    bubble.textContent = caption;
  }
  bubble.hidden = true;
  document.body.append(ring, bubble);
  const started = performance.now();
  const onClick = (event: Event) => {
    if ((event.target as Element | null)?.closest?.(`[data-coach="${target}"]`)) clearCoach();
  };
  const onKey = (event: KeyboardEvent) => { if (event.key === "Escape") clearCoach(); };
  document.addEventListener("click", onClick, true);
  document.addEventListener("keydown", onKey, true);
  const state: Active = { target, ring, bubble, frame: 0, until: started + LOOK_FOR_MS, scrolled: false, onClick, onKey };
  active = state;
  const tick = () => {
    if (active !== state) return;
    const now = performance.now();
    const el = find(target);
    if (!el) {
      ring.hidden = bubble.hidden = true;
      if (now > state.until) { clearCoach(); return; }
    } else {
      if (!state.scrolled) {
        state.scrolled = true;
        state.until = now + SHOW_MS;
        el.scrollIntoView({ block: "nearest", inline: "nearest" });
      }
      if (now > state.until) { clearCoach(); return; }
      const r = el.getBoundingClientRect();
      ring.hidden = bubble.hidden = false;
      Object.assign(ring.style, { left: `${r.left - 4}px`, top: `${r.top - 4}px`, width: `${r.width + 8}px`, height: `${r.height + 8}px` });
      const roomAbove = r.top - bubble.offsetHeight - 10 >= 8;
      const beside = placement === "right" && r.right + 20 + bubble.offsetWidth <= window.innerWidth - 8;
      const below = !beside && (placement !== "auto" && roomAbove ? false : r.bottom + 44 < window.innerHeight);
      const left = beside ? r.right + 20 : Math.max(8, Math.min(window.innerWidth - bubble.offsetWidth - 8, r.left));
      const top = beside ? Math.max(8, Math.min(window.innerHeight - bubble.offsetHeight - 8, r.top + r.height / 2 - bubble.offsetHeight / 2))
        : below ? r.bottom + 10 : Math.max(8, r.top - bubble.offsetHeight - 10);
      Object.assign(bubble.style, { left: `${left}px`, top: `${top}px` });
    }
    state.frame = requestAnimationFrame(tick);
  };
  state.frame = requestAnimationFrame(tick);
}
