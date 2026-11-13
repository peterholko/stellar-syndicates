// §research-sheets (Nov 3 2026): the 119 programme emblems still owed, as
// ChatGPT sheet prompts. One sheet = one field, up to six badges in the July
// field sheet's own 3 × 2 arrangement, on a transparent 1536 × 1024 canvas; the
// art build slices a delivered sheet by its clear gaps (build-research-art.mjs).
// Writes docs/research-emblem-sheets.md (the prompts Peter pastes) and
// art-src/research-emblems-v1/sheets/sheets.json (which keys each sheet holds,
// in reading order). The plan is frozen once a sheet has been delivered: the
// slicer trusts sheets.json, so a reshuffle would mislabel landed art.
import assert from "node:assert/strict";
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const client = fileURLToPath(new URL("..", import.meta.url));
const repo = path.resolve(client, "..");
const mastersDir = path.join(client, "art-src/research-emblems-v1");
const sheetsDir = path.join(mastersDir, "sheets");
const docPath = path.join(repo, "docs/research-emblem-sheets.md");
export const PER_SHEET = 6;
export const SHEET_SIZE = [1536, 1024];
export const CELL = 512;
export const BADGE_PX = 450; // 88% of a cell, the family's HEX_FILL

const FIELD_TITLE = { propulsion: "Propulsion", materials: "Materials", computation: "Computation", weapons: "Weapons", hulls: "Hulls", life: "Life" };
const ROMAN = ["", "I", "II", "III", "IV", "V", "VI", "VII", "VIII"];
const CELL_NAMES = {
  6: ["top left", "top centre", "top right", "bottom left", "bottom centre", "bottom right"],
  5: ["top left", "top centre", "top right", "bottom left", "bottom right"],
  4: ["top left", "top right", "bottom left", "bottom right"],
  3: ["left", "centre", "right"],
  2: ["left", "right"],
  1: ["centre"],
};
const LAYOUT = {
  6: "landscape 1536 x 1024 canvas, three columns and two rows, exactly Image 1's arrangement",
  5: "landscape 1536 x 1024 canvas, three badges across the top row and two centred beneath them",
  4: "square 1024 x 1024 canvas, two columns and two rows",
  3: "landscape 1536 x 1024 canvas, one row of three",
  2: "square 1024 x 1024 canvas, two badges side by side",
  1: "square 1024 x 1024 canvas, one badge centred",
};

const line = (prompt, label) => {
  const m = prompt.match(new RegExp(`^${label}: (.*)$`, "m"));
  assert.ok(m, `${label} line`);
  return m[1];
};

/** Group the programmes that still lack a single master into sheets: by field,
 * in catalogue order (tier first), six per sheet. */
export function planSheets(prompts, delivered) {
  const byField = new Map();
  for (const asset of prompts.assets) {
    if (delivered.has(asset.key)) continue;
    const field = asset.field.toLowerCase();
    if (!byField.has(field)) byField.set(field, []);
    byField.get(field).push(asset);
  }
  const sheets = [];
  for (const field of Object.keys(FIELD_TITLE)) {
    const assets = byField.get(field) ?? [];
    for (let i = 0; i < assets.length; i += PER_SHEET) {
      const slice = assets.slice(i, i + PER_SHEET);
      sheets.push({ name: `${field}-${String(i / PER_SHEET + 1).padStart(2, "0")}`, field, assets: slice });
    }
  }
  return sheets;
}

