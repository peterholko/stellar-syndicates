// In-memory UI checks and an optional isolated visual preview. No game socket.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import ts from "typescript";
import sharp from "sharp";
import { buildFoundingStoryArt, FOUNDING_STORY_ART, FOUNDING_STORY_WIDTHS } from "./build-founding-story-art.mjs";
const root = fileURLToPath(new URL("../", import.meta.url));
const source = name => readFileSync(resolve(root, "src", name), "utf8");
const compile = name => ts.transpileModule(source(name), { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS } }).outputText;
const icons = {};
vm.runInNewContext(compile("icons.ts"), { exports: icons });
class Dialog {
  open = false; innerHTML = ""; scrollTop = 0; handlers = {};
  setAttribute() {} contains() { return false; } querySelectorAll() { return []; } querySelector() { return null; }
  addEventListener(type, fn) { this.handlers[type] = fn; }
  showModal() { this.open = true; } close() { this.open = false; }
}
const elements = [], handlers = {};
const document = { activeElement: null, createElement: () => { const el = new Dialog(); elements.push(el); return el; },
  body: { append() {} }, addEventListener: (type, fn) => { handlers[type] = fn; } };
const exports = {};
const prng = {}, surfaces = {};
vm.runInNewContext(compile("prng.ts"), { exports: prng });
vm.runInNewContext(compile("planet-surfaces.ts"), { exports: surfaces, require: () => prng });
// §settle-near-friends (R7): the invite code a link left in this browser.
const invite = { code: null, cleared: 0 };
const social = { storedInvite: () => invite.code, clearStoredInvite: () => { invite.code = null; invite.cleared++; } };
const dependencies = name => ({ "./icons": icons, "./planet-surfaces": surfaces, "./core/derive/fleet": { WARP_FACTOR: 5 }, "./core/derive/social": social })[name];
vm.runInNewContext(compile("settlement.ts"), { exports, require: dependencies, document });
const { SettlementUI, SETTLEMENT_PROJECTS, FOUNDING_PROFILES, FOUNDING_STORY, projectStatus } = exports;
assert.equal(FOUNDING_STORY.length, 4, "the prologue includes the frontier warning before world selection");
assert.equal(FOUNDING_STORY[1].title, "Stellar Charter");
assert.equal(FOUNDING_STORY[1].text, "{corporation} has been granted a stellar charter by the Terran Charter Authority, the privilege to settle a new planet. Seven thousand settlers are ready to begin a new life under your leadership. The Authority will not forget what you owe.");
assert.deepEqual(Array.from(FOUNDING_STORY, slide => slide.art), FOUNDING_STORY_ART);
assert.equal(new Set(FOUNDING_STORY.map(slide => slide.art)).size, FOUNDING_STORY.length);
const provenance = JSON.parse(readFileSync(resolve(root, "art-src/founding-intro-v1/prompts.json"), "utf8"));
assert.equal(provenance.mode, "built-in image_gen");
assert.deepEqual(provenance.images.map(image => image.slug), FOUNDING_STORY_ART);
await buildFoundingStoryArt();
for (const slide of FOUNDING_STORY) {
  assert.ok(slide.title && slide.text && slide.alt, "captions and alternative text are separate from the art");
  const master = resolve(root, `art-src/founding-intro-v1/${slide.art}.png`);
  for (const width of FOUNDING_STORY_WIDTHS) {
    const derivative = resolve(root, `public/art/derived/founding-intro/v1/${slide.art}-${width}.webp`);
    const meta = await sharp(derivative).metadata();
    assert.equal(meta.width, width);
    assert.equal(meta.height, Math.round(width * 941 / 1672));
    const expected = await sharp(master).resize({ width, withoutEnlargement: true }).webp({ quality: 88, effort: 5 }).toBuffer();
    assert.ok(expected.equals(readFileSync(derivative)), "responsive art preserves the full approved frame without upscaling");
  }
}
assert.equal((await buildFoundingStoryArt()).written, 0, "unchanged story art does not rebuild");
assert.doesNotMatch(source("settlement.ts"), /setTimeout|setInterval/, "reading the story is untimed");
assert.equal(FOUNDING_PROFILES.length, 3);
assert.deepEqual(Array.from(FOUNDING_PROFILES, p => p.terrain), ["highland-plateau", "temperate-basin", "archipelago-world"]);
for (const profile of FOUNDING_PROFILES) assert.equal(profile.terrain,
  surfaces.planetSurface("/art/terrestrial-512.png", "home", { id: 1, kind: "terrestrial", habitable: true, founding_camp: { profile: profile.id, completed: [] } }),
  "the landing preview and the settled planet use the same terrain");
