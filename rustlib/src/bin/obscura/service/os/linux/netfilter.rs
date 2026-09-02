//! We maintain one nftables table, which has two purposes:
//! - Restore fwmark on inbound packets of service flows.
//! - Drop non-tunnel packets that don't carry our fwmark. Exceptions documented below.
//!
//! The table carries the owner flag, so it can't be modified by other netlink sockets and the kernel destroys it when our netlink socket closes. The socket is stored in the systemd fdstore so the table survives service restarts.
//!
//! The complete ruleset, engaged, with local network access enabled:
//!
//! ```text
//! $ sudo nft list table inet obscura
//! table inet obscura { # progname obscura
//!     flags owner # dies with our netlink socket, read-only for everyone else
//!
//!     # Save the mark of outgoing service traffic to its flow's conntrack entry.
//!     chain mark-save {
//!         type filter hook postrouting priority mangle; policy accept;
//!         meta mark 0x6f627363 ct mark set meta mark
//!     }
//!
//!     # Restore the mark on inbound packets of marked flows. If strict reverse path filtering is enabled, this is required to receive API and relay response traffic while connected.
//!     chain mark-restore {
//!         type filter hook prerouting priority mangle; policy accept;
//!         ct mark 0x6f627363 meta mark set ct mark
//!     }
//!
//!     # All traffic not explicitly accepted here is dropped. This chain only exists if the target state is connected.
//!     chain kill-switch {
//!         type filter hook postrouting priority filter; policy drop;
//!         # Loopback traffic is always accepted.
//!         oifname "lo" accept
//!         # Service relay and API traffic. Setting the mark requires CAP_NET_ADMIN.
//!         meta mark 0x6f627363 accept
//!         # All traffic entering the tun device is accepted.
//!         oifname "obscuravpn" accept
//!         # Tunnel resolver traffic may only leave via the tun device.
//!         ip daddr 10.64.0.1 drop
//!         # Traffic entering other tunnel devices.
//!         meta oifkind "tun" accept
//!         meta oifkind "wireguard" accept
//!         # Link scope DHCPv4 traffic.
//!         ip daddr 255.255.255.255 udp sport 68 udp dport 67 accept
//!         # Link scope DHCPv6 traffic.
//!         ip6 daddr ff02::1:2 udp sport 546 udp dport 547 accept
//!         # IPv6 neighbor discovery.
//!         icmpv6 type nd-router-solicit accept
//!         icmpv6 type nd-neighbor-solicit accept
//!         icmpv6 type nd-neighbor-advert accept
//!         # DNS to LAN resolvers, dropped ahead of the local network accepts. Not rendered if system DNS is used.
//!         meta l4proto udp th dport 53 drop
//!         meta l4proto udp th dport 853 drop
//!         meta l4proto tcp th dport 53 drop
//!         meta l4proto tcp th dport 853 drop
//!         # Local network, rendered only if enabled.
//!         ip daddr 10.0.0.0/8 accept
//!         ip daddr 172.16.0.0/12 accept
//!         ip daddr 192.168.0.0/16 accept
//!         ip daddr 169.254.0.0/16 accept
//!         ip daddr 255.255.255.255 accept
//!         ip daddr 224.0.0.0/24 accept
//!         ip daddr 239.0.0.0/8 accept
//!         ip6 daddr fe80::/10 accept
//!         ip6 daddr fc00::/7 accept
//!         ip6 daddr ff01::/16 accept
//!         ip6 daddr ff02::/16 accept
//!         ip6 daddr ff03::/16 accept
//!         ip6 daddr ff04::/16 accept
//!         ip6 daddr ff05::/16 accept
//!     }
//! }
//! ```

use crate::service::os::linux::TrafficPolicy;
use crate::service::os::linux::fd_store::FdStore;
use nix::errno::Errno;
use nix::fcntl::{FcntlArg, OFlag, fcntl};
use nix::sys::socket::{AddressFamily, MsgFlags, NetlinkAddr, SockFlag, SockProtocol, SockType, bind, getsockname, recv, send, socket};
use obscuravpn_client::int_helper::{try_c_int_into_u8, try_c_int_into_u16, try_c_int_into_u32, u32_into_usize};
use obscuravpn_client::local_network::{LAN_V4, LAN_V6};
use obscuravpn_client::net::FWMARK;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use tokio::io::Interest;
use tokio::io::unix::AsyncFd;

