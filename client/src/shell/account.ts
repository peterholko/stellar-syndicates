import "../styles/account.css";
import { accounts, NO_SIGNAL, type Account } from "../auth";
import { ALERT_KINDS, notifications, type NotifyPrefs } from "../notifications";
import { fmtBuildDur } from "../core/derive/format";
import { liveSimTime } from "../state";
import type { CoreContext } from "./types";

const CHARTER_REQUIRED = "Accept the Authority charter to register.";

// An in-universe onboarding acknowledgement, not legal terms or a new account
// permission. Keep the fiction separate from the real authentication contract.
function charterAgreement(prefix: string, disabled = false): string {
  return `<div class="account-charter">
    <div class="account-charter__controls">
      <label class="account-charter__accept" for="${prefix}-charter-accept">
        <input id="${prefix}-charter-accept" name="charter-agreement" type="checkbox" required${disabled ? " disabled" : ""}>
        <span>I accept the Authority charter.</span>
      </label>
      <button class="account-charter__read" type="button" data-charter-open aria-haspopup="dialog" aria-expanded="false" aria-controls="${prefix}-charter-terms">Read terms</button>
    </div>
    <dialog id="${prefix}-charter-terms" class="account-charter__terms" data-charter-terms aria-labelledby="${prefix}-charter-title">
      <header>
        <div><small>Terran Charter Authority</small><h2 id="${prefix}-charter-title">Articles of obligation</h2></div>
        <button class="account-charter__close" type="button" data-charter-close aria-label="Close charter terms" autofocus>×</button>
      </header>
      <div class="account-charter__body" data-charter-body>
      <p class="account-charter__edict">Settlement is a privilege. Sovereignty is not included.</p>
      <ol>
        <li><strong>Prior claim.</strong> Your chartered worlds, vessels, discoveries and profits remain subject to the Authority’s supreme claim. You administer them. You do not possess them.</li>
        <li><strong>Service without reservation.</strong> The Authority may demand cargo, ships or service. Compliance is not negotiable. Your distance from Terra does not diminish your debt.</li>
        <li><strong>No right of refusal.</strong> Inspection, requisition and removal from command require no further consent. Your signature is sufficient. Your objections are not.</li>
        <li><strong>Default and forfeiture.</strong> Defiance voids your charter. Assets may be seized without compensation. Dissolving your corporation does not discharge its obligations.</li>
      </ol>
      <p class="account-charter__edict">You may command the expedition. The Authority owns the charter—and everyone who sails under it.</p>
      </div>
    </dialog>
  </div>`;
}

function bindCharterAgreement(form: HTMLFormElement, signal: AbortSignal, changed: () => void) {
  const accept = form.querySelector<HTMLInputElement>("[name=charter-agreement]")!;
  const read = form.querySelector<HTMLButtonElement>("[data-charter-open]")!;
  const terms = form.querySelector<HTMLDialogElement>("[data-charter-terms]")!;
  const body = terms.querySelector<HTMLElement>("[data-charter-body]")!;
  function closeTerms(): void {
    if (terms.open) terms.close();
    read.setAttribute("aria-expanded", "false");
  }
  function reset(): void {
    accept.checked = false;
    closeTerms();
  }
  read.addEventListener("click", () => {
    if (terms.open) return;
    body.scrollTop = 0;
    // Native top-layer modal: no form reflow, background inert, focus trapped.
    // Closing via X or Escape restores focus to Read terms without clearing inputs.
    terms.showModal();
    read.setAttribute("aria-expanded", "true");
  }, { signal });
  terms.querySelector("[data-charter-close]")!.addEventListener("click", closeTerms, { signal });
  terms.addEventListener("close", () => read.setAttribute("aria-expanded", "false"), { signal });
  terms.addEventListener("keydown", event => event.stopPropagation(), { signal });
  signal.addEventListener("abort", closeTerms, { once: true });
  accept.addEventListener("change", changed, { signal });
  reset();
  return { accept, reset };
}

// Shared decorative art for sign-in and registration; never intercepts input.
export function accountBackdrop(prefix: string): string {
  return `<div class="${prefix}-join__art" aria-hidden="true">
    <img src="/art/derived/login/login-market-horizon-v1.webp" alt="" width="1672" height="941" decoding="async" fetchpriority="high">
  </div>`;
}

