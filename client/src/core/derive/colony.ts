import type { BodyView, ColonyOpportunityView, Commodity, SystemStateView } from "../../protocol";

// Mirrors production.rs::CONVERTERS' input identities (not rates). Used only to
// explain supply chains; actual output and colony-role scores come from reports.
export const PRODUCTION_INPUTS: Record<string, Commodity[]> = {
  smelter: ["metallic_ore", "fuel"], electronics_fabricator: ["rare_elements", "silicates"],
  chemical_works: ["volatiles", "biomass"], fuel_refinery: ["volatiles"], agroplex: ["biomass"],
  machine_works: ["alloys", "electronics", "fuel"], armaments_complex: ["alloys", "electronics", "polymers"],
};
const ROLE_PRODUCT: Record<ColonyOpportunityView["role"], Commodity[]> = {
  mining_world: ["metallic_ore", "rare_elements"], fuel_complex: ["fuel"],
  electronics_center: ["electronics"], agricultural_exporter: ["provisions"],
  population_world: [], shipbuilding_center: [], strategic_outpost: [],
};
const ROLE_NAME: Record<ColonyOpportunityView["role"], string> = {
  mining_world: "mining", fuel_complex: "fuel production", electronics_center: "electronics",
  agricultural_exporter: "agriculture", population_world: "population capacity",
  shipbuilding_center: "shipbuilding", strategic_outpost: "jump staging",
};
const ROLE_INPUTS: Partial<Record<ColonyOpportunityView["role"], Commodity[]>> = {
  electronics_center: PRODUCTION_INPUTS.electronics_fabricator,
  fuel_complex: PRODUCTION_INPUTS.fuel_refinery,
  agricultural_exporter: PRODUCTION_INPUTS.agroplex,
  shipbuilding_center: ["alloys", "machinery", "electronics"],
};

export interface ColonyPurpose {
  headline: string;
  role: ColonyOpportunityView["role"] | null;
  homeNeed: string;
  exports: Commodity[];
  imports: Commodity[];
  advantages: string;
}

/** A prospect recommendation is a comparison of ARRIVED surveys and the home
 * report, never current remote ownership/inventory or a client movement guess.
 * No opportunity label before a survey. Absence of a positive role alone does
 * not prove weakness: poor agriculture requires an observed lack of Biomass. */
export function colonyPurpose(candidate: SystemStateView, home?: SystemStateView,
  body?: BodyView): ColonyPurpose | null {
  const bodies = body ? [body] : candidate.bodies;
  if (!bodies.length || bodies.some(b => b.deposits == null)) return null;
  const deposits = bodies.flatMap(b => b.deposits ?? []).filter(d => d.reserves !== 0);
  const available = new Set(deposits.map(d => d.resource));
  const roles = (candidate.opportunities ?? []).filter(r => !body || r.body_id === body.id)
    .sort((a, b) => b.score - a.score || a.role.localeCompare(b.role));
  const role = roles[0];
  const poorFood = !available.has("biomass");
  // Stockpiles are shared across a system. A barren moon need not import food
  // across space if another surveyed world in the same colony can supply it.
  const systemFeedstock = new Set(candidate.bodies.flatMap(b => b.deposits ?? [])
    .filter(d => d.reserves !== 0).map(d => d.resource));
  const potentialExports = role ? ROLE_PRODUCT[role.role].filter(g =>
    !["metallic_ore", "rare_elements"].includes(g) || available.has(g))
    : [...available].slice(0, 2);
  const imports = new Set<Commodity>();
  if (!systemFeedstock.has("biomass")) imports.add("provisions");
  for (const input of role ? ROLE_INPUTS[role.role] ?? [] : []) {
    if (!systemFeedstock.has(input)) imports.add(input);
  }
  const adjective = role && role.score >= 1.8 ? "Excellent" : "Good";
  const headline = `${role ? `${adjective} ${ROLE_NAME[role.role]}` : "Resource site"}${poorFood ? " · poor agriculture" : " · local food potential"}`;
  let homeNeed = "Compare with a received home report.";
  let advantages = "";
  if (home) {
    const stock = (g: Commodity) => home.stockpile?.find(s => s.commodity === g)?.units;
    const stalled = home.converters?.find(c => c.status === "no_inputs"
      && (PRODUCTION_INPUTS[c.structure] ?? []).some(input => potentialExports.includes(input) && (stock(input) ?? Infinity) < 1));
    const foodNeed = potentialExports.includes("provisions") && !!home.food_state && home.food_state !== "well_supplied";
    if (foodNeed) homeNeed = "Can relieve home Provisions shortages.";
    else if (stalled) homeNeed = `Can feed the home ${stalled.title}'s missing inputs.`;
    else if (role?.role === "population_world" && home.workforce && home.workforce.units <= home.workforce.posted)
      homeNeed = "Room for another workforce; staffing is tight at home.";
    else if (role?.role === "shipbuilding_center" && (home.builds ?? []).some(b =>
      ["builder", "convoy", "raider", "corvette", "colony", "transport", "scout", "destroyer",
        "cruiser", "battlecruiser", "battleship", "carrier", "titan"].includes(b.key)))
      homeNeed = "Adds a second shipbuilding site while home yards are busy.";
    else homeNeed = potentialExports.length ? "Adds a specialist supply line for home industry." : "Adds capacity or reach beyond home.";
    const homeRole = role && home.opportunities?.filter(r => r.role === role.role)
      .sort((a, b) => b.score - a.score)[0];
    if (role && homeRole) advantages = `Natural ${ROLE_NAME[role.role]} rating: ${role.score.toFixed(2)}× · home ${homeRole.score.toFixed(2)}×`;
    else if (role) advantages = `Natural ${ROLE_NAME[role.role]} rating: ${role.score.toFixed(2)}× home reference`;
  }
  return { headline, role: role?.role ?? null, homeNeed, exports: potentialExports, imports: [...imports], advantages };
}
