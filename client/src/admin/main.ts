// Operator dashboard (/admin.html). Reads the server's admin API with the same
// session cookie as the game; the server decides who is an operator. Every
// player-supplied string reaches the page through textContent, never as HTML.
import "../styles/tokens.css";
import "../styles/admin.css";
import * as fmt from "./format";
import { tutorialRank } from "./tutorial";

interface CorporationStats {
  name: string; bot: boolean; home: string | null;
  systems: number; worlds: number; population: number;
  fleets: number; ships: number; credits: number; valuation: number;
}
interface OnlineNow { since: string; ip: string; device_id: string | null; actions: number; last_action_at: string | null }
interface StepTime { key: string; label: string; chapter: string; at: string | null }
interface PlayerTutorial { next: string | null; next_label: string | null; done: number; total: number; since: string | null; steps?: StepTime[] }
interface FunnelStage { key: string; label: string; chapter: string; reached: number; median_seconds: number | null; waiting_active: number; waiting_quiet: number }
interface Funnel { galaxy_id: string; galaxy_started_at: string; players: number; quiet_after_hours: number; stages: FunnelStage[] }
interface PlayerSummary {
  account_id: string; login: string | null; corporation: string; guest: boolean; disabled: boolean;
  registered_at: string; last_login_at: string | null; logins: number;
  last_ip: string | null; last_device: string | null; last_user_agent: string | null;
  sessions: number; online_seconds: number; active_seconds: number; actions: number;
  last_seen_at: string | null; online: OnlineNow | null; devices: number; shared_accounts: number;
  game: CorporationStats | null;
  tutorial: PlayerTutorial | null; operator: boolean; in_funnel: boolean; active: boolean;
}
interface Overview {
  generated_at: string;
  server: {
    started_at: string; online_players: number; connections: number; bot_players: number; bot_commands: number;
    corporations: number; galaxy_systems: number; tick: number; sim_time: number; holdings: boolean;
  };
  totals: { accounts: number; registered: number; guests: number; founded: number; online: number; new_24h: number; active_24h: number; active_7d: number };
  funnel: Funnel | null;
  players: PlayerSummary[];
  unlinked: CorporationStats[];
}
interface PlayerDetail {
  generated_at: string;
  player: PlayerSummary;
  logins: { kind: string; at: string; ip: string | null; user_agent: string | null; language: string | null; device_id: string | null }[];
  sessions: { connected_at: string; disconnected_at: string | null; last_seen_at: string; ip: string | null; user_agent: string | null; device_id: string | null; actions: number; active_seconds: number }[];
  days: { day: string; sessions: number; online_seconds: number; active_seconds: number; actions: number }[];
  commands: { kind: string; count: number; last_at: string }[];
  recent: { at: string; kind: string; detail: unknown }[];
  shared: { account_id: string; login: string | null; corporation: string; via: "device" | "ip"; value: string }[];
}

class ApiError extends Error {
  constructor(readonly code: string, message: string) { super(message); }
}

async function get<T>(path: string): Promise<T> {
  let response: Response;
  try {
    response = await fetch(path, {
      credentials: "same-origin", cache: "no-store", redirect: "error",
      headers: { "X-Stellar-Client": "1" },
    });
  } catch {
    throw new ApiError("offline", "The server did not answer. Is it running?");
  }
  const data = await response.json().catch(() => null);
  if (!response.ok) throw new ApiError(data?.code ?? "unavailable", data?.error ?? "The server did not answer.");
  return data as T;
}

// ---- DOM ------------------------------------------------------------------

type Child = Node | string | number | null | undefined | false | Child[];
type Props = Record<string, string | number | boolean | ((event: Event) => void) | undefined>;

function h<K extends keyof HTMLElementTagNameMap>(tag: K, props: Props | null = null, ...children: Child[]): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(props ?? {})) {
    if (value === undefined || value === false) continue;
    if (typeof value === "function") el.addEventListener(key.slice(2).toLowerCase(), value);
    else if (key === "class") el.className = String(value);
    else el.setAttribute(key, value === true ? "" : String(value));
  }
  append(el, children);
  return el;
}

function append(parent: Node, children: Child[]): void {
  for (const child of children) {
    if (child === null || child === undefined || child === false) continue;
    if (Array.isArray(child)) append(parent, child);
    else parent.appendChild(child instanceof Node ? child : document.createTextNode(String(child)));
  }
}

function fill(parent: HTMLElement, ...children: Child[]): void {
  parent.replaceChildren();
  append(parent, children);
}

function byId<T extends HTMLElement = HTMLElement>(id: string): T {
  return document.getElementById(id) as T;
}

// ---- State ----------------------------------------------------------------