// The game's logo above the sign-in card in both shells (account-stage).
export function accountLogo(): string {
  const src = (width: number) => `/art/derived/login/stellar-syndicates-banner-v1-${width}.webp`;
  return `<img class="account-logo" src="${src(960)}" srcset="${src(480)} 480w, ${src(960)} 960w, ${src(1440)} 1440w" sizes="(max-width: 528px) calc(100vw - 48px), 480px" alt="Stellar Syndicates" width="1879" height="576" decoding="async" fetchpriority="high">`;
}

// The two shells share the same credential workflow. Only account creation asks
// for a public corporation name; knowing that name is never authentication.
export function accountFields(prefix: string): string {
  return `
    <header class="account-heading">
      <div class="account-authority">
        <img class="account-authority__seal" src="/art/derived/authority-v2/terran-charter-authority-seal-192.webp" srcset="/art/derived/authority-v2/terran-charter-authority-seal-96.webp 1x, /art/derived/authority-v2/terran-charter-authority-seal-192.webp 2x, /art/derived/authority-v2/terran-charter-authority-seal-288.webp 3x, /art/derived/authority-v2/terran-charter-authority-seal-384.webp 4x" alt="Terran Authority seal" width="96" height="96">
        <div><small>Terran Authority</small><h1 id="${prefix}-join-title">Charter Command</h1></div>
      </div>
      <p class="account-lore" data-account-intro>Access your charter’s command center.</p>
    </header>
    <div class="account-fields">
      <div class="account-field" data-register-field hidden>
        <label for="${prefix}-name">Corporation name</label>
        <input id="${prefix}-name" name="corporation" maxlength="32" autocomplete="organization" disabled>
      </div>
      <div class="account-field">
        <div class="account-field__label"><label for="${prefix}-login">Universal Identifier</label><small id="${prefix}-login-hint">Username or email</small></div>
        <input id="${prefix}-login" name="username" type="text" required maxlength="254" autocomplete="username" autocapitalize="none" spellcheck="false" aria-describedby="${prefix}-login-hint">
      </div>
      <div class="account-field">
        <div class="account-field__label"><label for="${prefix}-password">Command passphrase</label><small id="${prefix}-password-hint" data-register-field hidden>8+ characters</small></div>
        <input id="${prefix}-password" name="password" type="password" required maxlength="128" autocomplete="current-password" aria-describedby="${prefix}-password-hint">
      </div>
      <div data-register-field hidden>${charterAgreement(prefix, true)}</div>
      <div id="${prefix}-join-error" role="alert" class="account-error"></div>
      <button id="${prefix}-join-button" class="account-primary" type="submit">Enter Command Center</button>
      <div class="account-alternatives">
        <button class="account-switch" type="button" data-account-toggle>Request Stellar Charter</button>
        <button class="account-guest__button" type="button" data-account-guest aria-describedby="${prefix}-guest-hint">Continue as guest</button>
      </div>
      <small id="${prefix}-guest-hint" class="account-guest-hint">Guest access: 30 days.</small>
    </div>`;
}

// §front-door (docs/front-door.md): whether Charter Command, the account
// service, answers. The page loads from Cloudflare even while the game server
// is down; until the service answers, the sign-in card's buttons wait and the
// page asks again every 10 s.
const SIGNAL_RETRY_MS = 10_000;
let signalLost = false;
let signalTimer: ReturnType<typeof setTimeout> | null = null;
let onSignalBack: ((account: Account | null) => void) | null = null;
const signalListeners = new Set<() => void>();

/** No answer from the account service: the sign-in card waits until it
 * answers. `back` then runs with the session the service reports. */
export function loseSignal(back?: (account: Account | null) => void): void {
  if (back) onSignalBack = back;
  if (!signalLost) {
    signalLost = true;
    signalListeners.forEach(listener => listener());
  }
  signalTimer ??= setTimeout(seekSignal, SIGNAL_RETRY_MS);
}

async function seekSignal(): Promise<void> {
  signalTimer = null;
  try {
    const account = await accounts.session();
    signalLost = false;
    signalListeners.forEach(listener => listener());
    const back = onSignalBack;
    onSignalBack = null;
    back?.(account);
  } catch {
    signalTimer = setTimeout(seekSignal, SIGNAL_RETRY_MS);
  }
}

