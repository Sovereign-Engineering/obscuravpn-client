const WG_HANDSHAKE_INITIATION_TYPE: u8 = 1;
const WG_HANDSHAKE_RESPONSE_TYPE: u8 = 2;
const WG_COOKIE_REPLY_TYPE: u8 = 3;
const WG_DATA_TYPE: u8 = 4;

const WG_KEEPALIVE_LEN: usize = 32;

#[derive(Clone, Copy, Debug)]
pub enum WgMessageDirection {
    Received,
    Sent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WgControlMessageHeader {
    HandshakeInitiation { sender_index: u32 },
    HandshakeResponse { sender_index: u32, receiver_index: u32 },
    CookieReply { receiver_index: u32 },
    Keepalive { receiver_index: u32 },
}

impl WgControlMessageHeader {
    pub fn parse(wg_message: &[u8]) -> Option<Self> {
        let mut u32_chunks = wg_message.as_chunks::<4>().0.iter();
        let message_type = u32_chunks.next()?[0];
        let mut u32s = u32_chunks.copied().map(u32::from_le_bytes);
        match (message_type, wg_message.len()) {
            (WG_HANDSHAKE_INITIATION_TYPE, _) => {
                let sender_index = u32s.next()?;
                Some(Self::HandshakeInitiation { sender_index })
            }
            (WG_HANDSHAKE_RESPONSE_TYPE, _) => {
                let sender_index = u32s.next()?;
                let receiver_index = u32s.next()?;
                Some(Self::HandshakeResponse { sender_index, receiver_index })
            }
            (WG_COOKIE_REPLY_TYPE, _) => {
                let receiver_index = u32s.next()?;
                Some(Self::CookieReply { receiver_index })
            }
            (WG_DATA_TYPE, WG_KEEPALIVE_LEN) => {
                let receiver_index = u32s.next()?;
                Some(Self::Keepalive { receiver_index })
            }
            (_, _) => None,
        }
    }

    pub fn log(self, direction: WgMessageDirection) {
        match self {
            Self::HandshakeInitiation { sender_index } => {
                tracing::info!(message_id = "HvdLOdJO", ?direction, sender_index, "wireguard handshake initiation")
            }
            Self::HandshakeResponse { sender_index, receiver_index } => {
                tracing::info!(
                    message_id = "SGaARr9p",
                    ?direction,
                    sender_index,
                    receiver_index,
                    "wireguard handshake response"
                )
            }
            Self::CookieReply { receiver_index } => {
                tracing::info!(message_id = "ZCL3noOc", ?direction, receiver_index, "wireguard cookie reply")
            }
            Self::Keepalive { receiver_index } => {
                tracing::info!(message_id = "ZXyH5sYx", ?direction, receiver_index, "wireguard keepalive")
            }
        }
    }
}
