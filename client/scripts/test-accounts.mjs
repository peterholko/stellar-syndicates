import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import sharp from "sharp";

function compile(path, context = {}, dependencies = {}) {
  const exports = {};
  const source = readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
  vm.runInNewContext(ts.transpileModule(source, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, Error, require: id => dependencies[id], ...context });
  return exports;
}

const requests = [];
let response;
const { accounts } = compile("auth.ts", { fetch: async (url, options) => { requests.push({ url, options }); return response; } });
const account = { id: "stable-account-id", corporation_name: "Aurora", is_guest: false };
response = { ok: true, status: 200, json: async () => account };
assert.deepEqual(JSON.parse(JSON.stringify(await accounts.register("pilot", "a very long passphrase", "Aurora"))), account);
assert.equal(requests.at(-1).url, "/api/account/register");
for (const [key, value] of Object.entries({ credentials: "same-origin", redirect: "error", cache: "no-store", method: "POST" })) {
  assert.equal(requests.at(-1).options[key], value);
}
assert.equal(requests.at(-1).options.headers["X-Stellar-Client"], "1");
assert.deepEqual(JSON.parse(requests.at(-1).options.body), { login: "pilot", password: "a very long passphrase", corporation_name: "Aurora" });
await accounts.signIn("pilot", "a very long passphrase");
assert.equal(requests.at(-1).url, "/api/account/login");
await accounts.guest();
assert.equal(requests.at(-1).url, "/api/account/guest");
assert.deepEqual(JSON.parse(requests.at(-1).options.body), {});
await accounts.completeRegistration("pilot", "orbital8");
assert.equal(requests.at(-1).url, "/api/account/complete-registration");
assert.deepEqual(JSON.parse(requests.at(-1).options.body), { login: "pilot", password: "orbital8" });
assert.equal(requests.at(-1).options.credentials, "same-origin");
assert.equal(requests.at(-1).options.redirect, "error");
await accounts.session();
assert.equal(requests.at(-1).options.method, "GET");
response = { ok: false, status: 401, json: async () => ({ error: "Sign-in failed." }) };
assert.equal(await accounts.session(), null);
await assert.rejects(accounts.signIn("pilot", "wrong password"), /Sign-in failed/);
response = { ok: true, status: 204 };
await accounts.signOut();
assert.equal(requests.at(-1).url, "/api/account/logout");
assert.equal(requests.at(-1).options.method, "POST");