export function bindAccountForm(prefix: string, ctx: CoreContext, signal: AbortSignal): void {
  const form = document.getElementById(`${prefix}-join-form`) as HTMLFormElement;
  const login = form.querySelector<HTMLInputElement>("[name=username]")!;
  const password = form.querySelector<HTMLInputElement>("[name=password]")!;
  const corporation = form.querySelector<HTMLInputElement>("[name=corporation]")!;
  const submit = form.querySelector<HTMLButtonElement>("[type=submit]")!;
  const toggle = form.querySelector<HTMLButtonElement>("[data-account-toggle]")!;
  const guest = form.querySelector<HTMLButtonElement>("[data-account-guest]")!;
  const error = document.getElementById(`${prefix}-join-error`)!;
  signal.addEventListener("abort", () => { password.value = ""; }, { once: true });
  let registering = false;
  const charter = bindCharterAgreement(form, signal, () => {
    settle();
    if (charter.accept.checked && error.textContent === CHARTER_REQUIRED) error.textContent = "";
  });
  /** The buttons wait while a request is out or there is no signal. */
  function settle(): void {
    const busy = form.dataset.busy === "true";
    submit.disabled = busy || signalLost || (registering && !charter.accept.checked);
    toggle.disabled = guest.disabled = busy || signalLost;
    charter.accept.disabled = busy || !registering;
  }
  function presentMode(): void {
    form.dataset.accountMode = registering ? "register" : "signin";
    form.querySelectorAll<HTMLElement>("[data-register-field]").forEach(el => { el.hidden = !registering; });
    corporation.required = registering;
    corporation.disabled = !registering;
    charter.accept.required = registering;
    if (!registering) charter.reset();
    settle();
    password.autocomplete = registering ? "new-password" : "current-password";
    password.minLength = registering ? 8 : 1;
    form.querySelector("h1")!.textContent = registering ? "Stellar charter" : "Charter Command";
    form.querySelector("[data-account-intro]")!.textContent = registering
      ? "Beyond the wormhole, a new world awaits your corporation."
      : "Access your charter’s command center.";
    submit.textContent = registering ? "Request Stellar Charter" : "Enter Command Center";
    toggle.textContent = registering ? "Sign in" : "Request Stellar Charter";
  }
  presentMode();
  toggle.addEventListener("click", () => {
    if (form.dataset.busy === "true") return;
    registering = !registering;
    presentMode();
    error.textContent = "";
    (registering ? corporation : login).focus();
  }, { signal });
  async function authenticate(action: () => Promise<Account>): Promise<void> {
    if (form.dataset.busy === "true") return;
    form.dataset.busy = "true";
    settle();
    error.textContent = "";
    try {
      await action();
      // Reload after authentication so no prior corporation's private view or
      // staged intent survives an account switch. Boot resumes the new cookie.
      ctx.net.disconnect();
      location.reload();
    } catch (failure) {
      if (!signal.aborted) error.textContent = failure instanceof Error ? failure.message : "Unable to sign in.";
      if (failure instanceof Error && failure.message === NO_SIGNAL) loseSignal();
    } finally {
      // Clear the field even when a request fails or the shell changes.
      password.value = "";
      delete form.dataset.busy;
      settle();
    }
  }
  form.addEventListener("submit", event => {
    event.preventDefault();
    if (form.dataset.busy === "true") return;
    if (registering && !charter.accept.checked) {
      error.textContent = CHARTER_REQUIRED;
      charter.accept.focus();
      return;
    }
    return authenticate(() => registering
      ? accounts.register(login.value, password.value, corporation.value)
      : accounts.signIn(login.value, password.value));
  }, { signal });
  // A button, not a form submit: an empty username/password never blocks entry.
  guest.addEventListener("click", () => authenticate(() => accounts.guest()), { signal });
  // Every card, including one a layout switch builds, shows the signal.
  function showSignal(): void {
    if (signalLost) error.textContent = NO_SIGNAL;
    else if (error.textContent === NO_SIGNAL) error.textContent = "";
    settle();
  }
  signalListeners.add(showSignal);
  signal.addEventListener("abort", () => signalListeners.delete(showSignal), { once: true });
  showSignal();
  bindAccountProfile(prefix, ctx, signal);
}

