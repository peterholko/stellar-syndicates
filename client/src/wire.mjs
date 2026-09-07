// Shared by the browser and Node smoke clients. One WebSocket frame holds
// one self-contained MessagePack message: no cross-frame dictionary or delta
// state that could break when the server replaces an unsent View.
import { Decoder, Encoder } from "@msgpack/msgpack";

export const PROTOCOL_VERSION = 32;
export const SUBPROTOCOL = `stellar.msgpack.v${PROTOCOL_VERSION}`;
export const PROTOCOL_CLOSE_CODE = 4002;
export const MAX_CLIENT_FRAME = 64 * 1024;
export const MAX_SERVER_FRAME = 64 * 1024 * 1024;
const HEADER = new Uint8Array([0x53, 0x53, PROTOCOL_VERSION >> 8, PROTOCOL_VERSION & 0xff]);
// Undefined object fields stay omitted, as with JSON. Never force f32: served
// times/positions need their original precision. 64-bit IDs remain strings.
const encoder = new Encoder({ ignoreUndefined: true, maxDepth: 32 });
const decoder = new Decoder({
  maxStrLength: MAX_SERVER_FRAME,
  maxBinLength: 0,
  maxExtLength: 0,
  maxArrayLength: 1_000_000,
  maxMapLength: 100_000,
});

export function encodeMessage(message) {
  checkNumbers(message);
  const packed = encoder.encode(message);
  if (packed.length + HEADER.length > MAX_CLIENT_FRAME) throw new Error("Order is too large to send.");
  const frame = new Uint8Array(HEADER.length + packed.length);
  frame.set(HEADER);
  frame.set(packed, HEADER.length);
  return frame;
}

function checkNumbers(value, depth = 0) {
  if (depth > 32) throw new Error("Order is nested too deeply.");
  if (typeof value === "number" && !Number.isFinite(value)) throw new Error("Order contains an invalid number.");
  if (value && typeof value === "object") {
    for (const item of Object.values(value)) checkNumbers(item, depth + 1);
  }
}

export function decodeMessage(data) {
  const frame = data instanceof ArrayBuffer ? new Uint8Array(data)
    : ArrayBuffer.isView(data) ? new Uint8Array(data.buffer, data.byteOffset, data.byteLength) : null;
  if (!frame || frame.length < HEADER.length || HEADER.some((byte, i) => frame[i] !== byte)) {
    throw new Error("Incompatible network protocol. Reload the game.");
  }
  if (frame.length > MAX_SERVER_FRAME) throw new Error("Server frame exceeds the size limit.");
  // decode(), not decodeMulti(): truncated or trailing payloads are errors.
  const message = decoder.decode(frame.subarray(HEADER.length));
  if (!message || typeof message !== "object" || Array.isArray(message) || typeof message.type !== "string") {
    throw new Error("Invalid server message.");
  }
  return message;
}