class Element {
  value = ""; dataset = {}; attributes = {}; disabled = false; hidden = false; checked = false; textContent = ""; listeners = new Map(); open = false;
  addEventListener(type, handler) { this.listeners.set(type, handler); }
  setAttribute(name, value) { this.attributes[name] = value; }
  async fire(type, detail = {}) { return this.listeners.get(type)?.({ preventDefault() {}, stopPropagation() {}, currentTarget: this, ...detail }); }
  focus() { this.focused = true; }
  showModal() { this.open = true; this.modalOpens = (this.modalOpens ?? 0) + 1; }
  close() { if (!this.open) return; this.open = false; void this.fire("close"); }
}
function charterFixture() {
  const accept = new Element(), toggle = new Element(), terms = new Element(), close = new Element(), body = new Element();
  toggle.textContent = "Read terms";
  terms.querySelector = selector => ({ "[data-charter-close]": close, "[data-charter-body]": body })[selector];
  return { accept, toggle, terms, close, body, selectors: {
    "[name=charter-agreement]": accept, "[data-charter-open]": toggle, "[data-charter-terms]": terms,
  } };
}
function assertCharterMarkup(markup, prefix) {
  assert.match(markup, new RegExp(`id="${prefix}-charter-accept" name="charter-agreement" type="checkbox" required`));
  assert.doesNotMatch(markup, /type="checkbox"[^>]*\schecked\b/, "agreement is never preselected");
  assert.match(markup, new RegExp(`aria-expanded="false" aria-controls="${prefix}-charter-terms"`));
  assert.match(markup, /data-charter-open aria-haspopup="dialog"/);
  assert.match(markup, new RegExp(`<dialog id="${prefix}-charter-terms"[^>]*data-charter-terms aria-labelledby="${prefix}-charter-title"`));
  assert.doesNotMatch(markup, /<dialog[^>]*\sopen\b/, "terms remain closed until requested");
  assert.match(markup, /type="button" data-charter-close aria-label="Close charter terms" autofocus/);
  assert.doesNotMatch(markup, /Fictional game lore|real-world legal agreement|account-charter__fiction/);
  assert.match(markup, /The Authority owns the charter—and everyone who sails under it\./);
}
function profileFixture(prefix, ids) {
  const ui = Object.fromEntries(["dialog", "open", "form", "login", "password", "confirm", "submit", "status", "error", "leave", "signOut", "leaveConfirm", "close", "registerFirst", "corporation"].map(key => [key, new Element()]));
  ui.charter = charterFixture();
  ui.form.querySelector = selector => ({ "[name=username]": ui.login, "[name=password]": ui.password,
    "[name=confirm-password]": ui.confirm, "[type=submit]": ui.submit, ...ui.charter.selectors })[selector];
  ui.dialog.querySelector = selector => ({ "[data-profile-register]": ui.form, "[data-profile-status]": ui.status,
    "[data-profile-error]": ui.error, "[data-profile-leave]": ui.leave, "[data-profile-leave-confirm]": ui.leaveConfirm,
    "[data-profile-close]": ui.close, "[data-profile-register-first]": ui.registerFirst, "[data-profile-corporation]": ui.corporation })[selector];
  Object.assign(ids, { [`${prefix}-profile`]: ui.dialog, [`${prefix}-profile-open`]: ui.open, [`${prefix}-sign-out`]: ui.signOut });
  return ui;
}
for (const prefix of ["deck", "m"]) {
  const login = new Element(), password = new Element(), corporation = new Element();
  const submit = new Element(), toggle = new Element(), guest = new Element(), heading = new Element(), intro = new Element(), error = new Element();
  const registerFields = Array.from({ length: 3 }, () => new Element());
  const charter = charterFixture();
  const form = new Element();
  const selectors = { "[name=username]": login, "[name=password]": password, "[name=corporation]": corporation,
    "[type=submit]": submit, "[data-account-toggle]": toggle, "[data-account-guest]": guest,
    h1: heading, "[data-account-intro]": intro, ...charter.selectors };
  form.querySelector = selector => selectors[selector];
  form.querySelectorAll = () => registerFields;
  const ids = { [`${prefix}-join-form`]: form, [`${prefix}-join-error`]: error };
  const profile = profileFixture(prefix, ids);
  const sent = []; let reloads = 0, disconnected = 0, finish;
  const api = { signIn: (...args) => { sent.push(["signIn", ...args]); return new Promise(resolve => { finish = resolve; }); },
    register: async (...args) => { sent.push(["register", ...args]); return account; }, signOut: async () => {}, session: async () => account,
    guest: () => { sent.push(["guest"]); return new Promise(resolve => { finish = resolve; }); } };
  const module = compile("shell/account.ts", { document: { getElementById: id => ids[id] }, location: { reload() { reloads++; } } }, { "../auth": { accounts: api } });
  const abort = new AbortController();
  module.bindAccountForm(prefix, { net: { disconnect() { disconnected++; } } }, abort.signal);
  const markup = module.accountFields(prefix);
  assert.match(markup, /type="password"/);
  assert.match(markup, /autocomplete="current-password"/);
  assert.match(markup, /data-register-field hidden/);
  assert.match(markup, /8\+ characters/);
  assert.match(markup, /type="button" data-account-guest[^>]*>Continue as guest/);
  assert.match(markup, /Terran Charter Authority/);
  assert.match(markup, /src="\/art\/derived\/authority-v2\/terran-charter-authority-seal-192\.webp"/);
  for (const [index, size] of [96, 192, 288, 384].entries()) {
    assert.ok(markup.includes(`/art/derived/authority-v2/terran-charter-authority-seal-${size}.webp ${index + 1}x`), "Authority seal supports high-density displays");
  }
  assert.match(markup, /alt="Terran Charter Authority seal" width="96" height="96"/);
  assert.match(markup, /<h1[^>]*>Charter Command<\/h1>/);
  assert.match(markup, /class="account-lore" data-account-intro>Access your charter’s command center\./);
  assert.match(markup, /type="submit">Enter Command Center<\/button>/);
  assert.match(markup, /data-account-toggle>Request Stellar Charter<\/button>/);
  assert.doesNotMatch(markup, /Register corporation/);
  assert.match(markup, new RegExp(`<label for="${prefix}-login">Universal Identifier</label>`));
  assert.match(markup, new RegExp(`<label for="${prefix}-password">Command passphrase</label>`));
  assert.match(markup, new RegExp(`aria-describedby="${prefix}-login-hint"`));
  assert.match(markup, new RegExp(`id="${prefix}-login-hint">Username or email`));
  assert.match(markup, /name="username"[^>]*autocomplete="username"/);
  assert.match(markup, /Guest access: 30 days\./);
  assert.doesNotMatch(markup, /<aside|<legend|data-corporation-preview|Office of Stellar Charters|Application draft|lasting legacy/);
  assertCharterMarkup(markup, prefix);
  const collapsedMarkup = markup.replace(/<dialog\b[^>]*data-charter-terms[\s\S]*?<\/dialog>/, "");
  assert.ok(collapsedMarkup.replace(/<[^>]*>/g, " ").trim().split(/\s+/).length <= 50, "collapsed charter adds only one compact agreement row");
  assert.ok(markup.indexOf('name="corporation"') < markup.indexOf('name="username"'), "corporation identity comes first in the form and keyboard order");
  assert.match(markup, new RegExp(`aria-describedby="${prefix}-password-hint"`));
  assert.equal(form.dataset.accountMode, "signin");
  assert.equal(heading.textContent, "Charter Command");
  assert.equal(intro.textContent, "Access your charter’s command center.");
  assert.equal(submit.textContent, "Enter Command Center");
  assert.equal(toggle.textContent, "Request Stellar Charter");
  assert.equal(corporation.disabled, true, "hidden corporation name cannot block sign-in validation");
  assert.ok(registerFields.every(field => field.hidden));
  assert.equal(charter.accept.disabled, true);
  assert.equal(charter.accept.required, false, "hidden agreement never blocks sign-in");
  assert.equal(charter.accept.checked, false);
  assert.equal(charter.terms.open, false);
  assert.equal(submit.disabled, false);
  const backdrop = module.accountBackdrop(prefix);
  assert.ok(backdrop.includes(`class="${prefix}-join__art"`));
  assert.match(backdrop, /aria-hidden="true"/);
  assert.match(backdrop, /src="\/art\/derived\/login\/login-market-horizon-v1\.webp"/);
  assert.match(backdrop, /alt=""/);
  assert.match(backdrop, /fetchpriority="high"/);
  login.value = "pilot"; password.value = "correct password";
  const inFlight = form.fire("submit");
  await form.fire("submit");
  await guest.fire("click");
  await toggle.fire("click");
  assert.equal(form.dataset.accountMode, "signin", "cannot change form mode during authentication");
  assert.equal(sent.length, 1, "double submit cannot create parallel logins");
  assert.ok(submit.disabled && toggle.disabled && guest.disabled);
  finish(account); await inFlight;
  assert.equal(password.value, "", "password is cleared even after success");
  assert.equal(reloads, 1, "account switch drops previous private view caches");
  await toggle.fire("click");
  assert.equal(corporation.required, true); assert.equal(corporation.disabled, false);
  assert.ok(registerFields.every(field => !field.hidden));
  assert.equal(form.dataset.accountMode, "register");
  assert.equal(heading.textContent, "Stellar charter");
  assert.equal(intro.textContent, "Beyond the wormhole, a new world awaits your corporation.");
  assert.equal(submit.textContent, "Request Stellar Charter");
  assert.equal(toggle.textContent, "Sign in");
  assert.ok(corporation.focused, "registration focuses the corporation name first");
  assert.equal(password.autocomplete, "new-password");
  assert.equal(password.minLength, 8);
  password.value = "orbital8"; corporation.value = "Aurora";
  assert.equal(submit.disabled, true, "registration requires an explicit agreement");
  assert.equal(charter.accept.required, true);
  assert.equal(charter.accept.disabled, false);
  await form.fire("submit");
  assert.equal(sent.length, 1, "direct submission also refuses an unchecked agreement");
  assert.match(error.textContent, /Accept the Authority charter/);
  assert.equal(charter.accept.focused, true);
  charter.body.scrollTop = 250;
  await charter.toggle.fire("click");
  assert.equal(charter.terms.open, true);
  assert.equal(charter.body.scrollTop, 0);
  assert.equal(charter.toggle.attributes["aria-expanded"], "true");
  assert.equal(charter.toggle.textContent, "Read terms");
  assert.equal(charter.accept.checked, false, "reading the terms never accepts them");
  await charter.toggle.fire("click");
  assert.equal(charter.terms.modalOpens, 1, "repeated open cannot create multiple modal layers");
  let termsKeyStopped = false;
  await charter.terms.fire("keydown", { stopPropagation() { termsKeyStopped = true; } });
  assert.equal(termsKeyStopped, true, "modal keys do not reach the registration form or map");
  await charter.close.fire("click");
  assert.equal(charter.terms.open, false);
  assert.equal(charter.toggle.attributes["aria-expanded"], "false");
  assert.equal(charter.toggle.textContent, "Read terms");
  assert.equal(corporation.value, "Aurora");
  assert.equal(login.value, "pilot");
  assert.equal(password.value, "orbital8", "closing the terms preserves the registration draft");
  charter.accept.checked = true;
  await charter.accept.fire("change");
  assert.equal(submit.disabled, false);
  assert.equal(error.textContent, "");
  assert.equal(charter.terms.open, false, "accepting does not force the lore open");
  await charter.toggle.fire("click");
  charter.terms.close(); // Native Escape closes the dialog through this same event.
  assert.equal(charter.toggle.attributes["aria-expanded"], "false");
  assert.equal(charter.accept.checked, true, "dismissing terms does not reset acknowledgement");
  charter.accept.checked = false;
  await charter.accept.fire("change");
  assert.equal(submit.disabled, true, "unchecking removes permission to register");
  charter.accept.checked = true;
  await charter.accept.fire("change");
  await form.fire("submit");
  assert.deepEqual(sent.at(-1), ["register", "pilot", "orbital8", "Aurora"]);
  api.register = async () => { throw new Error("Registration refused"); };
  password.value = "bad registration password";
  await form.fire("submit");
  assert.match(error.textContent, /Registration refused/);
  assert.equal(password.value, ""); assert.equal(submit.disabled, false);
  await toggle.fire("click");
  assert.equal(heading.textContent, "Charter Command");
  assert.equal(intro.textContent, "Access your charter’s command center.");
  assert.equal(submit.textContent, "Enter Command Center");
  assert.equal(toggle.textContent, "Request Stellar Charter");
  assert.equal(corporation.disabled, true); assert.equal(corporation.required, false);
  assert.ok(registerFields.every(field => field.hidden));
  assert.equal(password.autocomplete, "current-password"); assert.equal(password.minLength, 1);
  assert.equal(charter.accept.checked, false);
  assert.equal(charter.accept.disabled, true);
  assert.equal(charter.terms.open, false);
  assert.ok(login.focused); assert.equal(error.textContent, "");
  await toggle.fire("click");
  assert.equal(corporation.value, "Aurora", "switching modes preserves the corporation-name draft");
  assert.equal(submit.disabled, true, "returning to registration requires fresh acknowledgement");
  login.value = password.value = "";
  const guestRequest = guest.fire("click");
  await guest.fire("click");
  await form.fire("submit");
  assert.deepEqual(sent.at(-1), ["guest"], "guest does not depend on credentials or form validation");
  assert.equal(sent.filter(call => call[0] === "guest").length, 1);
  finish({ ...account, is_guest: true }); await guestRequest;
  assert.equal(reloads, 3);
  api.guest = async () => { throw new Error("Guest entry unavailable"); };
  await guest.fire("click");
  assert.match(error.textContent, /Guest entry unavailable/);
  assert.equal(guest.disabled, false);
  assert.equal(submit.disabled, true, "failed guest entry cannot bypass registration acknowledgement");
  await profile.signOut.fire("click");
  assert.equal(reloads, 4); assert.equal(disconnected, 4);
  await charter.toggle.fire("click");
  password.value = "forgotten unsent password"; abort.abort();
  assert.equal(password.value, "", "shell teardown clears credentials");
  assert.equal(charter.terms.open, false, "shell teardown closes the modal layer");
}