const NLM_F_REQUEST: u16 = try_c_int_into_u16(libc::NLM_F_REQUEST).unwrap();
const NLM_F_ACK: u16 = try_c_int_into_u16(libc::NLM_F_ACK).unwrap();
const NLM_F_CREATE: u16 = try_c_int_into_u16(libc::NLM_F_CREATE).unwrap();
const NLM_F_APPEND: u16 = try_c_int_into_u16(libc::NLM_F_APPEND).unwrap();
const NLMSG_ERROR: u16 = try_c_int_into_u16(libc::NLMSG_ERROR).unwrap();
const NLA_F_NESTED: u16 = try_c_int_into_u16(libc::NLA_F_NESTED).unwrap();

const NFNETLINK_V0: u8 = try_c_int_into_u8(libc::NFNETLINK_V0).unwrap();
const NFNL_SUBSYS_NFTABLES: u16 = try_c_int_into_u16(libc::NFNL_SUBSYS_NFTABLES).unwrap();
const NFNL_MSG_BATCH_BEGIN: u16 = try_c_int_into_u16(libc::NFNL_MSG_BATCH_BEGIN).unwrap();
const NFNL_MSG_BATCH_END: u16 = try_c_int_into_u16(libc::NFNL_MSG_BATCH_END).unwrap();
const NFT_MSG_NEWTABLE: u16 = (NFNL_SUBSYS_NFTABLES << 8) | try_c_int_into_u16(libc::NFT_MSG_NEWTABLE).unwrap();
const NFT_MSG_NEWCHAIN: u16 = (NFNL_SUBSYS_NFTABLES << 8) | try_c_int_into_u16(libc::NFT_MSG_NEWCHAIN).unwrap();
const NFT_MSG_NEWRULE: u16 = (NFNL_SUBSYS_NFTABLES << 8) | try_c_int_into_u16(libc::NFT_MSG_NEWRULE).unwrap();
const NFT_MSG_DESTROYTABLE: u16 = (NFNL_SUBSYS_NFTABLES << 8) | 26;

const NFPROTO_UNSPEC: u8 = try_c_int_into_u8(libc::NFPROTO_UNSPEC).unwrap();
const NFPROTO_INET: u8 = try_c_int_into_u8(libc::NFPROTO_INET).unwrap();
const AF_INET: u8 = try_c_int_into_u8(libc::AF_INET).unwrap();
const AF_INET6: u8 = try_c_int_into_u8(libc::AF_INET6).unwrap();

const NF_INET_PRE_ROUTING: u32 = try_c_int_into_u32(libc::NF_INET_PRE_ROUTING).unwrap();
const NF_INET_POST_ROUTING: u32 = try_c_int_into_u32(libc::NF_INET_POST_ROUTING).unwrap();
const NF_IP_PRI_MANGLE: i32 = libc::NF_IP_PRI_MANGLE;
const NF_IP_PRI_FILTER: i32 = libc::NF_IP_PRI_FILTER;

const NF_DROP: u32 = try_c_int_into_u32(libc::NF_DROP).unwrap();
const NF_ACCEPT: u32 = try_c_int_into_u32(libc::NF_ACCEPT).unwrap();

const NFTA_TABLE_NAME: u16 = 1;
const NFTA_TABLE_FLAGS: u16 = 2;
const NFT_TABLE_F_OWNER: u32 = 0x2;

const NFTA_CHAIN_TABLE: u16 = 1;
const NFTA_CHAIN_NAME: u16 = 3;
const NFTA_CHAIN_HOOK: u16 = 4;
const NFTA_CHAIN_POLICY: u16 = 5;
const NFTA_CHAIN_TYPE: u16 = 7;
const NFTA_HOOK_HOOKNUM: u16 = 1;
const NFTA_HOOK_PRIORITY: u16 = 2;

const NFTA_RULE_TABLE: u16 = 1;
const NFTA_RULE_CHAIN: u16 = 2;
const NFTA_RULE_EXPRESSIONS: u16 = 4;
const NFTA_LIST_ELEM: u16 = 1;
const NFTA_EXPR_NAME: u16 = 1;
const NFTA_EXPR_DATA: u16 = 2;