/** One sheet's ChatGPT message and the files to attach with it. */
export function sheetPrompt(sheet) {
  const n = sheet.assets.length;
  const names = CELL_NAMES[n], layout = LAYOUT[n];
  assert.ok(names && layout, `${sheet.name}: a layout for ${n} badges`);
  const colours = new Set(sheet.assets.map(a => line(a.prompt, "Colour")));
  assert.equal(colours.size, 1, `${sheet.name}: one colour line per sheet (${[...colours].join(" | ")})`);
  const colour = [...colours][0];
  const attach = [path.relative(repo, path.join(client, "art-src/research-topics-icons/research-topics-icons.png"))];
  const refs = [];
  const motifs = sheet.assets.map((asset, i) => {
    let motif = line(asset.prompt, "Motif");
    if (asset.reference_images.length > 1) {
      const image = attach.length + 1;
      attach.push(path.relative(repo, path.resolve(mastersDir, asset.reference_images[1])));
      const what = asset.prompt.match(/^Image 2 is (.*?)\. Use it ONLY/m);
      assert.ok(what, `${asset.key}: an Image 2 line`);
      motif = motif.replace(/Image 2/g, `Image ${image}`);
      refs.push(`Image ${image} is ${what[1]}: use it ONLY for that design's silhouette and details, drawn once, in an elevated three-quarter view, as motif ${i + 1}, in Image 1's emblem rendering. Ignore its own lighting and background.`);
    }
    return `${i + 1}. ${names[i][0].toUpperCase()}${names[i].slice(1)}: ${motif}`;
  });
  const count = ["zero", "one", "two", "three", "four", "five", "six"][n];
  const text = [
    `Use case: stylized-concept. Asset type: a sheet of ${count} transparent research emblems for Stellar Syndicates, a premium dark sci-fi strategy game. They join an existing emblem family: Image 1 is the approved sheet of six research field emblems, attached ONLY for the badge shape, rim, face, rendering, lighting and finish. Draw ${count.toUpperCase()} new emblems. Do not copy any of Image 1's six motifs, and ignore Image 1's grey backdrop.`,
    `Field: ${FIELD_TITLE[sheet.field]}. Colour, all ${count} badges: ${colour}`,
    `Layout: ${layout}. ${n === 1 ? "One" : `${count[0].toUpperCase()}${count.slice(1)} identical`} hexagon badge${n === 1 ? "" : "s"}, each about ${BADGE_PX} px tall, centred in ${n === 1 ? "the canvas" : `its own ${CELL} x ${CELL} cell`}, with clear empty space between badges and around the sheet. Badges never touch or overlap.`,
    `Badge (every one the same): a hexagon with one point at the top, one at the bottom and flat vertical sides; a deep navy-black face, slightly lighter toward its centre; a thin dark charcoal rim with a faint bevelled highlight along its upper-left edges. Each motif sits inside its rim, centred, about 70% of the face's width. Nothing crosses a rim.`,
    `Motifs, reading left to right, top row then bottom row:`,
    ...motifs,
    ...refs,
    `Style: Image 1's crisp, painterly game-emblem rendering: bold simple shapes, clean edges, soft light from the upper left, glow only where the colour line asks for it. Each motif is one clear subject that still reads at 48 px: a few large forms, restrained small detail. The ${count} must look clearly different from each other at a glance: vary silhouette, scale and composition, never the same object twice.`,
    `Background: genuinely transparent PNG with real alpha everywhere outside the hexagons. No drop shadow, outer glow, grey backdrop, panel, vignette, cell borders or grid lines.`,
    `Constraints: exactly ${count} badge${n === 1 ? "" : "s"}. No text, letters, numbers, Roman numerals, captions, labels, logos, watermark, frames or UI.`,
  ].join("\n");
  return { text, attach };
}

