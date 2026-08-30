const byId = (id: string): HTMLElement => document.getElementById(id)!;

// --- §single-click: the PRESS GUARD ------------------------------------------
// Views stream every ~100ms (BROADCAST_EVERY 3 ticks @ 30Hz) and every open
// panel re-renders on each one. A re-render landing MID-PRESS — between
// pointerdown and pointerup, i.e. inside a normal ~100ms human click — destroys
// the pressed button; the browser then retargets the `click` event to the old
// and new targets' common ANCESTOR (the panel root), where the delegated
// `closest("[data-*]")` lookup finds nothing, so the action silently never
// fires. That was the "buttons need a double click" bug (and the unbuildable
// Scout). Structural fix, applied to EVERY per-View-rebuilt panel: while a
// press is down inside a panel, that panel's re-renders are DEFERRED — the
// pressed node survives to pointerup, the click lands normally — and the
// deferred render flushes right after the click dispatches (pointerup fires
// mouseup+click synchronously in the same task; setTimeout(0) runs after).
// Event delegation on the stable panel roots (already the codebase pattern)
// handles the "handler orphaned by innerHTML" half; this guard handles the
// "node destroyed mid-press" half. Presses elsewhere (map pans, other panels)
// defer nothing — each panel is guarded independently.
export const pressGuard = { target: null as EventTarget | null, deferred: new Map<string, () => void>() };

let pressGuardInstalled = false;

// Both shells rebuild live panels from the 10 Hz served View. Install the guard
// once at the shared DOM boundary so swapping shells never duplicates global
// listeners, and mobile receives the same single-tap guarantee as desktop.
export function installPressGuard(): void {
  if (pressGuardInstalled) return;
  pressGuardInstalled = true;
  window.addEventListener("pointerdown", (event) => { pressGuard.target = event.target; }, true);
  window.addEventListener("pointerup", () => setTimeout(flushPressGuard, 0), true);
  window.addEventListener("pointercancel", () => setTimeout(flushPressGuard, 0), true);
}

export function __init_mapchrome_142(): void {
  installPressGuard();
}

export function flushPressGuard(): void {
  pressGuard.target = null;
  const fns = [...pressGuard.deferred.values()];
  pressGuard.deferred.clear();
  for (const f of fns) f();
}

export function __init_mapchrome_149(): void {
  installPressGuard();
}

export function __init_mapchrome_150(): void {
  installPressGuard();
}

// The same hazard one step earlier was HOVER: a panel that rebuilds on every View
// recreated its buttons ~10×/s, and a brand-new node has no `:hover` until the
// browser's next hit-test, so the border on whatever the cursor rested on blinked.
// That used to be answered by a second guard that HELD the rebuild until the
// cursor left the control — which quietly froze the panel for as long as the
// player kept the mouse still. Assign a worker, don't move the mouse, and the
// worker count never changed: the command had landed a tick later, but the panel was
// waiting on a `pointermove` that never came. It read as mysterious lag.
//
// The narrow fix is in `setHtml`: rebuild the panel but PRESERVE the node under
// the cursor, by reconciling the DOM in place instead of replacing it. The hovered
// button is the same element before and after, so its `:hover` never lapses and
// there is nothing to defer. No hover guard remains — panels stay live under the
// pointer.

/// True → `rootId` must NOT rebuild its DOM right now, because a press is down
/// inside it: the pressed node must survive to pointerup or the click is lost.
/// The render is queued and re-runs the moment the press lifts.
export function renderDeferred(rootId: string, render: () => void): boolean {
  const t = pressGuard.target;
  if (t instanceof Node && byId(rootId).contains(t)) {
    pressGuard.deferred.set(rootId, render);
    return true;
  }
  return false;
}

/// Write `html` into `el` only when it actually differs from the last thing we
/// wrote there. An identical rewrite looks like a no-op but isn't: it destroys
/// and recreates every descendant, dropping `:hover`, focus, and scroll on
/// whatever the player was pointing at. The last string is cached per element
/// (rather than read back from `innerHTML`) so the check costs a comparison,
/// not a DOM serialization.
///
/// When the html HAS changed, the update is applied by RECONCILING the existing
/// DOM against it (`morphChildren`) rather than by `innerHTML =`. A wholesale
/// rewrite destroys every descendant, and a brand-new node has no `:hover` until
/// the browser's next hit-test — so the control under the cursor blinks, and (the
/// reason this matters) the panel could not be rebuilt AT ALL while the cursor
/// rested on one of its buttons without that blink. Reusing nodes keeps the
/// cursor's node identity, so a panel refreshes under the pointer with no flicker
/// and no deferral: change a worker assignment, watch the number change, mouse never moves.
export const lastHtmlWritten = new WeakMap<HTMLElement, string>();

export function setHtml(el: HTMLElement, html: string): void {
  if (lastHtmlWritten.get(el) === html) return;
  lastHtmlWritten.set(el, html);
  const parsed = document.createElement("template");
  parsed.innerHTML = html; // <template> parses ANY fragment (incl. bare <option>)
  morphChildren(el, parsed.content);
}

