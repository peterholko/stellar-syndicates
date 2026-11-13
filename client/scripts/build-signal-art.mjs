import { mkdir, stat } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import sharp from "sharp";

const clientRoot = fileURLToPath(new URL("..", import.meta.url));

/** §signals: one painting per signal type (art-src/signals-v1/README.md), in
 * the Signals pop-out (client/src/shell/deck/signals.ts). A master not yet
 * delivered is skipped; its card shows the type's line icon. */
export const SIGNAL_ART = [
  "drifting-wreck", "cargo-pod", "ore-fragment", "silent-probe", "distress-beacon", "sensor-ghost",
  "authority-courier", "abandoned-outpost", "pirate-stash", "ghost-ship", "precursor-fragment",
  "exotic-flare", "teos-lost-crate",
];
/** The detail and report show the full frame. */
export const SIGNAL_FRAME_WIDTHS = [768, 1280];
/** Cards are 150 px tall: the full frame leaves the smaller subjects (the
 * probe, the outpost, the shard) too small to read, so cards take the same
 * centred 16:9 window from every master, as a fraction of its width. */
export const SIGNAL_CARD_CROP = 0.62;
export const SIGNAL_CARD_WIDTHS = [384, 640];
/** §signal-report: the aftermath paintings (art-src/signals-aftermath-v1),
 * the report panel's full frame; no card crops. A master not yet delivered
 * is skipped; the panel shows the card painting meanwhile. */
export const SIGNAL_AFTERMATH_DIR = "signals-aftermath-v1";

export async function buildSignalArt() {
  const outputRoot = path.join(clientRoot, "public/art/derived/signals/v1");
  await mkdir(outputRoot, { recursive: true });
  const script = (await stat(fileURLToPath(import.meta.url))).mtimeMs;
  let sources = 0, written = 0;
  for (const slug of SIGNAL_ART) {
    const source = path.join(clientRoot, `art-src/signals-v1/${slug}.png`);
    let sourceTime;
    try { sourceTime = (await stat(source)).mtimeMs; } catch { continue; }
    sources++;
    const meta = await sharp(source).metadata();
    // About 16:9 at native size; one master is a pixel narrower.
    if (Math.abs(meta.width / meta.height - 16 / 9) > 0.01 || meta.width < 1280) {
      throw new Error(`Signal paintings are 16:9, at least 1280 px wide: ${source}`);
    }
    const fresh = async (output) => {
      try { return (await stat(output)).mtimeMs >= Math.max(sourceTime, script); } catch { return false; }
    };
    for (const width of SIGNAL_FRAME_WIDTHS) {
      const output = path.join(outputRoot, `${slug}-${width}.webp`);
      if (await fresh(output)) continue;
      await sharp(source).resize({ width, height: Math.round(width * 9 / 16), fit: "cover" })
        .webp({ quality: 86, effort: 5 }).toFile(output);
      written++;
    }
    const cropWidth = Math.round(meta.width * SIGNAL_CARD_CROP);
    const cropHeight = Math.round(cropWidth * 9 / 16);
    const crop = {
      left: Math.round((meta.width - cropWidth) / 2), top: Math.round((meta.height - cropHeight) / 2),
      width: cropWidth, height: cropHeight,
    };
    for (const width of SIGNAL_CARD_WIDTHS) {
      const output = path.join(outputRoot, `${slug}-card-${width}.webp`);
      if (await fresh(output)) continue;
      await sharp(source).extract(crop).resize({ width, height: Math.round(width * 9 / 16) })
        .webp({ quality: 86, effort: 5 }).toFile(output);
      written++;
    }
  }
  // §signal-report: the aftermath masters, full frames only.
  const aftermathRoot = path.join(clientRoot, "public/art/derived/signals/aftermath-v1");
  await mkdir(aftermathRoot, { recursive: true });
  let aftermath = 0;
  for (const slug of SIGNAL_ART) {
    const source = path.join(clientRoot, `art-src/${SIGNAL_AFTERMATH_DIR}/${slug}.png`);
    let sourceTime;
    try { sourceTime = (await stat(source)).mtimeMs; } catch { continue; }
    aftermath++;
    const meta = await sharp(source).metadata();
    if (Math.abs(meta.width / meta.height - 16 / 9) > 0.01 || meta.width < 1280) {
      throw new Error(`Signal aftermath paintings are 16:9, at least 1280 px wide: ${source}`);
    }
    for (const width of SIGNAL_FRAME_WIDTHS) {
      const output = path.join(aftermathRoot, `${slug}-${width}.webp`);
      try { if ((await stat(output)).mtimeMs >= Math.max(sourceTime, script)) continue; } catch { /* write it */ }
      await sharp(source).resize({ width, height: Math.round(width * 9 / 16), fit: "cover" })
        .webp({ quality: 86, effort: 5 }).toFile(output);
      written++;
    }
  }
  return { sources, pending: SIGNAL_ART.length - sources, aftermath, aftermathPending: SIGNAL_ART.length - aftermath, written };
}
