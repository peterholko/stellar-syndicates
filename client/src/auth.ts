// Account requests stay on the page's origin (Vite proxies them in development).
// Passwords never go over the game socket; session tokens are HttpOnly cookies,
// never localStorage, URL parameters, game state, or JavaScript-readable tokens.
export interface Account { id: string; corporation_name: string; is_guest: boolean }

async function request(path: string, body?: object): Promise<Response> {
  return fetch(`/api/account/${path}`, {
    method: body === undefined ? "GET" : "POST",
    credentials: "same-origin",
    redirect: "error",
    cache: "no-store",
    headers: { "Content-Type": "application/json", "X-Stellar-Client": "1" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
}

async function checked(response: Response): Promise<Account> {
  const data = await response.json().catch(() => null);
  if (!response.ok) throw new Error(data?.error ?? "Account service unavailable. Please try again.");
  if (typeof data?.id !== "string" || typeof data?.corporation_name !== "string") {
    throw new Error("Account service unavailable. Please try again.");
  }
  return { id: data.id, corporation_name: data.corporation_name, is_guest: data.is_guest === true };
}

export const accounts = {
  async session(): Promise<Account | null> {
    const response = await request("session");
    return response.status === 401 ? null : checked(response);
  },
  async signIn(login: string, password: string): Promise<Account> {
    return checked(await request("login", { login, password }));
  },
  async register(login: string, password: string, corporation_name: string): Promise<Account> {
    return checked(await request("register", { login, password, corporation_name }));
  },
  async guest(): Promise<Account> {
    return checked(await request("guest", {}));
  },
  async completeRegistration(login: string, password: string): Promise<Account> {
    return checked(await request("complete-registration", { login, password }));
  },
  async signOut(): Promise<void> {
    const response = await request("logout", {});
    if (!response.ok) throw new Error("Could not sign out. Please try again.");
  },
};
