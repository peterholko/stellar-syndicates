import type { GhostView, InterdictionFieldView, Vec2 } from "../../protocol";
import type { ViewState } from "../../state";

/** Interdiction fields (battle rules v11). Mirrors crates/sim/src/interdiction.rs:
 * a destroyer carrying an Interdiction Field raises a temporary gravity well
 * where it stands. Fleets inside move on thrusters and can't jump. */
export const FIELD_RADIUS_SU = 1_500;
export const FIELD_SECONDS = 600;
export const FIELD_COOLDOWN_S = 1_800;
export const FIELD_FUEL = 20;

/** An own fleet with a destroyer carrying the generator, as received. */
export const interdictorCapable = (g: GhostView): boolean => g.own && !g.tca
  && !!g.loadouts?.some(s => s.kind === "destroyer" && s.n > 0 && s.modules.includes("interdiction_field"));

/** The arrived field covering `pos`, if any. */
export function fieldAt(pos: Vec2, st: ViewState): InterdictionFieldView | undefined {
  return (st.interdictionFields ?? []).find(f => Math.hypot(pos.x - f.pos.x, pos.y - f.pos.y) <= f.radius);
}

/** The gravity well (a system's, the Hub's, or an arrived field) covering `pos`. */
function wellAt(pos: Vec2, st: ViewState): string | null {
  const galaxy = st.galaxy;
  const near = (p: Vec2, r: number) => Math.hypot(pos.x - p.x, pos.y - p.y) < r;
  if (galaxy && near(galaxy.hub, galaxy.hyperlimit)) return "the Market Hub's gravity well";
  const system = galaxy?.systems.find(s => near(s.pos, galaxy.hyperlimit));
  if (system) return `${system.name}'s gravity well`;
  return fieldAt(pos, st) ? "an interdiction field" : null;
}

/** Why the panel can't stage the order now (empty: it can). The sim checks
 * everything again when the order reaches the fleet. */
export function interdictionReason(g: GhostView, st: ViewState): string {
  if (!interdictorCapable(g)) return "Needs a destroyer carrying an Interdiction Field.";
  const well = wellAt(g.pos, st);
  if (well) return `Served sighting is inside ${well}.`;
  const ready = g.interdiction_ready ?? null;
  const now = st.simTime;
  if (ready !== null && ready > now) return `Generator recharging · ready in ${Math.ceil((ready - now) / 60)} min.`;
  if (typeof g.fuel === "number" && g.fuel < FIELD_FUEL) return `Needs ${FIELD_FUEL} Deuterium in the tanks.`;
  return "";
}