const NFTA_DATA_VALUE: u16 = 1;
const NFTA_DATA_VERDICT: u16 = 2;
const NFTA_VERDICT_CODE: u16 = 1;

const NFT_REG_VERDICT: u32 = try_c_int_into_u32(libc::NFT_REG_VERDICT).unwrap();
const NFT_REG_1: u32 = try_c_int_into_u32(libc::NFT_REG_1).unwrap();

const NFTA_META_DREG: u16 = 1;
const NFTA_META_KEY: u16 = 2;
const NFTA_META_SREG: u16 = 3;
const NFT_META_MARK: u32 = try_c_int_into_u32(libc::NFT_META_MARK).unwrap();
const NFT_META_OIFNAME: u32 = try_c_int_into_u32(libc::NFT_META_OIFNAME).unwrap();
const NFT_META_OIFKIND: u32 = 27;
const NFT_META_NFPROTO: u32 = try_c_int_into_u32(libc::NFT_META_NFPROTO).unwrap();
const NFT_META_L4PROTO: u32 = try_c_int_into_u32(libc::NFT_META_L4PROTO).unwrap();

const NFTA_CMP_SREG: u16 = 1;
const NFTA_CMP_OP: u16 = 2;
const NFTA_CMP_DATA: u16 = 3;
const NFT_CMP_EQ: u32 = try_c_int_into_u32(libc::NFT_CMP_EQ).unwrap();

const NFTA_CT_DREG: u16 = 1;
const NFTA_CT_KEY: u16 = 2;
const NFTA_CT_SREG: u16 = 4;
const NFT_CT_MARK: u32 = try_c_int_into_u32(libc::NFT_CT_MARK).unwrap();

const NFTA_IMMEDIATE_DREG: u16 = 1;
const NFTA_IMMEDIATE_DATA: u16 = 2;

const NFTA_PAYLOAD_DREG: u16 = 1;
const NFTA_PAYLOAD_BASE: u16 = 2;
const NFTA_PAYLOAD_OFFSET: u16 = 3;
const NFTA_PAYLOAD_LEN: u16 = 4;
const NFT_PAYLOAD_NETWORK_HEADER: u32 = try_c_int_into_u32(libc::NFT_PAYLOAD_NETWORK_HEADER).unwrap();
const NFT_PAYLOAD_TRANSPORT_HEADER: u32 = try_c_int_into_u32(libc::NFT_PAYLOAD_TRANSPORT_HEADER).unwrap();

const NFTA_BITWISE_SREG: u16 = 1;
const NFTA_BITWISE_DREG: u16 = 2;
const NFTA_BITWISE_LEN: u16 = 3;
const NFTA_BITWISE_MASK: u16 = 4;
const NFTA_BITWISE_XOR: u16 = 5;

const IPPROTO_UDP: u8 = try_c_int_into_u8(libc::IPPROTO_UDP).unwrap();
const IPPROTO_TCP: u8 = try_c_int_into_u8(libc::IPPROTO_TCP).unwrap();
const IPPROTO_ICMPV6: u8 = try_c_int_into_u8(libc::IPPROTO_ICMPV6).unwrap();

const ND_ROUTER_SOLICIT: u8 = 133;
const ND_NEIGHBOR_SOLICIT: u8 = 135;
const ND_NEIGHBOR_ADVERT: u8 = 136;

const IPV4_DADDR_OFFSET: u32 = 16;
const IPV6_DADDR_OFFSET: u32 = 24;

const FD_NAME_NFT: &str = "nft";

const TABLE_NAME: &str = "obscura";
const CHAIN_MARK_SAVE: &str = "mark-save";
const CHAIN_MARK_RESTORE: &str = "mark-restore";
const CHAIN_KILL_SWITCH: &str = "kill-switch";

pub struct NftTable {
    socket: AsyncFd<OwnedFd>,
    last_unchecked_seq: Option<u32>,
}

