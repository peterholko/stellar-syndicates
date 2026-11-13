// §research-panel (Nov 1 2026): research art and presentation stay in step with
// the catalogue. One prompt per programme; every object unlock pictured from
// art the game already has, captioned by the sim's building rules; emblem
// fallbacks that never loop; and a derivative build that works with no
// programme masters, frames good ones and stops on bad ones.
import assert from "node:assert/strict";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import sharp from "sharp";
import ts from "typescript";
import { buildResearchArt, checkProgrammeMaster, deliveredProgrammes, findSheetBadges, HEX_FILL, SHEET_MIN_BADGE } from "./build-research-art.mjs";
import { planSheets, sheetPrompt, PER_SHEET } from "./build-research-sheet-prompts.mjs";

const clientRoot = fileURLToPath(new URL("../", import.meta.url));
const mastersDir = join(clientRoot, "art-src/research-emblems-v1");
const src = path => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const deps = {};
function compile(path, globals = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(src(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: name => deps[name] ?? {}, ...globals });
  return exports;
}
const plain = value => JSON.parse(JSON.stringify(value));
const publicFile = url => join(clientRoot, "public", url.replace(/\?.*$/, ""));
const urlsIn = html => [...html.matchAll(/(?:src|srcset)="([^"]+)"/g)].flatMap(m => m[1].split(",").map(part => part.trim().split(/\s+/)[0]));

// ── The catalogue and its prompts ───────────────────────────────────────────
const catalog = readFileSync(new URL("../../crates/sim/src/research.rs", import.meta.url), "utf8").replace(/^\s*\/\/.*$/gm, "");
const snake = value => value.replace(/([a-z])([A-Z])/g, "$1_$2").toLowerCase();
const programmes = [...catalog.matchAll(/id: "([^"]+)",\s*field: Field::(\w+),\s*school: (None|Some\(School::\w+\)),\s*tier: (\d+),\s*name: "([^"]+)",[\s\S]*?effects: &\[([\s\S]*?)\],\s*hidden: (false|true),/g)]
  .filter(m => m[7] === "false")
  .map(([, id, field, , tier, name, effects]) => ({ id, field: snake(field), tier: Number(tier), name, effects }));
assert.equal(programmes.length, [...catalog.matchAll(/hidden: false/g)].length, "every visible programme parsed");
const prompts = JSON.parse(readFileSync(join(mastersDir, "prompts.json"), "utf8"));
const promptFor = new Map(prompts.assets.map(asset => [asset.key, asset]));
assert.equal(promptFor.size, prompts.assets.length, "one prompt per emblem");
for (const p of programmes) {
  const asset = promptFor.get(p.id);
  assert.ok(asset, `${p.id}: has an emblem prompt`);
  assert.deepEqual([asset.name, asset.field.toLowerCase(), asset.tier], [p.name, p.field, p.tier], `${p.id}: prompt matches the catalogue`);
  for (const ref of asset.reference_images) assert.ok(existsSync(join(mastersDir, ref)), `${p.id}: reference image ${ref} exists`);
}
assert.equal(promptFor.size, programmes.length, "no prompt for a programme the catalogue lacks");

