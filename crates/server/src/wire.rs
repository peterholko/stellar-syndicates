//! Binary WebSocket boundary, deliberately downstream of all visibility gates.
//! One frame = `SS` + protocol version (u16 BE) + one named MessagePack value.
//! Named maps preserve optional/omitted fields and string IDs without a second
//! schema. Float precision is preserved (no position/time quantization).
//! The codec is stateless:
//! dropping a replaceable View cannot desynchronize the next frame, and reliable
//! battle/order increments keep their existing queue/cursor semantics.
//!
//! Only explicit ServerMsg DTOs enter the encoder, never World or battle archives.
//! Database snapshots/events stay JSON; this is a transport change, not a migration.

use std::{fmt, io::Cursor};

use serde::{
    Deserialize, Serialize,
    de::{self, MapAccess, SeqAccess, Visitor},
};

use crate::protocol::{ClientMsg, PROTOCOL_VERSION, ServerMsg};

pub const SUBPROTOCOL: &str = "stellar.msgpack.v32";
pub const PROTOCOL_CLOSE_CODE: u16 = 4002;
pub const MAX_CLIENT_FRAME: usize = 64 * 1024;
const MAX_DEPTH: usize = 32;
const HEADER: [u8; 4] = [
    b'S',
    b'S',
    (PROTOCOL_VERSION >> 8) as u8,
    PROTOCOL_VERSION as u8,
];

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("incompatible network protocol; reload the game")]
    Version,
    #[error("order frame exceeds the 64 KiB limit")]
    TooLarge,
    #[error("expected exactly one message per frame")]
    Trailing,
    #[error("invalid binary order: {0}")]
    Payload(#[from] rmp_serde::decode::Error),
}

pub fn encode_server(msg: &ServerMsg) -> Result<Vec<u8>, rmp_serde::encode::Error> {
    let mut bytes = Vec::with_capacity(1024);
    bytes.extend_from_slice(&HEADER);
    // Human-readable selects the SAME serde ID/enum representations as the
    // old JSON DTO, not a JSON intermediate or a textual payload.
    msg.serialize(
        &mut rmp_serde::Serializer::new(&mut bytes)
            .with_struct_map()
            .with_human_readable(),
    )?;
    Ok(bytes)
}

pub fn decode_client(bytes: &[u8]) -> Result<ClientMsg, DecodeError> {
    if bytes.len() > MAX_CLIENT_FRAME {
        return Err(DecodeError::TooLarge);
    }
    if !bytes.starts_with(&HEADER) {
        return Err(DecodeError::Version);
    }
    let payload = &bytes[HEADER.len()..];
    // MessagePack supports NaN/Infinity but JSON orders never did. Reject them
    // even in ignored fields before an intent can poison positions or prices.
    // This pass builds no object tree, bounds nesting and rejects trailing values.
    let mut checked = rmp_serde::Deserializer::new(Cursor::new(payload)).with_human_readable();
    checked.set_max_depth(MAX_DEPTH);
    Checked::deserialize(&mut checked)?;
    if checked.get_ref().position() != payload.len() as u64 {
        return Err(DecodeError::Trailing);
    }
    let mut decoder = rmp_serde::Deserializer::new(Cursor::new(payload)).with_human_readable();
    decoder.set_max_depth(MAX_DEPTH);
    Ok(ClientMsg::deserialize(&mut decoder)?)
}

struct Checked;
impl<'de> Deserialize<'de> for Checked {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Check;
        impl<'de> Visitor<'de> for Check {
            type Value = Checked;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a finite, bounded message value")
            }
            fn visit_bool<E: de::Error>(self, _: bool) -> Result<Checked, E> {
                Ok(Checked)
            }
            fn visit_unit<E: de::Error>(self) -> Result<Checked, E> {
                Ok(Checked)
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<Checked, E> {
                Ok(Checked)
            }
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<Checked, E> {
                Ok(Checked)
            }
            fn visit_f64<E: de::Error>(self, n: f64) -> Result<Checked, E> {
                if n.is_finite() {
                    Ok(Checked)
                } else {
                    Err(E::custom("non-finite number"))
                }
            }
            fn visit_f32<E: de::Error>(self, n: f32) -> Result<Checked, E> {
                self.visit_f64(f64::from(n))
            }
            fn visit_str<E: de::Error>(self, _: &str) -> Result<Checked, E> {
                Ok(Checked)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Checked, A::Error> {
                while seq.next_element::<Checked>()?.is_some() {}
                Ok(Checked)
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Checked, A::Error> {
                while map.next_entry::<Checked, Checked>()?.is_some() {}
                Ok(Checked)
            }
        }
        d.deserialize_any(Check)
    }
}

#[cfg(test)]
mod tests;
