import { bodyPoolUsage, POOL_OF, type Pool } from "../../core/derive/market";
import type { PlanetPanelModel } from "./planet";

export const SITE_POOLS: readonly Pool[] = ["resource", "industrial", "infrastructure"];
export const isSitePool = (value: unknown): value is Pool => SITE_POOLS.includes(value as Pool);
export interface PlanetSite { slot: number; pool: Pool; key?: string }
type Layout = { sites: PlanetSite[]; preferred: Record<string, number> };

/** Cosmetic addresses only, never a construction ledger. The arrived report alone
 * decides occupancy and capacity. Save the layout like a panel preference so new
 * slots, queued→built transitions and page reloads cannot shuffle a colony.
 * Existing buildings are seeded at their original authored addresses. */
export class PlanetSites {
  private layouts = new Map<string, Layout>();
  constructor(private storage?: Pick<Storage, "getItem" | "setItem">) {}

  private id(model: PlanetPanelModel): string {
    return `stellar.planet-sites.v1:${JSON.stringify([model.report.owner, model.system.id, model.body.id, model.system.pos])}`;
  }

  private load(model: PlanetPanelModel): Layout {
    const id = this.id(model);
    const cached = this.layouts.get(id);
    if (cached) return cached;
    let layout: Layout = { sites: [], preferred: {} };
    try {
      const saved = JSON.parse((this.storage ?? localStorage).getItem(id) ?? "null") as Layout | null;
      if (saved && Array.isArray(saved.sites) && saved.sites.length <= 24
        && saved.sites.every(s => s && isSitePool(s.pool) && Number.isInteger(s.slot) && s.slot >= 0 && s.slot < 24
          && (s.key === undefined || POOL_OF[s.key] === s.pool))
        && new Set(saved.sites.map(s => s.slot)).size === saved.sites.length
        && new Set(saved.sites.filter(s => s.key).map(s => s.key)).size === saved.sites.filter(s => s.key).length) {
        layout.sites = saved.sites;
        for (const [key, slot] of Object.entries(saved.preferred ?? {})) {
          if (isSitePool(POOL_OF[key]) && Number.isInteger(slot) && slot >= 0 && slot < 24) layout.preferred[key] = slot;
        }
      }
    } catch { /* Private browsing or old/corrupt preferences: derive from the report. */ }
    this.layouts.set(id, layout);
    return layout;
  }

  private save(model: PlanetPanelModel, layout: Layout): void {
    try { (this.storage ?? localStorage).setItem(this.id(model), JSON.stringify(layout)); } catch { /* Optional UI preference. */ }
  }

  snapshot(model: PlanetPanelModel): { occupied: PlanetSite[]; free: PlanetSite[] } {
    if (!model.mine) return { occupied: [], free: [] };
    const layout = this.load(model), before = JSON.stringify(layout);
    const order = Object.keys(model.descriptions);
    const jobs = model.report.builds.filter(job => job.body_id === model.body.id && isSitePool(POOL_OF[job.key]));
    const known = new Set([...Object.keys(model.body.structures).filter(key => model.body.structures[key] > 0), ...jobs.map(job => job.key)]);
    const usage = bodyPoolUsage(model.body, model.report);
    const firstReport = layout.sites.length === 0;
    for (const site of layout.sites) if (site.key && !known.has(site.key)) delete site.key;
    const allocate = (pool: Pool, preferred?: number, occupied = false): PlanetSite | undefined => {
      const used = new Set(layout.sites.map(s => s.slot));
      const slot = preferred !== undefined && preferred >= 0 && preferred < 24 && !used.has(preferred)
        ? preferred : Array.from({ length: 24 }, (_, i) => i).find(i => !used.has(i));
      if (slot === undefined) {
        // At the artwork's limit, reclaim only an empty address. Reported
        // buildings take priority; capacity growth may reuse hidden surplus
        // from a contracted pool, but never steal another pool's active sites.
        const spare = layout.sites.find(s => !s.key && s.pool !== pool && (occupied
          || layout.sites.filter(other => other.pool === s.pool).length > usage[s.pool].total));
        if (spare) spare.pool = pool;
        return spare;
      }
      const site = { pool, slot };
      layout.sites.push(site);
      return site;
    };
    for (const key of known) {
      const pool = POOL_OF[key];
      if (!isSitePool(pool) || layout.sites.some(s => s.key === key)) continue;
      const site = firstReport ? allocate(pool, order.indexOf(key), true)
        : layout.sites.find(s => !s.key && s.pool === pool && s.slot === layout.preferred[key])
          ?? layout.sites.find(s => !s.key && s.pool === pool) ?? allocate(pool, undefined, true);
      if (site) site.key = key;
      delete layout.preferred[key];
    }
    const free: PlanetSite[] = [];
    for (const pool of SITE_POOLS) {
      // Growth reveals additional sites, never repositions old ones. Contraction
      // hides surplus empty sites without forgetting their addresses.
      while (layout.sites.filter(s => s.pool === pool).length < usage[pool].total) {
        if (!allocate(pool)) break;
      }
      free.push(...layout.sites.filter(s => s.pool === pool && !s.key).slice(0, Math.max(0, usage[pool].total - usage[pool].used)));
    }
    if (JSON.stringify(layout) !== before) this.save(model, layout);
    return { occupied: layout.sites.filter(s => s.key && known.has(s.key)), free };
  }

  /** Remember the clicked address, NOT an optimistic job. No outline or slot
   * consumption appears until the command's construction report arrives. */
  prefer(model: PlanetPanelModel, key: string, slot: number): void {
    if (!this.snapshot(model).free.some(s => s.slot === slot && s.pool === POOL_OF[key])) return;
    const layout = this.load(model);
    layout.preferred[key] = slot;
    this.save(model, layout);
  }
}