impl NftTable {
    pub fn create_or_adopt(fd_store: &mut FdStore) -> Result<Self, ()> {
        let socket = match fd_store.take(FD_NAME_NFT) {
            Some(stored_fd) => {
                tracing::info!(message_id = "bQ6wZn3H", "adopting stored nftables socket, leaving the table untouched");
                stored_fd
            }
            None => {
                tracing::info!(message_id = "rT8kFm2W", "no stored nftables socket, creating a new one");
                let socket = socket(
                    AddressFamily::Netlink,
                    SockType::Raw,
                    SockFlag::SOCK_CLOEXEC,
                    SockProtocol::NetlinkNetFilter,
                )
                .map_err(|error| {
                    tracing::error!(message_id = "vB7mQd3J", ?error, "failed to create netfilter netlink socket");
                })?;
                bind(socket.as_raw_fd(), &NetlinkAddr::new(0, 0)).map_err(|error| {
                    tracing::error!(message_id = "nK2xWf9S", ?error, "failed to bind netfilter netlink socket");
                })?;
                fd_store.remove_old_and_store(FD_NAME_NFT, socket.as_fd());
                socket
            }
        };

        let addr: NetlinkAddr = getsockname(socket.as_raw_fd()).map_err(|error| {
            tracing::error!(message_id = "yD5rGm8C", ?error, "failed to get netlink socket address");
        })?;
        let flags = fcntl(&socket, FcntlArg::F_GETFL).map_err(|error| {
            tracing::error!(message_id = "qF6jZt2H", ?error, "failed to get netlink socket flags");
        })?;
        fcntl(&socket, FcntlArg::F_SETFL(OFlag::from_bits_retain(flags) | OFlag::O_NONBLOCK)).map_err(|error| {
            tracing::error!(message_id = "sM9cVb4L", ?error, "failed to set netlink socket nonblocking");
        })?;
        let socket = AsyncFd::new(socket).map_err(|error| {
            tracing::error!(
                message_id = "xT3nPk7W",
                ?error,
                "failed to register netlink socket with the async runtime"
            );
        })?;
        tracing::info!(message_id = "jP4vXc9L", portid = addr.pid(), "netfilter netlink socket ready");
        Ok(Self { socket, last_unchecked_seq: None })
    }

    pub async fn apply_ruleset(&mut self, policy: TrafficPolicy, tun_name: &str) -> Result<(), ()> {
        self.discard_stale_replies();
        let mut batch = Vec::new();

        let begin = Msg::new(NFNL_MSG_BATCH_BEGIN, NLM_F_REQUEST, self.next_seq(), NFPROTO_UNSPEC, NFNL_SUBSYS_NFTABLES);
        batch.extend(begin.finish());

        let mut destroy = self.change_msg(NFT_MSG_DESTROYTABLE, 0);
        destroy.attr_str(NFTA_TABLE_NAME, TABLE_NAME);
        batch.extend(destroy.finish());

        let mut table = self.change_msg(NFT_MSG_NEWTABLE, NLM_F_CREATE);
        table.attr_str(NFTA_TABLE_NAME, TABLE_NAME);
        table.attr_u32_be(NFTA_TABLE_FLAGS, NFT_TABLE_F_OWNER);
        batch.extend(table.finish());

        for Chain { name, hook, priority, policy: chain_policy, rules } in chains(&policy, tun_name) {
            let mut chain = self.change_msg(NFT_MSG_NEWCHAIN, NLM_F_CREATE);
            chain.attr_str(NFTA_CHAIN_TABLE, TABLE_NAME);
            chain.attr_str(NFTA_CHAIN_NAME, name);
            chain.attr_str(NFTA_CHAIN_TYPE, "filter");
            chain.nested(NFTA_CHAIN_HOOK, |chain| {
                chain.attr_u32_be(NFTA_HOOK_HOOKNUM, hook);
                chain.attr_u32_be(NFTA_HOOK_PRIORITY, priority.cast_unsigned());
            });
            chain.attr_u32_be(NFTA_CHAIN_POLICY, chain_policy);
            batch.extend(chain.finish());

            for exprs in rules {
                let mut rule = self.change_msg(NFT_MSG_NEWRULE, NLM_F_CREATE | NLM_F_APPEND);
                rule.attr_str(NFTA_RULE_TABLE, TABLE_NAME);
                rule.attr_str(NFTA_RULE_CHAIN, name);
                rule.nested(NFTA_RULE_EXPRESSIONS, |rule| {
                    for expr in &exprs {
                        expr.emit(rule);
                    }
                });
                batch.extend(rule.finish());
            }
        }

        let end = Msg::new(NFNL_MSG_BATCH_END, NLM_F_REQUEST, 0, NFPROTO_UNSPEC, NFNL_SUBSYS_NFTABLES);
        batch.extend(end.finish());

        tracing::info!(message_id = "wK6dPn2V", ?policy, "sending nftables batch");
        self.send_batch(&batch).await?;
        tracing::info!(message_id = "rT8jFq4X", ?policy, "checking nftables acks");
        self.check_acks().await?;
        tracing::info!(message_id = "gN7sDh5Y", ?policy, "applied nftables ruleset");
        Ok(())
    }