export function accountProfile(prefix: string): string {
  return `<dialog id="${prefix}-profile" class="account-profile" aria-labelledby="${prefix}-profile-title">
    <header><h2 id="${prefix}-profile-title">Profile</h2><button type="button" data-profile-close aria-label="Close profile">×</button></header>
    <p data-profile-corporation></p>
    <p data-profile-status role="status">Loading account…</p>
    <form data-profile-register hidden>
      <p>Register this account. Your corporation and progress stay unchanged.</p>
      <label for="${prefix}-profile-login">Universal Identifier</label>
      <input id="${prefix}-profile-login" name="username" required maxlength="254" autocomplete="username" autocapitalize="none" spellcheck="false" aria-describedby="${prefix}-profile-login-hint">
      <small id="${prefix}-profile-login-hint">Username or email</small>
      <label for="${prefix}-profile-password">Command passphrase</label>
      <input id="${prefix}-profile-password" name="password" type="password" required minlength="8" maxlength="128" autocomplete="new-password" aria-describedby="${prefix}-profile-password-hint">
      <small id="${prefix}-profile-password-hint">Use at least 8 characters.</small>
      <label for="${prefix}-profile-confirm">Confirm passphrase</label>
      <input id="${prefix}-profile-confirm" name="confirm-password" type="password" required minlength="8" maxlength="128" autocomplete="new-password">
      ${charterAgreement(`${prefix}-profile`)}
      <button type="submit" class="account-profile__primary">Complete registration</button>
    </form>
    <section class="account-leave" aria-labelledby="${prefix}-leave-title">
      <h3 id="${prefix}-leave-title">Leave</h3>
      <p data-leave-status></p>
      <button type="button" data-leave-toggle title="Nobody can attack you and you give no orders. Production, freight routes and standing rules keep running.">Take leave</button>
      <p class="account-leave__confirm" data-leave-confirm hidden><span data-leave-confirm-text></span>
        <button type="button" data-leave-yes>Take leave</button><button type="button" data-leave-no>Cancel</button></p>
    </section>
    <section class="account-notify" aria-labelledby="${prefix}-notify-title">
      <h3 id="${prefix}-notify-title">Alerts</h3>
      <p data-notify-status>While you're away.</p>
      <div class="account-notify__kinds">${ALERT_KINDS.map(([slug, label]) => `<label><input type="checkbox" data-notify-kind="${slug}">${label}</label>`).join("")}</div>
      <button type="button" data-notify-push>Push on this device</button>
      <label class="account-notify__digest"><input type="checkbox" data-notify-digest>Daily email digest</label>
      <input type="email" data-notify-email maxlength="254" placeholder="Email for the digest" autocomplete="email" aria-label="Email for the digest">
    </section>
    <p data-profile-error role="alert"></p>
    <div data-profile-leave hidden>
      <p>Signing out loses access to this guest corporation. Register first to keep it.</p>
      <button type="button" data-profile-register-first>Register first</button>
      <button type="button" data-profile-leave-confirm>Sign out anyway</button>
    </div>
    <button id="${prefix}-sign-out" type="button" disabled>Sign out</button>
  </dialog>`;
}