type Filter = "all" | "online" | "registered" | "guests";
type SortKey = "seen" | "corporation" | "tutorial" | "registered" | "online_seconds" | "active_seconds" | "actions" | "systems" | "worlds" | "ships" | "credits";
type Activity = "playing" | "quiet";

const REFRESH_MS = 15_000;
const state = {
  overview: null as Overview | null,
  error: null as ApiError | null,
  loadedAt: 0,
  loading: false,
  filter: "all" as Filter,
  query: "",
  sort: { key: "seen" as SortKey, dir: -1 },
  auto: true,
  /** Funnel drill-down: players whose next tutorial step is this one. */
  stage: null as { key: string; label: string; activity: Activity | null } | null,
  open: null as string | null,
  detail: null as PlayerDetail | null,
  detailError: null as ApiError | null,
};

// ---- Overview -------------------------------------------------------------

function seenAt(player: PlayerSummary, now: number): number {
  if (player.online) return now + 1;
  return player.last_seen_at ? Date.parse(player.last_seen_at) : 0;
}

function sortValue(player: PlayerSummary, key: SortKey, now: number): number | string {
  switch (key) {
    case "seen": return seenAt(player, now);
    case "corporation": return player.corporation.toLowerCase();
    case "tutorial": return tutorialRank(player.tutorial);
    case "registered": return Date.parse(player.registered_at);
    case "online_seconds": return player.online_seconds;
    case "active_seconds": return player.active_seconds;
    case "actions": return player.actions;
    case "systems": return player.game?.systems ?? -1;
    case "worlds": return player.game?.worlds ?? -1;
    case "ships": return player.game?.ships ?? -1;
    case "credits": return player.game?.credits ?? -Infinity;
  }
}

function visiblePlayers(overview: Overview, now: number): PlayerSummary[] {
  const query = state.query.trim().toLowerCase();
  const { key, dir } = state.sort;
  return overview.players
    .filter(player => state.filter === "all"
      || (state.filter === "online" && player.online)
      || (state.filter === "registered" && !player.guest)
      || (state.filter === "guests" && player.guest))
    .filter(player => !query || [player.corporation, player.login, player.last_ip, player.online?.ip, player.last_device, player.account_id]
      .some(value => value?.toLowerCase().includes(query)))
    .filter(player => !state.stage || (player.in_funnel && player.tutorial?.next === state.stage.key
      && (state.stage.activity === null || player.active === (state.stage.activity === "playing"))))
    .sort((a, b) => {
      const x = sortValue(a, key, now), y = sortValue(b, key, now);
      const order = x < y ? -1 : x > y ? 1 : 0;
      return order * dir || a.corporation.localeCompare(b.corporation);
    });
}

function card(label: string, value: Child, note?: Child): HTMLElement {
  return h("div", { class: "admin-card" }, h("span", null, label), h("b", null, value), note ? h("small", null, note) : null);
}

function renderSummary(overview: Overview, now: number): void {
  const { totals, server } = overview;
  byId("admin-cards").replaceChildren(
    card("Accounts", fmt.count(totals.accounts), `${fmt.count(totals.registered)} registered · ${fmt.count(totals.guests)} guests`),
    card("Online now", fmt.count(totals.online), `${fmt.count(server.connections)} game sockets`),
    card("Corporations founded", fmt.count(totals.founded), `${fmt.count(overview.unlinked.length)} without an account`),
    card("New in 24 h", fmt.count(totals.new_24h)),
    card("Active in 24 h", fmt.count(totals.active_24h)),
    card("Active in 7 days", fmt.count(totals.active_7d)),
  );
  byId("admin-server").replaceChildren(
    h("span", null, "Tick ", h("b", null, fmt.count(server.tick))),
    h("span", null, "Sim ", h("b", null, fmt.duration(server.sim_time))),
    h("span", { title: fmt.dateTime(server.started_at) }, "Up ", h("b", null, fmt.duration((now - Date.parse(server.started_at)) / 1000))),
    h("span", null, "Systems ", h("b", null, fmt.count(server.galaxy_systems))),
  );
  byId("admin-holdings-note").hidden = server.holdings;
}

function showStage(stage: FunnelStage, activity: Activity | null): void {
  state.stage = { key: stage.key, label: stage.label, activity };
  state.filter = "all";
  if (state.overview) renderPlayers(state.overview, Date.now());
  byId("admin-players").scrollIntoView({ behavior: "smooth", block: "start" });
}

function clearStage(): void {
  state.stage = null;
  if (state.overview) renderPlayers(state.overview, Date.now());
}