const recipes = readFileSync(resolve(root, "../crates/sim/src/settlement.rs"), "utf8");
// §habitat-rebase (Nov 1 2026): three projects; Permanent Quarters is gone.
assert.equal(SETTLEMENT_PROJECTS.length, 3);
assert.doesNotMatch(recipes, /Self::Quarters/);
for (const [i, variant] of ["Prospect", "SurfaceWorkings", "Storehouse"].entries()) {
  const recipe = recipes.match(new RegExp(`Self::${variant} => &Recipe \\{([\\s\\S]*?)\\n\\s*\\},`))?.[1];
  assert.ok(recipe, `authoritative recipe ${variant} exists`);
  assert.equal(Number(recipe.match(/build_ticks: (\d+) \* HZ/)[1]), SETTLEMENT_PROJECTS[i].seconds);
  assert.equal(JSON.stringify([...recipe.matchAll(/\((\w+), ([\d.]+)\)/g)].map(m => [m[1].toLowerCase(), Number(m[2])])), JSON.stringify(SETTLEMENT_PROJECTS[i].costs));
}
const body = { id: 1, name: "Freya II", population: .007, population_capacity: .0125, founding_camp: { profile: "mineral_highlands", completed: [] },
  structures: { warehouse: 1, habitat: 1, bioharvester: 1, agroplex: 1 } };
const system = { id: "home", owner: "1", bodies: [body], storage_used: 120, storage_cap: 250, builds: [], stockpile: [
  { commodity: "alloys", units: 30 }, { commodity: "machinery", units: 15 } ] };