    async fn send_batch(&mut self, batch: &[u8]) -> Result<(), ()> {
        let sent = self
            .socket
            .async_io(Interest::WRITABLE, |socket| {
                send(socket.as_raw_fd(), batch, MsgFlags::empty()).map_err(io::Error::from)
            })
            .await
            .map_err(|error| {
                tracing::error!(message_id = "fV3kNq8Z", ?error, "failed to send nftables batch");
            })?;
        if sent != batch.len() {
            tracing::error!(message_id = "aW6tDj2P", sent, len = batch.len(), "short send of nftables batch");
            return Err(());
        }
        Ok(())
    }

    fn discard_stale_replies(&mut self) {
        self.last_unchecked_seq = None;
        let mut buf = vec![0u8; 1 << 16];
        let mut count = 0u32;
        loop {
            match recv(self.socket.get_ref().as_raw_fd(), &mut buf, MsgFlags::empty()) {
                Ok(_) => count += 1,
                Err(Errno::EWOULDBLOCK) => break,
                Err(Errno::EINTR | Errno::ENOBUFS) => {}
                Err(error) => {
                    tracing::error!(message_id = "mB4wRz7K", ?error, "failed to drain stale netlink replies");
                    break;
                }
            }
        }
        if count > 0 {
            tracing::warn!(message_id = "tV6pXk3D", count, "discarded stale netlink replies");
        }
    }

    fn change_msg(&mut self, msg_type: u16, extra_flags: u16) -> Msg {
        Msg::new(msg_type, NLM_F_REQUEST | NLM_F_ACK | extra_flags, self.next_seq(), NFPROTO_INET, 0)
    }

    fn next_seq(&mut self) -> u32 {
        let next = match self.last_unchecked_seq {
            Some(seq) => seq + 1,
            None => 1,
        };
        self.last_unchecked_seq = Some(next);
        next
    }

    async fn check_acks(&mut self) -> Result<(), ()> {
        let Some(last_unchecked_seq) = self.last_unchecked_seq else {
            tracing::error!(message_id = "vJ4qXt8N", "no outstanding netlink seq to await");
            return Err(());
        };
        let mut buf = vec![0u8; 1 << 16];
        loop {
            let n = self
                .socket
                .async_io(Interest::READABLE, |socket| {
                    recv(socket.as_raw_fd(), &mut buf, MsgFlags::empty()).map_err(io::Error::from)
                })
                .await
                .map_err(|error| {
                    tracing::error!(message_id = "zR9mBc5X", ?error, "failed to receive nftables reply");
                })?;
            let mut offset = 0;
            while offset + 16 <= n {
                let len = u32_into_usize(u32::from_ne_bytes(buf[offset..offset + 4].try_into().unwrap()));
                let msg_type = u16::from_ne_bytes(buf[offset + 4..offset + 6].try_into().unwrap());
                let seq = u32::from_ne_bytes(buf[offset + 8..offset + 12].try_into().unwrap());
                if len < 16 || len > n - offset {
                    tracing::error!(message_id = "eK4sVw7H", len, offset, n, "truncated netlink message");
                    return Err(());
                }
                if msg_type == NLMSG_ERROR {
                    if len < 20 {
                        tracing::error!(message_id = "tG2xLp6M", len, "truncated netlink error message");
                        return Err(());
                    }
                    let code = i32::from_ne_bytes(buf[offset + 16..offset + 20].try_into().unwrap());
                    if code != 0 {
                        tracing::error!(message_id = "dQ8nFy3C", seq, error = ?io::Error::from_raw_os_error(-code), "nftables batch rejected");
                        return Err(());
                    } else if seq == last_unchecked_seq {
                        self.last_unchecked_seq = None;
                        return Ok(());
                    }
                }
                offset += len.next_multiple_of(4);
            }
        }
    }
}