function waitingButtons(stage: FunnelStage): Child {
  if (stage.waiting_active + stage.waiting_quiet === 0) return h("span", { class: "admin-muted" }, "—");
  return h("span", { class: "admin-waiting" },
    stage.waiting_active > 0
      ? h("button", { type: "button", class: "admin-chip is-playing", title: "Show the players on this step who are still playing", onclick: () => showStage(stage, "playing") },
        `${fmt.count(stage.waiting_active)} playing`)
      : null,
    stage.waiting_quiet > 0
      ? h("button", { type: "button", class: "admin-chip is-quiet", title: "Show the players who stopped here", onclick: () => showStage(stage, "quiet") },
        `${fmt.count(stage.waiting_quiet)} gone quiet`)
      : null);
}

function renderFunnel(overview: Overview): void {
  const box = byId("admin-funnel");
  const funnel = overview.funnel;
  box.hidden = !funnel;
  if (!funnel) return;
  const total = Math.max(1, funnel.players);
  // Where the most players went quiet: the step to look at first.
  const worst = Math.max(0, ...funnel.stages.map(stage => stage.waiting_quiet));
  let chapter = "";
  const rows: HTMLElement[] = [];
  funnel.stages.forEach((stage, index) => {
    if (stage.chapter !== chapter) {
      chapter = stage.chapter;
      rows.push(h("tr", { class: "admin-funnel-chapter" }, h("th", { colspan: 6, scope: "rowgroup" }, chapter)));
    }
    const previous = funnel.stages[index - 1];
    const kept = previous && previous.reached > 0 ? stage.reached / previous.reached : null;
    const share = stage.reached / total;
    rows.push(h("tr", { class: worst > 0 && stage.waiting_quiet === worst ? "is-worst" : undefined },
      h("td", { class: "num admin-muted" }, index + 1),
      h("th", { scope: "row" }, stage.label),
      h("td", { class: "admin-funnel-reached", title: `${fmt.count(stage.reached)} of ${fmt.count(funnel.players)} players` },
        h("i", { style: `--share:${share * 100}%` }),
        h("b", null, fmt.count(stage.reached)), h("small", null, fmt.percent(share))),
      h("td", { class: "num", title: "Share of those who reached the step before" }, kept === null ? "—" : fmt.percent(kept)),
      h("td", { class: "num", title: "Median time from the step before" }, stage.median_seconds === null ? "—" : fmt.duration(stage.median_seconds)),
      h("td", null, waitingButtons(stage))));
  });
  fill(box,
    h("div", { class: "admin-toolbar" },
      h("h2", null, "Tutorial funnel"),
      h("p", { class: "admin-muted admin-funnel-scope" },
        `${fmt.count(funnel.players)} players who arrived since this galaxy began (${fmt.dateTime(funnel.galaxy_started_at)}). Operators are left out. `,
        `Gone quiet: no activity for ${funnel.quiet_after_hours} h.`)),
    h("div", { class: "admin-table-wrap" },
      h("table", { class: "admin-table admin-funnel" },
        h("thead", null, h("tr", null,
          h("th", { class: "num", scope: "col" }, "#"),
          h("th", { scope: "col" }, "Step"),
          h("th", { scope: "col" }, "Reached"),
          h("th", { class: "num", scope: "col", title: "Share of those who reached the step before" }, "Kept"),
          h("th", { class: "num", scope: "col", title: "Median time from the step before" }, "Median time"),
          h("th", { scope: "col", title: "Players whose next step is this one" }, "On this step"))),
        h("tbody", null, rows))));
}

const COLUMNS: { key: SortKey | null; label: string; num?: boolean; title?: string }[] = [
  { key: "seen", label: "Last seen" },
  { key: "corporation", label: "Corporation" },
  { key: "tutorial", label: "Tutorial", title: "Founding charter: steps done, and the step the player is on" },
  { key: "registered", label: "Registered" },
  { key: null, label: "Last IP · browser" },
  { key: "online_seconds", label: "Online", num: true, title: "Total time with the game open" },
  { key: "active_seconds", label: "Active", num: true, title: "Five-minute windows with at least one command" },
  { key: "actions", label: "Commands", num: true },
  { key: "systems", label: "Systems", num: true },
  { key: "worlds", label: "Worlds", num: true, title: "Bodies with people living on them" },
  { key: "ships", label: "Ships", num: true },
  { key: "credits", label: "Credits", num: true },
];

function renderHead(): void {
  byId("admin-players-head").replaceChildren(h("tr", null, COLUMNS.map(column => {
    const sorted = column.key !== null && state.sort.key === column.key;
    return h("th", {
      class: [column.num ? "num" : "", column.key ? "sortable" : ""].join(" ").trim(),
      title: column.title,
      "aria-sort": sorted ? (state.sort.dir > 0 ? "ascending" : "descending") : undefined,
      scope: "col",
    }, column.key
      ? h("button", { type: "button", onclick: () => setSort(column.key as SortKey) }, column.label, sorted ? (state.sort.dir > 0 ? " ▲" : " ▼") : "")
      : column.label);
  })));
}

