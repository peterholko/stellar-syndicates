// Node smoke clients use the SAME binary codec and version as the browser.
// Install client dependencies first: npm --prefix client ci.
import { SUBPROTOCOL } from "../client/src/wire.mjs";
export { encodeMessage, decodeMessage } from "../client/src/wire.mjs";

export function gameSocket(url) {
  const socket = new WebSocket(url, SUBPROTOCOL);
  socket.binaryType = "arraybuffer";
  return socket;
}
