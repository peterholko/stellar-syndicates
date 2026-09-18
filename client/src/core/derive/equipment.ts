import type { GhostView, ModuleKind, ShipKind } from "../../protocol";
import { isPlayerFreighter } from "../../protocol";
import type { IconKey } from "../../icons";

/** Physical equipment catalog shared by both shells. Research unlocks making
 * crates, not fitting recovered ones. No entry changes the speed of reports. */
export const MODULES: { kind: ModuleKind; name: string; label: string; icon: IconKey; fit: number; role: string }[] = [
  { kind: "mass_driver", name: "Mass Driver", icon: "moduleMassDriver", fit: 2, role: "Kinetic weapon" },
  { kind: "torpedo_rack", name: "Torpedo Rack", icon: "moduleTorpedoRack", fit: 3, role: "Heavy strike" },
  { kind: "point_defense_screen", name: "Point-Defense Screen", icon: "modulePointDefense", fit: 2, role: "Torpedo interception" },
  { kind: "reflective_plating", name: "Reflective Plating", icon: "moduleReflectivePlating", fit: 2, role: "Beam protection" },
  { kind: "whipple_armor", name: "Whipple Armor", icon: "moduleWhippleArmor", fit: 3, role: "Kinetic protection" },
  { kind: "extended_tanks", name: "Extended Tanks", icon: "moduleExtendedTanks", fit: 2, role: "+75% tank capacity · fuel not included" },
  { kind: "recon_suite", name: "Recon Suite", icon: "moduleReconSuite", fit: 2, role: "Scout contacts 30k → 60k · sensors 40k; Interceptor sensors 120k su" },
  { kind: "cargo_pods", name: "Cargo Pods", icon: "moduleCargoPods", fit: 2, role: "Doubles this hull's cargo hold · replaces Extended Tanks" },
  { kind: "escort_datalink", name: "Escort Datalink", icon: "moduleEscortDatalink", fit: 2, role: "Corvette · requires Point-Defense Screen · priority intercept at 2× PD reach" },
  { kind: "fuel_transfer_rig", name: "Fuel Transfer Rig", icon: "moduleFuelTransferRig", fit: 2, role: "Freighter · refuels owned fleets from Fuel cargo · replaces Cargo Pods / Extended Tanks" },
  { kind: "survey_drive", name: "Survey Drive", icon: "moduleSurveyDrive", fit: 2, role: "Blueprint · Scout speed +25%, tank capacity −40% · no change to jump timing" },
  { kind: "nebula_spectrometer", name: "Nebula Spectrometer", icon: "moduleNebulaSpectrometer", fit: 2, role: "Blueprint · Scout site contacts 90k su before terrain · deep investigation · no combat sensor" },
  { kind: "prismatic_lance", name: "Prismatic Lance", icon: "modulePrismaticLance", fit: 3, role: "Blueprint · beam damage ×1.45 · countered by Reflective Plating" },
].map(entry => ({ ...entry, kind: entry.kind as ModuleKind, icon: entry.icon as IconKey, label: entry.name }));

export const isBlueprintOnly = (module: ModuleKind): boolean => ["survey_drive", "nebula_spectrometer", "prismatic_lance"].includes(module);
export const isUtility = (module: ModuleKind): boolean => module === "extended_tanks" || module === "recon_suite" || module === "cargo_pods" || module === "escort_datalink" || module === "fuel_transfer_rig" || module === "survey_drive" || module === "nebula_spectrometer";
export const utilityProgramme = (module: ModuleKind): string | null => module === "extended_tanks"
  ? "prop_expedition_iii_extended_tanks" : module === "recon_suite" ? "comp_shadow_iv_recon_suite"
    : module === "cargo_pods" ? "hull_cargo_pods" : module === "escort_datalink" ? "hull_line_iii_escort_datalink"
      : module === "fuel_transfer_rig" ? "prop_expedition_iv_fleet_tenders" : null;