function setSort(key: SortKey): void {
  state.sort = state.sort.key === key
    ? { key, dir: -state.sort.dir }
    : { key, dir: key === "corporation" ? 1 : -1 };
  renderHead();
  if (state.overview) renderPlayers(state.overview, Date.now());
}

function badges(player: PlayerSummary): Child[] {
  return [
    player.guest ? h("span", { class: "admin-badge is-guest" }, "guest") : null,
    player.disabled ? h("span", { class: "admin-badge is-disabled" }, "disabled") : null,
    player.shared_accounts > 0
      ? h("span", { class: "admin-badge is-shared", title: "Other accounts have signed in from this account's browser" }, `shares a browser with ${player.shared_accounts}`)
      : null,
    player.operator ? h("span", { class: "admin-badge is-operator", title: "Listed in ADMIN_LOGINS; left out of the funnel" }, "operator") : null,
  ];
}

function tutorialCell(player: PlayerSummary, now: number): HTMLTableCellElement {
  const tutorial = player.tutorial;
  if (!tutorial) return h("td", { class: "admin-muted" }, "—");
  const bar = h("span", { class: "admin-steps", style: `--done:${(tutorial.done / Math.max(1, tutorial.total)) * 100}%` });
  if (!tutorial.next) return h("td", { class: "admin-tutorial is-done" }, bar, "Charter fulfilled", h("small", null, `${tutorial.done}/${tutorial.total} steps`));
  const waiting = tutorial.since ? fmt.duration((now - Date.parse(tutorial.since)) / 1000) : null;
  const quiet = !player.active;
  return h("td", {
    class: `admin-tutorial${quiet ? " is-quiet" : ""}`,
    title: `${tutorial.done} of ${tutorial.total} charter steps done. Next: ${tutorial.next_label}${waiting ? `, waiting ${waiting}` : ""}${quiet ? ". Gone quiet." : ""}`,
  },
    bar, tutorial.next_label,
    h("small", null, `${tutorial.done}/${tutorial.total}${waiting ? ` · waiting ${waiting}` : ""}${quiet ? " · quiet" : ""}`));
}

function playerRow(player: PlayerSummary, now: number): HTMLTableRowElement {
  const game = player.game;
  const ip = player.online?.ip ?? player.last_ip;
  const open = () => openPlayer(player.account_id);
  return h("tr", {
    class: [player.online ? "is-online" : "", state.open === player.account_id ? "is-open" : ""].join(" ").trim(),
    tabindex: 0,
    onclick: open,
    onkeydown: (event: Event) => { if ((event as KeyboardEvent).key === "Enter") open(); },
  },
    h("td", { title: player.online ? `Online since ${fmt.dateTime(player.online.since)}` : fmt.dateTime(player.last_seen_at) },
      h("span", { class: `admin-dot${player.online ? " is-on" : ""}` }),
      player.online ? `online ${fmt.duration((now - Date.parse(player.online.since)) / 1000)}` : fmt.ago(player.last_seen_at, now)),
    h("td", { class: "admin-name" },
      h("b", null, player.corporation), " ", badges(player),
      h("small", null, player.login ?? "no login (guest)")),
    tutorialCell(player, now),
    h("td", { title: fmt.dateTime(player.registered_at) }, fmt.ago(player.registered_at, now)),
    h("td", { title: player.last_user_agent ?? "" },
      ip ?? "—", h("small", null, `${fmt.browser(player.last_user_agent)} · ${fmt.device(player.online?.device_id ?? player.last_device)}`)),
    h("td", { class: "num" }, fmt.duration(player.online_seconds)),
    h("td", { class: "num" }, fmt.duration(player.active_seconds)),
    h("td", { class: "num" }, fmt.count(player.actions)),
    h("td", { class: "num" }, game ? fmt.count(game.systems) : "—"),
    h("td", { class: "num" }, game ? fmt.count(game.worlds) : "—"),
    h("td", { class: "num", title: game ? `${fmt.count(game.fleets)} fleets` : "Not founded" }, game ? fmt.count(game.ships) : "—"),
    h("td", { class: "num", title: game ? `Valuation ${fmt.credits(game.valuation)}` : "Not founded" }, game ? fmt.credits(game.credits) : "—"),
  );
}

