import "../styles/account.css";
import { accounts, type Account } from "../auth";
import type { CoreContext } from "./types";

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
    <h1 id="${prefix}-join-title">Sign in</h1>
    <label for="${prefix}-login">Username or email</label>
    <input id="${prefix}-login" name="username" type="text" required maxlength="254" autocomplete="username" autocapitalize="none" spellcheck="false">
    <label for="${prefix}-password">Password</label>
    <input id="${prefix}-password" name="password" type="password" required maxlength="128" autocomplete="current-password">
    <label for="${prefix}-name" data-register-field hidden>Corporation name</label>
    <input id="${prefix}-name" name="corporation" maxlength="32" autocomplete="organization" data-register-field hidden>
    <small data-register-field hidden>Use at least 8 characters.</small>
    <button id="${prefix}-join-button" type="submit">Sign in</button>
    <button type="button" data-account-toggle>Create account</button>
    <button type="button" data-account-guest>Guest sign-in</button>
    <small class="account-guest-hint">Start now. Register later in Profile to keep access.</small>
    <div id="${prefix}-join-error" role="alert" class="${prefix === "deck" ? "deck-inline-error" : "m-join__error"}"></div>`;
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
  toggle.addEventListener("click", () => {
    if (form.dataset.busy === "true") return;
    registering = !registering;
    form.querySelectorAll<HTMLElement>("[data-register-field]").forEach(el => { el.hidden = !registering; });
    corporation.required = registering;
    password.autocomplete = registering ? "new-password" : "current-password";
    password.minLength = registering ? 8 : 1;
    form.querySelector("h1")!.textContent = registering ? "Create account" : "Sign in";
    submit.textContent = registering ? "Create account" : "Sign in";
    toggle.textContent = registering ? "Already have an account? Sign in" : "Create account";
    error.textContent = "";
  }, { signal });
  async function authenticate(action: () => Promise<Account>): Promise<void> {
    if (form.dataset.busy === "true") return;
    form.dataset.busy = "true";
    submit.disabled = toggle.disabled = guest.disabled = true;
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
      submit.disabled = toggle.disabled = guest.disabled = false;
    }
  }
  form.addEventListener("submit", event => {
    event.preventDefault();
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
      <label for="${prefix}-profile-login">Username or email</label>
      <input id="${prefix}-profile-login" name="username" required maxlength="254" autocomplete="username" autocapitalize="none" spellcheck="false">
      <label for="${prefix}-profile-password">Password</label>
      <input id="${prefix}-profile-password" name="password" type="password" required minlength="8" maxlength="128" autocomplete="new-password" aria-describedby="${prefix}-profile-password-hint">
      <small id="${prefix}-profile-password-hint">Use at least 8 characters.</small>
      <label for="${prefix}-profile-confirm">Confirm password</label>
      <input id="${prefix}-profile-confirm" name="confirm-password" type="password" required minlength="8" maxlength="128" autocomplete="new-password">
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
  const clearPasswords = () => { password.value = confirm.value = ""; };
  const setBusy = (value: boolean) => {
    busy = value;
    submit.disabled = signOut.disabled = leaveConfirm.disabled = value;
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
    status.textContent = "Loading account…";
    form.hidden = true;
    signOut.disabled = true;
    dialog.showModal(); // Browser-managed focus trap; underlying map is inert.
    await refresh();
  }, { signal });
  dialog.querySelector("[data-profile-close]")!.addEventListener("click", () => dialog.close(), { signal });
  dialog.addEventListener("close", () => {
    clearPasswords();
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
      error.textContent = "Passwords do not match.";
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