export const moduleFitsHull = (module: ModuleKind, hull: string): boolean => module === "extended_tanks"
  ? ["scout", "raider", "corvette"].includes(hull) || isPlayerFreighter(hull as ShipKind)
  : module === "recon_suite" ? ["scout", "raider"].includes(hull)
    : module === "survey_drive" || module === "nebula_spectrometer" ? hull === "scout"
      : module === "cargo_pods" || module === "fuel_transfer_rig" ? isPlayerFreighter(hull as ShipKind) : module === "escort_datalink" ? hull === "corvette" : !isPlayerFreighter(hull as ShipKind);
export const tankMultiplier = (modules: ModuleKind[]): number => (modules.includes("extended_tanks") ? 1.75 : 1) * (modules.includes("survey_drive") ? .6 : 1);
export const cargoMultiplier = (modules: ModuleKind[]): number => modules.includes("cargo_pods") ? 2 : 1;
export const utilityChange = (from: ModuleKind[], to: ModuleKind[]): boolean =>
  from.filter(m => !isUtility(m)).sort().join(",") === to.filter(m => !isUtility(m)).sort().join(",");

/** Only arrived fitting data places a sensor circle. A prospective fit never
 * changes the map, and a jump presumption does not establish a sensor source. */
export function sensorMultiplier(g: GhostView, scoutMult = 1.5, convoyMult = .25): number {
  const composition = g.composition?.length ? g.composition : [{ kind: g.kind, count: 1 }];
  const has = (kind: ShipKind) => composition.some(s => s.kind === kind && s.count > 0);
  const recon = (kind: ShipKind) => (g.loadouts ?? []).some(s => s.kind === kind && s.n > 0 && s.modules.includes("recon_suite"));
  const stock = has("raider") ? (has("scout") ? scoutMult : 1) : composition.some(s => isPlayerFreighter(s.kind) && s.count > 0) ? convoyMult : 0;
  return Math.max(stock, recon("raider") ? 1.5 : recon("scout") ? .5 : 0);
}

export function hullUtilitySummary(kind: ShipKind, from: ModuleKind[], to: ModuleKind[]): string {
  const parts: string[] = [];
  if (from.includes("survey_drive") !== to.includes("survey_drive")) parts.push(to.includes("survey_drive") ? "Scout travel +25% · smaller tanks" : "Standard Scout travel speed");
  if (from.includes("nebula_spectrometer") !== to.includes("nebula_spectrometer")) parts.push(to.includes("nebula_spectrometer") ? "Site contacts 90k su · deep investigation" : "Specialist site scanner removed");
  if (from.includes("fuel_transfer_rig") !== to.includes("fuel_transfer_rig")) parts.push(to.includes("fuel_transfer_rig")
    ? "Mobile refueling · uses Fuel cargo, not propulsion tanks" : "Mobile refueling removed");
  if (from.includes("escort_datalink") !== to.includes("escort_datalink")) parts.push(to.includes("escort_datalink")
    ? "Guard-target screening · one extra intercept/step at 2× PD reach" : "Standard point defense");
  if (tankMultiplier(from) !== tankMultiplier(to)) parts.push(`Tank capacity ${Math.round(tankMultiplier(from) * 100)}% → ${Math.round(tankMultiplier(to) * 100)}%`);
  if (from.includes("recon_suite") !== to.includes("recon_suite")) {
    const range = (mods: ModuleKind[]) => kind === "scout" ? (mods.includes("recon_suite") ? 40 : 0) : (mods.includes("recon_suite") ? 120 : 80);
    parts.push(`Sensors ${range(from)}k → ${range(to)}k su`);
    if (kind === "scout") parts.push(`Site contacts ${from.includes("recon_suite") ? 60 : 30}k → ${to.includes("recon_suite") ? 60 : 30}k su`);
    parts.push(`Threat lookout ${from.includes("recon_suite") ? 40 : 20}k → ${to.includes("recon_suite") ? 40 : 20}k su`);
  }
  return parts.join(" · ");
}
