// Isolated preview of the charter's story pop-ups: no socket, no game state.
import "../src/styles/tokens.css";
import "../src/styles/settlement.css";
import { FOLLOW_UP_MS, freeTraderEvent, piratesEvent, StoryEventUI } from "../src/story-events";

const ui = new StoryEventUI();
const raid = () => piratesEvent({ semiconductors: 40, armaments: 15 });
// In the game the trader shows on the server's docking line, no sooner than
// FOLLOW_UP_MS after the last pop-up closed; here it docks at once.
const show = () => {
  ui.show(raid());
  ui.showAfterGap(freeTraderEvent(), FOLLOW_UP_MS);
};
document.getElementById("fixture-pirates")?.addEventListener("click", show);
document.getElementById("fixture-trader")?.addEventListener("click", () => ui.show(freeTraderEvent()));
show();
