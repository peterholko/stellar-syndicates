function escapeAttribute(value: string): string {
  return value.replace(/[&<>"']/g, (character) => ({
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&#39;",
  })[character]!);
}

/** Captain art is authored at 512px but shown as a small UI portrait. Browsers
 * select a 96/192px AVIF or WebP derivative; PNG remains a compatibility
 * fallback. Roster callers opt into lazy loading now that keyed morphing keeps
 * the image node alive across live-view refreshes. */
export function captainPortrait(
  portrait: string,
  age: string,
  alt: string,
  className: string,
  lazy: boolean,
): string {
  const stem = encodeURIComponent(`${portrait}_${age}`);
  const derived = `/art/derived/captains/${stem}`;
  return `<picture class="captain-picture">` +
    `<source type="image/avif" srcset="${derived}-96.avif 1x, ${derived}-192.avif 2x">` +
    `<source type="image/webp" srcset="${derived}-96.webp 1x, ${derived}-192.webp 2x">` +
    `<img class="${escapeAttribute(className)}" src="/art/captains/${stem}.png" width="96" height="96" alt="${escapeAttribute(alt)}" decoding="async"${lazy ? ` loading="lazy"` : ""}>` +
    `</picture>`;
}