export function bindAccountProfile(prefix: string, ctx: CoreContext, signal: AbortSignal): void {
  const dialog = document.getElementById(`${prefix}-profile`) as HTMLDialogElement;
  const open = document.getElementById(`${prefix}-profile-open`) as HTMLButtonElement;
  const form = dialog.querySelector<HTMLFormElement>("[data-profile-register]")!;
  const login = form.querySelector<HTMLInputElement>("[name=username]")!;
  const password = form.querySelector<HTMLInputElement>("[name=password]")!;
  const confirm = form.querySelector<HTMLInputElement>("[name=confirm-password]")!;
  const submit = form.querySelector<HTMLButtonElement>("[type=submit]")!;
  const status = dialog.querySelector<HTMLElement>("[data-profile-status]")!;
  const error = dialog.querySelector<HTMLElement>("[data-profile-error]")!;
  const leave = dialog.querySelector<HTMLElement>("[data-profile-leave]")!;
  const signOut = document.getElementById(`${prefix}-sign-out`) as HTMLButtonElement;
  const leaveConfirm = dialog.querySelector<HTMLButtonElement>("[data-profile-leave-confirm]")!;
  let account: Account | null = null;
  let busy = false;
  let generation = 0;
  const charter = bindCharterAgreement(form, signal, () => {
    submit.disabled = busy || !charter.accept.checked;
    if (charter.accept.checked && error.textContent === CHARTER_REQUIRED) error.textContent = "";
  });
  submit.disabled = true;
  const clearPasswords = () => { password.value = confirm.value = ""; };
  const setBusy = (value: boolean) => {
    busy = value;
    signOut.disabled = leaveConfirm.disabled = charter.accept.disabled = value;
    submit.disabled = value || !charter.accept.checked;
  };
  // §leave (R4): the corporation's leave, from the served View.
  const leaveStatus = dialog.querySelector<HTMLElement>("[data-leave-status]")!;
  const leaveToggle = dialog.querySelector<HTMLButtonElement>("[data-leave-toggle]")!;
  const leaveConfirm2 = dialog.querySelector<HTMLElement>("[data-leave-confirm]")!;
  const leaveConfirmText = dialog.querySelector<HTMLElement>("[data-leave-confirm-text]")!;
  let profileLabel = "Profile";
  function renderLeave(): void {
    const view = ctx.state?.leave;
    const label = view?.on ? `${profileLabel} · On leave` : profileLabel;
    if (open.textContent !== label) open.textContent = label;
    if (!dialog.open) return;
    if (!view) {
      leaveStatus.textContent = "—";
      leaveToggle.disabled = true;
      return;
    }
    const now = liveSimTime();
    const days = (secs: number) => fmtBuildDur(Math.max(0, secs));
    if (view.on) {
      const left = (view.until ?? now) - now;
      const wait = (view.earliest_return ?? now) - now;
      leaveStatus.textContent = `On leave · ${days(left)} left`;
      leaveToggle.textContent = wait > 0 ? `Return in ${days(wait)}` : "Return";
      leaveToggle.disabled = wait > 0;
      leaveConfirm2.hidden = true;
    } else {
      const wait = view.next_start - now;
      leaveStatus.textContent = `${days(view.max_secs)} max · ${days(view.min_secs)} minimum`;
      leaveToggle.textContent = wait > 0 ? `Next leave in ${days(wait)}` : "Take leave";
      leaveToggle.disabled = wait > 0;
      leaveConfirmText.textContent = `No orders for ${days(view.min_secs)}.`;
    }
  }
  leaveToggle.addEventListener("click", () => {
    const view = ctx.state?.leave;
    if (!view || leaveToggle.disabled) return;
    if (view.on) ctx.send({ type: "SetLeave", on: false });
    else leaveConfirm2.hidden = false;
  }, { signal });
  dialog.querySelector("[data-leave-yes]")!.addEventListener("click", () => {
    leaveConfirm2.hidden = true;
    ctx.send({ type: "SetLeave", on: true });
  }, { signal });
  dialog.querySelector("[data-leave-no]")!.addEventListener("click", () => { leaveConfirm2.hidden = true; }, { signal });
  const leaveTimer = setInterval(renderLeave, 1000);
  signal.addEventListener("abort", () => clearInterval(leaveTimer), { once: true });
  function present(value: Account | null): void {
    account = value;
    profileLabel = value?.is_guest ? "Profile · Guest" : "Profile";
    open.textContent = profileLabel;
    renderLeave();
    dialog.querySelector<HTMLElement>("[data-profile-corporation]")!.textContent = value?.corporation_name ?? "";
    form.hidden = !value?.is_guest;
    signOut.disabled = busy || !value;
    status.textContent = value?.is_guest
      ? "Guest access lasts 30 days in this browser. Register before signing out or clearing browser data."
      : value ? "Registered account" : "Session ended. Please sign in again.";
  }
  async function refresh(): Promise<void> {
    const request = ++generation;
    try {
      const value = await accounts.session();
      if (!signal.aborted && request === generation) present(value);
    } catch (failure) {
      if (!signal.aborted && request === generation && dialog.open) {
        status.textContent = "Could not load your account. Close Profile and try again.";
        error.textContent = failure instanceof Error ? failure.message : NO_SIGNAL;
      }
    }
  }
  const loadAlerts = bindAlerts(dialog, error, signal);
  open.addEventListener("click", async () => {
    if (dialog.open) return;
    error.textContent = "";
    leave.hidden = true;
    charter.reset();
    submit.disabled = true;
    status.textContent = "Loading account…";
    form.hidden = true;
    signOut.disabled = true;
    leaveConfirm2.hidden = true;
    dialog.showModal(); // Browser-managed focus trap; underlying map is inert.
    renderLeave();
    void loadAlerts();
    await refresh();
  }, { signal });
  dialog.querySelector("[data-profile-close]")!.addEventListener("click", () => dialog.close(), { signal });
  dialog.addEventListener("close", () => {
    clearPasswords();
    charter.reset();
    submit.disabled = true;
    leave.hidden = true;
    generation++;
  }, { signal });
  // Do not let modal keystrokes reach map/order shortcuts behind the form.
  // Esc's native dialog default still closes this layer and restores focus.
  dialog.addEventListener("keydown", event => event.stopPropagation(), { signal });
  signal.addEventListener("abort", () => { clearPasswords(); dialog.close(); }, { once: true });
  form.addEventListener("submit", async event => {
    event.preventDefault();
    if (busy || !account?.is_guest) return;
    error.textContent = "";
    if (password.value !== confirm.value) {
      error.textContent = "Passphrases do not match.";
      return;
    }
    if (!charter.accept.checked) {
      error.textContent = CHARTER_REQUIRED;
      charter.accept.focus();
      return;
    }
    setBusy(true);
    try {
      await accounts.completeRegistration(login.value, password.value);
      // Server rotates the cookie. Rejoin the existing player, not a new one.
      ctx.net.disconnect();
      location.reload();
    } catch (failure) {
      if (!signal.aborted) error.textContent = failure instanceof Error ? failure.message : "Could not complete registration.";
    } finally {
      clearPasswords();
      setBusy(false);
    }
  }, { signal });
  async function logout(): Promise<void> {
    if (busy) return;
    setBusy(true);
    error.textContent = "";
    try {
      await accounts.signOut();
      ctx.net.disconnect();
      location.reload();
    } catch {
      if (!signal.aborted) error.textContent = "Sign out failed. Please try again.";
    } finally {
      setBusy(false);
    }
  }
  signOut.addEventListener("click", () => {
    if (busy || !account) return;
    if (account.is_guest) leave.hidden = false;
    else return logout();
  }, { signal });
  leaveConfirm.addEventListener("click", () => {
    if (account?.is_guest && !leave.hidden) return logout();
  }, { signal });
  dialog.querySelector("[data-profile-register-first]")!.addEventListener("click", () => {
    leave.hidden = true;
    login.focus();
  }, { signal });
  void refresh(); // One account request per shell mount, never per View/tick.
}