function renderPlayers(overview: Overview, now: number): void {
  const players = visiblePlayers(overview, now);
  byId("admin-players-body").replaceChildren(...players.map(player => playerRow(player, now)));
  byId("admin-players-empty").hidden = players.length > 0;
  byId("admin-players-count").textContent = players.length === overview.players.length
    ? `${players.length} accounts`
    : `${players.length} of ${overview.players.length} accounts`;
  const stage = state.stage;
  fill(byId("admin-stage-filter"), stage
    ? h("span", { class: "admin-chip is-filter" },
      `On “${stage.label}”${stage.activity === "quiet" ? " · gone quiet" : stage.activity === "playing" ? " · playing" : ""}`,
      h("button", { type: "button", "aria-label": "Clear the tutorial filter", onclick: clearStage }, "×"))
    : null);
  for (const button of document.querySelectorAll<HTMLButtonElement>("[data-filter]")) {
    button.setAttribute("aria-pressed", String(button.dataset.filter === state.filter));
  }
}

function renderUnlinked(overview: Overview): void {
  byId("admin-unlinked").hidden = overview.unlinked.length === 0;
  byId("admin-unlinked-body").replaceChildren(...overview.unlinked.map(corp => h("tr", null,
    h("td", { class: "admin-name" }, h("b", null, corp.name), " ",
      h("span", { class: `admin-badge${corp.bot ? " is-bot" : ""}` }, corp.bot ? "bot" : "no account")),
    h("td", null, corp.home ?? "—"),
    h("td", { class: "num" }, fmt.count(corp.systems)),
    h("td", { class: "num" }, fmt.count(corp.worlds)),
    h("td", { class: "num" }, fmt.population(corp.population)),
    h("td", { class: "num", title: `${fmt.count(corp.fleets)} fleets` }, fmt.count(corp.ships)),
    h("td", { class: "num" }, fmt.credits(corp.credits)),
    h("td", { class: "num" }, fmt.credits(corp.valuation)),
  )));
}

function renderStatus(now: number): void {
  const status = byId("admin-updated");
  if (state.loading && !state.overview) status.textContent = "Loading…";
  else if (state.overview) status.textContent = `Updated ${fmt.ago(new Date(state.loadedAt).toISOString(), now)}`;
  else status.textContent = "";
}

function renderError(): void {
  const box = byId("admin-message");
  const error = state.error;
  box.hidden = error === null;
  byId("admin-content").hidden = error !== null && state.overview === null;
  if (!error) return;
  const action = error.code === "sign_in"
    ? h("a", { href: "/", class: "admin-button" }, "Open the game to sign in")
    : error.code === "not_operator" || error.code === "off"
      ? null
      : h("button", { type: "button", class: "admin-button", onclick: () => void refresh() }, "Try again");
  fill(box, h("b", null, error.code === "sign_in" ? "Sign in first" : error.code === "not_operator" ? "Operators only" : error.code === "off" ? "Dashboard off" : "Server unavailable"),
    h("p", null, error.message), action);
}

async function refresh(): Promise<void> {
  if (state.loading) return;
  state.loading = true;
  renderStatus(Date.now());
  try {
    state.overview = await get<Overview>("/api/admin/overview");
    state.loadedAt = Date.now();
    state.error = null;
    const now = Date.now();
    renderSummary(state.overview, now);
    renderFunnel(state.overview);
    renderPlayers(state.overview, now);
    renderUnlinked(state.overview);
    if (state.open) await loadDetail(state.open);
  } catch (error) {
    state.error = error instanceof ApiError ? error : new ApiError("unavailable", String(error));
  } finally {
    state.loading = false;
    renderError();
    renderStatus(Date.now());
  }
}

// ---- Player detail --------------------------------------------------------

function fact(label: string, value: Child, title?: string): HTMLElement {
  return h("div", { class: "admin-fact", title }, h("span", null, label), h("b", null, value));
}

function section(title: string, ...children: Child[]): HTMLElement {
  return h("section", { class: "admin-section" }, h("h3", null, title), children);
}

function table(head: string[], rows: Child[][], numeric: number[] = []): HTMLElement {
  return h("div", { class: "admin-table-wrap" }, h("table", { class: "admin-table is-compact" },
    h("thead", null, h("tr", null, head.map((label, i) => h("th", { class: numeric.includes(i) ? "num" : undefined, scope: "col" }, label)))),
    h("tbody", null, rows.map(cells => h("tr", null, cells.map((cell, i) => h("td", { class: numeric.includes(i) ? "num" : undefined }, cell)))))));
}

function lastDays(detail: PlayerDetail, now: number): PlayerDetail["days"] {
  const byDay = new Map(detail.days.map(day => [day.day, day]));
  const days: PlayerDetail["days"] = [];
  for (let i = 29; i >= 0; i--) {
    const key = new Date(now - i * 86_400_000).toISOString().slice(0, 10);
    days.push(byDay.get(key) ?? { day: key, sessions: 0, online_seconds: 0, active_seconds: 0, actions: 0 });
  }
  return days;
}

