// Isolated preview of the client-update pill (src/client-update.ts): no socket,
// no game. ?need=required shows the protocol case; Reload goes to the console.
import "../src/styles/tokens.css";
import { showUpdatePill } from "../src/client-update";

const show = (need: "offer" | "required") => showUpdatePill(need, () => console.log("reload"));
show(new URLSearchParams(location.search).get("need") === "required" ? "required" : "offer");
for (const button of document.querySelectorAll<HTMLButtonElement>("[data-fixture]")) {
  button.addEventListener("click", () => show(button.dataset.fixture === "required" ? "required" : "offer"));
}