/** §notifications (R8): the Profile's Alerts section. Kinds and the digest
 * save as they change; push is per browser. Returns a loader run on open. */
function bindAlerts(dialog: HTMLDialogElement, error: HTMLElement, signal: AbortSignal): () => Promise<void> {
  const section = dialog.querySelector<HTMLElement>(".account-notify")!;
  const status = section.querySelector<HTMLElement>("[data-notify-status]")!;
  const kinds = [...section.querySelectorAll<HTMLInputElement>("[data-notify-kind]")];
  const push = section.querySelector<HTMLButtonElement>("[data-notify-push]")!;
  const digest = section.querySelector<HTMLInputElement>("[data-notify-digest]")!;
  const email = section.querySelector<HTMLInputElement>("[data-notify-email]")!;
  let prefs: NotifyPrefs | null = null;
  let saving: ReturnType<typeof setTimeout> | undefined;
  const fail = (failure: unknown) => { if (!signal.aborted) error.textContent = failure instanceof Error ? failure.message : "Notifications are unavailable right now."; };
  async function renderPush(): Promise<void> {
    const device = await notifications.device().catch(() => null);
    push.textContent = device ? "Push on · turn off" : "Push on this device";
    push.disabled = !notifications.pushSupported() || !prefs;
    const off = [prefs && !prefs.push_live ? "server push off" : "", prefs && !prefs.email_live ? "email off" : ""].filter(Boolean);
    status.textContent = `While you're away${off.length ? ` · ${off.join(" · ")}` : ""}.`;
  }
  function save(): void {
    clearTimeout(saving);
    saving = setTimeout(() => {
      const picked = kinds.filter((box) => box.checked).map((box) => box.dataset.notifyKind!);
      notifications.save(picked, email.value.trim() || null, digest.checked).then(() => { error.textContent = ""; }, fail);
    }, 500);
  }
  for (const box of [...kinds, digest]) box.addEventListener("change", save, { signal });
  email.addEventListener("input", save, { signal });
  push.addEventListener("click", async () => {
    if (!prefs) return;
    push.disabled = true;
    try {
      if (await notifications.device()) await notifications.unsubscribe();
      else await notifications.subscribe(prefs.public_key);
    } catch (failure) { fail(failure); }
    await renderPush();
  }, { signal });
  signal.addEventListener("abort", () => clearTimeout(saving), { once: true });
  return async () => {
    try {
      prefs = await notifications.get();
      for (const box of kinds) box.checked = prefs.kinds.includes(box.dataset.notifyKind!);
      digest.checked = prefs.email_digest;
      email.value = prefs.email ?? "";
    } catch (failure) { fail(failure); }
    await renderPush();
  };
}
