// Mechanical review layout only: generated art is never repainted or recolored.
import { readFile, access } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import sharp from "../../node_modules/sharp/lib/index.js";

const root = path.dirname(fileURLToPath(import.meta.url));
const { families: newFamilies } = JSON.parse(await readFile(path.join(root, "catalog.json"), "utf8"));
const all = [{ id: "mining-complex", title: "Mining Complex" }, ...newFamilies];
const requested = process.argv.slice(2);
const families = requested.length ? all.filter(f => requested.includes(f.id)) : all;
const exists = async p => { try { await access(p); return true; } catch { return false; } };
const escape = s => s.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll('"', "&quot;");
const width = 1280, left = 200, col = 180, row = 194, header = 64;
const numerals = ["I", "II", "III", "IV", "V", "VI"];
for (let first = 0; first < families.length; first += 4) {
  const group = families.slice(first, first + 4);
  const height = header + group.length * row;
  const layers = [];
  let labels = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}"><style>text{font-family:Arial,sans-serif;fill:#d9e8ee}</style>`;
  labels += '<text x="18" y="35" font-size="18">STRUCTURE TIERS</text>';
  numerals.forEach((n, i) => { labels += `<text x="${left+i*col+col/2}" y="35" font-size="20" text-anchor="middle">${n}</text>`; });
  for (let r = 0; r < group.length; r++) {
    const f = group[r];
    const y = header + r * row;
    labels += `<path d="M16 ${y}H1264" stroke="#27333d"/>`;
    const words = f.title.split(" ");
    if (f.title.length < 18) labels += `<text x="18" y="${y+90}" font-size="17">${escape(f.title)}</text>`;
    else {
      labels += `<text x="18" y="${y+80}" font-size="17">${escape(words.slice(0,-1).join(" "))}</text>`;
      labels += `<text x="18" y="${y+103}" font-size="17">${escape(words.at(-1))}</text>`;
    }
    for (let tier = 1; tier <= 6; tier++) {
      const final = path.join(root, f.id, `tier-${tier}.png`);
      const draft = path.join(root, f.id, `tier-${tier}-draft.png`);
      const file = await exists(final) ? final : await exists(draft) ? draft : null;
      const x = left + (tier-1)*col;
      if (file) {
        const input = await sharp(file).resize(160,160,{fit:"contain",background:{r:0,g:0,b:0,alpha:0},kernel:"mitchell"}).png().toBuffer();
        layers.push({input,left:x+10,top:y+12});
        const meta = await sharp(file).metadata();
        if (!meta.hasAlpha) labels += `<text x="${x+90}" y="${y+185}" font-size="11" text-anchor="middle" fill="#e5b779">CUTOUT PENDING</text>`;
      } else labels += `<text x="${x+90}" y="${y+94}" font-size="12" text-anchor="middle">PENDING</text>`;
    }
  }
  labels += "</svg>";
  layers.push({input:Buffer.from(labels),left:0,top:0});
  const filename = `sheet-${group.map(f=>f.id).join("-")}.png`;
  await sharp({create:{width,height,channels:4,background:"#101820"}}).composite(layers).png().toFile(path.join(root,filename));
  console.log(path.join(root,filename));
}