// ── The ChatGPT sheets (docs/research-emblem-sheets.md) ─────────────────────
// sheets.json is the slicer's only key for a landed sheet: every programme
// without a single master sits in exactly one sheet, by field, six at most, in
// catalogue order. A later single master can override a cell without changing
// that frozen plan or the prompts in the doc.
const singles = new Set(readdirSync(mastersDir).filter(name => name.endsWith(".png")).map(name => name.slice(0, -4)));
const sheetPlan = JSON.parse(readFileSync(join(mastersDir, "sheets/sheets.json"), "utf8"));
const planned = Object.values(sheetPlan).flat();
const plannedIds = new Set(planned);
assert.equal(plannedIds.size, planned.length, "no programme on two sheets");
assert.deepEqual([...new Set([...planned, ...singles])].sort(), programmes.map(p => p.id).sort(), "every programme has a single master or a planned sheet cell");
const singlesOutsidePlan = new Set([...singles].filter(id => !plannedIds.has(id)));
const plannedSheets = planSheets(prompts, singlesOutsidePlan);
for (const [name, keys] of Object.entries(sheetPlan)) {
  assert.ok(keys.length >= 1 && keys.length <= PER_SHEET, `${name}: 1–${PER_SHEET} badges`);
  const fields = new Set(keys.map(key => promptFor.get(key).field.toLowerCase()));
  assert.deepEqual([...fields], [name.replace(/-\d+$/, "")], `${name}: one field, named for it`);
  const tiers = keys.map(key => promptFor.get(key).tier);
  assert.deepEqual(tiers, [...tiers].sort((a, b) => a - b), `${name}: catalogue order (tier first)`);
}
assert.deepEqual(Object.fromEntries(plannedSheets.map(s => [s.name, s.assets.map(a => a.key)])), sheetPlan, "sheets.json keeps its original order when a single master overrides a cell");
const sheetDoc = readFileSync(new URL("../../docs/research-emblem-sheets.md", import.meta.url), "utf8");
for (const sheet of plannedSheets) {
  const { text, attach } = sheetPrompt(sheet);
  assert.ok(sheetDoc.includes(text), `${sheet.name}: the doc carries its prompt`);
  assert.ok(sheetDoc.includes(`### \`${sheet.name}\``), `${sheet.name}: a heading`);
  for (const file of attach) assert.ok(existsSync(join(clientRoot, "..", file)), `${sheet.name}: attachment ${file} exists`);
  assert.equal((text.match(/^\d+\. /gm) ?? []).length, sheet.assets.length, `${sheet.name}: one motif line per badge`);
  const refs = sheet.assets.filter(a => a.reference_images.length > 1).length;
  assert.equal(attach.length, 1 + refs, `${sheet.name}: the field sheet plus one reference per hull or station`);
  assert.doesNotMatch(text, /Image 2 is .*?Image 2 is/s, `${sheet.name}: references numbered apart`);
  for (let i = 0; i < refs; i++) assert.ok(text.includes(`Image ${i + 2} is`), `${sheet.name}: Image ${i + 2} described`);
  assert.doesNotMatch(text.split("\n").find(l => l.startsWith("Motifs")) ?? "", /Image 2/, "motifs cite their own image numbers");
}