struct Chain {
    name: &'static str,
    hook: u32,
    priority: i32,
    policy: u32,
    rules: Vec<Vec<Expr>>,
}

fn chains(policy: &TrafficPolicy, tun_name: &str) -> Vec<Chain> {
    use Expr::*;
    let mark = FWMARK.to_ne_bytes().to_vec();
    let mut chains = vec![
        Chain {
            name: CHAIN_MARK_SAVE,
            hook: NF_INET_POST_ROUTING,
            priority: NF_IP_PRI_MANGLE,
            policy: NF_ACCEPT,
            rules: vec![vec![MetaLoad(NFT_META_MARK), CmpEq(mark.clone()), CtSetMark]],
        },
        Chain {
            name: CHAIN_MARK_RESTORE,
            hook: NF_INET_PRE_ROUTING,
            priority: NF_IP_PRI_MANGLE,
            policy: NF_ACCEPT,
            rules: vec![vec![CtLoadMark, CmpEq(mark), MetaSetMark]],
        },
    ];
    match policy {
        TrafficPolicy::Engage { local_network_access, dns, use_system_dns } => {
            chains.push(kill_switch_chain(*local_network_access, dns, *use_system_dns, tun_name))
        }
        TrafficPolicy::Disengage => {}
    }
    chains
}

fn kill_switch_chain(local_network_access: bool, dns: &[IpAddr], use_system_dns: bool, tun_name: &str) -> Chain {
    use Expr::*;
    let mut rules = vec![
        vec![MetaLoad(NFT_META_OIFNAME), CmpEq(b"lo\0".to_vec()), Accept],
        vec![MetaLoad(NFT_META_MARK), CmpEq(FWMARK.to_ne_bytes().to_vec()), Accept],
        vec![MetaLoad(NFT_META_OIFNAME), CmpEq(nul_terminated(tun_name)), Accept],
    ];
    for ip in dns {
        rules.push(match ip {
            IpAddr::V4(ip) => daddr_rule(AF_INET, IPV4_DADDR_OFFSET, ip.octets().to_vec(), None, Drop),
            IpAddr::V6(ip) => daddr_rule(AF_INET6, IPV6_DADDR_OFFSET, ip.octets().to_vec(), None, Drop),
        });
    }
    rules.extend([
        vec![MetaLoad(NFT_META_OIFKIND), CmpEq(b"tun\0".to_vec()), Accept],
        vec![MetaLoad(NFT_META_OIFKIND), CmpEq(b"wireguard\0".to_vec()), Accept],
        dhcp_rule(AF_INET, IPV4_DADDR_OFFSET, Ipv4Addr::BROADCAST.octets().to_vec(), 68, 67),
        dhcp_rule(
            AF_INET6,
            IPV6_DADDR_OFFSET,
            Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 1, 2).octets().to_vec(),
            546,
            547,
        ),
    ]);
    for nd_type in [ND_ROUTER_SOLICIT, ND_NEIGHBOR_SOLICIT, ND_NEIGHBOR_ADVERT] {
        rules.push(vec![
            MetaLoad(NFT_META_NFPROTO),
            CmpEq(vec![AF_INET6]),
            MetaLoad(NFT_META_L4PROTO),
            CmpEq(vec![IPPROTO_ICMPV6]),
            Payload { base: NFT_PAYLOAD_TRANSPORT_HEADER, offset: 0, len: 1 },
            CmpEq(vec![nd_type]),
            Accept,
        ]);
    }
    if !use_system_dns {
        for l4proto in [IPPROTO_UDP, IPPROTO_TCP] {
            for port in [53u16, 853] {
                rules.push(vec![
                    MetaLoad(NFT_META_L4PROTO),
                    CmpEq(vec![l4proto]),
                    Payload { base: NFT_PAYLOAD_TRANSPORT_HEADER, offset: 2, len: 2 },
                    CmpEq(port.to_be_bytes().to_vec()),
                    Drop,
                ]);
            }
        }
    }
    if local_network_access {
        for net in LAN_V4 {
            rules.push(daddr_rule(
                AF_INET,
                IPV4_DADDR_OFFSET,
                net.network().octets().to_vec(),
                (net.prefix() < 32).then(|| net.mask().octets().to_vec()),
                Accept,
            ));
        }
        for net in LAN_V6 {
            rules.push(daddr_rule(
                AF_INET6,
                IPV6_DADDR_OFFSET,
                net.network().octets().to_vec(),
                (net.prefix() < 128).then(|| net.mask().octets().to_vec()),
                Accept,
            ));
        }
    }
    Chain {
        name: CHAIN_KILL_SWITCH,
        hook: NF_INET_POST_ROUTING,
        priority: NF_IP_PRI_FILTER,
        policy: NF_DROP,
        rules,
    }
}

