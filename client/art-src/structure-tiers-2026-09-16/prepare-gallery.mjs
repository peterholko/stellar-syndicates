// Art preview derivatives only; never touches live game assets or their registry.
import { readFile, writeFile, access, copyFile } from "node:fs/promises";
import { constants } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import sharp from "../../node_modules/sharp/lib/index.js";

const root = path.dirname(fileURLToPath(import.meta.url));
const catalog = JSON.parse(await readFile(path.join(root, "catalog.json"), "utf8"));
const families = [{ id: "mining-complex", title: "Mining Complex", group: "Extraction" }, ...catalog.families];
const exists = async (p) => { try { await access(p); return true; } catch { return false; } };
const report = [];
let gallery = "# Structure tier art — review\n\nGenerated with the built-in image tool; all 150 approved transparent masters are integrated into the client. Each row progresses I–VI. These source previews fit the current 150px maximum planet footprint; shipping art uses padded 128/256/512px WebP derivatives. Click a sprite for its full-resolution master.\n\nDesign specifications: [catalog](catalog.json), [shared style prompt](common.prompt.txt). Each structure folder also contains its six individual tier prompts. The planet scene reads the arrived tier; builders preview the proposed target tier.\n\n";
gallery += "## Comparison sheets\n\n";
for (let first = 0; first < families.length; first += 4) {
  const group = families.slice(first, first + 4);
  gallery += `- [${group.map(f => f.title).join(" · ")}](sheet-${group.map(f => f.id).join("-")}.png)\n`;
}
gallery += "\n";
for (const family of families) {
  const entries = [];
  for (let tier = 1; tier <= 6; tier++) {
    const file = path.join(root, family.id, `tier-${tier}.png`);
    const draft = path.join(root, family.id, `tier-${tier}-draft.png`);
    // A native generation that already has real alpha needs no edit.
    if (!await exists(file) && await exists(draft)) {
      const meta = await sharp(draft).metadata();
      const stats = await sharp(draft).stats();
      if (meta.hasAlpha && stats.channels[3]?.min === 0) await copyFile(draft, file, constants.COPYFILE_EXCL);
    }
    if (!await exists(file)) {
      entries.push({ tier, status: await exists(draft) ? "needs-alpha" : "not-generated" });
      continue;
    }
    const meta = await sharp(file).metadata();
    const stats = await sharp(file).stats();
    const transparent = Boolean(meta.hasAlpha && stats.channels[3]?.min === 0);
    if (!transparent) {
      entries.push({ tier, status: "needs-alpha" });
      continue;
    }
    for (const pixels of [96, 150, 256, 512]) {
      await sharp(file).resize(pixels, pixels, { fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 }, kernel: "mitchell", withoutEnlargement: true })
        .png().toFile(path.join(root, family.id, `tier-${tier}-${pixels}.png`));
    }
    entries.push({ tier, status: "ready", width: meta.width, height: meta.height });
  }
  report.push({ id: family.id, title: family.title, tiers: entries });
  gallery += `## ${family.title}\n\n| I | II | III | IV | V | VI |\n|---|---|---|---|---|---|\n|`;
  gallery += entries.map(e => e.status === "ready"
    ? `[![${family.title} ${e.tier}](${family.id}/tier-${e.tier}-150.png)](${family.id}/tier-${e.tier}.png)`
    : e.status === "needs-alpha" ? "Cutout pending" : "Generation pending").join("|") + "|\n\n";
}
await writeFile(path.join(root, "review.md"), gallery);
await writeFile(path.join(root, "status.json"), JSON.stringify(report, null, 2) + "\n");
const counts = {};
for (const f of report) for (const t of f.tiers) counts[t.status] = (counts[t.status] || 0) + 1;
console.log(counts);
for (const f of report) {
  const ready = f.tiers.filter(t => t.status === "ready").length;
  const generated = f.tiers.filter(t => t.status !== "not-generated").length;
  if (generated) console.log(`${f.title}: ${ready}/6 ready; ${generated}/6 generated`);
}
