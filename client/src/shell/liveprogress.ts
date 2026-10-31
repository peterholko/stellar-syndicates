// §live-progress (Peter, Oct 27 2026): construction bars looked "laggy/jittery".
// Their panels redraw on a whole-second heartbeat (and again whenever the
// light-delayed market ticker changes, off that beat), and a bar only moved
// when its panel redrew. Each bar now carries its progress line
// (`liveProgressAttr`), and the shells' frame tick sets its fill from the
// render clock, so it glides at the build's true rate between redraws.

import { progressAt, readLiveProgress } from "../core/derive/construction";

/** Set every live bar's fill under `root` to its line's percent at `now`. */
export function tickLiveProgress(root: ParentNode, now: number): void {
  for (const bar of root.querySelectorAll<HTMLElement>("[data-live-progress]")) {
    const line = readLiveProgress(bar.dataset.liveProgress);
    const fill = bar.firstElementChild as HTMLElement | null;
    if (line && fill) fill.style.width = `${progressAt(line, now).toFixed(2)}%`;
  }
}