for (const prefix of ["deck", "m"]) {
  const ids = {}, ui = profileFixture(prefix, ids);
  let current = { ...account, is_guest: true }, finish, claims = 0, logouts = 0, reloads = 0, disconnected = 0;
  const api = { session: async () => current,
    completeRegistration: (...args) => { claims++; assert.deepEqual(args, ["newpilot", "orbital8"]); return new Promise(resolve => { finish = resolve; }); },
    signOut: async () => { logouts++; } };
  const module = compile("shell/account.ts", { document: { getElementById: id => ids[id] }, location: { reload() { reloads++; } } }, { "../auth": { accounts: api } });
  const abort = new AbortController();
  module.bindAccountProfile(prefix, { net: { disconnect() { disconnected++; } } }, abort.signal);
  await ui.open.fire("click");
  assert.equal(ui.dialog.open, true); assert.equal(ui.form.hidden, false);
  assert.equal(ui.charter.accept.checked, false);
  assert.equal(ui.charter.terms.open, false);
  assert.equal(ui.submit.disabled, true);
  assert.equal(ui.open.textContent, "Profile · Guest");
  assert.match(ui.status.textContent, /30 days/);
  await ui.signOut.fire("click");
  assert.equal(logouts, 0, "a guest must confirm losing browser-only access before logout");
  assert.equal(ui.leave.hidden, false);
  await ui.registerFirst.fire("click");
  assert.equal(ui.leave.hidden, true); assert.equal(ui.login.focused, true);
  ui.login.value = "newpilot"; ui.password.value = "orbital8"; ui.confirm.value = "different";
  await ui.form.fire("submit");
  assert.equal(claims, 0); assert.match(ui.error.textContent, /do not match/);
  ui.confirm.value = "orbital8";
  await ui.form.fire("submit");
  assert.equal(claims, 0, "guest completion also requires explicit charter agreement");
  assert.match(ui.error.textContent, /Accept the Authority charter/);
  ui.charter.accept.checked = true;
  await ui.charter.accept.fire("change");
  assert.equal(ui.submit.disabled, false);
  assert.equal(ui.error.textContent, "");
  const claim = ui.form.fire("submit");
  await ui.form.fire("submit");
  await ui.signOut.fire("click");
  assert.equal(claims, 1); assert.equal(logouts, 0);
  assert.ok(ui.submit.disabled && ui.signOut.disabled);
  assert.equal(ui.charter.accept.disabled, true);
  finish(account); await claim;
  assert.equal(reloads, 1); assert.equal(disconnected, 1);
  assert.equal(ui.password.value, ""); assert.equal(ui.confirm.value, "");
  assert.equal(ui.charter.accept.disabled, false);
  api.completeRegistration = async () => { throw new Error("That username is unavailable"); };
  ui.password.value = ui.confirm.value = "orbital8";
  await ui.form.fire("submit");
  assert.match(ui.error.textContent, /unavailable/); assert.equal(reloads, 1);
  assert.equal(ui.submit.disabled, false); assert.equal(ui.password.value, "");
  let stopped = false;
  await ui.dialog.fire("keydown", { stopPropagation() { stopped = true; } });
  assert.ok(stopped, "profile keystrokes cannot confirm map orders behind the dialog");
  ui.password.value = ui.confirm.value = "clear on close";
  await ui.charter.toggle.fire("click");
  assert.equal(ui.charter.terms.open, true);
  await ui.charter.close.fire("click");
  assert.equal(ui.charter.terms.open, false);
  assert.equal(ui.dialog.open, true, "closing terms never closes the underlying Profile");
  assert.equal(ui.password.value, "clear on close");
  assert.equal(ui.charter.accept.checked, true);
  await ui.charter.toggle.fire("click");
  await ui.close.fire("click");
  assert.equal(ui.dialog.open, false); assert.equal(ui.password.value, ""); assert.equal(ui.confirm.value, "");
  assert.equal(ui.charter.accept.checked, false);
  assert.equal(ui.charter.terms.open, false, "closing Profile also dismisses its terms modal");
  assert.equal(ui.submit.disabled, true);
  await ui.open.fire("click"); await ui.signOut.fire("click"); await ui.leaveConfirm.fire("click");
  assert.equal(logouts, 1); assert.equal(reloads, 2);
  ui.dialog.close(); current = account;
  await ui.open.fire("click");
  assert.equal(ui.form.hidden, true); assert.equal(ui.status.textContent, "Registered account");
  await ui.signOut.fire("click");
  assert.equal(logouts, 2, "registered players can sign out normally");
  ui.password.value = ui.confirm.value = "clear on teardown"; abort.abort();
  assert.equal(ui.password.value, ""); assert.equal(ui.confirm.value, "");
  assert.equal(ui.dialog.open, false);
  const markup = module.accountProfile(prefix);
  assert.match(markup, /<dialog/); assert.match(markup, /autocomplete="new-password"/);
  assert.match(markup, /Complete registration/);
  assert.match(markup, new RegExp(`<label for="${prefix}-profile-login">Universal Identifier</label>`));
  assert.match(markup, new RegExp(`<label for="${prefix}-profile-password">Command passphrase</label>`));
  assert.match(markup, new RegExp(`<label for="${prefix}-profile-confirm">Confirm passphrase</label>`));
  assert.match(markup, new RegExp(`aria-describedby="${prefix}-profile-login-hint"`));
  assertCharterMarkup(markup, `${prefix}-profile`);
}
for (const size of [96, 192, 288, 384]) {
  const seal = sharp(readFileSync(new URL(`../public/art/derived/authority-v2/terran-charter-authority-seal-${size}.webp`, import.meta.url)));
  const meta = await seal.metadata();
  assert.equal(meta.width, size);
  assert.equal(meta.height, size);
  assert.equal(meta.hasAlpha, true, "Authority emblem must retain its transparent background");
  const { data, info } = await seal.ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  const alphaAt = (x, y) => data[(y * info.width + x) * info.channels + info.channels - 1];
  for (const [x, y] of [[0, 0], [size - 1, size - 1], [Math.round(size * .39), Math.round(size * .49)]]) {
    assert.equal(alphaAt(x, y), 0, "outside and inside the orbital ring remain transparent");
  }
  assert.ok((await seal.stats()).channels[3].max > 200, "the emblem itself is visible");
}
const accountCss = readFileSync(new URL("../src/styles/account.css", import.meta.url), "utf8");
const introCss = readFileSync(new URL("../src/styles/settlement.css", import.meta.url), "utf8");
function declarations(css, selector) {
  const block = css.split(`${selector} {`)[1]?.split("}")[0];
  assert.ok(block, `style block exists: ${selector}`);
  return Object.fromEntries(block.split(";").filter(part => part.trim()).map(part => part.trim().split(":").map(value => value.trim())));
}
const accountPanel = declarations(accountCss, ".account-card");
const desktopSeal = declarations(accountCss, ".account-authority__seal");
const mobileSeal = declarations(accountCss.split("@media (max-width: 700px)")[1], ".account-authority__seal");
for (const dimension of ["width", "height"]) {
  assert.equal(desktopSeal[dimension], "96px", "desktop Authority logo is 50% larger than 64px");
  assert.equal(mobileSeal[dimension], "72px", "mobile Authority logo is 50% larger than 48px");
}
assert.equal(desktopSeal["object-fit"], "contain", "larger logo preserves its proportions");
const introPanel = declarations(introCss, ".settlement-dialog");
for (const property of ["background", "color", "border", "border-radius", "box-shadow"]) {
  assert.equal(accountPanel[property], introPanel[property], `account panel matches the introduction: ${property}`);
}
const termsModal = declarations(accountCss, "dialog.account-charter__terms");
assert.equal(termsModal.position, "fixed", "terms never participate in registration card layout");
assert.equal(termsModal.background, introPanel.background);
assert.equal(declarations(accountCss, ".account-charter__body")["overflow-y"], "auto", "long terms scroll below a persistent close button");
const accountPrimary = declarations(accountCss, ".account-card .account-primary");
const introPrimary = declarations(introCss, ".settlement-dialog .settlement-primary");
for (const property of ["background", "color"]) assert.equal(accountPrimary[property], introPrimary[property]);
const accountButton = declarations(accountCss, "form.account-card button");
const introButton = declarations(introCss, ".settlement-dialog button");
for (const property of ["background", "color", "border", "border-radius", "padding", "min-height"]) {
  assert.equal(accountButton[property], introButton[property], `account controls match the introduction: ${property}`);
}
assert.equal(declarations(accountCss, ".account-card .account-alternatives button")["font-size"], "12px", "long charter-request label keeps compact sizing above the shared button font rule");
assert.doesNotMatch(accountCss, /\.account-card\s+\.account-switch(?:\s|:)/, "Request Stellar Charter inherits the same filled button and hover styles as guest entry");
assert.equal(declarations(accountCss, '.account-card[data-account-mode="register"] .account-switch').background, "transparent", "the return-to-sign-in link keeps its existing appearance");
console.log("PASS: Charter Command sign-in and charter-request wording, charter modal opening/closing, unchanged form drafts and acknowledgement, modal cleanup and explicit registration agreement, Authority seal and themed credentials, matching intro palette, both shells, guest entry and Profile registration, secure API, password clearing and session reloads.");
