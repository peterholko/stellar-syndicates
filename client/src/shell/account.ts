import { accounts } from "../auth";
import type { CoreContext } from "./types";

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
    <small data-register-field hidden>Use a passphrase of at least 15 characters.</small>
    <button id="${prefix}-join-button" type="submit">Sign in</button>
    <button type="button" data-account-toggle>Create account</button>
    <div id="${prefix}-join-error" role="alert" class="${prefix === "deck" ? "deck-inline-error" : "m-join__error"}"></div>`;
}

export function bindAccountForm(prefix: string, ctx: CoreContext, signal: AbortSignal): void {
  const form = document.getElementById(`${prefix}-join-form`) as HTMLFormElement;
  const login = form.querySelector<HTMLInputElement>("[name=username]")!;
  const password = form.querySelector<HTMLInputElement>("[name=password]")!;
  const corporation = form.querySelector<HTMLInputElement>("[name=corporation]")!;
  const submit = form.querySelector<HTMLButtonElement>("[type=submit]")!;
  const toggle = form.querySelector<HTMLButtonElement>("[data-account-toggle]")!;
  const error = document.getElementById(`${prefix}-join-error`)!;
  signal.addEventListener("abort", () => { password.value = ""; }, { once: true });
  let registering = false;
  toggle.addEventListener("click", () => {
    registering = !registering;
    form.querySelectorAll<HTMLElement>("[data-register-field]").forEach(el => { el.hidden = !registering; });
    corporation.required = registering;
    password.autocomplete = registering ? "new-password" : "current-password";
    password.minLength = registering ? 15 : 1;
    form.querySelector("h1")!.textContent = registering ? "Create account" : "Sign in";
    submit.textContent = registering ? "Create account" : "Sign in";
    toggle.textContent = registering ? "Already have an account? Sign in" : "Create account";
    error.textContent = "";
  }, { signal });
  form.addEventListener("submit", async event => {
    event.preventDefault();
    if (form.dataset.busy === "true") return;
    form.dataset.busy = "true";
    submit.disabled = toggle.disabled = true;
    error.textContent = "";
    try {
      if (registering) {
        await accounts.register(login.value, password.value, corporation.value);
      } else {
        await accounts.signIn(login.value, password.value);
      }
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
      submit.disabled = toggle.disabled = false;
    }
  }, { signal });
  document.getElementById(`${prefix}-sign-out`)?.addEventListener("click", async event => {
    const button = event.currentTarget as HTMLButtonElement;
    button.disabled = true;
    try {
      await accounts.signOut();
      ctx.net.disconnect();
      location.reload(); // clears all private view/intent caches as well as UI
    } catch {
      button.textContent = "Sign out failed — retry";
      button.disabled = false;
    }
  }, { signal });
}
