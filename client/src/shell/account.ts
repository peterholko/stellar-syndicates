import "../styles/account.css";
import { accounts, type Account } from "../auth";
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

// The two shells share the same credential workflow. Only account creation asks
// for a public corporation name; knowing that name is never authentication.
export function accountFields(prefix: string): string {
  return `
    <header class="account-heading">
      <div class="account-authority">
        <img class="account-authority__seal" src="/art/derived/authority-v2/terran-charter-authority-seal-192.webp" srcset="/art/derived/authority-v2/terran-charter-authority-seal-96.webp 1x, /art/derived/authority-v2/terran-charter-authority-seal-192.webp 2x, /art/derived/authority-v2/terran-charter-authority-seal-288.webp 3x, /art/derived/authority-v2/terran-charter-authority-seal-384.webp 4x" alt="Terran Charter Authority seal" width="96" height="96">
        <div><small>Terran Charter Authority</small><h1 id="${prefix}-join-title">Charter Command</h1></div>
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
    submit.disabled = form.dataset.busy === "true" || (registering && !charter.accept.checked);
    if (charter.accept.checked && error.textContent === CHARTER_REQUIRED) error.textContent = "";
  });
  function presentMode(): void {
    form.dataset.accountMode = registering ? "register" : "signin";
    form.querySelectorAll<HTMLElement>("[data-register-field]").forEach(el => { el.hidden = !registering; });
    corporation.required = registering;
    corporation.disabled = !registering;
    charter.accept.required = registering;
    charter.accept.disabled = !registering;
    if (!registering) charter.reset();
    submit.disabled = registering && !charter.accept.checked;
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
    submit.disabled = toggle.disabled = guest.disabled = true;
    charter.accept.disabled = true;
    error.textContent = "";
    try {
      await action();
      // Reload after authentication so no prior corporation's private view or
      // staged intent survives an account switch. Boot resumes the new cookie.
      ctx.net.disconnect();
      location.reload();
    } catch (failure) {
      if (!signal.aborted) error.textContent = failure instanceof Error ? failure.message : "Unable to sign in.";
    } finally {
      // Clear the field even when a request fails or the shell changes.
      password.value = "";
      delete form.dataset.busy;
      toggle.disabled = guest.disabled = false;
      charter.accept.disabled = !registering;
      submit.disabled = registering && !charter.accept.checked;
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
  function present(value: Account | null): void {
    account = value;
    open.textContent = value?.is_guest ? "Profile · Guest" : "Profile";
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
        error.textContent = failure instanceof Error ? failure.message : "Account service unavailable.";
      }
    }
  }
  open.addEventListener("click", async () => {
    if (dialog.open) return;
    error.textContent = "";
    leave.hidden = true;
    charter.reset();
    submit.disabled = true;
    status.textContent = "Loading account…";
    form.hidden = true;
    signOut.disabled = true;
    dialog.showModal(); // Browser-managed focus trap; underlying map is inert.
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
