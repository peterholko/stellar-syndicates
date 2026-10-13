// Save-game kinds deliberately differ from the art names. Keep this mapping
// explicit: convoy is Medium freight; freighter is the Authority hull.
export const SHIP_MODELS = [
  ["raider", "interceptor", "Interceptor"],
  ["corvette", "corvette", "Corvette"],
  ["destroyer", "destroyer", "Destroyer"],
  ["cruiser", "cruiser", "Cruiser"],
  ["battleship", "battleship", "Battleship"],
  ["dreadnought", "dreadnought", "Dreadnought"],
  ["titan", "titan", "Titan"],
  ["scout", "scout", "Scout"],
  ["colony", "colony-ship", "Colony Ship"],
  ["transport", "troop-transport", "Troop Transport"],
  ["builder", "construction-ship", "Construction Ship"],
  ["mining_barge", "mining-barge", "Mining Barge"],
  ["mining_belt_station", "mining-belt-station", "Mining Belt Station"],
  ["tiny_freighter", "tiny-freighter", "Tiny Freighter"],
  ["small_freighter", "small-freighter", "Small Freighter"],
  ["convoy", "medium-freighter", "Medium Freighter"],
  ["large_freighter", "large-freighter", "Large Freighter"],
  ["heavy_freighter", "heavy-freighter", "Heavy Freighter"],
  ["bulk_freighter", "bulk-freighter", "Bulk Freighter"],
  ["freighter", "authority-freighter", "Authority Freighter"],
].map(([kind, id, title]) => ({ kind, id, title }));

// A hull whose own art is not generated yet borrows a sibling's sprite, model
// and hardpoints under its own name (kind → the kind it borrows from). Remove
// the entry once the hull's art lands in the lists above.
export const ART_STAND_INS = {
  survey_probe: { from: "scout", title: "Survey Probe" },
};