function dayChart(detail: PlayerDetail, now: number): HTMLElement {
  const days = lastDays(detail, now);
  const most = Math.max(1, ...days.map(day => day.online_seconds));
  return h("div", { class: "admin-days-wrap" },
    h("div", { class: "admin-days", role: "img", "aria-label": "Time online per day over the last 30 days" },
      days.map(day => h("i", {
        style: `--online:${(day.online_seconds / most) * 100}%;--active:${Math.min(day.active_seconds, day.online_seconds) / most * 100}%`,
        title: `${day.day} (UTC): ${fmt.duration(day.online_seconds)} online, ${fmt.duration(day.active_seconds)} active, ${fmt.count(day.actions)} commands, ${fmt.count(day.sessions)} sessions`,
      }))),
    h("div", { class: "admin-days-axis" },
      h("span", null, days[0].day),
      h("span", { class: "admin-days-legend" }, h("span", null, h("i", { class: "is-online" }), "online"), h("span", null, h("i"), "active")),
      h("span", null, "today (UTC)")));
}

function commandBars(detail: PlayerDetail): HTMLElement {
  const most = Math.max(1, ...detail.commands.map(command => command.count));
  return h("div", { class: "admin-bars" }, detail.commands.map(command => h("div", { class: "admin-bar", title: `${command.kind} · last ${fmt.dateTime(command.last_at)}` },
    h("span", null, fmt.commandName(command.kind)),
    h("i", { style: `--share:${(command.count / most) * 100}%` }),
    h("b", null, fmt.count(command.count)))));
}

function tutorialTimeline(player: PlayerSummary, now: number): HTMLElement {
  const tutorial = player.tutorial;
  if (!tutorial?.steps?.length) return section("Tutorial", h("p", { class: "admin-muted" }, "No tutorial record in this galaxy yet."));
  const items: HTMLElement[] = [];
  let chapter = "";
  let last: number | null = null;
  for (const step of tutorial.steps) {
    if (step.chapter !== chapter) {
      chapter = step.chapter;
      items.push(h("li", { class: "admin-timeline-chapter" }, chapter));
    }
    const at = step.at ? Date.parse(step.at) : null;
    const current = tutorial.next === step.key;
    const gap = at !== null && last !== null && at >= last ? `+${fmt.duration((at - last) / 1000)}` : "";
    items.push(h("li", { class: at !== null ? "is-done" : current ? "is-current" : "is-todo" },
      h("i", { "aria-hidden": "true" }),
      h("span", null, step.label),
      at !== null && step.at
        ? h("time", { datetime: step.at, title: fmt.ago(step.at, now) }, fmt.dateTime(step.at))
        : current && tutorial.since
          ? h("time", null, `waiting ${fmt.duration((now - Date.parse(tutorial.since)) / 1000)}${player.active ? "" : " · gone quiet"}`)
          : h("time", null, ""),
      h("small", null, gap)));
    if (at !== null) last = Math.max(last ?? at, at);
  }
  return section(tutorial.next ? `Tutorial · ${tutorial.done} of ${tutorial.total} steps` : "Tutorial · charter fulfilled",
    player.operator ? h("p", { class: "admin-muted" }, "Operator account: left out of the funnel.") : null,
    h("ol", { class: "admin-timeline" }, items));
}

