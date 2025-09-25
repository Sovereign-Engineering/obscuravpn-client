use etherparse::{IcmpEchoHeader, Icmpv4Type, PacketBuilder, SlicedPacket, TransportSlice};
use rand::{RngCore, thread_rng};
use static_assertions::{const_assert, const_assert_ne};
use std::cmp::min;
use std::collections::VecDeque;
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

const MAX_ALLOWED_LOST_PROBES: usize = 4;
const MAX_ALLOWED_LOST_PROBES_AFTER_SLEEP: usize = 1;
const BUSY_PING_PERIOD: Duration = Duration::from_secs(1);
const IDLE_PING_PERIOD: Duration = Duration::from_secs(55);
const_assert_ne!(
    0,
    crate::quicwg::QUIC_IDLE_TIMEOUT
        .saturating_sub(IDLE_PING_PERIOD)
        .saturating_sub(Duration::from_millis(4999))
        .as_millis()
);
const PROBE_LOST_PERIOD: Duration = Duration::from_secs(1);

// Randomly generated value to reliably distinguish our probes from other pings
const PROBE_PREFIX: &[u8; 32] = b"obs-ping\x75\xf8\xb9\x47\x4b\xe1\x61\xeb\x1c\xb1\xeb\x5e\xc0\x6c\xde\xb7\xa1\x1b\x7b\xe5\x85\xca\x3a\x95";

pub struct LivenessChecker {
    next_id_seq: u32,
    mtu: u16,
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    sent_user_traffic_since_last_ping: bool,
    is_waking: bool,
    outstanding_pongs: VecDeque<Ping>,
    last_ping_sent_at: Option<Instant>,
}

struct Ping {
    sent_at: Instant,
    payload: Vec<u8>,
}

impl LivenessChecker {
    pub fn new(mtu: u16, client_ip: Ipv4Addr, ping_target_ip: Ipv4Addr) -> Self {
        Self {
            next_id_seq: 0,
            mtu,
            src_ip: client_ip,
            dst_ip: ping_target_ip,
            sent_user_traffic_since_last_ping: false,
            is_waking: false,
            outstanding_pongs: Default::default(),
            last_ping_sent_at: None,
        }
    }

    // returns the number of likely lost probes, as well as when the next one would be considered lost
    fn lost_probe_count_and_time_of_next_loss(&self, now: Instant) -> (usize, Option<Instant>) {
        const_assert!(PROBE_LOST_PERIOD.as_nanos() <= BUSY_PING_PERIOD.as_nanos());
        const_assert!(PROBE_LOST_PERIOD.as_nanos() <= IDLE_PING_PERIOD.as_nanos());
        if let Some(last) = self.outstanding_pongs.back() {
            let last_expires_at = last.sent_at + PROBE_LOST_PERIOD;
            if last_expires_at > now {
                return (self.outstanding_pongs.len() - 1, Some(last_expires_at));
            }
        }
        (self.outstanding_pongs.len(), None)
    }

    // Call when sending a packet that does not originate from the liveness checker. May return a packet for sending.
    #[must_use = "may return a packet, which needs to be sent"]
    pub fn sent_traffic(&mut self) -> Option<Vec<u8>> {
        let now = Instant::now();
        if self.last_ping_sent_at.is_none_or(|last_ping| now > last_ping + BUSY_PING_PERIOD) {
            // Ping is overdue. Don't wait for next poll call.
            tracing::info!(message_id = "k5jg6f3w", "liveness checker sent_traffic returning packet");
            return Some(self.send_ping(now));
        }
        self.sent_user_traffic_since_last_ping = true;
        None
    }

    // Call after sleep. Reduces the number of lost probes needed to classify as dead until a probe succeeded. Returns a packet for sending.
    #[allow(unused)] // TODO: Call on wake - https://linear.app/soveng/issue/OBS-2311/sleep-robust-tunnels-remove-quinn-keepalive-use-wake-aware-ping
    #[must_use = "the returned packet needs to be sent"]
    pub fn wake(&mut self) -> Vec<u8> {
        tracing::info!(message_id = "OsZ6HBJO", "liveness checker wake called");
        let now = Instant::now();
        // Instants may or may not continue ticking during system sleep. Reset the whole state.
        *self = Self::new(self.mtu, self.src_ip, self.dst_ip);
        self.is_waking = true;
        // Immediately test connection after wake.
        self.send_ping(now)
    }