const state = { playerId: "1", name: "Atlas Ventures", link: "online", systems: [], commandCenter: { x: 0, y: 0 }, galaxy: { instance_id: "fixture", c: 1000, systems: [{ id: "home", pos: { x: 0, y: 0 } }] } };
const sent = [], ui = new SettlementUI(state, msg => sent.push(msg));
const dialog = elements[0];
const click = (act, extra = {}) => dialog.handlers.click({ target: { closest: () => ({ disabled: false, dataset: { settlementAct: act, ...extra } }) } });
ui.onMessage({ type: "Welcome", player_id: "1", galaxy: { instance_id: "fixture" } });
assert.equal(dialog.open, false, "existing players never see an unsolicited intro");
// §founding: the server deals the home world; there is nothing to choose.
const DEALT = { type: "FoundingOffer", profile: "coastal_plains" };
ui.onMessage(DEALT);
assert.ok(dialog.open); assert.ok(dialog.innerHTML.includes(FOUNDING_STORY[0].title));
click("back"); assert.ok(dialog.innerHTML.includes(FOUNDING_STORY[0].title), "Back cannot go before the first slide");
for (const [i, slide] of FOUNDING_STORY.entries()) {
  assert.ok(dialog.innerHTML.includes(slide.title));
  if (i === 1) {
    assert.ok(dialog.innerHTML.includes("Atlas Ventures has been granted a stellar charter by the Terran Charter Authority,"), "the charter uses the signed-in corporation's name");
    for (const [name, expected] of [
      ["  Venture 42  ", "Venture 42"],
      ["", "Your corporation"], ["   ", "Your corporation"], [undefined, "Your corporation"],
      ["<img src=x onerror=alert(1)> & \"$&\" 'Co'", "&lt;img src=x onerror=alert(1)&gt; &amp; &quot;$&amp;&quot; &#39;Co&#39;"],
    ]) {
      state.name = name; ui.connectionChanged();
      assert.ok(dialog.innerHTML.includes(`<p>${expected} has been granted a stellar charter by the Terran Charter Authority,`), "names render as escaped literal text, with a fallback when unavailable");
      assert.doesNotMatch(dialog.innerHTML, /<img src=x|\{corporation\}/);
    }
    state.name = "Atlas Ventures"; ui.connectionChanged();
    assert.equal(sent.length, 0, "personalization never issues an order");
  }
  assert.doesNotMatch(dialog.innerHTML, /Your colony clock starts/);
  if (state.link === "online") assert.doesNotMatch(dialog.innerHTML, /role="status"/, "no empty reminder row remains");
  assert.ok(dialog.innerHTML.includes(`Prologue · ${i + 1} / ${FOUNDING_STORY.length}`));
  assert.ok(dialog.innerHTML.includes(`/founding-intro/v1/${slide.art}-1280.webp`));
  assert.match(dialog.innerHTML, /width="1672" height="941" alt="[^\"]+"/);
  assert.match(dialog.innerHTML, /srcset="[^\"]*768w[^\"]*1280w[^\"]*1672w/);
  assert.doesNotMatch(dialog.innerHTML, /data-settlement-act="land"/);
  click("land"); assert.equal(sent.length, 0, "a stale or premature landing click cannot send an order");
  dialog.scrollTop = 100; click("skip"); assert.match(dialog.innerHTML, /Settle the Broad Coastal Plains/, "Skip goes to the dealt world's landing card");
  assert.equal(dialog.scrollTop, 0, "navigation resets the story scroll");
  assert.doesNotMatch(dialog.innerHTML, /data-settlement-act="profile"|Choose where/, "no world to choose");
  click("back"); assert.ok(dialog.innerHTML.includes(FOUNDING_STORY.at(-1).title));
  for (let previous = FOUNDING_STORY.length - 1; previous > i; previous--) click("back");
  if (i === 1) {
    state.link = "offline"; ui.connectionChanged();
    assert.match(dialog.innerHTML, /keep reading/);
    ui.onMessage({ type: "Welcome", player_id: "1", galaxy: { instance_id: "fixture" } });
    ui.onMessage(DEALT);
    assert.ok(dialog.innerHTML.includes(slide.title), "reconnect preserves the current story slide");
    assert.ok(dialog.innerHTML.includes("Atlas Ventures has been granted"), "reconnect preserves the personalized charter");
  }
  click("next");
}
assert.match(dialog.innerHTML, /Settle the Broad Coastal Plains/);
assert.equal(state.link, "offline", "reading and navigation work without a connection");
state.link = "online"; ui.connectionChanged();
let cancelled = false;
click("back"); dialog.handlers.cancel({ preventDefault() { cancelled = true; } });
assert.equal(cancelled, true); assert.equal(dialog.open, true);
assert.match(dialog.innerHTML, /Settle the Broad Coastal Plains/, "Escape skips the story without settling or dismissing the landing");
const coastal = FOUNDING_PROFILES.find(p => p.id === "coastal_plains");
assert.ok(dialog.innerHTML.includes(`/habitable-v1/${coastal.terrain}-`), "the dealt world's own terrain");
assert.ok(dialog.innerHTML.includes(coastal.opportunity), "and its local opportunity");
for (const other of FOUNDING_PROFILES.filter(p => p.id !== "coastal_plains")) assert.ok(!dialog.innerHTML.includes(`/habitable-v1/${other.terrain}-`));
assert.doesNotMatch(dialog.innerHTML, /barren-/, "a habitable home never uses barren art");
assert.doesNotMatch(dialog.innerHTML, /Your colony clock starts|role="status"/, "confirmation has no clock reminder or empty row");
click("back"); click("land"); assert.equal(sent.length, 0, "a stale landing click after Back is ignored");
click("next");
assert.equal(sent.length, 0, "slides are drafts only");
state.link = "offline"; ui.connectionChanged(); click("land");
assert.equal(sent.length, 0, "offline landing is not sent");
ui.onMessage({ type: "Welcome", player_id: "1", galaxy: { instance_id: "fixture" } });
ui.onMessage(DEALT);
assert.match(dialog.innerHTML, /Settle the Broad Coastal Plains/, "reconnect keeps the landing card");
state.link = "online"; click("land"); click("land");
assert.equal(sent.length, 1, "double landing click is ignored");
assert.deepEqual(JSON.parse(JSON.stringify(sent[0])), { type: "FoundColony" }, "no choice travels with the landing");
assert.match(dialog.innerHTML, /Landing order sent\. Waiting for your colony report/, "real status messages remain visible");
click("back"); click("skip"); dialog.handlers.cancel({ preventDefault() {} });
assert.match(dialog.innerHTML, /Establishing colony/, "pending settlement cannot be navigated away from");
assert.ok(dialog.open, "wait for actual served colony, not optimistic success");
state.systems = [system]; ui.onMessage({ type: "View", systems: [system] });
assert.equal(dialog.open, false);
assert.equal(projectStatus(body, system, SETTLEMENT_PROJECTS[1]), "Needs survey and mine");
assert.equal(projectStatus(body, system, SETTLEMENT_PROJECTS[0]), "Ready");
system.industry = { reservations: [{ goods: { alloys: 29 } }] };
assert.equal(projectStatus(body, system, SETTLEMENT_PROJECTS[2]), "Needs materials", "reserved goods are not spendable");
system.industry = undefined;
handlers.click({ target: { closest: () => ({ dataset: { system: "home", body: "1" } }) } });
state.commandCenter = { x: 50000, y: 0 };
click("review", { project: "storehouse" }); assert.match(dialog.innerHTML, /Confirm project/);
assert.match(dialog.innerHTML, /Order travel: 10s/, "project delay uses warp-light speed");
state.commandCenter = { x: 0, y: 0 };
assert.equal(sent.length, 1); click("confirm"); assert.equal(sent[1].type, "DevelopSettlement");
assert.equal(body.population_capacity, .0125, "click cannot update housing");
system.builds.push({ body_id: 1, key: "camp_storehouse", queued: true });
assert.equal(projectStatus(body, system, SETTLEMENT_PROJECTS[2]), "Queued");
system.builds = []; body.founding_camp.completed.push("storehouse");
assert.equal(projectStatus(body, system, SETTLEMENT_PROJECTS[2]), "Complete");
body.founding_camp.completed.push("prospect"); body.structures.mining_complex = 1;
system.stockpile = []; assert.equal(projectStatus(body, system, SETTLEMENT_PROJECTS[1]), "Needs materials");
ui.endSession(); assert.equal(dialog.open, false, "session replacement dismisses the modal");
state.name = "Nova Industries";
ui.onMessage({ type: "Welcome", player_id: "2", galaxy: { instance_id: "new-fixture" } });
assert.equal(dialog.open, false);
ui.onMessage({ type: "FoundingOffer", profile: "verdant_lowlands" });
assert.ok(dialog.innerHTML.includes(FOUNDING_STORY[0].title), "a different account or galaxy starts at the first story slide");
click("next");
assert.ok(dialog.innerHTML.includes("Nova Industries has been granted"), "a different account uses its own corporation name");
assert.doesNotMatch(dialog.innerHTML, /Atlas Ventures/);
click("skip"); assert.match(dialog.innerHTML, /Settle the Verdant Lowlands/, "each account lands on the world it was dealt");
ui.endSession();
// §settle-near-friends (R7): an invite link is redeemed with the offer and
// named on the landing card; a refused one says why and is forgotten.
{
  invite.code = "ABCD2345";
  const sentInvite = [], invited = new SettlementUI({ ...state, systems: [] }, msg => sentInvite.push(msg));
  const card = elements.at(-1);
  invited.onMessage({ type: "Welcome", player_id: "7", galaxy: { instance_id: "invite" } });
  invited.onMessage({ type: "FoundingOffer", profile: "coastal_plains" });
  assert.deepEqual(JSON.parse(JSON.stringify(sentInvite)), [{ type: "RedeemInvite", code: "ABCD2345" }]);
  invited.onMessage({ type: "InviteStatus", ok: true, from: "Harbor <Co>", near: "Vega", syndicate: "Pact" });
  card.handlers.click({ target: { closest: () => ({ disabled: false, dataset: { settlementAct: "skip" } }) } });
  assert.match(card.innerHTML, /Invited by <b>Harbor &lt;Co&gt;<\/b> · home near Vega · joins Pact/);
  invited.onMessage({ type: "InviteStatus", ok: false, message: "This invite has expired or been used." });
  assert.match(card.innerHTML, /settlement-invite is-refused">This invite has expired or been used\./);
  assert.equal(invite.code, null, "a refused code is forgotten");
}

console.log("Settlement UI: four illustrated slides, personalized and escaped charter names, responsive art, manual navigation, skip, the dealt world's landing card, duplicate clicks, reconnect, project requirements and served-only progress passed.");

if (process.argv.includes("--serve")) {
  const previewState = { ...state, systems: [] };
  const previewSystem = { ...system, stockpile: [{ commodity: "alloys", units: 30 }, { commodity: "machinery", units: 15 }],
    bodies: [{ ...body, founding_camp: { profile: "mineral_highlands", completed: [] } }] };
  const script = `const icons={};((exports)=>{${compile("icons.ts")}})(icons);
    const prng={};((exports)=>{${compile("prng.ts")}})(prng);
    const surfaces={};((exports,require)=>{${compile("planet-surfaces.ts")}})(surfaces,()=>prng);
    const module={};((exports,require)=>{${compile("settlement.ts")}})(module,name=>({'./icons':icons,'./planet-surfaces':surfaces,'./core/derive/fleet':{WARP_FACTOR:5},'./core/derive/social':{storedInvite:()=>null,clearStoredInvite(){}}})[name]);
    const state=${JSON.stringify(previewState)};const system=${JSON.stringify(previewSystem)};
    const ui=new module.SettlementUI(state, msg=>{if(msg.type==='FoundColony'){system.bodies[0].founding_camp.profile='verdant_lowlands';state.systems=[system];ui.onMessage({type:'View',systems:[system]});document.getElementById('projects').hidden=false;}else{document.getElementById('last-order').textContent=JSON.stringify(msg);}});
    ui.onMessage({type:'Welcome',player_id:'1',galaxy:{instance_id:'fixture'}});ui.onMessage({type:'FoundingOffer',profile:'verdant_lowlands'});`;
  const html = `<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Founding preview</title><style>body{font-family:system-ui;background:#060b12;color:#dce7ed} ${source("styles/settlement.css")}</style><body><p>Offline UI preview · no live game connection</p><button id="projects" data-settlement-open data-system="home" data-body="1" hidden>Settlement projects</button><pre id="last-order"></pre><script>${script}</script></body></html>`;
  const publicRoot = resolve(root, "public");
  const server = createServer((req, res) => {
    if (req.url === "/") { res.setHeader("Content-Type", "text/html"); res.end(html); return; }
    const file = resolve(publicRoot, "." + decodeURIComponent(new URL(req.url, "http://localhost").pathname));
    if (!file.startsWith(publicRoot + sep)) { res.writeHead(403); res.end(); return; }
    try { res.setHeader("Content-Type", file.endsWith(".webp") ? "image/webp" : file.endsWith(".png") ? "image/png" : "image/svg+xml"); res.end(readFileSync(file)); }
    catch { res.writeHead(404); res.end(); }
  });
  // --port=N pins the preview's port for a launch configuration (default: any free port).
  const port = Number(process.argv.find(arg => arg.startsWith("--port="))?.slice("--port=".length)) || 0;
  server.listen(port, "127.0.0.1", () => console.log(`Preview: http://127.0.0.1:${server.address().port}`));
}