fn dhcp_rule(nfproto: u8, daddr_offset: u32, daddr: Vec<u8>, sport: u16, dport: u16) -> Vec<Expr> {
    use Expr::*;
    let daddr_len = u32::try_from(daddr.len()).unwrap();
    vec![
        MetaLoad(NFT_META_NFPROTO),
        CmpEq(vec![nfproto]),
        MetaLoad(NFT_META_L4PROTO),
        CmpEq(vec![IPPROTO_UDP]),
        Payload { base: NFT_PAYLOAD_NETWORK_HEADER, offset: daddr_offset, len: daddr_len },
        CmpEq(daddr),
        Payload { base: NFT_PAYLOAD_TRANSPORT_HEADER, offset: 0, len: 2 },
        CmpEq(sport.to_be_bytes().to_vec()),
        Payload { base: NFT_PAYLOAD_TRANSPORT_HEADER, offset: 2, len: 2 },
        CmpEq(dport.to_be_bytes().to_vec()),
        Accept,
    ]
}

fn daddr_rule(nfproto: u8, offset: u32, network: Vec<u8>, mask: Option<Vec<u8>>, verdict: Expr) -> Vec<Expr> {
    use Expr::*;
    let len = u32::try_from(network.len()).unwrap();
    let mut exprs = vec![
        MetaLoad(NFT_META_NFPROTO),
        CmpEq(vec![nfproto]),
        Payload { base: NFT_PAYLOAD_NETWORK_HEADER, offset, len },
    ];
    if let Some(mask) = mask {
        exprs.push(BitwiseMask(mask));
    }
    exprs.extend([CmpEq(network), verdict]);
    exprs
}

fn nul_terminated(value: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(value.len() + 1);
    bytes.extend_from_slice(value.as_bytes());
    bytes.push(0);
    bytes
}

enum Expr {
    MetaLoad(u32),
    MetaSetMark,
    CmpEq(Vec<u8>),
    CtLoadMark,
    CtSetMark,
    Accept,
    Drop,
    Payload { base: u32, offset: u32, len: u32 },
    BitwiseMask(Vec<u8>),
}