    pub fn poll(&mut self) -> LivenessCheckerPoll {
        let now = Instant::now();

        let (lost_probes, next_probe_loss) = self.lost_probe_count_and_time_of_next_loss(now);
        let max_lost_probes = if self.is_waking {
            MAX_ALLOWED_LOST_PROBES_AFTER_SLEEP
        } else {
            MAX_ALLOWED_LOST_PROBES
        };
        if lost_probes > max_lost_probes {
            tracing::error!(
                message_id = "2sonYhc2",
                lost_probes,
                max_lost_probes,
                "liveness checker poll returning Dead"
            );
            return LivenessCheckerPoll::Dead;
        }

        let ping_period = if self.sent_user_traffic_since_last_ping || lost_probes != 0 {
            BUSY_PING_PERIOD
        } else {
            IDLE_PING_PERIOD
        };
        tracing::info!(
            message_id = "KZjNGhxu",
            lost_probes,
            max_lost_probes,
            since_last_ping_ms = ?self.last_ping_sent_at.map(|i| now.saturating_duration_since(i).as_millis()),
            until_next_probe_loss_ms = ?next_probe_loss.map(|i| i.saturating_duration_since(now).as_millis()),
            ping_period_ms = ping_period.as_millis(),
            "liveness checker probe loss ok"
        );

        if self.last_ping_sent_at.is_none_or(|last_ping| last_ping + ping_period <= now) {
            tracing::info!(message_id = "7UnUaqos", "liveness checker poll returning SendPacket",);
            return LivenessCheckerPoll::SendPacket(self.send_ping(now));
        }

        const_assert!(BUSY_PING_PERIOD.as_nanos() <= IDLE_PING_PERIOD.as_nanos());
        let mut next_poll = now + BUSY_PING_PERIOD;
        if let Some(next_probe_loss) = next_probe_loss {
            next_poll = min(next_poll, next_probe_loss)
        }
        if let Some(last_ping_sent_at) = self.last_ping_sent_at {
            next_poll = min(next_poll, last_ping_sent_at + ping_period)
        }
        tracing::info!(
            message_id = "Yd79pARH",
            until_next_poll_ms = next_poll.saturating_duration_since(now).as_millis(),
            "liveness checker poll returning AliveUntil",
        );
        LivenessCheckerPoll::AliveUntil(next_poll)
    }

    // Checks if a packet is an expected probe response and returns the probe latency if it is.
    pub fn process_potential_probe_response(&mut self, packet: &[u8]) -> Option<Duration> {
        let now = Instant::now();
        let ip = SlicedPacket::from_ip(packet).ok()?;
        let Some(TransportSlice::Icmpv4(icmp)) = ip.transport else { return None };
        let pong_id_seq = {
            let Icmpv4Type::EchoReply(IcmpEchoHeader { id, seq }) = icmp.icmp_type() else {
                return None;
            };
            let id = id.to_be_bytes();
            let seq = seq.to_be_bytes();
            u32::from_be_bytes([id[0], id[1], seq[0], seq[1]])
        };
        if !icmp.payload().starts_with(PROBE_PREFIX) {
            return None;
        }
        let last_sent_id_seq = self.next_id_seq.wrapping_sub(1);
        let mut matched_pong_index = None;
        for (i, Ping { payload, .. }) in self.outstanding_pongs.iter().enumerate() {
            if payload == icmp.payload() {
                matched_pong_index = Some(i);
                break;
            }
        }
        if let Some(matched_pong_index) = matched_pong_index {
            let sent_at = self.outstanding_pongs[matched_pong_index].sent_at;
            let probe_rtt = now.checked_duration_since(sent_at).unwrap_or_default();
            tracing::info!(
                message_id = "ETUFSKaF",
                pong_id_seq,
                last_sent_id_seq,
                ?probe_rtt,
                "received liveness checker pong"
            );
            self.outstanding_pongs.drain(0..=matched_pong_index);
            self.is_waking = false;
            Some(probe_rtt)
        } else {
            tracing::info!(
                message_id = "tDMDB46X",
                pong_id_seq,
                last_sent_id_seq,
                "ignoring liveness checker pong with unrecognized payload"
            );
            None
        }
    }

    fn send_ping(&mut self, now: Instant) -> Vec<u8> {
        self.last_ping_sent_at = Some(now);
        self.sent_user_traffic_since_last_ping = false;

        let id_seq = self.next_id_seq.to_be_bytes();
        self.next_id_seq += 1;
        let id = u16::from_be_bytes(id_seq[0..2].try_into().unwrap());
        let seq = u16::from_be_bytes(id_seq[2..4].try_into().unwrap());
        let builder = PacketBuilder::ipv4(self.src_ip.octets(), self.dst_ip.octets(), 255).icmpv4_echo_request(id, seq);
        let mut payload: Vec<u8> = vec![0; self.mtu as usize - 28];
        payload[0..32].copy_from_slice(PROBE_PREFIX);
        thread_rng().fill_bytes(&mut payload[32..]);
        let mut packet = Vec::<u8>::with_capacity(builder.size(payload.len()));
        builder.write(&mut packet, &payload).unwrap();
        self.outstanding_pongs.push_back(Ping { sent_at: now, payload });

        packet
    }
}

#[must_use = "this `LivenessCheckerPoll` may need to be handled"]
#[derive(Debug)]
pub enum LivenessCheckerPoll {
    Dead,
    AliveUntil(Instant),
    SendPacket(Vec<u8>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_packet_size() {
        let mut checker = LivenessChecker::new(1234, Ipv4Addr::LOCALHOST, Ipv4Addr::LOCALHOST);
        assert_eq!(checker.send_ping(Instant::now()).len(), usize::from(checker.mtu));
    }
}
