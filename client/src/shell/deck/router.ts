export type DeckRouteName =
  | "command"
  | "system"
  | "world"
  | "build"
  | "fleet"
  | "fleets"
  | "logistics"
  | "doctrine"
  | "market"
  | "research"
  | "officers"
  | "operations"
  | "syndicate"
  | "faction"
  | "rankings"
  | "log"
  | "battle";

export type DeckWidth = "standard" | "wide";

export interface DeckRoute {
  name: DeckRouteName;
  params?: Record<string, string>;
  query?: Record<string, string>;
}

export interface DeckRouteMeta {
  title: string;
  width: DeckWidth;
}

export interface DeckCrumb {
  label: string;
  route: DeckRoute | null;
}

export const DECK_ROUTES: Record<DeckRouteName, DeckRouteMeta> = {
  command: { title: "Command", width: "standard" },
  system: { title: "System", width: "standard" },
  world: { title: "World", width: "standard" },
  build: { title: "Build", width: "standard" },
  fleet: { title: "Fleet", width: "standard" },
  fleets: { title: "Fleets", width: "standard" },
  logistics: { title: "Logistics", width: "standard" },
  doctrine: { title: "Doctrine", width: "standard" },
  market: { title: "Market Hub", width: "wide" },
  research: { title: "Research", width: "wide" },
  officers: { title: "Officers", width: "wide" },
  operations: { title: "Operations", width: "standard" },
  syndicate: { title: "Syndicate", width: "standard" },
  faction: { title: "Faction", width: "standard" },
  rankings: { title: "Rankings", width: "wide" },
  log: { title: "Log", width: "standard" },
  battle: { title: "Battle", width: "standard" },
};

const HISTORY_KEY = "stellarSyndicatesDeckRoute";
let nextSession = 1;

interface DeckHistoryMarker {
  session: number;
  routes: DeckRoute[];
}

type RouteChange = (route: DeckRoute | null, stack: readonly DeckRoute[]) => void;

/** One route stack owns top-nav, breadcrumb, workspace Back, close, and browser
 * Back. The browser marker includes the routes (not only a depth), so Forward
 * can faithfully restore a Deck workspace after a Back. */
export class DeckRouter {
  private entries: DeckRoute[] = [];
  private readonly session = nextSession++;

  constructor(private readonly onChange: RouteChange, signal: AbortSignal) {
    window.addEventListener("popstate", (event) => this.onPopState(event), { signal });
  }

  get current(): DeckRoute | null {
    return this.entries.at(-1) ?? null;
  }

  go(route: DeckRoute): void {
    const next = cloneRoute(route);
    if (this.current && routeKey(this.current) === routeKey(next)) {
      this.entries[this.entries.length - 1] = next;
      this.replaceMarker();
      this.emit();
      return;
    }
    this.entries.push(next);
    history.pushState({
      ...(typeof history.state === "object" && history.state ? history.state : {}),
      [HISTORY_KEY]: this.marker(),
    }, "");
    this.emit();
  }

  /** Swap the current entry for a sibling (a world switch inside one system)
   * without growing the stack, so Back still returns to the parent. */
  replace(route: DeckRoute): void {
    if (!this.entries.length) {
      this.go(route);
      return;
    }
    this.entries[this.entries.length - 1] = cloneRoute(route);
    this.replaceMarker();
    this.emit();
  }

  back(): void {
    if (!this.entries.length) return;
    if (this.historyMarker()?.session === this.session) history.back();
    else {
      this.entries.pop();
      this.emit();
    }
  }

  close(): void {
    if (!this.entries.length) return;
    const depth = this.entries.length;
    if (this.historyMarker()?.session === this.session) history.go(-depth);
    else {
      this.entries.length = 0;
      this.emit();
    }
  }

  breadcrumbs(route = this.current): DeckCrumb[] {
    if (!route) return [];
    const command: DeckCrumb = { label: "Command", route: { name: "command" } };
    if (route.name === "command") return [command];
    if (route.name === "system" || route.name === "world" || route.name === "build") {
      const systemId = route.params?.systemId ?? route.params?.id ?? "system";
      const systemLabel = route.params?.systemLabel ?? route.params?.systemName ?? "System";
      const systemRoute: DeckRoute = { name: "system", params: { id: systemId, systemLabel } };
      const crumbs: DeckCrumb[] = [{ label: "Galaxy", route: null }, { label: systemLabel, route: systemRoute }];
      if (route.name === "world") {
        crumbs.push({ label: route.params?.worldLabel ?? route.params?.bodyId ?? "World", route });
      } else if (route.name === "build") {
        crumbs.push({ label: "Build", route });
      }
      return crumbs;
    }
    if (route.name === "fleet") {
      return [
        command,
        { label: "Fleets", route: { name: "fleets" } },
        { label: route.params?.fleetLabel ?? route.params?.id ?? "Fleet", route },
      ];
    }
    if (route.name === "logistics" || route.name === "doctrine") {
      return [command, { label: "Fleets", route: { name: "fleets" } }, { label: DECK_ROUTES[route.name].title, route }];
    }
    // Global destinations already name themselves in the workspace title;
    // unlike fleet/system drill-downs they have no useful parent crumb.
    return [{ label: route.params?.label ?? DECK_ROUTES[route.name].title, route }];
  }

  teardown(): void {
    this.entries.length = 0;
  }

  private marker(): DeckHistoryMarker {
    return { session: this.session, routes: this.entries.map(cloneRoute) };
  }

  private replaceMarker(): void {
    history.replaceState({
      ...(typeof history.state === "object" && history.state ? history.state : {}),
      [HISTORY_KEY]: this.marker(),
    }, "");
  }

  private historyMarker(state: unknown = history.state): DeckHistoryMarker | null {
    if (!state || typeof state !== "object") return null;
    const marker = (state as Record<string, unknown>)[HISTORY_KEY];
    if (!marker || typeof marker !== "object") return null;
    const candidate = marker as Partial<DeckHistoryMarker>;
    return typeof candidate.session === "number" && Array.isArray(candidate.routes)
      ? { session: candidate.session, routes: candidate.routes.filter(isDeckRoute).map(cloneRoute) }
      : null;
  }

  private onPopState(event: PopStateEvent): void {
    const marker = this.historyMarker(event.state);
    this.entries = marker?.session === this.session ? marker.routes : [];
    this.emit();
  }

  private emit(): void {
    this.onChange(this.current, this.entries);
  }
}

function cloneRoute(route: DeckRoute): DeckRoute {
  return {
    name: route.name,
    ...(route.params ? { params: { ...route.params } } : {}),
    ...(route.query ? { query: { ...route.query } } : {}),
  };
}

function routeKey(route: DeckRoute): string {
  return JSON.stringify([route.name, sortedEntries(route.params), sortedEntries(route.query)]);
}

function sortedEntries(value?: Record<string, string>): [string, string][] {
  return Object.entries(value ?? {}).sort(([a], [b]) => a.localeCompare(b));
}

function isDeckRoute(value: unknown): value is DeckRoute {
  if (!value || typeof value !== "object") return false;
  const name = (value as { name?: unknown }).name;
  return typeof name === "string" && name in DECK_ROUTES;
}