export function writeSheetPlan({ masters = mastersDir, doc = docPath, sheets: outDir = sheetsDir } = {}) {
  const prompts = JSON.parse(readFileSync(path.join(masters, "prompts.json"), "utf8"));
  const delivered = new Set(readdirSync(masters).filter(n => n.endsWith(".png")).map(n => n.slice(0, -4)));
  const sheets = planSheets(prompts, delivered);
  const planPath = path.join(outDir, "sheets.json");
  const landed = existsSync(outDir) ? readdirSync(outDir).filter(n => /\.(png|webp)$/i.test(n)) : [];
  if (landed.length) {
    const frozen = JSON.parse(readFileSync(planPath, "utf8"));
    for (const sheet of sheets) {
      assert.deepEqual(frozen[sheet.name], sheet.assets.map(a => a.key), `${sheet.name}: the plan is frozen once a sheet has landed (${landed.length} on disk); keep sheets.json`);
    }
  }
  mkdirSync(outDir, { recursive: true });
  const plan = Object.fromEntries(sheets.map(s => [s.name, s.assets.map(a => a.key)]));
  writeFileSync(planPath, `${JSON.stringify(plan, null, 2)}\n`);

  const total = sheets.reduce((n, s) => n + s.assets.length, 0);
  const firstWave = sheets.filter(s => s.name.endsWith("-01"));
  const md = [];
  md.push("# Research emblems: ChatGPT sheet prompts");
  md.push("");
  md.push(`Peter (Nov 3 2026): "more unique graphics for the research panel". The ${total}`);
  md.push("programme emblems still showing their field's badge, as sheets ChatGPT draws six");
  md.push("at a time, in the arrangement of the approved July field sheet. Generated from");
  md.push("`client/art-src/research-emblems-v1/prompts.json` by");
  md.push("`node client/scripts/build-research-sheet-prompts.mjs`; the motifs and colours are");
  md.push("the ones in `research-art-prompts.md`, only the packaging differs.");
  md.push("");
  md.push("## How to run it");
  md.push("");
  md.push(`1. One ChatGPT message per sheet: paste the block, attach the files its **Attach**`);
  md.push("   line names (Image 1 is always the field sheet; convert a `.webp` to PNG if the");
  md.push("   upload refuses it). Ask for the image at the size the block's Layout line gives.");
  md.push(`2. Save the result as \`client/art-src/research-emblems-v1/sheets/<sheet>.png\` (the`);
  md.push("   heading's name, for example `propulsion-01.png`). Keep the file as downloaded;");
  md.push("   the build slices and re-frames it.");
  md.push("3. `cd client && npm run assets:research`. It finds each badge by the clear gaps,");
  md.push("   checks the count against `sheets/sheets.json`, and derives the 64–512 px levels.");
  md.push("   A sheet with a backdrop, the wrong number of badges or a touching pair stops");
  md.push("   the build with the sheet's name: ask ChatGPT for the same sheet again, quoting");
  md.push("   the Background or Constraints line.");
  md.push(`4. Order: the six \`-01\` sheets first (${firstWave.reduce((n, s) => n + s.assets.length, 0)} badges, the Tier I–II programmes a`);
  md.push("   new player meets), then the rest. Lay the first finished sheet beside the field");
  md.push("   sheet before going on: if the badge drifts (shape, rim, face colour), fix the");
  md.push("   Badge line here and in `prompts.json` before the run.");
  md.push("5. A single emblem that needs redoing: `research-art-prompts.md` has its 1254 px");
  md.push("   prompt. A delivered `<key>.png` master wins over the sheet's cell.");
  md.push("");
  md.push("## Checking a sheet");
  md.push("");
  md.push("- Every badge the same hexagon as the field sheet: point up, navy-black face,");
  md.push("  thin dark rim, faint upper-left bevel; the same size as its neighbours.");
  md.push("- Real alpha between and around the badges; no shadow, glow or backdrop.");
  md.push("- The motifs in the listed order, each different from the others at a glance and");
  md.push("  readable at 48 px; no text or numerals anywhere.");
  md.push("");
  md.push(`## Sheets (${sheets.length})`);
  md.push("");
  md.push("| Sheet | Field | Badges | Programmes |");
  md.push("|---|---|---|---|");
  for (const s of sheets) md.push(`| \`${s.name}\` | ${FIELD_TITLE[s.field]} | ${s.assets.length} | ${s.assets.map(a => `${a.name} ${ROMAN[a.tier]}`).join(", ")} |`);
  for (const s of sheets) {
    const { text, attach } = sheetPrompt(s);
    md.push("");
    md.push(`### \`${s.name}\` · ${FIELD_TITLE[s.field]} · ${s.assets.length} badge${s.assets.length === 1 ? "" : "s"}`);
    md.push("");
    md.push(`Keys in reading order: ${s.assets.map(a => `\`${a.key}\``).join(", ")}.`);
    md.push("");
    md.push(`**Attach:** ${attach.map((f, i) => `Image ${i + 1} \`${f}\``).join("; ")}.`);
    md.push("");
    md.push("```text");
    md.push(text);
    md.push("```");
  }
  writeFileSync(doc, `${md.join("\n")}\n`);
  return { sheets: sheets.length, emblems: total, delivered: delivered.size };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  console.log("Research sheet prompts:", writeSheetPlan());
}