// ── Unlock pictures ─────────────────────────────────────────────────────────
deps["./protocol"] = deps["../../protocol"] = compile("protocol.ts");
deps["./icons"] = compile("icons.ts");
deps["./core/derive/station"] = compile("core/derive/station.ts");
deps["./core/derive/equipment"] = compile("core/derive/equipment.ts");
deps["./ship-models.generated"] = compile("ship-models.generated.ts");
const presentation = compile("research-presentation.ts");
const expected = {};
for (const p of programmes) {
  const unlocks = [...p.effects.matchAll(/Effect::(UnlockModule|UnlockHull|UnlockStructureTier)\(([^)]*)\)/g)].map(([, kind, args]) => {
    const [what, tier] = args.split(",").map(part => part.trim());
    const key = snake(what.replace(/^.*::/, ""));
    return kind === "UnlockStructureTier" ? { structure: key, tier: Number(tier) } : kind === "UnlockHull" ? { hull: key } : { module: key };
  });
  if (unlocks.length) expected[p.id] = unlocks;
}
assert.deepEqual(plain(presentation.RESEARCH_UNLOCKS), expected, "the unlock table carries every catalogue unlock, in order");
const objects = programmes.filter(p => (expected[p.id] ?? []).some(u => !("module" in u))).length;
const moduleOnly = programmes.filter(p => expected[p.id]?.every(u => "module" in u)).length;
// §research-locked (Nov 11 2026): Escort Datalink unlocks the Corvette too.
assert.deepEqual([objects, moduleOnly, programmes.length - objects - moduleOnly], [37, 5, 79], "37 hull/structure unlocks, 5 module-only, 79 modifier programmes");
const pic = unlock => presentation.unlockPicture(unlock, 72);
const check = (unlock, name, opens, art) => {
  const p = pic(unlock);
  assert.equal(p.name, name);
  assert.equal(p.opens, opens, `${name}: ${opens}`);
  assert.match(p.art, art, `${name}: depicts ${art}`);
};
check({ structure: "smelter", tier: 1 }, "Smelter", "Building", /structures\/2026-09-16\/smelter\/tier-1-128\.webp/);
check({ structure: "naval_drydock", tier: 1 }, "Orbital Shipyard", "Building", /naval_drydock\/tier-1/);
// build.rs max_buildable_tier: a marker of 4+ opens the prize tiers V–VI.
check({ structure: "mining_complex", tier: 4 }, "Mining Complex", "Tier V–VI", /mining_complex\/tier-5-128\.webp/);
check({ structure: "shipyard", tier: 6 }, "Shipyard", "Tier V–VI", /shipyard\/tier-5/);
check({ structure: "academy", tier: 4 }, "Academy", "Tier V–VI", /academy\/tier-5/);
check({ structure: "power_plant", tier: 4 }, "Power Plant", "Tier V–VI", /buildings\/power_plant\/tier-5/);
// Stations: the marker is the level. Command Centers stop at Tier III.
check({ structure: "orbital_defense", tier: 2 }, "Defense Platform", "Level 2", /stations\/v2\/level-2-128\.webp/);
check({ structure: "orbital_defense", tier: 6 }, "Star Fortress", "Level 6", /stations\/v2\/level-6/);
check({ structure: "command_center", tier: 3 }, "Command Center", "Tier III", /command-center\/v1\/tier-3/);
check({ structure: "veil_projector", tier: 1 }, "Veil Projector", "Building", /buildings\/veil_projector\/tier-1/);
check({ structure: "veil_projector", tier: 3 }, "Veil Projector", "Tier III", /buildings\/veil_projector\/tier-3/);
// The Antimatter Plant still borrows the Fuel Refinery (docs/fuel-overhaul-art-prompt.md).
check({ structure: "antimatter_plant", tier: 1 }, "Antimatter Plant", "Building", /fuel_refinery\/tier-1/);
check({ hull: "convoy" }, "Medium Freighter", "Hull", /models-v1\/previews\/convoy-768\.webp/);
check({ hull: "mining_belt_station" }, "Mining Belt Station", "Hull", /mining_belt_station-768\.webp/);
check({ module: "jump_drive" }, "Jump Drive", "Module", /class="icon/);
for (const [id, unlocks] of Object.entries(expected)) {
  for (const unlock of unlocks) {
    const p = pic(unlock);
    assert.ok(p.name && p.opens, `${id}: captioned`);
    for (const url of urlsIn(p.art).filter(url => url.startsWith("/art/derived/"))) assert.ok(existsSync(publicFile(url)), `${id}: ${url} exists`);
  }
}
assert.ok(programmes.some(p => p.id === "prop_drive_tuning") && !expected.prop_drive_tuning, "Drive Tuning is a modifier programme");
assert.equal(presentation.researchUnlocks("prop_drive_tuning").length, 0, "modifier programmes have no Unlocks section");

// ── Emblem resolution and fallback ──────────────────────────────────────────
deps["./research-art.generated"] = compile("research-art.generated.ts");
const art = compile("research-art.ts");
const generated = deps["./research-art.generated"];
assert.deepEqual(Object.keys(generated.RESEARCH_FIELD_ART).sort(), ["computation", "hulls", "life", "materials", "propulsion", "weapons"]);
for (const [field, levels] of Object.entries(generated.RESEARCH_FIELD_ART)) {
  assert.deepEqual(Object.keys(levels), ["64", "128", "256"], `${field}: three sizes`);
  for (const url of Object.values(levels)) assert.ok(existsSync(publicFile(url)), `${field}: ${url} exists`);
}
const delivered = await deliveredProgrammes(mastersDir);
assert.deepEqual(Object.keys(generated.RESEARCH_PROGRAMME_ART).sort(), delivered, "the manifest lists exactly the delivered masters and sheet cells (run npm run assets:derive)");
for (const [id, levels] of Object.entries(generated.RESEARCH_PROGRAMME_ART)) {
  assert.ok(promptFor.has(id), `${id}: a catalogue programme`);
  assert.deepEqual(Object.keys(levels), ["64", "128", "256", "512"], `${id}: four sizes`);
  for (const url of Object.values(levels)) assert.ok(existsSync(publicFile(url)), `${id}: ${url} exists`);
}
const missing = programmes.find(p => !generated.RESEARCH_PROGRAMME_ART[p.id]);
if (missing) {
  const html = art.researchEmblem(missing.field, missing.id, 128);
  assert.match(html, new RegExp(`fields/${missing.field}-128\\.webp`), "a programme without its master shows its field emblem");
  assert.doesNotMatch(html, /programmes\//, "and never requests a file that does not exist");
}

// With a delivered emblem: programme art first, then the field's, then the
// field's icon. A failed request is remembered and swapped once, never retried.
deps["./research-art.generated"] = {
  RESEARCH_FIELD_ART: generated.RESEARCH_FIELD_ART,
  RESEARCH_PROGRAMME_ART: { prop_drive_tuning: { 64: "/p-64.webp", 128: "/p-128.webp", 256: "/p-256.webp", 512: "/p-512.webp" } },
};
let errorHandler = null;
class FakeImage {
  constructor(attrs) { Object.assign(this, attrs); this.swaps = 0; }
  get classList() { return { contains: name => this.className.split(" ").includes(name), [Symbol.iterator]: () => this.className.split(" ")[Symbol.iterator]() }; }
  set outerHTML(html) { this.swaps++; this.replacement = html; }
}
const fallbackArt = compile("research-art.ts", { HTMLImageElement: FakeImage,
  document: { addEventListener: (type, handler, capture) => { if (type === "error" && capture) errorHandler = handler; } } });
const first = fallbackArt.researchEmblem("propulsion", "prop_drive_tuning", 128, { eager: true, cls: "x" });
assert.match(first, /src="\/p-128\.webp" srcset="\/p-64\.webp 64w, \/p-128\.webp 128w, \/p-256\.webp 256w, \/p-512\.webp 512w" sizes="128px" width="128" height="128" alt=""/);
assert.match(first, /data-research-art="programme:prop_drive_tuning"/);
assert.ok(errorHandler, "one capture-phase error listener");
const broken = new FakeImage({ className: "research-emblem x", dataset: { researchArt: "programme:prop_drive_tuning", researchField: "propulsion", researchPx: "128" }, loading: "eager", width: 128 });
errorHandler({ target: broken });
assert.equal(broken.swaps, 1);
assert.match(broken.replacement, /fields\/propulsion-128\.webp[\s\S]*class="research-emblem x"|class="research-emblem x"[\s\S]*fields\/propulsion-128\.webp/, "a failed programme emblem swaps to the field emblem, keeping its classes");
assert.match(broken.replacement, /loading="eager"/);
errorHandler({ target: broken });
assert.equal(broken.swaps, 1, "an emblem already known to fail is not swapped again");
assert.match(fallbackArt.researchEmblem("propulsion", "prop_drive_tuning", 64), /fields\/propulsion-64\.webp/, "later renders skip the failed emblem without a request");
fallbackArt.markEmblemFailed("field:propulsion");
const last = fallbackArt.researchEmblem("propulsion", "prop_drive_tuning", 48);
assert.match(last, /<span class="research-emblem research-emblem--icon" style="--emblem:48px"[^>]*aria-hidden="true">/, "the field icon is the final fallback");
assert.doesNotMatch(last, /research\/v1/);
assert.match(fallbackArt.researchEmblem("materials", null, 40), /fields\/materials-64\.webp/, "other fields are unaffected");
// Every master is delivered now, so the real-manifest check above finds
// nothing missing; this manifest still lacks Deep Bores.
const unpainted = fallbackArt.researchEmblem("materials", "mat_deep_bores", 128);
assert.match(unpainted, /fields\/materials-128\.webp/, "a programme without its master shows its field emblem");
assert.doesNotMatch(unpainted, /programmes\//, "and never requests a file that does not exist");
const unrelated = new FakeImage({ className: "icon", dataset: {} });
errorHandler({ target: unrelated });
assert.equal(unrelated.swaps, 0, "other images are left alone");

// ── The derivative build ────────────────────────────────────────────────────
const hexMaster = async (size, fill, background = { r: 0, g: 0, b: 0, alpha: 0 }) => {
  const h = size * fill, w = h * 0.91, cx = size / 2, cy = size / 2;
  const points = [[cx, cy - h / 2], [cx + w / 2, cy - h / 4], [cx + w / 2, cy + h / 4], [cx, cy + h / 2], [cx - w / 2, cy + h / 4], [cx - w / 2, cy - h / 4]];
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}"><polygon points="${points.map(p => p.join(",")).join(" ")}" fill="#13203a" stroke="#2b2f36" stroke-width="${size / 60}"/><circle cx="${cx}" cy="${cy}" r="${size / 6}" fill="#7fd0ff"/></svg>`;
  return sharp({ create: { width: size, height: size, channels: 4, background } })
    .composite([{ input: Buffer.from(svg) }]).png().toBuffer();
};
const badgeShare = async file => {
  const { data, info } = await sharp(file).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  let top = info.height, bottom = -1;
  for (let y = 0; y < info.height; y++) for (let x = 0; x < info.width; x++) {
    if (data[(y * info.width + x) * 4 + 3] >= 128) { top = Math.min(top, y); bottom = Math.max(bottom, y); }
  }
  return { share: (bottom - top + 1) / info.height, corner: data[3], centre: data[((info.height >> 1) * info.width + (info.width >> 1)) * 4 + 3] };
};
const scratch = mkdtempSync(join(tmpdir(), "research-art-"));
try {
  const masters = join(scratch, "masters"), output = join(scratch, "out"), generatedFile = join(scratch, "research-art.generated.ts");
  mkdirSync(masters);
  copyFileSync(join(mastersDir, "prompts.json"), join(masters, "prompts.json"));
  const run = () => buildResearchArt({ masters, output, generated: generatedFile });
  let result = await run();
  assert.deepEqual([result.fields, result.programmes, result.known], [6, 0, programmes.length], "builds with zero programme masters");
  assert.match(readFileSync(generatedFile, "utf8"), /RESEARCH_PROGRAMME_ART[^=]*= \{\};/);
  for (const field of ["propulsion", "hulls"]) {
    for (const size of [64, 256]) {
      const badge = await badgeShare(join(output, `fields/${field}-${size}.webp`));
      assert.ok(Math.abs(badge.share - HEX_FILL) < 0.02, `${field} ${size}: hexagon at 88% (${badge.share.toFixed(3)})`);
      assert.equal(badge.corner, 0, `${field} ${size}: the sheet's backdrop is masked away`);
      assert.equal(badge.centre, 255, `${field} ${size}: the badge face is opaque`);
    }
  }
  // A good master with loose margins is reframed to the shared 88%.
  writeFileSync(join(masters, "prop_drive_tuning.png"), await hexMaster(1254, 0.7));
  result = await run();
  assert.equal(result.programmes, 1);
  for (const size of [64, 128, 256, 512]) {
    const badge = await badgeShare(join(output, `programmes/prop_drive_tuning-${size}.webp`));
    assert.ok(Math.abs(badge.share - HEX_FILL) < 0.02, `programme ${size}: reframed to 88% (${badge.share.toFixed(3)})`);
    assert.equal(badge.corner, 0);
  }
  assert.match(readFileSync(generatedFile, "utf8"), /"prop_drive_tuning": \{\s*"64": "\/art\/derived\/research\/v1\/programmes\/prop_drive_tuning-64\.webp\?v=[0-9a-f]{12}"/);
  assert.equal((await run()).written, 0, "an unchanged master is not derived again");
  writeFileSync(join(masters, "not_a_programme.png"), await hexMaster(1254, 0.88));
  await assert.rejects(run(), /no programme has the id "not_a_programme"/, "an unknown id stops the build");
  rmSync(join(masters, "not_a_programme.png"));
  writeFileSync(join(masters, "mat_deep_bores.png"), await hexMaster(1254, 0.88, { r: 60, g: 64, b: 70, alpha: 1 }));
  await assert.rejects(run(), /mat_deep_bores: real transparency/, "a baked backdrop stops the build");
  writeFileSync(join(masters, "mat_deep_bores.png"), await sharp(await hexMaster(1254, 0.88)).extract({ left: 0, top: 0, width: 1254, height: 1100 }).png().toBuffer());
  await assert.rejects(run(), /mat_deep_bores: square master/, "a cropped master stops the build");
  writeFileSync(join(masters, "mat_deep_bores.png"), await sharp(await hexMaster(1254, 0.88)).resize(512, 512).png().toBuffer());
  await assert.rejects(run(), /mat_deep_bores: native master/, "a preview-sized master stops the build");
  rmSync(join(masters, "mat_deep_bores.png"));

  // ── Sheets: badges found by their gaps, in reading order, framed like masters.
  const sheetOf = async (cells, { width = 1536, height = 1024, background = { r: 0, g: 0, b: 0, alpha: 0 }, badge = 450 } = {}) => {
    const composite = [];
    for (const [cx, cy, mark] of cells) {
      // A solid square of one colour at each badge's centre tells the cells apart.
      const square = { input: { create: { width: 80, height: 80, channels: 4, background: { ...(mark ?? { r: 255, g: 255, b: 255 }), alpha: 1 } } }, left: 216, top: 216 };
      const png = await sharp(await hexMaster(512, badge / 512)).composite([square]).png().toBuffer();
      composite.push({ input: png, left: Math.round(cx - 256), top: Math.round(cy - 256) });
    }
    return sharp({ create: { width, height, channels: 4, background } }).composite(composite).png().toBuffer();
  };
  const grid = [[256, 256], [768, 256], [1280, 256], [256, 768], [768, 768], [1280, 768]];
  mkdirSync(join(masters, "sheets"));
  writeFileSync(join(masters, "sheets/sheets.json"), JSON.stringify({
    "materials-01": ["mat_deep_bores", "mat_enrichment", "mat_bulk_storage", "mat_ore_recovery", "mat_beneficiation", "mat_prefab_construction"],
    "weapons-04": ["weap_cm_v_grand_batteries", "weap_cm_v_star_fortress", "weap_cm_v_citadel_compartments"],
  }));
  writeFileSync(join(masters, "sheets/materials-01.png"), await sheetOf(grid.map((c, i) => [...c, { r: 40 * i + 50, g: 90, b: 160 }])));
  result = await run();
  assert.deepEqual([result.programmes, result.sheets], [7, 1], "one master and a six-badge sheet");
  for (const [i, id] of ["mat_deep_bores", "mat_enrichment", "mat_bulk_storage", "mat_ore_recovery", "mat_beneficiation", "mat_prefab_construction"].entries()) {
    for (const size of [256, 512]) {
      const badge = await badgeShare(join(output, `programmes/${id}-${size}.webp`));
      assert.ok(Math.abs(badge.share - HEX_FILL) < 0.02, `${id} ${size}: sheet badge reframed to 88% (${badge.share.toFixed(3)})`);
      assert.equal(badge.corner, 0);
    }
    const { data } = await sharp(join(output, `programmes/${id}-64.webp`)).raw().toBuffer({ resolveWithObject: true });
    assert.ok(Math.abs(data[(32 * 64 + 32) * 4] - (40 * i + 50)) < 12, `${id}: cell ${i + 1} of the sheet, in reading order (red ${data[(32 * 64 + 32) * 4]})`);
  }
  const manifest = JSON.parse(readFileSync(join(output, "manifest.json"), "utf8"));
  assert.match(manifest.art["programme:mat_ore_recovery"].source, /sheets\/materials-01\.png#4$/, "the manifest names the sheet and cell");
  assert.equal((await run()).written, 0, "an unchanged sheet is not sliced again");
  // A single master wins over its sheet cell.
  writeFileSync(join(masters, "mat_enrichment.png"), await hexMaster(1254, 0.88));
  result = await run();
  assert.equal(result.programmes, 7);
  assert.match(JSON.parse(readFileSync(join(output, "manifest.json"), "utf8")).art["programme:mat_enrichment"].source, /mat_enrichment\.png$/, "a delivered master replaces the sheet's cell");
  rmSync(join(masters, "mat_enrichment.png"));
  // Bad sheets stop the build and name themselves.
  writeFileSync(join(masters, "sheets/weapons-04.png"), await sheetOf(grid.slice(0, 3), { background: { r: 60, g: 64, b: 70, alpha: 1 } }));
  await assert.rejects(run(), /weapons-04: real transparency/, "a sheet with a backdrop stops the build");
  writeFileSync(join(masters, "sheets/weapons-04.png"), await sheetOf(grid.slice(0, 2)));
  await assert.rejects(run(), /weapons-04: 2 badges found, 3 expected/, "a short sheet stops the build");
  writeFileSync(join(masters, "sheets/weapons-04.png"), await sheetOf([[256, 256], [600, 256], [1280, 256]]));
  await assert.rejects(run(), /weapons-04: 2 badges found, 3 expected \(touching/, "touching badges merge and stop the build");
  writeFileSync(join(masters, "sheets/weapons-04.png"), await sheetOf(grid.slice(0, 3), { badge: 300 }));
  await assert.rejects(run(), new RegExp(`weapons-04 badge 1: 3\\d\\d px tall, ${SHEET_MIN_BADGE} needed`), "small badges stop the build");
  writeFileSync(join(masters, "sheets/weapons-04.png"), await sheetOf(grid.slice(0, 3)));
  writeFileSync(join(masters, "sheets/sheets.json"), JSON.stringify({ "materials-01": ["mat_deep_bores"], "weapons-04": ["weap_cm_v_grand_batteries", "weap_cm_v_star_fortress", "weap_cm_v_citadel_compartments"] }));
  await assert.rejects(run(), /materials-01: 6 badges found, 1 expected/, "a sheet is held to its plan");
  writeFileSync(join(masters, "sheets/sheets.json"), JSON.stringify({ "weapons-04": ["weap_cm_v_grand_batteries", "nope", "weap_cm_v_citadel_compartments"] }));
  await assert.rejects(run(), /sheets\/weapons-04\.png: no programme has the id "nope"|sheets\/materials-01\.png: no sheet "materials-01"/, "unknown ids and unplanned sheets stop the build");
} finally {
  rmSync(scratch, { recursive: true, force: true });
}
assert.throws(() => checkProgrammeMaster("wide", Buffer.alloc(1254 * 1254 * 4), { width: 1254, height: 1254, channels: 4 }), /a visible badge/);

console.log(`PASS: ${programmes.length} programme prompts on ${Object.keys(sheetPlan).length} sheets, ${Object.keys(expected).length} unlock tables with existing art, emblem fallback without retries, and the derivative build (empty, framed, cached, sliced, rejected).`);