impl Expr {
    fn emit(&self, msg: &mut Msg) {
        match self {
            Expr::MetaLoad(key) => expr(msg, "meta", |data| {
                data.attr_u32_be(NFTA_META_KEY, *key);
                data.attr_u32_be(NFTA_META_DREG, NFT_REG_1);
            }),
            Expr::MetaSetMark => expr(msg, "meta", |data| {
                data.attr_u32_be(NFTA_META_KEY, NFT_META_MARK);
                data.attr_u32_be(NFTA_META_SREG, NFT_REG_1);
            }),
            Expr::CmpEq(value) => expr(msg, "cmp", |data| {
                data.attr_u32_be(NFTA_CMP_SREG, NFT_REG_1);
                data.attr_u32_be(NFTA_CMP_OP, NFT_CMP_EQ);
                data.nested(NFTA_CMP_DATA, |data| data.attr(NFTA_DATA_VALUE, value));
            }),
            Expr::CtLoadMark => expr(msg, "ct", |data| {
                data.attr_u32_be(NFTA_CT_KEY, NFT_CT_MARK);
                data.attr_u32_be(NFTA_CT_DREG, NFT_REG_1);
            }),
            Expr::CtSetMark => expr(msg, "ct", |data| {
                data.attr_u32_be(NFTA_CT_KEY, NFT_CT_MARK);
                data.attr_u32_be(NFTA_CT_SREG, NFT_REG_1);
            }),
            Expr::Accept => expr(msg, "immediate", |data| {
                data.attr_u32_be(NFTA_IMMEDIATE_DREG, NFT_REG_VERDICT);
                data.nested(NFTA_IMMEDIATE_DATA, |data| {
                    data.nested(NFTA_DATA_VERDICT, |data| data.attr_u32_be(NFTA_VERDICT_CODE, NF_ACCEPT));
                });
            }),
            Expr::Drop => expr(msg, "immediate", |data| {
                data.attr_u32_be(NFTA_IMMEDIATE_DREG, NFT_REG_VERDICT);
                data.nested(NFTA_IMMEDIATE_DATA, |data| {
                    data.nested(NFTA_DATA_VERDICT, |data| data.attr_u32_be(NFTA_VERDICT_CODE, NF_DROP));
                });
            }),
            Expr::Payload { base, offset, len } => expr(msg, "payload", |data| {
                data.attr_u32_be(NFTA_PAYLOAD_DREG, NFT_REG_1);
                data.attr_u32_be(NFTA_PAYLOAD_BASE, *base);
                data.attr_u32_be(NFTA_PAYLOAD_OFFSET, *offset);
                data.attr_u32_be(NFTA_PAYLOAD_LEN, *len);
            }),
            Expr::BitwiseMask(mask) => expr(msg, "bitwise", |data| {
                data.attr_u32_be(NFTA_BITWISE_SREG, NFT_REG_1);
                data.attr_u32_be(NFTA_BITWISE_DREG, NFT_REG_1);
                data.attr_u32_be(NFTA_BITWISE_LEN, u32::try_from(mask.len()).unwrap());
                data.nested(NFTA_BITWISE_MASK, |data| data.attr(NFTA_DATA_VALUE, mask));
                data.nested(NFTA_BITWISE_XOR, |data| data.attr(NFTA_DATA_VALUE, &vec![0u8; mask.len()]));
            }),
        }
    }
}

fn expr(msg: &mut Msg, name: &str, data: impl FnOnce(&mut Msg)) {
    msg.nested(NFTA_LIST_ELEM, |elem| {
        elem.attr_str(NFTA_EXPR_NAME, name);
        elem.nested(NFTA_EXPR_DATA, data);
    });
}

/// One netlink message: nlmsghdr, nfgenmsg, attributes. Length fields are patched in `finish`/`nested`.
struct Msg {
    buf: Vec<u8>,
}

impl Msg {
    fn new(msg_type: u16, flags: u16, seq: u32, family: u8, res_id: u16) -> Self {
        let mut buf = Vec::with_capacity(256);
        buf.extend(0u32.to_ne_bytes());
        buf.extend(msg_type.to_ne_bytes());
        buf.extend(flags.to_ne_bytes());
        buf.extend(seq.to_ne_bytes());
        buf.extend(0u32.to_ne_bytes());
        buf.push(family);
        buf.push(NFNETLINK_V0);
        buf.extend(res_id.to_be_bytes());
        Self { buf }
    }

    fn attr(&mut self, kind: u16, data: &[u8]) {
        let len = u16::try_from(4 + data.len()).unwrap();
        self.buf.extend(len.to_ne_bytes());
        self.buf.extend(kind.to_ne_bytes());
        self.buf.extend(data);
        self.pad();
    }

    fn attr_u32_be(&mut self, kind: u16, value: u32) {
        self.attr(kind, &value.to_be_bytes());
    }

    fn attr_str(&mut self, kind: u16, value: &str) {
        self.attr(kind, &nul_terminated(value));
    }

    fn nested(&mut self, kind: u16, content: impl FnOnce(&mut Self)) {
        let start = self.buf.len();
        self.buf.extend(0u16.to_ne_bytes());
        self.buf.extend((kind | NLA_F_NESTED).to_ne_bytes());
        content(self);
        let len = u16::try_from(self.buf.len() - start).unwrap();
        self.buf[start..start + 2].copy_from_slice(&len.to_ne_bytes());
    }

    fn pad(&mut self) {
        while self.buf.len() % 4 != 0 {
            self.buf.push(0);
        }
    }

    fn finish(mut self) -> Vec<u8> {
        let len = u32::try_from(self.buf.len()).unwrap();
        self.buf[0..4].copy_from_slice(&len.to_ne_bytes());
        self.buf
    }
}
