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
const account = { id: "stable-account-id", corporation_name: "Aurora" };
response = { ok: true, status: 200, json: async () => account };
assert.deepEqual(await accounts.register("pilot", "a very long passphrase", "Aurora"), account);
assert.equal(requests.at(-1).url, "/api/account/register");
for (const [key, value] of Object.entries({ credentials: "same-origin", redirect: "error", cache: "no-store", method: "POST" })) {
  assert.equal(requests.at(-1).options[key], value);
}
assert.equal(requests.at(-1).options.headers["X-Stellar-Client"], "1");
assert.deepEqual(JSON.parse(requests.at(-1).options.body), { login: "pilot", password: "a very long passphrase", corporation_name: "Aurora" });
await accounts.signIn("pilot", "a very long passphrase");
assert.equal(requests.at(-1).url, "/api/account/login");
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
  value = ""; dataset = {}; disabled = false; hidden = false; textContent = ""; listeners = new Map();
  addEventListener(type, handler) { this.listeners.set(type, handler); }
  async fire(type) { return this.listeners.get(type)?.({ preventDefault() {}, currentTarget: this }); }
}
for (const prefix of ["deck", "m"]) {
  const login = new Element(), password = new Element(), corporation = new Element();
  const submit = new Element(), toggle = new Element(), heading = new Element(), error = new Element(), signOut = new Element();
  const form = new Element();
  const selectors = { "[name=username]": login, "[name=password]": password, "[name=corporation]": corporation,
    "[type=submit]": submit, "[data-account-toggle]": toggle, h1: heading };
  form.querySelector = selector => selectors[selector];
  form.querySelectorAll = () => [corporation];
  const ids = { [`${prefix}-join-form`]: form, [`${prefix}-join-error`]: error, [`${prefix}-sign-out`]: signOut };
  const sent = []; let reloads = 0, disconnected = 0, finish;
  const api = { signIn: (...args) => { sent.push(["signIn", ...args]); return new Promise(resolve => { finish = resolve; }); },
    register: async (...args) => { sent.push(["register", ...args]); return account; }, signOut: async () => {} };
  const module = compile("shell/account.ts", { document: { getElementById: id => ids[id] }, location: { reload() { reloads++; } } }, { "../auth": { accounts: api } });
  const abort = new AbortController();
  module.bindAccountForm(prefix, { net: { disconnect() { disconnected++; } } }, abort.signal);
  const markup = module.accountFields(prefix);
  assert.match(markup, /type="password"/);
  assert.match(markup, /autocomplete="current-password"/);
  assert.match(markup, /data-register-field hidden/);
  login.value = "pilot"; password.value = "correct password";
  const inFlight = form.fire("submit");
  await form.fire("submit");
  assert.equal(sent.length, 1, "double submit cannot create parallel logins");
  assert.ok(submit.disabled && toggle.disabled);
  finish(account); await inFlight;
  assert.equal(password.value, "", "password is cleared even after success");
  assert.equal(reloads, 1, "account switch drops previous private view caches");
  await toggle.fire("click");
  assert.equal(corporation.required, true); assert.equal(corporation.hidden, false);
  assert.equal(password.autocomplete, "new-password");
  assert.equal(password.minLength, 15);
  password.value = "another long passphrase"; corporation.value = "Aurora";
  await form.fire("submit");
  assert.deepEqual(sent.at(-1), ["register", "pilot", "another long passphrase", "Aurora"]);
  api.register = async () => { throw new Error("Registration refused"); };
  password.value = "bad registration password";
  await form.fire("submit");
  assert.match(error.textContent, /Registration refused/);
  assert.equal(password.value, ""); assert.equal(submit.disabled, false);
  password.value = "forgotten unsent password"; abort.abort();
  assert.equal(password.value, "", "shell teardown clears credentials");
  await signOut.fire("click");
  assert.equal(reloads, 3); assert.equal(disconnected, 3);
}
console.log("PASS: same-origin account API, sign-in/registration on both shells, double-submit guard, errors, password clearing, logout/cache reset.");
