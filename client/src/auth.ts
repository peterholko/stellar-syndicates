// Account requests stay on the page's origin (Vite proxies them in development).
// Passwords never go over the game socket; session tokens are HttpOnly cookies,
// never localStorage, URL parameters, game state, or JavaScript-readable tokens.
/** §front-door: `game_url`, the game socket when it has its own host. */
export interface Account { id: string; corporation_name: string; is_guest: boolean; game_url?: string }

/** The account service is unreachable (the server is down, or the network). */
export const NO_SIGNAL = "No signal from Charter Command. Standing by…";

async function request(path: string, body?: object): Promise<Response> {
  return fetch(`/api/account/${path}`, {
    method: body === undefined ? "GET" : "POST",
    credentials: "same-origin",
    redirect: "error",
    cache: "no-store",
    headers: { "Content-Type": "application/json", "X-Stellar-Client": "1" },
    body: body === undefined ? undefined : JSON.stringify(body),
  }).catch(() => { throw new Error(NO_SIGNAL); });
}

async function checked(response: Response): Promise<Account> {
  const data = await response.json().catch(() => null);
  if (!response.ok) throw new Error(data?.error ?? NO_SIGNAL);
  if (typeof data?.id !== "string" || typeof data?.corporation_name !== "string") {
    throw new Error(NO_SIGNAL);
  }
  return { id: data.id, corporation_name: data.corporation_name, is_guest: data.is_guest === true,
    game_url: typeof data.game_url === "string" ? data.game_url : undefined };
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
