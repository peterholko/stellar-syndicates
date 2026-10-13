// Mirrors the three-level Welcome contract; the server suite verifies that
// contract against the authoritative recipes. Used only by isolated UI tests.
export const commandCenter = {
  key: "command_center", label: "Command Center", costs: [], build_secs: 180,
  consumes_slot: false, min_population: 1000,
  tiers: [
    { tier: 1, label: "Landing Command", build_secs: 180, research_prerequisite: null,
      costs: [{ commodity: "alloys", units: 15 }, { commodity: "machinery", units: 5 }],
      description: "Landing administration. Included free on settlement. No workforce or extra building slot." },
    { tier: 2, label: "Colony Command Center", build_secs: 300, research_prerequisite: null,
      costs: [{ commodity: "alloys", units: 40 }, { commodity: "machinery", units: 12 }, { commodity: "electronics", units: 8 }],
      description: "+1 Infrastructure slot and 10% shorter structure construction on this world. No workforce or extra building slot." },
    { tier: 3, label: "Syndicate Headquarters", build_secs: 900, research_prerequisite: "mat_prefab_construction",
      costs: [{ commodity: "alloys", units: 120 }, { commodity: "machinery", units: 40 }, { commodity: "electronics", units: 30 }, { commodity: "polymers", units: 20 }],
      description: "+2 Infrastructure slots and 20% shorter structure construction on this world (total bonuses, not cumulative). No workforce or extra building slot." },
  ],
};