function renderDetail(): void {
  const drawer = byId("admin-drawer");
  const body = byId("admin-drawer-body");
  drawer.classList.toggle("is-open", state.open !== null);
  byId("admin-scrim").hidden = state.open === null;
  document.body.classList.toggle("has-drawer", state.open !== null);
  if (!state.open) { body.replaceChildren(); return; }
  if (state.detailError) {
    body.replaceChildren(h("p", { class: "admin-muted" }, state.detailError.message));
    return;
  }
  const detail = state.detail;
  if (!detail || detail.player.account_id !== state.open) {
    body.replaceChildren(h("p", { class: "admin-muted" }, "Loading…"));
    return;
  }
  const now = Date.now();
  const player = detail.player;
  const game = player.game;
  const ip = player.online?.ip ?? player.last_ip;
  fill(body,
    h("header", { class: "admin-drawer-head" },
      h("div", null,
        h("h2", null, h("span", { class: `admin-dot${player.online ? " is-on" : ""}` }), player.corporation),
        h("p", null, player.login ?? "guest — no login yet", " ", badges(player))),
      h("button", { type: "button", class: "admin-close", "aria-label": "Close", onclick: closePlayer }, "×")),
    tutorialTimeline(player, now),
    section("Account",
      h("div", { class: "admin-facts" },
        fact("Status", player.online ? `Online for ${fmt.duration((now - Date.parse(player.online.since)) / 1000)}` : `Last seen ${fmt.ago(player.last_seen_at, now)}`),
        fact("Registered", fmt.dateTime(player.registered_at)),
        fact("Last sign-in", fmt.dateTime(player.last_login_at)),
        fact("Sign-ins", fmt.count(player.logins)),
        fact("Last IP", ip ?? "—"),
        fact("Device", fmt.device(player.online?.device_id ?? player.last_device), player.online?.device_id ?? player.last_device ?? undefined),
        fact("Browser", fmt.browser(player.last_user_agent), player.last_user_agent ?? undefined),
        fact("Browsers used", fmt.count(player.devices)),
        fact("Account id", player.account_id.slice(0, 8), player.account_id))),
    section("Play",
      h("div", { class: "admin-facts" },
        fact("Time online", fmt.duration(player.online_seconds)),
        fact("Active", fmt.duration(player.active_seconds), "Five-minute windows with at least one command"),
        fact("Sessions", fmt.count(player.sessions)),
        fact("Commands", fmt.count(player.actions))),
      dayChart(detail, now)),
    section("Corporation",
      game
        ? h("div", { class: "admin-facts" },
          fact("Home", game.home ?? "—"),
          fact("Systems", fmt.count(game.systems)),
          fact("Worlds", fmt.count(game.worlds), "Bodies with people living on them"),
          fact("Population", fmt.population(game.population)),
          fact("Fleets", fmt.count(game.fleets)),
          fact("Ships", fmt.count(game.ships)),
          fact("Credits", fmt.credits(game.credits)),
          fact("Valuation", fmt.credits(game.valuation)))
        : h("p", { class: "admin-muted" }, state.overview?.server.holdings === false ? "The game loop did not answer in time." : "No corporation founded in this galaxy yet.")),
    detail.shared.length
      ? section("Shares a browser or network with",
        h("ul", { class: "admin-shared" }, detail.shared.map(entry => h("li", null,
          h("button", { type: "button", class: "admin-link", onclick: () => openPlayer(entry.account_id) }, entry.corporation),
          ` ${entry.login ?? "(guest)"} — same ${entry.via === "device" ? `browser ${fmt.device(entry.value)}` : `IP ${entry.value}`}`))))
      : null,
    section(`Commands by type (${fmt.count(detail.commands.reduce((sum, c) => sum + c.count, 0))})`,
      detail.commands.length ? commandBars(detail) : h("p", { class: "admin-muted" }, "No commands recorded yet.")),
    section("Recent commands",
      detail.recent.length
        ? table(["When", "Command", "Details"], detail.recent.map(action => [
          h("span", { title: fmt.ago(action.at, now) }, fmt.dateTime(action.at)),
          h("span", { title: action.kind }, fmt.commandName(action.kind)),
          h("code", { title: JSON.stringify(action.detail ?? null, null, 2) }, fmt.detail(action.detail)),
        ]))
        : h("p", { class: "admin-muted" }, "No commands recorded yet.")),
    section("Sessions",
      detail.sessions.length
        ? table(["Started", "Length", "Commands", "Active", "IP", "Browser"], detail.sessions.map(play => [
          fmt.dateTime(play.connected_at),
          play.disconnected_at ? fmt.duration((Date.parse(play.disconnected_at) - Date.parse(play.connected_at)) / 1000) : "online",
          fmt.count(play.actions),
          fmt.duration(play.active_seconds),
          play.ip ?? "—",
          h("span", { title: play.user_agent ?? "" }, `${fmt.browser(play.user_agent)} · ${fmt.device(play.device_id)}`),
        ]), [2, 3])
        : h("p", { class: "admin-muted" }, "Has not entered the game yet.")),
    section("Sign-ins",
      table(["When", "How", "IP", "Browser", "Language"], detail.logins.map(login => [
        fmt.dateTime(login.at),
        { register: "registered", login: "signed in", guest: "started as guest", upgrade: "guest registered" }[login.kind] ?? login.kind,
        login.ip ?? "—",
        h("span", { title: login.user_agent ?? "" }, `${fmt.browser(login.user_agent)} · ${fmt.device(login.device_id)}`),
        login.language?.split(",")[0] ?? "—",
      ]))),
  );
}

async function loadDetail(id: string): Promise<void> {
  try {
    const detail = await get<PlayerDetail>(`/api/admin/players/${encodeURIComponent(id)}`);
    if (state.open !== id) return;
    state.detail = detail;
    state.detailError = null;
  } catch (error) {
    if (state.open !== id) return;
    state.detailError = error instanceof ApiError ? error : new ApiError("unavailable", String(error));
  }
  renderDetail();
}

