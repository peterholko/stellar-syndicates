import type { ClientMsg, ServerMsg } from "./protocol";

export const PROTOCOL_VERSION: number;
export const SUBPROTOCOL: string;
export const PROTOCOL_CLOSE_CODE: number;
export const MAX_CLIENT_FRAME: number;
export const MAX_SERVER_FRAME: number;
export function encodeMessage(message: ClientMsg): Uint8Array<ArrayBuffer>;
export function decodeMessage(data: unknown): ServerMsg;
