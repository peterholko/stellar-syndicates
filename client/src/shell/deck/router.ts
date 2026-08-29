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

export const DECK_ROUTES: Record<DeckRouteName, DeckRouteMeta> = {
  command: { title: "Command", width: "standard" },
  system: { title: "System", width: "standard" },
  world: { title: "World", width: "standard" },
  build: { title: "Build", width: "wide" },
  fleet: { title: "Fleet", width: "standard" },
  fleets: { title: "Fleets", width: "standard" },
  logistics: { title: "Logistics", width: "standard" },
  doctrine: { title: "Doctrine", width: "standard" },
  market: { title: "Market", width: "wide" },
  research: { title: "Research", width: "wide" },
  officers: { title: "Officers", width: "wide" },
  operations: { title: "Operations", width: "standard" },
  syndicate: { title: "Syndicate", width: "standard" },
  faction: { title: "Faction", width: "standard" },
  rankings: { title: "Rankings", width: "wide" },
  log: { title: "Log", width: "standard" },
  battle: { title: "Battle", width: "standard" },
};

export class DeckRouter {
  current: DeckRoute | null = null;

  go(route: DeckRoute): void {
    this.current = route;
  }

  back(): void {
    this.current = null;
  }

  close(): void {
    this.current = null;
  }

  teardown(): void {
    this.current = null;
  }
}