function openPlayer(id: string): void {
  if (state.open !== id) {
    state.open = id;
    state.detail = null;
    state.detailError = null;
    history.replaceState(null, "", `#player=${id}`);
  }
  renderDetail();
  if (state.overview) renderPlayers(state.overview, Date.now());
  void loadDetail(id);
}

function closePlayer(): void {
  state.open = null;
  state.detail = null;
  history.replaceState(null, "", location.pathname);
  renderDetail();
  if (state.overview) renderPlayers(state.overview, Date.now());
}

// ---- Shell ----------------------------------------------------------------

function build(): void {
  const filters: [Filter, string][] = [["all", "All"], ["online", "Online"], ["registered", "Registered"], ["guests", "Guests"]];
  document.body.classList.add("admin");
  document.body.replaceChildren(
    h("header", { class: "admin-top" },
      h("div", { class: "admin-title" }, h("small", null, "Stellar Syndicates"), h("h1", null, "Operations")),
      h("div", { class: "admin-server", id: "admin-server" }),
      h("div", { class: "admin-refresh" },
        h("span", { id: "admin-updated", class: "admin-muted", "aria-live": "polite" }),
        h("label", { class: "admin-toggle" },
          h("input", { type: "checkbox", checked: state.auto, onchange: (event: Event) => { state.auto = (event.target as HTMLInputElement).checked; } }),
          " Auto"),
        h("button", { type: "button", class: "admin-button", onclick: () => void refresh() }, "Refresh"))),
    h("main", { class: "admin-main" },
      h("div", { id: "admin-message", class: "admin-message", hidden: true, role: "alert" }),
      h("div", { id: "admin-content" },
        h("div", { id: "admin-cards", class: "admin-cards" }),
        h("p", { id: "admin-holdings-note", class: "admin-note", hidden: true }, "The game loop did not answer in time, so holdings are missing from this refresh."),
        h("section", { id: "admin-funnel", class: "admin-players", hidden: true }),
        h("section", { id: "admin-players", class: "admin-players" },
          h("div", { class: "admin-toolbar" },
            h("h2", null, "Players ", h("small", { id: "admin-players-count", class: "admin-muted" })),
            h("span", { id: "admin-stage-filter" }),
            h("div", { class: "admin-filters", role: "group", "aria-label": "Show" }, filters.map(([value, label]) =>
              h("button", { type: "button", "data-filter": value, "aria-pressed": String(state.filter === value), onclick: () => { state.filter = value; if (state.overview) renderPlayers(state.overview, Date.now()); } }, label))),
            h("input", {
              type: "search", placeholder: "Search name, login, IP or device", "aria-label": "Search players",
              oninput: (event: Event) => { state.query = (event.target as HTMLInputElement).value; if (state.overview) renderPlayers(state.overview, Date.now()); },
            })),
          h("div", { class: "admin-table-wrap" },
            h("table", { class: "admin-table" },
              h("thead", { id: "admin-players-head" }),
              h("tbody", { id: "admin-players-body" }))),
          h("p", { id: "admin-players-empty", class: "admin-muted", hidden: true }, "No players match.")),
        h("section", { id: "admin-unlinked", class: "admin-players", hidden: true },
          h("div", { class: "admin-toolbar" }, h("h2", null, "Corporations without an account")),
          h("div", { class: "admin-table-wrap" },
            h("table", { class: "admin-table" },
              h("thead", null, h("tr", null, ["Corporation", "Home", "Systems", "Worlds", "Population", "Ships", "Credits", "Valuation"]
                .map((label, i) => h("th", { class: i >= 2 ? "num" : undefined, scope: "col" }, label)))),
              h("tbody", { id: "admin-unlinked-body" })))))),
    h("div", { id: "admin-scrim", class: "admin-scrim", hidden: true, onclick: closePlayer }),
    h("aside", { id: "admin-drawer", class: "admin-drawer", "aria-label": "Player details" }, h("div", { id: "admin-drawer-body" })),
  );
  renderHead();
  document.addEventListener("keydown", event => { if (event.key === "Escape" && state.open) closePlayer(); });
}

build();
const linked = location.hash.match(/^#player=([0-9a-f-]{36})$/i)?.[1];
if (linked) openPlayer(linked);
void refresh();
// Keep retrying through a server restart; stop only when signing in or
// operator access is what is missing.
const STOPS = new Set(["sign_in", "not_operator", "off"]);
setInterval(() => {
  if (state.auto && document.visibilityState === "visible" && !STOPS.has(state.error?.code ?? "")) void refresh();
}, REFRESH_MS);
setInterval(() => renderStatus(Date.now()), 5_000);