/// Attributes that give a node a STABLE IDENTITY across rebuilds, so it is reused
/// even when the list around it reorders (production lines re-sort as workers move).
/// Order matters only for determinism — the first present wins.
export const NODE_KEY_ATTRS = ["id", "data-deck-act", "data-deck-command", "data-crew", "data-build", "data-body", "data-action", "data-act", "data-mtab", "data-tab", "data-rid", "data-sy"];

export function nodeKey(e: Element): string | null {
  for (const a of NODE_KEY_ATTRS) {
    const v = e.getAttribute(a);
    if (v !== null) return `${a}=${v}`;
  }
  return null;
}

/// Reconcile `target`'s children against `source`'s, IN PLACE: reuse a node when
/// it matches (by key, else by position + tag), recurse into it, and only create
/// or drop what genuinely appeared or vanished. Live DOM state a rewrite would
/// destroy — `:hover`, focus, caret, scroll, a dirtied input's value, an <img>
/// already decoded — survives on every reused node.
export function morphChildren(target: Element, source: ParentNode): void {
  // Index the current children that carry a key, so a reorder MOVES nodes rather
  // than recreating them.
  //
  // Each key maps to a QUEUE, not a single node, because keys are NOT guaranteed
  // unique among siblings: `nodeKey` returns the first attribute a node carries
  // from NODE_KEY_ATTRS, and plenty of generated lists share one — every battle
  // scrubber tick is `data-act=round`, every speed button `data-act=speed`,
  // every split button `data-act=split`, and the per-item attribute that
  // actually distinguishes them (`data-round`, `data-speed`, `data-kind`) is not
  // a key attribute. With one node per key, every one of those source children
  // resolved to the SAME target node: they all morphed onto it, the cursor
  // finished immediately after it, and the trailing cleanup deleted the rest of
  // the list. The list rendered correctly on first paint (nothing to reuse) and
  // collapsed to a single item on the first re-render after — silently, since
  // nothing errors. Matching duplicates in document order degrades them to
  // positional reuse WITHIN the key group, which is exactly right, and leaves
  // genuinely-unique keys behaving as they always did.
  const keyed = new Map<string, Element[]>();
  for (let n = target.firstElementChild; n; n = n.nextElementSibling) {
    const k = nodeKey(n);
    if (!k) continue;
    const q = keyed.get(k);
    if (q) q.push(n);
    else keyed.set(k, [n]);
  }
  let cursor: ChildNode | null = target.firstChild;
  for (const src of [...source.childNodes]) {
    let match: ChildNode | null = null;
    if (src.nodeType === Node.ELEMENT_NODE) {
      const k = nodeKey(src as Element);
      if (k) {
        // Same key AND same tag — a key that changed element type is a different
        // thing wearing an old name, and reusing it would leave the cursor
        // pointing at a node we then replace (and delete the rest of the list).
        // Consume the first queued node of that tag, so each source child claims
        // a distinct target node and no two can land on the same one.
        const q = keyed.get(k);
        const at = q ? q.findIndex((e) => e.tagName === (src as Element).tagName) : -1;
        if (q && at >= 0) match = q.splice(at, 1)[0];
      } else if (
        cursor?.nodeType === Node.ELEMENT_NODE
        && (cursor as Element).tagName === (src as Element).tagName
        && !nodeKey(cursor as Element) // never steal a keyed node for an unkeyed slot
      ) {
        match = cursor;
      }
    } else if (cursor && cursor.nodeType === src.nodeType) {
      match = cursor; // text / comment in the same slot
    }
    if (!match) {
      target.insertBefore(src, cursor); // genuinely new — adopt it from the parse
      continue;
    }
    if (match !== cursor) target.insertBefore(match, cursor); // reordered
    if (src.nodeType === Node.ELEMENT_NODE) {
      morphElement(match as Element, src as Element);
    } else if (match.nodeValue !== src.nodeValue) {
      match.nodeValue = src.nodeValue;
    }
    cursor = match.nextSibling;
  }
  // Anything left past the cursor is gone from the new html.
  while (cursor) {
    const next: ChildNode | null = cursor.nextSibling;
    cursor.remove();
    cursor = next;
  }
}

/// Bring one reused element up to date: attributes first, then its children.
/// Only ever called with matching tag names (`morphChildren` guarantees it).
export function morphElement(target: Element, source: Element): void {
  for (const a of [...target.attributes]) {
    if (!source.hasAttribute(a.name)) target.removeAttribute(a.name);
  }
  for (const a of [...source.attributes]) {
    if (target.getAttribute(a.name) !== a.value) target.setAttribute(a.name, a.value);
  }
  // A <select>'s selection lives in the PROPERTY, not the attribute: reconciling
  // its <option>s can leave the property pointing at nothing, so restore it when
  // the previously-selected value still exists.
  const wanted = target instanceof HTMLSelectElement ? target.value : null;
  morphChildren(target, source);
  if (target instanceof HTMLSelectElement && wanted !== null && target.value !== wanted
    && [...target.options].some((o) => o.value === wanted)) {
    target.value = wanted;
  }
}
