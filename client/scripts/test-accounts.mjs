import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

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
  value = ""; dataset = {}; disabled = false; hidden = false; textContent = ""; listeners = new Map(); open = false;
  addEventListener(type, handler) { this.listeners.set(type, handler); }
  async fire(type, detail = {}) { return this.listeners.get(type)?.({ preventDefault() {}, stopPropagation() {}, currentTarget: this, ...detail }); }
  focus() { this.focused = true; }
  showModal() { this.open = true; }
  close() { this.open = false; void this.fire("close"); }
}
function profileFixture(prefix, ids) {
  const ui = Object.fromEntries(["dialog", "open", "form", "login", "password", "confirm", "submit", "status", "error", "leave", "signOut", "leaveConfirm", "close", "registerFirst", "corporation"].map(key => [key, new Element()]));
  ui.form.querySelector = selector => ({ "[name=username]": ui.login, "[name=password]": ui.password,
    "[name=confirm-password]": ui.confirm, "[type=submit]": ui.submit })[selector];
  ui.dialog.querySelector = selector => ({ "[data-profile-register]": ui.form, "[data-profile-status]": ui.status,
    "[data-profile-error]": ui.error, "[data-profile-leave]": ui.leave, "[data-profile-leave-confirm]": ui.leaveConfirm,
    "[data-profile-close]": ui.close, "[data-profile-register-first]": ui.registerFirst, "[data-profile-corporation]": ui.corporation })[selector];
  Object.assign(ids, { [`${prefix}-profile`]: ui.dialog, [`${prefix}-profile-open`]: ui.open, [`${prefix}-sign-out`]: ui.signOut });
  return ui;
}
for (const prefix of ["deck", "m"]) {
  const login = new Element(), password = new Element(), corporation = new Element();
  const submit = new Element(), toggle = new Element(), guest = new Element(), heading = new Element(), error = new Element();
  const form = new Element();
  const selectors = { "[name=username]": login, "[name=password]": password, "[name=corporation]": corporation,
    "[type=submit]": submit, "[data-account-toggle]": toggle, "[data-account-guest]": guest, h1: heading };
  form.querySelector = selector => selectors[selector];
  form.querySelectorAll = () => [corporation];
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
  assert.match(markup, /Use at least 8 characters\./);
  assert.match(markup, /type="button" data-account-guest>Guest sign-in/);
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
  assert.equal(sent.length, 1, "double submit cannot create parallel logins");
  assert.ok(submit.disabled && toggle.disabled && guest.disabled);
  finish(account); await inFlight;
  assert.equal(password.value, "", "password is cleared even after success");
  assert.equal(reloads, 1, "account switch drops previous private view caches");
  await toggle.fire("click");
  assert.equal(corporation.required, true); assert.equal(corporation.hidden, false);
  assert.equal(password.autocomplete, "new-password");
  assert.equal(password.minLength, 8);
  password.value = "orbital8"; corporation.value = "Aurora";
  await form.fire("submit");
  assert.deepEqual(sent.at(-1), ["register", "pilot", "orbital8", "Aurora"]);
  api.register = async () => { throw new Error("Registration refused"); };
  password.value = "bad registration password";
  await form.fire("submit");
  assert.match(error.textContent, /Registration refused/);
  assert.equal(password.value, ""); assert.equal(submit.disabled, false);
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
  await profile.signOut.fire("click");
  assert.equal(reloads, 4); assert.equal(disconnected, 4);
  password.value = "forgotten unsent password"; abort.abort();
  assert.equal(password.value, "", "shell teardown clears credentials");
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
  const claim = ui.form.fire("submit");
  await ui.form.fire("submit");
  await ui.signOut.fire("click");
  assert.equal(claims, 1); assert.equal(logouts, 0);
  assert.ok(ui.submit.disabled && ui.signOut.disabled);
  finish(account); await claim;
  assert.equal(reloads, 1); assert.equal(disconnected, 1);
  assert.equal(ui.password.value, ""); assert.equal(ui.confirm.value, "");
  api.completeRegistration = async () => { throw new Error("That username is unavailable"); };
  ui.password.value = ui.confirm.value = "orbital8";
  await ui.form.fire("submit");
  assert.match(ui.error.textContent, /unavailable/); assert.equal(reloads, 1);
  assert.equal(ui.submit.disabled, false); assert.equal(ui.password.value, "");
  let stopped = false;
  await ui.dialog.fire("keydown", { stopPropagation() { stopped = true; } });
  assert.ok(stopped, "profile keystrokes cannot confirm map orders behind the dialog");
  ui.password.value = ui.confirm.value = "clear on close";
  await ui.close.fire("click");
  assert.equal(ui.dialog.open, false); assert.equal(ui.password.value, ""); assert.equal(ui.confirm.value, "");
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
}
console.log("PASS: guest entry, secure same-origin API, registration in Profile on both shells, no parallel auth writes, guest logout warning, modal key isolation, password clearing and session reloads.");
